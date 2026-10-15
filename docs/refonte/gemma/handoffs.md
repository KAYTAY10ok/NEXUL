# Passations QA aux propriétaires

## Studio natif — fermeture de fenêtre et modifications non enregistrées

**Destinataire selon la charte présente sur main : propriétaire de studio/app (Claude Studio).** La passation V3 proposée renomme ce rôle ; ce rapport ne modifie pas la charte ni la propriété des fichiers.

**Constat statique au commit 548b1f6 :**

- studio/app/src/main.rs termine l’application avec cx.on_window_closed puis cx.quit().
- studio/app/src/studio.rs enregistre automatiquement un document modifié dans close_doc avant de supprimer l’onglet.
- Le test closing_a_modified_file_saves_it couvre la fermeture d’un onglet, pas de la fenêtre.
- Aucun dialogue Save / Don’t Save / Cancel avant la fermeture de la fenêtre n’a été trouvé.

**Demande :** avant de quitter avec des documents modifiés, proposer Enregistrer / Ne pas enregistrer / Annuler. Ne quitter après Enregistrer que si toutes les écritures réussissent ; garder fenêtre et texte après Annuler ou erreur.

**Validation souhaitée :** test d’un et plusieurs onglets ; vérifier le contenu disque après chaque choix ; une erreur de sauvegarde empêche la fermeture.

**Transmission GitHub :** après autorisation de l’utilisateur, la création de l’issue et de la PR brouillon a été tentée ; le connecteur a renvoyé HTTP 403, « Resource not accessible by integration ». Aucune issue ni PR n’a été créée. La branche contenant ce handoff est poussée sur refonte/gemma-qa-docs ; page de création de PR : https://github.com/KAYTAY10ok/NEXUL/pull/new/refonte/gemma-qa-docs. Le propriétaire peut reprendre ce texte pour ouvrir l’issue dans le dépôt.
