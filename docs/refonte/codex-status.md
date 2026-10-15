# État Codex — NEXUL Core

Mise à jour : 15 octobre 2026. Branche refonte/codex-core, worktree C:/NEXUL-core, base d537a7b. Aucun changement dans studio/ ou installer/.

## Réalisé

- Lecture du pipeline LLVM, du choix des cibles et de la suite numérique ; [audit initial](audit-core.md).
- Correction des faux succès possibles dans les vérificateurs numériques et contrôle du statut des exécutables.
- Sept tests de régression des vérificateurs.
- [Contrat WebAssembly initial](interop-web.md) distinguant les commandes existantes des propositions navigateur.

## Validation effectuée

Windows ; rustc 1.99.0 (b940084d7, 2026-09-28), Python 3.14.7.

- NEXUL_TEST_NUMERICS=1 cargo test --release -p nxc -p nxc_syntax -p nxc_sema -p nxc_nir -p nxc_codegen_llvm : succès (22 tests Rust, dont 8 tests d'intégration). Le test cross est inactif faute de NEXUL_TEST_TARGETS ; aucun résultat cross revendiqué.
- Après modification du contrôle des statuts : NEXUL_TEST_NUMERICS=1 cargo test --release -p nxc --test suite numerics_against_python : succès.
- python -I tests/numerics/test_verifiers.py : 7 tests réussis.

## Fichiers modifiés

compiler/nxc/tests/suite.rs ; tests/numerics/{format_verify.py,math_verify.py,parse_compare.py,test_verifiers.py,README.md} ; docs/refonte/{audit-core.md,interop-web.md,codex-status.md}.

## En cours et prochaines étapes

L'audit global n'est pas terminé. Benchmarks comparatifs, inspection du code machine et cible navigateur restent à réaliser. Aucune optimisation ni promesse de performance dans ce lot. Compléter la couverture IEEE et f32. Le mapping macOS vers le runtime système Linux est un défaut confirmé du code, encore non corrigé.

## Coordination et interfaces

Demandes Claude lues depuis son worktree : LSP stdio, versionnement JSON et contrat web. LSP et versionnement restent à implémenter. Aucune interface publique existante modifiée. Aucune dépendance GUI intégrée. Claude peut continuer avec nxc ide ; le document interop-web ne constitue pas encore un feu vert pour une intégration navigateur.
