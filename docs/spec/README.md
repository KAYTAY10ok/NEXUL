# Spécification du langage NEXUL — version 0.2

Ce dossier contient la **spécification formelle** de NEXUL. Elle transforme le
[cahier des charges](../cahier-des-charges.md) en règles précises, vérifiables et testées.

NEXUL 0.1 était volontairement un **noyau** : un sous-ensemble cohérent, entièrement
spécifié et entièrement implémenté (cahier des charges §20 : « limiter la première
grammaire »). La version 0.2 l'étend sans le remettre en cause : traits et contraintes
génériques, répartition dynamique (`dyn`), fonctions anonymes, `Map` et `Set`, variables
statiques, compilation conditionnelle, export de fonctions vers C, cibles sans système
d'exploitation, et un compilateur autonome (backend LLVM, runtime écrit en NEXUL). Les
mentions « Hors 0.1 » signalent ce qui reste à venir.

| Chapitre | Contenu |
|---|---|
| [01 — Structure lexicale](01-lexique.md) | Encodage, jetons, commentaires, littéraux, fins d'instruction |
| [02 — Grammaire](02-grammaire.md) | Grammaire EBNF complète |
| [03 — Types](03-types.md) | Types primitifs, composés, génériques, inférence, conversions, traits |
| [04 — Expressions et instructions](04-expressions.md) | Sémantique, ordre d'évaluation, opérateurs, contrôle de flux, motifs, fonctions anonymes |
| [05 — Modèle mémoire](05-memoire.md) | Propriété, déplacements, emprunts, destruction, exclusivité, `unsafe` |
| [06 — Modules](06-modules.md) | Fichiers, imports, visibilité, prélude |
| [07 — Erreurs](07-erreurs.md) | Paniques, `Option`/`Result`, opérateur `?`, diagnostics |
| [08 — Interopérabilité C](08-interop-c.md) | `extern "C"`, `@export`, bibliothèques, `Ptr[T]`, types compatibles |
| [09 — Bibliothèque standard](09-bibliotheque.md) | Prélude, traits standards, `String`, `Vec`, `Map`, `Set`, `std.*` |
| [10 — Compilateur `nxc`](10-compilateur.md) | Pipeline, IR, backend LLVM, runtime, cibles, ligne de commande, tests |
| [11 — Tâches et concurrence](11-taches.md) | `Transfer`, `Share`, `tache.portee`, `lancer`, exclusivité entre tâches, threads |
| [12 — Effets et temps réel](12-temps-reel.md) | `with noalloc, nonblocking` ; tâches périodiques, interruptions, ressources à plafond ; pile bornée (à venir) |

## Conventions

- Les mots **DOIT**, **NE DOIT PAS**, **PEUT** ont le sens normatif habituel.
- Les fragments de grammaire utilisent l'EBNF décrite au chapitre 02.
- Un exemple marqué `// erreur E0xxx` est un programme que le compilateur **doit** rejeter
  avec ce code de diagnostic. Ces exemples sont repris dans la suite de tests `tests/ui/`.
- Chaque règle normative est couverte par au moins un test (`tests/run/` ou `tests/ui/`).

## Principes de conception (rappel du cahier des charges)

1. **Une seule façon évidente** de faire chaque chose (§4.1).
2. **Rien d'implicite qui surprenne** : pas de conversion numérique implicite, pas de
   copie coûteuse cachée, pas de valeur nulle, pas de comportement indéfini hors `unsafe`.
3. **Sûr par défaut, contrôlable explicitement** : la mémoire est vérifiée statiquement ;
   le code non sûr est délimité par `unsafe`.
4. **Performance native** : compilation anticipée, pas de ramasse-miettes, coût des
   abstractions visible.
5. **Diagnostics pédagogiques** : chaque erreur a un code, une localisation précise et,
   quand c'est possible, une suggestion de correction.

## Statut des fonctionnalités

Chaque chapitre indique les fonctionnalités **prévues pour des versions ultérieures**
(marquées *« Hors 0.1 »*). Elles sont citées pour montrer que la conception du noyau
leur réserve déjà une place (mots-clés réservés, grammaire extensible).
