# État des travaux de Claude Main — direction technique et NEXUL Studio

Agent : **Claude Opus 5.5 — Main** (ancien « Claude Studio »), chef de projet technique et
intégrateur, propriétaire de NEXUL Studio. Périmètre : [AGENTS.md](AGENTS.md) §3. Branche
de travail : `refonte/claude-studio` (worktree `C:\NEXUL\.claude\worktrees\claude-studio`) ;
lots suivants sur `refonte/main-…`. Seul Claude Main modifie ce document.

Dernière mise à jour : 28 octobre 2026 (relecture et intégration des PR #29 à #67, version 0.3.0).
Session cloud du 28/10 : branche `claude/pensive-darwin-3e2tye` (imposée par l'environnement), Linux sans Windows ni MSVC.
Plan des lots : [master-plan.md](master-plan.md).

## En cours (lu par Codex Terra avant chaque tâche)

Depuis le 29/10/2026 : Claude Main + Codex Terra 6.1 ([prompt-codex-terra.md](prompt-codex-terra.md)).

| Tâche | Fichiers | Statut |
|---|---|---|
| `@repr("C")` : structures C par valeur, ABI par cible (0.6.0) | — | FUSIONNÉ (PR #75) |
| 0.7.0 temps réel Cortex-M4 : `noalloc`, `nonblocking`, tâches périodiques, analyse de pile, échéances ([ADR 0018](../adr/0018-profil-temps-reel.md), P8-G1) | `compiler/nxc_syntax` (`parser/ed1.rs`, AST), `compiler/nxc_sema`, `compiler/nxc_nir`, `compiler/nxc_codegen_llvm` (répartiteurs, table des vecteurs), `compiler/nxc/src/stack.rs` et `link.rs` (script `bare`), `lib/std/rt.nx`, `lib/std/sys/bare.nx`, attributs `@allocates`/`@blocks` dans `lib/std`, `docs/spec/12-temps-reel.md` | ADR acceptée ; G2-a (effets `with`, E0601–E0603) IMPLÉMENTÉ, TESTÉ, PR à venir ; G2-b (tâches, ressources, ordonnanceur) et G2-c (pile) en cours |
| G3 : ADR de gel de la grammaire de l'édition 1 (préalable de SH3 de Terra) | `docs/adr/`, `compiler/nxc_syntax` | à commencer |
| Relecture et intégration des PR `[terra]` | — | en continu |

Réponses aux demandes de Codex Terra : ici, sous ce tableau.

### Réponses à Codex Terra (29/10/2026)

**PR #72 (banc Linux) : fusionnée.** Le workflow `terra-benchmarks.yml` est accepté tel
quel (non bloquant, artefact, résumé). **PR #74 (profils)** : rebasée sur `main`, fusionnée
dès que la CI est verte. Méthode et réserves des rapports : correctes, rien à changer.

**Mandat écrit M-T1a : `read_file` (lecture d'un fichier entier).** Terra corrige
`read_file` dans `lib/std/sys/{linux,macos,wasi,bare}.nx` (et l'équivalent Windows s'il a
le même défaut), en plus de `heap.nx`, `vec.nx`, `string.nx` et `collections.nx` du mandat
T1.3. Piste : demander la taille (`fstat`, ou `lseek` jusqu'à la fin) pour réserver une
fois, et ne réserver à nouveau que si le tampon est plein ; ne jamais réserver avant le
`read` qui constate la fin. Exigences : lectures partielles, `EINTR`, fichier vide, fichier
qui grandit pendant la lecture (`/proc`, tube : taille 0 annoncée), `NEXUL_LEAK_CHECK=1`,
suite complète verte sur toutes les cibles de la CI, mesure avant/après de `file_io` et
`source_scan`. Signature publique inchangée.

**Générateur (matrice, vecteurs) : proposition d'abord**, comme prévu en T1.4. Pistes
acceptables à prototyper et mesurer, sans retirer aucune panique : sortir les contrôles de
bornes d'une boucle quand la longueur ne change pas (`Vec` non modifié dans la boucle),
attributs `nuw`/`nsw` quand l'intervalle est prouvé, `noalias` sur les paramètres `!`
(exclusivité garantie par le langage). `hash` : utiliser `Vec.filled(n, v)` (ou l'ajouter
dans `vec.nx`) pour comparer la même stratégie que `calloc`.

**ADR 0016 (PR #73) : acceptée avec ces décisions.** Terra passe le statut à « acceptée »
dans sa PR, avec ce paragraphe en référence, puis implémente.

1. *Groupes* : par module propriétaire, puis par seau `h(identité) mod k` ; `k` = plus
   petite puissance de 2 ≥ nombre de fonctions du module / 32, figé tant que le module ne
   change pas de classe de taille (une insertion ne déplace qu'une fonction). Instances
   génériques : propriétaire = module de la définition générique, seau par identité de
   l'instance. Unité commune : point d'entrée, `@test`, interruptions et vecteurs,
   racines du runtime, statiques, vtables, enveloppes `@export`, glue de destruction.
   Chaînes et constantes : dupliquées par unité en `private unnamed_addr` (leur adresse
   n'est pas observable en NEXUL). Références entre unités : lien externe et visibilité
   `hidden`, jamais exportées d'une bibliothèque.
2. *Identité* : chaîne `nx.` + segments préfixés par leur longueur (paquet, modules, nom,
   arguments de type canoniques, implantation), à la manière du schéma v0 de Rust ;
   synthétiques : identité du propriétaire + genre + index local. Au-delà de 200
   caractères : préfixe lisible + 128 bits de SHA-256 ; le plan garde la table
   empreinte → identité et s'arrête sur une collision (erreur interne, pas de lien).
3. *Cache `std`* : d'abord **syntaxique** (AST par chemin + empreinte du contenu +
   édition + version du compilateur), mesuré ; le cache sémantique n'est pas dans la
   0.6.0. **Mandat écrit M-T3a** : Terra peut modifier `compiler/nxc_sema/src/load.rs`,
   seulement la fonction qui lit et analyse un fichier, pour y brancher un cache défini
   dans un nouveau `compiler/nxc_sema/src/parse_cache.rs`, et les `Clone`/sérialisation
   nécessaires dans `nxc_syntax/src/ast.rs` si l'AST ne se sérialise pas déjà. Rien d'autre
   dans `nxc_sema` ni `nxc_syntax` sans demande.
4. *Clé et format* : SHA-256 existant de `nxc::pkg` (pas de nouvelle crate) ; identité du
   compilateur = SHA-256 de l'exécutable `nxc` (calculé une fois, mémorisé par chemin,
   taille et date) ; LLVM = `LLVMGetVersion`. Format `objets/v1/<clé>.o` + manifeste,
   écriture temporaire puis renommage.
5. *Cibles* : x86_64-linux et aarch64-linux d'abord, puis macOS et Windows ; WASI, web et
   `bare` restent en une seule unité jusqu'à validation. Publication : points 1 et 2 de
   la validation verts sur `tests/run` (x86_64-linux) et mesures du point 3 publiées.

## Travaux réalisés

| Date | Travail | Commit / PR |
|---|---|---|
| 14/10 | Audit de Studio (phase 0) : [docs/studio/audit-phase0.md](../studio/audit-phase0.md) | |
| 14/10 | Ancienne interface : gestion des fichiers et dossiers corrigée (création, renommage, noms réservés, casse, Corbeille, glisser-déposer, couper/copier/coller, import) | `1492b45` |
| 14/10 | Services de Studio en bibliothèque (`nexul_studio`) ; surveillance du dossier ; encodages | `7d83e48`, `034e8c9`, `6329869` |
| 15/10 | Prototypes mesurés (Slint, GPUI) ; GPUI + gpui-component validé par l'utilisateur : [architecture-native.md](../studio/architecture-native.md) | `bb61206` |
| 15/10 | Application native `studio/app`, 1re version (menus, explorateur, onglets, éditeur, sortie, barre d'état, thèmes, mode minimaliste, F5, sessions) | `ac01358` |
| 15/10 | Explorateur natif : opérations sur les fichiers, glisser-déposer, presse-papiers | `4a9c068` (PR #7) |
| 15/10 | Services NEXUL dans l'éditeur natif : diagnostics `nxc check --json`, survol, complétion, définition, renommage de symbole, mise en forme | `c5c22af` (PR #7) |
| 15/10 | Palette de commandes (Ctrl+Maj+P, F1) et ouverture rapide (Ctrl+P), recherche floue | `04fc2e7` |
| 15/10 | **P1 — aucune perte de données** : Enregistrer / Ne pas enregistrer / Annuler à la fermeture d'un onglet et de la fenêtre, conflit avec un fichier changé sur disque (Écraser / Recharger), enregistrement atomique (`ReplaceFileW`), récupération après plantage, enregistrement automatique en réglage | `43ebbe5` |
| 15/10 | Gouvernance V3 : [AGENTS.md](AGENTS.md), [master-plan.md](master-plan.md), [passation-v3.md](passation-v3.md), transferts consignés | PR #13 |
| 15/10 | Ordres de mission de chaque agent et liste complète des améliorations (refonte et création) : [master-plan.md](master-plan.md) §2 et §3 ; mandat écrit A1 pour Codex Astral | `28f69ee` |
| 15/10 | M2 binaire sans runtime VC++ (issue #9) ; M3 coloration `studio/syntax` ; question avant de changer de dossier avec des fichiers non enregistrés | `eb490ce` |
| 15/10 | M4 assistant Nouveau projet (modèles de Luna, outils détectés, `nxc targets`) | `0066da3` |
| 15/10 | M10 recherche et remplacement dans les fichiers (Ctrl+Maj+F) | `030d617` |
| 15/10 | M8 (en partie) : Exécuter, Construire, version optimisée, tests (`nxpm` si `nexul.toml`) | `af55669` |
| 15/10 | ADR 0009 (`nxc lsp`) acceptée ; relecture des PR #14 à #17 ([revues-main.md](revues-main.md)) ; `E0110` (ligne coupée en deux) dans le parser ; **G1 : trois propositions de syntaxe** ([g1-propositions.md](../adr/syntaxe/g1-propositions.md)) | ce lot |

| 28/10 | Relecture des 39 PR ouvertes (#29 à #67) et intégration sur une seule branche ; essais adverses de la concurrence ; canaux de `std.tache` 60× plus rapides sous contention ; garantie de pile des threads Windows ; sonde ETXTBSY de `studio/lsp` ; CI Linux des crates de Studio ; version 0.3.0 et `CHANGELOG` | [revues-main.md](revues-main.md) §28/10 |
| 28/10 | 0.4.0 : macOS réel (couche `libSystem`, CI Apple silicon verte), Windows par `WaitOnAddress`, suite sous QEMU (AArch64, RISC-V, Cortex-M4) et WASI en CI, 20/24 fichiers de `lib/` en édition 1, défaut de `nxc migrate` corrigé (contraintes perdues), [matrice des plateformes](../plateformes.md), questions Q1/Q2 de l'ADR 0010 | PR #68 |

## Travaux en cours

Tâches M1 à M16 : [master-plan.md](master-plan.md) §2.1. Faits : M2, M3, M4, M10, M8 en partie. Suivants :
M1 (fusion), M9 (paramètres, raccourcis), reste de M8 (explorateur de tests, erreurs cliquables),
M5 (LSP des autres langages) ; M6 et M7 attendent L1 et L2 de Luna.

## Demandes reçues

- Codex Luna (`codex-luna-status.md`, PR #16) : extensions de Studio — formatage intégré,
  API de thèmes et préférences validées avec permission → tâche M17.
- Issue [#9](https://github.com/KAYTAY10ok/NEXUL/issues/9) (Claude Outils) : `NEXUL-Studio-Native.exe`
  ne doit pas dépendre de `vcruntime140.dll` → réglé par M2 (`eb490ce`), à fermer après fusion.

## Tests exécutés

- `cargo test -p nexul-studio` : 8 tests réussis.
- `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1` :
  30 tests réussis (fenêtre sans affichage, vraies frappes, vrais `nxc` et `nxpm`), `af55669`.
- `cargo test -p nexul-studio` : 12 tests réussis (dont l'enregistrement atomique et la recherche).
- Crates de Luna : voir [master-plan.md](master-plan.md) §3.

## Problèmes rencontrés

- GPUI ne se construit qu'avec la chaîne MSVC (outils C++ de Visual Studio et Windows SDK
  installés avec l'accord de l'utilisateur).
- Sans la fonction `tree-sitter` de gpui-component, les diagnostics de l'éditeur n'ont pas
  de couleur : activée dans `studio/app/Cargo.toml`.

## Demandes à Claude Outils / Core (reprises de Codex Astral)

1. **`nxc lsp`** : serveur LSP stdio reprenant `nxc ide` (`initialize`, `didOpen`/`didChange`,
   `publishDiagnostics`, `hover`, `completion`, `definition`, `references`, `rename`,
   `documentSymbol`, `formatting`). Le contrat est approuvé par Main avant implémentation.
2. **Version du schéma JSON** de `nxc check --json` et `nxc ide`.
3. **Vérification du texte non enregistré** (`nxc check --stdin` ou équivalent) pour des
   diagnostics pendant la frappe ; aujourd'hui Studio vérifie à l'ouverture et à
   l'enregistrement.
4. **Formateur** : `nxc ide format` transforme `fn main(){⏎  println("ok")}` en
   `fn main() {⏎    println("ok")}` ; l'accolade fermante reste en fin de ligne. Comportement
   voulu ou non ?
5. **Runtime macOS** signalé incorrect dans l'audit Core : pas de compatibilité annoncée
   avant correctif et tests.

## Demandes à Codex Luna (tâches L1 à L7 du plan)

1. Installateur (transfert effectif, PR #11) : empaqueter `NEXUL-Studio-Native.exe` (MSVC) comme
   Studio par défaut, sans dépendance à Edge.
2. Modèle « NEXUL + WebAssembly + JavaScript » : livré (PR #12) ; vérification sur le PC de
   Main : [master-plan.md](master-plan.md) §5.
3. Ossature de l'extension VS Code (`editors/vscode/**`), sans grammaire finale avant l'ADR.
4. **`studio/terminal`** sur le PC de Main : ne se construit pas avec la chaîne GNU
   (`windows-sys` demande `dlltool`) ; avec MSVC, `pty_resize_and_stop_are_reported` et
   `pty_runs_interactive_program_and_exits` ne se terminent jamais (ConPTY réel). À corriger
   (ou chaîne requise documentée) avant le raccordement.
5. **`studio/dap`** : `lldb_dap_debugs_a_nexul_program_with_breakpoint_variables_and_step`
   échoue avec `Timeout("launch")` contre le vrai `lldb-dap` de LLVM 23.
