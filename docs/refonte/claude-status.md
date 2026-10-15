# État des travaux de Claude Main — direction technique et NEXUL Studio

Agent : **Claude Opus 5.5 — Main** (ancien « Claude Studio »), chef de projet technique et
intégrateur, propriétaire de NEXUL Studio. Périmètre : [AGENTS.md](AGENTS.md) §3. Branche
de travail : `refonte/claude-studio` (worktree `C:\NEXUL\.claude\worktrees\claude-studio`) ;
lots suivants sur `refonte/main-…`. Seul Claude Main modifie ce document.

Dernière mise à jour : 15 octobre 2026 (gouvernance V3 validée par l'utilisateur).
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
| 15/10 | Gouvernance V3 : [AGENTS.md](AGENTS.md), [master-plan.md](master-plan.md), [passation-v3.md](passation-v3.md), transferts consignés | ce lot |

## Travaux en cours

- **P1 — protection des données de Studio** (passation §11 P0) : boîte « Enregistrer / Ne
  pas enregistrer / Annuler » à la fermeture d'onglet et de fenêtre (aujourd'hui la
  fermeture d'un onglet modifié **enregistre sans demander**), conflits avec un fichier
  changé sur disque, enregistrement atomique, récupération après plantage.
- **P3 — raccordement des cinq crates** dans `studio/app` : `syntax` (remplace la coloration
  provisoire), `lsp`, `terminal`, `dap`, `templates`.

## Tests exécutés

- `cargo test -p nexul-studio` : 8 tests réussis.
- `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1` :
  21 tests réussis (fenêtre sans affichage, vraies frappes, vrai `nxc`).

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

## Demandes à Codex Luna

1. Installateur (après transfert) : empaqueter `NEXUL-Studio-Native.exe` (MSVC) comme
   Studio par défaut, sans dépendance à Edge.
2. Modèle « NEXUL + WebAssembly + JavaScript » fondé sur l'ABI v1 et `lib/web/nexul.mjs`.
3. Ossature de l'extension VS Code (`editors/vscode/**`), sans grammaire finale avant l'ADR.
