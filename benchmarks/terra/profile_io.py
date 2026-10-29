#!/usr/bin/env python3
"""Measure source_scan on temporary files of controlled sizes (Linux)."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
LANGUAGES = ("C", "Rust", "NEXUL")
EXTENSIONS = {"C": "c", "Rust": "rs", "NEXUL": "nx"}
SIZES = (0, 1, 3783, 65536)


def tool(name):
    found = shutil.which(name)
    if not found:
        raise RuntimeError(f"Outil introuvable : {name}")
    # Preserve the executable name for dispatchers such as rustup.
    return os.path.abspath(found)


def run(command, cwd=ROOT):
    start = time.perf_counter_ns()
    result = subprocess.run(command, cwd=cwd, capture_output=True, check=True)
    return result.stdout, (time.perf_counter_ns() - start) / 1_000_000


def main():
    parser = argparse.ArgumentParser(description="Sonder les lectures de fichiers de tailles contrôlées")
    parser.add_argument("--nxc", required=True)
    parser.add_argument("--cc", default="cc")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--output", type=Path, required=True, help="nouveau fichier JSON")
    args = parser.parse_args()
    if sys.platform != "linux":
        raise RuntimeError("Cette expérience nécessite Linux")
    if args.output.exists():
        raise RuntimeError("Le fichier de résultat existe déjà")
    tools = {"C": tool(args.cc), "Rust": tool(args.rustc), "NEXUL": tool(args.nxc)}
    sources = {key: HERE / f"source_scan.{ext}" for key, ext in EXTENSIONS.items()}
    report = {
        "date_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "platform": platform.platform(),
        "cpu": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                     if line.startswith("model name")), "inconnu"),
        "commit": run(["git", "rev-parse", "HEAD"])[0].decode().strip(),
        "working_tree_status": run(["git", "status", "--porcelain", "--untracked-files=all"])[0].decode().strip(),
        "nexul_leak_check": os.environ.get("NEXUL_LEAK_CHECK"),
        "nexul_llvm": os.environ.get("NEXUL_LLVM"),
        "tools": tools,
        "versions": {key: run([path, "version" if key == "NEXUL" else "--version"])[0].decode().strip()
                     for key, path in tools.items()},
        "nxc_sha256": hashlib.sha256(Path(tools["NEXUL"]).read_bytes()).hexdigest(),
        "source_sha256": {key: hashlib.sha256(path.read_bytes()).hexdigest() for key, path in sources.items()},
        "method": "7 process wall-clock samples per language; rotated execution order; 400 reads per process; synthetic files contain ASCII a",
        "cases": {},
    }
    with tempfile.TemporaryDirectory(prefix="nexul-io-probe-") as tmp:
        directory = Path(tmp)
        binaries = {key: directory / key.lower() for key in LANGUAGES}
        commands = {
            "C": [tools["C"], "-std=c17", "-O2", "-ffp-contract=off", "-fno-fast-math", "-pthread", str(sources["C"]), "-o", str(binaries["C"]), "-lm"],
            "Rust": [tools["Rust"], "--edition=2021", "-C", "opt-level=2", str(sources["Rust"]), "-o", str(binaries["Rust"])],
            "NEXUL": [tools["NEXUL"], "build", str(sources["NEXUL"]), "--release", "-o", str(binaries["NEXUL"])],
        }
        report["build_commands"] = commands
        for command in commands.values():
            run(command)
        source_file = directory / "lib" / "std" / "io.nx"
        source_file.parent.mkdir(parents=True)
        for size in SIZES:
            content = b"a" * size
            source_file.write_bytes(content)
            samples = {key: [] for key in LANGUAGES}
            expected = None
            for repeat in range(7):
                for key in LANGUAGES[repeat % 3:] + LANGUAGES[:repeat % 3]:
                    output, elapsed = run([str(binaries[key])], cwd=directory)
                    if not output or (expected is not None and output != expected):
                        raise RuntimeError(f"Sortie différente ou vide : taille {size}, {key}, passage {repeat + 1}")
                    expected = output
                    samples[key].append(elapsed)
            report["cases"][str(size)] = {
                "input_sha256": hashlib.sha256(content).hexdigest(),
                "stdout": expected.decode("utf-8"),
                "languages": {key: {"samples_ms": samples[key], "median_ms": statistics.median(samples[key])}
                              for key in LANGUAGES},
            }
            print(f"{size} octets : " + " · ".join(f"{key} {statistics.median(samples[key]):.3f} ms" for key in LANGUAGES), flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x", encoding="utf-8") as output:
        json.dump(report, output, ensure_ascii=False, indent=2)
        output.write("\n")
    print(f"Données : {args.output}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"ERREUR : {error}", file=sys.stderr)
        sys.exit(1)
