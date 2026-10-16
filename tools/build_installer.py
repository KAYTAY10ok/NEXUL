"""Builds `NEXUL Setup.exe`, the installer of NEXUL for Windows.

usage: python -I tools/build_installer.py [output.exe]

Steps: compile the workspace in release mode, gather the files of each component in
`target/installer-staging/<component>/` (relative to the installation folder), then pack
them with `nexul-pack` (LZMS compression, SHA-256 of every file).

The LLVM files (LLVM-C.dll, LLD, LLDB) come from the LLVM installation found in
NEXUL_LLVM or C:\\Program Files\\LLVM; the Python runtime of the debugger from the newest
Python 3 installation (or NEXUL_PYTHON).
"""

import json
import os
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TARGET = os.path.join(ROOT, "target", "release")
STAGING = os.path.join(ROOT, "target", "installer-staging")

COMPONENTS = [
    ("core", True, True, "Compilateur NEXUL", "nxc et nxpm : compiler, tester, formater, gérer les paquets. La bibliothèque standard est incluse."),
    ("llvm", True, True, "Génération de code LLVM", "LLVM et l'éditeur de liens LLD : code machine optimisé pour Windows, Linux, macOS, WebAssembly et microcontrôleurs."),
    ("studio", False, True, "NEXUL Studio", "L'application native est utilisée par défaut; l'ancienne interface web reste disponible séparément."),
    ("debugger", False, True, "Débogueur", "Points d'arrêt, pas à pas, variables et pile d'appels dans NEXUL Studio (LLDB et son moteur Python)."),
    ("samples", False, True, "Exemples et documentation", "Programmes d'exemple, guide du langage, spécification et documentation des extensions de Studio."),
]


def llvm_dir():
    for d in [os.environ.get("NEXUL_LLVM"), r"C:\Program Files\LLVM"]:
        if d and os.path.isfile(os.path.join(d, "bin", "LLVM-C.dll")):
            return d
    sys.exit("LLVM introuvable (définissez NEXUL_LLVM)")


def python_dir():
    if os.environ.get("NEXUL_PYTHON"):
        return os.environ["NEXUL_PYTHON"]
    cands = []
    for base in [r"C:\\", os.path.expandvars(r"%LOCALAPPDATA%\Programs\Python")]:
        if os.path.isdir(base):
            for d in os.listdir(base):
                if d.lower().startswith("python3") and os.path.isfile(os.path.join(base, d, "python3.dll")):
                    cands.append(os.path.join(base, d))
    cands.sort(key=lambda p: int("".join(c for c in os.path.basename(p) if c.isdigit()) or 0))
    # LLDB loads python3.dll (stable ABI): any recent Python 3 works.
    if not cands:
        sys.exit("Python 3 introuvable (définissez NEXUL_PYTHON)")
    return cands[0]


def copy(src, dst):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copy2(src, dst)


def copy_tree(src, dst, exts=None):
    for root, dirs, files in os.walk(src):
        dirs[:] = [d for d in dirs if not d.startswith(".") and d != "target"]
        for f in files:
            if exts and not f.lower().endswith(exts):
                continue
            s = os.path.join(root, f)
            copy(s, os.path.join(dst, os.path.relpath(s, src)))


def package_versions(manifest=None):
    command = ["cargo", "metadata", "--format-version", "1", "--no-deps"]
    if manifest:
        command.extend(["--manifest-path", manifest])
    metadata = json.loads(subprocess.check_output(command, cwd=ROOT, text=True))
    return {package["name"]: package["version"] for package in metadata["packages"]}


