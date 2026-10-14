# 10 — Le compilateur `nxc`

## 10.1 Architecture

```
 source .nx ──► Lexer ──► Parser ──► AST ──► @derive, @cfg ──► Résolution des noms
                                                                    │
                                                                    ▼
                         Vérification des types (inférence, traits, méthodes,
                         fonctions anonymes, opérateurs, exhaustivité) ──► THIR
                                                                    │
                                                                    ▼
                         NIR (IR typée, CFG) ◄── analyse de propriété (initialisation,
                                                  déplacements, drapeaux de destruction,
                                                  exclusivité)
                                                                    │
                                                                    ▼
                         Monomorphisation ──► IR LLVM (texte) + informations de débogage
                                                                    │
                          bibliothèque LLVM (chargée dynamiquement) ▼
                         analyse de l'IR ──► optimisation ──► fichier objet
                                                                    │
                                                    LLD             ▼
                                                    exécutable, bibliothèque, module WebAssembly,
                                                    micrologiciel
```

| Composant (crate Rust) | Rôle |
|---|---|
| `nxc_syntax` | sources, positions, diagnostics, lexer, parser, AST |
| `nxc_sema` | modules, `@derive`, `@cfg`, résolution, traits, types, inférence, THIR (arbre typé), exhaustivité, description des cibles |
| `nxc_nir` | **NIR** : IR typée en graphe de blocs de base, construction depuis le THIR, analyse de propriété, élaboration des destructions |
| `nxc_codegen_llvm` | monomorphisation, disposition mémoire, génération d'IR LLVM textuelle, appels de traits et tables de `dyn`, informations de débogage (DWARF ou CodeView) |
| `nxc` (pilote) | ligne de commande, chargement de LLVM, édition de liens, cache, services d'éditeur (`nxc ide`), formateur, `nxpm`, suite de conformité |

Choix structurants (détaillés dans `docs/adr/`) :
- compilateur écrit en **Rust**, **sans aucune dépendance externe** (cahier des charges §16.1) ;
- frontend et backend séparés par la **NIR**, indépendante de la cible (§3.1) ;
- backend **LLVM** sans lien à la compilation : `nxc` produit l'IR LLVM sous forme de
  texte et la confie à la bibliothèque C de LLVM (`LLVM-C.dll`, `libLLVM.so`), chargée à
  l'exécution, qui l'optimise et produit le code machine ([ADR 0008](../adr/0008-backend-llvm-et-runtime-nexul.md)) ;
- **aucun compilateur C** : le runtime est écrit en NEXUL (§09) ; l'édition de liens est
  faite par **LLD** ; sur Windows, `nxc` écrit lui-même les bibliothèques d'importation
  des DLL du système, sur Linux les exécutables sont statiques et appellent directement
  le noyau, en WebAssembly le module importe les fonctions WASI.

## 10.2 NIR (NEXUL Intermediate Representation)

Pour chaque fonction : une liste de variables locales typées et un graphe de blocs de
base. Chaque bloc contient des instructions (`place = rvalue`, `drop(place)`,
vérifications) et se termine par un terminateur (`goto`, `switch`, `call`, `return`,
`unreachable`). Les places sont des chemins `local.champ[indice]...` explicites.
Toutes les opérations implicites du source y deviennent explicites : temporaires,
destructions, vérifications de débordement et de bornes, drapeaux de destruction.

C'est sur la NIR que s'effectuent l'analyse d'initialisation et de déplacement (flot de
données avant) et l'élaboration des destructions. Les appels de méthodes de traits y
restent génériques : la monomorphisation choisit l'implémentation (ou la table de
méthodes d'un `dyn`).

## 10.3 Ligne de commande

```
nxc run    programme.nx [options] [-- arguments]   compile et exécute
nxc build  programme.nx [options]                  exécutable ou bibliothèque
nxc check  programme.nx                            analyse sans générer de code
nxc test   programme.nx [nom...]                   exécute les fonctions @test
nxc fmt    programme.nx [--check]                  met en forme
nxc ide    <requête> programme.nx [ligne colonne]  services d'éditeur (JSON)
nxc explain E0402                                  explique un code de diagnostic
nxc targets                                        liste les cibles
nxc version
```

| Option | Effet |
|---|---|
| `-o <chemin>` | fichier produit |
| `--release` | optimise (`-O2`) ; toutes les vérifications de sûreté restent actives |
| `-O0` … `-O3`, `-Os`, `-Oz` | niveau d'optimisation (taille avec `s` et `z`) |
| `--target <nom>` | compilation croisée (§10.5) |
| `--lib`, `--staticlib` | bibliothèque partagée ou statique des fonctions `@export` (§8.4) |
| `--emit llvm-ir` | écrit l'IR LLVM optimisée au lieu du binaire |
| `--no-debug` | sans informations de débogage |
| `--link-arg <arg>` | argument supplémentaire pour l'éditeur de liens |
| `--json` | diagnostics en JSON sur la sortie standard (éditeurs) |

