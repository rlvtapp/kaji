"""Offline layout-audit regression tests; no compiler or dependency installation."""
import importlib.util
import json
import os
import subprocess
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

loader = importlib.util.spec_from_file_location('layout_audit', Path(__file__).with_name('layout-audit.py'))
audit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(audit)


class LayoutAuditTests(unittest.TestCase):
    def test_source_size_ratchet_rejects_new_oversized_files_and_legacy_growth(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'crates/example/src'
            source.mkdir(parents=True)
            old = source / 'legacy.rs'
            new = source / 'new.rs'
            old.write_text('x\n' * 450)
            new.write_text('x\n' * 401)
            baseline = root / 'baseline.json'
            baseline.write_text(json.dumps({'legacy_line_budgets': {'crates/example/src/legacy.rs': 450}}))
            report = audit.source_size_audit(root, baseline)
            self.assertEqual([item['path'] for item in report['violations']], ['crates/example/src/new.rs'])
            old.write_text('x\n' * 451)
            report = audit.source_size_audit(root, baseline)
            self.assertEqual({item['path'] for item in report['violations']},
                             {'crates/example/src/legacy.rs', 'crates/example/src/new.rs'})

    def fake_generation(self, mutate=False, remove_authored=False):
        generations = []
        def run(command, environment, log, timeout):
            config = json.loads(Path(command[command.index('--config') + 1]).read_text())
            package = config['packages'][0]
            output = Path(config['output']['path']) / package['path']
            output.mkdir(parents=True, exist_ok=True)
            generations.append(package['plugins'][0].get('jobs'))
            (output / 'client.go').write_text('package probe' + (' changed' if mutate and len(generations) == 2 else ''))
            sentinel = output / 'layout-audit-authored.txt'
            if remove_authored and sentinel.exists():
                sentinel.unlink()
            return {'exit_code': 0, 'seconds': .01,
                    'peak_memory': {'status': 'measured', 'peak_bytes': 123456}}
        return run, generations

    def run_case(self, root, run):
        source = root / 'source.yaml'
        source.write_text('openapi: 3.1.0')
        with patch.object(audit.corpus, 'run_logged', side_effect=run):
            return audit.audit_case(source, 'sample', 'go', root, Path('/poolster'), {}, 10, 10)

    def test_repeat_generation_keeps_overlay_and_compares_worker_counts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run, jobs = self.fake_generation()
            result = self.run_case(root, run)
            self.assertEqual(result['status'], 'passed')
            self.assertEqual(jobs, [1, 1, 2])
            self.assertTrue(result['authored_file_preserved'])
            self.assertEqual(result['worker_determinism']['worker_counts'], [1, 2])
            self.assertEqual(result['generation']['peak_memory']['peak_bytes'], 123456)
            self.assertEqual(result['native']['status'], 'unavailable')

    def test_byte_drift_fails_even_when_all_generation_commands_succeed(self):
        with tempfile.TemporaryDirectory() as directory:
            run, _ = self.fake_generation(mutate=True)
            result = self.run_case(Path(directory), run)
            self.assertEqual(result['status'], 'failed')
            self.assertEqual(result['regeneration']['changed_paths'], ['client.go'])

    def test_customer_overlay_deletion_is_a_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            run, _ = self.fake_generation(remove_authored=True)
            result = self.run_case(Path(directory), run)
            self.assertEqual(result['status'], 'failed')
            self.assertFalse(result['authored_file_preserved'])

    def test_native_check_never_installs_missing_dependencies(self):
        with tempfile.TemporaryDirectory() as directory:
            result = audit.native_check('auxiliary', Path(directory), {}, Path(directory) / 'log', 10)
            self.assertEqual(result['status'], 'unavailable')
            self.assertIn('node_modules', result['reason'])

    @unittest.skipUnless(os.environ.get('POOLSTER_TEST_LAYOUT_BIN'), 'set POOLSTER_TEST_LAYOUT_BIN for actual generator cleanup probe')
    def test_real_generation_removes_stale_chunks_and_preserves_authored_file(self):
        binary = Path(os.environ['POOLSTER_TEST_LAYOUT_BIN']).resolve()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source.json'
            def write_source(count):
                source.write_text(json.dumps({'openapi': '3.0.3', 'info': {'title': 'Chunk Probe', 'version': '1'},
                    'paths': {'/items/' + str(i): {'get': {'operationId': 'getItem' + str(i),
                              'responses': {'200': {'description': 'ok'}}}} for i in range(count)}}))
            write_source(205)
            config = root / 'poolster.json'
            config.write_text(json.dumps({'openapi': {'input': str(source)},
                'output': {'path': str(root / 'generated')},
                'packages': [{'language': 'typescript-cli', 'path': 'cli',
                              'plugins': [{'name': 'cli'}]}]}))
            command = [str(binary), 'generate', '--config', str(config), '--color', 'never']
            first = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(first.returncode, 0, first.stderr)
            output = root / 'generated/cli'
            before = {p.relative_to(output) for p in output.rglob('*.ts')}
            self.assertGreater(len(before), 4)
            authored = output / 'customer.txt'
            authored.write_text('preserve authored overlay')
            write_source(1)
            shrink = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(shrink.returncode, 0, shrink.stderr)
            after = {p.relative_to(output) for p in output.rglob('*.ts')}
            removed = before - after
            self.assertTrue(removed, 'shrinking across chunk boundary must remove stale files')
            self.assertTrue(all(not (output / path).exists() for path in removed))
            self.assertEqual(authored.read_text(), 'preserve authored overlay')
            self.assertFalse(any('getItem204' in p.read_text() for p in output.rglob('*.ts')))
            snapshot = audit.corpus.output_fingerprint(output)
            repeat = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(repeat.returncode, 0, repeat.stderr)
            self.assertEqual(snapshot, audit.corpus.output_fingerprint(output))

    def test_auxiliary_profile_explicitly_includes_every_audited_generator(self):
        package = audit.package('auxiliary')
        self.assertEqual([plugin['name'] for plugin in package['plugins']], ['sdk', *audit.AUXILIARY])


if __name__ == '__main__':
    unittest.main()
