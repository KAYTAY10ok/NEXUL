# État de Codex Luna — extension VS Code

Mis à jour le 15 octobre 2026. Branche de travail : refonte/luna-vscode-extension, créée depuis main (edb886c).

## Lot P10 — Extension officielle VS Code

Statut : IMPLÉMENTÉ ; TESTÉ_UNITAIREMENT ; VSIX installé localement. VALIDÉ_E2E reste À FAIRE.

Le dossier editors/vscode contient maintenant une extension JavaScript installable pour VS Code :

- association des fichiers .nx au langage NEXUL ;
- grammaire TextMate alignée sur le lexique documenté de NEXUL 0.2 et snippets de départ ;
- commandes nxc build, run, test, check et format, plus tâches VS Code découvrant nexul.toml ou les fichiers .nx ;
- diagnostics issus de nxc check --json à l’ouverture et à l’enregistrement ;
- complétion, survol, définition, références, renommage, formatage et CodeLens de test alimentés par nxc ide avec le contenu non enregistré du document ;
- formatage à l’enregistrement désactivé par défaut ;
- configuration nexul.nxcPath pour sélectionner le compilateur ;
- exécution des commandes comme processus sans shell et lancements de programme dans le terminal intégré.

nxc lsp n’est pas implémenté dans le compilateur de référence. L’extension n’invente pas de serveur LSP : elle appelle les commandes nxc ide déjà disponibles. Les diagnostics check lisent le fichier enregistré ; les autres services ide et le formateur acceptent le texte en mémoire. La coloration et les snippets devront suivre l’ADR de la nouvelle syntaxe lorsqu’elle sera approuvée.

## Vérifications

- node --check sur src/cli.js, src/tasks.js et src/extension.js : réussi.
- Validation JSON du manifeste, de la grammaire, de la configuration et des snippets : réussie.
- npm test : 6 tests réussis, 0 échec.
- npm run package : VSIX généré avec succès, dist/kaytay10ok.nexul-language-0.1.0.vsix.
- VS Code 1.132.0 : code --install-extension a réussi ; code --list-extensions --show-versions confirme kaytay10ok.nexul-language@0.1.0.
- nxc n’est pas installé ni dans PATH de cet environnement. La compilation/exécution d’un programme réel et un test dans l’hôte VS Code avec nxc restent non testés. Le Marketplace n’a pas été sollicité ni publié.

## Demande à Claude Main — extensions NEXUL Studio

À la demande de l’utilisateur, demander à Main d’ajouter à Studio :
1. une extension intégrée de formatage qui appelle nexul.compiler.format, avec une commande et un formatage à l’enregistrement désactivé par défaut ;
2. une API de thèmes/styles et préférences persistantes, limitée à des valeurs validées et à une permission explicite, sans injection CSS/DOM arbitraire ;
3. des exemples et tests de persistance, réinitialisation, valeurs invalides et accès sans permission.

Les dossiers studio/plugins, studio/src, studio/ui et docs/studio restent sous la propriété de Claude Main et n’ont pas été modifiés par Luna.

Conformément à la charte V3, une issue GitHub devait accompagner cette demande. La tentative de création via le connecteur GitHub a échoué avec HTTP 403 « Resource not accessible by integration ». Aucun numéro d’issue n’est donc déclaré. Cette section conserve la demande complète pour le rapport de PR ; elle reste à transmettre à Main par un mécanisme GitHub autorisé.
