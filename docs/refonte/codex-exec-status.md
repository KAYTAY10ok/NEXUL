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
- L'API `ToolchainProvider` relie les modèles au catalogue de SDK de Studio. Le chemin
  `studio/lsp` n'existe pas dans `origin/main`; la crate n'ajoute donc pas une seconde
  détection de SDK.
- Aucun modèle NEXUL/WebAssembly n'est exposé : le contrat navigateur de Codex Astra n'est
  pas présent ni validé dans la branche de départ.

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
  `studio/lsp` (outils `nxc`, `nxpm`, Python, Node, compilateurs C/C++, Cargo et dotnet).
- Les vues GPUI et le branchement des crates dans `studio/app/Cargo.toml` restent dans le
  périmètre Claude Studio.
- Les limites du modèle terminal (écran alternatif, souris, hyperliens, images et
  extensions propres à un terminal) sont décrites dans son README.

## Demandes

Aucune pour l'instant.
