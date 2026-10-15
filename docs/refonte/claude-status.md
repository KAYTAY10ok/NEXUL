# État des travaux de Claude — NEXUL Studio

Agent : **Claude Studio**. Périmètre (voir [AGENTS.md](AGENTS.md)) : `studio/src/**`,
`studio/ui/**`, `studio/app/**`, `studio/plugins/**`, `docs/studio/**` et ce fichier ;
`installer/**`, `studio/syntax/**` et `studio/lsp/**` sont à Claude Outils ;
`studio/terminal/**`, `studio/dap/**` et `studio/templates/**` à Codex Luna. Branche :
`refonte/claude-studio` (worktree séparé). Seul Claude Studio modifie ce document.

Dernière mise à jour : 15 octobre 2026. Répartition des rôles (Codex Astra, Claude Studio,
Claude Outils) : [AGENTS.md](AGENTS.md).

## Travaux réalisés

| Date | Travail | Commit |
|---|---|---|
| 14/10 | Audit de Studio (phase 0) : [docs/studio/audit-phase0.md](../studio/audit-phase0.md) | |
| 14/10 | **Phase 1 — gestion des fichiers et dossiers corrigée** : création et renommage depuis l'explorateur (ne fonctionnaient pas), validation des noms, renommage de casse, messages d'erreur clairs, Corbeille rapide, glisser-déposer, couper/copier/coller, import depuis l'Explorateur, protection contre la navigation sur dépôt de fichier, session | `1492b45` |
| 14/10 | Services de Studio en bibliothèque (`nexul_studio`) ; surveillance du dossier (`ReadDirectoryChangesW`) ; encodages des fichiers | `7d83e48`, `034e8c9`, `6329869` |
| 15/10 | Prototypes mesurés (Slint, GPUI) ; **GPUI + gpui-component validé par l'utilisateur** : [architecture-native.md](../studio/architecture-native.md) | `bb61206` |
| 15/10 | **Application native** `studio/app` (1re version) : menus, explorateur, onglets, éditeur de code, sortie, barre d'état, thèmes, mode minimaliste, F5, sessions ; 6 tests fonctionnels (fenêtre sans affichage, vraies frappes) | `ac01358` |
| 15/10 | Consignes de coordination des trois agents : [AGENTS.md](AGENTS.md) | |

## Travaux en cours

- Application native : opérations sur les fichiers dans l'explorateur, diagnostics,
  services `nxc ide` dans l'éditeur, terminal intégré, palette de commandes, paramètres.

## Fichiers modifiés

- `studio/src/workspace.rs` (nouveau), `studio/src/main.rs`, `studio/ui/app.js`,
  `studio/ui/style.css`
- `docs/studio/audit-phase0.md`, `docs/studio/architecture-native.md`,
  `docs/refonte/claude-status.md`

Aucun fichier de Codex Astra n'a été modifié (`compiler/`, `lib/`, `tests/`, `docs/spec/`,
`Cargo.toml`, `Cargo.lock`, `README.md`).

## Tests exécutés

- `cargo test -p nexul-studio` : 8 tests, tous réussis.
- `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1` :
  6 tests fonctionnels, tous réussis.
- Essais manuels de l'interface (détaillés dans l'audit, §3).

## Problèmes rencontrés

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
4. **Workspace Cargo** : l'application native aura son propre espace de travail Cargo
   sous `studio/` (son propre `Cargo.lock`) pour ne pas toucher aux fichiers de Codex Astra.
   Codex Astra peut décider de l'intégrer au workspace racine.

## Changements attendus dans les interfaces

Aucun pour l'instant : Studio utilise uniquement les commandes existantes de `nxc` et
`nxpm` (`check --json`, `ide`, `fmt`, `explain`, `run`, `test`, `build`, `nxpm list --json`…).
