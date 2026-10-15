# NEXUL — plan maître de la refonte

Tenu par **Claude Main** (voir [AGENTS.md](AGENTS.md)). Source des exigences :
[passation-v3.md](passation-v3.md). Statuts : `CONÇU`, `IMPLÉMENTÉ`, `TESTÉ_UNITAIREMENT`,
`INTÉGRÉ`, `VALIDÉ_E2E`, `LIVRÉ`, `BLOQUÉ`, ou `À FAIRE`. Un statut n'avance qu'avec une
preuve (commande, test, PR) ; les résultats déclarés par un agent et non revérifiés par Main
sont marqués « déclaré ».

Dernière mise à jour : 15 octobre 2026, `main` à `548b1f6`.

## 1. Lots

| Lot | Urgence | Travail et preuve de sortie | Propriétaire | Dépend de | Statut |
|---|---|---|---|---|---|
| P0 | Immédiat | Gouvernance V3 : charte, transferts, plan maître | Main | accord utilisateur (obtenu le 15/10) | IMPLÉMENTÉ (PR à fusionner) |
| P1 | Immédiat | Studio sans perte de données : Enregistrer / Ne pas enregistrer / Annuler (onglet, fenêtre), conflits disque, enregistrement atomique, récupération, tests E2E | Main | — | TESTÉ_UNITAIREMENT (`43ebbe5`, 25 tests) ; essai manuel à faire |
| P2 | Immédiat | Nouvelle syntaxe : G0 audit des usages 0.2, G1 trois propositions sur 12 programmes, G2 ADR + EBNF validées par l'utilisateur, plan de migration | Main + Outils (un auteur par fichier) | décision utilisateur | À FAIRE |
| P3 | Élevée | Raccorder `syntax`, `lsp`, `terminal`, `dap`, `templates` dans Studio ; UI native stable | Main | crates fusionnées | À FAIRE |
| P4 | Élevée | Installateur Windows du Studio natif testé avec un vrai binaire | Luna | transfert installateur, P3 minimal | À FAIRE |
| P5 | Élevée | Conception typage renforcé, OOP, modèle mémoire et concurrence ; tests d'acceptation | Outils, revue Main | P2 | À FAIRE |
| P6 | Élevée | Nouveau lexer/parser, types, formatage, diagnostics ; migration des sources 0.2 | Outils | ADR P2 | À FAIRE |
| P7 | Élevée | Concurrence sûre (compilateur + runtime), canaux, tests négatifs et de stress | Outils | P5, P6 | À FAIRE |
| P8 | Élevée | Profil temps réel v1 sur une cible définie, budgets et mesures | Outils | P7, choix de cible validé | À FAIRE |
| P9 | Élevée | `nxc lsp`, JSON versionné, texte non enregistré | Outils | contrat approuvé par Main | À FAIRE |
| P10 | Élevée | Extension VS Code : VSIX installable, commandes `nxc`, LSP, format, tests | Luna, revue Main | P2 (coloration finale), P9 (LSP) | À FAIRE |
| P11 | Élevée | Web dans Studio : modèle NEXUL + Web, aperçu isolé `wasm32-web` | Luna (modèle), Main (UI) | ABI v1 (fusionnée) | À FAIRE |
| P12 | Majeur | Auto-hébergement stage0 → stage1 → stage2, parité, reproductibilité | Outils, revue Main | noyau stable | À FAIRE |
| P13 | Majeur | Benchmarks NEXUL/C/Rust/C#, précision, optimisations, multi-cibles | Outils ; Astral sur mandat | base stable | À FAIRE |
| P14 | Bloquant pour « fini » | Scénarios E2E A à H (passation §15.2), documentation exacte, version stable, migration, retour arrière | Main + chaque propriétaire | tous les lots | À FAIRE |

En parallèle dès maintenant (passation §13) : Main fait P1 puis P3 ; Outils prépare P2 (G0)
et P9 ; Luna l'ossature de P10 et le modèle de P11 ; Astral rien sans mandat ; Gemma la
matrice des preuves et les écarts code/documentation.

## 2. Composants existants (audit du 15/10)

