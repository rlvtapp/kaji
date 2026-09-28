import os
import unittest
from pathlib import Path
from unittest import mock

import kaji_cli


class LauncherTests(unittest.TestCase):
    def test_development_override_is_resolved(self):
        with mock.patch.dict(os.environ, {"KAJI_BINARY": "./test-kaji"}, clear=False):
            self.assertEqual(kaji_cli.executable_path(), Path("./test-kaji").resolve())

    @unittest.skipIf(os.name == "nt", "Unix uses execv; Windows uses subprocess")
    def test_unix_replaces_the_python_process_with_kaji(self):
        executable = Path("/tmp/kaji")
        with mock.patch("kaji_cli.executable_path", return_value=executable), mock.patch(
            "kaji_cli.os.execv", side_effect=OSError("test error")
        ) as execute:
            with self.assertRaises(OSError):
                kaji_cli.run(["languages"])
        execute.assert_called_once_with("/tmp/kaji", ["/tmp/kaji", "languages"])
