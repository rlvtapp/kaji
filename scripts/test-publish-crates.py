import importlib.util
import subprocess
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location('publish_crates', Path(__file__).with_name('publish-crates.py'))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


def package(name, dependencies=(), public=True):
    return {'name': name, 'id': name, 'version': '0.5.0',
            'publish': ['crates-io'] if public else [],
            'dependencies': [{'name': dep, 'path': '/workspace/' + dep,
                              'kind': None, 'optional': True} for dep in dependencies]}


class PublicationTests(unittest.TestCase):
    def order(self, *packages):
        return release.publication_order({'packages': packages,
                                          'workspace_members': [p['id'] for p in packages]})

    def test_orders_optional_dependencies_and_excludes_launchers(self):
        ordered = self.order(package('poolster', ['plugin']), package('plugin', ['core']),
                             package('core'), package('cli', ['poolster'], False))
        self.assertEqual([p['name'] for p in ordered], ['core', 'plugin', 'poolster'])

    def test_cycle_rejected(self):
        with self.assertRaisesRegex(ValueError, 'cycle'):
            self.order(package('a', ['b']), package('b', ['a']))

    def test_private_dependency_rejected(self):
        with self.assertRaisesRegex(ValueError, 'unpublished'):
            self.order(package('a', ['private']), package('private', public=False))

    def test_resume_skips_existing_version(self):
        run = Mock()
        release.publish([package('core'), package('poolster')], lambda name, _: name == 'core', run)
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.args[0][-1], 'poolster')
        self.assertTrue(run.call_args.kwargs['check'])

    def test_publish_failure_stops_dependents(self):
        run = Mock(side_effect=subprocess.CalledProcessError(101, 'cargo'))
        with self.assertRaises(subprocess.CalledProcessError):
            release.publish([package('core'), package('poolster')], lambda *_: False, run)
        self.assertEqual(run.call_count, 1)

    def test_registry_errors_are_not_treated_as_absent(self):
        for status in (403, 429, 500):
            with self.subTest(status=status), patch.object(release.urllib.request, 'urlopen',
                    side_effect=urllib.error.HTTPError('url', status, 'error', {}, None)):
                with self.assertRaises(urllib.error.HTTPError):
                    release.already_published('core', '0.5.0')

    def test_missing_version_is_publishable(self):
        with patch.object(release.urllib.request, 'urlopen',
                          side_effect=urllib.error.HTTPError('url', 404, 'missing', {}, None)):
            self.assertFalse(release.already_published('core', '0.5.0'))

    def test_yanked_version_is_rejected(self):
        response = Mock()
        response.__enter__ = Mock(return_value=response)
        response.__exit__ = Mock(return_value=False)
        response.read.return_value = b'{"version":{"yanked":true}}'
        with patch.object(release.urllib.request, 'urlopen', return_value=response):
            with self.assertRaisesRegex(RuntimeError, 'yanked'):
                release.already_published('core', '0.5.0')


if __name__ == '__main__':
    unittest.main()
