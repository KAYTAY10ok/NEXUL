# État de Codex Sol — composants, outils et distribution

Mis à jour le 16 octobre 2026 après validation L4/L5 et contrôle des PRs ouvertes. Mandat : `prompt-codex-sol.md` et
charte « Équipe réduite » du worktree de Claude Main, lus à la demande de l'utilisateur.

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
| 1 | L1 — arrêt et sortie ConPTY | TESTÉ_UNITAIREMENT : 7 tests MSVC réussis, ConPTY réel inclus ; [PR #20](https://github.com/KAYTAY10ok/NEXUL/pull/20) **INTÉGRÉ** par Main le 16/10 (revue : 5 suites + 90 exécutions sans blocage ; suites dans `revues-main.md`) |
| 2 | L2 — vrai `lldb-dap` | TESTÉ_UNITAIREMENT : 8 tests réussis, dont le test réel ; [PR #21](https://github.com/KAYTAY10ok/NEXUL/pull/21) **INTÉGRÉ** par Main le 16/10 (vrai `lldb-dap` 23 revérifié ; suites dans `revues-main.md`) |
| 3 | P9 — CLI et LSP | INTÉGRÉ dans [PR #24](https://github.com/KAYTAY10ok/NEXUL/pull/24), sur `origin/main` `496949f` ; tests MSVC précédemment exécutés et consignés ci-dessous. |
| 4 | L3 — installateur natif | VALIDÉ_E2E en bac à sable. [PR #25](https://github.com/KAYTAY10ok/NEXUL/pull/25) **INTÉGRÉ** par Main le 16/10 avec le CI de `main` (PR #28 : échecs FFI Linux corrigés par `e78a357`), l'analyseur JSON de la migration rendu linéaire et la migration sautée en mode élevé (revue). |
| 5 | L4 — extension VS Code et édition 1 | VALIDÉ_E2E dans un hôte VS Code Insiders isolé avec `nxc` du dépôt ; [PR #26](https://github.com/KAYTAY10ok/NEXUL/pull/26) ouverte et fusionnable. Son CI Linux échoue en compilant l’installateur Windows (`std::os::windows`, `kernel32`), problème couvert par la séparation de workflow de la PR #25, en attente de revue Main. Le formatage édition 1 reste bloqué sans modifier le document. |
| 6 | L5 — modèles de projet | TESTÉ : 5 tests unitaires et 2 fonctionnels réussis ; [PR #27](https://github.com/KAYTAY10ok/NEXUL/pull/27) ouverte et fusionnable. Son CI est aussi bloquée par le build Linux global de l’installateur Windows. |

## Travail courant

- Base actuelle : `origin/main` à `496949f`, récupérée le 16 octobre 2026.
- L4 : branche `refonte/sol-l4-vscode`, validation réelle de l’extension, du serveur LSP et des commandes `nxc` réussie sur ce poste.
- Les PR #20, #21, #26 et #27 sont ouvertes et fusionnables ; leurs runs Linux respectifs (#11, #12, #16 et #15) échouent au build global sur l’installateur Windows (`std::os::windows`, `kernel32`). Les tests restent ignorés après cet échec.
- La PR #25 porte la séparation des jobs Linux/Windows. Son run #10 passe les tests Windows et le build Linux, mais 2 tests Linux FFI échouent sur `abs`, `strlen` et `sqrt`, dans `compiler/**` / Core. Revue Main attendue avant intégration de cette correction de workflow.
- L1 et L2 ont intégré `origin/main` (`496949f`) et sont fusionnables ; revue Main attendue. L3, L4 et L5 ont leurs PR ouvertes et attendent aussi revue/intégration.
- Base du lot sécurité : `origin/main` à `f438283`, branche `refonte/sol-securite-outils`.
- Branche L1 : `refonte/sol-l1-terminal` ; L2 : `refonte/sol-l2-dap`. Worktree indépendant sous
  `files-mentioned-by-the-user-prompt/work/NEXUL-sol`.
- Main a remplacé le brouillon `docs/spec/lsp.md` par
  `docs/adr/0009-contrat-nxc-lsp.md`, seul contrat accepté. Le prompt et les tâches
  actualisés ajoutent les deux corrections de `revues-main.md` en priorité 0.
- Le code P9 reste dans le pilote et ses tests, plus le point d'entrée autorisé de lecture superposée dans `compiler/nxc_sema/src/load.rs`; aucune grammaire ou règle de typage n'a été changée.

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

Revue de la PR L1 et raccordement M6 : consommer les événements jusqu'à `Exited`,
conserver le snapshot après sortie et exécuter les opérations bloquantes hors du fil UI.

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
- Main : revue sécurité et validation de l'élévation réelle demandées avant intégration.

## Preuves L3 — installateur natif

- Tests du bac à sable Windows : installation, mise à jour et désinstallation, Studio
  natif par défaut, migration des préférences, association `.nx` et raccourci vérifiés.
- Sur le commit `868aced` de la PR #25, le run Actions `38867914441` passe le build Linux
  et les tests Windows de l’installateur. Huit tests Linux passent ; deux échouent dans
  `tests/run/ffi.nx` car `ld.lld` ne résout pas `abs`, `strlen` et `sqrt`. Ces chemins
  appartiennent à `compiler/**` / Core.
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
  Pour l’édition 1, la commande de formatage prévient et laisse le texte intact.
- `git diff --check` : réussi. Le VSIX n’a pas été installé dans une instance utilisateur
  et l’extension n’a pas été publiée sur le Marketplace. Le formatage Edition 1 dépend
  de son implémentation par Core dans `nxc`.

## Diagnostic et correction de la CI L3

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


## C2 — dépassement de pile Windows/RISC-V (24/10/2026)

Statut : IMPLÉMENTÉ, revue Claude et validation native requises.
Branche `refonte/sol-c2-stack-overflow`, base #43 (`a824c76`).

- Windows : SetThreadStackGuarantee(64 Kio), gestionnaire vectorisé avant l'entrée
  utilisateur, filtrage de 0xC00000FD, sortie directe WriteFile et ExitProcess(101).
  Les autres exceptions continuent leur recherche.
- RISC-V : sigaction sans sa_restorer, masque au troisième mot, pile alternative activée.
- `cargo test --release -p nxc --test codegen_ir --test suite --test robustness` :
  IR 2/2, robustesse 4/4, suite 12/13 (O0/O2 réussis). Le test de migration
  échoue à lancer un binaire avec `Text file busy` (OS 26), également lors de sa relance isolée.
  Cet échec n'est ni ignoré ni déclaré réussi.
- Compilations réelles `stack_overflow.nx` vers x86_64-windows et riscv64-linux : réussies.
- Windows/RISC-V : exécution non testée (pas de Windows/Wine ni de QEMU).
  C6 doit exécuter la suite Windows, où stack_overflow est désormais activé.
- Rust 1.99.1, LLVM 20.1, LLD 23.1.1 ; `CARGO_PROFILE_RELEASE_LTO=false`,
  `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1` (la chaîne locale a aussi produit des objets vides
  avec plusieurs unités ; la compilation à une unité réussit). `git diff --check` : réussi.
- macOS : dépend de C12, inchangé.

Sources ABI vérifiées : documentation Microsoft SetThreadStackGuarantee ;
Linux `include/uapi/asm-generic/signal.h` (champ restorer conditionnel).
