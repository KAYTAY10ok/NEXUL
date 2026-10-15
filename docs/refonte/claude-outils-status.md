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
- Branches : une par brique, comme Codex Luna. `refonte/claude-tools` a porté
  `studio/syntax` (PR #6, fusionnée) ; `refonte/claude-tools-lsp` a porté `studio/lsp`
  (PR #8, fusionnée) ; `refonte/claude-tools-etudes` porte les études écrites.
- Conséquence pour plus tard : l'installateur doit empaqueter `NEXUL-Studio-Native.exe`,
  construit avec la chaîne MSVC. Sur ce PC, il faudra soit installer les outils C++ de
  Visual Studio et le Windows SDK (accord de l'utilisateur nécessaire), soit utiliser
  l'exécutable construit sur l'autre PC. Sans LLVM (`llvm-rc`), l'installateur construit
  ici n'a ni icône ni manifeste (`installer/build.rs`).

## Travaux réalisés

| Date | Travail | Commit |
|---|---|---|
| 15/10 | **`studio/syntax`** — coloration lexicale de NEXUL, sans dépendance : API `tokens(lang, texte)`, API par ligne avec état (`tokenize_line`, `LineState`), `Highlighter` incrémental (`replace`, `edit_lines`), 16 classes (les 15 proposées + `Invalid`) ; tolérante aux erreurs ; 26 tests dont la conformité jeton par jeton avec le lexer de `nxc` ; documentation : [studio/syntax/README.md](../../studio/syntax/README.md) | `aec4927` |
| 15/10 | **`studio/lsp`** — client Language Server Protocol générique, sans dépendance (fils et canaux) : transport `Content-Length`, `initialize`, `didOpen`/`didChange` (complet et incrémental)/`didSave`/`didClose`, `publishDiagnostics`, `hover`, `completion`, `definition`, `references`, `rename`, `formatting`, annulation, délais, arrêt propre ; registre de 11 langages et 13 serveurs, 12 SDK, détection vérifiée sans téléchargement (« coloration seulement » sinon) ; 32 tests dont un faux serveur LSP et rust-analyzer réel ; documentation : [studio/lsp/README.md](../../studio/lsp/README.md) | `e85a252` |
| 15/10 | **Études écrites** (installation et distribution) : [interface native de l'installateur](../../installer/docs/etude-interface-native.md) (Win32 sans dépendance recommandé ; inventaire des 10 appels à PowerShell et des appels à `reg.exe`, coûts mesurés ; empaquetage de Studio natif) ; [distribution Linux et macOS](../../installer/docs/etude-linux-macos.md) (archive + `install.sh` sous Linux, `.app` signé et notarisé sous macOS, bloqué par la couche système macOS du runtime) ; [mécanisme de mise à jour](../../installer/docs/etude-mises-a-jour.md) (manifeste signé Ed25519, `nexul-update`, un dossier par version, `winget`) | |
| 15/10 | **Installateur, étape 1 de l'interface native** (accord de l'utilisateur) : PowerShell, `reg.exe` et `taskkill` remplacés par l'API de Windows, sans dépendance (`installer/src/win/` : registre, PATH et `NEXUL_HOME`, raccourcis `IShellLinkW`, dossiers connus, choix du dossier `IFileOpenDialog`, élévation `ShellExecuteEx`, arrêt des seuls programmes du dossier traité) ; désinstallateur effacé par une copie dans `%TEMP%` ; interface HTML inchangée ; 13 tests unitaires + test du choix du dossier + installation complète de bout en bout dans un bac à sable | |

## Travaux en cours

- Interface native de l'installateur, étapes 2 et 3 (assistant Win32, retrait de l'interface
  HTML) : à faire après relecture de l'étape 1.
- Autres décisions de l'utilisateur en attente : fin de chaque étude (Linux et macOS, mises
  à jour, construction de Studio natif).

## Fichiers modifiés

- `studio/syntax/` (nouveau) : `Cargo.toml`, `Cargo.lock`, `.gitignore`, `rustfmt.toml`,
  `README.md`, `src/lib.rs`, `src/nexul.rs`, `tests/common/mod.rs`, `tests/corpus.rs`,
  `tests/conformance.rs`.
- `studio/lsp/` (nouveau) : `Cargo.toml`, `Cargo.lock`, `.gitignore`, `rustfmt.toml`,
  `README.md`, `src/lib.rs`, `src/json.rs`, `src/rpc.rs`, `src/types.rs`, `src/client.rs`,
  `src/registry.rs`, `src/bin/fake_server.rs`, `tests/common/mod.rs`, `tests/client.rs`,
  `tests/detect.rs`, `tests/real_servers.rs`.
- `installer/docs/` (nouveau) : `etude-interface-native.md`, `etude-linux-macos.md`,
  `etude-mises-a-jour.md` ; `docs/installation.md` (renvoi vers ces études).
- Installateur (étape 1) : `installer/src/win/` (nouveau : `mod.rs`, `ffi.rs`, `reg.rs`,
  `env.rs`, `shell.rs`, `process.rs`), `installer/src/main.rs`, `installer/src/old.rs`,
  `installer/Cargo.toml` (fonctionnalité `test-sandbox`, programme et test de test, aucune
  dépendance), `installer/tests/silent.rs`, `installer/tests/support/sleeper.rs`,
  `docs/installation.md`.
- `docs/refonte/claude-outils-status.md` (ce fichier).

Aucun autre fichier du dépôt n'est modifié. `compiler/nxc_syntax` est utilisé en
**lecture seule**, comme dépendance de développement du test de conformité de
`studio/syntax`.

## Tests exécutés

Rust 1.99.0 `windows-gnu`, en debug et en `--release`. `cargo clippy --all-targets` :
aucun avertissement ; `cargo fmt --check` : conforme (configuration des crates :
120 colonnes).

**Installateur (étape 1)** :

- `cargo test -p nexul-installer` : **13 tests unitaires réussis**, en 1,4 à 1,8 s. Ils
  couvrent :
  - registre (texte, texte à variables, nombre, valeurs longues et accentuées,
    arborescence) ;
  - PATH (forme brute conservée, comparaison après expansion, dossiers inclus, pas de
    doublon) ;
  - raccourcis (création, relecture, remplacement, chemins accentués) ;
  - dossiers connus ;
  - lancement et attente avec code de sortie ;
  - arrêt des programmes d'un seul dossier : deux « NEXUL Studio.exe », seul celui du
    dossier traité est arrêté ;
  - les 2 tests existants de `old.rs`.
- `--ignored picker` : la vraie boîte de choix du dossier s'ouvre, répond OK (dossier
  existant le plus proche) puis Annuler.
