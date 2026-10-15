# NEXUL — consignes pour les agents (Codex, Claude Studio, Claude Outils, Agent Exécution)

Document de coordination rédigé à la demande de l'utilisateur (15 octobre 2026). Lisez-le
**en entier avant de commencer**. Il complète la mission « Refonte majeure du langage et
de NEXUL Studio » donnée par l'utilisateur ; en cas de doute, la mission fait foi.
Seul l'utilisateur (ou Claude Studio à sa demande) modifie ce fichier : proposez vos
changements dans votre fichier d'état ou par une issue.

## 1. Le projet

**NEXUL** est un langage de programmation compilé, sûr et autonome :

```
code NEXUL → analyse NEXUL → NIR → IR LLVM → code machine (LLD)
```

- `compiler/` : le compilateur `nxc` (Rust, aucune dépendance) — syntaxe, sémantique,
  NIR, génération d'IR LLVM, pilote, `nxpm` (paquets), services d'éditeur (`nxc ide`).
- `lib/std/` : bibliothèque standard **et runtime, écrits en NEXUL** (pas de C).
- `studio/` : **NEXUL Studio**, l'IDE officiel.
  - `studio/src/` : services Rust partagés (bibliothèque `nexul_studio` : fichiers,
    surveillance du dossier, exécution, débogueur, localisation de `nxc`) et l'ancienne
    interface (serveur HTTP local + pages web affichées par Edge) ;
  - `studio/ui/` : pages de l'ancienne interface (à retirer à la parité) ;
  - `studio/app/` : **la nouvelle application native** (GPUI + gpui-component, choix
    validé par l'utilisateur), dans son propre espace de travail Cargo.
- `installer/` : le programme d'installation Windows `NEXUL Setup.exe`.
- `docs/` : spécification (`docs/spec/`), guide, feuille de route, ADR, documentation
  de Studio (`docs/studio/`), coordination de la refonte (`docs/refonte/`).

**GitHub** : https://github.com/KAYTAY10ok/NEXUL (public). `main` = NEXUL 0.2.0
(tag `v0.2.0`, release avec `NEXUL-Setup.exe`).

**Sur ce PC** (même dépôt Git, un *worktree* par agent) :

| Emplacement | Branche | Agent |
|---|---|---|
| `C:\NEXUL` | `nexul-0.2-autonomie` (= `main`) | personne : **ne pas y travailler** |
| `C:\NEXUL-core` | `refonte/codex-core` | **Codex** |
| `C:\NEXUL\.claude\worktrees\claude-studio` | `refonte/claude-studio` | **Claude Studio** |
| `C:\NEXUL-tools` (à créer) | `refonte/claude-tools` | **Claude Outils** |
| `C:\NEXUL-exec` (à créer) | `refonte/execution` | **Agent Exécution** |

Création des worktrees de Claude Outils et de l'Agent Exécution :

```
git -C C:\NEXUL worktree add C:\NEXUL-tools -b refonte/claude-tools origin/main
git -C C:\NEXUL worktree add C:\NEXUL-exec -b refonte/execution origin/main
```

## 2. Règles communes

1. Chacun travaille **uniquement dans son worktree et sur sa branche**, et ne modifie
   **que les fichiers de son périmètre** (§3). Lire les autres branches est permis
   (`git show refonte/codex-core:chemin`), les modifier non.
2. **Jamais** de push sur `main`, de `git reset --hard`, de push forcé, de suppression
   du travail d'un autre. La pile `git stash` est **partagée** entre les worktrees : ne
   l'utilisez pas (préférez un commit temporaire).
3. Livraison par **pull request** vers `main` (titre préfixé `[core]`, `[studio]`,
   `[outils]` ou `[exec]`), avec : changements, fichiers, tests exécutés et résultats, limites,
   changements d'API. Une PR n'est fusionnée qu'après les tests et la résolution des
   conflits ; l'utilisateur décide.
4. Chaque agent tient **son fichier d'état** dans `docs/refonte/` (réalisé, en cours,
   fichiers modifiés, tests, problèmes, demandes aux autres, changements d'interface).
5. Une demande à un autre agent = une entrée « Demandes » dans votre fichier d'état
   **et** une issue GitHub (besoin, comportement attendu, critères de validation). Ne
   faites jamais vous-même le travail d'un autre.
6. **Aucun résultat inventé** : rien n'est « fait » sans compilation et tests. Une
   fonction partielle est annoncée partielle. Pas de fonctionnalité factice.
7. `Cargo.toml` et `Cargo.lock` **racine** appartiennent à Codex. `studio/app/` a son
   propre `Cargo.toml` / `Cargo.lock` (Claude Studio). Ne modifiez pas un `Cargo.lock`
   qui n'est pas à vous.
8. Ne touchez pas aux réglages réels de l'utilisateur dans les tests
   (`%APPDATA%\NEXUL Studio` : utilisez `NEXUL_STUDIO_CONFIG` pour l'application native).

## 3. Répartition des responsabilités

### Codex — NEXUL Core

**Fichiers** : `compiler/**`, `lib/**`, `tests/**`, `examples/**`, `docs/spec/**`,
`docs/guide.md`, `docs/roadmap.md`, `docs/adr/**`, `Cargo.toml` et `Cargo.lock` racine,
`README.md`, `CHANGELOG.md`, `.github/**`, `tools/wasi-run.mjs`,
`docs/refonte/codex-status.md`, `docs/refonte/audit-core.md`,
`docs/refonte/interop-web.md`.

**Tâches** (mission, parties 3.1 à 3.6) : audit du compilateur ; benchmarks
reproductibles C / Rust / C# / NEXUL (médianes, variabilité, versions documentées) ;
précision numérique (f32/f64, NaN, arrondis, mode strict / mode rapide optionnel) ;
optimisations LLVM justifiées par les mesures ; cible WebAssembly **navigateur**
(exports, imports hôte, chaînes, chargeur, exemples) et le contrat `interop-web.md`.

**Demandes de Studio en attente** (voir `docs/refonte/claude-status.md`) :
1. `nxc lsp` : serveur Language Server Protocol (stdio) sur les services de `nxc ide`.
2. Champ de version dans les sorties JSON de `nxc check --json` et `nxc ide`.
3. Contrat WebAssembly validé (cible réellement implémentée, exemple, chargeur, tests).
4. Décision : intégrer `studio/app` au workspace racine, ou le laisser séparé.

### Claude Studio — l'application NEXUL Studio

**Fichiers** : `studio/src/**`, `studio/ui/**`, `studio/app/**`, `studio/plugins/**`,
`studio/Cargo.toml`, `studio/README.md`, `studio/assets/**`, `docs/studio/**`,
`docs/refonte/claude-status.md` — **sauf** les nouveaux dossiers attribués à Claude Outils
et à l'Agent Exécution ci-dessous.

**Tâches** : l'application native `studio/app` — explorateur (créer, renommer,
supprimer, déplacer, glisser-déposer via `workspace.rs`), éditeur et onglets, panneau de
diagnostics (`nxc check --json`), palette de commandes, paramètres et raccourcis
personnalisables, thèmes, mode minimaliste, dispositions mémorisées, services NEXUL dans
l'éditeur (`nxc ide` : survol, complétion, définition, références, renommage, mise en
forme), aperçu web isolé ; puis **l'interface graphique des briques des autres agents** :
coloration et LSP (Claude Outils), terminal, débogueur et assistant « Nouveau projet »
(Agent Exécution), et la future interface WebAssembly de Codex ; parité, puis retrait de
l'ancienne interface web.

