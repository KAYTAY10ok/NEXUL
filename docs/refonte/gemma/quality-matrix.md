# Matrice QA NEXUL

Date : 16 octobre 2026 (mise à jour par Claude Main ; première version de Gemma le 15/10 sur `548b1f6`)
Révision de référence : `main` à `a0ef361`
Nature : état des preuves. Cette matrice ne certifie pas une version.

## Lire les statuts

- **Code présent** : le composant existe dans l’arbre inspecté.
- **Tests présents** : les fichiers de test existent ; cela ne prouve pas qu’ils passent.
- **Rejoué le 16/10** : suite exécutée par Main sur son PC (Windows 11, MSVC, LLVM 23) au commit indiqué.
- **Rapport propriétaire** : un fichier d’état décrit un essai, non rejoué ici.
- **Non raccordé** : la crate n’est pas encore utilisée par Studio natif (`studio/app`).
- **À valider** : dépend d’un essai sur machine ou d’une version intégrée.

| Domaine | Éléments | Tests et preuves | État QA / propriétaire actuel |
|---|---|---|---|
| Compilateur et langage | `compiler`, `lib`, `examples` ; suite `compiler/nxc/tests/suite.rs` | `cargo test --release --workspace` vert au 16/10 (`a0ef361`) : `tests/run`, `tests/ui`, numérique, cibles, catalogue des diagnostics | Main. Édition 1 de la syntaxe (ADR 0010) : tranches 1 à 3 intégrées, revue adversariale en cours. |
| Édition 1 et `nxc migrate` | `nxc_syntax/src/parser/ed1.rs`, `nxc/src/migrate.rs`, `pkg::edition_for` | `tests/run/edition1.nx`, 4 tests UI ; 33 programmes 0.2 migrés à sortie identique ; `lib/` (21 fichiers) et exemples migrés sans reprise | Main. Restent : repli `else`, `===`, `in`, `distinct`, `where`, `expects panic` (refusés avec `E0111`). |
| WebAssembly navigateur | `examples/web`, `lib/web/nexul.mjs`, `docs/refonte/interop-web.md` | `tests/web/interop.mjs` ; `compiler/nxc/tests/web.rs` ignoré par défaut | Rapport Core (essai Chromium), non rejoué. Main. |
| Studio natif | `studio/app` : éditeur, onglets, explorateur, services NEXUL, palette, recherche, exécution/construction/tests, nouveau projet | 33 tests (`cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1`) ; **essai manuel P1 passé le 16/10** ([liste](manual-studio-checklist.md)) | Main. Binaire autonome (CRT statique, `dumpbin` vérifié). |
| Coloration NEXUL | `studio/syntax` (0.2 et édition 1, langage `nexul1`) | tests unitaires, `corpus.rs`, `conformance.rs` (lexer du compilateur dans les deux éditions) ; raccordée à Studio (M3) | Codex Sol (crate), Main (adaptateur `highlight.rs`). |
| Client LSP | `studio/lsp`, client et détection | faux serveur, `client.rs`, `detect.rs`, `real_servers.rs` | Codex Sol. Catalogue des outils utilisé par l’assistant Nouveau projet ; client non raccordé (M5). `nxc lsp` : ADR 0009, à faire (P9). |
| Terminal | `studio/terminal`, PTY et modèle d’écran | tests unitaires ; 2 tests ConPTY ne se terminent pas sous MSVC | Codex Sol (L1). Non raccordé (M6). |
| DAP | `studio/dap` et faux adaptateur | `client.rs`, `nexul_lldb.rs` : `Timeout("launch")` avec le vrai `lldb-dap` | Codex Sol (L2). Non raccordé (M7). |
| Modèles de projets | `studio/templates` | `functional.rs` (builds selon les outils trouvés) ; raccordé à Studio (M4), création vérifiée à l’essai manuel | Codex Sol. |
| Installateur Windows | `installer`, `tools/build_installer.py` | tests Rust ; assistant Win32 (PR #14) | Codex Sol (L3 : empaqueter `NEXUL-Studio-Native.exe`). Correctif de sécurité demandé (`revues-main.md`). Aucun essai sur machine propre. |
| Extension VS Code | `editors/vscode` (PR #16) | 6 tests, VSIX construit | Codex Sol. Correctif de sécurité demandé (`nexul.nxcPath`) ; essai avec `nxc` réel et édition 1 à faire. |
| Benchmarks comparatifs | — | aucun corpus NEXUL/C/Rust/C# | Codex Sol (A1), à faire. |

## Prochaine validation utile

- Rejouer les suites des crates de Codex Sol après L1 et L2, puis raccorder terminal et débogueur.
- Scénario A (P14) : installateur → Studio natif → projet → erreurs → construire → exécuter → fermer et rouvrir sans perte, dans une VM jetable.
- Pour l’application native, toujours une configuration isolée (`NEXUL_STUDIO_CONFIG`) et un `nxc` connu (`NEXUL_NXC`).
