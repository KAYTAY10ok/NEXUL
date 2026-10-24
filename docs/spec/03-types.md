# 03 — Types

NEXUL est **statiquement typé**. Chaque expression a un type connu à la compilation.
Les signatures (paramètres, type de retour, champs, constantes) sont **toujours annotées** ;
l'inférence ne s'applique qu'à l'intérieur des corps de fonctions. Ainsi une signature se
lit sans consulter son implémentation, et la compilation séparée reste possible.

## 3.1 Types primitifs

| Type | Représentation | Valeurs |
|---|---|---|
| `i8` `i16` `i32` `i64` | complément à deux, 8/16/32/64 bits | −2ⁿ⁻¹ … 2ⁿ⁻¹−1 |
| `u8` `u16` `u32` `u64` | binaire non signé | 0 … 2ⁿ−1 |
| `isize` `usize` | taille d'un pointeur de la cible (32 ou 64 bits) | idem selon la largeur |
| `f32` `f64` | IEEE 754 binary32 / binary64 | y compris ±inf et NaN |
| `bool` | 1 octet, 0 ou 1 | `true`, `false` |
| `char` | 32 bits | valeur scalaire Unicode (U+0000–U+10FFFF hors U+D800–U+DFFF) |
| `()` | 0 octet utile | l'unique valeur `()` (« unité ») |
| `Never` | aucune | type des expressions qui ne terminent pas (`return`, `panic(...)`) |

La largeur de `isize` et `usize` est celle de la **cible de compilation** (`nxc targets`) :
littéraux, constantes, `MIN`, `MAX`, `BITS` et calculs à la compilation sont vérifiés
pour cette cible. Un programme peut donc être valide pour une cible 64 bits et refusé
pour une cible 32 bits (`let n: usize = 5_000_000_000` : E0301 sur `wasm32-wasi`) ; une
valeur qui doit dépasser 32 bits partout s'écrit en `u64` ou `i64`.

Il n'existe **pas de valeur nulle** : l'absence de valeur s'exprime par `Option[T]`.

Constantes associées : `MIN`, `MAX` et `BITS` pour chaque entier (`i32.MIN`, `u8.MAX`) ;
`INFINITY`, `NEG_INFINITY`, `NAN`, `EPSILON`, `MAX`, `MIN` et `MIN_POSITIVE` pour `f32` et
`f64` ; `char.MAX`.

## 3.2 Types composés

| Forme | Exemple | Description |
|---|---|---|
| Tuple | `(i32, String)` | produit anonyme ; accès `t.0`, `t.1` ; `(T,)` pour un 1-uplet |
| Tableau | `[u8; 16]` | `N` éléments contigus, `N` constante entière ≥ 0 connue à la compilation |
| Structure | `struct Point { x: f64, y: f64 }` | produit nommé |
| Énumération | `enum Forme { Cercle(f64), Vide }` | somme étiquetée (union discriminée) |
| Fonction | `fn(i32, inout String) -> bool` | pointeur vers une fonction nommée ; en paramètre, accepte aussi une fonction anonyme (§4.12) |
| Objet de trait | `dyn Forme` | valeur de n'importe quel type qui implémente le trait, appelée par répartition dynamique (§3.9) |
| Pointeur brut | `Ptr[T]` | adresse non vérifiée, utilisable seulement en `unsafe` (§05, §08) |

### Structures

```nexul
struct Personne {
    pub nom: String
    pub age: u32
    identifiant: u64        // privé au module
}

let p = Personne(nom: "Ada", age: 36, identifiant: 1)
```

La construction utilise la syntaxe d'appel avec **tous les champs nommés, dans l'ordre de
déclaration** (E0310 sinon). Cette contrainte rend l'ordre d'évaluation évident et la
lecture uniforme. Les champs sont privés au module par défaut ; un champ `pub` est
accessible partout où le type l'est. Une structure dont un champ n'est pas visible ne
peut pas être construite hors de son module (on fournit alors une fonction de construction).

### Énumérations

```nexul
enum Forme {
    Cercle(f64)
    Rectangle(f64, f64)
    Vide
}

let f = Forme.Rectangle(3.0, 4.0)
```

Les variantes portent zéro ou plusieurs valeurs positionnelles. Les variantes sont
toujours publiques si l'énumération l'est.

### Stockage indirect (`indirect`)

Un type récursif aurait une taille infinie. Le modificateur `indirect` place la valeur
dans un stockage alloué sur le tas, **de façon transparente** : on lit, construit et filtre
la valeur exactement comme si elle était stockée directement.

```nexul
enum Arbre {
    Feuille(i64)
    indirect Noeud(Arbre, Arbre)
}

struct Maillon {
    valeur: i32
    indirect suivant: Option[Maillon]
}
```

