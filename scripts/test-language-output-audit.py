"""Offline checks for truthful language output audit reporting."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

loader = importlib.util.spec_from_file_location('audit', Path(__file__).with_name('language-output-audit.py'))
audit = importlib.util.module_from_spec(loader)
loader.loader.exec_module(audit)


class OutputAuditTests(unittest.TestCase):
    def test_final_source_bytes_and_newlines_are_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'model.py').write_bytes(b'class Model:\r\n    pass')
            result = audit.statistics(root, 8)
            self.assertEqual(len(result['violations']), 1)
            file = result['source_files'][0]
            self.assertFalse(file['final_newline'])
            self.assertFalse(file['lf_only'])
            self.assertFalse(file['within_budget'])

    def test_empty_package_markers_are_valid_source_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / '__init__.py').write_bytes(b'')
            self.assertEqual(audit.statistics(root, 1)['violations'], [])

    def test_size_exception_does_not_excuse_bad_line_endings(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'model.php').write_bytes(b'<?php\r\n')
            (root / 'source-layout-diagnostics.json').write_text(json.dumps([
                {'path': 'model.php', 'reason': 'atomic declaration'}]))
            result = audit.statistics(root, 1)
            self.assertTrue(result['source_files'][0]['size_exception'])
            self.assertEqual(len(result['violations']), 1)

    def test_determinism_compares_the_file_set_and_contents(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'a.ts').write_text('export {};\n')
            first = audit.fingerprint(root)
            self.assertEqual(first, audit.fingerprint(root))
            (root / 'b.ts').write_text('export {};\n')
            self.assertNotEqual(first, audit.fingerprint(root))

    def test_protocol_styles_are_configured_separately(self):
        graphql = audit.package('php', 'graphql', 'raw')
        self.assertEqual(graphql['plugins'][0]['contracts']['graphql']['style'], 'raw')
        self.assertNotIn('client_style', graphql)
        http = audit.package('php', 'http', 'flat')
        self.assertEqual(http['client_style'], 'flat')
        self.assertNotIn('contracts', http['plugins'][0])


if __name__ == '__main__':
    unittest.main()
