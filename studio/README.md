# NEXUL Studio

L'éditeur de code du langage **NEXUL** : écrire, vérifier, exécuter, tester et compiler des
programmes NEXUL, et concevoir des sites web avec un **aperçu en direct** (live server).

## Lancer

Double-cliquez sur **NEXUL Studio** sur le Bureau (ou sur `NEXUL Studio.exe` dans ce dossier).
L'interface s'ouvre dans sa propre fenêtre (Microsoft Edge en mode application).
Le Studio s'arrête tout seul quelques secondes après la fermeture de la fenêtre.

Le compilateur `nxc` est trouvé automatiquement dans `C:\NEXUL\target\release\nxc.exe`.
Sinon : variable d'environnement `NEXUL_NXC` (chemin de `nxc.exe`) ou `NEXUL_HOME`
(dossier de NEXUL), ou **Paramètres → Compilateur NEXUL**.

## Ce que fait le Studio

| Domaine | Fonctions |
|---|---|
| Éditeur | coloration NEXUL, HTML, CSS, JavaScript, JSON, Markdown ; crochets colorés par niveau et appariés ; guides d'indentation ; surlignage du mot courant ; minimap ; numéros de ligne |
| Écriture | indentation automatique (Entrée entre `{}` ou entre deux balises) ; fermeture automatique de `( [ { " '` ; Tab / Maj+Tab sur plusieurs lignes ; commenter (Ctrl+/ ou Ctrl+:) ; déplacer ou dupliquer des lignes ; annulation propre à chaque fichier |
| Suggestions | mots-clés, types, fonctions de la bibliothèque standard, méthodes après `.`, extraits (`main`, `fn`, `for`, `match`, `test`…), propriétés et valeurs CSS, balises et attributs HTML |
| NEXUL | vérification à chaque enregistrement : erreurs soulignées, pastilles dans la marge, info-bulle avec le code d'erreur et un lien « Expliquer » ; panneau **Problèmes** ; les modules sont vérifiés à travers le programme qui les importe |
| Exécution | **F5** compile et lance ; le programme lit le clavier depuis le terminal ; **Maj+F5** l'arrête ; **F6** lance les tests `@test` ; **Ctrl+Maj+B** produit un `.exe` |
| Web | **Aperçu en direct** à côté du code (Alt+L), rechargé à chaque modification ; formats ordinateur, tablette et téléphone ; ouverture dans le navigateur ; `!` + Tab (squelette HTML), `div.carte` + Tab (balise), fermeture automatique des balises ; sélecteur de couleur sur les couleurs CSS ; **Concepteur web** : blocs HTML, styles CSS et palette prêts à insérer |
| Espace de travail | explorateur (créer, renommer, supprimer vers la Corbeille), recherche dans les fichiers, ouverture rapide (Ctrl+P), palette de commandes (Ctrl+Maj+P), onglets, fil d'Ariane, nouveaux projets NEXUL ou web, thèmes sombre et clair, session restaurée au démarrage |

Raccourcis complets : menu **Aide → Raccourcis clavier**.

## Construire

```
cargo build --release
copy target\release\NEXUL-Studio.exe "NEXUL Studio.exe"
```

Le programme n'a aucune dépendance : un petit serveur HTTP en Rust (`src/`) sert l'interface
(`ui/`, intégrée dans l'exécutable) sur `127.0.0.1`, et donne à l'interface l'accès aux fichiers
du dossier ouvert et au compilateur.

- `src/main.rs` : démarrage, fenêtre, API (fichiers, recherche, vérification, exécution), live server.
- `src/runner.rs` : exécution des programmes (sortie lue en continu, entrée clavier, arrêt).
- `src/http.rs` : serveur HTTP minimal.
- `ui/langs.js` : langages (coloration, suggestions) ; `ui/editor.js` : l'éditeur ; `ui/app.js` : l'atelier.

Options : `--no-window` (affiche l'adresse au lieu d'ouvrir la fenêtre), `--port N`.
Pour modifier l'interface sans recompiler : lancez le Studio avec la variable `NEXUL_STUDIO_DEV=1`,
il lit alors `ui/` à chaque chargement ; Ctrl+R recharge l'interface.

## Sécurité

Le serveur n'écoute que sur `127.0.0.1`, vérifie l'en-tête `Host`, et chaque appel à l'API porte un
jeton aléatoire tiré au lancement. L'API refuse tout chemin hors du dossier ouvert. Les fichiers
supprimés vont dans la Corbeille. Réglages et sessions : `%APPDATA%\NEXUL Studio\state.json`.