- `cargo test -p nexul-installer --features test-sandbox` : test `silent` réussi en 14 s,
  3 passages sur 3. C'est le vrai installateur, construit par `nexul-pack` :
  - installation ;
  - mise à jour par-dessus l'installation précédente et une ancienne copie de NEXUL
    Studio (sauvegardée et détachée), avec un Studio lancé (arrêté) et un autre ailleurs
    (épargné) ;
  - désinstallation avec `nxc` lancé et un fichier de l'utilisateur (conservé) ;
  - vérifications indépendantes par `reg.exe` et Windows Script Host.
- Après les tests, vérifié en PowerShell : aucune clé de test, aucun processus, ni
  inscription, PATH, `NEXUL_HOME`, association ou raccourci réels touchés.
- `cargo clippy -p nexul-installer --features test-sandbox --all-targets` : aucun
  avertissement.
- Programme seul, release : 828 Ko (768 Ko avant).

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
- Installateur : une première version de `ShellExecuteEx` sans `SEE_MASK_FLAG_NO_UI`
  affichait une boîte d'erreur de Windows quand le programme n'existe pas. Le test
  correspondant restait bloqué jusqu'à la fermeture de la boîte (80 s, 120 s, puis
  arrêt), et ces boîtes ont pu apparaître à l'écran de l'utilisateur. C'est corrigé :
  l'erreur revient au programme, et la demande d'élévation de Windows reste affichée.
- Sous Git Bash, `reg … /f` est converti en chemin par MSYS (`/f` → `F:/`) : les
  vérifications de l'environnement réel se font en PowerShell. Les tests Rust ne sont
  pas concernés, car ils passent leurs arguments directement.

## Demandes aux autres agents

**Ouverte** :

1. **Claude Studio** — vérifier si `NEXUL-Studio-Native.exe` dépend de
   `vcruntime140.dll` (`dumpbin /dependents`), et dans ce cas le construire avec
   `-C target-feature=+crt-static`. Un Windows neuf n'a pas ce runtime, et l'installateur
   doit pouvoir livrer Studio seul. Issue : [#9](https://github.com/KAYTAY10ok/NEXUL/issues/9).

**Envisagées**, à ouvrir selon les décisions de l'utilisateur sur les études :

- **Claude Studio** : réglages de Studio dans les emplacements de Linux
  (`$XDG_CONFIG_HOME`) et de macOS (`~/Library/Application Support`) ; notification et
  bouton de mise à jour dans Studio (`nexul-update --check/--apply`).
- **Codex Astra** :
  - dépendance de vérification Ed25519 dans `installer/` (workspace racine) ;
  - cache de `nxc` et `nxpm` aux emplacements XDG et macOS ;
  - matrice d'intégration continue Linux et macOS (`.github/**`) ;
  - la couche système macOS du runtime, déjà relevée dans `audit-core.md`, conditionne
    toute distribution sur Mac.

`studio/syntax` n'a eu besoin d'aucune information lexicale absente de la spécification
ou du compilateur.

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
