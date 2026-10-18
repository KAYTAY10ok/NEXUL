# Contrats entre les composants du compilateur

Statut : **proposé** par Claude (architecture, sémantique, sûreté) le 17/10/2026.
Ce document dit ce que chaque étage de `nxc` **garantit** à l'étage suivant et ce que
celui-ci **ne doit pas** faire. Il sert de référence quand un agent modifie un composant
dont un autre dépend (mission maîtresse §4.3) : un changement qui casse une clause passe
par une revue sémantique (Claude) et, s'il est visible des utilisateurs, par une ADR.

Pipeline : spec §10.1. Numéros des clauses : stables, à citer dans les PR (« respecte K3 »).

## K1 — Syntaxe → analyse (`nxc_syntax` → `nxc_sema`)

- K1.1 Chaque fichier porte son **édition** (ADR 0010) ; l'AST est commun aux éditions :
  deux écritures de même sens donnent le même AST (vérifié par `nxc migrate` et le test
  `migrated_programs_behave_the_same`).
- K1.2 Les erreurs de syntaxe sont des diagnostics, jamais des plantages ; un AST partiel
  marque ses trous (`Error`) pour éviter les erreurs en cascade.

## K2 — Analyse → NIR (`nxc_sema` → `nxc_nir`)

- K2.1 La NIR n'est construite que si l'analyse n'a produit **aucune erreur**.
- K2.2 Le THIR est entièrement typé (aucune variable d'inférence), les méthodes et
  opérateurs sont résolus, les filtrages sont exhaustifs.
- K2.3 Les règles de places de la spec ch. 05 (E0403, E0405, E0410, E0420) sont vérifiées
  sur le THIR ; l'initialisation et les déplacements (E0401, E0402) sur la NIR. Une règle
  ajoutée au langage dit où elle est vérifiée.
- K2.4 Les entiers sont vérifiés pour la **cible** (`Program::int_bits`, `int_min`,
  `int_max`) : une constante qui arrive à la NIR tient dans son type sur la cible.

## K3 — NIR → génération de code (`nxc_nir` → `nxc_codegen_llvm`)

Ce que la NIR garantit :

- K3.1 **Ordre d'évaluation** : l'ordre des instructions et des blocs est l'ordre de la
  spec §4.1. Aucun effet observable n'est implicite.
- K3.2 **Destructions élaborées** : après l'analyse de propriété, il ne reste aucun
  `DropIfInit` ; chaque `Drop` détruit une valeur initialisée, exactement une fois sur
  chaque chemin.
- K3.3 Les vérifications de bornes sont des instructions (`BoundsCheck`) ; les
  vérifications arithmétiques sont **portées par les opérations** (`Binary`, `Unary`,
  `Cast` sur des entiers).

Ce que le générateur doit faire et ne doit pas faire :

- K3.4 Émettre la vérification de **toute** opération entière vérifiée (`+ - * / %`,
  `<< >>`, négation, `as`, `abs`, `pow`) à **tous** les niveaux d'optimisation ; seules
  les opérations `+% -% *%` enroulent.
- K3.5 Flottants IEEE 754 stricts : aucun drapeau *fast-math*, aucune contraction
  (`fmul` + `fadd` → FMA) non demandée par le source.
- K3.6 Ne déduire **aucun** attribut d'alias (`noalias`, `readonly`, `nocapture`,
  `dereferenceable` au-delà de la taille du type) de la loi d'exclusivité sans ADR : une
  erreur de l'analyse deviendrait une mauvaise compilation, et une `static var` modifiée
  en `unsafe` pendant un emprunt contredit `noalias`.
- K3.7 Ne jamais ajouter, retirer ou déplacer une destruction ; un `DropIfInit` résiduel
  est une **erreur interne** (`func.rs` s'arrête avec `internal: DropIfInit…` au lieu de
  détruire sans condition).
- K3.8 **Déterminisme** : même programme, même cible, mêmes options → IR identique à
  l'octet (aucun parcours de table de hachage non trié).
