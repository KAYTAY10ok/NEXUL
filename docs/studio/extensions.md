# Extensions de NEXUL Studio — API version 1

Une extension ajoute des commandes, des informations dans la barre d'état ou des
traitements du code à NEXUL Studio. Elle est écrite en JavaScript et s'exécute **isolée**
dans un *Web Worker* : elle n'a accès ni à l'interface, ni aux fichiers, ni au réseau du
Studio, seulement à l'objet `nexul`, dont chaque fonction vérifie une **permission**
déclarée par l'extension et acceptée par l'utilisateur.

## Emplacement

Une extension est un dossier qui contient `plugin.json` et son script :

| Dossier | Extensions |
|---|---|
| `%APPDATA%\NEXUL Studio\plugins\<id>\` | les vôtres (bouton *Ouvrir le dossier des extensions* de la vue Extensions) |
| `<dossier de NEXUL Studio.exe>\plugins\<id>\` | celles installées avec Studio |

Le nom du dossier est l'identifiant de l'extension (lettres, chiffres, `-`, `_`, `.`).
La vue **Extensions** (barre d'activité) les liste ; une extension ne démarre qu'une fois
**activée** par l'utilisateur, qui voit alors les permissions demandées. Si une nouvelle
version de l'extension demande d'autres permissions, elle reste arrêtée jusqu'à ce que
l'utilisateur les accepte à nouveau.

## `plugin.json`

```json
{
  "id": "statistiques",
  "name": "Statistiques du fichier",
  "version": "1.0.0",
  "apiVersion": "1",
  "description": "Affiche dans la barre d'état le nombre de lignes et de fonctions du fichier ouvert.",
  "author": "NEXUL",
  "main": "main.js",
  "permissions": ["editor.read", "statusbar"]
}
```

| Champ | Rôle |
|---|---|
| `name`, `version`, `description`, `author` | affichés dans la vue Extensions |
| `apiVersion` | version de l'API utilisée : `"1"` (une autre version n'est pas démarrée) |
| `main` | le script (par défaut `main.js`) |
| `permissions` | les permissions demandées (tableau ci-dessous) |

## Permissions

| Permission | Donne accès à |
|---|---|
| `commands` | `nexul.commands` : ajouter des commandes (palette Ctrl+Maj+P), exécuter certaines commandes du Studio |
| `notifications` | `nexul.window.showMessage`, `nexul.window.prompt` |
| `statusbar` | `nexul.window.setStatus` |
| `editor.read` | lire le fichier ouvert, sa sélection, son chemin et son langage ; recevoir les événements |
| `editor.write` | modifier le fichier ouvert |
| `workspace.read` | lister et lire les fichiers du dossier ouvert |
| `workspace.write` | créer et modifier des fichiers du dossier ouvert |
| `compiler` | vérifier un fichier NEXUL et mettre du code en forme avec le compilateur |

Appeler une fonction sans sa permission rejette la promesse avec une erreur
« permission non déclarée ». Les chemins sont relatifs au dossier ouvert ; un chemin qui
en sort est refusé.

## L'objet `nexul`

Toutes les fonctions renvoient une **promesse**.

### `nexul.commands`

| Fonction | Effet |
|---|---|
| `register(id, titre, fonction)` | ajoute la commande « *Nom de l'extension* : *titre* » à la palette ; `fonction` (éventuellement `async`) est appelée quand l'utilisateur la choisit |
| `execute(id)` | exécute une commande de l'extension (`plugin:<extension>:<id>`) ou du Studio parmi : `save`, `saveAll`, `check`, `run`, `test`, `build`, `formatDocument`, `gotoLine`, `find`, `showProblems`, `toggleTerminal`, `showOutline`, `showTests` |

### `nexul.window`

| Fonction | Effet |
|---|---|
| `showMessage(texte, genre)` | notification ; `genre` : `"info"` (par défaut), `"success"`, `"error"` |
| `prompt(titre, valeur)` | demande un texte à l'utilisateur ; renvoie le texte, ou `null` s'il annule |
| `setStatus(texte)` | texte de l'extension dans la barre d'état (80 caractères au plus ; `""` l'efface) |

### `nexul.editor`

| Fonction | Effet |
|---|---|
| `getText()` | contenu du fichier ouvert |
| `getSelection()` | texte sélectionné |
| `getPath()` | chemin du fichier ouvert, relatif au dossier (ou `null`) |
| `getLanguage()` | `"nexul"`, `"html"`, `"css"`, `"js"`, `"json"`, `"md"` ou `"text"` |
| `replaceSelection(texte)` / `insert(texte)` | remplace la sélection (ou insère au curseur) |
| `setText(texte)` | remplace tout le contenu (annulable par Ctrl+Z) |

### `nexul.workspace`

| Fonction | Effet |
|---|---|
| `listFiles()` | chemins de tous les fichiers du dossier ouvert |
| `readFile(chemin)` | contenu d'un fichier |
| `writeFile(chemin, texte)` | écrit un fichier (s'il est ouvert, l'éditeur est mis à jour et enregistré) |

### `nexul.compiler`

| Fonction | Effet |
|---|---|
| `check(chemin)` | diagnostics du compilateur pour un fichier NEXUL (tableau d'objets JSON, comme `nxc check --json`) |
| `format(texte)` | le texte mis en forme par `nxc fmt` |

### `nexul.events`

`nexul.events.on(nom, fonction)` (permission `editor.read`) :

| Événement | Quand | Donnée |
|---|---|---|
| `activate` | un autre fichier devient actif | `{ path }` |
| `change` | le fichier actif est modifié (au plus toutes les 400 ms) | `{ path }` |
| `save` | un fichier est enregistré | `{ path }` |

### Divers

- `nexul.apiVersion` : `"1"`.
- `nexul.log(...valeurs)` : écrit dans la console des outils de développement du Studio.
- Les erreurs non rattrapées d'une commande ou d'un événement sont écrites dans cette
  console ; une erreur au démarrage du script arrête l'extension et s'affiche dans la vue
  Extensions.

## Exemples

Les extensions fournies avec Studio (`studio/plugins/`) servent de modèles :

- `statistiques` : nombre de lignes et de fonctions dans la barre d'état
  (`editor.read`, `statusbar`) ;
- `horodatage` : une commande qui insère la date et l'heure (`commands`, `editor.write`) ;
- `todo` : liste les `TODO` du dossier ouvert.

```js
// main.js — une commande qui met le fichier ouvert en forme.
nexul.commands.register('formater', 'Mettre en forme', async () => {
  const texte = await nexul.editor.getText();
  await nexul.editor.setText(await nexul.compiler.format(texte));
  await nexul.window.showMessage('Fichier mis en forme', 'success');
});
```

avec `"permissions": ["commands", "editor.read", "editor.write", "compiler", "notifications"]`.

## Compatibilité

L'API version 1 ne changera pas de façon incompatible : les ajouts (fonctions,
événements, permissions) restent compatibles ; une évolution incompatible portera le
numéro 2, et le Studio indiquera alors la version attendue par chaque extension.
