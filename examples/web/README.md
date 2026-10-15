# Exemple NEXUL navigateur

Depuis la racine du dépôt :

```powershell
cargo build --release -p nxc
.\target\release\nxc.exe build examples/web/calcul.nx --target wasm32-web --lib --release -o examples/web/calcul.wasm
python -I examples/web/serve.py
```

Ouvrir http://127.0.0.1:8766/examples/web/. Résultat : 42 et Bonjour, NEXUL 🌍 — été !
Le fichier .wasm est généré, pas versionné. Le chargeur se trouve dans lib/web/nexul.mjs.
Le serveur nécessite Python 3 ; il fixe les types MIME des modules JS sous Windows.
Ce modèle est indépendant de Studio ; aucune dépendance npm ni WASI.

Voir [le contrat](../../docs/refonte/interop-web.md) pour la mémoire, les erreurs, les imports hôte et les limites. La v0.2.0 publiée ne fournit pas cette cible : utiliser le compilateur construit depuis cette branche.