### Claude Outils — briques de Studio, installation et distribution

**Fichiers** (nouveaux dossiers, rien d'autre) :
- `studio/syntax/**` : nouvelle crate **sans dépendance à GPUI** ;
- `studio/lsp/**` : nouvelle crate **sans dépendance à GPUI** ;
- `installer/**`, `tools/build_installer.py`, `tools/make-python-runtime.py`,
  `tools/licenses/**`, `docs/installation.md` ;
- `docs/refonte/claude-outils-status.md`.

**Tâches** :
1. **`studio/syntax`** — coloration lexicale :
   - NEXUL : tokenisation en Rust pur d'après `docs/spec/01-lexique.md` et
     `02-grammaire.md` (mots-clés, types, nombres, chaînes avec interpolation `{…}`,
     caractères, commentaires dont `///`, attributs `@…`, opérateurs), **tolérante aux
     erreurs** (texte en cours de frappe) et incrémentale ou rapide (100 000 lignes) ;
   - API proposée : `fn tokens(lang: &str, text: &str) -> Vec<Token { range: Range<usize>, kind: Kind }>`
     avec `Kind` ∈ {Keyword, Control, Type, Function, Variable, Constant, String,
     Escape, Interpolation, Number, Comment, DocComment, Attribute, Operator,
     Punctuation} ;
   - tests : tous les fichiers `examples/*.nx` et `tests/run/*.nx` (lecture seule)
     doivent être tokenisés sans erreur et sans perte de texte ;
   - ne modifiez pas le compilateur : s'il manque une information lexicale, demandez à
     Codex.
2. **`studio/lsp`** — client Language Server Protocol générique :
   - transport JSON-RPC sur stdio (en-têtes `Content-Length`), `initialize`,
     `didOpen` / `didChange` / `didSave` / `didClose`, `publishDiagnostics`, `hover`,
     `completion`, `definition`, `references`, `rename`, `formatting`, arrêt propre ;
   - registre des serveurs et **détection** de ceux installés (sans rien télécharger) :
     rust-analyzer, clangd, pyright ou pylsp, typescript-language-server, serveurs
     HTML/CSS/JSON de VS Code, csharp-ls ou OmniSharp, serveurs SQL ; et des SDK
     (Python, Node, gcc/clang/cl, dotnet, cargo) ; chaque langage indique clairement
     « coloration seulement » quand aucun serveur n'est trouvé ;
   - API sans GPUI (fils + canaux), documentée dans `studio/lsp/README.md`, testée avec
     un faux serveur LSP ;
   - NEXUL n'en fait pas partie tant que `nxc lsp` n'existe pas (Claude Studio garde
     `nxc ide`).
3. **Installation et distribution** :
   - l'installateur empaquette la **nouvelle application native**
     (`studio/app`, construite avec la chaîne MSVC) sous le nom `NEXUL Studio.exe`
     (raccourcis, association `.nx`, désinstallation, gestion des anciennes versions déjà
     en place : à conserver et à re-tester) ; l'ancienne interface peut rester livrée
     sous un autre nom pendant la transition ;
   - le lancement de Studio ne doit plus dépendre d'Edge ; étudier une interface native
     pour l'installateur lui-même (il affiche aujourd'hui ses pages dans Edge) ;
   - `installer/` est membre du workspace racine (fichiers de Codex) : **n'y ajoutez pas
     de dépendance** sans demande à Codex ;
   - étude écrite de la distribution Linux et macOS, et d'un mécanisme de mise à jour.

