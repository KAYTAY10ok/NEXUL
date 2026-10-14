# NEXUL Studio

L'environnement de développement officiel du langage **NEXUL** : écrire, comprendre,
exécuter, tester, déboguer et compiler des programmes NEXUL, gérer les paquets, et
concevoir des sites web avec un **aperçu en direct**.

## Lancer

NEXUL Studio est installé avec NEXUL (`NEXUL Setup.exe`, voir
[docs/installation.md](../docs/installation.md)) : raccourci sur le Bureau, menu
Démarrer, ou double clic sur un fichier `.nx`. L'interface s'ouvre dans sa propre fenêtre
(Microsoft Edge en mode application) ; le Studio s'arrête tout seul quelques secondes
après la fermeture de la fenêtre. Le bouton « Lancer NEXUL Studio » de l'installateur ouvre
le dossier des exemples ; ensuite, la dernière session est restaurée.

Le compilateur `nxc` est cherché à côté du Studio (installation), puis avec les variables
`NEXUL_NXC` (chemin de `nxc.exe`) et `NEXUL_HOME`, dans le `PATH`, et enfin dans une
construction du dépôt (`C:\NEXUL\target\release`). On peut aussi le choisir dans
**Paramètres → Compilateur NEXUL**.

## Ce que fait le Studio

| Domaine | Fonctions |
|---|---|
| Compréhension du code | services fournis par le compilateur (`nxc ide`) : **documentation au survol** (types, signatures, commentaires `///`), **complétion** des champs, méthodes et méthodes de traits, **aller à la définition** (F12, y compris dans la bibliothèque standard), **références** (Maj+F12), **renommage** (F2), **plan** du fichier, **mise en forme** (`nxc fmt`) |
| Éditeur | coloration NEXUL, HTML, CSS, JavaScript, JSON, Markdown ; crochets colorés et appariés ; guides d'indentation ; minimap ; indentation et fermeture automatiques ; commentaires (Ctrl+/) ; lignes déplacées ou dupliquées ; annulation propre à chaque fichier |
| Diagnostics | vérification en direct : erreurs soulignées, pastilles dans la marge, info-bulle avec le code d'erreur et un lien « Expliquer » ; panneau **Problèmes** ; les modules sont vérifiés à travers le programme qui les importe |
| Exécution et tests | **F5** compile et lance (le programme lit le clavier depuis le terminal) ; **Maj+F5** l'arrête ; **F6** et la vue **Tests** lancent les fonctions `@test`, une par une ou toutes ; **Ctrl+Maj+B** produit un exécutable, y compris pour une autre cible |
| Débogage | **F9** : points d'arrêt (clic dans la marge), continuer (F8), pas à pas, variables locales, pile d'appels, évaluation d'expressions, console ; s'appuie sur LLDB (`lldb-dap`) installé avec NEXUL |
| Paquets | vue **Paquets** : projet `nexul.toml`, dépendances locales ou Git, installation, mise à jour, vérification de l'intégrité (`nxpm`) |
| Extensions | extensions en JavaScript isolées et soumises à permissions ([API](../docs/studio/extensions.md)) ; trois exemples fournis (`plugins/`) |
| Web | **aperçu en direct** à côté du code (Alt+L), rechargé à chaque modification ; formats ordinateur, tablette et téléphone ; squelette HTML (`!` + Tab), abréviations (`div.carte` + Tab) ; sélecteur de couleur ; **Concepteur web** |
| Espace de travail | explorateur (créer, renommer, supprimer vers la Corbeille), recherche dans les fichiers, ouverture rapide (Ctrl+P), palette de commandes (Ctrl+Maj+P), onglets, fil d'Ariane, nouveaux projets NEXUL ou web, thèmes sombre et clair, session restaurée au démarrage |

Raccourcis complets : menu **Aide → Raccourcis clavier**.

## Construire

```
cargo build --release -p nexul-studio
```

produit `target/release/NEXUL-Studio.exe` (l'installateur le renomme `NEXUL Studio.exe`).
Le programme n'a aucune dépendance : un petit serveur HTTP en Rust (`src/`) sert
l'interface (`ui/`, intégrée dans l'exécutable) sur `127.0.0.1`, et lui donne accès aux
fichiers du dossier ouvert, au compilateur et au débogueur.

- `src/main.rs` : démarrage, fenêtre, API (fichiers, recherche, vérification, services
  d'éditeur, exécution, paquets, extensions), live server.
- `src/runner.rs` : exécution des programmes (sortie lue en continu, entrée clavier, arrêt).
- `src/debug.rs` : pont vers `lldb-dap` (Debug Adapter Protocol).
- `src/http.rs` : serveur HTTP minimal.
- `ui/editor.js` : l'éditeur ; `ui/langs.js` : langages ; `ui/app.js` : l'atelier ;
  `ui/ide.js` : services du compilateur, tests, débogueur, paquets ; `ui/plugins.js` :
  extensions.

Options : `--no-window` (affiche l'adresse au lieu d'ouvrir la fenêtre), `--port N`, et
un fichier ou un dossier à ouvrir. Pour modifier l'interface sans recompiler : lancez le
Studio avec la variable `NEXUL_STUDIO_DEV=1`, il lit alors `ui/` à chaque chargement ;
Ctrl+R recharge l'interface.

## Sécurité

Le serveur n'écoute que sur `127.0.0.1`, vérifie l'en-tête `Host`, et chaque appel à l'API
porte un jeton aléatoire tiré au lancement. L'API refuse tout chemin hors du dossier
ouvert. Les fichiers supprimés vont dans la Corbeille. Les extensions s'exécutent dans des
Web Workers, sans accès direct à l'API. Réglages et sessions :
`%APPDATA%\NEXUL Studio\state.json`.
