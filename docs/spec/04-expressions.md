# 04 — Expressions et instructions

## 4.1 Ordre d'évaluation

L'évaluation est **strictement de gauche à droite**, sans exception non documentée :
opérandes d'un opérateur binaire, arguments d'un appel, champs d'une construction,
éléments d'un tableau, expressions interpolées d'une chaîne.

Pour une affectation `place = valeur` (et les affectations composées) :
1. les sous-expressions de la place qui calculent des valeurs (indices) sont évaluées,
   de gauche à droite ;
2. puis la valeur de droite ;
3. puis la place est résolue (vérification des bornes) et l'écriture a lieu.

Pour un appel, tous les arguments sont évalués de gauche à droite **avant** que les
places passées par emprunt ou `inout` ne soient résolues (vérification des bornes
comprise). Ainsi aucune adresse calculée ne peut être invalidée par l'évaluation d'un
autre argument.

## 4.2 Opérateurs arithmétiques

| Opérateur | Types | Sémantique |
|---|---|---|
| `+` `-` `*` | entiers | **panique en cas de débordement**, dans tous les modes de compilation |
| `+%` `-%` `*%` | entiers | arithmétique modulaire (débordement par enroulement, explicite) |
| `/` | entiers | division tronquée vers zéro ; panique si diviseur nul ou `MIN / -1` |
| `%` | entiers | reste du signe du dividende ; panique si diviseur nul ou `MIN % -1` |
| `+ - * / %` | flottants | IEEE 754 ; `%` est le reste de `fmod` ; pas de panique |
| `-` unaire | entiers signés, flottants | panique sur `-MIN` ; interdit sur les non signés (E0303) |
| `+` | `String` | concaténation : nouvelle chaîne, opérandes empruntés |

Choix de conception : le débordement silencieux est une source majeure de failles
(cahier des charges §11.1). Comme Swift, NEXUL le détecte **toujours** ; l'intention
d'enroulement s'écrit `+%`. Le coût est une instruction de branchement rarement prise.

## 4.3 Opérateurs bit à bit et décalages

`&`, `|`, `^`, `~` s'appliquent aux entiers (mêmes types des deux côtés).
`a << n` et `a >> n` : `n` peut être de n'importe quel type entier ; panique si `n < 0`
ou `n >=` largeur de `a`. `>>` est arithmétique pour les signés, logique pour les non
signés. Les bits sortants de `<<` sont perdus (pas de panique).

## 4.4 Comparaisons et logique

`==` `!=` : entiers, flottants (IEEE : `NaN != NaN`), `bool`, `char`, `String`, et
énumérations **sans données**. `<` `<=` `>` `>=` : entiers, flottants, `char`
(ordre des valeurs scalaires), `String` (ordre lexicographique des octets UTF-8).
Les deux opérandes doivent avoir le même type.

`and`, `or` : court-circuit, opérandes `bool`. `not` : négation `bool`.

## 4.5 Blocs et expressions conditionnelles

`if`, `match` et `unsafe { }` sont des **expressions**. Utilisées comme valeur :

```nexul
let signe = if x < 0 { "négatif" } else { "positif ou nul" }
let aire = match f {
    Cercle(r) => 3.14159 * r * r
    Rectangle(l, h) => l * h
    Vide => 0.0
}
```

- La valeur d'un bloc est celle de sa **dernière instruction si c'est une expression**.
- Un `if` utilisé comme valeur doit avoir une branche `else` (E0330).
- Toutes les branches doivent avoir le même type, sauf les branches qui divergent
  (`return`, `break`, `continue`, `panic(...)`, type `Never`).
- Utilisés comme instructions, `if` et `match` ignorent la valeur de leurs branches.

Le corps d'une fonction **n'a pas** de valeur implicite : une fonction qui renvoie une
valeur utilise `return` (sauf si le corps diverge). Le compilateur vérifie que tout
chemin d'exécution se termine par `return` ou diverge (E0331).

## 4.6 Boucles

```nexul
while condition { ... }
for i in 0..10 { ... }          // 0 à 9
for i in 0..=10 { ... }         // 0 à 10
for x in tableau { ... }        // éléments d'un tableau [T; N]
for x in vecteur { ... }        // éléments d'un Vec[T]
for c in texte { ... }          // caractères (char) d'une String
exterieur: for i in 0..n {
    for j in 0..n {
        if j == i { continue exterieur }
        if j > 5 { break exterieur }
    }
}
```

