"""No-network checks for the pinned-contract runner's failure boundaries."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("test-public-contracts.sh")

class PublicContractRunnerTests(unittest.TestCase):
    def test_changed_cached_contract_fails_before_sdk_generation(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            cache = root / "cache"
            cache.mkdir()
            for name in ("forecast", "air-quality"):
                (cache / f"{name}.yml").write_text("unexpected contract revision")
            output = root / "output"
            result = subprocess.run(["bash", str(SCRIPT), "generate"], env={**os.environ,
                "KAJI_PUBLIC_CONTRACT_ROOT": str(output), "KAJI_PUBLIC_SPEC_DIR": str(cache),
                "KAJI_BINARY": str(root / "must-not-execute")}, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("checksum mismatch", result.stderr)
            self.assertFalse((output / "forecast").exists())
            self.assertFalse((output / "air-quality").exists())

    def test_unknown_language_is_rejected_before_entering_a_package(self):
        with tempfile.TemporaryDirectory() as temp:
            result = subprocess.run(["bash", str(SCRIPT), "check", "../unsupported"],
                env={**os.environ, "KAJI_PUBLIC_CONTRACT_ROOT": temp}, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Unsupported language", result.stderr)

if __name__ == "__main__":
    unittest.main()
