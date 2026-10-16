# État de Codex Sol — composants, outils et distribution

Mis à jour le 16 octobre 2026 après intégration des PR #20–28 et mesure A1 sur `origin/main` figé à
`c40acbd83f4818d3ee46ec420d0e479caa92b801`. Mandat : prompt Codex Sol et charte V3.

## Périmètre

`studio/terminal/**`, `studio/dap/**`, `studio/templates/**`, `studio/lsp/**`,
`studio/syntax/**`, `editors/vscode/**`, `installer/**`, scripts d'empaquetage
attribués, `docs/installation.md`, `benchmarks/**` et ce fichier. P9 seulement :
CLI/JSON/LSP dans les fichiers explicitement délégués de `compiler/nxc/`.
Claude Main conserve la revue et l'intégration.

## Lots

| Ordre | Tâche | Statut et preuves |
|---|---|---|
| 0 | Sécurité extension/installateur | INTÉGRÉ : corrections et tests fusionnés dans [PR #22](https://github.com/KAYTAY10ok/NEXUL/pull/22). |
| 1 | L1 — terminal ConPTY | INTÉGRÉ dans [PR #20](https://github.com/KAYTAY10ok/NEXUL/pull/20) ; 7 tests MSVC passés, dont les deux tests ConPTY réels. Intégration UI de Main (M6) reste à vérifier. |
| 2 | L2 — débogueur LLDB | INTÉGRÉ dans [PR #21](https://github.com/KAYTAY10ok/NEXUL/pull/21) ; 8 tests passés, dont le test réel avec `lldb-dap` 23. Intégration UI de Main (M7) reste à vérifier. |
| 3 | P9 — CLI et LSP | INTÉGRÉ dans [PR #24](https://github.com/KAYTAY10ok/NEXUL/pull/24). La question des diagnostics issus de `@derive` demeure ouverte dans [issue #23](https://github.com/KAYTAY10ok/NEXUL/issues/23), en attente d'une décision de Main. |
| 4 | L3 — installateur Studio natif | INTÉGRÉ dans [PR #25](https://github.com/KAYTAY10ok/NEXUL/pull/25) ; scénario installation/mise à jour/désinstallation validé en bac à sable. Build complet de Studio MSVC non vérifié dans ce lot. |
| 5 | L4 — extension VS Code | INTÉGRÉ dans [PR #26](https://github.com/KAYTAY10ok/NEXUL/pull/26), puis formatage automatique dans [PR #28](https://github.com/KAYTAY10ok/NEXUL/pull/28). VSIX public sous `KAYTAY10ok.nexul-language`; édition 1 passe par le formateur LSP et garde ses limites documentées. |
| 6 | L5 — modèles de projet | INTÉGRÉ dans [PR #27](https://github.com/KAYTAY10ok/NEXUL/pull/27) ; 5 tests unitaires et 2 fonctionnels passés sous Windows MSVC et Linux. |
| 7 | A1 — benchmarks P13 | VALIDÉ_E2E sur le commit figé `c40acbd` : 4 langages × 4 charges × 3 tailles × 3 répétitions ; toutes les sorties concordent avec C. Sources et mesures datées sous `benchmarks/astral/` ; [PR #29](https://github.com/KAYTAY10ok/NEXUL/pull/29) ouverte, vérifications Linux/Windows en attente. |

## Travail courant

- Base de travail A1 : branche `refonte/sol-a1-benchmarks`, créée depuis `origin/main` à `c40acbd`.
- PR #29 ouverte depuis `refonte/sol-a1-benchmarks` ; CI Linux et Windows en cours. Intégration laissée à la revue de Main.
- Les tâches 0 à 6 du prompt sont fusionnées. La publication Marketplace existe ; aucun nouveau VSIX n'a été publié pendant le lot A1.
- Le rapport daté inclut les versions d'outils, les options d'optimisation, les temps de compilation, le pic du working set, les sorties et les mesures individuelles.
- Les résultats sont une base sur une seule machine Windows x64; ils ne permettent pas une promesse générale de performance.
- À coordonner avec Main : issue #23 (format d'URI/provenance des diagnostics `@derive`) et raccordement des crates terminal/DAP dans Studio (M6/M7).
- La v0.3.0 n'est pas complète : compilation séparée/cache, interop C/Python, concurrence et messages français relèvent du cœur/Main ; P14 exige les scénarios E2E de release.

## Preuves L1

- Avant correction : commande MSVC lancée ; 3 tests du modèle réussis, les 2 tests
  ConPTY restent bloqués. Exécution interrompue, sans résultat de réussite déclaré.
- Correction : surveillance indépendante de la fin du processus ; fermeture des
  ressources PTY pendant que le lecteur draine ; événement `Exited` unique après
  drainage ; réponses aux requêtes de position du curseur nécessaires à ConPTY.
- `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/terminal/Cargo.toml` :
  **7 réussis, 0 échec, 0 ignoré**, tests en 0,51 s, commande complète en **2,76 s**
  (compilation incrémentale incluse). PowerShell et `cmd.exe` réels.
- Régressions : requête curseur fragmentée, sortie normale, redimensionnement et
  arrêt idempotent, sortie non nulle avec plus de 64 Kio de texte à drainer.
- `cargo +stable-x86_64-pc-windows-msvc fmt --manifest-path studio/terminal/Cargo.toml` : réussi.
- README : chaîne MSVC requise documentée ; cycle de vie et contrat des événements
  décrits pour la vue GPUI de Main. Chaîne GNU et Unix : non testés ici.
- Intégration GPUI et validation de Main : non testées ; aucun statut `LIVRÉ` déclaré.

## Demandes à Claude Main

1. Traiter [issue #23](https://github.com/KAYTAY10ok/NEXUL/issues/23) : décider comment exposer en LSP les diagnostics créés depuis une source synthétique `@derive`.
2. Raccorder dans Studio les APIs du terminal et du DAP livrées par L1/L2 (M6/M7), puis valider une session réelle via l'interface.
3. Le cœur v0.3.0 et les scénarios finaux P14 restent sous la responsabilité de Main/Core ; le rapport A1 fournit maintenant une base de performance figée.

## Preuves L2

- Avant correction : test réel lancé ; échec reproduit `Timeout("launch")`.
- Correction additive : `request_async`, `PendingRequest::wait`, `begin_launch` et
  `begin_attach`. Le client peut configurer après `initialized`, envoyer
  `configurationDone`, puis attendre la réponse de lancement. Les appels synchrones
  existants restent disponibles. Une requête abandonnée ou expirée est désinscrite.
- `cargo test --manifest-path studio/dap/Cargo.toml -- --nocapture`, avec
  `NEXUL_NXC=%LOCALAPPDATA%\Programs\NEXUL\bin\nxc.exe` et
  `NEXUL_LLVM=C:\Program Files\LLVM` : **3 unitaires + 4 faux adaptateur + 1 réel
  réussis, 0 échec, aucun test sauté**. Le test réel réussit en **1,81 s** : PDB
  adjacent, breakpoint vérifié ligne 3, `answer = 42`, pas à pas ligne 4.
- Le faux adaptateur diffère maintenant launch/attach jusqu'à configurationDone ;
  tests de réponse tardive après expiration/abandon et de sortie de l'adaptateur.
- `cargo +stable-x86_64-pc-windows-msvc fmt --manifest-path studio/dap/Cargo.toml` :
  réussi. `cargo fmt` GNU non disponible (composant absent) ; aucun outil ajouté.
- README : séquence DAP correcte et API pour M7 décrites. Aucun changement du
  compilateur ou de GPUI ; autres plateformes et interface GPUI non testées.
- Main : revoir l'API additive et employer `begin_launch`/`begin_attach` dans M7.

## Preuves sécurité (revue Main des PR #16 et #14)

- Extension : `nxcPath` en `machine-overridable`, déclaration Workspace Trust limitée
  et configuration restreinte ; gardes sur les événements, services IDE, commandes
  et tâches. Les outils reprennent après l'approbation du dossier.
- `npm test` dans `editors/vscode` : **9/9 réussis**. Le test charge le vrai module
  dans un hôte VS Code simulé, expose volontairement un chemin de dossier hostile
  et vérifie zéro lecture de ce réglage et zéro exécution avant confiance, puis la
  reprise après confiance. Vrai hôte VS Code : non testé dans ce lot.
- Contrôles `node --check` réussis ; `npm ci` et `npm run package` réussis,
  VSIX de 13 fichiers, 18,87 Ko. Aucun réglage utilisateur ou Marketplace modifié.
- Installateur : le relais ne transmet que des choix ; le processus élevé refait
  `old::scan`, résout toutes les sélections avant toute opération et rejette les
  commandes brutes et les cibles absentes. Plus de journal privilégié écrit dans un
  chemin fourni par l'appelant. Les résultats passent par le code de sortie.
- Ancien `install.txt` : validation intégrale des chemins avant suppression,
  refus d'absolus, `..`, noms Windows ambigus et jonctions sortant de la racine.
- `cargo test -p nexul-installer --features test-sandbox --test silent --bins` :
  **20 unitaires réussis, 2 anciens ignorés** (picker/sleeper), **4 silent réussis**.
  Relais falsifié, manifeste hostile, vraie jonction NTFS et cycle réel
  installation/mise à jour/désinstallation vérifiés dans le bac à sable.
- `git diff --check` : réussi. Vraie boîte UAC : non testée. Les contrôles canoniques
  sont répétés avant suppression ; pas de verrouillage complet de tous les composants
  Windows contre une course de remplacement d'une jonction.
- Limites restant documentées : vraie boîte UAC et verrouillage complet contre une course de remplacement de jonction non validés dans ce lot.

## Preuves L3 — installateur natif

- Tests du bac à sable Windows : installation, mise à jour et désinstallation, Studio
  natif par défaut, migration des préférences, association `.nx` et raccourci vérifiés.
- Sur le commit `868aced` de la PR #25, le run Actions `38867914441` passe le build Linux
  et les tests Windows de l’installateur. Huit tests Linux passent ; deux échouent dans
  `tests/run/ffi.nx` car `ld.lld` ne résout pas `abs`, `strlen` et `sqrt`. Ces chemins
  appartiennent à `compiler/**` / Core. Ce résultat décrit ce run historique ; le statut CI
  du commit actuel `c40acbd` n'a pas été revérifié pendant le lot A1.
- Construction de `studio/app` et paquet complet via MSVC : non testés sur cette machine.

## Preuves L4 — extension VS Code et édition 1

- `cargo build --release -p nxc` réussi depuis la base `496949f`.
- `cargo test -p nxc` : 32 tests réussis (11 unitaires, 6 fondations, 5 LSP, 10 conformité), 0 échec ; 1 test Web ignoré comme prévu sans sa chaîne dédiée.
- `npm ci` : réussi, 0 vulnérabilité signalée. `npm test` : **14/14** réussis.
- `npm run package` : réussi ; le VSIX (331 fichiers, 479,95 Ko) inclut le client LSP et
  ses dépendances d’exécution.
- `npm run test:host` avec le vrai binaire `nxc` du dépôt : réussi dans VS Code Insiders
  avec profil et workspace temporaires. Le test vérifie le démarrage de `nxc lsp`, le
  CodeLens, l’absence de faux diagnostics sur le fichier Edition 1 valide, puis les
  commandes `check`, `build`, `run` et `test`. En 0.2, formatage, diagnostic après une
  édition non enregistrée et effacement du diagnostic après correction passent aussi.
- PR #28 a ensuite activé `nxc` comme formateur par défaut et le formatage à l'enregistrement
  pour les deux modes de langue ; le mode Edition 1 envoie la requête au formateur LSP,
  avec une gestion d'indentation documentée pour `|>`.
- Le Marketplace public a été confirmé dans la tâche de publication antérieure. Ce lot A1
  n'a ni republié ni modifié le VSIX.

## Historique de la CI L3 (PR #25)

- Le journal reçu est un échec réel de `cargo build --verbose` sous Ubuntu : le workspace
  compile `nexul-installer`, qui importe `std::os::windows::ffi::OsStrExt`, utilise
  `encode_wide` et lie `kernel32`. La compilation se termine avec le code 101; ce n'est
  pas un avertissement à ignorer et les tests ne démarrent pas.
- `.github/workflows/rust.yml` sépare maintenant les plateformes : Ubuntu construit et
  teste le workspace en excluant `nexul-installer`; Windows lance les tests bac à sable
  de l'installateur avec `test-sandbox`.
- `git diff --check` : réussi. Le premier passage de la PR #25 a révélé deux échecs :
  l'LLVM fourni par Ubuntu rejetait le debug record `#dbg_declare`, et Windows comparait
  un chemin court 8.3 au chemin long équivalent. LLVM 19 corrige le premier problème; le
  test de raccourci unitaire passe. Le second passage échoue ensuite dans `tests/run/ffi.nx` :
  `abs`, `strlen` et `sqrt` ne sont pas résolus parce que l'éditeur de liens Linux NEXUL
  produit des exécutables statiques sans libc. Ce code relève de `compiler/**` (Core).
  `Path::canonicalize` ne résolvait pas le nom court 8.3 dans l'environnement sandbox.
  `GetLongPathNameW` a corrigé les comparaisons, puis le test E2E a révélé que cette aliasation
  empêchait aussi la désinstallation de reconnaître son propre raccourci. `old::norm` étend
  maintenant les chemins existants avant le contrôle de contenance. Sur le run GitHub Actions
  `38867503229` du commit `0a61b50`, le build Linux et le job Windows réussissent; les tests
  Linux échouent ensuite dans `tests/run/ffi.nx` sur les symboles C `abs`, `strlen` et `sqrt`
  non résolus. Ces tests relèvent de `compiler/**` / Core et la PR L3 ne les modifie pas.
  La construction de `studio/app` et du paquet complet reste à vérifier par
  `tools/build_installer.py` sur Windows avec MSVC.

## Preuves A1 — base P13 des performances

- Sources équivalentes livrées pour NEXUL, C, Rust et C# : boucle/mathématiques, vecteur
  transformé par une matrice 3×3, allocations de petits blocs et construction/lecture de
  chaînes ASCII.
- Base figée : `origin/main` `c40acbd83f4818d3ee46ec420d0e479caa92b801`. Le compilateur n'a
  pas été modifié. `cargo build --release -p nxc` : réussi sur Windows le 16/10/2026.
- `python benchmarks/astral/run.py --sizes 1000000 10000000 100000000 --repetitions 3` :
  terminé ; toutes les sorties des 4 langages concordent avec la référence C. 144 mesures
  (4 charges × 3 tailles × 4 langages × 3 répétitions).
- Options : Clang `-O2 -ffp-contract=off -fno-fast-math`, Rust `-C opt-level=2`, .NET
  `Release` avec Tiered PGO, NEXUL `--release`. Le pilote conserve les mesures individuelles,
  médianes, écarts-types, temps de compilation et pics `PeakWorkingSetSize`.
- Rapport lisible : [`benchmarks/astral/results/2026-10-16.md`](../../benchmarks/astral/results/2026-10-16.md) ;
  données brutes : [`benchmarks/astral/results/2026-10-16.json`](../../benchmarks/astral/results/2026-10-16.json).
- À 100M, NEXUL mesure environ 1,00 s en boucle scientifique, 0,87 s vecteur/matrice,
  5,20 s en allocations et 0,45 s en chaînes. Ces valeurs valent pour cette machine et ces
  implémentations seulement ; le rapport montre aussi les écarts aux références.
- Statut : `VALIDÉ_E2E` sur un seul PC Windows x64 ; pas une certification comparative
  générale ni une preuve pour Linux/macOS.
