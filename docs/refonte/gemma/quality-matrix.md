# Matrice QA NEXUL

Date : 15 octobre 2026
Révision de référence : main à 548b1f6
Nature : audit de l’arbre et des rapports. Cette matrice ne certifie pas une version.

## Lire les statuts

- **Code présent** : le composant existe dans l’arbre inspecté.
- **Tests présents** : les fichiers de test existent ; cela ne prouve pas qu’ils passent.
- **Rapport propriétaire** : un fichier d’état décrit un essai, non rejoué par Gemma.
- **Non raccordé** : aucune intégration correspondante n’a été trouvée dans le manifeste de Studio natif.
- **À valider** : dépend d’un essai sur machine ou d’une version intégrée.

| Domaine | Éléments repérés | Tests et preuves | État QA / propriétaire actuel |
|---|---|---|---|
| Compilateur et langage | compiler, lib, examples ; suite dans compiler/nxc/tests/suite.rs | tests/run et sorties attendues, tests/ui, suites numériques et cibles | Code et corpus présents. codex-status.md rapporte des suites réussies sur Windows, non rejouées ici. Propriétaire du cœur selon AGENTS.md. |
| WebAssembly navigateur | examples/web, lib/web/nexul.mjs, docs/refonte/interop-web.md | tests/web/interop.mjs ; compiler/nxc/tests/web.rs indique que le test est ignoré par défaut | Le rapport Core décrit un essai Chromium et une exécution explicite. Non revalidé par Gemma. |
| Studio natif | studio/app : éditeur, onglets, explorateur, services NEXUL | tests intégrés dans les modules Rust | Le manifeste dépend de nexul-studio et GPUI, mais pas des crates syntax, lsp, terminal, dap ou templates. Fermeture de fenêtre : voir handoffs.md. Propriétaire selon AGENTS.md : Studio. |
| Coloration NEXUL | studio/syntax, analyse incrémentale | studio/syntax/tests/corpus.rs, studio/syntax/tests/conformance.rs et tests unitaires | Tests source présents ; le README décrit corpus et mesures. Crate autonome, pas référencée dans le manifeste app inspecté. Outils. |
| Client LSP | studio/lsp, client et détection | faux serveur, studio/lsp/tests/client.rs, studio/lsp/tests/detect.rs, studio/lsp/tests/real_servers.rs | Le README rapporte seulement rust-analyzer essayé en vrai. Crate non raccordée au manifeste GPUI inspecté. Outils. |
| Terminal | studio/terminal, PTY et modèle d’écran | cinq tests unitaires dans studio/terminal/src/lib.rs | codex-exec-status.md rapporte des tests Linux et un check Windows ; pas d’exécution ConPTY sur cet hôte. Non raccordé à l’app inspectée. Luna. |
| DAP | studio/dap et faux adaptateur | studio/dap/tests/client.rs, studio/dap/tests/nexul_lldb.rs | Le rapport Luna indique que l’essai faux adaptateur passe ; nxc/LLDB a été ignoré faute d’outils. Non raccordé à l’app inspectée. Luna. |
| Modèles de projets | studio/templates | studio/templates/tests/functional.rs conditionne les builds sur les outils détectés | Le README de cette révision indique que le modèle wasm est encore absent. Ne pas confondre une branche avec une fonctionnalité livrée sur main. Luna. |
| Installateur Windows | installer et tools/build_installer.py | tests Rust ; pas de preuve d’installation de la nouvelle app dans cette session | Le script copie target/NEXUL-Studio.exe, tandis que le manifeste de studio/app produit NEXUL-Studio-Native. Le problème est suivi dans l’issue 9. Aucun essai sur machine propre. Propriétaire selon AGENTS.md : Outils. |
| Extension VS Code | editors/vscode absent de cette révision | aucun VSIX ni test d’extension repéré | Absente de main 548b1f6. Le guide livré ici est un brouillon non installable. La passation V3 proposée attribue ce lot à Luna. |
| Benchmarks comparatifs | mesure GPUI/Slint dans docs/studio/architecture-native.md | aucun corpus comparatif reproductible NEXUL/C/Rust/C# repéré dans les chemins inspectés | La mesure de boîtes à outils n’est pas un benchmark de compilation. À établir par les propriétaires du cœur. |

## Prochaine validation utile

Sur une machine équipée, exécuter les tests de chaque crate puis une session E2E avec un Studio construit et empaqueté. Pour chaque essai, enregistrer révision, OS, versions d’outils, commande, résultat et logs.

Pour l’application native, utiliser une configuration isolée via NEXUL_STUDIO_CONFIG. Pour l’installateur, utiliser une VM jetable et vérifier installation, lancement, réparation, mise à niveau et désinstallation.
