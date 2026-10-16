# Checklist manuelle — NEXUL Studio

Les scénarios ci-dessous restent à exécuter, sauf indication contraire. Ils définissent des critères, pas des fonctions certifiées.

## Fiche de session

- Révision / branche : `a0ef361`, `refonte/claude-studio` (= `main`)
- Binaire testé et chemin : `studio\app\target\release\NEXUL-Studio-Native.exe`, construit le 16/10/2026 08:18 (MSVC, CRT statique)
- Windows / version : Windows 11 Pro 10.0.26200
- Compilateur nxc et version : `target\release\nxc.exe` du même commit (`nxc 0.2.0`, éditions 0.2 et 1), donné par `NEXUL_NXC`
- Configuration isolée NEXUL_STUDIO_CONFIG : dossier jetable du bloc-notes de session ; projet jetable `proj` (édition 1 par `nexul.toml`)
- Date / personne : 16/10/2026, Claude Main (pilotage de l'application avec l'accord de l'utilisateur)
- Logs ou captures : captures d'écran de la session ; contenu disque vérifié après chaque étape

## P0 — documents et projets

Essai du 16/10 (M19) : **tout le bloc P0 passe** dans la vraie application.

- [x] **Fermeture de fenêtre avec un document modifié** : dialogue Enregistrer / Ne pas enregistrer / Annuler. Annuler garde la fenêtre et le texte ; avec deux onglets modifiés, un seul dialogue liste les deux fichiers et « Tout enregistrer » écrit les deux puis ferme ; un fichier en lecture seule fait échouer l'enregistrement, affiche l'erreur et **annule la fermeture**.
- [x] **Fermeture d’onglet modifié** : dialogue « Enregistrer les modifications de « b.nx » ? » ; Ne pas enregistrer ferme l'onglet et garde la version disque.
- [x] **Fermeture forcée / redémarrage** : processus tué avec un document modifié → au redémarrage, « Modifications non enregistrées de « a.nx » récupérées » ; le fichier a ensuite été changé sur disque pendant que Studio était fermé : le texte récupéré n'écrase rien, l'enregistrement ouvre le dialogue de conflit (Recharger prend la version disque). Les fichiers de récupération disparaissent après l'enregistrement.
- [x] **Modification externe** : document propre → rechargé tout seul ; document modifié → Ctrl+S ouvre « a été modifié en dehors de Studio » (Recharger / Annuler / Écraser) ; Annuler laisse le disque intact, Écraser écrit la version de Studio. Aucun fichier temporaire laissé (écriture atomique).
- [x] **Erreur d’écriture** : fichier en lecture seule → message « est en lecture seule : retirez cet attribut… », document gardé modifié, fermeture annulée.
- [x] **Changement de dossier** (Nouveau projet pendant qu'un document est modifié) : « Enregistrer les modifications de « a.nx » avant de changer de dossier ? » ; Annuler reste dans le dossier courant. Le projet demandé est quand même créé sur disque (seule son ouverture est annulée).

Écarts vus pendant l'essai (hors P1) :

- La complétion s'ouvre en tapant dans un commentaire (`// modif b` propose `b`, `println`…).
- Studio prend `nxc` dans `NEXUL_HOME` avant `PATH` : avec l'installation 0.2 de l'utilisateur, un projet d'édition 1 est coloré « NEXUL 1 » mais analysé par un `nxc` qui ne connaît pas l'édition 1 (fausses erreurs). Studio doit vérifier que son `nxc` connaît l'édition du fichier et le dire.
- `nxc` refuse un `nexul.toml` enregistré avec BOM UTF-8 (Bloc-notes, PowerShell 5) : « nexul.toml:1: expected `key = value` ».
- Ouvrir un dossier : le sélecteur de Windows ne peut pas être piloté au clavier par l'outil d'essai ; le chemin a été vérifié par « Nouveau projet ».

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
  - 16/10 : un projet `edition = "1"` s'ouvre en « NEXUL 1 » (barre d'état) ; `when`, `then`, `else` colorés en mots-clés ; aucun diagnostic avec le `nxc` du commit.
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