def check_release_versions():
    workspace = package_versions()
    native = package_versions(os.path.join("studio", "app", "Cargo.toml"))
    distributed = {
        "nxc/nxpm": workspace.get("nxc"),
        "NEXUL Studio (web)": workspace.get("nexul-studio"),
        "installateur": workspace.get("nexul-installer"),
        "NEXUL Studio natif": native.get("nexul-studio-app"),
    }
    missing = [name for name, version in distributed.items() if version is None]
    if missing:
        sys.exit("Paquets Cargo introuvables pour vérifier les versions : " + ", ".join(missing))
    versions = set(distributed.values())
    if len(versions) != 1:
        details = ", ".join(f"{name}={version}" for name, version in distributed.items())
        sys.exit(
            "Versions de distribution incohérentes : " + details +
            ". Alignez les versions avant de construire l'installateur."
        )
    print("versions de distribution :", next(iter(versions)))


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.expanduser("~"), "Desktop", "NEXUL Setup.exe")
    print("== vérification des versions")
    check_release_versions()
    print("== compilation")
    subprocess.check_call(["cargo", "build", "--release", "--workspace"], cwd=ROOT)
    print("== Studio natif (MSVC)")
    app = os.path.join(ROOT, "studio", "app")
    subprocess.check_call(["cargo", "+stable-x86_64-pc-windows-msvc", "build", "--release"], cwd=app)
    if os.path.isdir(STAGING):
        shutil.rmtree(STAGING)
    os.makedirs(STAGING)
    llvm = llvm_dir()
    st = lambda comp, *p: os.path.join(STAGING, comp, *p)

    print("== composants")
    # core
    copy(os.path.join(TARGET, "nxc.exe"), st("core", "bin", "nxc.exe"))
    copy(os.path.join(TARGET, "nxpm.exe"), st("core", "bin", "nxpm.exe"))
    for f in ["README.md", "LICENSE-MIT", "LICENSE-APACHE", "CHANGELOG.md"]:
        if os.path.isfile(os.path.join(ROOT, f)):
            copy(os.path.join(ROOT, f), st("core", f if not f.startswith("LICENSE") else f + ".txt"))
    copy(os.path.join(ROOT, "tools", "wasi-run.mjs"), st("core", "tools", "wasi-run.mjs"))
    # llvm
    copy(os.path.join(llvm, "bin", "LLVM-C.dll"), st("llvm", "llvm", "bin", "LLVM-C.dll"))
    copy(os.path.join(llvm, "bin", "lld.exe"), st("llvm", "llvm", "bin", "lld.exe"))
    # The license of LLVM: Apache 2.0 with the LLVM exceptions.
    with open(os.path.join(ROOT, "LICENSE-APACHE"), encoding="utf-8") as f:
        apache = f.read()
    with open(os.path.join(ROOT, "tools", "licenses", "LLVM-exception.txt"), encoding="utf-8") as f:
        exception = f.read()
    with open(st("llvm", "llvm", "LICENSE-LLVM.txt"), "w", encoding="utf-8") as f:
        f.write("LLVM, LLD and LLDB (https://llvm.org) are distributed under the Apache License v2.0\n"
                "with LLVM Exceptions. The files in this folder are unmodified binaries of the\n"
                "official LLVM release.\n\n"
                "==============================================================================\n"
                + apache + "\n" + exception)
    # studio
    copy(os.path.join(app, "target", "release", "NEXUL-Studio-Native.exe"), st("studio", "bin", "NEXUL Studio.exe"))
    copy(os.path.join(TARGET, "NEXUL-Studio.exe"), st("studio", "bin", "NEXUL Studio (web).exe"))
    copy_tree(os.path.join(ROOT, "studio", "plugins"), st("studio", "bin", "plugins"))
    copy(os.path.join(ROOT, "studio", "assets", "nexul-studio.ico"), st("studio", "bin", "nexul-studio.ico"))
    # debugger
    for f in ["lldb-dap.exe", "liblldb.dll", "lldb-server.exe", "lldb-argdumper.exe"]:
        copy(os.path.join(llvm, "bin", f), st("debugger", "llvm", "bin", f))
    subprocess.check_call([sys.executable, "-I", os.path.join(ROOT, "tools", "make-python-runtime.py"), python_dir(), st("debugger", "llvm", "bin", "python")])
    # samples
    copy_tree(os.path.join(ROOT, "examples"), st("samples", "examples"), (".nx",))
    copy_tree(os.path.join(ROOT, "docs"), st("samples", "docs"), (".md",))

    with open(os.path.join(STAGING, "components.txt"), "w", encoding="utf-8") as f:
        for (cid, req, dflt, name, desc) in COMPONENTS:
            f.write(f"{cid}\t{int(req)}\t{int(dflt)}\t{name}\t{desc}\n")

    print("== paquetage")
    subprocess.check_call([os.path.join(TARGET, "nexul-pack.exe"), os.path.join(TARGET, "NEXUL-Setup.exe"), STAGING, out])
    print("installateur :", out)


if __name__ == "__main__":
    main()
