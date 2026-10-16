# NEXUL Language Support pour Visual Studio Code

Cette extension donne à VS Code la coloration et les outils de développement du compilateur NEXUL (nxc) pour les fichiers .nx.

## Fonctions

- Coloration lexicale NEXUL 0.2 ou édition 1, commentaires imbriqués, chaînes, interpolations et attributs.
- L’édition est choisie depuis `// edition: 1` dans les trois premières lignes ou depuis `edition = "1"` sous `[package]` dans le `nexul.toml` le plus proche. Le mode de langage VS Code et la grammaire changent en conséquence; l’édition 0.2 reste le choix par défaut.
- Snippets pour main, fonctions, structures, tests, match et boucles.
- Diagnostics du compilateur dans le panneau Problèmes pendant la saisie, y compris pour les documents non enregistrés et les fichiers importés ouverts.
- Complétion, survol, aller à la définition, références, renommage et symboles via le serveur officiel `nxc lsp`.
- Formatage du document via le serveur LSP. Le formatage à l’enregistrement est facultatif et désactivé par défaut.
- Commandes et tâches VS Code pour compiler, exécuter, vérifier et tester un fichier ou un projet. Les programmes s’exécutent dans le terminal intégré.

L’extension ne contient pas de compilateur NEXUL ni de serveur LSP distinct. Elle lance `nxc lsp` avec le compilateur configuré par `nexul.nxcPath`. Elle demande aussi au serveur les CodeLens de tests et le texte de la bibliothèque standard. Le Workspace Trust doit être accordé avant le lancement du compilateur.

Les grammaires et snippets suivent séparément la syntaxe documentée des éditions 0.2 et 1 (ADR 0010). Le formateur `nxc ide format` ne prend pas encore en charge l’édition 1 : l’extension laisse le document inchangé et le signale au lieu de lui appliquer le formateur 0.2.

## Prérequis

1. Installer NEXUL pour obtenir nxc (ou nxc.exe sous Windows), ou construire le compilateur depuis la racine du dépôt avec cargo build --release -p nxc.
2. Vérifier que nxc fonctionne dans un terminal.
3. Si nxc n’est pas dans PATH, configurer son chemin complet dans les paramètres VS Code :

nexul.nxcPath : chemin vers le compilateur natif de votre plateforme (`nxc` ou `nxc.exe`).

Exemple Linux dans `settings.json` :

```json
"nexul.nxcPath": "/home/you/.local/bin/nxc"
```

Exemple Windows dans `settings.json` :

"nexul.nxcPath": "C:\\NEXUL\\target\\release\\nxc.exe"

Le chemin peut être défini dans les paramètres utilisateur ou remplacé par un espace
de travail approuvé. En mode restreint (Workspace Trust), VS Code ignore les valeurs
de `nexul.nxcPath` définies par le dossier ou l’espace de travail. La coloration et les
snippets restent disponibles ; diagnostics, fonctionnalités LSP, formatage, commandes
et tâches n’exécutent aucun compilateur avant l’approbation. Une fois la confiance
accordée, le serveur LSP démarre et analyse automatiquement les documents ouverts.

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
| nexul.formatOnSave | false | Applique le formatage officiel avant l’enregistrement |

Le lancement du programme utilise ProcessExecution avec arguments séparés, sans passage par un shell. Pour passer des arguments à un programme, utiliser une tâche VS Code personnalisée de type nexul ou lancer nxc run dans le terminal intégré.

## Développement de l’extension

npm test exécute les tests unitaires Node.js sans exiger VS Code. Ils vérifient notamment la détection de l’édition, le démarrage LSP soumis au Workspace Trust, la grammaire et les snippets. npm run package vérifie que le manifeste et les fichiers requis peuvent être assemblés en VSIX.

Pour lancer aussi le test dans un vrai hôte d’extension VS Code avec le nxc construit depuis le dépôt :

Linux/macOS :

```sh
export NEXUL_NXC=/chemin/vers/nxc
npm run test:host
```

Windows PowerShell :

```powershell
$env:NEXUL_NXC = "C:\chemin\vers\nxc.exe"
npm run test:host
```

Le test utilise VS Code Insiders et un profil isolé. Il vérifie le démarrage du vrai serveur `nxc lsp`, le CodeLens et l’absence de faux diagnostics sur le projet d’édition 1, puis les commandes `check`, `build`, `run` et `test` avec le compilateur réel. Pour l’édition 0.2, il vérifie le formatage, les diagnostics sur une édition non enregistrée et leur effacement après correction. L’extension bloque le formatage de l’édition 1 tant que `nxc` ne le prend pas en charge.