| Composant | Où | Propriétaire | Statut | Preuve / manque |
|---|---|---|---|---|
| Compilateur NEXUL 0.2 (`nxc`, `nxpm`, `nxc ide`) | `compiler/` | Outils | LIVRÉ (0.2.0) | release v0.2.0 ; refonte du langage à faire (P2, P5–P9, P12) |
| WebAssembly navigateur (`wasm32-web`, ABI v1, chargeur) | `compiler/`, `lib/web/`, `examples/web/` | Outils | TESTÉ_UNITAIREMENT (déclaré) | PR #1 ; intégration Studio et modèle à faire (P11) |
| Studio natif | `studio/app` | Main | INTÉGRÉ | 21 tests fonctionnels (Main) ; fermeture sans demande (P1), crates non raccordées (P3) |
| Services Studio (`nexul_studio`) | `studio/src` | Main | TESTÉ_UNITAIREMENT | 8 tests |
| Ancienne interface web (Edge) | `studio/src`, `studio/ui` | Main | LIVRÉ (0.2.0) | à retirer à la parité seulement |
| Coloration NEXUL | `studio/syntax` | Luna (maintenance) | TESTÉ_UNITAIREMENT | voir §3 ; à raccorder (P3) |
| Client LSP + détection SDK | `studio/lsp` | Luna (maintenance) | TESTÉ_UNITAIREMENT | voir §3 ; à raccorder (P3) |
| Terminal PTY + émulation | `studio/terminal` | Luna | IMPLÉMENTÉ | 2 essais PTY bloqués (§3) ; vue GPUI à faire (P3) |
| Client DAP + adaptateurs | `studio/dap` | Luna | IMPLÉMENTÉ | essai réel `lldb-dap` en échec (§3) ; UI à faire (P3) |
| Modèles de projets | `studio/templates` | Luna | TESTÉ_UNITAIREMENT | voir §3 ; assistant GPUI (P3) |
| Installateur Windows | `installer/` | Outils → Luna | LIVRÉ (0.2.0) | empaquette encore l'ancien Studio (P4) ; branches installateur d'Outils non fusionnées |
| Extension VS Code | `editors/vscode/` | Luna | À FAIRE | P10 |
| Auto-hébergement | — | Outils | À FAIRE | P12 |

## 3. Vérification par Main des crates de Studio

Commande : `cargo test --manifest-path studio/<crate>/Cargo.toml` (chaîne GNU), sur
`main` à `548b1f6`, PC de Main.

| Crate | Résultat |
|---|---|
| `studio/syntax` | 26 tests réussis |
| `studio/lsp` | 32 tests réussis |
| `studio/terminal` | **ne se construit pas avec la chaîne GNU** (`windows-sys` : `dlltool` introuvable) ; avec MSVC : 3 tests réussis, **2 bloqués** sans fin (`pty_resize_and_stop_are_reported`, `pty_runs_interactive_program_and_exits`, arrêtés après 60 s chacun) |
| `studio/dap` | 4 tests réussis ; **échec** de `lldb_dap_debugs_a_nexul_program_with_breakpoint_variables_and_step` : `Timeout("launch")` avec le vrai `lldb-dap` (LLVM 23) |
| `studio/templates` | 6 tests réussis |

Statuts révisés en conséquence : terminal et DAP restent `IMPLÉMENTÉ` tant que ces essais
réels ne passent pas sur Windows (demandes à Luna dans
[claude-main-status.md](claude-main-status.md)).

## 4. Décisions à faire valider par l'utilisateur (passation §16)

| # | Décision | Recommandation de Main | Statut |
|---|---|---|---|
| 1 | Gouvernance V3 et transferts | appliquer | **validée le 15/10** |
| 2 | Édition du langage : nouvelle syntaxe incompatible ou édition opt-in avec migrateur | version majeure + outil de migration, pas de casse silencieuse | à décider (après G1) |
| 3 | Première cible temps réel | à proposer après étude (Cortex-M nu, RTOS ou Linux PREEMPT_RT) | à décider |
| 4 | Niveau des contrats (partout ou profil strict) | à proposer avec P5 | à décider |
| 5 | Concurrence par cible | natif d'abord | à décider |
| 6 | Périmètre initial de l'auto-hébergement | à proposer avec P12 | à décider |
| 7 | Extension VS Code : nom, éditeur, publication Marketplace | publication seulement sur action explicite de l'utilisateur | à décider |
| 8 | CI de confiance (Windows, Linux, cibles embarquées) | GitHub Actions Windows + Linux pour commencer | à décider |
