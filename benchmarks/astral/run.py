#!/usr/bin/env python3
"""Build and compare equivalent NEXUL, C, Rust, and C# workloads on Windows."""

from __future__ import annotations

import argparse
import ctypes
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
from ctypes import wintypes
from pathlib import Path

try:
    import winreg
except ImportError:  # pragma: no cover - this benchmark records Windows working sets.
    winreg = None


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SRC = HERE / "src"
BUILD = HERE / "build"
RESULTS = HERE / "results"
LANGUAGES = ("C", "Rust", "C#", "NEXUL")
WORKLOADS = ("scientific", "vectors", "allocations", "strings")
SCIENTIFIC_METRICS = (
    "sum_x", "sum_x2", "harmonic", "basel", "ln2", "pi_leibniz", "pi_integral",
    "quarter_circle", "sqrt_integral", "inverse_sqrt_integral", "e_approx", "sin_approx",
    "cos_approx", "position", "velocity", "population", "energy", "error_squared",
)


class ProcessMemoryCountersEx(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("PageFaultCount", wintypes.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
        ("PrivateUsage", ctypes.c_size_t),
    ]


_psapi = ctypes.WinDLL("psapi.dll", use_last_error=True) if os.name == "nt" else None
if _psapi is not None:
    _psapi.GetProcessMemoryInfo.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(ProcessMemoryCountersEx),
        wintypes.DWORD,
    ]
    _psapi.GetProcessMemoryInfo.restype = wintypes.BOOL


def process_peak_working_set(process: subprocess.Popen[str]) -> int:
    """Read the kernel's peak working set after exit, before closing its process handle."""
    if _psapi is None:
        raise RuntimeError("peak working-set collection currently requires Windows")
    counters = ProcessMemoryCountersEx()
    counters.cb = ctypes.sizeof(counters)
    handle = wintypes.HANDLE(int(process._handle))  # type: ignore[attr-defined]
    if not _psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
        raise ctypes.WinError(ctypes.get_last_error())
    return int(counters.PeakWorkingSetSize)


def run(
    command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None
) -> tuple[str, float, int]:
    start = time.perf_counter()
    process = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    stdout, stderr = process.communicate()
    elapsed = time.perf_counter() - start
    peak_working_set = process_peak_working_set(process)
    if process.returncode:
        raise RuntimeError(
            f"Command failed ({process.returncode}): {subprocess.list2cmdline(command)}\n"
            f"stdout:\n{stdout}\nstderr:\n{stderr}"
        )
    return stdout.strip(), elapsed, peak_working_set


def find_program(name: str, candidates: tuple[Path, ...] = ()) -> str:
    found = shutil.which(name)
    if found:
        return found
    for candidate in candidates:
        if candidate.is_file():
            return str(candidate)
    raise FileNotFoundError(f"Required tool not found: {name}")


