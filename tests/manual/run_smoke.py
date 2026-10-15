#!/usr/bin/env python3
"""Run a small, read-only black-box smoke set with an installed nxc."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional

CASES = (
    Path("tests/manual/hello_unicode.nx"),
    Path("tests/run/control_flow.nx"),
    Path("tests/run/closures.nx"),
)


def normalize(text: str) -> str:
    return text.replace("\r\n", "\n")


def resolve_nxc(argument: Optional[str]) -> Optional[str]:
    candidate = argument or os.environ.get("NEXUL_NXC") or "nxc"
    found = shutil.which(candidate)
    if found:
        return found
    path = Path(candidate).expanduser()
    if path.is_file():
        return str(path.resolve())
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--nxc",
        help="path to nxc (default: NEXUL_NXC, then PATH)",
    )
    args = parser.parse_args()

    nxc = resolve_nxc(args.nxc)
    if not nxc:
        print(
            "nxc introuvable. Installez NEXUL ou passez --nxc <chemin>.",
            file=sys.stderr,
        )
        return 2

    root = Path(__file__).resolve().parents[2]
    failures = 0
    passed = 0
    for relative_source in CASES:
        source = root / relative_source
        expected_path = source.with_suffix(".out")
        if not source.is_file() or not expected_path.is_file():
            failures += 1
            print(f"ERREUR : source ou référence absente pour {relative_source}", file=sys.stderr)
            continue

        result = subprocess.run(
            [nxc, "run", str(source)],
            cwd=root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        stdout = normalize(result.stdout.decode("utf-8", errors="replace"))
        expected = normalize(expected_path.read_text(encoding="utf-8"))
        if result.returncode != 0 or stdout != expected:
            failures += 1
            print(f"ÉCHEC : {relative_source} (code {result.returncode})")
            print("--- attendu ---")
            print(expected, end="" if expected.endswith("\n") else "\n")
            print("--- obtenu ---")
            print(stdout, end="" if stdout.endswith("\n") else "\n")
            stderr = result.stderr.decode("utf-8", errors="replace")
            if stderr:
                print("--- stderr ---", file=sys.stderr)
                print(stderr, file=sys.stderr, end="" if stderr.endswith("\n") else "\n")
        else:
            passed += 1
            print(f"OK : {relative_source}")

    print(f"{passed}/{len(CASES)} cas réussis.")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
