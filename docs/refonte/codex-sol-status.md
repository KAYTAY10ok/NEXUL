# État de Codex Sol — composants, outils et distribution

Mis à jour le 15 octobre 2026. Mandat : `prompt-codex-sol.md` et charte « Équipe
réduite » du worktree de Claude Main, lus à la demande de l'utilisateur.

## Périmètre

`studio/terminal/**`, `studio/dap/**`, `studio/templates/**`, `studio/lsp/**`,
`studio/syntax/**`, `editors/vscode/**`, `installer/**`, scripts d'empaquetage
attribués, `docs/installation.md`, `benchmarks/**` et ce fichier. P9 seulement :
CLI/JSON/LSP dans les fichiers explicitement délégués de `compiler/nxc/`.
Claude Main conserve la revue et l'intégration.

## Lots

| Ordre | Tâche | Statut et preuves |
|---|---|---|
| 1 | L1 — arrêt et sortie ConPTY | TESTÉ_UNITAIREMENT : 7 tests MSVC réussis, ConPTY réel inclus ; [PR #20](https://github.com/KAYTAY10ok/NEXUL/pull/20), revue Main attendue |
| 2 | L2 — vrai `lldb-dap` | Non testé ; après L1 |
| 3 | P9 — CLI et LSP | Non testé ; lecture du contrat de Main obligatoire avant tout code |
| 4 | L3 — installateur natif | Non testé ; après P9 |

## Travail courant

- Base : `origin/main` à `f4f42cd`, récupérée le 15 octobre 2026.
- Branche L1 : `refonte/sol-l1-terminal` ; worktree indépendant sous
  `files-mentioned-by-the-user-prompt/work/NEXUL-sol`.
- `main` ne contient pas encore l'encadré d'équipe réduite ni `docs/spec/lsp.md` ;
  le mandat fourni par l'utilisateur et les documents de Main font référence.
- Aucun changement dans les worktrees de Main ni dans le cœur du compilateur.

## Preuves L1

- Avant correction : commande MSVC lancée ; 3 tests du modèle réussis, les 2 tests
  ConPTY restent bloqués. Exécution interrompue, sans résultat de réussite déclaré.
- Correction : surveillance indépendante de la fin du processus ; fermeture des
  ressources PTY pendant que le lecteur draine ; événement `Exited` unique après
  drainage ; réponses aux requêtes de position du curseur nécessaires à ConPTY.
- `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/terminal/Cargo.toml` :
  **7 réussis, 0 échec, 0 ignoré**, tests en 0,51 s, commande complète en **2,76 s**
  (compilation incrémentale incluse). PowerShell et `cmd.exe` réels.
- Régressions : requête curseur fragmentée, sortie normale, redimensionnement et
  arrêt idempotent, sortie non nulle avec plus de 64 Kio de texte à drainer.
- `cargo +stable-x86_64-pc-windows-msvc fmt --manifest-path studio/terminal/Cargo.toml` : réussi.
- README : chaîne MSVC requise documentée ; cycle de vie et contrat des événements
  décrits pour la vue GPUI de Main. Chaîne GNU et Unix : non testés ici.
- Intégration GPUI et validation de Main : non testées ; aucun statut `LIVRÉ` déclaré.

## Demandes à Claude Main

Revue de la PR L1 et raccordement M6 : consommer les événements jusqu'à `Exited`,
conserver le snapshot après sortie et exécuter les opérations bloquantes hors du fil UI.
