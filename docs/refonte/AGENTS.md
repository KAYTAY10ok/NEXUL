# NEXUL — charte des agents (V3)

Charte de coordination de la refonte de NEXUL, **version 3, validée par l'utilisateur le
15 octobre 2026**. Elle applique la passation [passation-v3.md](passation-v3.md) (cahier
des charges complet : langage, Studio, auto-hébergement, extension VS Code) et remplace
les versions précédentes de ce fichier. Lisez-la **en entier avant tout code**, puis
[master-plan.md](master-plan.md) (lots, propriétaires, statuts) et les fichiers d'état.

Ce fichier est tenu par **Claude Main**. Pour le modifier : proposition dans votre fichier
d'état ou issue GitHub ; Claude Main l'intègre, l'utilisateur tranche les changements de
cap.

> **Équipe réduite — 29/10/2026 (décision de l'utilisateur).** Seuls **Claude Main** et
> **Codex Terra 6.1** travaillent sur NEXUL ; Codex Terra reprend le périmètre de Codex Sol
> 6.1 (parti) et plus de la moitié du travail restant jusqu'à la 1.0. Cet encadré prime sur
> les tableaux des §2 et §3 :
>
> - **Codex Terra 6.1** : performance (`benchmarks/**` et, sur mandat, l'allocateur et les
>   collections de `lib/std`), `std.net` complet, compilation incrémentale (ADR 0016),
>   auto-hébergement SH2–SH3 (`selfhost/**`), outils et distribution (`editors/vscode/**`,
>   `installer/**`, crates de Studio sans GPUI). Chemins exacts, mandats et règles
>   anti-doublon : [prompt-codex-terra.md](prompt-codex-terra.md). Branches
>   `refonte/terra-…`, PR `[terra]`, état [codex-terra-status.md](codex-terra-status.md).
> - **Claude Main** possède tout le reste, dont la conception du langage (syntaxe, ADR,
>   spécification), le cœur sémantique du compilateur et de la sûreté, le runtime, le temps
>   réel (0.7.0), `@repr("C")`, Studio natif et les fichiers transversaux. Il relit et
>   intègre chaque PR ; l'utilisateur valide les ruptures de compatibilité.
> - Avant chaque tâche, chacun inscrit la tâche et ses fichiers dans la section « En cours »
>   de son fichier d'état et lit celle de l'autre.
> - Les fichiers d'état de Sol, Outils, Luna, Luna 2 et Gemma restent en archive.
> - Tâches : [taches.md](taches.md).
## 1. Le projet

**NEXUL** est un langage compilé, sûr et autonome :

```
code NEXUL → analyse NEXUL → NIR → IR LLVM → code machine (LLD) ou WebAssembly
```

La refonte vise une **version majeure complète** : syntaxe propre et reconnaissable,
typage fort renforcé, orientation objet complète, concurrence sûre, profil temps réel
vérifié sur une cible définie, performances et précision mesurées, **compilateur
auto-hébergé** (stage0 → stage1 → stage2), **NEXUL Studio natif** professionnel et
**extension officielle VS Code**. Détails et critères de fin : passation, §0 à §15.

- `compiler/` : `nxc` (Rust, sans dépendance), `nxpm`, services d'éditeur (`nxc ide`).
- `lib/std/` : bibliothèque standard et runtime **écrits en NEXUL** ; `lib/web/` : chargeur
  JavaScript du WebAssembly navigateur.
- `studio/` : NEXUL Studio — `studio/app` (application native GPUI), `studio/src`
  (services partagés `nexul_studio` et ancienne interface web), `studio/ui` (ancienne
  interface), crates sans GPUI : `syntax`, `lsp`, `terminal`, `dap`, `templates`.
- `installer/` : programme d'installation Windows. `editors/vscode/` : extension VS Code
  (à créer). `docs/` : spécification, guide, feuille de route, ADR, Studio, refonte.

**GitHub** : https://github.com/KAYTAY10ok/NEXUL (public). On ne pousse jamais sur `main` :
tout passe par une PR. Référence de la passation : `main` à `548b1f6` (PR #1, #3 à #8
fusionnées) ; depuis, PR #11 (installateur), #12 (modèle NEXUL + Web) et #13 (cette charte)
fusionnées.

## 2. Gouvernance : Claude Main dirige cinq agents

| Agent | Rôle | Autorité |
|---|---|---|
| **Claude Opus 5.5 — Main** (ancien « Claude Studio ») | Chef de projet technique, architecte en chef, intégrateur principal, responsable de NEXUL Studio, codécideur du langage | Ordre des travaux, arbitrages techniques, **revue obligatoire et veto sécurité/compatibilité** sur tout changement de langage, intégration des PR, tests transversaux. Seul auteur de l'interface GPUI. |
| **Claude Opus 5.5 — Outils / Core** | Ingénieur principal du compilateur et de la sûreté | Seul auteur régulier du compilateur, du runtime, de la spécification et des tests du langage. Conçoit avec Main syntaxe, typage, OOP, concurrence, temps réel, auto-hébergement ; implémente par étapes revues. |
| **Codex Luna** | Composants Studio sans GPUI, intégrations, extension VS Code, distribution | Modèles, `lsp`/`syntax` (maintenance), extension VS Code, installateur. Fournit des API et des preuves à Main. Aucun accès au langage critique. |
| **Codex Luna 2** (ancien Codex Astral, depuis le 15/10) | Exécution et débogage dans Studio, mesures | Terminal (PTY, ConPTY) et débogueur (DAP, `lldb-dap`) sans GPUI ; mesures de performances sur mandat. Travaille sur le PC de Main (MSVC, LLVM, `lldb-dap` réels). Fournit des API et des preuves à Main. Aucun accès au langage critique. |
| **Gemma 4:31B (Claude Code)** | QA, documentation, corpus, tâches non critiques | Checklists, guides, matrices, exemples proposés, tests de fumée isolés. Pas de fusion, pas de code du compilateur ni de sûreté, pas de spécification normative sans revue d'un Opus. |

**Claude Main est le point d'entrée unique** : livrables, rapports, risques et demandes
lui sont adressés. Il propose les arbitrages ; **l'utilisateur décide** des changements de
cap majeurs et des fusions.

Changement de langage (syntaxe, sémantique, sûreté, mémoire, concurrence, temps réel) :
**Outils propose et implémente, Main approuve** (deux revues), plus validation de
l'utilisateur pour toute rupture de compatibilité ou nouveau mot-clé fondamental.

## 3. Propriété des fichiers — un seul propriétaire par chemin

| Propriétaire | Chemins |
|---|---|
| **Claude Main** | `studio/app/**`, `studio/src/**`, `studio/ui/**`, `studio/plugins/**`, `studio/assets/**`, `studio/Cargo.toml`, `studio/README.md`, `docs/studio/**`, `docs/refonte/claude-main-status.md`, `docs/refonte/master-plan.md`, `docs/refonte/taches.md`, `docs/refonte/AGENTS.md`, `docs/refonte/passation-v3.md` ; **fichiers transversaux** : `Cargo.toml` et `Cargo.lock` racine, `README.md` racine, `CHANGELOG.md`, `.github/**` |
| **Claude Outils / Core** | `compiler/**`, `lib/**`, `tests/run/**`, `tests/ui/**`, `tests/numerics/**`, `tests/web/**`, `examples/**`, `docs/spec/**`, `docs/guide.md`, `docs/roadmap.md`, `docs/adr/**` (langage), `tools/wasi-run.mjs`, `docs/refonte/interop-web.md`, `docs/refonte/audit-core.md`, `docs/refonte/claude-core-status.md`, et les sources du compilateur auto-hébergé (`selfhost/**` ou `compiler/selfhost/**`, choix par ADR) |
| **Codex Luna 2** | `studio/terminal/**`, `studio/dap/**`, `benchmarks/**` (sur mandat), `docs/refonte/codex-luna2-status.md` ; `docs/refonte/codex-status.md` (archive d'Astral) ; lecture seule ailleurs |
| **Codex Luna** | `studio/templates/**`, `studio/lsp/**`, `studio/syntax/**`, `editors/vscode/**`, `installer/**`, `tools/build_installer.py`, `tools/make-python-runtime.py`, `tools/licenses/**`, `docs/installation.md`, `docs/refonte/codex-luna-status.md` |
| **Gemma** | `docs/refonte/gemma/**`, `docs/refonte/gemma-status.md`, `tests/manual/**` |

Les fichiers d'état historiques (`claude-outils-status.md`, `codex-exec-status.md`)
restent à leur auteur comme archives ; chacun ouvre son nouveau fichier d'état V3.

Pour toucher un chemin qui n'est pas à vous : demande au propriétaire (fichier d'état +
issue GitHub), jamais de modification surprise. Gemma rédige des propositions que le
propriétaire applique.

### 3.1 Transferts consignés (V3, 15/10/2026)

| Chemins | Ancien propriétaire | Nouveau | Effet |
|---|---|---|---|
| `compiler/**`, `lib/**`, `tests/**` (langage), `examples/**`, `docs/spec/**`, `docs/guide.md`, `docs/roadmap.md`, `docs/adr/**`, `tools/wasi-run.mjs`, `interop-web.md`, `audit-core.md` | Codex Astral | Claude Outils / Core | **Immédiat** : la branche `refonte/codex-core` est entièrement fusionnée (PR #1). Astral n'y écrit plus sans mandat. |
| `Cargo.toml`, `Cargo.lock` racine, `README.md`, `CHANGELOG.md`, `.github/**` | Codex Astral | Claude Main | **Immédiat** : intégration par Main, après les PR des propriétaires concernés. |
| `studio/lsp/**`, `studio/syntax/**` | Claude Outils | Codex Luna | **Immédiat** : PR #6 et #8 fusionnées. Maintenance uniquement ; l'adaptateur GPUI reste à Main. |
| `installer/**`, `tools/build_installer.py`, `tools/make-python-runtime.py`, `tools/licenses/**`, `docs/installation.md` | Claude Outils | Codex Luna | **Effectif** depuis la fusion de la PR #11 (branches `refonte/claude-tools-installer` et `refonte/claude-tools-etudes`). Les études d'Outils (`installer/docs/`) sont reprises par Luna. |
| `editors/vscode/**` | — | Codex Luna | Nouveau dossier. |
| `studio/terminal/**`, `studio/dap/**` | Codex Luna | Codex Luna 2 | **15/10** : Codex Astral devient Codex Luna 2 ; Luna garde l'extension VS Code, l'installateur, les modèles, `lsp` et `syntax`. |
| `docs/refonte/reviews/astral/**`, `benchmarks/astral/**` | Codex Astral | Codex Luna 2 (`benchmarks/**`) | Même date. |
| `docs/refonte/gemma/**`, `gemma-status.md`, `tests/manual/**` | — | Gemma | Nouveaux chemins. |

## 4. Règles communes

1. Chacun travaille dans **son clone ou worktree**, sur **une branche par lot**
   (`refonte/main-…`, `refonte/core-…`, `refonte/luna2-…`, `refonte/luna-…`,
   `refonte/gemma-…` ; les branches existantes terminent leur lot sous leur nom actuel).
2. **Jamais** de push sur `main`, de `git reset --hard`, de push forcé, de suppression du
   travail d'un autre. La pile `git stash` est partagée entre worktrees : ne l'utilisez
   pas (commit temporaire à la place).
3. Livraison par **PR courte** vers `main`, titre préfixé `[main]`, `[core]`, `[luna2]`,
   `[luna]` ou `[gemma]`, avec : changements, fichiers, **tests réellement exécutés** et
   résultats, limites, impact sur l'existant, preuves, demande de revue. Claude Main revoit
   et intègre ; l'utilisateur fusionne.
4. Chacun tient **son fichier d'état** et le met à jour après chaque PR ; Claude Main tient
   [master-plan.md](master-plan.md).
5. Une demande à un autre agent = une entrée « Demandes » dans votre fichier d'état **et**
   une issue GitHub (besoin, comportement attendu, critères de validation).
6. **Aucun résultat inventé** : un outil absent ou une preuve manquante s'écrit
   « non testé ». Statuts obligatoires (passation §14) : `CONÇU`, `IMPLÉMENTÉ`,
   `TESTÉ_UNITAIREMENT`, `INTÉGRÉ`, `VALIDÉ_E2E`, `LIVRÉ`, `BLOQUÉ`. Jamais « terminé »
   sur la foi d'une capture, d'un extrait ou d'un test ignoré.
7. Aucune syntaxe nouvelle n'est présentée comme officielle avant l'ADR approuvée (passation
   §3.2) ; les esquisses sont marquées **illustratives**.
8. Ne touchez pas aux réglages réels de l'utilisateur dans les tests (`NEXUL_STUDIO_CONFIG`
   pour l'application native).
9. Chaque crate sous `studio/` (hors `studio/app` et `studio/src`) a son propre espace de
   travail Cargo (`[workspace]` vide, son `Cargo.lock`) ; `studio/app` aussi. Le
   `cargo test --workspace` racine ne les teste pas.

## 5. Interfaces entre les agents

| Interface | Fournisseur | Utilisateurs |
|---|---|---|
| CLI `nxc` (`check --json`, `ide … --stdin`, `fmt`, `explain`, `run`, `test`, `build`, `targets`), `nxpm list --json` ; futur `nxc lsp`, JSON versionné | Outils / Core | Main (Studio), Luna (VS Code, modèles) |
| `docs/refonte/interop-web.md` (ABI web v1, `lib/web/nexul.mjs`) | Outils / Core | Main (aperçu), Luna (modèle NEXUL + Web) |
| `nexul_studio` (`studio/src`) : `workspace`, `watch`, `runner`, `compiler`, `debug` | Main | Luna (lecture seule) |
| `studio/syntax`, `studio/lsp`, `studio/templates` (API Rust sans GPUI) | Luna | Main (vues et adaptateurs GPUI dans `studio/app`) |
| `studio/terminal`, `studio/dap` (API Rust sans GPUI) | Luna 2 | Main (vues et adaptateurs GPUI dans `studio/app`) |
| `NEXUL-Studio-Native.exe` (construit avec MSVC) | Main | Luna (installateur) |
| Contrat `nxc lsp` (capacités, version, texte non enregistré) | Outils propose | **Main approuve** ; Studio et VS Code consomment |

## 6. Environnements

- **PC de Main** (Windows 11) : `C:\NEXUL` (ne pas y travailler), worktrees
  `C:\NEXUL\.claude\worktrees\claude-studio` (Main) et `C:\NEXUL-core` (Luna 2, ex-Astral). Rust 1.99 :
  `stable-x86_64-pc-windows-gnu` par défaut (compilateur), `stable-x86_64-pc-windows-msvc`
  pour `studio/app` (Visual Studio 2026 « Développement Desktop en C++ » + Windows SDK).
  LLVM 23 dans `C:\Program Files\LLVM` (`LLVM-C.dll`, LLD, `lldb-dap`), Python 3.14, Node,
  WSL, QEMU.
- **Autre PC** (Outils, Luna) : clones GitHub (`C:\NEXUL-tools`…) ; sans outils C++ de
  Visual Studio ni LLVM à ce jour (voir leurs fichiers d'état).
- Commandes (passation §15.1) :

```
cargo test --release --workspace
cargo test -p nexul-studio
cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1
cd studio/app; cargo +stable-x86_64-pc-windows-msvc build --release   # depuis studio/app : CRT statique (.cargo/config.toml)
cargo test --manifest-path studio/syntax/Cargo.toml        # idem lsp, terminal, dap, templates
```

Pièges : GPUI ne se construit pas avec `windows-gnu` (MSVC obligatoire pour `studio/app`) ;
sans la fonction `tree-sitter` de gpui-component, les diagnostics de l'éditeur n'ont pas de
couleur.

## 7. Premiers pas de chaque agent (V3)

1. Récupérer `main`, lire la passation, cette charte, [master-plan.md](master-plan.md) et les
   fichiers d'état ; confirmer son identité.
2. Ouvrir son nouveau fichier d'état (§3) avec : périmètre, lots en cours, statuts, demandes.
3. Exécuter son **ordre de mission** : [master-plan.md](master-plan.md) §2 (tâches
   numérotées, priorités, critères de fin ; consignes à coller au §2.6). En résumé, au
   15/10 :
   - **Main** (M1 à M16) : intégration de P1, binaire natif autonome, raccordement des
     crates (P3), exécution/tests/paramètres/recherche dans Studio, pilotage de P2 avec
     Outils, revue de toutes les PR Core ;
   - **Outils / Core** (O1 à O10) : audit G0 et trois propositions de syntaxe, contrat puis
     implémentation de `nxc lsp`, formateur, conception typage/OOP/concurrence, étude de la
     cible temps réel, runtime macOS, inventaire de l'auto-hébergement ;
   - **Luna** (L3 à L7) : extension VS Code (suite de la PR #16), installateur du Studio
     natif, modèles ;
   - **Luna 2** (L1, L2, A1) : correction du terminal ConPTY et du débogueur avec le vrai
     `lldb-dap`, puis base de performances NEXUL/C/Rust/C# ;
   - **Gemma** (Q1 à Q6) : matrice des preuves, écarts code/documentation, essais manuels
     du Studio natif, corpus de programmes 0.2.
4. Une PR par lot, revue et intégrée par Main. Le master-plan §3 liste **toutes** les
   améliorations attendues (refonte et création) : proposez-y vos ajouts par votre fichier
   d'état.
