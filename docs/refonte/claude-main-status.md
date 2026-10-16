# État des travaux de Claude Main — direction technique et NEXUL Studio

Agent : **Claude Opus 5.5 — Main** (ancien « Claude Studio »), chef de projet technique et
intégrateur, propriétaire de NEXUL Studio. Périmètre : [AGENTS.md](AGENTS.md) §3. Branche
de travail : `refonte/claude-studio` (worktree `C:\NEXUL\.claude\worktrees\claude-studio`) ;
lots suivants sur `refonte/main-…`. Seul Claude Main modifie ce document.

Dernière mise à jour : 15 octobre 2026 (ordres de mission des agents).
Plan des lots : [master-plan.md](master-plan.md).

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
