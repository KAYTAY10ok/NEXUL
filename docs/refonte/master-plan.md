# NEXUL — plan maître de la refonte

Tenu par **Claude Main** (voir [AGENTS.md](AGENTS.md)). Source des exigences :
[passation-v3.md](passation-v3.md). Statuts : `CONÇU`, `IMPLÉMENTÉ`, `TESTÉ_UNITAIREMENT`,
`INTÉGRÉ`, `VALIDÉ_E2E`, `LIVRÉ`, `BLOQUÉ`, ou `À FAIRE`. Un statut n'avance qu'avec une
preuve (commande, test, PR). Un résultat déclaré par un agent et non revérifié par Main est
marqué « déclaré ».

Dernière mise à jour : 15 octobre 2026, `main` à `edb886c` (PR #11, #12, #13 fusionnées).

Sommaire : §1 lots · §2 **ordres de mission par agent** · §3 **toutes les améliorations
(refonte et création)** · §4 composants existants · §5 vérifications de Main · §6 décisions
de l'utilisateur · §7 intégration.

## 1. Lots

| Lot | Urgence | Travail et preuve de sortie | Propriétaire | Dépend de | Statut |
|---|---|---|---|---|---|
| P0 | Immédiat | Gouvernance V3 : charte, transferts, plan maître | Main | accord utilisateur (15/10) | INTÉGRÉ (PR #13) |
| P1 | Immédiat | Studio sans perte de données : Enregistrer / Ne pas enregistrer / Annuler (onglet, fenêtre), conflits disque, enregistrement atomique, récupération, tests | Main | — | TESTÉ_UNITAIREMENT (`43ebbe5`, 25 tests) ; PR à fusionner ; essai manuel à faire (Q3) |
| P2 | Immédiat | Nouvelle syntaxe : G0 audit des usages 0.2, G1 trois propositions sur 12 programmes, G2 ADR + EBNF validées par l'utilisateur, plan de migration | Outils (auteur) + Main (co-conception, approbation) | décision utilisateur | À FAIRE (O1, O2) |
| P3 | Élevée | Raccorder `syntax`, `lsp`, `terminal`, `dap`, `templates` dans Studio ; UI native stable | Main | crates fusionnées ; L1, L2 pour terminal et DAP | À FAIRE (M3 à M7) |
| P4 | Élevée | Installateur Windows du Studio natif testé avec un vrai binaire | Luna | transfert (effectif : PR #11 fusionnée), M2 | À FAIRE (L3) |
| P5 | Élevée | Conception typage renforcé, OOP, modèle mémoire et concurrence ; tests d'acceptation | Outils, revue Main | P2 | À FAIRE (O5) |
| P6 | Élevée | Nouveau lexer/parser, types, formatage, diagnostics ; migration des sources 0.2 | Outils | ADR P2 | À FAIRE |
| P7 | Élevée | Concurrence sûre (compilateur + runtime), canaux, tests négatifs et de stress | Outils | P5, P6 | À FAIRE |
| P8 | Élevée | Profil temps réel v1 sur une cible définie, budgets et mesures | Outils | P7, cible validée (décision 3) | À FAIRE (étude O6) |
| P9 | Élevée | `nxc lsp`, JSON versionné, vérification du texte non enregistré | Outils | contrat approuvé par Main | À FAIRE (O3) |
| P10 | Élevée | Extension VS Code : VSIX installable, commandes `nxc`, LSP, format, tests | Luna, revue Main | P2 (coloration finale), P9 (LSP) | À FAIRE (ossature L4) |
| P11 | Élevée | Web dans Studio : modèle NEXUL + Web, aperçu isolé `wasm32-web` | Luna (modèle), Main (UI) | ABI v1 | modèle TESTÉ_UNITAIREMENT (PR #12, revérifié §5) ; aperçu À FAIRE (M11) |
| P12 | Majeur | Auto-hébergement stage0 → stage1 → stage2, parité, reproductibilité | Outils, revue Main | noyau stable | À FAIRE (inventaire O8) |
| P13 | Majeur | Benchmarks NEXUL/C/Rust/C#, précision, optimisations, multi-cibles | Outils ; Astral sur mandat | base figée | À FAIRE (mandat A1) |
| P14 | Bloquant pour « fini » | Scénarios E2E A à H (passation §15.2), documentation exacte, version stable, migration, retour arrière | Main + chaque propriétaire | tous les lots | À FAIRE |

Séquence : la suite de tests reste verte à chaque lot ; la syntaxe est arrêtée (ADR) avant
la grammaire finale de VS Code et de `studio/syntax` ; les garanties temps réel sont
spécifiées avant les primitives ; le self-hosting vient après un noyau stable.

## 2. Ordres de mission (réattribution du 15/10)

Chaque tâche : une branche, une PR courte vers `main` (préfixe `[main]`, `[core]`,
`[luna]`, `[astral]`, `[gemma]`), tests réellement exécutés dans la description, fichier
d'état mis à jour. Claude Main revoit et intègre. Priorité : 1 = à commencer maintenant,
2 = ensuite, 3 = quand les dépendances sont prêtes.

### 2.1 Claude Main

| Tâche | Prio | Travail | Critère de fin | Statut |
|---|---|---|---|---|
| M1 | 1 | Intégrer P1 (PR `refonte/claude-studio`) | PR fusionnée ; 25 tests verts | TESTÉ_UNITAIREMENT |
| M2 | 1 | Binaire natif sans dépendance au runtime VC++ (issue #9 d'Outils) | `dumpbin /dependents` sans `vcruntime140.dll` ; Studio démarre sur un Windows neuf | INTÉGRÉ (`eb490ce`) : CRT statique (`studio/app/.cargo/config.toml`), `dumpbin` sans `vcruntime140.dll` ni `api-ms-win-crt-*`, démarrage vérifié ; Windows neuf non testé |
| M3 | 1 | P3 — `studio/syntax` remplace la coloration provisoire (`highlight.rs`) | couleurs identiques ou meilleures, diagnostics toujours visibles, tests | INTÉGRÉ (`eb490ce`) : coloration incrémentale, test d'éditions successives |
| M4 | 1 | P3 — assistant Nouveau projet (`studio/templates`) + `ToolchainProvider` (catalogue `studio/lsp`, `nxc targets`) | créer, ouvrir, construire un projet NEXUL et NEXUL + Web depuis Studio | INTÉGRÉ (`0066da3`) : création et ouverture testées ; construction depuis Studio avec M8 |
| M5 | 2 | P3 — `studio/lsp` pour les autres langages (serveurs détectés) ; NEXUL par `nxc ide` jusqu'à `nxc lsp` | diagnostics/survol/complétion d'un serveur réel (ex. rust-analyzer) ; absence expliquée | À FAIRE |
| M6 | 3 | P3 — vue terminal (PTY, clavier, défilement, copie, plusieurs terminaux) | dépend de L1 ; test avec un vrai shell | À FAIRE |
| M7 | 3 | P3 — interface de débogage (points d'arrêt, pas à pas, variables, pile) | dépend de L2 ; session réelle `lldb-dap` sur un programme NEXUL | À FAIRE |
| M8 | 2 | Exécuter / construire / tester depuis Studio (`nxc run/build/test`, profils, explorateur de tests `nxc ide tests`) | sorties et erreurs cliquables, arrêt, tests | À FAIRE |
| M9 | 2 | Paramètres (fenêtre) et raccourcis personnalisables ; enregistrement automatique réglable | réglages persistés, tests | À FAIRE |
| M10 | 2 | Recherche et remplacement dans les fichiers | résultats cliquables, gros dossiers, tests | INTÉGRÉ (`030d617`) : Ctrl+Maj+F, texte non enregistré, casse, mot entier, Tout remplacer ; gros dossiers non mesurés |
| M11 | 3 | P11 — aperçu Web isolé (`wasm32-web`, [interop-web.md](interop-web.md)) | modèle NEXUL + Web construit et affiché dans son panneau | À FAIRE |
| M12 | 3 | Retrait de l'ancienne interface Edge après parité, migration des préférences | liste de parité cochée et testée | À FAIRE |
| M13 | 2 | CI GitHub (`.github/**`) : compilateur (GNU), `studio/app` (MSVC), crates séparées | décision 8 ; tous les espaces de travail testés | CONÇU |
| M14 | 1 | P2 — co-concevoir les propositions de syntaxe avec Outils, les comparer, présenter le choix à l'utilisateur | ADR co-signée, validée par l'utilisateur | À FAIRE |
| M15 | 1 | Approuver le contrat `nxc lsp` (O3) ; revoir toutes les PR Core (veto sécurité/compatibilité) | revue écrite dans chaque PR | continu |
| M16 | 3 | P14 — scénarios A à H, notes de version, `CHANGELOG.md`, `README.md` | scénarios exécutés et consignés | À FAIRE |

### 2.2 Claude Outils / Core

| Tâche | Prio | Travail | Livrable et critère de fin | Statut |
|---|---|---|---|---|
| O1 | 1 | P2-G0 — audit des usages de NEXUL 0.2 (runtime, `lib/std`, tests, exemples, guide, Studio) : formes à garder, faiblesses, ambiguïtés, impact outillage | `docs/adr/syntaxe-g0-audit.md`, relu par Main | À FAIRE |
| O2 | 1 | P2-G1 — trois propositions crédibles, chacune sur les 12 programmes de la passation §3.2 (illustratives), lisibilité, nombre de termes, complexité du parser | `docs/adr/syntaxe-g1-propositions.md` ; Main co-auteur par revue | À FAIRE (après O1) |
| O3 | 1 | P9 — contrat `nxc lsp` (capacités, version, texte non enregistré) puis implémentation : `initialize`, `didOpen`/`didChange`, `publishDiagnostics`, `hover`, `completion`, `definition`, `references`, `rename`, `documentSymbol`, `formatting` ; champ `version` dans `check --json` et `ide` ; vérification depuis l'entrée standard | contrat approuvé par Main, puis tests stdio réels | À FAIRE |
| O4 | 1 | Formateur : l'accolade fermante reste en fin de ligne (`fn main(){⏎ println("ok")}`) | comportement décidé, test de non-régression | À FAIRE |
| O5 | 2 | P5 — conception : types distincts, sous-types contraints, invariants, contrats, effets, OOP, modèle mémoire et concurrence ; corpus de tests d'acceptation (positifs et négatifs) | ADR relues par Main ; corpus prêt à exécuter | À FAIRE |
| O6 | 2 | P8-G0 — étude comparée de la première cible temps réel (Cortex-M nu, Cortex-M + RTOS, Linux PREEMPT_RT) : contraintes, instrumentation, matériel nécessaire | étude ; Main propose, l'utilisateur choisit (décision 3) | À FAIRE |
| O7 | 2 | Couche système macOS du runtime (défaut relevé dans [audit-core.md](audit-core.md)) | correctif + tests, ou BLOQUÉ documenté (pas de Mac) | À FAIRE |
| O8 | 2 | P12-SH0 — inventaire de ce qui manque à NEXUL pour écrire son compilateur (chaînes UTF-8, collections, fichiers, diagnostics, appels LLVM) | `docs/adr/selfhost-sh0.md` | À FAIRE |
| O9 | continu | WebAssembly, LLVM, précision numérique, cibles : aucune régression ; tests navigateur supplémentaires | suites vertes à chaque PR | continu |
| O10 | 2 | Stratégie d'édition et migration (`nxc migrate` ou `--edition`) | dans l'ADR G2 ; décision 2 | À FAIRE |

Les tâches P6, P7, P8 (implémentation), P12 (SH1 à SH5) et P13 suivent l'ADR de syntaxe et
les ADR de P5 ; elles sont détaillées au §3.

### 2.3 Codex Luna

| Tâche | Prio | Travail | Critère de fin | Statut |
|---|---|---|---|---|
| L1 | 1 | `studio/terminal` sous Windows : les essais ConPTY `pty_resize_and_stop_are_reported` et `pty_runs_interactive_program_and_exits` ne se terminent jamais sur le PC de Main (MSVC) ; la chaîne GNU échoue (`dlltool`) | `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/terminal/Cargo.toml` vert en moins d'une minute ; chaîne requise écrite dans le README ou dépendance GNU corrigée | BLOQUÉ (à corriger) |
| L2 | 1 | `studio/dap` : `lldb_dap_debugs_a_nexul_program_with_breakpoint_variables_and_step` échoue avec `Timeout("launch")` contre `lldb-dap` de LLVM 23 | test réel vert sur Windows avec `nxc` et `lldb-dap` | BLOQUÉ (à corriger) |
| L3 | 1 | P4 — installateur (transfert effectif) : empaqueter `NEXUL-Studio-Native.exe` (construit avec MSVC **depuis `studio/app`** : CRT statique, issue #9 réglée) comme Studio par défaut, raccourcis, association `.nx`, désinstallation, migration depuis l'ancien Studio ; étapes 2 et 3 de l'interface native (études d'Outils) | installation et désinstallation réelles testées sur Windows ; plus de Studio Edge installé par défaut | À FAIRE |
| L4 | 1 | P10 — ossature de l'extension VS Code `editors/vscode/**` : `package.json`, langage `.nx` et `nexul.toml`, réglage du chemin de `nxc`, commandes check/build/run/test/fmt/explain par la CLI, diagnostics de `check --json`, tâches, Workspace Trust, tests hôte, script VSIX ; coloration 0.2 marquée provisoire | VSIX installé dans un VS Code propre, diagnostics et commandes réels ; aucune grammaire finale avant l'ADR | À FAIRE |
| L5 | 2 | `studio/templates` : documenter `ToolchainProvider` pour M4 | API décrite dans le README | suite Rust verte chez Main (§5) |
| L6 | 3 | Maintenance `studio/syntax` et `studio/lsp` ; adaptation à la nouvelle grammaire après l'ADR ; client `nxc lsp` quand il existe | tests de conformité sur le corpus de l'ADR | À FAIRE |
| L7 | 3 | Mises à jour (`nexul-update`, signature) et Linux/macOS selon les décisions sur les études d'installation | selon décision | CONÇU (études) |

### 2.4 Codex Astral — mandat écrit A1

Mandat donné par Claude Main le 15/10/2026, périmètre d'écriture limité à
`benchmarks/astral/**` et `docs/refonte/reviews/astral/**`, branche `refonte/astral-a1`,
lecture seule ailleurs, aucune modification du compilateur.

| Tâche | Travail | Livrable |
|---|---|---|
| A1 | Base de référence P13 sur `main` figé (commit noté) : programmes équivalents NEXUL, C, Rust, C# ; boucles 1M/10M/100M, fonctions mathématiques, vecteurs/matrices, allocations, chaînes ; même machine, modes optimisés documentés, protection contre l'élimination du code mort ; médianes, dispersion, mesures brutes, temps de compilation et mémoire | `benchmarks/astral/` (sources + script), `docs/refonte/reviews/astral/a1-base-performances.md` (matériel, chaînes d'outils, résultats bruts) |
| A2 | Conditionnel, sur demande ultérieure : revue de l'IR LLVM des boucles chaudes et des vérifications redondantes ; revue des ordres mémoire quand la conception O5 existe | note de revue |

### 2.5 Gemma 4:31B

Écriture limitée à `docs/refonte/gemma/**`, `docs/refonte/gemma-status.md`, `tests/manual/**`.

| Tâche | Prio | Travail | Livrable |
|---|---|---|---|
| Q1 | 1 | Matrice des preuves : composant, statut, preuve, propriétaire, limites (passation §12.5) | `docs/refonte/gemma/matrice-preuves.md` |
| Q2 | 1 | Écarts code/documentation : faire passer les exemples de `docs/guide.md` et `docs/spec/**` par `nxc` 0.2, consigner chaque écart (sans corriger) | `docs/refonte/gemma/ecarts-docs.md` → Outils |
| Q3 | 1 | Liste d'essais manuels du Studio natif : P1 (fermeture, conflit, récupération), explorateur, palette, services NEXUL | `tests/manual/studio-natif.md` ; résultats → Main |
| Q4 | 2 | Corpus de programmes 0.2 non critiques avec sortie attendue (pour G0, la migration et le scénario H) | `tests/manual/programmes/` |
| Q5 | 3 | Brouillon du guide utilisateur de l'extension VS Code (après L4) | `docs/refonte/gemma/guide-vscode.md` (proposition) |
| Q6 | continu | Résumés des PR et index des fonctions du langage, vérifiés par les propriétaires | `docs/refonte/gemma/` |

### 2.6 Consignes à transmettre à chaque agent

À coller au début de la session de l'agent :

- **Claude Outils / Core** : « Tu es Claude Opus 5.5 — Outils/Core sur NEXUL. Récupère
  `main`, lis `docs/refonte/AGENTS.md` puis `docs/refonte/master-plan.md` §2.2 et §3.
  Ouvre `docs/refonte/claude-core-status.md`. Commence par O1 (audit G0) et O3 (contrat
  `nxc lsp` à faire approuver par Claude Main), puis O4. Une branche `refonte/core-…` et une
  PR `[core]` par tâche, tests réellement exécutés. Aucune syntaxe officielle avant l'ADR. »
- **Codex Luna** : « Tu es Codex Luna sur NEXUL. Récupère `main`, lis
  `docs/refonte/AGENTS.md` puis `docs/refonte/master-plan.md` §2.3. Ouvre
  `docs/refonte/codex-luna-status.md`. Corrige d'abord L1 (terminal ConPTY) et L2 (DAP
  `Timeout("launch")`), puis L3 (installateur du Studio natif) et L4 (ossature VS Code, sans
  grammaire finale). Une branche `refonte/luna-…` et une PR `[luna]` par tâche. »
- **Codex Astral** : « Tu es Codex Astral sur NEXUL. Mandat écrit A1 dans
  `docs/refonte/master-plan.md` §2.4 : base de performances, écriture seulement dans
  `benchmarks/astral/**` et `docs/refonte/reviews/astral/**`, branche `refonte/astral-a1`,
  aucun changement du compilateur, aucun résultat inventé. »
- **Gemma** : « Tu es Gemma (QA et documentation) sur NEXUL. Lis `docs/refonte/AGENTS.md`
  et `docs/refonte/master-plan.md` §2.5. Ouvre `docs/refonte/gemma-status.md`. Fais Q1, Q2,
  Q3. Écriture seulement dans `docs/refonte/gemma/**`, `docs/refonte/gemma-status.md`,
  `tests/manual/**` ; branche `refonte/gemma-…`, PR `[gemma]`. »

## 3. Toutes les améliorations (refonte et création)

Liste de référence de ce qui doit exister à la fin (passation §0 à §15). Chaque ligne
devient une tâche quand son lot démarre ; une ligne n'est cochée qu'avec sa preuve.

### 3.1 Syntaxe (P2, P6) — Outils, approbation Main, validation utilisateur

| ID | Amélioration | Statut |
|---|---|---|
| S1 | G0 : audit des formes 0.2 (runtime, std, tests, exemples, guide, Studio), formes à garder, ambiguïtés, impact sur débogueur, attributs et outils | À FAIRE (O1) |
| S2 | G1 : trois propositions sur les 12 mêmes programmes (Bonjour, calcul, structures et méthodes, interface polymorphe, génériques, erreurs, fichiers, ownership, threads et canaux, tâche périodique RT, web WASM, tests) | À FAIRE (O2) |
| S3 | G2 : ADR signée des deux Opus et validée par l'utilisateur ; lexique, EBNF, précédence, nommage, commentaires, style standard, ambiguïtés ; exemples devenus tests | À FAIRE |
| S4 | Style rédactionnel NEXUL : modules, fonctions, expressions, déclarations, conditions, boucles, propriétés, méthodes, traits, erreurs, ressources, concurrence, temps réel | À FAIRE |
| S5 | G3 : chaîne complète lexer → parser → AST/THIR → typage/NIR → LLVM → diagnostics → formateur → Studio → VS Code ; aucune grammaire acceptée non compilable | À FAIRE |
| S6 | Effets visibles dans la syntaxe : allocations, blocages, exceptions identifiables par le compilateur | À FAIRE |
| S7 | G4 : transition `nxc migrate` ou `--edition`, projets 0.2 préservés, rejets expliqués, aucune suppression silencieuse | À FAIRE (O10) |
| S8 | G5 : parser robuste (fichiers malformés, préfixes incomplets, erreurs non en cascade), formateur déterministe, fuzzing lexer/parser, corpus de régression | À FAIRE |

### 3.2 Typage fort (P5, P6) — Outils, revue Main

| ID | Amélioration | Statut |
|---|---|---|
| T1 | Préserver l'existant 0.2 : types statiques, inférence, conversions explicites, `Option`/`Result`, génériques, traits, exclusivité, ownership, RAII | LIVRÉ (0.2) |
| T2 | Types distincts (newtypes) : identifiants, unités (secondes/millisecondes, degrés/radians) non mélangeables | À FAIRE |
| T3 | Sous-types contraints : plages, prédicats simples, constantes vérifiées à la compilation, contrôles à l'exécution sinon | À FAIRE |
| T4 | Invariants de structures et d'objets préservés à la construction et à la mutation | À FAIRE |
| T5 | Contrats : préconditions, postconditions, invariants de boucle ; statique / dynamique / profil défini (décision 4) | À FAIRE |
| T6 | Effets : allocation, blocage, I/O, panique, accès matériel, interruption, partage ; `no_alloc` et `nonblocking` réellement vérifiés | À FAIRE |
| T7 | Typage des ressources : transfert, exclusivité, portée, destruction déterministe, aucun alias mutable non contrôlé hors `unsafe` | À FAIRE |
| T8 | Capacité de transfert/partage entre threads déterminée statiquement | À FAIRE |
| T9 | Numérique : conversions vérifiées, débordements définis, flottants documentés, unités si retenues | À FAIRE |
| T10 | Exhaustivité : filtrage total, enums typés, erreurs non ignorées silencieusement (état 0.2 à vérifier en G0) | À FAIRE |
| T11 | Diagnostics pédagogiques : source, raisonnement, correction proposée, position exacte dans Studio et VS Code | À FAIRE |
| T12 | Aucune protection essentielle retirée en mode release ; programmes 0.2 migrés équivalents | À FAIRE |

### 3.3 Orientation objet (P5, P6) — Outils, revue Main

| ID | Amélioration | Statut |
|---|---|---|
| OO1 | État privé, visibilité module/type, méthodes d'instance et de type | À FAIRE |
| OO2 | Interfaces/traits, polymorphisme statique et dynamique (existant 0.2 : traits, `dyn`, à reprendre dans la nouvelle syntaxe) | À FAIRE |
| OO3 | Composition, délégation, extensions, génériques ; dispatch compréhensible | À FAIRE |
| OO4 | Constructeurs/usines avec validation, cycle de vie et destruction sûrs | À FAIRE |
| OO5 | Collections hétérogènes par interface sans fuite ni double destruction | À FAIRE |
| OO6 | Réflexion limitée et explicite seulement si nécessaire ; pas de runtime dynamique caché | À FAIRE |
| OO7 | Frontières temps réel : allocation implicite, dispatch imprévisible, destructeurs complexes contrôlés ou refusés en profil RT | À FAIRE |
| OO8 | Programmes d'acceptation en NEXUL : bibliothèque graphique typée, moteur de simulation, système d'événements, pilotes, serveur, hiérarchie de services | À FAIRE |

### 3.4 Concurrence et modèle mémoire (P5, P7) — Outils, revue Main

| ID | Amélioration | Statut |
|---|---|---|
| C1 | Modèle mémoire formalisé avant les primitives | À FAIRE |
| C2 | Threads et/ou tâches : lancement, jonction, arrêt, propagation des erreurs | À FAIRE |
| C3 | Transfert sûr entre threads, règles de partage | À FAIRE |
| C4 | Canaux bornés et non bornés (hors RT), fermeture, annulation | À FAIRE |
| C5 | Mutex / RW locks, variables de condition, événements, sémaphores, politique d'empoisonnement | À FAIRE |
| C6 | Atomiques, ordres mémoire, barrières, cibles prises en charge | À FAIRE |
| C7 | Exclusion statique des courses de données dans le code sûr, `unsafe` confiné | À FAIRE |
| C8 | Annulation, ressources abandonnées, destruction déterministe documentées | À FAIRE |
| C9 | Capacités par cible (natif Windows/Linux/macOS, embarqué, Wasm) documentées (décision 5) | À FAIRE |
| C10 | Tests négatifs de compilation, tests de stress, instrumentation ; limites (interblocage, famine) écrites | À FAIRE |

### 3.5 Temps réel (P8) — Outils, revue Main, cible validée par l'utilisateur

| ID | Amélioration | Statut |
|---|---|---|
| R1 | G0 : étude et choix de la première cible (décision 3) | À FAIRE (O6) |
| R2 | Profil RT : allocation interdite ou bornée après initialisation, pools, budgets d'objets | À FAIRE |
| R3 | Boucles, récursions et attentes bornées là où une preuve est exigée ; annotations vérifiées | À FAIRE |
| R4 | Classement des fonctions : ISR-safe, non bloquant, sans allocation, déterministe, matériel, I/O | À FAIRE |
| R5 | Tâches périodiques et événementielles, priorités, échéances, horloges monotones, alarmes, files bornées, chien de garde | À FAIRE |
| R6 | Contrat langage / runtime / OS ou bare metal ; protection contre l'inversion de priorité | À FAIRE |
| R7 | `volatile`, atomiques, barrières pour registres et interruptions, sections critiques | À FAIRE |
| R8 | Budgets pile, tas, tâches, temps de réaction ; comportement en surcharge | À FAIRE |
| R9 | Instrumentation : gigue, latence, échéances manquées, maximum observé (≠ WCET), pile maximale | À FAIRE |
| R10 | Applications témoins (contrôle périodique, capteur/actionneur, communication, ISR) et rapport prouvé / mesuré / dépendant de la plateforme | À FAIRE |

### 3.6 Performances et précision (P13) — Outils ; Astral sur mandat

| ID | Amélioration | Statut |
|---|---|---|
| PF1 | Références NEXUL/C/Rust/C# sur la même machine (1M/10M/100M, maths, matrices, allocations, chaînes, tâches) | À FAIRE (A1) |
| PF2 | Mesures : compilation, exécution, mémoire, dispersion, données brutes ; pas d'élimination du travail mesuré | À FAIRE (A1) |
| PF3 | Flottants : NaN, infinis, zéro signé, subnormaux, arrondis, erreurs relatives/absolues (tests numériques existants à étendre) | TESTÉ_UNITAIREMENT (partiel, 0.2) |
| PF4 | Mode strict par défaut, mode rapide explicite ; jamais de `fast-math` silencieux | À FAIRE |
| PF5 | Analyse IR/assembleur : inlining, vectorisation, boucles, élimination prouvée des contrôles, LTO/PGO si rentables | À FAIRE (A2) |
| PF6 | Coût mesuré des abstractions OOP, contrats, concurrence, RT | À FAIRE |
| PF7 | Garde-fous automatiques contre les régressions de performance et de correction (CI) | À FAIRE |

### 3.7 Auto-hébergement (P12) — Outils, revue Main

| ID | Amélioration | Statut |
|---|---|---|
| SH0 | Inventaire des capacités manquantes et des liaisons système nécessaires | À FAIRE (O8) |
| SH1 | Bibliothèques d'outillage en NEXUL : tokenisation, collections, UTF-8, diagnostics, fichiers, CLI | À FAIRE |
| SH2 | Passes du compilateur portées en NEXUL, comparées au compilateur Rust sur le même corpus | À FAIRE |
| SH3 | Chaîne stage0 → stage1 → stage2, même suite de conformité | À FAIRE |
| SH4 | Équivalence comportementale, reproductibilité après normalisation, performances | À FAIRE |
| SH5 | Bascule seulement après réussite complète ; procédure d'amorçage et retour arrière documentés | À FAIRE |

### 3.8 Outils `nxc` — Outils

| ID | Amélioration | Statut |
|---|---|---|
| N1 | `nxc lsp` (stdio) avec le contrat approuvé par Main | À FAIRE (O3) |
| N2 | Version du schéma JSON de `check --json` et `nxc ide` | À FAIRE (O3) |
| N3 | Vérification du texte non enregistré (diagnostics pendant la frappe) | À FAIRE (O3) |
| N4 | Formateur : accolade fermante, déterminisme | À FAIRE (O4) |
| N5 | `nxc explain` pour chaque code d'erreur | à vérifier (Q2) |
| N6 | `nxc migrate` ou `--edition` | À FAIRE (O10) |
| N7 | Runtime macOS corrigé avant toute annonce de compatibilité | À FAIRE (O7) |

### 3.9 Web et WebAssembly (P11) — Outils (cible), Luna (modèle), Main (aperçu)

| ID | Amélioration | Statut |
|---|---|---|
| W1 | `wasm32-web`, ABI v1, chargeur `lib/web/nexul.mjs`, exemple | TESTÉ_UNITAIREMENT (déclaré, PR #1) |
| W2 | Modèle NEXUL + WebAssembly + JavaScript | TESTÉ_UNITAIREMENT (PR #12, revérifié par Main : §5) |
| W3 | Aperçu isolé dans Studio (moteur web dans son panneau seulement), erreurs et rechargement | À FAIRE (M11) |
| W4 | Tests navigateur supplémentaires : Unicode, réentrance, appels synchrones, DOM | À FAIRE (O9) |

### 3.10 NEXUL Studio natif — Main (UI), Luna (crates)

| ID | Amélioration | Statut |
|---|---|---|
| ST1 | Explorateur : créer, renommer, Corbeille, couper/copier/coller, glisser-déposer, import | TESTÉ_UNITAIREMENT |
| ST2 | Services NEXUL : diagnostics, survol, complétion, définition, renommage, formatage | TESTÉ_UNITAIREMENT |
| ST3 | Palette de commandes et ouverture rapide | TESTÉ_UNITAIREMENT |
| ST4 | Aucune perte : dialogue de fermeture (onglet, fenêtre, **changement de dossier**), conflits disque, enregistrement atomique, récupération | TESTÉ_UNITAIREMENT (P1 ; changement de dossier `eb490ce`) |
| ST5 | Cas réels : disque plein, permissions, chemins longs, UNC, liens symboliques, encodages | À FAIRE (Q3 + M) |
| ST6 | Coloration `studio/syntax` | INTÉGRÉ (M3) |
| ST7 | LSP multi-langage avec serveurs détectés | À FAIRE (M5) |
| ST8 | Terminal intégré interactif | À FAIRE (L1 → M6) |
| ST9 | Débogueur (DAP) | À FAIRE (L2 → M7) |
| ST10 | Assistant Nouveau projet et SDK | INTÉGRÉ (M4) |
| ST11 | Exécution, construction, tests, explorateur de tests | À FAIRE (M8) |
| ST12 | Paramètres, raccourcis personnalisables, enregistrement automatique | À FAIRE (M9) |
| ST13 | Recherche et remplacement globaux | INTÉGRÉ (M10) |
| ST14 | Plan du document, symboles du projet, références (`nxc ide symbols/references`) | À FAIRE |
| ST15 | Panneaux déplaçables, dispositions, éditeurs côte à côte | À FAIRE |
| ST16 | Performances sur gros fichiers, Unicode, accessibilité | À FAIRE |
| ST17 | Binaire autonome (CRT statique), icône, informations de version | CRT statique INTÉGRÉ (M2) ; icône et version À FAIRE |
| ST18 | Réglages Linux (`$XDG_CONFIG_HOME`) et macOS ; notification de mise à jour | À FAIRE (après L7) |
| ST19 | Retrait de l'ancienne interface Edge après parité | À FAIRE (M12) |
| ST20 | Avancé : centre NEXUL (benchmarks, IR/assembleur, précision, simulation RT, concurrence), Git, extensions, documentation intégrée, observation des threads et échéances | À FAIRE (après stabilité) |
| ST21 | Diagnostics du nouveau typage (effets, contrats) quand ils existent | À FAIRE |

### 3.11 Extension VS Code (P10) — Luna, revue Main

| ID | Amélioration | Statut |
|---|---|---|
| V1 | `.nx` et `nexul.toml` reconnus, icônes, langage | À FAIRE (L4) |
| V2 | Grammaire TextMate alignée sur la syntaxe officielle, tests de jetons | À FAIRE (après ADR) |
| V3 | Extraits, indentation, appariement, commentaires, plissage, conventions de `nxc fmt` | À FAIRE (après ADR) |
| V4 | Diagnostics de `nxc check` puis `nxc lsp`, colonnes Unicode correctes, texte non enregistré | À FAIRE (L4, puis N1) |
| V5 | Complétion, survol, définition, références, renommage, symboles, formatage selon le serveur | À FAIRE (après N1) |
| V6 | Commandes : nouveau projet, build debug/release, run, test, check, format, `explain` | À FAIRE (L4) |
| V7 | `tasks.json`, `launch.json` avec un vrai débogueur (LLDB/DAP) | À FAIRE |
| V8 | Projets `nxpm`, cibles (`nxc targets`), builds WebAssembly | À FAIRE |
| V9 | Documentation, dépannage Windows/Linux/macOS, versions de `nxc` prises en charge | À FAIRE (Q5) |
| V10 | VSIX reproductible, tests hôte, essais réels ; publication Marketplace seulement par l'utilisateur (décision 7) | À FAIRE |

### 3.12 Installation et distribution — Luna

| ID | Amélioration | Statut |
|---|---|---|
| I1 | Installateur sans PowerShell, `reg.exe` ni `taskkill` (API Windows) | INTÉGRÉ (PR #11, déclaré) |
| I2 | Studio natif installé par défaut, ancien Studio Edge retiré de l'installation | À FAIRE (L3) |
| I3 | Raccourcis, association `.nx`, désinstallation, migration des préférences | À FAIRE (L3) |
| I4 | Interface native de l'installateur (étapes 2 et 3) | À FAIRE (L3) |
| I5 | Mises à jour signées, Linux et macOS selon les études | CONÇU (L7) |

### 3.13 Intégration continue, versions, qualité — Main

| ID | Amélioration | Statut |
|---|---|---|
| Q-CI1 | CI Windows (GNU compilateur, MSVC `studio/app`) et Linux ; espaces de travail séparés testés | CONÇU (M13, décision 8) |
| Q-CI2 | Cibles embarquées et RT selon le matériel disponible | À FAIRE |
| Q-CI3 | Artefacts de version installables, notes de version, retour arrière | À FAIRE (M16) |
| Q-CI4 | Scénarios E2E A à H exécutés et consignés | À FAIRE (P14) |
| Q-CI5 | Matrice des preuves tenue à jour | À FAIRE (Q1) |

### 3.14 Documentation — chaque propriétaire

| ID | Amélioration | Propriétaire | Statut |
|---|---|---|---|
| D1 | Spécification et guide identiques au compilateur | Outils (écarts : Q2) | à vérifier |
| D2 | Guide de la nouvelle syntaxe et style | Outils | après ADR |
| D3 | Guide de Studio natif (raccourcis, paramètres, récupération) | Main | À FAIRE |
| D4 | Guide de l'extension VS Code | Luna (brouillon Q5) | À FAIRE |
| D5 | Installation et dépannage | Luna | à mettre à jour (L3) |
| D6 | `README.md` racine, `CHANGELOG.md` | Main | à mettre à jour (M16) |

## 4. Composants existants

| Composant | Où | Propriétaire | Statut | Preuve / manque |
|---|---|---|---|---|
| Compilateur NEXUL 0.2 (`nxc`, `nxpm`, `nxc ide`) | `compiler/` | Outils | LIVRÉ (0.2.0) | release v0.2.0 ; refonte du langage à faire (§3.1 à §3.8) |
| WebAssembly navigateur (`wasm32-web`, ABI v1, chargeur) | `compiler/`, `lib/web/`, `examples/web/` | Outils | TESTÉ_UNITAIREMENT (déclaré) | PR #1 ; aperçu Studio à faire (M11) |
| Studio natif | `studio/app` | Main | INTÉGRÉ | 25 tests fonctionnels ; crates non raccordées (P3) |
| Services Studio (`nexul_studio`) | `studio/src` | Main | TESTÉ_UNITAIREMENT | 9 tests |
| Ancienne interface web (Edge) | `studio/src`, `studio/ui` | Main | LIVRÉ (0.2.0) | à retirer à la parité (M12) |
| Coloration NEXUL | `studio/syntax` | Luna | TESTÉ_UNITAIREMENT | §5 ; à raccorder (M3) |
| Client LSP + détection SDK | `studio/lsp` | Luna | TESTÉ_UNITAIREMENT | §5 ; à raccorder (M4, M5) |
| Terminal PTY + émulation | `studio/terminal` | Luna | IMPLÉMENTÉ | 2 essais PTY bloqués (L1) |
| Client DAP + adaptateurs | `studio/dap` | Luna | IMPLÉMENTÉ | essai réel `lldb-dap` en échec (L2) |
| Modèles de projets (dont NEXUL + Web) | `studio/templates` | Luna | TESTÉ_UNITAIREMENT | §5 ; assistant GPUI (M4) |
| Installateur Windows | `installer/` | Luna | LIVRÉ (0.2.0) | API Windows (PR #11) ; empaquette encore l'ancien Studio (L3) |
| Extension VS Code | `editors/vscode/` | Luna | À FAIRE | L4 |
| Auto-hébergement | — | Outils | À FAIRE | O8 |

## 5. Vérifications par Main

Commande : `cargo test --manifest-path studio/<crate>/Cargo.toml` (chaîne GNU sauf mention),
PC de Main.

| Crate | Révision | Résultat |
|---|---|---|
| `studio/app` | `030d617` (MSVC) | 29 tests réussis |
| `studio/src` (`nexul-studio`) | `030d617` | 12 tests réussis |
| `studio/syntax` | `548b1f6` | 26 tests réussis |
| `studio/lsp` | `548b1f6` | 32 tests réussis |
| `studio/terminal` | `548b1f6` | **ne se construit pas avec la chaîne GNU** (`dlltool` introuvable) ; avec MSVC : 3 tests réussis, **2 bloqués** (`pty_resize_and_stop_are_reported`, `pty_runs_interactive_program_and_exits`, arrêtés après 60 s) |
| `studio/dap` | `548b1f6` | 4 tests réussis ; **échec** de `lldb_dap_debugs_a_nexul_program_with_breakpoint_variables_and_step` : `Timeout("launch")` (LLVM 23) |
| `studio/templates` | `edb886c` | 7 tests réussis avec `nxc` construit depuis `main` (`wasm32-web` annoncé) : le modèle NEXUL + Web est construit en vrai et son ABI vérifiée avec Node.js (aucun essai sauté) |

## 6. Décisions à faire valider par l'utilisateur (passation §16)

| # | Décision | Recommandation de Main | Statut |
|---|---|---|---|
| 1 | Gouvernance V3 et transferts | appliquer | **validée le 15/10** |
| 2 | Édition du langage : nouvelle syntaxe incompatible ou édition opt-in avec migrateur | version majeure + outil de migration, pas de casse silencieuse | à décider (après G1) |
| 3 | Première cible temps réel | à proposer après l'étude O6 | à décider |
| 4 | Niveau des contrats (partout ou profil strict) | à proposer avec O5 | à décider |
| 5 | Concurrence par cible | natif d'abord | à décider |
| 6 | Périmètre initial de l'auto-hébergement | à proposer avec O8 | à décider |
| 7 | Extension VS Code : nom, éditeur, publication Marketplace | publication seulement sur action explicite de l'utilisateur | à décider |
| 8 | CI de confiance (Windows, Linux, cibles embarquées) | GitHub Actions Windows + Linux pour commencer | à décider |

## 7. Intégration

| Branche | Contenu | État |
|---|---|---|
| `refonte/main-p0-gouvernance` | gouvernance V3 | fusionnée (PR #13) |
| `refonte/claude-tools-installer`, `refonte/claude-tools-etudes` | installateur sur API Windows, études | fusionnées (PR #11) → transfert de `installer/**` à Luna effectif |
| `refonte/codex-exec-wasm-template` | modèle NEXUL + Web | fusionnée (PR #12) |
| `refonte/claude-studio` | palette, ouverture rapide, P1, ce plan, M2, M3, M4, M10 | **PR à ouvrir et fusionner** (`main` déjà fusionné dedans, 29 + 12 tests verts) |
