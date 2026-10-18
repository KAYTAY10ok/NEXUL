# 0012 — Copiabilité explicite des types publics (`Copy`)

Auteur : Claude (architecture, sémantique). **Statut : proposée** (17/10/2026). Change
le sens d'un programme selon l'édition : validation de l'utilisateur nécessaire
(AGENTS.md §2, rupture de compatibilité).

## Problème

Une valeur est **copiée** ou **déplacée** selon son type (spec §3.7, §5.2). Aujourd'hui
la catégorie est calculée **structurellement**, champs privés compris
(`Program::is_copy`, `compiler/nxc_sema/src/defs.rs`) : une structure est copiable si
tous ses champs le sont, sans `drop` ni `indirect`. Deux conséquences difficiles à
corriger après la 1.0 :

1. **Rupture invisible d'API.** L'auteur d'un paquet ajoute un champ privé `String` à
   une structure publique : sa signature publique ne change pas, mais chez tous ses
   utilisateurs `let b = a` devient un déplacement et `a` n'est plus utilisable
   (E0402). Un vérificateur de compatibilité (semver) ne peut pas le voir dans les
   signatures. L'inverse (retirer le dernier champ ressource) change silencieusement des
   déplacements en copies.
2. **Incohérence avec l'ADR 0010**, acceptée : elle écrit `fn T plus_grand[T is Ord and
   Copy](Vec[T] v)`, mais aucune interface `Copy` n'existe (`lib/std/core.nx`) ; dans un
   corps générique, `T` est toujours une ressource (spec §3.3), donc ce code ne peut pas
   être écrit aujourd'hui.

## Contexte

- Rust exige `#[derive(Copy, Clone)]` sur chaque type ; Swift infère `BitwiseCopyable`
  pour les types non publics et l'exige explicitement pour les types publics stables.
- NEXUL a des interfaces (traits) depuis la 0.2, des dérivations (`@derive`) et une
  édition 1 avec migration automatique (`nxc migrate`, ADR 0010).

## Solutions envisagées

| | Description | Coût | Risque |
|---|---|---|---|
| A | Garder l'inférence structurelle | nul | rupture d'API invisible ; pas de `T: Copy` |
| B | Déclaration obligatoire pour **tous** les types (comme Rust) | une annotation sur chaque petit type local | verbeux, contraire à la simplicité visée (cahier §1.3) |
| **C** | **Inférée pour les types privés à leur module, déclarée pour les types `pub`** ; interface `Copy` utilisable comme contrainte | annotation sur les seuls types publics copiables | deux règles à apprendre, mais une seule à retenir : « un type public dit s'il est copiable » |

## Décision proposée : C

1. `Copy` devient une interface standard **sans méthode** (marqueur) du prélude.
   Primitifs, `Ptr[T]`, types `fn`, tuples et tableaux de types `Copy` l'implémentent.
2. Un type **non public** est `Copy` s'il remplit les conditions structurelles
   actuelles (rien ne change pour le code privé).
3. Un type **public** n'est `Copy` que s'il le **déclare** : `@derive(Copy)` en 0.2,
   `type Point is Copy { … }` en édition 1. La déclaration est vérifiée : un champ non
   `Copy`, un `drop` ou un `indirect` est une erreur (nouveau code `E0250`, nom à
   confirmer).
4. Édition 1 seulement : un type public structurellement copiable **sans** déclaration
   est une ressource. Les fichiers en édition 0.2 gardent la règle actuelle (aucune
   rupture pour eux) ; `nxc migrate` ajoute `is Copy` à chaque type public aujourd'hui
   copiable, si bien que le passage à l'édition 1 ne change le sens d'aucun programme.
5. `T is Copy` (`T: Copy`) dans un générique permet de copier les valeurs de `T` dans
   le corps ; la monomorphisation est inchangée. `Clone` reste l'interface de la copie
   explicite et coûteuse (`.clone()`).
6. Retirer `is Copy` d'un type public est un changement incompatible visible dans sa
   déclaration ; l'ajouter est compatible.

## Conséquences

- Positives : la copiabilité d'un type public fait partie de son contrat écrit ; le
  futur vérificateur de compatibilité des paquets la voit ; `T is Copy` de l'ADR 0010
  devient réalisable ; pas de changement pour le code privé ni pour l'édition 0.2.
- Négatives : une annotation de plus sur les types publics copiables ; une règle
  différente selon `pub`, à expliquer dans le guide ; travail dans `derive.rs`,
  `defs.rs` (`is_copy` prend en compte la déclaration), `migrate.rs`, `ide.rs`
  (survol : afficher « copiable »).

## État de l'implémentation (18/10/2026)

- **Fait** (compatible, sans attendre la décision) : points 1 et 5. `Copy` est une
  interface du prélude (`lib/std/core.nx`) satisfaite par la structure du type ; `T:
  Copy` et `T is Copy` (édition 1) permettent de copier les valeurs de `T` ; un argument
  de type non copiable est refusé (E0249, avec un message propre à `Copy`). Tests :
  `tests/ui/copy_bound.nx`, `tests/run/copy_bound.nx`, `tests/run/edition1_copy_bound.nx`.
- **En attente de la décision** : points 2 à 4 et 6 (déclaration obligatoire pour les
  types publics en édition 1, `nxc migrate`, code E0250).

## Composants concernés

`compiler/nxc_sema/src/defs.rs` (`is_copy`), `derive.rs`, `collect.rs`,
`compiler/nxc/src/migrate.rs`, `lib/std/core.nx`, types publics de `lib/std`
(`Location`, `FormatSpec`, `Ordering`, `Weekday`, `Date`, `DateTime`…),
`docs/spec/03-types.md` §3.7, ADR 0010 §3.

## Tests et preuves attendus

- `tests/ui` : type public sans déclaration utilisé après affectation (E0402 en
  édition 1, accepté en 0.2) ; déclaration fausse (`E0250`) ; `T is Copy` dans un
  générique (copie acceptée) et sans la contrainte (E0402).
- `nxc migrate` : programmes de `tests/run` migrés, sortie identique (test
  `migrated_programs_behave_the_same`).
- `lib/std` compile sans changement de comportement (suite complète, avec LLVM).

## Risques

- Oublier `is Copy` sur un type public très utilisé rend son usage pénible : le
  message E0402 doit proposer « ajoutez `is Copy` à `Point` » quand c'est possible.
- Décision à prendre **avant** que l'édition 1 soit déclarée stable : après, elle
  imposerait une édition de plus.
