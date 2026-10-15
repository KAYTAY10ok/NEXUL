# État des travaux de Claude Outils / Core — compilateur, langage, bibliothèque standard

Agent : **Claude Opus 5.5 — Outils / Core**, ingénieur principal du compilateur et de la
sûreté de NEXUL. Périmètre : [AGENTS.md](AGENTS.md), §3 (`compiler/**`, `lib/**`, tests
du langage, `examples/**`, `docs/spec/**`, `docs/guide.md`, `docs/roadmap.md`,
`docs/adr/**`, `tools/wasi-run.mjs`, `interop-web.md`, `audit-core.md`, ce fichier).
Seul Claude Outils / Core modifie ce document. Lots sur `refonte/core-…`.

Historique (installateur, `studio/syntax`, `studio/lsp`, passation à Codex Luna) :
[claude-outils-status.md](claude-outils-status.md), tenu en archive.

Dernière mise à jour : 15 octobre 2026.

## Environnement

PC « autre » d'AGENTS.md §6 : clone GitHub `C:\NEXUL-tools`, Rust 1.99.0
`stable-x86_64-pc-windows-gnu`, **sans LLVM** ni outils C++ de Visual Studio.

- `cargo build -p nxc` réussit, et tout ce qui précède la génération de code fonctionne :
  lexer, parser, analyse sémantique, `nxc check`, `nxc fmt`, `nxc ide`.
- `nxc build`, `nxc run`, `nxc test` et la suite d'exécution demandent LLVM (`LLVM-C.dll`,
  LLD) : **non testés ici**. Pour les lots qui touchent la génération de code ou le
  runtime, il faudra installer LLVM sur ce PC (accord de l'utilisateur) ou faire
  vérifier par Claude Main sur son PC.

## Lots

| Lot | Contenu | Statut | Preuve |
|---|---|---|---|
| P2 — G0 | Audit des usages de la syntaxe 0.2 : [g0-audit.md](../adr/syntaxe/g0-audit.md), [recensement](../adr/syntaxe/g0-recensement.md), outil `compiler/nxc_syntax/examples/g0_census.rs` | `CONÇU` (étude livrée, PR à relire par Main) | recensement relançable ; deux pièges vérifiés avec `nxc check` |
| P2 — G1 | Trois propositions de style sur les 12 programmes de la passation | `À FAIRE` | après relecture de G0, avec Claude Main (un auteur par fichier) |
| P9 — contrat | `nxc lsp`, JSON versionné, texte non enregistré : [ADR 0009](../adr/0009-contrat-nxc-lsp.md), proposée | `CONÇU` (en attente de l'approbation de Main ; rien n'est implémenté) | constats C1 à C8 vérifiés sur `main` (§ Contexte de l'ADR) |
| P9 — implémentation | deux PR : fondations (chargeur, JSON v1, `check --stdin`), puis serveur | `À FAIRE` | après approbation |
| P5 | Typage renforcé, OOP, modèle mémoire et concurrence | `À FAIRE` | dépend de P2 |

## Demandes reçues

De **Claude Main** (`claude-status.md`, reprises de Codex Astral), non encore traitées :

1. `nxc lsp` : serveur LSP stdio reprenant `nxc ide` — **contrat proposé** :
   [ADR 0009](../adr/0009-contrat-nxc-lsp.md), §1 à §8 ; 6 questions à Main en fin
   d'ADR.
2. Version du schéma JSON de `nxc check --json` et `nxc ide` — **proposé** : ADR 0009, §9
   (`--json=1`, `--json` inchangé pendant la série 0.2).
3. Vérification du texte non enregistré — **proposé** : ADR 0009, §3 (tous les documents
   ouverts, dans `nxc lsp`) et §10 (`nxc check --stdin`).
4. Formateur : accolade fermante restée en fin de ligne après `fn main(){⏎…}`. Le G0
   constate que `format_source` travaille ligne par ligne sur l'indentation et non sur
   l'arbre (§6 de l'audit) : la correction de fond est un formateur sur l'AST (P6).
5. Runtime macOS : pas de compatibilité annoncée avant correctif et tests (rappel de
   l'audit Core).

## Demandes envoyées

Aucune pour l'instant. Envisagée après G1 : une source unique du lexique pour les outils
d'édition (recommandation 4 de l'audit), qui concernera Codex Luna (`studio/syntax`,
extension VS Code) et Claude Main (coloration de `studio/app`).

## Tests exécutés

- `cargo run -p nxc_syntax --example g0_census -- .` : 170 fichiers ou blocs, 80 346
  jetons ; mêmes nombres à chacune des 4 exécutions (les 2 dernières après correction du
  classement des `;` et de l'échappement des `|` dans le tableau).
- Sur ce PC, sans LLVM : `nxc check`, `nxc fmt --check` et `nxc ide symbols` réussissent
  sur `examples/bonjour.nx` ; `nxc build` s'arrête sur « LLVM was not found ».
- `cargo clippy -p nxc_syntax --examples` : aucun avertissement dans `g0_census.rs`. Le
  lexer et le parser ont des avertissements Clippy plus anciens, laissés tels quels (pas de
  modification du lexer ni du parser avant G0).
- Contrat P9 : sorties réelles de `nxc check --json` et de `nxc ide` (survol, complétion,
  définition, références, renommage, symboles, tests) relevées sur un projet à deux
  fichiers ; coût d'une requête mesuré en release (65 à 80 ms) ; renommage vers `let` et
  `async` accepté à tort (constat C7, à corriger dans l'implémentation).
- `nxc check` sur les programmes de l'annexe de l'audit : piège des fins de ligne accepté
  sans message (code 0) avec `-`, `not`, `move`, `(…)` et `[…]` en début de ligne ;
  variante mal orthographiée : avertissements W0007 et W0001, code 0.

## Problèmes rencontrés

- Pas de LLVM sur ce PC (voir Environnement).
