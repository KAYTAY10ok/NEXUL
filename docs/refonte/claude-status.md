# État des travaux de Claude — NEXUL Studio

Périmètre : `studio/**`, `installer/**`, `docs/studio/**` et ce fichier. Branche :
`refonte/claude-studio` (worktree séparé). Seul Claude modifie ce document.

Dernière mise à jour : 14 octobre 2026.

## Travaux réalisés

| Date | Travail | Commit |
|---|---|---|
| 14/10 | Audit de Studio (phase 0) : [docs/studio/audit-phase0.md](../studio/audit-phase0.md) | |
| 14/10 | **Phase 1 — gestion des fichiers et dossiers corrigée** : création et renommage depuis l'explorateur (ne fonctionnaient pas), validation des noms, renommage de casse, messages d'erreur clairs, Corbeille rapide, glisser-déposer, couper/copier/coller, import depuis l'Explorateur, protection contre la navigation sur dépôt de fichier, session | `1492b45` |
| 14/10 | Comparatif des boîtes à outils natives et prototypes : [docs/studio/architecture-native.md](../studio/architecture-native.md) (**choix à valider par l'utilisateur**) | |

## Travaux en cours

- Phase 2 : choix de la boîte à outils native (en attente de validation), puis
  extraction des services Rust de Studio en bibliothèque (`studio/core`).

## Fichiers modifiés

- `studio/src/workspace.rs` (nouveau), `studio/src/main.rs`, `studio/ui/app.js`,
  `studio/ui/style.css`
- `docs/studio/audit-phase0.md`, `docs/studio/architecture-native.md`,
  `docs/refonte/claude-status.md`

Aucun fichier de Codex n'a été modifié (`compiler/`, `lib/`, `tests/`, `docs/spec/`,
`Cargo.toml`, `Cargo.lock`, `README.md`).

## Tests exécutés

- `cargo test -p nexul-studio` : 5 tests, tous réussis.
- Essais manuels de l'interface (détaillés dans l'audit, §3).

## Problèmes rencontrés

- Machine de développement sans compilateur C/C++ : les dépendances qui compilent du
  C/C++ ne se construisent pas. Les crates `windows` récentes (raw-dylib) exigent un
  `dlltool` fonctionnel avec la chaîne `windows-gnu` : celui de rustup n'a pas
  d'assembleur ; `llvm-dlltool` (fourni avec LLVM) convient
  (`-C dlltool=C:/Program Files/LLVM/bin/llvm-dlltool.exe`).

## Dépendances envers Codex (demandes)

1. **`nxc lsp`** : serveur Language Server Protocol (stdio) reprenant les services de
   `nxc ide`. Critères : `initialize`, `didOpen`/`didChange`, `publishDiagnostics`,
   `hover`, `completion`, `definition`, `references`, `rename`, `documentSymbol`,
   `formatting`. En attendant, Studio continue d'utiliser `nxc ide`.
2. **Version du schéma JSON** de `nxc check --json` et `nxc ide` (champ `version`).
3. **Contrat WebAssembly** `docs/refonte/interop-web.md` : l'intégration « Compiler
   pour le Web » de Studio attend ce document ; aucune commande `nxc` ne sera supposée.
4. **Workspace Cargo** : l'application native aura son propre espace de travail Cargo
   sous `studio/` (son propre `Cargo.lock`) pour ne pas toucher aux fichiers de Codex.
   Codex peut décider de l'intégrer au workspace racine.

## Changements attendus dans les interfaces

Aucun pour l'instant : Studio utilise uniquement les commandes existantes de `nxc` et
`nxpm` (`check --json`, `ide`, `fmt`, `explain`, `run`, `test`, `build`, `nxpm list --json`…).
