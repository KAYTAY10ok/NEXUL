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

## 3.4 Alias de types

`type Metres = f64` introduit un second nom pour un type existant. Ce n'est **pas** un
nouveau type : `Metres` et `f64` sont interchangeables. *(Hors 0.1 : types distincts.)*

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

Chaque type appartient à exactement une catégorie, déterminée **structurellement** :

- **Copiable (trivial)** : primitifs, `Ptr[T]`, types `fn`, et tout tuple, tableau,
  structure ou énumération dont tous les composants sont copiables, qui n'a pas de
  méthode `drop` et ne contient pas de stockage `indirect`.
- **Ressource** : tous les autres (`String`, `Vec[T]`, types avec `drop`, types avec
  `indirect`, et les paramètres de type dans un corps générique, sauf contrainte `Copy`).

L'interface **`Copy`** du prélude nomme la catégorie copiable. Elle n'a pas de méthode
et ne s'implémente pas par un `impl` : un type la satisfait par sa structure (ci-dessus).
Elle sert de contrainte générique. *(Proposé par l'ADR 0012, en attente de décision :
qu'un type public doive déclarer `Copy` pour être copiable à partir de l'édition 1.)*

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
types primitifs, `String`, `Vec`, `Option`, `Result`, `Map` et `Set` (§09).

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