def visual_studio_environment() -> dict[str, str]:
    vswhere_candidates = (
        Path(r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"),
        Path(r"C:\Program Files\Microsoft Visual Studio\Installer\vswhere.exe"),
    )
    vswhere = shutil.which("vswhere") or next(
        (str(path) for path in vswhere_candidates if path.is_file()), None
    )
    vcvars: Path | None = None
    if vswhere:
        output, _, _ = run(
            [vswhere, "-latest", "-products", "*", "-find", r"VC\Auxiliary\Build\vcvars64.bat"]
        )
        vcvars = next((Path(line.strip()) for line in output.splitlines() if line.strip()), None)
    if vcvars is None or not vcvars.is_file():
        raise FileNotFoundError("MSVC vcvars64.bat not found; install Visual Studio C++ Build Tools")
    helper = BUILD / "init-vs-env.cmd"
    helper.write_text(
        "@echo off\r\n"
        f'call "{vcvars}" -host_arch=x64 -arch=x64 >nul\r\n'
        "if errorlevel 1 exit /b %errorlevel%\r\n"
        "set\r\n",
        encoding="utf-8",
    )
    try:
        proc = subprocess.run(
            [os.environ.get("COMSPEC", "cmd.exe"), "/d", "/c", str(helper)],
            text=True,
            capture_output=True,
        )
    finally:
        helper.unlink(missing_ok=True)
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
    if winreg is not None:
        try:
            with winreg.OpenKey(
                winreg.HKEY_LOCAL_MACHINE,
                r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
            ) as key:
                return winreg.QueryValueEx(key, "ProcessorNameString")[0].strip()
        except OSError:
            pass
    return platform.processor() or "unknown CPU"


def parse_output(output: str) -> dict[str, int | float]:
    fields: dict[str, int | float] = {}
    integer_fields = {"elapsed_ns", "length"}
    for line in output.splitlines():
        if "=" not in line:
            continue
        name, raw = line.split("=", 1)
        try:
            if name in integer_fields:
                fields[name] = int(raw)
            elif name == "checksum" and raw.isdigit():
                fields[name] = int(raw)
            else:
                fields[name] = float(raw)
        except ValueError:
            continue
    if "elapsed_ns" not in fields:
        raise ValueError(f"Program output did not contain elapsed_ns:\n{output}")
    return fields


def workload_keys(workload: str) -> tuple[str, ...]:
    return SCIENTIFIC_METRICS if workload == "scientific" else ("checksum", "length")


def compare_value(expected: int | float, actual: int | float, *, label: str) -> None:
    if isinstance(expected, int) and isinstance(actual, int):
        if expected != actual:
            raise ValueError(f"{label}: {actual} differs from C reference {expected}")
        return
    expected_float = float(expected)
    actual_float = float(actual)
    tolerance = max(2e-9, abs(expected_float) * 2e-9)
    if not math.isfinite(actual_float) or abs(actual_float - expected_float) > tolerance:
        raise ValueError(f"{label}: {actual} differs from C reference {expected}")


def emit_markdown(report: dict) -> str:
    lines = [
        f"# Résultats des benchmarks — {report['date_local']}",
        "",
        f"Commit NEXUL figé : `{report['source_commit']}` · machine : {report['machine']} · "
        f"OS : {report['os']}",
        "",
        "Médiane +/- écart-type des temps internes en ms; mémoire = médiane du pic de working set du processus en Mio.",
        "Les résultats bruts par répétition et les sommes de contrôle sont dans le JSON associé.",
        "",
        "| Charge | Itérations | C (ms +/- écart-type / Mio) | Rust (ms +/- écart-type / Mio) | C# (ms +/- écart-type / Mio) | NEXUL (ms +/- écart-type / Mio) | C / NEXUL |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    labels = {
        "scientific": "Boucles + maths",
        "vectors": "Vecteurs + matrice 3×3",
        "allocations": "Allocations libérées par itération",
        "strings": "Chaîne ASCII construite",
    }
    for workload in WORKLOADS:
        for size in report["sizes"]:
            row = report["measurements"][workload][str(size)]
            cells = [labels[workload], str(size)]
            for language in LANGUAGES:
                data = row[language]
                cells.append(
                    f"{data['median_ms']:.3f} +/- {data['stdev_ms']:.3f} / "
                    f"{data['median_peak_working_set_mib']:.1f}"
                )
            cells.append(f"{row['C']['median_ms'] / row['NEXUL']['median_ms']:.2f}x")
            lines.append("| " + " | ".join(cells) + " |")
    lines.extend(("", "Temps mural et pic mémoire maximal des commandes de compilation :", ""))
    for language in LANGUAGES:
        data = report["builds"][language]
        lines.append(
            f"- {language} : {data['wall_seconds']:.2f} s · "
            f"{data['peak_working_set_mib']:.1f} Mio"
        )
    lines.extend(
        (
            "",
            "Toutes les sorties sont comparées à C avant de conserver les temps. Les modes optimisés sont ceux indiqués dans le JSON et le README.",
            "La mesure NEXUL porte sur le commit figé indiqué ci-dessus, pas sur une promesse de version 0.3.0.",
            "Une machine et ce corpus ne permettent pas de conclure sur les performances générales ou sur d'autres cibles.",
            "",
        )
    )
    return "\n".join(lines)


def build_programs(args: argparse.Namespace) -> tuple[dict, dict, dict]:
    BUILD.mkdir(parents=True, exist_ok=True)
    nxc_candidates = (ROOT / "target" / "release" / "nxc.exe", ROOT / "target" / "release" / "nxc")
    nxc = args.nxc or find_program("nxc", nxc_candidates)
    clang = args.clang or find_program("clang", (Path(r"C:\Program Files\LLVM\bin\clang.exe"),))
    rustup = args.rustup or find_program("rustup", (Path.home() / ".cargo" / "bin" / "rustup.exe",))
    dotnet = args.dotnet or find_program("dotnet")

    commands: dict[str, dict[str, list[str]]] = {language: {} for language in LANGUAGES}
    compile_records: dict[str, list[dict[str, float | int | str]]] = {
        language: [] for language in LANGUAGES
    }
    msvc_env = visual_studio_environment()

    clang_version, _, _ = run([clang, "--version"])
    toolchains = {"Clang": clang_version.splitlines()[0]}
    rust_version, _, _ = run([rustup, "run", "stable-x86_64-pc-windows-msvc", "rustc", "-V"])
    toolchains["Rust"] = rust_version.strip()
    dotnet_version, _, _ = run([dotnet, "--version"])
    toolchains["C#"] = f".NET SDK {dotnet_version.strip()} (Release JIT, TieredPGO)"
    nxc_version, _, _ = run([nxc, "version"])
    toolchains["NEXUL"] = f"{nxc_version.strip()} (binary: {nxc})"

    def record_build(language: str, label: str, command: list[str], *, cwd: Path | None = None,
                     env: dict[str, str] | None = None) -> str:
        output, seconds, peak = run(command, cwd=cwd, env=env)
        compile_records[language].append(
            {"program": label, "wall_seconds": seconds, "peak_working_set_bytes": peak}
        )
        return output

    c_builds = (
        ("scientific", SRC / "scientific.c", BUILD / "scientific-c.exe"),
        ("dynamic", SRC / "dynamic.c", BUILD / "dynamic-c.exe"),
    )
    for label, source, output in c_builds:
        record_build(
            "C",
            label,
            [clang, "-std=c17", "-O2", "-ffp-contract=off", "-fno-fast-math", str(source), "-o", str(output)],
            cwd=HERE,
            env=msvc_env,
        )
        commands["C"][label] = [str(output)]

    rust_builds = (
        ("scientific", SRC / "scientific.rs", BUILD / "scientific-rust.exe"),
        ("dynamic", SRC / "dynamic.rs", BUILD / "dynamic-rust.exe"),
    )
    for label, source, output in rust_builds:
        record_build(
            "Rust",
            label,
            [rustup, "run", "stable-x86_64-pc-windows-msvc", "rustc", "--edition=2021", "-C", "opt-level=2", str(source), "-o", str(output)],
            cwd=HERE,
        )
        commands["Rust"][label] = [str(output)]

    csharp_projects = (
        ("scientific", SRC / "csharp" / "Scientific.csproj", "csharp-scientific"),
        ("dynamic", SRC / "csharp-dynamic" / "Dynamic.csproj", "csharp-dynamic"),
    )
    for label, project, output_name in csharp_projects:
        output = BUILD / output_name
        record_build(
            "C#",
            label,
            [dotnet, "build", str(project), "--configuration", "Release", "--output", str(output), "--nologo"],
            cwd=HERE,
        )
        commands["C#"][label] = [dotnet, str(output / ("Scientific.dll" if label == "scientific" else "Dynamic.dll"))]

    nxc_builds = (
        ("scientific", SRC / "scientific.nx", BUILD / "scientific-nexul.exe"),
        ("dynamic", SRC / "dynamic.nx", BUILD / "dynamic-nexul.exe"),
    )
    for label, source, output in nxc_builds:
        record_build(
            "NEXUL",
            label,
            [nxc, "build", str(source), "--release", "-o", str(output)],
            cwd=ROOT,
        )
        commands["NEXUL"][label] = [str(output)]

    builds = {}
    for language, records in compile_records.items():
        builds[language] = {
            "wall_seconds": sum(float(row["wall_seconds"]) for row in records),
            "peak_working_set_bytes": max(int(row["peak_working_set_bytes"]) for row in records),
            "peak_working_set_mib": max(int(row["peak_working_set_bytes"]) for row in records) / (1024 * 1024),
            "programs": records,
        }
    return commands, builds, toolchains


def main() -> int:
    if os.name != "nt":
        raise RuntimeError("This run records Windows peak working sets; run it on Windows")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sizes", type=int, nargs="+", default=[1_000_000, 10_000_000, 100_000_000])
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--nxc")
    parser.add_argument("--clang")
    parser.add_argument("--rustup", "--rustc", dest="rustup")
    parser.add_argument("--dotnet")
    args = parser.parse_args()
    if args.repetitions < 2 or any(size <= 0 for size in args.sizes):
        parser.error("use at least two repetitions and positive iteration counts")

    commands, builds, toolchains = build_programs(args)
    source_commit, _, _ = run(["git", "rev-parse", "HEAD"], cwd=ROOT)
    csharp_env = os.environ.copy()
    csharp_env["DOTNET_TieredCompilation"] = "1"
    csharp_env["DOTNET_TieredPGO"] = "1"
    report = {
        "date_local": dt.datetime.now().astimezone().isoformat(timespec="seconds"),
        "source_commit": source_commit.strip(),
        "machine": machine_name(),
        "os": platform.platform(),
        "toolchains": toolchains,
        "builds": builds,
        "sizes": args.sizes,
        "repetitions": args.repetitions,
        "measurement": {
            "time": "program-internal monotonic nanoseconds; excludes process launch and output parsing",
            "memory": "Windows kernel PeakWorkingSetSize, sampled after each child process exits",
            "c_options": ["-O2", "-ffp-contract=off", "-fno-fast-math"],
            "rust_options": ["--edition=2021", "-C opt-level=2", "x86_64-pc-windows-msvc"],
            "csharp_options": ["Release", "TieredCompilation=1", "TieredPGO=1"],
            "nexul_options": ["nxc build --release"],
        },
        "measurements": {workload: {} for workload in WORKLOADS},
    }

    for workload in WORKLOADS:
        binary = "scientific" if workload == "scientific" else "dynamic"
        for size in args.sizes:
            print(f"{workload}: {size:,} iterations".replace(",", " "), flush=True)
            samples: dict[str, list[dict]] = {language: [] for language in LANGUAGES}
            for language in LANGUAGES:
                command = commands[language][binary]
                workload_args = [str(size)] if workload == "scientific" else [workload, str(size)]
                env = csharp_env if language == "C#" else None
                for repetition in range(args.repetitions):
                    output, wall_seconds, peak_bytes = run(
                        [*command, *workload_args], cwd=HERE, env=env
                    )
                    fields = parse_output(output)
                    keys = workload_keys(workload)
                    missing = set(keys) - fields.keys()
                    if missing:
                        raise ValueError(f"{language}/{workload} omitted fields {sorted(missing)}\n{output}")
                    samples[language].append(
                        {
                            "internal_ms": int(fields["elapsed_ns"]) / 1_000_000,
                            "wall_ms": wall_seconds * 1000,
                            "peak_working_set_bytes": peak_bytes,
                            "outputs": {key: fields[key] for key in keys},
                        }
                    )
                    print(f"  {language}: {repetition + 1}/{args.repetitions}", flush=True)

            reference = samples["C"][0]["outputs"]
            row = {}
            for language in LANGUAGES:
                for sample in samples[language]:
                    for key in workload_keys(workload):
                        compare_value(
                            reference[key],
                            sample["outputs"][key],
                            label=f"{language}/{workload}/{size}/{key}",
                        )
                times = [sample["internal_ms"] for sample in samples[language]]
                walls = [sample["wall_ms"] for sample in samples[language]]
                peaks = [sample["peak_working_set_bytes"] for sample in samples[language]]
                row[language] = {
                    "median_ms": statistics.median(times),
                    "stdev_ms": statistics.stdev(times),
                    "median_wall_ms": statistics.median(walls),
                    "median_peak_working_set_bytes": statistics.median(peaks),
                    "median_peak_working_set_mib": statistics.median(peaks) / (1024 * 1024),
                    "max_peak_working_set_bytes": max(peaks),
                    "outputs_match_c": True,
                    "samples": samples[language],
                }
            report["measurements"][workload][str(size)] = row

    RESULTS.mkdir(parents=True, exist_ok=True)
    (RESULTS / "latest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (RESULTS / "latest.md").write_text(emit_markdown(report), encoding="utf-8")
    print("All workloads completed; checksums match the C reference.")
    print(f"Reports saved in {RESULTS}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (FileNotFoundError, RuntimeError, ValueError) as error:
        print(f"ERREUR: {error}", file=sys.stderr)
        raise SystemExit(1)
