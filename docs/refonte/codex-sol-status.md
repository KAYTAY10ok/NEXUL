# État de Codex Sol — composants, outils et distribution

Mis à jour le 16 octobre 2026. Mandat : `prompt-codex-sol.md` et charte « Équipe réduite » ;
Claude Main conserve la revue et l'intégration de chaque PR.

Mis à jour le 16 octobre 2026 après validation de la matrice P9, mesure release et contrôle des PRs. Mandat : `prompt-codex-sol.md` et charte « Équipe réduite » du worktree de Claude Main, lus à la demande de l'utilisateur.

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
| 1 | L1 — arrêt et sortie ConPTY | [PR #20](https://github.com/KAYTAY10ok/NEXUL/pull/20) **INTÉGRÉE** par Main ; corrections de revue restantes à traiter dans une PR dédiée. |
| 2 | L2 — vrai `lldb-dap` | [PR #21](https://github.com/KAYTAY10ok/NEXUL/pull/21) **INTÉGRÉE** par Main ; corrections de revue en cours sur `refonte/sol-l2-dap-followup`, tests MSVC avec vrai `nxc`/`lldb-dap` réussis. |
| 3 | P9 — CLI et LSP | INTÉGRÉ dans [PR #24](https://github.com/KAYTAY10ok/NEXUL/pull/24), sur `origin/main` `496949f` ; tests MSVC précédemment exécutés et consignés ci-dessous. |
| 4 | L3 — installateur natif | VALIDÉ_E2E en bac à sable. [PR #25](https://github.com/KAYTAY10ok/NEXUL/pull/25) **INTÉGRÉ** par Main le 16/10 avec le CI de `main` (PR #28 : échecs FFI Linux corrigés par `e78a357`), l'analyseur JSON de la migration rendu linéaire et la migration sautée en mode élevé (revue). |
| 5 | L4 — extension VS Code et édition 1 | [PR #26](https://github.com/KAYTAY10ok/NEXUL/pull/26) **INTÉGRÉE** ; le formatage automatique des deux éditions est validé dans un hôte isolé. Le VSIX 0.3.0 est proposé dans [PR #30](https://github.com/KAYTAY10ok/NEXUL/pull/30), publication Marketplace encore à faire. |
| 6 | L5 — modèles de projet | [PR #27](https://github.com/KAYTAY10ok/NEXUL/pull/27) intégrée. |
| 7 | A1 — base de performances P13 | [PR #29](https://github.com/KAYTAY10ok/NEXUL/pull/29) ouverte vers `main`. |

## Travail courant

- Base actuelle : `origin/main` à `c40acbd`, récupérée le 16 octobre 2026. Les PR #20–28 ont été intégrées ; les seuls PR ouvertes détectées sont #29 et #30.
- VSIX : [PR #30](https://github.com/KAYTAY10ok/NEXUL/pull/30) ouverte. Le run CI `38903204531` a réussi Linux et Windows sur le commit `a56896e`; le commit documentaire `1685995` a ensuite avancé la branche, et son état CI est à revérifier.
- Marketplace : la fiche `KAYTAY10ok.nexul-language` sert actuellement la version `0.1.0`. Le VSIX 0.3.0 préparé n'est pas publié ; la publication reste à la charge de l'utilisateur.
- Corrections de revue L2 : branche dédiée `refonte/sol-l2-dap-followup`, issue de `origin/main` à `c40acbd`; test complet MSVC avec outils configurés et vrai `lldb-dap` réussi. PR à créer après mise à jour du présent état.
- La PR #25 et la correction CI #28 sont intégrées ; l'échec FFI Linux `abs`/`strlen`/`sqrt` a été corrigé par Core dans `e78a357`.
- Base du lot sécurité : `origin/main` à `f438283`, branche `refonte/sol-securite-outils`.
- Branche L1 : `refonte/sol-l1-terminal` ; L2 : `refonte/sol-l2-dap`. Worktree indépendant sous
  `files-mentioned-by-the-user-prompt/work/NEXUL-sol`.
- Main a remplacé le brouillon `docs/spec/lsp.md` par
  `docs/adr/0009-contrat-nxc-lsp.md`, seul contrat accepté. Le prompt et les tâches
  actualisés ajoutent les deux corrections de `revues-main.md` en priorité 0.
- Le code P9 reste dans le pilote et ses tests, plus le point d'entrée autorisé de lecture superposée dans `compiler/nxc_sema/src/load.rs`; aucune grammaire ou règle de typage n'a été changée.

| 5 | L4 — extension VS Code et édition 1 | [PR #26](https://github.com/KAYTAY10ok/NEXUL/pull/26) intégrée. Préparation du VSIX 0.3.0 sur `refonte/sol-vscode-release` : formatage automatique vérifié dans un hôte VS Code Insiders isolé pour les deux éditions ; preuves ci-dessous. |
| 6 | L5 — modèles de projet | TESTÉ : 5 tests unitaires et 2 fonctionnels réussis ; [PR #27](https://github.com/KAYTAY10ok/NEXUL/pull/27) ouverte et fusionnable. Son CI est aussi bloquée par le build Linux global de l’installateur Windows. |

## Travail courant

- Base de cette branche : `origin/main` à `c40acbd`, récupérée le 16 octobre 2026.
- Les PR #20, #21, #25, #26, #27 et #28 sont présentes dans l’historique de `origin/main`.
- Branche de préparation du VSIX 0.3.0 : `refonte/sol-vscode-release`; voir les preuves ci-dessous.
- La PR [#30](https://github.com/KAYTAY10ok/NEXUL/pull/30) est ouverte vers `main`. Le run CI `38903204531` a terminé avec succès sur Linux et Windows ; revue/intégration de Main attendue. La branche est à jour avec `origin/main` au commit `c40acbd`.
- Le Marketplace sert actuellement la version `0.1.0` de `KAYTAY10ok.nexul-language`. Le VSIX `0.3.0` est préparé et testé, mais pas publié ; la publication reste à la charge de l’utilisateur.
- Le contrat accepté de `nxc lsp` est `docs/adr/0009-contrat-nxc-lsp.md`. Le code P9 est intégré ; aucune grammaire ni règle de typage n’a été changée dans le lot Sol.

## Mise à jour — VSIX 0.3.0

- Branche `refonte/sol-vscode-release`, basée sur `origin/main` au commit `c40acbd`.
- `editors/vscode/package.json` et le lockfile portent la version `0.3.0`; ajout du changelog et de l’icône Marketplace.
- Formatage à l’enregistrement activé par défaut via `nexul.formatOnSave` (désactivable par espace de travail). Il respecte Workspace Trust et passe le texte non enregistré à `nxc ide format` sur stdin.
- `npm test` : **15/15 réussis** sur Windows et Ubuntu 26.04 sous WSL2.
- Linux : `cargo build --release -p nxc` réussi sous WSL2 ; `nxc check/build/run/test` réussis sur le fixture édition 1 avec LLD système.
- `npm run test:host` : **réussi** sous Windows avec le `nxc.exe` release, et sous WSL2 Ubuntu 26.04 avec VS Code Insiders Linux et le `nxc` Linux. Dans les deux cas, profil et extensions isolés, VSIX installé, serveur LSP, CodeLens, commandes `check/build/run/test`, formatage automatique des éditions 0.2/1, formatage manuel 0.2 et diagnostics non enregistrés passent. Le run Windows de suivi vérifie aussi qu’en édition 1 l’enregistrement réindente les bras `.Rouge` et `.Vert` délibérément désindentés dans le fixture `when`; ce cas précis n’a pas encore été rejoué sous WSL2. Le journal VS Code Linux contient des erreurs non bloquantes `native-keymap`/WebGL propres à l’environnement WSL ; le test se termine avec le code 0.
- `npm run package` : réussi sous Windows et WSL2 ; VSIX `0.3.0`, 333 fichiers, 483,87 Ko, licences incluses. Linux bare metal, macOS et publication Marketplace non testés.

| 1 | L1 — arrêt et sortie ConPTY | TESTÉ_UNITAIREMENT : 7 tests MSVC réussis, ConPTY réel inclus ; [PR #20](https://github.com/KAYTAY10ok/NEXUL/pull/20) **INTÉGRÉ** par Main le 16/10 (revue : 5 suites + 90 exécutions sans blocage ; suites dans `revues-main.md`) |
| 2 | L2 — vrai `lldb-dap` | TESTÉ_UNITAIREMENT : 8 tests réussis, dont le test réel ; [PR #21](https://github.com/KAYTAY10ok/NEXUL/pull/21) **INTÉGRÉ** par Main le 16/10 (vrai `lldb-dap` 23 revérifié ; suites dans `revues-main.md`) |
| 3 | P9 — CLI et LSP | Socle INTÉGRÉ dans [PR #24](https://github.com/KAYTAY10ok/NEXUL/pull/24). [PR #34](https://github.com/KAYTAY10ok/NEXUL/pull/34) ajoute les tests d’éditions LSP mixtes et les mesures ADR 0009 ; CI réussie, revue Main attendue. |
| 4 | L3 — installateur natif | VALIDÉ_E2E en bac à sable. [PR #25](https://github.com/KAYTAY10ok/NEXUL/pull/25) **INTÉGRÉ** par Main le 16/10 avec le CI de `main` (PR #28 : échecs FFI Linux corrigés par `e78a357`), l'analyseur JSON de la migration rendu linéaire et la migration sautée en mode élevé (revue). |
| 5 | L4 — extension VS Code et édition 1 | [PR #26](https://github.com/KAYTAY10ok/NEXUL/pull/26) INTÉGRÉE ; [PR #28](https://github.com/KAYTAY10ok/NEXUL/pull/28) active le formatage à la sauvegarde pour les éditions 0.2 et 1. [PR #30](https://github.com/KAYTAY10ok/NEXUL/pull/30) prépare le VSIX 0.3.0 ; tests hôte Windows et WSL2 documentés, CI réussie, revue Main attendue. |
| 6 | L5 — modèles de projet | INTÉGRÉ dans [PR #27](https://github.com/KAYTAY10ok/NEXUL/pull/27) après 5 tests unitaires et 2 fonctionnels réussis. |

## Travail courant

- Base actuelle : `origin/main` à `c40acbd`, récupérée le 16 octobre 2026.
- Les PR #20, #21, #22 et #24 à #28 concernant L1, L2, la sécurité, P9, L3, L4, L5 et le formatage ont été intégrées par Main ; les anciennes preuves détaillées ci-dessous restent historiques.
- PRs ouvertes : #29 benchmarks (aucun run Actions associé au head trouvé ; résultats locaux dans la PR), #30 VSIX 0.3.0, #31 DAP, #32 terminal, #33 garde de cohérence des versions du paquet et #34 matrice LSP.
- La [fiche Marketplace NEXUL](https://marketplace.visualstudio.com/items?itemName=KAYTAY10ok.nexul-language) affiche encore la version publiée **0.1.0** (vérifiée le 16/10/2026). La PR #30 prépare le VSIX 0.3.0 ; après revue et intégration, la publication de cette version reste une action du propriétaire du publisher.
- CI Actions réussie pour les heads des PR #30, #31, #32, #33 et #34. La PR #34 est fusionnable ; aucun merge n’est demandé à Sol.
- La garde de la PR #33 bloque intentionnellement l’empaquetage tant que Main n’a pas aligné les versions des composants à 0.3.0. Après cet alignement, la construction complète et la mise à niveau sandbox 0.2 → 0.3 restent à rejouer.
- Le formatage des bras `.Variant` est couvert par `format_keeps_variant_arms_and_indents_method_chains` (1 test ciblé réussi dans le worktree P9). Le formatage à la sauvegarde des deux éditions est activé par la PR #28 ; la PR #30 apporte une validation hôte Windows et WSL2.
- Le code P9 reste dans le pilote et ses tests, plus le point d’entrée autorisé de lecture superposée dans `compiler/nxc_sema/src/load.rs`; aucune grammaire ou règle de typage n’a été changée.
- Restent non validés sur une machine dédiée : Linux bare metal et macOS. La revue Main et la fusion par l’utilisateur des PR encore ouvertes restent requises.

## Revue corrective L2 — DAP

- Branche `refonte/sol-l2-dap-followup`, séparée de la préparation VSIX et issue de `origin/main` (`c40acbd`).
- Les appels synchrones publics `launch`/`attach` sont dépréciés au profit de `begin_launch`/`begin_attach`, adaptés à la séquence différée de LLDB.
- `PendingRequest::try_wait` permet le sondage non bloquant. La fermeture du lecteur marque l'état DAP fermé sous le même verrou que les requêtes en attente, réveille les demandes existantes avec `AdapterExited` et fait échouer immédiatement toute nouvelle requête.
- Le test faux adaptateur couvre la sortie spontanée pendant `launch`, la réception de `AdapterExited` et le refus d'une requête postérieure. Le test réel ne saute plus si un chemin `NEXUL_NXC` explicite est invalide ou si `NEXUL_LLVM` est configuré sans `lldb-dap` disponible.
- Validation MSVC : `cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/dap/Cargo.toml -- --nocapture` — 5 unitaires, 4 tests client/faux adaptateur et 1 test réel LLDB/NEXUL réussis, aucun test ignoré. Le test ciblé avec un `NEXUL_NXC` explicitement inexistant échoue comme prévu (code Cargo 101) au lieu d'être ignoré.
- `cargo +stable-x86_64-pc-windows-msvc fmt --manifest-path studio/dap/Cargo.toml` et `git diff --check` réussis. Linux WSL2 : mêmes 5 tests unitaires et 4 tests client réussis ; le test réel est explicitement ignoré faute de `lldb-dap`. Linux bare metal non testé.

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

## Preuves L4 initiales — intégration PR #26

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
  et l’extension n’a pas été publiée sur le Marketplace. À cette étape, le formatage
  Edition 1 ne modifiait pas le document ; le test a été complété dans la section
  « Mise à jour — VSIX 0.3.0 » ci-dessus.

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


## Mandat C1–C12 — 24 octobre 2026

Base de travail : `a824c76`, sommet de la pile #35–#43 non fusionnée.
Mandat explicite : `codex-taches-2026-10-24.md`. Codex intervient sur les sous-périmètres
délégués ; Claude conserve la revue de sûreté et l'utilisateur les fusions.

### C1 — sondes de pile

Statut : TESTÉ_UNITAIREMENT sur Linux x86-64, revue Claude requise avant intégration.
Une PR `[sol]`, branche `refonte/sol-c1-stack-probes`, basée sur la branche de #43.

- Attribut LLVM sur toutes les définitions générées Linux/macOS x86-64/AArch64,
  y compris glue, shims et auxiliaires ; Windows conserve sa stratégie `__chkstk`.
- `cargo test --release -p nxc --test codegen_ir` : 2 réussis.
- `cargo test --release -p nxc --test suite --test robustness --test foundations --test lsp` :
  28 réussis (13 + 4 + 6 + 5), 0 échec. Tests d'exécution à O0/O2 inclus.
- `nxc run tests/run/stack_probe.nx --no-debug` : sortie `before`, panique attendue, code 101.
- Compilation croisée AArch64 Linux réussie ; exécution du binaire O0 sous QEMU user 8.2.2 réussie : before, panique exacte et code 101. O2 AArch64 non testé.
- Mesures P13 : `benchmarks/sol/c1-probes-2026-10-24.{md,json}`, 9 répétitions, 1 M/10 M.
- Chaîne locale Rust 1.99.1, LLVM 20.1, LLD 23.1.1 ; tests Rust avec
  `CARGO_PROFILE_RELEASE_LTO=false` (échec de lien du libtest de la chaîne en LTO thin).
- macOS et Windows : tests IR seulement, exécution non testée.


## C2 — dépassement de pile Windows/RISC-V (24/10/2026)

Statut : TESTÉ_UNITAIREMENT sous QEMU RISC-V et dans la suite Windows native ; revue Claude requise.
Branche `refonte/sol-c2-stack-overflow`, base #43 (`a824c76`).

- Windows : SetThreadStackGuarantee(64 Kio), gestionnaire vectorisé avant l'entrée
  utilisateur, filtrage de 0xC00000FD, sortie directe WriteFile et ExitProcess(101).
  Les autres exceptions continuent leur recherche.
- RISC-V : sigaction sans sa_restorer, masque au troisième mot, pile alternative activée.
- IR 2/2, robustesse 4/4, suite 12/13 à la première exécution (O0/O2 réussis).
  Migration initialement en échec `Text file busy` (OS 26), puis relance finale isolée
  réussie 1/1 (1,46 s). Aucun test désactivé ; échec initial conservé dans la PR.
- Compilations de stack_overflow.nx vers x86_64-windows et riscv64-linux : réussies.
- QEMU user 8.2.2 installé localement : stack_overflow.nx riscv64-linux exécuté
  à O0 et O2, sortie exacte `before\n`, message exact attendu sur stderr, code 101.
- Windows : run d’intégration #61 40132439801, suite compilateur complète réussie. `stack_overflow.nx` cible Linux et Windows ; le harnais exécute les programmes d’hôte à O0 et O2, sortie attendue avant débordement, message panic, code 101. La validation Windows est ainsi couverte. Revue Claude requise.
- Rust 1.99.1, LLVM 20.1, LLD 23.1.1 ; LTO=false et codegen-units=1 pour contourner
  les défauts de la chaîne locale. `git diff --check` réussi. macOS dépend de C12.


## C3 — pile WebAssembly avant les données (24/10/2026)

Statut : TESTÉ_UNITAIREMENT ; revue Claude requise.
Branche `refonte/sol-c3-wasm-stack-first`, base #43 (`a824c76`).

- `link_wasm` passe explicitement `--stack-first` pour WASI et le navigateur.
- `cargo test --release -p nxc --test web -- --ignored --nocapture` : 1/1 réussi ;
  contrat navigateur existant et nouveau piège de pile testés à O0/O2 sur WASI/web.
  Le test vérifie l'adresse des statiques et leurs 16 octets après le piège.
- `NEXUL_TEST_TARGETS=wasm32-wasi cargo test --release -p nxc --test suite run_programs_cross` :
  1/1 réussi (suite d'exécution cross complète).
- LLD 23.1.1 utilise déjà stack-first par défaut ; le drapeau rend cette garantie
  indépendante du défaut du linker. Un contrôle négatif `--no-stack-first` est refusé
  par le nouveau test (statiques avant la pile).
- ABI navigateur et documentation du contrat inchangées. Aucun test nouvellement ignoré.
- Rust 1.99.1, LLVM 20.1, LLD 23.1.1 ; `CARGO_PROFILE_RELEASE_LTO=false`,
  `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1` pour contourner les défauts de la chaîne locale.


## C5 — runtime embarqué et interruptions (24/10/2026)

Statut : TESTÉ_UNITAIREMENT sur Cortex-M QEMU, revue Claude requise.
Branche `refonte/sol-c5-runtime-critical`, base #43.

- Gardes à destruction de portée autour des listes libres, compteurs d'allocation
  et sorties ; restauration du masque précédent sur chaque retour et appel imbriqué.
- Cortex-M : PRIMASK ; RISC-V : MIE de mstatus ; AArch64 : DAIF IRQ/FIQ.
  Assemblage sideeffect et clobber mémoire ; aucune réactivation inconditionnelle.
- allocation_balance fournit un instantané protégé du compteur pour les cibles sans
  variables d'environnement ; la fixture vérifie zéro allocation vivante après l'arrêt du timer.
- `cargo test --release -p nxc --test codegen_ir --test suite -- --test-threads=1` :
  15/15 après la dernière modification, dont O0/O2 et migration.
- Dix compilations/liens embarqués O0/O2 réussis sur thumbv7em, thumbv7m,
  riscv32, riscv64 et aarch64-none. Fixture matérielle Cortex-M uniquement.
- QEMU 8.2.2 installé localement après un premier essai bloqué par son absence.
  `python tools/check-runtime-interrupts.py --nxc target/release/nxc --qemu <qemu-system-arm>` :
  O0 et O2 réussis sur mps2-an386 ; interruptions livrées, 100000 tours du main,
  checksums main/ISR corrects, zéro allocation vivante, 100 lignes main et lignes IRQ intactes.
- Limite explicite : cœur unique et interruptions masquables. NMI/fautes et multicœur
  non couverts ; revue/décision Claude nécessaire sur ces contextes. PR en brouillon
  pour cette limite de sûreté, sans prétendre à une garantie générale de réentrance.
- `git diff --check` réussi. Rust 1.99.1, LTO=false, codegen-units=1, LLVM 20.1, LLD 23.1.1.


## C8 — clés de registre isolées et erreur 1018 (24/10/2026)

Statut : TESTÉ_UNITAIREMENT sur le registre Windows, revue requise.
Branche `refonte/sol-c8-registry-tests`, base #43.
Chaque clé de test combine PID, nanosecondes et compteur atomique.
RegCreateKeyExW réessaie uniquement ERROR_KEY_DELETED (1018), au maximum 20 fois,
avec 10 ms entre tentatives : couvre aussi la suppression concurrente du parent
NEXUL-Tests par un autre test. Les autres erreurs sont rendues immédiatement.

- `cargo check --release -p nexul-installer --target x86_64-pc-windows-msvc
  --features test-sandbox --tests` : réussi, depuis Linux avec std Windows 1.99.1.
- `git diff --check` : réussi.
- Run d’intégration #61 40132439801 sur Windows : test ciblé `win::reg::tests::keys_tree` réussi 20/20 fois consécutives, avec contrôle du code de sortie à chaque passage. Suite sandbox installateur : 24 réussis, 2 anciens ignorés.


## C10 — audit scientifique et chemin ASCII (24/10/2026)

Statut : TESTÉ_UNITAIREMENT pour l'optimisation ASCII, tâche globale partiellement ouverte.
Branche `refonte/sol-c10-perf-audit`, base #43.

- String.push sépare l'entrée ASCII du traitement multioctet privé ; Vec reste inchangé.
- P13 à 10 M, 25 répétitions alternées après les compilations : temps chaîne -12,67 %.
  Premier relevé 9 répétitions : -12,35 % à 10 M, gain non établi à 1 M.
  Médianes, dispersions et tous les échantillons : `benchmarks/sol/c10-*2026-10-24*`.
- Noyau scientifique : principalement f64, 3 llvm.sqrt.f64. Contrôles du reste par
  constante 2 présents O0, repliés par LLVM à O2 ; contrôles de l'addition i64 inconnue
  toujours présents et panique MAX+1 vérifiée avec position via wasm.
- Suite/robustesse : 17/17 ; navigateur : 1/1 ; frontières Unicode, littéral, croissance
  et compteur de fuite O0/O2 : réussis via check-string-push.py.
- Revue Claude requise pour lib/std/string.nx. Aucun changement de règle, alias,
  fast-math ou fichier tests/run de Claude.
- Non terminé : gain vectoriel non démontré, rapport 1,12× C sur Windows non reproduit,
  100 M et autres cibles non mesurés. PR en brouillon ; rapport causal prudent, pas de
  conclusion globale NEXUL/C ni sur la libm de la machine d'origine.


## C6 — CI Windows appliquée après accord (24/10/2026)

Statut : IMPLÉMENTÉ, accord explicite de l'utilisateur reçu après le premier bilan.
LLVM 20.1.8 Windows installé et précontrôlé ; suite compilateur release complète.
Les deux jobs lancent aussi le test navigateur avec --ignored (C4 sinon absent de la CI).
C7 est conservé comme dépendance de cette PR, sans retirer ses déclencheurs.
Validation locale du YAML et de son patch réussie ; exécution Actions à suivre dans #52.
Les tests Windows natifs ne sont pas déclarés réussis avant le résultat des jobs.


## C7 — CI des PR empilées (24/10/2026)

Statut : IMPLÉMENTÉ, validation Actions après intégration.
Branche `refonte/sol-c7-stacked-ci`, base #43.
Le filtre de base des pull_request est retiré ; workflow_dispatch est ajouté.
Déclenchement push main et jobs Linux/Windows conservés.
Validation locale : analyse YAML et assertions sur les événements et jobs réussies ;
`git diff --check` réussi. Exécution GitHub Actions : non testée avant intégration
(la branche de base ne contient pas encore le nouveau déclenchement).
C6 reste distinct : aucun changement des chaînes LLVM ou des commandes de tests ici.


## Réanalyse après accord C6 — 24/10/2026

Base Core actualisée : #60, 7d72bba (C4, ADR 0012/0014/0015).
75 tests workspace portable + 1 navigateur réussis sur cette base ; fuzz 40 tours, seed 2410.
Deux défauts nouveaux reproduits à O0/O2 : export de distinct char/bool sans validation
de la base ; contrainte f32 acceptée en littéral puis refusée par son propre check.
Preuves et reproductions : `benchmarks/sol/reanalyse-2026-10-24`.

C6 appliqué après accord utilisateur. Premier run : installation LLVM Windows et job
Linux réussis ; test foundations en échec sur comparaison de chemin court/canonique.
Assertion corrigée sans retirer la vérification du chemin complet ; --no-fail-fast activé.
La branche de validation réunit Core #60 + C1/C2/C3/C5/C6/C7/C8/C10 sans modifier main.
Résolutions de conflits : ajout des tests IR de chaque branche, conservation de tous les
paragraphes d'état ; pas de modification sémantique du code de Claude.
C9 et C11 ont maintenant une revue préalable approuvée (commentaires #53/#54).
C9 doit réduire le domaine aux locaux suivis et garder un oracle de domaine complet.
C11 doit utiliser nxc_nir::mono et inclure les conversions vers dyn Error ajoutées par #56.


## Réanalyse — correctif LSP et preuves publiées

Rapport : sol-reanalyse-2026-10-24.md. Deux P1 Core reproduits O0/O2.
Correction LSP #62 : six tests Linux passent, alias physique conservé pour les
diagnostics et le manifeste. Les deux anciens tests Windows restent actifs.
C6 #52 et le correctif LSP #62 sont prêts pour revue ; run 40132365202 vert sur Linux et Windows.
L’intégration #61 passe au run 40132439801 sur les deux systèmes, avec 20/20 répétitions du test de registre Windows réussies. C7 passait déjà au run 40124607293.

Statut : CONÇU, accord utilisateur requis par le mandat avant modification de `.github/**`.
Branche `refonte/sol-c6-ci-proposal`. YAML et patch exacts dans `docs/refonte/proposals/sol-c6-*`.
Installation LLVM 20.1.8 avec C API, précontrôle des outils et tests complets release
Windows ; tests Linux workspace en release. Analyse YAML et diff check réussis.
Aucune exécution Windows/Actions déclarée. Le workflow actif reste inchangé.


## C9 — proposition et référence d'analyse (24/10/2026)

Statut : CONÇU, revue Claude obligatoire avant ownership.rs.
Proposition : `proposals/sol-c9-ownership.md`. Bitsets persistants par pages,
validation par oracle dense, diagnostics et destructions inchangés.
20 000 if : médiane locale 5,307 s (3 essais, machine partagée). 50 000 bras :
SIGKILL à 27,126 s, cause non établie. Aucun objectif de temps atteint ou revendiqué.
Données brutes conservées. Aucun code sémantique modifié ; `git diff --check` réussi.


## C11 — proposition NIR monomorphe (24/10/2026)

Statut : CONÇU, revue Claude préalable requise par C11.
Proposition : `proposals/sol-c11-monomorphic-nir.md`. Types concrets et tables dyn
symboliques hors LLVM ; nommage paresseux préservé pour l'identité à l'octet.
Aucun code extrait. Matrice d'équivalence future explicitée, non exécutée/non revendiquée.
`git diff --check` réussi.


## C12 — macOS (24/10/2026)

Statut : BLOQUÉ : pas de Mac. Analyse dans `proposals/sol-c12-macos.md`.
La sélection SYS_LINUX et l'entrée ELF partagée sont confirmées ; aucun résultat
macOS revendiqué. Couche Darwin ou exclusion explicite à arbitrer/revoir par Claude,
notamment les chemins réservés par #45. C2 macOS reste bloqué.

## Preuves P9 — éditions LSP et performances (16/10/2026)

- Ajout dans `compiler/nxc/tests/lsp.rs` de tests d’intégration avec un vrai processus
  `nxc lsp` : projet d’édition 1 important un module ouvert d’édition 0.2, projet 1
  important une dépendance locale sans édition explicite (donc 0.2), et diagnostics
  recalculés après ajout/retrait de l’en-tête d’édition pendant `didChange`.
- Trois tests ciblés en profil debug : **3 réussis**.
- `cargo test --release -p nxc --test lsp -- --nocapture` : **8 réussis, 0 échec,
  0 ignoré**. Mesure Windows release sur ce poste : P95 à chaud — hover **0,322 ms**,
  définition **0,371 ms**, complétion **0,841 ms** ; diagnostics après modification
  **261,93 ms** (attente de 200 ms comprise) ; écart de mémoire de travail sur 1 000
  modifications **16,80 Mio**. Les trois critères de performance de l’ADR 0009 §8 passent.
- `rustfmt --edition 2021 --check compiler/nxc/tests/lsp.rs` : réussi. La vérification
  `cargo fmt --check` de tout le crate signale des fichiers préexistants non formatés ;
  aucun autre fichier n’a été reformaté.
- Non testé : client VS Code/Studio réel, Linux bare metal, macOS et dépendance Git
  téléchargée. Aucun téléchargement n’est fait par le serveur LSP.
