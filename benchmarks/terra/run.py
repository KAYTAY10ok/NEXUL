#!/usr/bin/env python3
"""Compile and compare NEXUL, C and Rust workloads on Linux."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import shlex
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
LANGUAGES = ("C", "Rust", "NEXUL")
EXTENSIONS = {"C": "c", "Rust": "rs", "NEXUL": "nx"}


def run(command: list[str], cwd: Path = ROOT) -> tuple[bytes, int]:
    start = time.perf_counter_ns()
    result = subprocess.run(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    elapsed = time.perf_counter_ns() - start
    if result.returncode:
        raise RuntimeError(
            f"Échec ({result.returncode}) : {shlex.join(command)}\n"
            f"stdout : {result.stdout.decode('utf-8', 'replace')}\n"
            f"stderr : {result.stderr.decode('utf-8', 'replace')}"
        )
    return result.stdout, elapsed


def find_tool(requested: str | None, default: str) -> str:
    name = requested or default
    found = shutil.which(name)
    if not found:
        raise RuntimeError(f"Outil introuvable : {name}")
    # Rustup dispatches on argv[0]; resolving the rustc symlink runs rustup instead.
    return os.path.abspath(found)


def version(command: list[str]) -> str:
    output, _ = run(command)
    return output.decode("utf-8", "replace").strip()


def workloads(requested: list[str] | None) -> list[str]:
    names = sorted(requested if requested else {p.stem for p in HERE.glob("*.nx")})
    if not names:
        raise RuntimeError("Aucune charge trouvée dans benchmarks/terra")
    if len(names) != len(set(names)):
        raise RuntimeError("Une charge est demandée plusieurs fois")
    for name in names:
        if not name.isidentifier() or not name.isascii():
            raise RuntimeError(f"Nom de charge invalide : {name}")
        for extension in EXTENSIONS.values():
            if not (HERE / f"{name}.{extension}").is_file():
                raise RuntimeError(f"Source manquante : {name}.{extension}")
    return names


def build_commands(name: str, tools: dict[str, str], directory: Path) -> dict[str, list[str]]:
    outputs = {language: directory / f"{name}-{language.lower()}" for language in LANGUAGES}
    return {
        "C": [tools["C"], "-std=c17", "-O2", "-ffp-contract=off", "-fno-fast-math", "-pthread",
              str(HERE / f"{name}.c"), "-o", str(outputs["C"]), "-lm"],
        "Rust": [tools["Rust"], "--edition=2021", "-C", "opt-level=2",
                 str(HERE / f"{name}.rs"), "-o", str(outputs["Rust"])],
        "NEXUL": [tools["NEXUL"], "build", str(HERE / f"{name}.nx"), "--release",
                  "-o", str(outputs["NEXUL"])],
    }


def markdown(report: dict) -> str:
    lines = [
        "# Campagne de performance Linux", "",
        f"Date : {report['date_utc']} · commit : `{report['commit']}` · "
        f"arbre : {'propre' if report['working_tree_status'] == 'propre' else 'modifié (voir JSON)'}", "",
        f"Machine : {report['platform']} · CPU : {report['cpu']}", "",
        "Temps mural du processus en millisecondes, médiane de sept exécutions ; "
        "chaque sortie a été comparée octet pour octet à la sortie C.", "",
        "| Charge | C (ms) | Rust (ms) | NEXUL (ms) | NEXUL / C |",
        "|---|---:|---:|---:|---:|",
    ]
    for name, result in report["measurements"].items():
        medians = [result["languages"][lang]["median_ms"] for lang in LANGUAGES]
        lines.append(f"| {name} | {medians[0]:.3f} | {medians[1]:.3f} | "
                     f"{medians[2]:.3f} | {medians[2] / medians[0]:.3f} |")
    lines.extend(["", "Le JSON associé contient les échantillons bruts, les options de "
                  "compilation, les versions et les empreintes des sources. La mesure inclut "
                  "le démarrage et la sortie des processus ; elle ne démontre pas à elle "
                  "seule l'origine d'un écart.", ""])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Comparer les charges NEXUL, C et Rust sous Linux")
    parser.add_argument("--nxc", help="chemin de nxc (défaut : target/release/nxc ou PATH)")
    parser.add_argument("--cc", help="compilateur C (défaut : cc)")
    parser.add_argument("--rustc", help="compilateur Rust (défaut : rustc)")
    parser.add_argument("--workloads", nargs="+", help="charges à exécuter (défaut : toutes)")
    parser.add_argument("--output", type=Path, default=HERE / "results",
                        help="dossier des rapports JSON et Markdown")
    args = parser.parse_args()
    if sys.platform != "linux":
        raise RuntimeError("Ce pilote nécessite Linux")
    if any((args.output / name).exists() for name in ("latest.json", "latest.md")):
        raise RuntimeError("Le dossier contient déjà des rapports : choisir un dossier vide pour préserver les mesures précédentes")
    names = workloads(args.workloads)
    local_nxc = ROOT / "target" / "release" / "nxc"
    nxc = args.nxc or (str(local_nxc) if local_nxc.is_file() else None)
    tools = {"C": find_tool(args.cc, "cc"), "Rust": find_tool(args.rustc, "rustc"),
             "NEXUL": find_tool(nxc, "nxc")}
    commit = version(["git", "rev-parse", "HEAD"])
    dirty = version(["git", "status", "--porcelain", "--untracked-files=all", "--", "."])
    try:
        cpu = next(line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                   if line.startswith("model name"))
    except (OSError, StopIteration):
        cpu = platform.processor() or "inconnu"
    report = {
        "date_utc": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        "commit": commit, "working_tree_status": dirty or "propre", "platform": platform.platform(),
        "cpu": cpu, "python": platform.python_version(),
        "toolchains": {"C": version([tools["C"], "--version"]),
                       "Rust": version([tools["Rust"], "--version"]),
                       "NEXUL": version([tools["NEXUL"], "version"])},
        "tool_paths": tools, "repetitions": 7,
        "measurement": "wall clock process execution including startup and output; rotating order",
        "nexul_leak_check": os.environ.get("NEXUL_LEAK_CHECK"), "measurements": {},
    }
    with tempfile.TemporaryDirectory(prefix="nexul-bench-") as temp:
        directory = Path(temp)
        for name in names:
            print(f"Compilation : {name}", flush=True)
            commands = build_commands(name, tools, directory)
            for language in LANGUAGES:
                run(commands[language])
            samples = {language: [] for language in LANGUAGES}
            expected = None
            for repetition in range(7):
                # Rotate the first language to reduce fixed-order bias.
                order = LANGUAGES[repetition % len(LANGUAGES):] + LANGUAGES[:repetition % len(LANGUAGES)]
                for language in order:
                    output, elapsed = run([str(directory / f"{name}-{language.lower()}")])
                    if not output:
                        raise RuntimeError(f"Sortie vide : {name}/{language}")
                    if expected is None:
                        expected = output
                    elif output != expected:
                        raise RuntimeError(f"Sorties différentes : {name}/{language}, passage {repetition + 1}\n"
                                           f"référence : {expected!r}\nobtenu : {output!r}")
                    samples[language].append(elapsed / 1_000_000)
            assert expected is not None
            report["measurements"][name] = {
                "source_sha256": {lang: hashlib.sha256((HERE / f"{name}.{ext}").read_bytes()).hexdigest()
                                  for lang, ext in EXTENSIONS.items()},
                "build_commands": commands,
                "output_utf8": expected.decode("utf-8", "strict"),
                "languages": {lang: {"samples_ms": samples[lang],
                                     "median_ms": statistics.median(samples[lang])} for lang in LANGUAGES},
            }
            if name == "source_scan":
                source = ROOT / "lib/std/io.nx"
                report["measurements"][name]["input_sha256"] = {
                    "lib/std/io.nx": hashlib.sha256(source.read_bytes()).hexdigest()
                }
            row = report["measurements"][name]["languages"]
            print("  " + " · ".join(f"{lang} {row[lang]['median_ms']:.3f} ms" for lang in LANGUAGES), flush=True)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "latest.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (args.output / "latest.md").write_text(markdown(report), encoding="utf-8")
    print(f"Rapports : {args.output / 'latest.json'} et {args.output / 'latest.md'}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, UnicodeError) as error:
        print(f"ERREUR : {error}", file=sys.stderr)
        raise SystemExit(1)
