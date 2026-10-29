# État des travaux de Codex Terra 6.1

Agent : **Codex Terra 6.1**. Périmètre et tâches : [prompt-codex-terra.md](prompt-codex-terra.md)
(T1 à T5). Seul Codex Terra modifie ce document ; Claude Main y répond dans
[claude-main-status.md](claude-main-status.md).

Créé par Claude Main le 29/10/2026 ; à remplir par Codex Terra.

## En cours

À remplir **avant** d'écrire du code : tâche, branche, fichiers touchés (et, pour un fichier
partagé, les sections).

| Tâche | Branche | Fichiers | Depuis |
|---|---|---|---|
| T1 — campagne de performance et profils | `refonte/terra-performance`, `refonte/terra-profil` | `benchmarks/**` ; PR #72 et #74 | 29/10/2026 |
| T2 — UDP IPv4 | `refonte/terra-udp` | `lib/std/net.nx`, sections réseau système, nouveau test UDP | 29/10/2026 |
| T3 — ADR proposée | `refonte/terra-incremental` | `docs/adr/0016-compilation-incrementale.md` ; PR #73 | 29/10/2026 |
| T4 SH2 — oracle JSON et lexer NEXUL | `refonte/terra-sh2` | `compiler/nxc/src/main.rs` : commande cachée `debug tokens --json` seulement ; `selfhost/lexer/**`, fichier d'état, proposition de CI | 29/10/2026 |

## Tâches

| Tâche | Statut | PR | Preuves |
|---|---|---|---|
| T1 Campagne de performance | CONÇU | | |
| T2 `std.net` complet (UDP, délais, IPv6, noms) | CONÇU | | |
| T3 Compilation incrémentale (ADR 0016 d'abord) | CONÇU | | |
| T4 Auto-hébergement SH2 (lexer), SH3 (parser, après G3) | CONÇU | | |
| T5 Outils et distribution | CONÇU | | |

## Demandes à Claude Main

—

## Manques de NEXUL rencontrés (pour Main)

—

Le plan SH0 antérieur mentionne le gel G3 pour SH2 ; le mandat du 29/10 demande SH2
maintenant et conditionne explicitement SH3 à G3. Le lexer SH2 doit donc rester adaptable
aux décisions de grammaire ; SH3 attend G3.
