"""Fetch immutable public contracts and retain pinned local reference trees."""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import sys
import urllib.request
import zipfile


def contracts(manifest):
    values = json.loads(Path(manifest).read_text())["contracts"]
    requested = os.environ.get("POOLSTER_PUBLIC_CONTRACTS", "").split(",")
    requested = [name for name in requested if name]
    known = {value["name"] for value in values}
    if len(known) != len(values) or any(not re.fullmatch(r"[a-z][a-z0-9-]*", name) for name in known):
        raise ValueError("Invalid or duplicate contract name")
    if set(requested) - known:
        raise ValueError("Unknown selected contract")
    return [value for value in values if not requested or value["name"] in requested]


def source_path(root, contract):
    if "archive_entry" in contract:
        return root / contract["name"] / contract["archive_entry"]
    if contract.get("references"):
        return root / contract["name"] / contract.get("source_path", "source.yml")
    return root / (contract["name"] + ".yml")


def fetch(manifest, root, cache):
    root.mkdir(parents=True, exist_ok=True)
    for contract in contracts(manifest):
        archive = "archive_entry" in contract
        filename = contract["name"] + (".zip" if archive else ".yml")
        destination = root / filename
        limit = contract.get("max_bytes", 2 * 1024 * 1024)
        if not isinstance(limit, int) or not 1 <= limit <= 64 * 1024 * 1024:
            raise ValueError("Invalid contract size bound")
        if cache:
            cached = Path(cache) / filename
            if cached.stat().st_size > limit:
                raise ValueError("Contract exceeds size bound")
            data = cached.read_bytes()
        else:
            if not contract["url"].startswith("https://"):
                raise ValueError("Contract URL must use HTTPS")
            with urllib.request.urlopen(contract["url"], timeout=60) as response:
                data = response.read(limit + 1)
        if len(data) > limit:
            raise ValueError("Contract exceeds size bound")
        if hashlib.sha256(data).hexdigest() != contract["sha256"]:
            raise ValueError("Contract checksum mismatch: " + contract["name"])
        destination.write_bytes(data)
        if archive:
            unpack(root / contract["name"], destination, contract["archive_entry"])
        if contract.get("references"):
            if archive:
                raise ValueError("Archive contracts must carry their own reference tree")
            references = contract["references"]
            if len(references) > 1000:
                raise ValueError("Too many supplemental references")
            staged = []
            source = PurePosixPath(contract.get("source_path", "source.yml"))
            if source.is_absolute() or ".." in source.parts or "\\" in str(source) or not source.parts:
                raise ValueError("Unsafe supplemental source path")
            seen = {str(source)}
            for reference in references:
                path = PurePosixPath(reference["path"])
                if path.is_absolute() or ".." in path.parts or "\\" in reference["path"] or str(path) in seen or not path.parts:
                    raise ValueError("Unsafe supplemental reference path")
                seen.add(str(path))
                bound = reference.get("max_bytes", 1048576)
                if not isinstance(bound, int) or not 1 <= bound <= 16777216:
                    raise ValueError("Invalid supplemental reference size bound")
                if cache:
                    cached = Path(cache) / contract["name"] / str(path)
                    if cached.stat().st_size > bound:
                        raise ValueError("Supplemental reference exceeds size bound")
                    payload = cached.read_bytes()
                else:
                    if not reference["url"].startswith("https://"):
                        raise ValueError("Supplemental reference URL must use HTTPS")
                    with urllib.request.urlopen(reference["url"], timeout=60) as response:
                        payload = response.read(bound + 1)
                if len(payload) > bound or hashlib.sha256(payload).hexdigest() != reference["sha256"]:
                    raise ValueError("Supplemental reference checksum/size mismatch")
                staged.append((path, payload))
            if sum(len(payload) for _, payload in staged) > 64 * 1024 * 1024:
                raise ValueError("Supplemental reference tree exceeds size bound")
            tree = root / contract["name"]
            if tree.exists():
                raise ValueError("Reference output already exists; use a fresh output directory")
            tree.mkdir()
            source_file = tree.joinpath(*source.parts)
            source_file.parent.mkdir(parents=True, exist_ok=True)
            source_file.write_bytes(data)
            for path, payload in staged:
                target = tree.joinpath(*path.parts)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(payload)
        print("Verified pinned contract:", contract["name"])


def unpack(root, archive, entry):
    entry_parts = PurePosixPath(entry)
    if entry_parts.is_absolute() or ".." in entry_parts.parts:
        raise ValueError("Invalid archive entry")
    with zipfile.ZipFile(archive) as source:
        members = source.infolist()
        if len(members) > 20000 or sum(member.file_size for member in members) > 256 * 1024 * 1024:
            raise ValueError("Contract archive exceeds extraction bound")
        prefix = None
        validated = []
        for member in members:
            path = PurePosixPath(member.filename)
            if path.is_absolute() or ".." in path.parts or "\\" in member.filename:
                raise ValueError("Unsafe archive path")
            if not path.parts:
                continue
            prefix = prefix or path.parts[0]
            if path.parts[0] != prefix or (member.external_attr >> 16) & 0o170000 == 0o120000:
                raise ValueError("Unsafe archive member")
            if len(path.parts) > 1:
                validated.append((member, root.joinpath(*path.parts[1:])))
        if root.exists():
            raise ValueError("Archive output already exists; use a fresh output directory")
        for member, target in validated:
            if member.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.open(member) as reader, target.open("xb") as writer:
                    shutil.copyfileobj(reader, writer)
    if not (root / entry).is_file():
        raise ValueError("Contract archive entry missing")


if __name__ == "__main__":
    mode, manifest, root = sys.argv[1:4]
    try:
        if mode == "fetch":
            fetch(manifest, Path(root), sys.argv[4])
        elif mode == "list":
            for contract in contracts(manifest):
                print(contract["name"] + "\t" + str(source_path(Path(root), contract)))
        else:
            raise ValueError("Unknown contract command")
    except (ValueError, OSError, zipfile.BadZipFile) as error:
        raise SystemExit(str(error))
