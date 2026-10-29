# NEXUL — tâches des agents

> **Depuis le 29/10/2026 : Claude Main + Codex Terra 6.1.** Les tâches de Codex Terra (T1 à
> T5 : performance, `std.net`, compilation incrémentale, auto-hébergement SH2–SH3, outils)
> sont dans [prompt-codex-terra.md](prompt-codex-terra.md) ; celles de Claude Main dans
> [claude-main-status.md](claude-main-status.md), section « En cours ». La suite de ce
> fichier est l'archive de la période Claude Main + Codex Sol 6.1 (15/10 au 28/10).

## Archive : équipe Claude Main + Codex Sol 6.1 (15/10/2026)

Depuis le 15/10, l'équipe est **Claude Main + Codex Sol 6.1**. Les autres agents (Outils,
Luna, Luna 2, Gemma) ne travaillent plus sur NEXUL ; leurs tâches sont reprises ci-dessous.
Les identifiants (L1, O3…) renvoient au détail de [master-plan.md](master-plan.md) §2.

**Pour tous** : `git pull` sur `main`, lire [AGENTS.md](AGENTS.md). Une branche et une PR par
tâche, tests réellement exécutés et écrits dans la PR, aucun résultat inventé. Une question
ou un blocage : à Claude Main, qui relit et intègre chaque PR.

---

## Codex Sol 6.1 — composants, outils, distribution (mis à jour le 16/10, objectif 0.3.0)

```
Tu es Codex Sol 6.1 sur NEXUL, seul agent avec Claude Main (qui relit chacune de tes PR).
Lis docs/refonte/AGENTS.md (encadré « Équipe réduite »), docs/refonte/taches.md et
docs/refonte/master-plan.md §0.3.0. Objectif : la version 0.3.0 (édition 1, Studio natif,
nxc lsp, extension VS Code, installateur). Publication (Marketplace, GitHub Release) :
seulement par l'utilisateur lui-même ; tu prépares et tu testes.
0. Marketplace (en cours) : VSIX 0.3.0 prêt, README/CHANGELOG/icône/licence, `vsce ls`
   propre, essai d'installation du VSIX dans un VS Code isolé. Pas de publication.
1. Retours de revue de Main sur les PR #20 (terminal), #21 (DAP), #25 (installateur + CI) :
   docs/refonte/revues-main.md, section du 16/10. Corriger sur les mêmes branches ; Main fusionne.
2. nxc lsp et éditions : Main a rendu l'édition propre à chaque fichier (en-tête, sinon
   nexul.toml de son paquet, y compris les dépendances ; BOM accepté) et l'a branchée dans
   lsp.rs (diagnostics JSON v1). Ajoute les tests LSP : projet d'édition 1 important un
   module `// edition: 0.2`, dépendance 0.2 depuis un projet 1, en-tête ajouté/retiré en
   cours d'édition (didChange) → diagnostics justes. Mesures de §8 de l'ADR 0009 en release.
3. Formatage de l'édition 1 dans VS Code : `nxc fmt` ne réindente plus les bras `.Variante`
   (test `format_keeps_variant_arms…`). Lève le blocage de l'extension si les essais
   passent ; sinon, liste les cas restants dans ton fichier d'état.
4. Paquet 0.3.0 : installateur 0.3.0 avec nxc/nxpm 0.3.0, Studio natif
   (cd studio/app; cargo +stable-x86_64-pc-windows-msvc build --release), VSIX optionnel ;
   installation neuve, mise à niveau depuis 0.2 (réglages et projets gardés),
   désinstallation, dans un bac à sable ; preuves dans la PR.
5. CI (avec #25) : jobs Windows MSVC pour studio/terminal, studio/dap, studio/syntax,
   studio/lsp, studio/templates et la construction de studio/app ; le job Linux ne
   construit pas l'installateur Windows.
En plus si fini : A1 benchmarks NEXUL/C/Rust/C# (mesures brutes, compilateur intact) ;
L4 essais de l'extension sur Linux réel ; API README terminal/DAP à jour pour M6/M7.
Branches refonte/sol-…, PR [sol], état docs/refonte/codex-sol-status.md.
```
## Claude Main — direction, langage, Studio

1. **Langage (repris d'Outils)** : G1, trois propositions de syntaxe sur les 12 programmes,
   présentées à l'utilisateur ; puis ADR et EBNF (G2) ; contrat `docs/spec/lsp.md` pour
   Codex Sol (avant sa tâche 3) ; conception typage, objets, concurrence, temps réel.
2. **M19** : essai manuel de P1 dans la vraie application.
3. **M18** : relire les PR #14 à #17.
4. **M5, M9** : LSP des autres langages dans Studio ; paramètres et raccourcis.
5. **M6, M7** : vue terminal et débogueur dès que Codex Sol livre L1 et L2.
6. Revue et intégration de chaque PR de Codex Sol.

En plus si nécessaire : reste de M8 (explorateur de tests, erreurs cliquables), M11 (aperçu
Web), M13 (CI GitHub), M17 (extensions de Studio), scénarios E2E (P14, repris de Gemma).

---

**Pour l'utilisateur** : fusionner la PR `refonte/claude-studio`, puis coller le bloc de
Codex Sol dans sa session.
