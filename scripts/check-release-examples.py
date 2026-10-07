"""Generate release examples and verify user-owned npm manifest behavior."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent
cli = root / "target/debug/kaji"

workspace = tempfile.TemporaryDirectory(prefix="kaji-release-examples-")
examples = Path(workspace.name)
for name in ["symfony-sdk", "manifest-merging", "rust-cli"]:
    target = examples / name
    target.mkdir()
    for filename in ["kaji.json", "openapi.yaml"]:
        shutil.copyfile(root / "examples" / name / filename, target / filename)

def generate(name):
    subprocess.run([str(cli), "generate", "--config", str(examples / name / "kaji.json")], check=True)

for name in ["symfony-sdk", "manifest-merging", "rust-cli"]:
    generate(name)
manifest = examples / "manifest-merging/generated/typescript/package.json"
shutil.copyfile(root / "examples/manifest-merging/package.seed.json", manifest)
generate("manifest-merging")
value = json.loads(manifest.read_text())
assert value["scripts"]["test"] == "node --test"
assert value["devDependencies"]["typescript"] == "5.9.3"
assert "@tanstack/react-query" not in value.get("dependencies", {})
assert value["peerDependenciesMeta"]["@tanstack/react-query"]["optional"]
first = manifest.read_bytes()
generate("manifest-merging")
assert first == manifest.read_bytes(), "Manifest changes on repeated generation"
print("Release examples generated; npm customization is preserved")

workspace.cleanup()
