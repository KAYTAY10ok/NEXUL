# État Gemma — assurance qualité et documentation

Mise à jour : 15 octobre 2026. Branche : refonte/gemma-qa-docs. Audit statique de main 548b1f6.

## Livrables

- [Matrice qualité](gemma/quality-matrix.md) : code, tests repérés, rapports antérieurs et état d’intégration.
- [Checklist manuelle de Studio](gemma/manual-studio-checklist.md) : scénarios et champs de preuve.
- [Guide VS Code](gemma/vscode-install-guide.md) : brouillon bloqué jusqu’à la livraison d’une extension installable.
- [Passation Studio](gemma/handoffs.md) : constat source et demande préparée au propriétaire.
- [Tests de fumée](../../tests/manual/README.md) : petit lanceur réutilisant deux programmes existants en lecture seule et un cas UTF-8.

## Validation de cette session

- Python 3.14 et Node.js sont présents ; Cargo, rustc et nxc ne sont pas disponibles dans le PATH ni aux emplacements locaux vérifiés.
- Le script Python est contrôlé syntaxiquement. Les programmes NEXUL n’ont pas pu être compilés ou exécutés.
- L’audit Studio est fondé sur le code à 548b1f6, sans essai interactif.
- La matrice distingue les tests visibles dans le dépôt des résultats rapportés par leurs propriétaires.

## Constat transmis

La fermeture de la fenêtre GPUI appelle directement le quit de l’application ; la fermeture d’un onglet modifié enregistre automatiquement le fichier. Le scénario de fermeture de fenêtre avec modifications n’est pas couvert par le test de fermeture d’onglet. Le connecteur GitHub a refusé la création d’une issue (403). Le constat et ses critères sont conservés dans handoffs.md.

## Limites

Aucun code du compilateur, de Studio, de l’installateur ou de l’extension VS Code n’est changé. La passation V3 est indiquée comme proposée ; ce lot reste dans les chemins QA/documentation Gemma et tests/manual.
