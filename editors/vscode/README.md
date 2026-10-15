# NEXUL Language Support pour Visual Studio Code

Cette extension donne à VS Code la coloration et les outils de développement du compilateur NEXUL (nxc) pour les fichiers .nx.

## Fonctions

- Coloration lexicale NEXUL 0.2, commentaires imbriqués, chaînes, interpolations et attributs.
- Snippets pour main, fonctions, structures, tests, match et boucles.
- Diagnostics du compilateur dans le panneau Problèmes après l’ouverture et l’enregistrement. La commande Vérifier le fichier courant enregistre d’abord le document.
- Complétion, survol, aller à la définition, références et renommage via nxc ide, avec le contenu non enregistré du document.
- Formatage du document via le formateur officiel nxc ide format. Le formatage à l’enregistrement est facultatif et désactivé par défaut.
- Commandes et tâches VS Code pour compiler, exécuter, vérifier et tester un fichier ou un projet. Les programmes s’exécutent dans le terminal intégré.

L’extension ne contient pas de compilateur NEXUL ni de serveur LSP distinct. Elle réutilise le nxc configuré. La commande nxc lsp n’existe pas dans la version de référence; les fonctionnalités de langage utilisent les requêtes ponctuelles nxc ide jusqu’à l’arrivée du serveur officiel. Les diagnostics du compilateur sont mis à jour à l’ouverture et à l’enregistrement; le contenu non enregistré est pris en charge par les fonctionnalités nxc ide et le formateur.

La grammaire et les snippets suivent la syntaxe documentée du compilateur NEXUL 0.2. Ils devront être révisés quand l’ADR de la prochaine syntaxe sera approuvée.

## Prérequis

1. Installer NEXUL pour obtenir nxc.exe, ou construire le compilateur depuis la racine du dépôt avec cargo build --release -p nxc.
2. Vérifier que nxc fonctionne dans un terminal.
3. Si nxc n’est pas dans PATH, configurer son chemin complet dans les paramètres VS Code :

nexul.nxcPath : chemin de nxc.exe.

Exemple Windows dans settings.json :

"nexul.nxcPath": "C:\\NEXUL\\target\\release\\nxc.exe"

Le chemin peut être défini dans les paramètres utilisateur ou remplacé par un espace
de travail approuvé. En mode restreint (Workspace Trust), VS Code ignore les valeurs
de `nexul.nxcPath` définies par le dossier ou l’espace de travail. La coloration et les
snippets restent disponibles ; diagnostics, fonctionnalités `nxc ide`, formatage,
commandes et tâches n’exécutent aucun compilateur avant l’approbation. Une fois la
confiance accordée, les diagnostics des documents ouverts reprennent automatiquement
si `nexul.checkOnSave` est activé.

## Construire et installer le VSIX

Depuis le répertoire editors/vscode :

npm ci

npm test

npm run package

Le VSIX est produit sous dist/kaytay10ok.nexul-language-0.1.0.vsix. Dans VS Code, ouvrir la palette de commandes, choisir Extensions: Install from VSIX..., puis sélectionner ce fichier. On peut également exécuter code --install-extension <chemin-du-vsix>.

Cette version est installée depuis un VSIX local. Elle n’est pas publiée sur le Marketplace.

## Utilisation

Ouvrir un fichier .nx, puis choisir dans la palette :

- NEXUL: Vérifier le fichier courant
- NEXUL: Construire le fichier courant
- NEXUL: Exécuter le fichier courant
- NEXUL: Lancer les tests du fichier courant
- NEXUL: Formater le document

Les tests annotés @test affichent aussi un CodeLens Exécuter le test. Les tâches disponibles apparaissent dans Tasks: Run Task. Dans un projet avec nexul.toml, elles utilisent le projet; sinon, elles sont générées pour les fichiers .nx trouvés dans l’espace de travail.

Paramètres disponibles :

| Paramètre | Valeur par défaut | Effet |
|---|---:|---|
| nexul.nxcPath | nxc | Chemin vers le compilateur |
| nexul.checkOnSave | true | Met à jour les diagnostics du fichier enregistré |
| nexul.formatOnSave | false | Applique le formatage officiel avant l’enregistrement |

Le lancement du programme utilise ProcessExecution avec arguments séparés, sans passage par un shell. Pour passer des arguments à un programme, utiliser une tâche VS Code personnalisée de type nexul ou lancer nxc run dans le terminal intégré.

## Développement de l’extension

npm test exécute les tests unitaires Node.js sans exiger VS Code. npm run package vérifie que le manifeste et les fichiers requis peuvent être assemblés en VSIX.

La validation complète en hôte d’extension VS Code reste à effectuer sur une installation Windows avec nxc disponible. L’extension n’active pas un serveur LSP imaginaire; son remplacement par nxc lsp devra se faire lorsque le compilateur l’implémentera.