### Agent Exécution — terminal, débogage et projets

**Fichiers** (nouveaux dossiers, rien d'autre) :
- `studio/terminal/**` : nouvelle crate **sans dépendance à GPUI** ;
- `studio/dap/**` : nouvelle crate **sans dépendance à GPUI** ;
- `studio/templates/**` : modèles de projets et leur crate d'instanciation ;
- `docs/refonte/execution-status.md`.

**Tâches** :
1. **`studio/terminal`** — terminal intégré :
   - pseudo-console **ConPTY** sous Windows (pty sous Linux/macOS) : lancer un shell
     (PowerShell par défaut, configurable : cmd, Git Bash, WSL), écrire l'entrée,
     redimensionner, arrêter proprement ;
   - émulation VT/ANSI vers un **modèle d'écran** (grille de cellules avec caractère,
     couleurs, gras/italique/souligné, curseur, historique défilant, sélection), en
     réutilisant une bibliothèque mûre (par exemple `alacritty_terminal` ou `vte`) plutôt
     qu'un émulateur écrit de zéro ;
   - API sans GPUI : `Terminal::spawn(shell, cwd, cols, rows)`, `write`, `resize`,
     `snapshot()`, événements (sortie, titre, fin) ; plusieurs terminaux à la fois ;
   - tests : `echo`, couleurs ANSI, redimensionnement, Unicode (accents, CJK, emoji),
     programme interactif, fin du processus.
