# C6 — proposition de CI Windows

Statut : IMPLÉMENTÉ après accord utilisateur le 24/10/2026 ; validation Actions en cours.

Le mandat C6 dit : « C'est un fichier transversal : demandez l'accord de l'utilisateur. »
Le patch est maintenant appliqué au workflow dans cette PR. Le YAML exact est dans
`sol-c6-rust.yml` et le diff applicable dans `sol-c6-windows-ci.patch`.

- Windows installe LLVM 20.1.8, archive complète officielle (bibliothèque C incluse,
  environ 940 Mo), téléchargée directement, vérifiée par SHA-256 et extraite avec le tar Windows.
- Le précontrôle exige LLVM-C.dll, lld-link.exe et wasm-ld.exe ; il échoue si un outil
  manque. NEXUL_LLVM désigne explicitement cette chaîne.
- Windows lance `cargo test --release -p nxc` : diagnostics, robustesse, migration,
  exécution normale et optimisée ; les tests existants de l'installateur et de Studio restent.
- Linux conserve son workspace sans l'installateur Windows et passe ses tests en release.
- C7 reste indépendant : conserver ses déclencheurs lors de l'application du patch.

Validation locale : YAML analysé, commandes des deux jobs vérifiées, `git diff --check`
réussi. Validation Windows et exécution Actions : non testées. Aucun test désactivé.

La version et le nom de l'archive sont vérifiés dans l'API officielle de la release
llvmorg-20.1.8 ; SHA-256 publié :
`f229769f11d6a6edc8ada599c0cda964b7dee6ab1a08c6cf9dd7f513e85b107f`.

Après intégration : intégrer C2 puis
exécuter stack_overflow sous Windows. C8 doit aussi passer 20 fois sur Windows avant
sa sortie de brouillon ; ce résultat n'est pas acquis par la seule installation d'LLVM.

La relecture a corrigé le mécanisme initial : install-llvm-action v2.0.9 appelle
7z une seule fois sous Windows, ce qui ne déploie pas les deux couches tar.xz.
L'extraction native directe évite cette erreur ; le précontrôle interdit une réussite
sans bibliothèque C ou linker. Les archives ne sont pas considérées installées
sur la seule réussite de leur téléchargement.
