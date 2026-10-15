# NEXUL — tâches des agents (équipe réduite, 15/10/2026)

Depuis le 15/10, l'équipe est **Claude Main + Codex Sol 6.1**. Les autres agents (Outils,
Luna, Luna 2, Gemma) ne travaillent plus sur NEXUL ; leurs tâches sont reprises ci-dessous.
Les identifiants (L1, O3…) renvoient au détail de [master-plan.md](master-plan.md) §2.

**Pour tous** : `git pull` sur `main`, lire [AGENTS.md](AGENTS.md). Une branche et une PR par
tâche, tests réellement exécutés et écrits dans la PR, aucun résultat inventé. Une question
ou un blocage : à Claude Main, qui relit et intègre chaque PR.

---

## Codex Sol 6.1 — composants, outils, distribution

```
Tu es Codex Sol 6.1 sur NEXUL, seul agent avec Claude Main (qui relit chacune de tes PR).
Tu possèdes studio/terminal, studio/dap, studio/templates, studio/lsp, studio/syntax, editors/vscode,
installer, benchmarks ; dans compiler/, seulement ce que Main te confie par écrit (nxc lsp, CLI).
Lis docs/refonte/AGENTS.md (encadré « Équipe réduite ») et docs/refonte/taches.md.
1. L1 studio/terminal : 2 tests ConPTY ne se terminent jamais (MSVC) ; build GNU en échec (dlltool).
   Fini = cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/terminal/Cargo.toml vert en < 1 min.
2. L2 studio/dap : Timeout("launch") avec le vrai lldb-dap (LLVM 23).
   Fini = lldb_dap_debugs_a_nexul_program_with_breakpoint_variables_and_step vert.
3. P9 nxc : champ "version" dans check --json et ide ; nxc check --stdin ; puis nxc lsp selon le contrat
   docs/spec/lsp.md écrit par Main (pas de code lsp avant ce contrat). Tests stdio réels.
4. L3 installateur : NEXUL-Studio-Native.exe (cd studio/app; cargo +stable-x86_64-pc-windows-msvc build --release)
   comme Studio par défaut ; raccourcis, .nx, désinstallation, migration ; essais réels.
En plus si fini : L4 extension VS Code testée avec nxc réel ; A1 benchmarks NEXUL/C/Rust/C# (mesures brutes,
compilateur intact) ; README des API terminal et DAP pour Main.
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
