# État Codex Astra — NEXUL Core

15 octobre 2026. Branche refonte/codex-core, worktree C:/NEXUL-core, base d537a7b. Livraison : [PR #1](https://github.com/KAYTAY10ok/NEXUL/pull/1), code au commit 99d1fc2 ; pas intégrée à main. Aucun contrôle CI publié sur GitHub au moment de la livraison : validation locale uniquement. Aucun fichier Studio/installateur modifié.

## Réalisé

- Audit initial du pipeline, de LLVM et des oracles numériques ; défaut macOS consigné dans audit-core.md, la spécification et la feuille de route (non corrigé).
- Fiabilisation des vérificateurs numériques et contrôle des statuts des exécutables, commit a9a2084 ; sept tests négatifs/positifs du vérificateur.
- Cible wasm32-web (alias wasm32-unknown-unknown), cfg web, runtime NEXUL avec imports hôte explicites sans WASI ; mémoire et buffers ABI v1 exportés automatiquement.
- Allocateur de pages Wasm partagé avec WASI, sans changement de sa stratégie.
- Chargeur lib/web/nexul.mjs : imports applicatifs, UTF-8, allocations suivies, renouvellement des vues mémoire, refus de réentrance et abandon de l'instance après trap/panique/exception hôte.
- Exemple navigateur examples/web et serveur loopback avec MIME .mjs portable.
- Contrat interop-web.md révisé avec commandes testées, règles d'ownership et limites.
- Test IO devenu déterministe : variable NEXUL_TEST_ENV injectée au lieu de dépendre de PATH/Path sous Windows.

## Validation

Windows, Rust 1.99.0, Python 3.14.7, Node 24.19.0, LLVM de C:/Program Files/LLVM ; wasm-ld 23.1.3 (0d261d1).

- NEXUL_TEST_NUMERICS=1 et NEXUL_TEST_TARGETS=x86_64-linux,wasm32-wasi ; cargo test --release -p nxc -p nxc_syntax -p nxc_sema -p nxc_nir -p nxc_codegen_llvm : 22 tests réussis, dont la suite native, les comparaisons Python et la conformité croisée Linux/WSL et WASI.
- cargo test --release -p nxc --test web -- --ignored --nocapture : réussi en debug et release (6 modules compilés). Ce test est explicitement ignoré par défaut car il exige Node/LLVM.
- cargo test --release --workspace avec les mêmes variables : 24 tests réussis (dont 2 tests installateur), test web ignoré par défaut puis exécuté explicitement ci-dessus.
- python -I tests/numerics/test_verifiers.py : 7 tests réussis.
- Exemple compilé par la CLI et exécuté dans Chromium (navigateur Codex) : 42 et Bonjour, NEXUL 🌍 — été !.
- nxc run --target wasm32-web : refus explicite attendu, avec orientation vers build et le chargeur.

Le test web couvre appels hôte, imports absents, Unicode/NUL/grandes chaînes, croissance/réutilisation mémoire, doubles libérations, bornes, f32/f64 (NaN/infinis/zéro signé/sous-normaux), i64 BigInt, horloge/aléa injectés, absence de fichiers, débordement, panique, trap, exceptions hôte et réentrance. Ce n'est pas une certification exhaustive de l'IEEE ou des navigateurs.

## Fichiers modifiés dans le lot web

compiler/nxc/{src/{lib.rs,link.rs,main.rs},tests/{suite.rs,web.rs}} ; compiler/nxc_codegen_llvm/src/lib.rs ; compiler/nxc_sema/src/{load.rs,target.rs} ; lib/std/{web.nx,wasm.nx,sys/{web.nx,wasi.nx}} ; lib/web/nexul.mjs ; examples/web/* ; tests/web/* ; tests/run/io.{nx,out} ; tests/numerics/README.md (encodage réparé) ; docs/refonte/{codex-status.md,interop-web.md,audit-core.md} ; docs/spec/{06-modules.md,10-compilateur.md} ; docs/roadmap.md.

## Limites et travail restant

Benchmarks comparatifs C/Rust/C#/NEXUL, inspection du code machine et optimisations mesurées non réalisés. Pas de promesse de gain. LSP stdio et versionnement JSON restent à implémenter. Aucune modification des garanties numériques. macOS non opérationnel avec la couche système actuelle. Pas de DOM/async/réseau implicite, pas de fichier ni stdin dans la cible web ; sommeil bloquant non nul refusé. Pas de validation Firefox/Safari.

## Décisions et demandes

- Laisser studio/app et les crates de services agents dans leurs workspaces séparés pour cette phase, conformément aux consignes. Aucune dépendance GUI à intégrer au workspace racine dans ce lot.
- Claude Studio : après intégration du lot, utiliser interop-web.md pour détecter wasm32-web, analyser, compiler puis afficher l'exemple dans l'aperçu web isolé. Critères : erreur de compilation visible, 42 et chaîne Unicode affichés, imports explicites et aucune tentative de réutilisation après panique.
- Codex Luna : après intégration, modèle NEXUL+Web basé sur examples/web, chargeur inclus, chemins relatifs conservés, compilation et exécution vérifiées. Demande commune publiée dans [issue #2](https://github.com/KAYTAY10ok/NEXUL/issues/2), liée à la PR.
- L'interface nxc ide et les JSON existants restent inchangés. Les aides ABI web sont versionnées par nx_abi_version() == 1 et le module d'import nexul_v1.
