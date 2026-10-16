# Relectures de Claude Main

Une entrée par PR relue. « À corriger » devient une tâche de son propriétaire actuel
(équipe réduite du 15/10 : Codex Sol pour l'extension et l'installateur, Main ailleurs).

## PR #15 — audit G0 de la syntaxe (Outils) — 15/10/2026

**Acceptée.** Audit fait sur le vrai lexer et le vrai parser, recensement relançable, deux
pièges vérifiés avec `nxc check`. Les recommandations du §7 sont reprises dans G1.

Remarques :

1. Le piège §4.1 (ligne qui commence par `-`, `not`, `(`…) est grave : un programme mal
   découpé compile sans message. En plus de la grammaire G1, la 0.2 peut le signaler dès
   maintenant : une instruction-expression dont la valeur n'est pas `()` et n'est pas
   utilisée devrait être une erreur. → tâche de Main (compilateur), avant P6.
2. §4.2 (`Vret` devient une variable) : W0007 doit devenir une erreur dans la nouvelle
   syntaxe ; les trois propositions G1 le règlent par la grammaire.
3. Recommandation 4 (une seule source du lexique, `nxc ide lexicon`) : retenue ; à
   concevoir avec l'ADR G2.

## PR #16 — extension VS Code (Luna) — 15/10/2026

**Acceptée, deux corrections demandées** (propriétaire : Codex Sol).

1. **Sécurité — à corriger.** `nexul.nxcPath` n'a pas de `scope` : un dépôt peut le
   définir dans `.vscode/settings.json` et faire exécuter son propre programme à
   l'ouverture ou à l'enregistrement d'un `.nx` (`checkOnSave` est vrai par défaut). Corriger
   dans `package.json` : `"scope": "machine-overridable"` (ou `machine`) pour
   `nexul.nxcPath`, et déclarer `capabilities.untrustedWorkspaces`
   (`"supported": "limited"`, `"restrictedConfigurations": ["nexul.nxcPath"]`). Test :
   un réglage de dossier non approuvé n'est pas utilisé.
2. **Essais réels manquants** : `nxc` absent sur la machine de Luna ; diagnostics, build,
   run, test et format n'ont pas été essayés avec le vrai compilateur (tâche L4).
3. Bien : processus lancés sans shell (`shell: false`), formatage à l'enregistrement
   désactivé par défaut, aucune grammaire présentée comme finale.

## PR #14 — installateur, assistant Win32 (Outils) — 15/10/2026

**Acceptée** (fusionnée avant le transfert effectif à Luna puis à Codex Sol), **une
correction demandée** (propriétaire : Codex Sol).

1. **Sécurité — à corriger.** Pour les anciennes versions installées « pour la machine »,
   l'installateur écrit la liste des opérations (`del`, suppression de dossiers, registre)
   dans `%TEMP%\nexul-setup-<pid>\job.txt`, puis relance **le même programme avec
   élévation** qui exécute ce fichier (`old::run_elevated`, `old::job_main`). Le dossier
   est modifiable par tout programme de l'utilisateur : entre l'écriture et la lecture
   élevée, il peut remplacer les opérations et obtenir des suppressions avec les droits
   d'administrateur. Corriger : le processus élevé **refait lui-même l'inventaire** des
   anciennes versions (`old::scan`) et n'exécute que des opérations qu'il a calculées ; à
   défaut, il refuse toute opération hors des emplacements connus de NEXUL. Test : un
   `job.txt` modifié n'entraîne aucune suppression hors de ces emplacements.
2. Le mécanisme date de l'étape 1 (PR #11) ; l'étape 2 l'utilise tel quel.

## PR #17 — QA et documentation (Gemma) — 15/10/2026

**Acceptée.** Méthode honnête (ce qui est rapporté, non rejoué, à valider).

1. Vérifié par Main : `python tests/manual/run_smoke.py` → 3/3 réussis (`nxc` construit
   depuis `main`).
2. `quality-matrix.md` et `manual-studio-checklist.md` décrivent `main` à `548b1f6` : ils
   sont dépassés (fermeture avec dialogue, coloration et modèles raccordés, extension VS
   Code livrée). Main les met à jour avec l'essai manuel M19.