Un type récursif sans `indirect` sur le chemin de récursion est rejeté (E0311) avec une
suggestion indiquant où placer `indirect`. Un type contenant du stockage indirect est un
type *ressource* (§05) : sa destruction libère le stockage.

## 3.3 Génériques

```nexul
struct Paire[A, B] {
    pub premier: A
    pub second: B
}

fn echanger[A, B](owned p: Paire[A, B]) -> Paire[B, A] {
    return Paire(premier: p.second, second: p.premier)
}
```

- Fonctions, structures et énumérations peuvent avoir des paramètres de type.
- Un bloc `impl Paire { ... }` voit automatiquement les paramètres `A` et `B` du type.
- Les arguments de type sont **inférés** aux sites d'utilisation ; on peut les donner
  explicitement avec des crochets : `Vec[i32].new()`, `taille_de[u64]()`.
- Implémentation par **monomorphisation** : chaque instanciation produit du code
  spécialisé, sans indirection à l'exécution.
- Sans contrainte, une valeur de type `T` peut être déplacée, passée, stockée et
  détruite, mais pas comparée ni additionnée. Les **contraintes de traits** (§3.9),
  `[T: Ord + Clone]` ou `where T: Ord`, donnent accès aux méthodes et opérateurs des
  traits nommés ; à chaque appel, les arguments de type doivent les implémenter (E0249).
- Dans un corps générique, `T` est traité comme un type *ressource* (non copiable),
  ce qui garantit que le corps est valide pour **toute** instanciation, sauf s'il porte
  la contrainte **`Copy`** (`[T: Copy]`, `T is Copy` en édition 1) : ses valeurs sont
  alors copiées (`var best = v[0]`, `return (x, x)`), et chaque argument de type doit être
  copiable (E0249). Test : `tests/run/copy_bound.nx`.

## 3.4 Alias, types distincts et types contraints

`type Metres = f64` introduit un second nom pour un type existant. Ce n'est **pas** un
nouveau type : `Metres` et `f64` sont interchangeables.

L'édition 1 ajoute deux formes (ADR 0014), sur une **base** qui est un nombre, `char` ou
`bool` ; la valeur a la représentation et la disposition mémoire de sa base
(`mem.size_of[Metres]() == mem.size_of[f64]()`) :

```
type Metres = distinct f64 is Copy and Add and Sub and Eq and Ord and Display
type Pourcent = i32 where it in 0..=100
type Age = distinct u8 is Copy and Eq and Display where it <= 150
```

**Type distinct** (`distinct`) : un nouveau type, sans conversion implicite dans aucun
sens. `x as Metres` et `m as f64` convertissent sans rien vérifier ; un **littéral** prend
le type distinct attendu (`let Metres d = 3.5`), une variable de la base jamais (E0302).
Il n'a que les interfaces de sa base qu'il **nomme** après `is` (`Copy`, `Eq`, `Ord`,
`Display`, `Hash`, `Clone`, `Default`, `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`), réalisées
en déléguant à la base avec le type distinct à la place (`Metres + Metres` donne des
`Metres`) ; les autres opérations s'écrivent dans `extend Metres { … }`. Pas d'analyse
dimensionnelle : `Metres * Metres` n'existe que si on le déclare. Un type distinct public
n'est copiable que s'il nomme `Copy` (§3.7).

**Type contraint** (`where`) : ses valeurs satisfont la **condition**, qui compare `it`
avec des constantes littérales (`it >= 0`, `it in 1..=9`, `it in 0..10`, combinées par
`and`, `or`, `not`), sans appel ni variable (E0251). Il n'existe aucune valeur de
`Pourcent` qui ne la satisfait pas dans du code sûr ; la condition est vérifiée aux
**entrées** seulement :

| Entrée | Vérification |
|---|---|
| littéral ou constante où `Pourcent` est attendu | à la compilation (E0252) |
| `x as Pourcent` | à l'exécution : panique à la position de `as` si la condition est fausse |
| `Pourcent.check(x)` | rend `Option[Pourcent]` (`None` si la condition est fausse) |

Un type contraint qui n'est pas `distinct` **est** une valeur de sa base : il s'y convertit
implicitement (argument, opérande, récepteur des méthodes de la base), et il a les
interfaces de sa base (`Copy`, `Eq`, `Ord`, `Display`, `Hash`, `Clone`). L'arithmétique
opère sur la base et rend la base (`p + 1` est un `i32`) : revenir à `Pourcent` demande
`as Pourcent`, si bien que la seule panique possible est là où le programme écrit `as`.
Aucune option ne retire ces vérifications. Un type à la fois `distinct` et contraint n'a
pas de conversion implicite et ne peut pas reprendre l'arithmétique (E0251).

*(Pas encore : types distincts génériques ou sur une base non scalaire ; filtrage qui
utilise la condition pour l'exhaustivité ; vérification à l'entrée des `@export`, tâche C4.)*