2. **`studio/dap`** — client Debug Adapter Protocol :
   - transport stdio, `initialize`, `launch` / `attach`, `setBreakpoints` (avec
     conditions), `configurationDone`, `threads`, `stackTrace`, `scopes`, `variables`,
     `continue`, `next`, `stepIn`, `stepOut`, `pause`, `evaluate`, `disconnect`,
     événements (`stopped`, `output`, `terminated`…) ;
   - registre des adaptateurs et **détection** (sans téléchargement) : `lldb-dap`
     (NEXUL, C, C++, Rust ; livré avec NEXUL dans `llvm\bin`), `debugpy` (Python),
     `netcoredbg` (C#), débogueur de Node ;
   - pour NEXUL : `nxc build` en mode debug produit l'exécutable et sa PDB ; s'inspirer de
     `studio/src/debug.rs` et de la logique DAP de `studio/ui/ide.js` (lecture seule) ;
   - API sans GPUI, testée avec `lldb-dap` sur un petit programme NEXUL (point d'arrêt,
     variables, pas à pas).
3. **`studio/templates`** — modèles de projets **fonctionnels** :
   - NEXUL console, bibliothèque NEXUL, projet `nxpm` avec dépendance, site
     HTML/CSS/JavaScript ; Python, C, C++, Rust, C# quand l'outil correspondant est
     détecté ; NEXUL + WebAssembly + JavaScript **seulement** quand le contrat de Codex
     sera validé ;
   - chaque modèle : fichiers, commandes de construction/exécution, outils requis ; un
     test vérifie que chaque modèle disponible se construit et s'exécute ;
   - la détection des SDK vient de `studio/lsp` (Claude Outils) : utilisez son API, ne la
     dupliquez pas.

### Interfaces entre les agents

| Interface | Fournisseur | Utilisateur |
|---|---|---|
| CLI `nxc` (`check --json`, `ide`, `fmt`, `explain`, `run`, `test`, `build`, `targets`), `nxpm list --json` | Codex | Studio |
| `docs/refonte/interop-web.md` (WebAssembly) | Codex | Studio |
| `nexul_studio` (`studio/src`) : `workspace`, `watch`, `runner`, `compiler`, `debug` | Claude Studio | Claude Outils (lecture seule) |
| `studio/syntax`, `studio/lsp` (API Rust) | Claude Outils | Claude Studio (adaptateurs GPUI dans `studio/app`) ; Agent Exécution (détection des SDK) |
| `studio/terminal`, `studio/dap`, `studio/templates` (API Rust) | Agent Exécution | Claude Studio (vues GPUI dans `studio/app`) |
| `studio/app/target/release/NEXUL-Studio-Native.exe` | Claude Studio | Claude Outils (installateur) |

Quand une crate de Claude Outils ou de l'Agent Exécution est prête, son auteur le
signale dans son fichier d'état ; **Claude Studio** ajoute la dépendance dans
`studio/app/Cargo.toml`. Les crates `studio/syntax`, `studio/lsp`, `studio/terminal`,
`studio/dap` et `studio/templates` ont chacune leur propre espace de travail Cargo
(section `[workspace]` vide dans leur `Cargo.toml`, leur propre `Cargo.lock`) pour ne
toucher ni au workspace racine ni à celui de `studio/app`.

## 4. Environnement de ce PC

- Windows 11. Rust 1.99 (rustup) : `stable-x86_64-pc-windows-gnu` par défaut (workspace
  du compilateur), `stable-x86_64-pc-windows-msvc` pour l'application native.
- Visual Studio Community 2026 avec « Développement Desktop en C++ » et le Windows SDK
  (nécessaires à GPUI : shaders HLSL compilés par `fxc.exe`).
- LLVM 23 dans `C:\Program Files\LLVM` (`LLVM-C.dll`, LLD, `lldb-dap`, `llvm-dlltool`).
- Python 3.14, Node, WSL (Ubuntu), QEMU (`C:\Program Files\qemu`).

Commandes :

```
cargo test --release --workspace                         # compilateur + services de Studio + installateur (racine)
cargo test -p nexul-studio                               # services de Studio
cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1
cargo +stable-x86_64-pc-windows-msvc build --release --manifest-path studio/app/Cargo.toml
python -I tools/build_installer.py "chemin\NEXUL Setup.exe"
```

Pièges connus : les crates d'interface (GPUI, Slint…) **ne se construisent pas** avec
la chaîne `windows-gnu` de rustup (import `raw-dylib`, bibliothèques d'importation
manquantes) : utilisez MSVC pour `studio/app`. La première construction de `studio/app`
prend ~6 min.

## 5. État au 15 octobre 2026

- **main** : NEXUL 0.2.0 publié.
- **Codex** (`refonte/codex-core`, local) : audit initial (`docs/refonte/audit-core.md`),
  vérificateurs numériques corrigés et testés, contrat WebAssembly initial (inventaire,
  **pas encore** un feu vert pour le navigateur).
- **Claude Studio** (`refonte/claude-studio`) : création et renommage de fichiers
  réparés dans l'ancienne interface, glisser-déposer, couper/copier/coller, Corbeille
  rapide, surveillance du dossier, encodages (UTF-8, BOM, UTF-16, Windows-1252),
  services en bibliothèque ; audit et comparatif des boîtes à outils
  (`docs/studio/audit-phase0.md`, `architecture-native.md`) ; **première version de
  l'application native** (`studio/app`) avec 6 tests fonctionnels.
- **Claude Outils** : à démarrer.
- **Agent Exécution** : à démarrer.

## 6. Premiers pas des nouveaux agents

1. Créer son worktree (§1) ; lire ce fichier, la mission, `docs/studio/audit-phase0.md`,
   `docs/studio/architecture-native.md`, `docs/refonte/claude-status.md`,
   `docs/refonte/codex-status.md` (branche de Codex).
2. Créer son fichier d'état : `docs/refonte/claude-outils-status.md` ou
   `docs/refonte/execution-status.md`.
3. Claude Outils : `studio/syntax`, puis `studio/lsp`, puis l'installateur.
   Agent Exécution : `studio/terminal`, puis `studio/dap`, puis `studio/templates`.
   Chaque brique avec ses tests.
4. Une PR par brique.