- Les bornes d'un intervalle sont évaluées **une fois**, avant la première itération ;
  elles ont le même type entier, qui est celui de la variable de boucle.
  `for i in a..b` n'itère pas si `a >= b`. Aucun débordement n'est possible, même avec
  `0..=u8.MAX`.
- Dans `for x in collection`, `x` est un **emprunt en lecture** de l'élément courant
  (une copie pour les types copiables) ; la collection ne peut pas être modifiée
  pendant la boucle (§05, E0420).
- Dans `for x in &collection`, `x` est un **emprunt en écriture** : `for p in &points { p.x += 1.0 }`
  modifie les éléments sur place. La collection doit être modifiable et n'est accessible
  que par `x` pendant la boucle. Les caractères d'une `String` ne se modifient pas ainsi.
- `while true { ... }` est reconnu comme boucle infinie pour l'analyse des chemins de
  retour.
- `break` et `continue` sans étiquette visent la boucle la plus interne ; une étiquette
  inconnue est l'erreur E0207.

## 4.7 Filtrage par motif (`match`, `if let`)

```nexul
match commande {
    "quitter" | "q" => return
    "aide" => afficher_aide()
    _ => println("commande inconnue : {commande}")
}

match valeur {
    0 => println("zéro")
    1..=9 => println("chiffre")
    n if n < 0 => println("négatif")
    _ => println("grand")
}

if let Some(x) = chercher(cle) {
    println("trouvé {x}")
} else {
    println("absent")
}
```

- Les bras sont testés **dans l'ordre** ; le premier motif qui correspond (et dont la
  garde `if` éventuelle est vraie) est exécuté.
- Le filtrage doit être **exhaustif** (E0340), avec la liste des cas manquants dans le
  message. Les entiers, caractères et chaînes nécessitent un bras `_` ou une liaison.
- Un bras inaccessible déclenche l'avertissement W0002.
- Les alternatives `|` ne peuvent pas lier de variables en 0.1 (E0341).
- **Liaisons** : si le scrutin est une *place* (variable, champ, élément), les liaisons
  sont des **emprunts en lecture** et le scrutin est gelé pendant le bras ; si le
  scrutin est une *valeur* (appel, construction, `move x`), les liaisons **prennent
  possession** des parties qu'elles désignent ; avec `match &place` (ou
  `if let P = &place`), les liaisons sont des **emprunts en écriture** (§05) :

```nexul
fn incrementer(inout compteur: Option[i32]) {
    match &compteur {
        Some(n) => n += 1
        None => compteur = Some(1)
    }
}
```

## 4.8 Déclarations de variables

```nexul
let nom = "Ada"           // immuable
var compteur = 0          // modifiable
var total: i64            // déclaration sans valeur : doit être affectée avant lecture
let (q, r) = division(17, 5)
```

- Lire une variable avant son initialisation certaine est l'erreur E0401.
- Redéclarer un nom **dans la même portée** est l'erreur E0201 ; le masquer dans une
  portée imbriquée est permis.
- Une variable `var` jamais modifiée produit l'avertissement W0003 (« utilisez `let` »).
- Une variable jamais lue produit W0001, sauf si son nom commence par `_`.

## 4.9 Appels, méthodes et fonctions associées

```nexul
impl Point {
    pub fn origine() -> Point {                 // fonction associée
        return Point(x: 0.0, y: 0.0)
    }
    pub fn norme(self) -> f64 {                 // méthode : self emprunté en lecture
        return (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn translater(inout self, dx: f64, dy: f64) {   // méthode modifiante
        self.x += dx
        self.y += dy
    }
}

var p = Point.origine()
p.translater(1.0, 2.0)
let n = p.norme()
```

- `Type.fonction(...)` appelle une fonction associée ; `valeur.methode(...)` appelle une
  méthode, le receveur étant passé selon la convention de `self`.
- Un appel à une méthode `inout self` exige que le receveur soit une place modifiable
  (variable `var`, paramètre `inout`, ou un de leurs champs ou éléments) (E0410).