## 3.5 Inférence

À l'intérieur d'un corps de fonction, le compilateur résout les types par **unification**
(variante de Hindley-Milner restreinte aux corps, sans généralisation locale) :

- `let x = 5` : le type de `x` est déterminé par **l'ensemble du corps**. Si rien ne le
  contraint, un littéral entier est de type `i32` et un littéral flottant de type `f64`.
- `let v = Vec.new()` puis `v.push(1u8)`... : le paramètre de `Vec` est déduit de
  l'usage ultérieur.
- Si un type ne peut pas être déterminé (par exemple `let v = Vec.new()` jamais utilisé),
  c'est une erreur E0320 « annotation de type nécessaire », avec l'endroit où l'ajouter.
- L'appel d'une méthode exige que le type du receveur soit connu à cet endroit
  (dans l'ordre du texte) ; sinon E0320.

## 3.6 Conversions

**Aucune conversion implicite** entre types numériques, ni entre `bool` et entiers.
`let x: i64 = y` avec `y: i32` est une erreur (E0302) qui suggère `y as i64`.

Seule coercition implicite : une expression de type `Never` est acceptée partout.

L'opérateur `as` réalise les conversions explicites suivantes :

| De → Vers | Sémantique |
|---|---|
| entier → entier | **vérifiée** : panique si la valeur n'est pas représentable dans le type cible |
| entier → flottant | arrondi au plus proche (pair en cas d'égalité) |
| flottant → entier | troncature vers zéro ; **panique** si NaN ou hors de l'intervalle du type cible |
| `f32` ↔ `f64` | IEEE 754 (arrondi au plus proche) |
| `bool` → entier | `false` → 0, `true` → 1 |
| `char` → entier | valeur scalaire ; vérifiée comme entier → entier |
| entier → `char` | vérifiée : panique si la valeur n'est pas une valeur scalaire Unicode |
| `Ptr[T]` → `Ptr[U]` | réinterprétation (en contexte `unsafe` uniquement) |

Une conversion dont le résultat est connu à la compilation est vérifiée à la compilation
(`300 as u8` est l'erreur E0301). Pour tronquer volontairement, on masque d'abord :
`(x & 0xFF) as u8`.

## 3.7 Types copiables et types ressources

Chaque type appartient à exactement une catégorie :

- **Copiable (trivial)** : primitifs, `Ptr[T]`, types `fn`, et tout tuple, tableau,
  structure ou énumération dont tous les composants sont copiables, qui n'a pas de
  méthode `drop` et ne contient pas de stockage `indirect` ; en **édition 1**, une
  structure ou une énumération **publique** doit en plus **déclarer** `Copy` (ADR 0012).
- **Ressource** : tous les autres (`String`, `Vec[T]`, types avec `drop`, types avec
  `indirect`, types publics de l'édition 1 sans déclaration, et les paramètres de type
  dans un corps générique, sauf contrainte `Copy`).

L'interface **`Copy`** du prélude nomme la catégorie copiable. Elle n'a pas de méthode et
sert de contrainte générique (`T: Copy`, `T is Copy`). Un type la **déclare** par
`@derive(Copy)`, `impl Copy for T {}`, ou `type T is Copy { … }` / `type T is Copy = .A | .B`
en édition 1 ; les fonctions écrites dans `type T is Copy { … }` restent celles du type.
La déclaration est **vérifiée** : un champ ou une valeur de variante non copiable, une
méthode `drop` ou un stockage `indirect` donnent E0250 (les paramètres d'un type générique
comptent comme copiables : `Paire[i32, String]` reste une ressource).

La règle dépend de l'édition du fichier **qui déclare le type** :

| Type | Édition 0.2 | Édition 1 |
|---|---|---|
| privé à son module | structure | structure |
| public (`pub`) | structure | structure **et** déclaration `Copy` |

Ainsi la copiabilité d'un type public fait partie de son contrat écrit : ajouter un champ
`String` privé à un type public déclaré `Copy` est refusé (E0250) au lieu de changer en
silence les copies de ses utilisateurs en déplacements. Un type public de l'édition 1
copiable par sa structure mais non déclaré est une ressource ; E0402 propose alors la
déclaration. `nxc migrate` ajoute `is Copy` aux types publics que l'édition 0.2 copie,
si bien que la migration ne change pas le sens d'un programme. Les types publics
copiables de `std` (`Option`, `Result`, `Ordering`, `fmt.FormatSpec`, `rt.Location`,
`time.Weekday`, `time.Date`, `time.DateTime`) déclarent `Copy`.

Une valeur copiable est dupliquée bit à bit quand on la réutilise ; une ressource est
**déplacée**. Les règles complètes sont au chapitre 05.

## 3.8 Disposition mémoire

En 0.1, la disposition des structures, tuples et énumérations n'est **pas spécifiée**
(le compilateur peut la choisir et la changer). Les tableaux sont contigus, sans
remplissage entre éléments autre que celui imposé par l'alignement du type d'élément.
*(Hors 0.1 : `@repr("C")` pour garantir la disposition C, `@align(n)`, `@packed`.)*
Seuls les types primitifs et `Ptr[T]` ont une représentation garantie à la frontière C
(§08).

## 3.9 Traits

Un **trait** nomme un ensemble de méthodes qu'un type peut implémenter. Une méthode d'un
trait peut avoir un corps par défaut ; `Self` désigne le type qui implémente le trait.

```nexul
trait Forme {
    fn aire(self) -> f64
    fn nom(self) -> String

    fn decrire(self) -> String {
        return "{self.nom()} d'aire {self.aire():.2}"
    }
}

struct Cercle {
    rayon: f64
}

struct Rectangle {
    l: f64
    h: f64
}

impl Forme for Cercle {
    fn aire(self) -> f64 {
        return 3.14159 * self.rayon * self.rayon
    }

    fn nom(self) -> String {
        return "cercle"
    }
}

impl Forme for Rectangle {
    fn aire(self) -> f64 {
        return self.l * self.h
    }

    fn nom(self) -> String {
        return "rectangle"
    }
}

fn aire_totale[T: Forme](formes: Vec[T]) -> f64 {
    var total = 0.0
    for f in formes {
        total += f.aire()
    }
    return total
}

fn main() {
    var cercles: Vec[Cercle] = Vec.new()
    cercles.push(Cercle(rayon: 1.0))
    println("{aire_totale(cercles):.3}")

    var formes: Vec[dyn Forme] = Vec.new()
    formes.push(Cercle(rayon: 0.5))
    formes.push(Rectangle(l: 2.0, h: 3.0))
    for f in formes {
        println(f.decrire())
    }
}
```

Règles :

- `impl Trait for Type` fournit chaque méthode sans corps par défaut (E0242), avec
  exactement la signature du trait, `Self` remplacé par le type (E0241). Un type
  implémente un trait **au plus une fois** dans un programme (E0243) ; l'`impl` se trouve
  dans le module du trait ou dans celui du type (E0247).
- `trait Ord: Eq` déclare un **super-trait** : tout type qui implémente `Ord` implémente
  aussi `Eq` (E0244).
- Une implémentation peut être conditionnelle : `impl Eq for Option[T: Eq]`.
- Contraintes : `fn f[T: Ord + Clone](...)` ou `where T: Ord, U: Display` après la
  signature. Dans le corps, les méthodes des traits sont appelées sur `T` (`x.clone()`),
  et les fonctions sans `self` sur le type (`T.default()`). Les appels génériques sont
  **monomorphisés** : aucun coût à l'exécution.
- **Répartition dynamique** : une valeur de type `dyn Trait` contient une valeur de
  n'importe quel type qui implémente le trait, rangée sur le tas, et une table de ses
  méthodes. La conversion est implicite là où `dyn Trait` est attendu, ou explicite avec
  `x as dyn Trait`. Un trait utilisable en `dyn` n'a que des méthodes qui prennent `self`
  ou `inout self`, sans paramètre générique ni `Self` ailleurs dans la signature (E0246).
  Une valeur `dyn` est une ressource : elle est détruite avec sa valeur.

### Traits standards et opérateurs

Le prélude fournit `Eq`, `Ord` (avec `Ordering`), `Clone`, `Display`, `Hash`, `Default`
et les traits d'opérateurs `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`, implémentés par les
types primitifs, `String`, `Vec`, `Option`, `Result`, `Map` et `Set` (§09), ainsi que
`Error` (`trait Error: Display`), l'interface des erreurs que `?` peut ranger dans un
`dyn Error` (§7.2).

| Écriture | Devient |
|---|---|
| `a == b`, `a != b` | `a.eq(b)`, `not a.eq(b)` |
| `a < b`, `a <= b`, `a > b`, `a >= b` | comparaison de `a.cmp(b)` avec `Ordering.Less`, etc. |
| `a + b`, `a - b`, `a * b`, `a / b`, `a % b` | `a.add(b)`, `a.sub(b)`, `a.mul(b)`, `a.div(b)`, `a.rem(b)` |
| `-a` | `a.neg()` |
| `"{x}"`, `print(x)` | `x.to_string()` |

`@derive(Eq, Ord, Clone, Display, Hash, Default)` devant une structure ou une énumération
génère ces implémentations champ par champ (dans l'ordre de déclaration ; pour une
énumération, l'ordre des variantes, puis leurs valeurs). `Display` affiche
`Point(x: 1, y: 2)` ; `Default` n'est dérivable que pour une structure.

