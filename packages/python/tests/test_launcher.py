import os
import unittest
from pathlib import Path
from unittest import mock

import poolster


class LauncherTests(unittest.TestCase):
    def test_development_override_is_resolved(self):
        with mock.patch.dict(os.environ, {"POOLSTER_BINARY": "./test-poolster"}, clear=False):
            self.assertEqual(poolster.executable_path(), Path("./test-poolster").resolve())

    @unittest.skipIf(os.name == "nt", "Unix uses execv; Windows uses subprocess")
    def test_unix_replaces_the_python_process_with_poolster(self):
        executable = Path("/tmp/poolster")
        with mock.patch("poolster.executable_path", return_value=executable), mock.patch(
            "poolster.os.execv", side_effect=OSError("test error")
        ) as execute:
            with self.assertRaises(OSError):
                poolster.run(["languages"])
        execute.assert_called_once_with("/tmp/poolster", ["/tmp/poolster", "languages"])
