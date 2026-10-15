# État des travaux de Codex Luna — terminal, débogage et projets

Branche : `refonte/codex-exec`, créée depuis `origin/main` (`d537a7b`). Périmètre :
`studio/terminal/**`, `studio/dap/**`, `studio/templates/**` et ce fichier.

Dernière mise à jour : 15 octobre 2026.

## Travaux réalisés

- `studio/terminal` fournit un terminal PTY indépendant de GPUI : shells Windows et
  Unix, écriture, redimensionnement, arrêt, événements, modèle de cellules et historique,
  sélection, couleurs/styles ANSI, titres OSC et caractères Unicode de largeur variable.
- `studio/dap` fournit le transport DAP stdio avec trames bornées, routage des réponses,
  événements, commandes de session et points d'arrêt conditionnels. Le registre détecte
  `lldb-dap`, `debugpy`, `netcoredbg` et l'adaptateur JavaScript de VS Code sans installer
  d'outils. L'ID transmis à `initialize` est propre à chaque adaptateur. Le crate sait
  construire un exécutable NEXUL avec ses informations de débogage et vérifier le PDB sous
  Windows.
- `studio/templates` fournit des modèles NEXUL console/bibliothèque, nxpm avec dépendance
  locale, site HTML/CSS/JavaScript, Python, C, C++, Rust et C#. Les modèles décrivent
  aussi leurs commandes de construction et d'exécution, ignorent les artefacts locaux et
  refusent les chemins dangereux ou l'écrasement de fichiers.
- L'API `ToolchainProvider` relie les modèles au catalogue de SDK de Studio sans scanner
  `PATH` ni télécharger d'outils.
- Le rapport initial ne contenait pas de modèle NEXUL/WebAssembly, car le contrat navigateur
  n'était pas encore intégré à `main`; il a depuis été intégré par la PR #1.

## Vérifications effectuées

- Terminal : `cargo check --tests` Windows GNU réussi; `cargo test` sous WSL/Linux réussi
  (5 tests). Le test exercite notamment un PTY interactif, le redimensionnement et
  l'événement de fin après arrêt. L'exécution de la suite Windows a été bloquée par
  l'absence de `dlltool.exe` requis par la cible GNU; le backend Windows a été compilé
  avec `cargo check`, mais ConPTY n'a pas été exécuté sur cet hôte.
- DAP : `cargo test` Windows réussi (3 tests unitaires, test d'intégration avec le faux
  adaptateur et test NEXUL/LLDB conditionnel). Le test faux adaptateur valide le cycle DAP
  et l'ID d'adaptateur transmis. Le test NEXUL/LLDB a sauté explicitement, car `nxc` et
  `lldb-dap` ne sont pas installés; l'intégration avec le vrai compilateur et débogueur
  n'est donc pas validée ici.
- Modèles : `cargo test` Windows réussi (4 tests unitaires et 2 tests fonctionnels).
  Le test fonctionnel a généré les modèles, puis construit/exécuté ceux dont les outils
  étaient présents, y compris Python, Node.js et .NET 10.
- `cargo fmt` exécuté pour les trois crates.

## Suivi du modèle NEXUL/WebAssembly

Branche de suivi : `refonte/codex-exec-wasm-template`, créée depuis `origin/main`
(`548b1f6`). Le lot ajoute `nexul-webassembly` dans `studio/templates` après l'intégration
du contrat ABI v1. Il génère l'exemple de calcul et de chaîne UTF-8, le chargeur partagé
depuis `lib/web/nexul.mjs`, une page navigateur et les serveurs Node/Python. La commande de
build cible `wasm32-web`; la disponibilité exige que le catalogue signale ce target via
`nxc targets` et qu'un serveur local soit installé. Le résultat du contrôle de cible est
mis en cache dans le catalogue pour éviter de relancer `nxc` lors de chaque actualisation
des modèles. Le serveur sert `.mjs` et `.wasm` avec leurs types MIME.

Vérifications de ce suivi :
- Node.js 24.19.0 : `--check` réussi sur le script navigateur et le serveur Node.
- Python 3.14.7 : `py_compile` réussi sur le serveur Python.
- Tests HTTP directs sur les serveurs Node et Python : code 200, types MIME JavaScript et
  WebAssembly et octets du fichier `.wasm` préservés.
- La suite Rust `cargo test --manifest-path studio/templates/Cargo.toml` n'a pas été
  exécutée ici : Cargo/rustc et `nxc` ne sont pas installés dans cet environnement. Des
  tests Rust sont ajoutés pour la disponibilité cible, le plan de build, le chargeur, le
  build réel conditionnel, l'ABI via Node et les réponses HTTP.

## Fichiers

- `studio/terminal/Cargo.toml`, `studio/terminal/src/lib.rs`, `studio/terminal/README.md`
- `studio/dap/Cargo.toml`, `studio/dap/src/lib.rs`, `studio/dap/src/bin/fake-dap-adapter.rs`,
  `studio/dap/tests/client.rs`, `studio/dap/tests/nexul_lldb.rs`, `studio/dap/README.md`
- `studio/templates/Cargo.toml`, `studio/templates/src/lib.rs`,
  `studio/templates/tests/functional.rs`, `studio/templates/README.md`
- `docs/refonte/codex-exec-status.md`

## Interfaces et limites

- API proposée à Claude Studio : `Terminal` / `TerminalManager`, `DapClient`,
  `AdapterRegistry`, `ProjectTemplate::create` et `ProjectTemplate::actions`.
- Claude Studio doit implémenter `ToolchainProvider` en adaptant le catalogue de
  `studio/lsp` pour `supports_target` (requête `nxc targets`) et pour les exécutables des
  outils. Sans cette confirmation, le modèle WebAssembly reste caché.
- Les vues GPUI et le branchement des crates dans `studio/app/Cargo.toml` restent dans le
  périmètre Claude Studio.
- Les limites du modèle terminal (écran alternatif, souris, hyperliens, images et
  extensions propres à un terminal) sont décrites dans son README.

## Demandes

Aucune pour l'instant.
