#!/usr/bin/env python3
"""Build and run the native scientific comparison on one Windows machine."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import os
import platform
import shutil
import statistics
import subprocess
import sys
import time
import winreg
from pathlib import Path


HERE = Path(__file__).resolve().parent
SRC = HERE / "src"
BUILD = HERE / "build"
RESULTS = HERE / "results"
LANGUAGES = ("C", "Rust", "C#", "NEXUL")
METRICS = (
    "sum_x", "sum_x2", "harmonic", "basel", "ln2", "pi_leibniz", "pi_integral",
    "quarter_circle", "sqrt_integral", "inverse_sqrt_integral", "e_approx", "sin_approx",
    "cos_approx", "position", "velocity", "population", "energy", "error_squared",
)


def run(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None) -> tuple[str, float]:
    start = time.perf_counter()
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True)
    elapsed = time.perf_counter() - start
    if result.returncode:
        raise RuntimeError(
            f"Command failed ({result.returncode}): {subprocess.list2cmdline(command)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result.stdout.strip(), elapsed


def find_program(name: str, candidates: tuple[Path, ...] = ()) -> str:
    found = shutil.which(name)
    if found:
        return found
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    raise FileNotFoundError(f"Required compiler not found: {name}")


def visual_studio_environment() -> dict[str, str]:
    roots = (Path(r"C:\Program Files\Microsoft Visual Studio\2022"), Path(r"C:\Program Files (x86)\Microsoft Visual Studio\2022"))
    vcvars = next((p for root in roots if root.exists() for p in root.glob("*/VC/Auxiliary/Build/vcvars64.bat")), None)
    if vcvars is None:
        return os.environ.copy()
    command = f'call "{vcvars}" -host_arch=x64 -arch=x64 >nul && set'
    proc = subprocess.run([os.environ.get("COMSPEC", "cmd.exe"), "/d", "/c", command], text=True, capture_output=True)
    if proc.returncode:
        raise RuntimeError(f"Could not initialize the Visual Studio C toolchain:\n{proc.stderr}")
    environment = os.environ.copy()
    for line in proc.stdout.splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            if key and not key.startswith("="):
                environment[key] = value
    return environment


def machine_name() -> str:
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") as key:
            return winreg.QueryValueEx(key, "ProcessorNameString")[0].strip()
    except OSError:
        return platform.processor() or "unknown CPU"


def parse_output(output: str) -> dict[str, float]:
    fields: dict[str, float] = {}
    for line in output.splitlines():
        if "=" not in line:
            continue
        name, raw = line.split("=", 1)
        if name == "elapsed_ns":
            fields[name] = int(raw)
        else:
            try:
                fields[name] = float(raw)
            except ValueError:
                continue
    missing = {"elapsed_ns", *METRICS} - fields.keys()
    if missing:
        raise ValueError(f"Program output did not contain expected values: {sorted(missing)}\n{output}")
    return fields


def emit_markdown(report: dict) -> str:
    lines = [
        f"# Résultats du comparatif — {report['date_local']}",
        "",
        f"Machine : {report['machine']} · Toolchains : {', '.join(f'{k} {v}' for k, v in report['toolchains'].items())}",
        "",
        "Médiane du temps interne par passe, en millisecondes; `±` = écart-type des processus mesurés.",
        "",
        "| Itérations | C | Rust | C# | NEXUL | Facteur C / NEXUL |",
        "|---:|---:|---:|---:|---:|---:|",
    ]
    for size in report["sizes"]:
        row = report["measurements"][str(size)]
        fastest = min(row[lang]["median_ms"] for lang in LANGUAGES)
        cells = [str(size)]
        for lang in LANGUAGES:
            data = row[lang]
            cells.append(f"{data['median_ms']:.3f} ± {data['stdev_ms']:.3f}")
        cells.append(f"{row['C']['median_ms'] / row['NEXUL']['median_ms']:.2f}×")
        lines.append("| " + " | ".join(cells) + " |")
    lines.extend(("", "Durée murale des commandes de build release (une invocation par programme) :", ""))
    lines.extend(f"- {lang} : {seconds:.2f} s" for lang, seconds in report["compile_seconds"].items())
    lines.extend((
        "",
        "Le facteur C / NEXUL compare les médianes; au-dessus de 1, NEXUL est plus rapide sur cette charge.",
        "Les sorties numériques ont été comparées à C avant l'acceptation des temps.",
        "Une machine, une exécution et une seule charge ne permettent pas de conclure sur les performances générales.",
        "",
    ))
    return "\n".join(lines)


def build_programs(args: argparse.Namespace) -> tuple[dict[str, list[str]], dict[str, float], dict[str, str]]:
    BUILD.mkdir(parents=True, exist_ok=True)
    nxc = args.nxc or find_program("nxc", (Path(r"C:\Users\Admin\AppData\Local\Programs\NEXUL\bin\nxc.exe"),))
    clang = args.clang or find_program("clang", (Path(r"C:\Program Files\LLVM\bin\clang.exe"),))
    rustc = args.rustc or find_program("rustup", (Path(r"C:\Users\Admin\.cargo\bin\rustup.exe"),))
    dotnet = args.dotnet or find_program("dotnet")

    commands: dict[str, list[str]] = {}
    compile_seconds: dict[str, float] = {}
    toolchains: dict[str, str] = {}
    msvc_env = visual_studio_environment()

    version, _ = run([clang, "--version"])
    toolchains["Clang"] = version.splitlines()[0]
    c_exe = BUILD / "scientific-c.exe"
    _, compile_seconds["C"] = run(
        [clang, "-std=c17", "-O2", "-ffp-contract=off", "-fno-fast-math", str(SRC / "scientific.c"), "-o", str(c_exe)],
        cwd=HERE,
        env=msvc_env,
    )
    commands["C"] = [str(c_exe)]

    rust_cmd = [rustc, "run", "stable-x86_64-pc-windows-msvc", "rustc", "--edition=2021", "-C", "opt-level=2", str(SRC / "scientific.rs"), "-o", str(BUILD / "scientific-rust.exe")]
    version, _ = run([rustc, "run", "stable-x86_64-pc-windows-msvc", "rustc", "-V"])
    toolchains["Rust"] = version.strip()
    _, compile_seconds["Rust"] = run(rust_cmd, cwd=HERE)
    commands["Rust"] = [str(BUILD / "scientific-rust.exe")]

    dotnet_version, _ = run([dotnet, "--version"])
    toolchains["C#"] = f".NET SDK {dotnet_version.strip()} (Release JIT, TieredPGO)"
    csharp_out = BUILD / "csharp"
    csharp_out.mkdir(parents=True, exist_ok=True)
    _, compile_seconds["C#"] = run([dotnet, "build", str(SRC / "csharp" / "Scientific.csproj"), "--configuration", "Release", "--output", str(csharp_out), "--nologo"], cwd=HERE)
    commands["C#"] = [dotnet, str(csharp_out / "Scientific.dll")]

    version, _ = run([nxc, "version"])
    toolchains["NEXUL"] = version.strip()
    nx_exe = BUILD / "scientific-nexul.exe"
    _, compile_seconds["NEXUL"] = run([nxc, "build", str(SRC / "scientific.nx"), "--release", "-o", str(nx_exe)], cwd=HERE)
    commands["NEXUL"] = [str(nx_exe)]
    return commands, compile_seconds, toolchains


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sizes", type=int, nargs="+", default=[1_000_000, 10_000_000, 100_000_000])
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--nxc")
    parser.add_argument("--clang")
    parser.add_argument("--rustc")
    parser.add_argument("--dotnet")
    args = parser.parse_args()
    if args.repetitions < 2 or any(size <= 0 for size in args.sizes):
        parser.error("use at least two repetitions and positive iteration counts")

    commands, compile_seconds, toolchains = build_programs(args)
    toolchains["Python"] = platform.python_version()
    report = {
        "date_local": dt.datetime.now().astimezone().isoformat(timespec="seconds"),
        "machine": machine_name(),
        "os": platform.platform(),
        "toolchains": toolchains,
        "compile_seconds": compile_seconds,
        "sizes": args.sizes,
        "repetitions": args.repetitions,
        "measurements": {},
        "algorithm": "same sequential f64 scientific loop; ten 100k-or-smaller warmups; one timed pass per process",
    }

    csharp_env = os.environ.copy()
    csharp_env["DOTNET_TieredCompilation"] = "1"
    csharp_env["DOTNET_TieredPGO"] = "1"
    for size in args.sizes:
        print(f"Mesure {size:,} itérations".replace(",", " "), flush=True)
        samples: dict[str, list[dict[str, float]]] = {lang: [] for lang in LANGUAGES}
        for lang in LANGUAGES:
            for repetition in range(args.repetitions):
                output, _ = run([*commands[lang], str(size)], cwd=HERE, env=csharp_env if lang == "C#" else None)
                samples[lang].append(parse_output(output))
                print(f"  {lang}: répétition {repetition + 1}/{args.repetitions}", flush=True)

        reference = samples["C"][0]
        accepted: dict[str, bool] = {}
        for lang in LANGUAGES:
            for sample in samples[lang]:
                for metric in METRICS:
                    expected = reference[metric]
                    actual = sample[metric]
                    tolerance = max(2e-9, abs(expected) * 2e-9)
                    if not math.isfinite(actual) or abs(actual - expected) > tolerance:
                        raise ValueError(f"{lang} differs from C for {metric} at n={size}: {actual} vs {expected}")
            accepted[lang] = True

        row = {}
        for lang in LANGUAGES:
            milliseconds = [sample["elapsed_ns"] / 1_000_000 for sample in samples[lang]]
            row[lang] = {
                "median_ms": statistics.median(milliseconds),
                "stdev_ms": statistics.stdev(milliseconds),
                "samples_ms": milliseconds,
                "outputs_match_c": accepted[lang],
                "numeric_reference": {key: reference[key] for key in METRICS},
            }
        report["measurements"][str(size)] = row

    RESULTS.mkdir(parents=True, exist_ok=True)
    (RESULTS / "latest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (RESULTS / "latest.md").write_text(emit_markdown(report), encoding="utf-8")
    print(emit_markdown(report))
    print(f"Résultats enregistrés dans {RESULTS}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (FileNotFoundError, RuntimeError, ValueError) as error:
        print(f"ERREUR: {error}", file=sys.stderr)
        raise SystemExit(1)
