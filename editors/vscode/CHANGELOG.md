# Historique des versions

## 0.3.0

- Prend en charge l’édition 1 en plus de NEXUL 0.2, avec détection par en-tête ou
  `nexul.toml` et grammaire dédiée.
- Utilise le serveur `nxc lsp` pour les diagnostics pendant la saisie, les symboles,
  les références, la complétion et les CodeLens de tests.
- Formate automatiquement les fichiers NEXUL 0.2 et édition 1 à l’enregistrement via
  `nexul.formatOnSave` (activé par défaut), et conserve la commande de formatage manuel.
- Ajoute les commandes et tâches de vérification, compilation, exécution et tests avec
  le compilateur `nxc` configuré.
- Respecte Workspace Trust avant de démarrer le compilateur ou d’exécuter une commande.
- Ajoute l’icône de l’extension et conserve les licences MIT et Apache-2.0.
