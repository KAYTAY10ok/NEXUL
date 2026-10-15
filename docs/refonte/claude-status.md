# État des travaux de Claude — NEXUL Studio

Agent : **Claude Studio**. Périmètre (voir [AGENTS.md](AGENTS.md)) : `studio/src/**`,
`studio/ui/**`, `studio/app/**`, `studio/plugins/**`, `docs/studio/**` et ce fichier ;
`installer/**`, `studio/syntax/**` et `studio/lsp/**` sont à Claude Outils ;
`studio/terminal/**`, `studio/dap/**` et `studio/templates/**` à Codex Luna. Branche :
`refonte/claude-studio` (worktree séparé). Seul Claude Studio modifie ce document.

Dernière mise à jour : 15 octobre 2026. Répartition des rôles (Codex Astra, Codex Luna,
Claude Studio, Claude Outils) : [AGENTS.md](AGENTS.md).

## Travaux réalisés

| Date | Travail | Commit |
|---|---|---|
| 14/10 | Audit de Studio (phase 0) : [docs/studio/audit-phase0.md](../studio/audit-phase0.md) | |
| 14/10 | **Phase 1 — gestion des fichiers et dossiers corrigée** : création et renommage depuis l'explorateur (ne fonctionnaient pas), validation des noms, renommage de casse, messages d'erreur clairs, Corbeille rapide, glisser-déposer, couper/copier/coller, import depuis l'Explorateur, protection contre la navigation sur dépôt de fichier, session | `1492b45` |
| 14/10 | Services de Studio en bibliothèque (`nexul_studio`) ; surveillance du dossier (`ReadDirectoryChangesW`) ; encodages des fichiers | `7d83e48`, `034e8c9`, `6329869` |
| 15/10 | Prototypes mesurés (Slint, GPUI) ; **GPUI + gpui-component validé par l'utilisateur** : [architecture-native.md](../studio/architecture-native.md) | `bb61206` |
| 15/10 | **Application native** `studio/app` (1re version) : menus, explorateur, onglets, éditeur de code, sortie, barre d'état, thèmes, mode minimaliste, F5, sessions ; 6 tests fonctionnels (fenêtre sans affichage, vraies frappes) | `ac01358` |
| 15/10 | Consignes de coordination des agents : [AGENTS.md](AGENTS.md) | |
| 15/10 | **Explorateur natif** : nouveau fichier/dossier (Ctrl+N, saisie dans l'arbre), renommage sur place (F2), Corbeille après confirmation (Suppr), couper/copier/coller (y compris des fichiers copiés dans l'Explorateur Windows), glisser-déposer interne (Ctrl copie) et depuis l'Explorateur, copier le chemin, afficher dans l'Explorateur ; les éditeurs ouverts suivent renommages et déplacements | `4a9c068` |
| 15/10 | **Services NEXUL dans l'éditeur natif** : diagnostics de `nxc check --json` (soulignés, panneau Problèmes, compteurs), survol, complétion, aller à la définition (`nxc ide --stdin`), renommage de symbole, mise en forme, couleurs du code NEXUL (provisoires, en attendant `studio/syntax`) | `c5c22af` |

## Travaux en cours

- Application native : palette de commandes, paramètres et raccourcis, recherche dans
  les fichiers, exécution (`nxc run/test/build`) ; puis intégration des briques des
  autres agents (coloration et LSP de Claude Outils ; terminal, débogueur et modèles de
  Codex Luna) quand elles seront prêtes.

## Fichiers modifiés

- `studio/src/workspace.rs` (nouveau), `studio/src/main.rs`, `studio/ui/app.js`,
  `studio/ui/style.css`
- `studio/app/**` (application native : `main.rs`, `studio.rs`, `studio/files.rs`,
  `studio/lang.rs`, `explorer.rs`, `ide.rs`, `highlight.rs`, `settings.rs`, `assets.rs`)
- `docs/studio/audit-phase0.md`, `docs/studio/architecture-native.md`,
  `docs/refonte/claude-status.md`

Aucun fichier de Codex Astra n'a été modifié (`compiler/`, `lib/`, `tests/`, `docs/spec/`,
`Cargo.toml`, `Cargo.lock`, `README.md`).

## Tests exécutés

- `cargo test -p nexul-studio` : 8 tests, tous réussis.
- `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1` :
  18 tests (fenêtre sans affichage, vraies frappes, vrai `nxc`), tous réussis.
- Essais manuels de l'interface (détaillés dans l'audit, §3).

## Problèmes rencontrés

- Sans la fonction `tree-sitter` de gpui-component, l'éditeur dessine les diagnostics
  avec une couleur transparente : elle est activée dans `studio/app/Cargo.toml`.

- Les crates d'interface (GPUI, Slint) ne se construisent pas avec la chaîne
  `windows-gnu` de rustup (import `raw-dylib`, bibliothèques d'importation manquantes) ;
  GPUI exige aussi `fxc.exe` du Windows SDK. Les outils C++ de Visual Studio et le
  Windows SDK ont été installés (accord de l'utilisateur) : `studio/app` se construit
  avec la chaîne MSVC.

## Dépendances envers Codex Astra (demandes)

1. **`nxc lsp`** : serveur Language Server Protocol (stdio) reprenant les services de
   `nxc ide`. Critères : `initialize`, `didOpen`/`didChange`, `publishDiagnostics`,
   `hover`, `completion`, `definition`, `references`, `rename`, `documentSymbol`,
   `formatting`. En attendant, Studio continue d'utiliser `nxc ide`.
2. **Version du schéma JSON** de `nxc check --json` et `nxc ide` (champ `version`).
3. **Contrat WebAssembly** `docs/refonte/interop-web.md` : l'intégration « Compiler
   pour le Web » de Studio attend ce document ; aucune commande `nxc` ne sera supposée.
4. **`nxc check --stdin`** (ou `nxc ide check --stdin`) : vérifier le texte non
   enregistré de l'éditeur, pour des diagnostics pendant la frappe. Aujourd'hui Studio
   vérifie le fichier à l'ouverture et à l'enregistrement.
5. **Observation sur `nxc ide format`** : `fn main(){
  println("ok")}` devient
   `fn main() {
    println("ok")}` — l'accolade fermante reste en fin de ligne. À
   confirmer comme comportement voulu ou non (Studio applique le texte rendu tel quel).
6. **Workspace Cargo** : l'application native aura son propre espace de travail Cargo
   sous `studio/` (son propre `Cargo.lock`) pour ne pas toucher aux fichiers de Codex Astra.
   Codex Astra peut décider de l'intégrer au workspace racine.

## Changements attendus dans les interfaces

Aucun pour l'instant : Studio utilise uniquement les commandes existantes de `nxc` et
`nxpm` (`check --json`, `ide`, `fmt`, `explain`, `run`, `test`, `build`, `nxpm list --json`…).
