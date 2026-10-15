# État des travaux de Claude Outils — briques de Studio, installation, distribution

Agent : **Claude Outils**. Périmètre (voir [AGENTS.md](AGENTS.md), §3) : `studio/syntax/**`,
`studio/lsp/**`, `installer/**`, `tools/build_installer.py`, `tools/make-python-runtime.py`,
`tools/licenses/**`, `docs/installation.md` et ce fichier. Branche : `refonte/claude-tools`.
Seul Claude Outils modifie ce document.

Dernière mise à jour : 15 octobre 2026.

## Environnement

Claude Outils ne travaille **pas** sur le PC décrit au §4 d'AGENTS.md : ici, ni `C:\NEXUL`,
ni LLVM, ni les outils C++ de Visual Studio (Visual Studio 2022 et 2026 sont installés
sans la charge « Développement Desktop en C++ »).

- Dépôt : clone de GitHub dans `C:\NEXUL-tools` (et non un *worktree* de `C:\NEXUL`),
  branche `refonte/claude-tools` créée depuis `origin/main` (`d537a7b`), sans suivi de
  `origin/main` pour qu'un `git push` ne puisse pas y aller.
- Installés avec l'accord de l'utilisateur (15/10) : rustup, Rust **1.99.0**
  `stable-x86_64-pc-windows-gnu` par défaut (même version que l'autre PC), la chaîne
  `stable-x86_64-pc-windows-msvc` (inutilisable ici sans `link.exe`), GitHub CLI 2.102.0,
  le composant `rust-analyzer` (pour essayer le client LSP contre un vrai serveur).
- Branches : une par brique, comme Codex Luna. `refonte/claude-tools` porte
  `studio/syntax` (PR #6) ; `refonte/claude-tools-lsp`, partie de celle-ci, porte
  `studio/lsp` (PR empilée sur #6 : à fusionner après elle).
- Conséquence pour plus tard : l'installateur doit empaqueter `NEXUL-Studio-Native.exe`,
  construit avec la chaîne MSVC. Sur ce PC, il faudra soit installer les outils C++ de
  Visual Studio et le Windows SDK (accord de l'utilisateur nécessaire), soit utiliser
  l'exécutable construit sur l'autre PC.

## Travaux réalisés

| Date | Travail | Commit |
|---|---|---|
| 15/10 | **`studio/syntax`** — coloration lexicale de NEXUL, sans dépendance : API `tokens(lang, texte)`, API par ligne avec état (`tokenize_line`, `LineState`), `Highlighter` incrémental (`replace`, `edit_lines`), 16 classes (les 15 proposées + `Invalid`) ; tolérante aux erreurs ; 26 tests dont la conformité jeton par jeton avec le lexer de `nxc` ; documentation : [studio/syntax/README.md](../../studio/syntax/README.md) | `aec4927` |
| 15/10 | **`studio/lsp`** — client Language Server Protocol générique, sans dépendance (fils et canaux) : transport `Content-Length`, `initialize`, `didOpen`/`didChange` (complet et incrémental)/`didSave`/`didClose`, `publishDiagnostics`, `hover`, `completion`, `definition`, `references`, `rename`, `formatting`, annulation, délais, arrêt propre ; registre de 11 langages et 13 serveurs, 12 SDK, détection vérifiée sans téléchargement (« coloration seulement » sinon) ; 32 tests dont un faux serveur LSP et rust-analyzer réel ; documentation : [studio/lsp/README.md](../../studio/lsp/README.md) | |

## Travaux en cours

- Installation et distribution (troisième brique) : l'installateur doit empaqueter
  l'application native ; voir « Environnement » pour la chaîne MSVC.

## Fichiers modifiés

- `studio/syntax/` (nouveau) : `Cargo.toml`, `Cargo.lock`, `.gitignore`, `rustfmt.toml`,
  `README.md`, `src/lib.rs`, `src/nexul.rs`, `tests/common/mod.rs`, `tests/corpus.rs`,
  `tests/conformance.rs`.
- `studio/lsp/` (nouveau) : `Cargo.toml`, `Cargo.lock`, `.gitignore`, `rustfmt.toml`,
  `README.md`, `src/lib.rs`, `src/json.rs`, `src/rpc.rs`, `src/types.rs`, `src/client.rs`,
  `src/registry.rs`, `src/bin/fake_server.rs`, `tests/common/mod.rs`, `tests/client.rs`,
  `tests/detect.rs`, `tests/real_servers.rs`.
- `docs/refonte/claude-outils-status.md` (ce fichier).

Aucun autre fichier du dépôt n'est modifié. `compiler/nxc_syntax` est utilisé en
**lecture seule**, comme dépendance de développement du test de conformité de
`studio/syntax`.

## Tests exécutés

Rust 1.99.0 `windows-gnu`, en debug et en `--release`. `cargo clippy --all-targets` :
aucun avertissement ; `cargo fmt --check` : conforme (configuration des crates :
120 colonnes).

**`studio/lsp`** : **32 tests, tous réussis**, et 10 passages consécutifs des tests du
client et de la détection sans échec.

- 16 unitaires ;
- 10 contre le faux serveur `fake-lsp-server` : session complète, UTF-8 négocié,
  délais et annulation, plantage, message illisible, erreurs, abandon du client,
  400 requêtes concurrentes, serveur `.cmd`, arrêt de toute l'arborescence. Ce dernier
  test échoue bien quand on retire `taskkill /T`, ce qui a été vérifié ;
- 5 de détection ;
- 1 contre rust-analyzer 1.99.0 réel : diagnostic E0061, survol, définition, références,
  complétion, renommage, formatage, arrêt avec le code 0.

Détection réelle sur ce PC, en 0,5 s : aucun serveur LSP (le proxy rustup
`rust-analyzer` sans composant est bien écarté ; l'alias Store `python3` n'est pas
exécuté), SDK Python 3.14.7, Node 24.19.0, Cargo et rustc 1.99.0, .NET 10.0.401. Le
composant rust-analyzer a été installé ensuite pour le test réel.

**`studio/syntax`** : **26 tests, tous réussis** (17 unitaires, 7 sur le corpus, 2 de
conformité).

- Les 59 fichiers `examples/*.nx`, `tests/run/*.nx` et `lib/**.nx` : aucun jeton invalide,
  aucune perte de texte ; les 128 fichiers `.nx` du dépôt : garanties respectées,
  `Highlighter` identique à `tokens`.
- Conformité : 126 fichiers concordent avec `nxc_syntax::lexer` (69 074 jetons) ; dans les
  2 fichiers d'erreurs lexicales (`tests/ui/lex_bidi.nx`, `lex_errors.nx`), les 7 erreurs
  que la coloration doit montrer sont marquées `Invalid`.
- 100 000 lignes (105 463 lignes, 2,8 Mo), `--release` : tokenisation complète 20 à 24 ms,
  `Highlighter::new` 45 à 48 ms, frappe 0,4 ms, retokenisation après `/*` en tête 9 ms.

## Problèmes rencontrés

- Environnement différent de celui d'AGENTS.md (voir plus haut) : résolu pour les crates
  Rust pures ; reste à régler pour l'installateur (chaîne MSVC).
- Le dépôt n'impose pas de formatage (le code existant n'est conforme à aucune
  configuration courante de `rustfmt`) : `studio/syntax/rustfmt.toml` fixe 120 colonnes,
  ce qui est le plus proche du style existant, pour cette crate seulement.

## Demandes aux autres agents

Aucune pour l'instant. `studio/syntax` n'a eu besoin d'aucune information lexicale absente
de la spécification ou du compilateur.

## Briques prêtes (pour Claude Studio)

- **`studio/syntax`** est prête à être intégrée dans `studio/app`
  (`nexul-studio-syntax = { path = "../syntax" }`). L'adaptateur GPUI peut s'appuyer sur
  `Highlighter::replace` (modifications de l'éditeur en octets) et `line_tokens` (lignes
  visibles), ou sur `tokenize_line` avec son propre cache. Correspondance proposée avec
  l'ancienne interface (`studio/ui/langs.js`) : `kw`→`Keyword`, `ctl`→`Control`,
  `type`→`Type`, `fn`/`fndef`→`Function`, `ident`→`Variable`, `const`→`Constant`,
  `str`→`String`, `esc`→`Escape`, `interp`→`Interpolation`, `num`→`Number`,
  `comment`→`Comment`/`DocComment`, `attr`→`Attribute`, `op`→`Operator`,
  `punct`/`b0..b2`→`Punctuation` (les crochets « arc-en-ciel » se calculent sur les
  jetons `Punctuation`), `self`→`Keyword`/`Type`. Nouveau : `Invalid` (souligner en rouge).

- **`studio/lsp`** est prête à être intégrée dans `studio/app`
  (`nexul-studio-lsp = { path = "../lsp" }`). Au démarrage :
  `Detector::from_env().detect()` sur un fil de fond, puis
  `LanguageStatus::label()` dans la barre d'état (« coloration seulement » sans serveur).
  Pour chaque dossier et langage : `Client::start(&server.command, …)`. Les réponses
  arrivent par `Pending::wait` sur des tâches de fond, ou par `try_take` ; les
  diagnostics et la fin du serveur par le canal d'`Event`. Pour les modèles de Codex
  Luna, le pont `ToolchainProvider` est donné dans le README : les identifiants de SDK
  sont exactement ceux de son `enum Tool`.

## Changements d'interface

- Nouvelle API Rust `nexul-studio-lsp` (voir son README). Les identifiants de SDK sont
  les noms des exécutables (`nxc`, `g++`, `cl`…), alignés sur `studio/templates`.
- Nouvelle API Rust `nexul-studio-syntax` (voir son README). Écart avec l'API proposée
  dans AGENTS.md : une 16ᵉ classe, `Kind::Invalid`, pour ce que le compilateur refuse
  (Trojan Source, échappement inconnu, `!` seul…), sans quoi « aucune perte de texte »
  obligerait à mal classer ces caractères. Langage : `"nexul"` (ou `"nx"`) ; un langage
  inconnu donne une liste vide.
