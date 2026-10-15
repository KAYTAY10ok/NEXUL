# NEXUL — tâches des agents (15/10/2026)

Version courte de [master-plan.md](master-plan.md) §2. Tenue par Claude Main.

**Pour tous** : `git pull` sur `main`, lire [AGENTS.md](AGENTS.md). Une branche et une PR par
tâche, tests réellement exécutés et écrits dans la PR, aucun résultat inventé. Mettre à jour
son fichier d'état. Une question ou un blocage : à Claude Main.

---

## Codex Luna 2 (ancien Astral) — terminal, débogueur, mesures

```
Tu es Codex Luna 2 (ex-Astral) sur NEXUL, dans C:\NEXUL-core (PC de Main : MSVC, LLVM 23, lldb-dap).
Tu possèdes studio/terminal, studio/dap, benchmarks. Lis docs/refonte/AGENTS.md et master-plan.md §2.4.
1. L1 studio/terminal : 2 tests ConPTY ne se terminent jamais (MSVC) ; build GNU en échec (dlltool).
   Fini = cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/terminal/Cargo.toml vert en < 1 min.
2. L2 studio/dap : Timeout("launch") avec le vrai lldb-dap.
   Fini = lldb_dap_debugs_a_nexul_program_with_breakpoint_variables_and_step vert.
3. A1 : benchmarks NEXUL/C/Rust/C# (1M/10M/100M, maths, allocations, chaînes), mesures brutes, sans toucher au compilateur.
En plus si fini : décrire l'API terminal et DAP pour Main (README) ; lancer cargo test --release --workspace
sur les PR Core (Outils n'a pas LLVM) et noter le résultat.
Branche refonte/luna2-…, PR [luna2], état docs/refonte/codex-luna2-status.md.
```

## Codex Luna — extension VS Code, installateur

```
Tu es Codex Luna sur NEXUL. Tu possèdes editors/vscode, installer, studio/templates, studio/lsp, studio/syntax
(plus le terminal ni le DAP : Luna 2). Lis docs/refonte/AGENTS.md et master-plan.md §2.3.
1. L4 extension VS Code : essais réels avec nxc (diagnostics, build, run, test, format), tests dans l'hôte VS Code.
   Pas de grammaire finale avant l'ADR de syntaxe.
2. L3 installateur : empaqueter NEXUL-Studio-Native.exe (cd studio/app; cargo +stable-x86_64-pc-windows-msvc build --release)
   comme Studio par défaut ; raccourcis, .nx, désinstallation, migration ; installer et désinstaller pour de vrai.
En plus si fini : README de ToolchainProvider (studio/templates) ; scénario F (VS Code, passation §15.2) joué et noté dans ton état.
Branche refonte/luna-…, PR [luna], état docs/refonte/codex-luna-status.md.
```

## Claude Outils / Core — langage et compilateur

```
Tu es Claude Opus 5.5 Outils/Core sur NEXUL. Tu possèdes compiler, lib, tests du langage, docs/spec, docs/adr.
L'installateur n'est plus à toi (Luna). Lis docs/refonte/AGENTS.md et master-plan.md §2.2.
1. O2 (G1) : 3 propositions de syntaxe sur les 12 programmes (passation §3.2), illustratives,
   dans docs/adr/syntaxe/g1-propositions.md. Fais relire chaque proposition par Claude Main.
2. O3 (P9) : contrat nxc lsp (capacités, version du JSON, texte non enregistré) approuvé par Main AVANT le code,
   puis implémentation et tests stdio.
3. O4 : formateur (accolade fermante laissée en fin de ligne) : décider, tester.
En plus si fini : O6 étude de la cible temps réel ; O8 inventaire du self-hosting (docs/adr/selfhost-sh0.md) ;
O7 runtime macOS. Génération de code non testable chez toi (pas de LLVM) : demande la vérification à Luna 2.
Branche refonte/core-…, PR [core], état docs/refonte/claude-core-status.md.
```

## Claude Main — direction et Studio

1. **M19** : essai manuel de P1 dans la vraie application (fermeture, conflit, récupération,
   changement de dossier), avec la liste de Gemma.
2. **M18** : relire les PR #14 à #17 (fusionnées sans ma revue), demander les corrections.
3. **M14, M15** : concevoir G1 avec Outils et relire chaque proposition ; approuver le contrat
   `nxc lsp`.
4. **M5** : LSP des autres langages dans Studio ; **M9** : paramètres et raccourcis.
5. **M6, M7** : vue terminal et débogueur dès que Luna 2 livre L1 et L2.

En plus si nécessaire : reste de M8 (explorateur de tests, erreurs cliquables), M11 (aperçu
Web), M13 (CI GitHub), M17 (extensions de Studio demandées par Luna).

---

**Pour l'utilisateur** : fusionner la PR `refonte/claude-studio`, puis coller chaque bloc
dans la session de l'agent. Gemma : consigne au master-plan §2.6 (scénarios E2E, Q7).
