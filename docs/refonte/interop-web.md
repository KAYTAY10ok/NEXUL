# Contrat WebAssembly pour Studio — état initial

Date : 15 octobre 2026. Statut : inventaire de l'interface existante et proposition, pas livraison d'une cible navigateur. Responsable : Codex.

## Commandes existantes

```text
nxc check calcul.nx --target wasm32-wasi --json
nxc build calcul.nx --target wasm32-wasi --release -O3 -o calcul.wasm
nxc run calcul.nx --target wasm32-wasi
nxc build calcul.nx --target wasm32-wasi --lib --release -o calcul.wasm
nxc build calcul.nx --target wasm32-wasi --release --emit llvm-ir -o calcul.ll
```

`run` utilise Node et le lanceur WASI embarqué. `--lib` sélectionne les fonctions `@export("nom")` comme racines, sans entrée principale. `--emit llvm-ir` produit du texte LLVM, pas un module Wasm. La compilation nécessite LLVM-C et wasm-ld. Ces commandes sont présentes dans le driver ; leur disponibilité ne garantit pas que tout programme puisse fonctionner dans un navigateur.

## Diagnostics

`check --json` fournit les diagnostics sur stdout, avec code de sortie 1 en cas d'erreur sémantique et 0 sinon. Ne pas ajouter `--json` à une commande build en espérant obtenir aussi un binaire : le driver quitte après les diagnostics. Pour construire, lancer séparément build et vérifier son statut. Les erreurs de génération/link sont des messages textuels ; ne pas analyser leur formulation comme un protocole stable. Le schéma JSON n'est pas encore versionné.

## ABI existante et limites

Les fonctions exportées et externes utilisent l'ABI externe du compilateur. Sur Wasm, les attributs `@link` d'une fonction externe déterminent le module importé, et le nom de fonction détermine le nom importé. Éviter d'exposer directement String, Vec, dyn ou des agrégats internes à JavaScript : leur disposition et leur ownership ne sont pas un contrat public web.

WASI requiert un hôte qui fournit ses imports ; un navigateur n'offre pas automatiquement WASI. Aucun chargeur navigateur officiel, accès DOM ou ABI de chaînes stable n'est livré par ce document. Studio ne doit pas inventer une cible wasm32-browser, une commande web ou un bouton fonctionnel reposant sur ces noms.

## Proposition à stabiliser par des tests

Le premier contrat navigateur visera des scalaires i32/f32/f64, puis des couples pointeur/longueur en mémoire Wasm pour les octets UTF-8. L'API devra préciser qui alloue/libère, la validité des pointeurs, les erreurs et le renouvellement des vues JavaScript après memory.grow. Les imports hôte seront explicites ; aucun accès DOM implicite.

Avant intégration Studio : fournir une cible réellement implémentée, un exemple reproductible, un chargeur, un test des imports/exports et un test d'échange de chaînes. Une révision de ce document annoncera les commandes validées et leurs limitations. L'interface Studio et ses modèles restent sous la responsabilité de Claude.
