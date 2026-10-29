# État des travaux de Codex Terra 6.1

Agent : **Codex Terra 6.1**. Périmètre : prompt de démarrage T1 à T5 ; Claude Main répond dans son fichier d'état.

## En cours

| Tâche | Branche | Fichiers | Depuis |
|---|---|---|---|
| T1 — banc Linux, mesures initiales ; profilage et gains à poursuivre | `refonte/terra-performance` | `benchmarks/**`, `docs/refonte/codex-terra-status.md` ; proposition de job CI `.github/workflows/terra-benchmarks.yml` pour revue de Main | 29/10/2026 |

## Tâches

| Tâche | Statut | PR | Preuves |
|---|---|---|---|
| T1 Campagne de performance | TESTÉ_UNITAIREMENT | À proposer | Douze triplets compilés et exécutés sept fois par langage ; sorties identiques, y compris avec `NEXUL_LEAK_CHECK=1`. [Mesures exploratoires](../../benchmarks/rapports/2026-10-29-linux.md). Profilage, gains et CI distante non testés. |
| T2 `std.net` complet | CONÇU | | |
| T3 Compilation incrémentale | CONÇU | | ADR préalable. |
| T4 Auto-hébergement SH2 et SH3 | CONÇU | | SH3 après G3. |
| T5 Outils et distribution | CONÇU | | |

## Demandes à Claude Main

- Revoir le workflow de mesures non bloquant proposé dans `.github/workflows/terra-benchmarks.yml` et les limites du protocole. Les écarts observés ne sont pas encore expliqués ni proposés pour acceptation.

## Manques de NEXUL rencontrés

—
