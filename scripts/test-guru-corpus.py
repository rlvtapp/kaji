"""Offline regression checks for corpus integrity, failure reporting and cleanup."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

loader = importlib.util.spec_from_file_location('guru_corpus', Path(__file__).with_name('guru-corpus.py'))
corpus = importlib.util.module_from_spec(loader)
loader.loader.exec_module(corpus)


class CorpusTests(unittest.TestCase):
    def test_manifest_is_large_diverse_and_immutable(self):
        data = json.loads((corpus.PROJECT / 'scripts/fixtures/guru-contracts.json').read_text())
        values = data['contracts']
        self.assertEqual(len(values), 205)
        self.assertEqual(len({v['provider'] for v in values}), 201)
        self.assertEqual(sum(v['provider'] == 'azure.com' for v in values), 5)
        self.assertTrue(all(value['bytes'] >= 1000000 for value in values[:32]))
        for value in values:
            self.assertGreaterEqual(value['bytes'], 100000)
            self.assertLessEqual(value['bytes'], value['max_bytes'])
            self.assertRegex(value['sha256'], r'^[0-9a-f]{64}$')
            self.assertIn('/' + data['catalog_revision'] + '/', value['url'])

    def reference_fixture(self, root, path="models/Child.yml", digest=None):
        cache = root / 'cache'
        cache.mkdir()
        data = b"openapi: 3.0.3\ninfo: {title: Test, version: '1'}\npaths: {}\n"
        (cache / 'sample.yml').write_bytes(data)
        helper = b"type: string\n"
        (cache / 'sample/models').mkdir(parents=True)
        (cache / 'sample/models/Child.yml').write_bytes(helper)
        contract = {'name': 'sample', 'url': 'https://example.invalid/root',
                    'sha256': hashlib.sha256(data).hexdigest(),
                    'references': [{'path': path, 'url': 'https://example.invalid/helper',
                                    'sha256': digest or hashlib.sha256(helper).hexdigest()}]}
        manifest = root / 'manifest.json'
        manifest.write_text(json.dumps({'contracts': [contract]}))
        return manifest, cache, contract, data, helper

    def test_supplemental_reference_tree_preserves_root_and_helper_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, cache, contract, data, helper = self.reference_fixture(root)
            corpus.public.fetch(manifest, root / 'output', cache)
            self.assertEqual(corpus.public.source_path(root / 'output', contract).read_bytes(), data)
            self.assertEqual((root / 'output/sample/models/Child.yml').read_bytes(), helper)

    def test_corrupt_reference_fails_before_assembling_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, cache, contract, _, _ = self.reference_fixture(root, digest='0' * 64)
            with self.assertRaisesRegex(ValueError, 'checksum'):
                corpus.public.fetch(manifest, root / 'output', cache)
            self.assertFalse(corpus.public.source_path(root / 'output', contract).exists())

    def test_reference_path_cannot_escape_contract_tree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest, cache, _, _, _ = self.reference_fixture(root, path='../escaped.yml')
            with self.assertRaisesRegex(ValueError, 'Unsafe'):
                corpus.public.fetch(manifest, root / 'output', cache)
            self.assertFalse((root / 'output/escaped.yml').exists())

    def fixture(self, root):
        (root / 'work').mkdir()
        (root / 'specs').mkdir()
        return {'name': 'sample', 'url': 'https://example.invalid/pinned', 'sha256': 'a' * 64, 'bytes': 1000000}

    def test_generation_failure_is_not_counted_as_native_coverage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            contract = self.fixture(root)
            with patch.object(corpus, 'run_logged', return_value={'exit_code': 2}) as run:
                result = corpus.run_case(contract, 'go', root / 'manifest', root, {}, 10, False)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(result['status'], 'failed')
            self.assertIsNone(result['native'])
            self.assertEqual(result['source_sha256'], contract['sha256'])
            self.assertEqual(list((root / 'work').iterdir()), [])

    def test_native_failure_retains_metadata_and_cleans_only_owned_scratch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            contract = self.fixture(root)
            sentinel = root / 'customer.txt'
            sentinel.write_text('preserve me')
            def run(command, env, log, timeout):
                if command[-1] == 'generate':
                    sdk = Path(env['KAJI_PUBLIC_CONTRACT_ROOT']) / 'sample'
                    (sdk / 'go').mkdir(parents=True)
                    (sdk / 'go/client.go').write_text('package sdk')
                    (sdk / '.kaji').mkdir()
                    (sdk / '.kaji/generation.lock.json').write_text('{}')
                    return {'exit_code': 0}
                return {'exit_code': 42}
            with patch.object(corpus, 'run_logged', side_effect=run):
                result = corpus.run_case(contract, 'go', root / 'manifest', root, {}, 10, False)
            self.assertEqual(result['status'], 'failed')
            self.assertEqual(result['native']['exit_code'], 42)
            self.assertEqual(result['generated_files'], 1)
            self.assertTrue((root / 'metadata/sample/go/.kaji/generation.lock.json').exists())
            self.assertEqual(sentinel.read_text(), 'preserve me')
            self.assertEqual(list((root / 'work').iterdir()), [])

    def test_corpus_continues_after_failure_and_returns_nonzero(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'run'
            selected = [{'name': 'first'}, {'name': 'second'}]
            with patch.object(corpus.public, 'contracts', return_value=selected), \
                 patch.object(corpus.public, 'fetch'), \
                 patch.object(corpus, 'run_case', side_effect=[{'status': 'failed'}, {'status': 'passed'}]) as run:
                result = corpus.main(['--output', str(root), '--language', 'go'])
            self.assertEqual(result, 1)
            self.assertEqual(run.call_count, 2)
            report = json.loads((root / 'report.json').read_text())
            self.assertEqual((report['passed'], report['failed']), (1, 1))

    def test_timeout_is_explicit_in_report_and_log(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / 'probe.log'
            result = corpus.run_logged([sys.executable, '-c', 'import time; time.sleep(1)'],
                                       dict(os.environ), log, 0.01)
            self.assertTrue(result['timed_out'])
            self.assertIsNone(result['exit_code'])
            self.assertIn('time limit', log.read_text())

    def test_existing_output_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / 'report.json'
            marker.write_text('customer data')
            with self.assertRaises(FileExistsError):
                corpus.main(['--output', directory, '--language', 'go'])
            self.assertEqual(marker.read_text(), 'customer data')


if __name__ == '__main__':
    unittest.main()
