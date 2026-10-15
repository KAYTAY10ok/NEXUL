# Checklist manuelle — NEXUL Studio

Les scénarios ci-dessous restent à exécuter, sauf indication contraire. Ils définissent des critères, pas des fonctions certifiées.

## Fiche de session

- Révision / branche :
- Binaire testé et chemin :
- Windows / version :
- Compilateur nxc et version :
- Configuration isolée NEXUL_STUDIO_CONFIG :
- Date / personne :
- Logs ou captures :

## P0 — documents et projets

- [ ] **Fermeture de fenêtre avec un document modifié** : modifier un fichier puis fermer la fenêtre. Attendu : choix Enregistrer / Ne pas enregistrer / Annuler ; Annuler garde fenêtre et texte ; Enregistrer persiste le texte ; Ne pas enregistrer garde la version disque. Refaire avec deux onglets ; une erreur d’écriture empêche la fermeture.
  - Revue source à 548b1f6 : écart probable confirmé dans le code, voir handoffs.md ; pas de reproduction interactive.
- [ ] **Fermeture d’onglet modifié** : vérifier le comportement présenté à la personne ; le code actuel enregistre sans dialogue.
- [ ] **Fermeture forcée / redémarrage** : vérifier qu’une session restaurée ne remplace pas un contenu disque plus récent.
- [ ] **Modification externe** : modifier le même fichier dans Studio et un autre éditeur ; aucun écrasement silencieux.
- [ ] **Erreur d’écriture** : fichier en lecture seule / dossier sans droits ; message clair, document gardé en mémoire et fermeture annulée.

## Explorateur et éditeur

- [ ] Créer fichier et dossier ; annuler la saisie ; essayer un nom invalide et un nom réservé Windows.
- [ ] Renommer par F2, y compris un changement de casse uniquement.
- [ ] Copier/coller et couper/coller ; tester collision de nom et absence d’écrasement.
- [ ] Déplacer un dossier dans lui-même ; opération refusée sans perte.
- [ ] Glisser depuis l’Explorateur Windows ; contrôler fichier texte et binaire.
- [ ] Supprimer puis restaurer depuis la Corbeille dans une VM ou un dossier jetable.
- [ ] Ouvrir plusieurs fichiers, redémarrer, vérifier état et sélection restaurés.

## Langage, diagnostics et commandes

- [ ] Ouvrir examples/bonjour.nx ; contrôler coloration, indentation, Unicode, absence de modification spontanée.
- [ ] Créer une erreur de type ; confirmer code, plage, message et lien d’aide ; corriger et vérifier la disparition.
- [ ] Tester complétion, survol, définition, références, renommage et formatage.
- [ ] Lancer F5 et arrêter avec Maj+F5 ; contrôler code de sortie, Unicode et saisie interactive.
- [ ] Lancer les fonctions @test ; vérifier qu’un échec volontaire est visible dans un projet jetable.
- [ ] Construire un exécutable et une cible non native ; vérifier les diagnostics quand un outil manque.

## Briques séparées ou non raccordées

- [ ] Coloration : interpolation, commentaire multiligne, saisie incomplète, erreur lexicale ; comparer aux diagnostics.
- [ ] LSP : serveur présent et absent ; le second cas annonce « coloration seulement » et ne télécharge rien.
- [ ] Terminal : commande interactive, redimensionnement, ANSI, accents, CJK, emoji, fermeture du shell et second terminal.
- [ ] DAP : point d’arrêt, pile, variables, pas à pas, arrêt et fin ; consigner l’adaptateur.
- [ ] Modèles : génération, build/run conditionnels, destination existante ; aucun écrasement.
- [ ] WebAssembly navigateur : exemple ABI v1, erreur de compilation visible, imports explicites, instance après trap ; attend le raccordement à l’aperçu Studio.
- [ ] VS Code : non applicable à main 548b1f6 ; attendre un VSIX avant de suivre le guide brouillon.

## Installation et accessibilité

- [ ] Installation neuve dans une VM ; démarrer l’application native, ouvrir .nx, vérifier raccourcis et découverte de nxc.
- [ ] Mise à niveau d’une ancienne version ; conserver projets et réglages, contrôler PATH et association .nx.
- [ ] Désinstallation ; retirer binaires/raccourcis sans retirer projets, configuration ou cache utilisateur.
- [ ] Clavier seul : menus, explorateur, onglets, diagnostics, terminal et fermeture.
- [ ] Narrateur Windows : nom, rôle, état, annonces de diagnostic, dialogues, ordre de tabulation.
- [ ] Thèmes clair/sombre et contraste élevé ; aucune information de diagnostic portée par la couleur seule.

Ne pas tester migration, désinstallation ou Corbeille sur l’installation réelle de l’utilisateur. Utiliser une VM ou un profil dédié.
