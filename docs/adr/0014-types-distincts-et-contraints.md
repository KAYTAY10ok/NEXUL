# 0014 — Types distincts, types contraints et orientation des contrats

Auteur : Claude (architecture, sémantique). **Statut : proposée** (17/10/2026). Lot P5
(tâche O5) ; donne la sémantique des formes `distinct` et `where` que l'ADR 0010
(acceptée) a réservées et que `nxc` refuse aujourd'hui (E0111). Éclaire la décision 4 du
plan maître (contrats partout ou profil strict).

## Problème

La passation (§4.2, §4.3) demande des **types distincts** (ne pas mélanger des secondes et
des millisecondes, un identifiant d'utilisateur et un identifiant de projet) et des
**sous-types contraints** (plages simples, constantes vérifiées à la compilation, entrées
vérifiées à l'exécution), sans solveur complexe ni comportement caché, et sans « mode
release » qui retire les protections. L'ADR 0010 fixe l'écriture
(`type Metres = distinct f64`, `type Pourcent = i32 where it in 0..=100`, grammaire
`definition = "distinct" type [restriction] | type [restriction]`) mais pas le sens :
conversions, opérations, endroits où la condition est vérifiée, interaction avec `Copy`,
le filtrage et la frontière C. Sans ces règles, l'implémentation (P6) choisirait au cas
par cas, et chaque choix deviendrait un contrat public difficile à changer.

## Solutions envisagées

| | Idée | Problème |
|---|---|---|
| A | Distinct = alias avec avertissement | ne protège pas ; contraire à §4.3 |
| B | Distinct hérite de toutes les opérations et méthodes de sa base | `Metres * Metres` donne des `Metres`, `secondes.sqrt()` des secondes : faux sens sans le dire |
| C | Contraint = vérifié après **chaque** opération | vérifications cachées à chaque `+`, coût et paniques surprenantes |
| D | Types raffinés avec preuves (solveur SMT) | coût de complexité refusé par la passation |
| **E** | Distinct **n'hérite que ce qu'il déclare** ; contraint vérifié **seulement aux conversions** ; l'arithmétique rend le type de base | voir Décision |

## Décision proposée : E

### 1. Types distincts — `type Metres = distinct f64`

1. Nouveau type **nominal**, de même représentation et même disposition mémoire que sa
   base.
2. Aucune conversion implicite, dans aucun sens : `Metres` ↔ `f64` s'écrit `x as Metres`
   et `m as f64`. Ces conversions ne vérifient rien et ne coûtent rien.
3. **Littéraux** : un littéral sans type prend le type distinct attendu
   (`let Metres d = 3.5`) ; une **variable** de la base ne le fait jamais.
4. **Opérations** : un type distinct n'a d'abord **aucune** interface ni méthode. Il
   réutilise celles de sa base qu'il **nomme** :
   `type Metres = distinct f64 is Add and Sub and Eq and Ord and Display` (même forme
   `bornes` que les contraintes génériques ; la règle `definition` de l'EBNF de l'ADR 0010
   devient `"distinct" type [ "is" bornes ] [ restriction ]`). Chaque interface nommée est
   implémentée en délégant à la base, avec `Self` à la place de la base
   (`Metres + Metres → Metres`). Les autres opérations s'écrivent dans un `extend`
   (`fn Metres fois(self, f64 k)`). NEXUL 1.0 ne fait **pas** d'analyse dimensionnelle
   (`Metres * Metres` n'est un `Metres²` que si l'utilisateur le déclare).
5. Copiabilité : celle de la base, avec la règle de l'ADR 0012 (déclarée si le type est
   public).

### 2. Types contraints — `type Pourcent = i32 where it in 0..=100`

1. Une valeur de `Pourcent` est une valeur de `i32` qui **satisfait la condition**.
   Invariant : il n'existe aucune valeur de `Pourcent` qui ne la satisfait pas, dans du
   code sûr.
2. **Conditions permises (1.0)** : sur un type entier, flottant ou `char`, une expression
   sans effet formée de `it`, de constantes, de comparaisons, de `in` avec un intervalle
   constant, de `and`, `or`, `not`. Aucun appel de fonction. Le compilateur peut donc
   l'évaluer à la compilation sur une constante et en quelques instructions à
   l'exécution. Une condition hors de ce sous-ensemble est une erreur.
3. **Entrées** (les seuls endroits vérifiés) :
   - constante ou littéral : vérifiés **à la compilation** (nouveau code, à la manière de
     E0301) ;
   - `x as Pourcent` : vérifié **à l'exécution**, panique si la condition est fausse
     (comme `as` entre entiers, spec §3.6) ;
   - `Pourcent.check(x) → Option[Pourcent]` (fonction engendrée, nom à confirmer) pour les
     entrées attendues invalides (saisies, fichiers).
4. **Sortie** : `Pourcent` se convertit **implicitement** en `i32` (un `Pourcent` est un
   `i32`) quand le type contraint n'est pas aussi `distinct`.
5. **Opérations** : l'arithmétique opère sur la base et rend la **base**
   (`p + 1` est un `i32`). Réaffecter à un `Pourcent` demande `as Pourcent`. Aucune
   vérification cachée : la seule panique possible est à l'endroit où le programme écrit
   `as`.
6. `distinct` et `where` se combinent (`type Age = distinct u8 where it <= 150`) : alors
   pas de conversion implicite vers la base (règle 1.2), et l'entrée est vérifiée.
7. **Frontière C** : un type contraint dans une signature `@export` est vérifié à
   l'entrée (même règle que `bool` et `char`, ADR 0011 T4).
8. **Filtrage** (après 1.0, compatible) : l'exhaustivité peut utiliser la condition
   (`when p { 0..=50 then … 51..=100 then … }` sans `else`).
9. **Toujours actives** : aucune option ne retire ces vérifications (passation §4.3).

### 3. Invariants des types et contrats (orientation pour la décision 4)

- **Invariants des types** : en 1.0, par l'encapsulation, qui existe déjà. Les champs
  sont privés, la construction passe par des fonctions du type, et la loi d'exclusivité
  empêche toute modification cachée pendant un emprunt. Pas de mot `invariant` en 1.0.
- **Contrats** (`requires`, `ensures`, réservés par l'ADR 0010) : recommandation
  « **partout, vérifiés à l'exécution dans tous les modes** ». Les conditions suivent le
  même sous-ensemble sans effet que §2.2, étendu aux paramètres et à `self` ; elles sont
  prouvées à la compilation seulement quand elles portent sur des constantes. En profil
  temps réel (P8), elles ne doivent ni allouer ni bloquer. Une ADR dédiée fixera la
  syntaxe des postconditions (`result`) avant l'implémentation.

## Conséquences

- Positives : mélanger deux unités est une erreur de compilation ; aucune vérification
  ni allocation cachée ; aucun solveur ; la sémantique existante (`as` vérifié, panique)
  est réutilisée ; une seule notion nouvelle à apprendre, la clause `is` qui nomme ce
  qu'on réutilise.
- Négatives : écrire `as Pourcent` après un calcul ; déclarer les interfaces réutilisées ;
  pas d'unités composées.

## Composants concernés

`compiler/nxc_syntax/src/parser/ed1.rs` (lever E0111 pour `distinct`/`where`),
`compiler/nxc_sema` (nouveau `TyKind` ou ADT nominal à base primitive, conversions,
délégation des interfaces, évaluation des conditions dans `consts.rs`),
`compiler/nxc_nir` (vérification aux conversions), `compiler/nxc_codegen_llvm` (aucun
changement de disposition), `docs/spec/03-types.md`, `compiler/nxc/src/migrate.rs` (rien
à migrer : formes nouvelles).

## Tests et preuves attendus

- `tests/ui` : `Secondes + Millisecondes` refusé ; variable de base affectée à un distinct
  refusée ; littéral accepté ; constante hors condition refusée ; condition avec appel
  refusée ; `p + 1` affecté à un `Pourcent` sans `as` refusé.
- `tests/run` : `x as Pourcent` hors condition → panique, code 101, position exacte, en
  debug **et** en `-O2` ; `Pourcent.check` rend `None`.
- Taille et disposition identiques à la base (`mem.size_of`).

## Risques

- La conversion implicite `Pourcent → i32` peut surprendre dans une surcharge générique :
  à valider sur le corpus O5 (12 programmes de la passation).
- Ergonomie de `as` après chaque calcul : mesurer sur deux programmes réels avant de figer.
