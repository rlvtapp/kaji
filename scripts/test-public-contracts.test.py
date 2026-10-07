"""No-network checks for the pinned-contract runner's failure boundaries."""
import os
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
import zipfile

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

    def test_archive_reference_tree_cannot_escape_output(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            cache = root / "cache"
            cache.mkdir()
            data = io.BytesIO()
            with zipfile.ZipFile(data, "w") as archive:
                archive.writestr("bundle/specification/api.yaml", "openapi: 3.1.0")
                archive.writestr("bundle/../../outside", "must not escape")
            payload = data.getvalue()
            (cache / "nested.zip").write_bytes(payload)
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({"contracts": [{"name": "nested",
                "url": "https://example.invalid/pinned.zip", "sha256": hashlib.sha256(payload).hexdigest(),
                "archive_entry": "specification/api.yaml"}]}))
            result = subprocess.run(["python3", str(SCRIPT.with_name("public-contracts.py")),
                "fetch", str(manifest), str(root / "output"), str(cache)], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Unsafe archive path", result.stderr)
            self.assertFalse((root / "outside").exists())
            self.assertFalse((root / "output/nested/specification/api.yaml").exists())

    def test_unknown_contract_selection_fails_in_check_mode(self):
        with tempfile.TemporaryDirectory() as temp:
            result = subprocess.run(["bash", str(SCRIPT), "check", "go"],
                env={**os.environ, "KAJI_PUBLIC_CONTRACT_ROOT": temp,
                     "KAJI_PUBLIC_CONTRACTS": "not-in-manifest"}, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Unknown selected contract", result.stderr)

if __name__ == "__main__":
    unittest.main()