- K3.9 Disposition mémoire : seules celles des primitifs, de `Ptr[T]` et des tableaux
  sont garanties (spec §3.8) ; les autres peuvent changer à chaque version.
- K3.10 Un bloc qu'on ne peut pas atteindre depuis l'entrée suit une expression de type
  `Never` (`f(x, panic("…"))`, `1 + return`) et peut en utiliser la « valeur », qui n'a
  pas de type machine : le générateur n'émet pour lui qu'`unreachable`.

## K4 — Génération de code ↔ runtime (`lib/std`)

- K4.1 Les fonctions et types du runtime utilisés par le générateur sont les éléments
  `@lang("…")` de `lib/std` ; en ajouter un ou changer sa signature modifie les deux côtés
  dans la même PR.
- K4.2 `panic*` ne revient jamais ; elle vide la sortie standard, écrit le message et la
  position, termine avec le code 101 sans exécuter de destructeur (ADR 0011 §4).
- K4.3 L'allocation ne rend jamais un pointeur nul : mémoire épuisée = panique.
- K4.4 Les blocs `unsafe` de `lib/std` respectent la spec §5.7 ; chaque nouveau bloc a un
  commentaire qui dit pourquoi il est correct (condition C1 de l'ADR 0011).
- K4.5 Aucune API sûre ne modifie une valeur à travers un emprunt en lecture
  (invariant I1 de l'ADR 0013).

## K5 — Cibles

- K5.1 Une cible est une entrée de `TARGETS` (`nxc_sema/src/target.rs`) ; `ptr_bits` y est
  la source unique de la largeur de `isize`/`usize` pour l'analyse et le générateur.
- K5.2 Une cible n'est dite **prise en charge** qu'avec sa couche système (`lib/std/sys`),
  la suite `tests/run` exécutée sur elle (`NEXUL_TEST_TARGETS`) et une ligne dans la
  matrice des plateformes ; sinon elle est marquée expérimentale (macOS aujourd'hui).

## K6 — Diagnostics et sorties pour les outils

- K6.1 Un code (`E0403`) ne change jamais de sens ; chaque code émis a son explication
  (`codes.rs`) et sa ligne dans `diagnostics.md` (tests `every_code_is_explained`,
  `diagnostics_catalogue_is_up_to_date`).
- K6.2 Les sorties JSON sont versionnées (ADR 0009) : un champ ne disparaît pas dans une
  même version de schéma.
- K6.3 `nxc` ne plante sur aucune entrée ; tout plantage est un défaut. Les analyses sont
  récursives : `nxc` (et `nxc lsp`) les exécutent sur une pile de 256 Mio, et les limites
  d'E0113 (1000 niveaux d'imbrication, 10 000 opérateurs à la suite) tiennent dans cette
  pile avec une large marge (50 000 niveaux mesurés sans dépassement). Un outil qui
  appelle `nxc_driver::compile_source` dans son propre processus fournit une pile aussi
  grande. Preuve : `compiler/nxc/tests/robustness.rs`.

## K7 — Tests

- K7.1 Chaque règle normative de la spécification a un test (`tests/ui` ou `tests/run`)
  cité dans le texte quand c'est utile. Chaque code de diagnostic est produit par au
  moins un test de `tests/ui` (vérifié par `every_code_has_a_ui_test`, sauf l'erreur
  interne E0900).
- K7.2 Les tests UI peuvent fixer l'édition (`// edition: 1`) et la cible
  (`// target: wasm32-wasi`) sur l'une de leurs trois premières lignes.
- K7.3 Les garanties tiennent à tous les niveaux d'optimisation : `tests/run` est
  exécuté sans optimisation (`run_programs`) **et** en `-O2` (`run_programs_optimized`),
  avec les mêmes sorties attendues (`@cfg` reste `debug`).

## Responsabilités

| Clauses | Implémentation | Revue sémantique |
|---|---|---|
| K1, K6 (outils, diagnostics), K7 (infrastructure) | Codex | Claude pour le sens des messages |
| K2 | Claude | Codex (faisabilité, performance) |
| K3, K4, K5 | Codex | **Claude obligatoire** (sûreté) |
