# 10 — Le compilateur `nxc`

## 10.1 Architecture

```
 source .nx ──► Lexer ──► Parser ──► AST ──► Résolution des noms ──► Vérification des types
                                                                         │ (inférence, méthodes,
                                                                         │  exhaustivité)
                                                                         ▼
     exécutable ◄── zig cc ◄── C11 ◄── Backend C ◄── Monomorphisation ◄── NIR (IR typée, CFG)
                                                                         ▲
                                         Analyse de propriété ───────────┘
                                         (initialisation, déplacements, drapeaux
                                          de destruction, exclusivité)
```

| Composant (crate Rust) | Rôle |
|---|---|
| `nxc_syntax` | sources, positions, diagnostics, lexer, parser, AST |
| `nxc_sema` | modules, résolution, types, inférence, THIR (arbre typé), exhaustivité |
| `nxc_nir` | **NIR** : IR typée en graphe de blocs de base, construction depuis le THIR, analyse de propriété, élaboration des destructions |
| `nxc_codegen_c` | monomorphisation et génération de C11 portable |
| `nxc` (driver) | ligne de commande, recherche de la toolchain, tests |

Choix structurants (détaillés dans `docs/adr/`) :
- compilateur écrit en **Rust**, **sans aucune dépendance externe** (cahier des charges §16.1) ;
- frontend et backends séparés par la **NIR**, indépendante de la cible (§3.1) ;
- premier backend : **C11 portable**, compilé par `zig cc` (Clang/LLVM), ce qui donne
  immédiatement la compilation croisée vers x86-64, ARM64, RISC-V, WebAssembly, Linux,
  macOS et Windows (§3.3). Un backend LLVM IR direct suivra sur la même NIR ;
- le C produit contient des directives `#line` : débogueurs et messages du compilateur C
  pointent vers le **source NEXUL**.

## 10.2 NIR (NEXUL Intermediate Representation)

Pour chaque fonction : une liste de variables locales typées et un graphe de blocs de
base. Chaque bloc contient des instructions (`place = rvalue`, `drop(place)`,
vérifications) et se termine par un terminateur (`goto`, `switch`, `call`, `return`,
`unreachable`). Les places sont des chemins `local.champ[indice]...` explicites.
Toutes les opérations implicites du source y deviennent explicites : temporaires,
destructions, vérifications de débordement et de bornes, drapeaux de destruction.

C'est sur la NIR que s'effectuent l'analyse d'initialisation et de déplacement (flot de
données avant) et l'élaboration des destructions.

## 10.3 Ligne de commande

```
nxc run    programme.nx [-- arguments]   compile et exécute
nxc build  programme.nx [-o sortie] [--release] [--target <triplet>] [--emit c]
                                         (--emit c : écrit le C généré au lieu de l'exécutable)
nxc check  programme.nx                  analyse sans générer de code
nxc test   programme.nx                  exécute les fonctions @test
nxc explain E0402                        explique un code de diagnostic
nxc version
```

- Mode **debug** par défaut (`-O0 -g`), **release** avec `--release` (`-O2`). Toutes les
  vérifications de sûreté (débordement, bornes) sont actives **dans les deux modes**.
- `--target` accepte les triplets de Zig (`x86_64-linux-gnu`, `aarch64-macos`,
  `wasm32-wasi`, `riscv64-linux-musl`...).
- La toolchain C est cherchée dans cet ordre : variable `NEXUL_CC`, `zig` dans le `PATH`.
- Code de sortie de `nxc` : 0 en cas de succès, 1 en cas d'erreur de compilation.

## 10.4 Validation

| Niveau | Contenu |
|---|---|
| Tests unitaires Rust | lexer, parser, rendu des diagnostics, suggestions |
| `tests/run/*.nx` | programmes compilés et exécutés **avec détection de fuites** (`NEXUL_LEAK_CHECK`) ; sortie comparée à `*.out`, code de sortie à `*.exit`, erreurs à `*.stderr` |
| `tests/ui/*.nx` | programmes erronés ; diagnostics comparés à `*.stderr` |
| Documentation | chaque exemple complet du README, du guide et de la spec est compilé |
| Catalogue | chaque code émis est expliqué ; `diagnostics.md` est généré et vérifié |
| Fuzzing | entrées aléatoires et mutations du corpus : le compilateur ne doit jamais planter |
| Exemples | `examples/` compilés sans aucun diagnostic |

La génération de code est **déterministe** : un même programme produit un C identique à
l'octet près à chaque compilation (builds reproductibles, cahier des charges §11.4).

Chaque règle normative de cette spécification doit être couverte par au moins un test.
`cargo test` exécute l'ensemble.
