"""Console launcher for Poolster's packaged native generator."""

from __future__ import annotations

import os
import subprocess
import sys
from importlib.resources import files
from pathlib import Path
from typing import Sequence


def executable_path() -> Path:
    """Return the packaged generator executable for this installation."""

    override = os.environ.get("POOLSTER_BINARY")
    if override:
        return Path(override).expanduser().resolve()
    name = "poolster.exe" if os.name == "nt" else "poolster"
    executable = Path(str(files("poolster").joinpath("bin", name)))
    if not executable.is_file():
        raise RuntimeError(
            f"the bundled Poolster executable is missing: {executable}. "
            "Reinstall poolster."
        )
    return executable


def run(arguments: Sequence[str]) -> int:
    """Launch Poolster, preserving signals and exit status on every platform."""

    executable = executable_path()
    command = [str(executable), *arguments]
    if os.name != "nt":
        os.execv(str(executable), command)
        raise AssertionError("os.execv unexpectedly returned")
    return subprocess.run(command, check=False).returncode


def main() -> None:
    try:
        code = run(sys.argv[1:])
    except (OSError, RuntimeError) as error:
        print(f"poolster: {error}", file=sys.stderr)
        code = 1
    raise SystemExit(code)