- Un champ et une méthode du même type ne peuvent pas porter le même nom (E0212).
- Les types primitifs possèdent des méthodes intégrées :
  - entiers : `abs()` (signés ; panique sur `MIN`), `min(y)`, `max(y)`, `pow(n: u32)`
    (vérifiée), `to_string()` ;
  - flottants : `sqrt`, `floor`, `ceil`, `round`, `trunc`, `abs`, `sin`, `cos`, `tan`,
    `asin`, `acos`, `atan`, `exp`, `ln`, `log10`, `log2` (sans argument), `powf(y)`,
    `atan2(y)`, `hypot(y)`, `min(y)`, `max(y)`, `is_nan()`, `is_infinite()`,
    `is_finite()`, `to_string()` ;
  - `char` : `is_ascii`, `is_ascii_digit`, `is_ascii_alphabetic`, `is_ascii_alphanumeric`,
    `is_ascii_uppercase`, `is_ascii_lowercase`, `is_whitespace` (Unicode),
    `to_ascii_uppercase`, `to_ascii_lowercase`, `len_utf8`, `to_string` ;
  - `bool` : `to_string()`.

## 4.10 Fonctions intégrées

Toujours visibles, sans import :

| Fonction | Rôle |
|---|---|
| `print(v)` / `println(v)` / `println()` | écrit `v` (tout type qui implémente `Display`) sur la sortie standard, avec ou sans saut de ligne |
| `eprint(v)` / `eprintln(v)` | idem sur la sortie d'erreur |
| `panic(message: String) -> Never` | arrête le programme (§07) |
| `assert(condition)` / `assert(condition, message)` | panique si la condition est fausse |
| `unreachable() -> Never` | panique « code supposé inaccessible atteint » |

## 4.11 Interpolation et formatage

`"{expr}"` produit une nouvelle `String`. Types formatables : entiers, flottants,
`bool`, `char`, `String` et tout type qui implémente `Display` (§3.9), dont `Vec`,
`Option`, `Map` et les types dérivés avec `@derive(Display)`. Les flottants s'affichent sous la **forme la plus courte qui
relit exactement la même valeur** (`0.1` s'affiche `0.1`, `1.0` s'affiche `1.0`,
`1e300` s'affiche `1e300`), `inf`, `-inf`, `NaN`. Avec une précision (`{x:.2}`), arrondi
au plus proche. Les spécifications de format (§1.5) s'appliquent aux nombres et chaînes.
Un type non formatable est l'erreur E0350.

## 4.12 Fonctions anonymes

Une fonction anonyme s'écrit `fn(x) => expression` ou `fn(x: i32) -> String { ... }`.
Les types des paramètres et du résultat sont inférés du paramètre qui la reçoit.

```nexul
fn appliquer_deux_fois(x: i32, f: fn(i32) -> i32) -> i32 {
    return f(f(x))
}

fn repeter(n: i32, action: fn(i32)) {
    for i in 0..n {
        action(i)
    }
}

fn main() {
    let decalage = 10
    println("{appliquer_deux_fois(1, fn(x) => x + decalage)}")

    var total = 0
    repeter(5, fn(i) {
        total += i
    })

    var v: Vec[i32] = Vec.new()
    for i in 1..=6 {
        v.push(i)
    }
    let pairs = v.filter(fn(x) => x % 2 == 0)
    println("{total} {pairs} {v.map(fn(x) => x * x)} {v.fold(0, fn(acc, x) => acc + x)}")

    var pile = v.clone()
    while let Some(sommet) = pile.pop() {
        if sommet < 5 {
            break
        }
    }
}
```

- Une fonction anonyme utilise les variables qui l'entourent **sur place** (sans copie) :
  `total += i` modifie la variable de `main`. Les règles d'exclusivité (§05) s'appliquent
  pendant l'appel qui la reçoit.
- Elle est **non échappante** : elle ne peut être passée que directement en argument à un
  paramètre de type fonction, et ce paramètre ne peut être ni stocké, ni renvoyé, ni rangé
  dans une collection (E0248). Aucune annotation de durée de vie n'est donc nécessaire.
  Pour conserver une fonction dans une valeur, on utilise une fonction nommée.
- Un paramètre de type fonction accepte aussi une fonction nommée (`v.map(doubler)`).
- `while let Motif = expr { ... }` répète le corps tant que la valeur correspond au motif.

