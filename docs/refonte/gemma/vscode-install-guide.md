# Extension NEXUL pour VS Code — brouillon

**Statut : non installable depuis main 548b1f6.** L’arbre inspecté ne contient ni editors/vscode ni paquet VSIX. Ce document prépare la validation ; ce n’est pas un guide d’installation d’une extension publiée.

## Informations à fixer par le propriétaire

- Identifiant Marketplace et éditeur, ou nom/version du VSIX.
- Version minimale de VS Code et plateformes testées.
- Commandes disponibles et configuration du chemin nxc.
- Portée réelle de coloration, diagnostics, formatage, build/run/test, LSP et DAP.
- Canal officiel de publication et empreinte des artefacts.

Sans ces informations et un VSIX, ne pas demander aux utilisateurs de chercher une extension sous un nom supposé.

## Procédure à compléter après livraison d’un VSIX

1. Installer NEXUL selon docs/installation.md.
2. Ouvrir un nouveau terminal et confirmer que nxc est accessible. Si le compilateur n’est pas dans PATH, suivre la méthode de configuration documentée par l’extension ; ne pas inventer un paramètre.
3. Installer le VSIX fourni par un canal officiel, puis recharger VS Code.
4. Ouvrir un fichier .nx et vérifier que le mode de langage NEXUL est affiché.
5. Exécuter les commandes documentées. Vérifier une erreur de compilation et la construction d’un fichier valide.
6. Vérifier Unicode, commentaires et chaînes interpolées.
7. Si LSP ou DAP est annoncé, tester l’outil absent puis installé, sans téléchargement silencieux.
8. Désinstaller, redémarrer VS Code et vérifier qu’aucun processus ou résidu n’est conservé.

Commande possible quand le nom du VSIX sera connu :

~~~powershell
code --install-extension .\NEXUL-<version>.vsix
~~~

Le nom contient un espace réservé et n’est pas un fichier réel.

## À compléter avant publication comme guide utilisateur

- Remplacer l’espace réservé par l’artefact publié et son empreinte.
- Ajouter l’éditeur Marketplace vérifié, les paramètres et commandes réellement implémentés.
- Exécuter la procédure sur une installation propre, avec puis sans nxc.
- Ajouter preuves et limitations connues.
