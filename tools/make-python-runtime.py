"""Builds the minimal Python runtime needed by lldb-dap (the debugger of NEXUL Studio).

LLDB embeds Python for its scripting features: without a Python runtime it does not
start. This script copies, from an installed CPython, the DLLs and a zip of the pure
Python standard library (without tests, tkinter, idlelib...), plus a `._pth` file so that
the runtime only looks at its own folder.

usage: python -I tools/make-python-runtime.py <python install dir> <output dir>
"""

import os
import shutil
import sys
import zipfile

SKIP_DIRS = {"test", "tests", "idlelib", "tkinter", "turtledemo", "ensurepip", "site-packages", "__pycache__", "lib2to3", "venv", "pydoc_data"}


def main():
    src, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    dlls = [f for f in os.listdir(src) if f.lower().endswith(".dll")]
    version = next(f[len("python"):-4] for f in dlls if f.lower().startswith("python3") and f.lower() != "python3.dll")
    for f in dlls:
        shutil.copy2(os.path.join(src, f), os.path.join(out, f))
    # Extension modules used by the standard library.
    for f in os.listdir(os.path.join(src, "DLLs")):
        if f.lower().endswith((".pyd", ".dll")) and not f.lower().startswith(("_tkinter", "tcl", "tk")):
            shutil.copy2(os.path.join(src, "DLLs", f), os.path.join(out, f))
    lib = os.path.join(src, "Lib")
    zpath = os.path.join(out, f"python{version}.zip")
    with zipfile.ZipFile(zpath, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for root, dirs, files in os.walk(lib):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
            for f in files:
                if f.endswith(".py"):
                    p = os.path.join(root, f)
                    z.write(p, os.path.relpath(p, lib))
    with open(os.path.join(out, f"python{version}._pth"), "w") as f:
        f.write(f"python{version}.zip\n.\n")
    lic = os.path.join(src, "LICENSE.txt")
    if os.path.exists(lic):
        shutil.copy2(lic, os.path.join(out, "LICENSE-Python.txt"))
    print(f"python{version} runtime in {out}")


if __name__ == "__main__":
    main()
