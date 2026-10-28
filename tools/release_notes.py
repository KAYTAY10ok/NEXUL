"""Writes the notes of a release from its section of CHANGELOG.md.

usage: python -I tools/release_notes.py <version> <notes.md> [<title.txt>]

Writes the title of the version (`NEXUL 0.3.0 — « Refonte »`) to <title.txt>, in UTF-8
(the console of Windows would not keep its characters), or prints it; the notes are an
introduction on installing, then the section `## <version>` of CHANGELOG.md, up to the
next `## ` heading. Fails if the changelog has no such section.
"""

import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

INTRO = """## Installer sur Windows

Téléchargez **NEXUL-Setup.exe** ci-dessous et lancez-le (sans droits d'administrateur ;
installation sans fenêtre : `NEXUL-Setup.exe --silent`, voir
[docs/installation.md](https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/installation.md)).
Windows peut afficher un avertissement SmartScreen, l'exécutable n'étant pas signé :
*Informations complémentaires → Exécuter quand même*.

Depuis les sources (Windows, Linux, macOS) : Rust et LLVM 20 avec LLD, puis
`cargo build --release`.

## Nouveautés

"""


def main():
    version, out = sys.argv[1].lstrip("v"), sys.argv[2]
    with open(os.path.join(ROOT, "CHANGELOG.md"), encoding="utf-8") as f:
        text = f.read()
    m = re.search(r"^## " + re.escape(version) + r"\b([^\n]*)\n(.*?)(?=^## |\Z)", text, re.S | re.M)
    if not m:
        sys.exit(f"CHANGELOG.md has no section for {version}")
    name = m.group(1).strip(" —-")
    with open(out, "w", encoding="utf-8") as f:
        f.write(INTRO + m.group(2).strip() + "\n")
    title = f"NEXUL {version} — {name}" if name else f"NEXUL {version}"
    if len(sys.argv) > 3:
        with open(sys.argv[3], "w", encoding="utf-8") as f:
            f.write(title)
    else:
        print(title)


if __name__ == "__main__":
    main()