- Mode **debug** par défaut (`-O0`, informations de débogage : PDB sur Windows, DWARF
  ailleurs), **release** avec `--release`. Les vérifications de sûreté (débordement,
  bornes, conversions) sont actives **dans les deux modes**.
- `nxc run` et `nxc test` placent les binaires dans un cache (`%LOCALAPPDATA%\nexul\cache`,
  `~/.cache/nexul` ; `NEXUL_CACHE_DIR` pour le déplacer, `NEXUL_NO_CACHE` pour le
  désactiver) : un programme inchangé n'est pas recompilé.
- Dans un projet `nxpm` (`nexul.toml`), le fichier principal et les dépendances sont
  trouvés automatiquement.
- Code de sortie de `nxc` : 0 en cas de succès, 1 en cas d'erreur de compilation.

## 10.4 Recherche de LLVM

Dans l'ordre : la variable `NEXUL_LLVM` (dossier de LLVM, ou ses sous-dossiers `bin` et
`lib`) ; un dossier `llvm` installé avec NEXUL (`<dossier de nxc>/llvm` ou
`<dossier de nxc>/../llvm`) ; les emplacements habituels (`C:\Program Files\LLVM`,
`/usr/lib/llvm-*`, Homebrew) et le `PATH`. LLVM 17 ou plus récent est requis, avec les
éditeurs de liens LLD (`lld-link`, `ld.lld`, `wasm-ld`, `ld64.lld`).

## 10.5 Cibles

| Nom | Triplet LLVM | Système |
|---|---|---|
| `x86_64-windows`, `aarch64-windows` | `*-pc-windows-msvc` | Windows (DLL du système, sans bibliothèque C) |
| `x86_64-linux`, `aarch64-linux`, `riscv64-linux` | `*-unknown-linux-gnu` | Linux (exécutable statique, appels système directs) |
| `x86_64-macos`, `aarch64-macos` | `*-apple-macosx11.0.0` | macOS |
| `wasm32-wasi` | `wasm32-unknown-wasi` | WebAssembly + WASI (Node, Wasmtime, navigateurs avec WASI) |
| `thumbv7em-none`, `thumbv7m-none` | `thumbv7em-none-eabihf`, `thumbv7m-none-eabi` | ARM Cortex-M4F / Cortex-M3, sans système |
| `riscv32-none`, `riscv64-none`, `aarch64-none` | `*-unknown-none(-elf)` | RISC-V et ARM64 sans système |

`--target` accepte aussi les triplets complets (`aarch64-unknown-linux-gnu`). Les cibles
sans système (`@cfg("bare")`) fournissent la table des vecteurs, le démarrage, un script
de liens par défaut et l'accès au terminal par semi-hébergement ; `exit` et les paniques
s'y terminent par semi-hébergement (le programme reste ensuite arrêté). Sans unité flottante, les flottants sont calculés par
`std.builtins` (flottants logiciels exacts au bit près).

## 10.6 Validation

| Niveau | Contenu |
|---|---|
| Tests unitaires Rust | lexer, parser, rendu des diagnostics, suggestions, cibles, `@cfg`, formateur, paquets |
| `tests/run/*.nx` | programmes compilés et exécutés **avec détection de fuites** (`NEXUL_LEAK_CHECK`) ; sortie comparée à `*.out`, code de sortie à `*.exit`, erreurs à `*.stderr` ; aussi sur Linux (WSL), WebAssembly (Node) et microcontrôleurs (QEMU) avec `NEXUL_TEST_TARGETS` |
| `tests/ui/*.nx` | programmes erronés ; diagnostics comparés à `*.stderr` |
| `tests/numerics/` | formatage et analyse des flottants et fonctions mathématiques comparés à Python ; flottants logiciels comparés bit à bit au processeur |
| Documentation | chaque exemple complet du README, du guide et de la spécification est compilé |
| Catalogue | chaque code émis est expliqué ; `diagnostics.md` est généré et vérifié |
| Exemples | `examples/` compilés sans aucun diagnostic |

La génération de code est **déterministe** : un même programme produit une IR LLVM
identique à l'octet près à chaque compilation (builds reproductibles, cahier des charges
§11.4).

Chaque règle normative de cette spécification doit être couverte par au moins un test.
`cargo test --release` exécute l'ensemble.
