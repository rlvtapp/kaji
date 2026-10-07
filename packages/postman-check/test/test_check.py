import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('checker', Path(__file__).parents[1] / 'check.py')
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)

class Checks(unittest.TestCase):
    def collection(self):
        return {'info': {'name': 'Test', 'schema': 'https://schema.getpostman.com/json/collection/v2.1.0/collection.json'}, 'item': [{'id': 'get-widget', 'name': 'Get widget', 'request': {'method': 'GET', 'url': '{{base_url}}/widgets'}}]}

    def test_schema_and_environment(self):
        self.assertEqual(checker.check(self.collection(), {'values': [{'key': 'token', 'value': '', 'type': 'secret'}]}), 1)
        with self.assertRaises(ValueError):
            checker.check(self.collection(), {'values': [{'key': 'token', 'value': 'private', 'type': 'secret'}]})

    def test_invalid_collection_and_duplicate_ids(self):
        with self.assertRaises(ValueError):
            checker.check({'info': {}, 'item': []})
        collection = self.collection()
        collection['item'] *= 2
        with self.assertRaises(ValueError):
            checker.check(collection)

    def test_paths(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            ((root / 'collection.json').resolve()).write_text('{}')
            self.assertEqual(checker.checkout_file(root, 'collection.json'), (root / 'collection.json').resolve())
            (root / 'link.json').symlink_to((root / 'collection.json').resolve())
            for value in ['../escape.json', str(root / 'collection.json'), 'link.json']:
                with self.assertRaises(ValueError):
                    checker.checkout_file(root, value)

if __name__ == '__main__':
    unittest.main()
