# Guide de démarrage NEXUL

Ce guide présente NEXUL 0.1 par l'exemple. Chaque programme complet de ce guide est
compilé par la suite de tests (`cargo test`) : s'il ne compile plus, le test échoue.

## 1. Installer et lancer

1. Installer [Rust](https://rustup.rs) et [Zig](https://ziglang.org).
2. Construire le compilateur : `cargo build --release` à la racine du dépôt.
3. Écrire un fichier `bonjour.nx` :

```nexul
fn main() {
    println("Bonjour !")
}
```

4. L'exécuter : `nxc run bonjour.nx`. Pour produire un exécutable : `nxc build bonjour.nx`.

Il n'y a pas de point-virgule : une instruction se termine en fin de ligne. Une ligne qui
se termine par un opérateur (`+`, `=`, `,`...) continue sur la ligne suivante.

## 2. Variables et types

```nexul
fn main() {
    let nom = "Ada"          // immuable, type String déduit
    var age: u32 = 36        // modifiable, type explicite
    age += 1
    let pi = 3.14159         // f64
    let initiale = 'A'       // char
    let actif = true         // bool
    println("{nom}, {age} ans, initiale {initiale}, actif : {actif}, pi ≈ {pi:.2}")
}
```

- `let` déclare une variable immuable, `var` une variable modifiable.
- Entiers : `i8` `i16` `i32` `i64` `isize` et `u8` ... `usize` ; flottants `f32` `f64`.
  Un littéral entier est `i32` par défaut, un littéral flottant `f64`.
- **Aucune conversion implicite** : `x as f64`, `n as u8`. Les conversions sont vérifiées :
  `300 as u8` est refusé à la compilation, et une conversion qui perd de l'information à
  l'exécution provoque une panique.
- Les chaînes interpolent des expressions entre accolades : `"{a + b}"`, avec un format
  optionnel : `"{prix:.2}"`, `"{n:>8}"`, `"{octet:02x}"`.

## 3. Fonctions

```nexul
fn carre(x: i64) -> i64 {
    return x * x
}

fn saluer(nom: String) {
    println("Bonjour {nom}")
}

fn main() {
    saluer("monde")
    println("{carre(12)}")
}
```

Les types des paramètres et du résultat sont toujours écrits. Une fonction qui renvoie
une valeur utilise `return` sur chaque chemin : il n'y a pas de retour implicite.

## 4. Contrôle de flux

```nexul
fn main() {
    let n = 17
    let parite = if n % 2 == 0 { "pair" } else { "impair" }
    println("{n} est {parite}")

    for i in 0..3 {          // 0, 1, 2
        print("{i} ")
    }
    println()

    var compte = 0
    while compte < 3 {
        compte += 1
    }

    let description = match n {
        0 => "zéro"
        1..=9 => "un chiffre"
        x if x < 0 => "négatif"
        _ => "grand"
    }
    println(description)
}
```

`if` et `match` sont des expressions. Un `match` doit couvrir **tous** les cas : le
compilateur liste ceux qui manquent.

## 5. Structures, énumérations et méthodes

```nexul
struct Point {
    x: f64
    y: f64
}

impl Point {
    fn origine() -> Point {
        return Point(x: 0.0, y: 0.0)
    }

    fn distance(self, autre: Point) -> f64 {
        let dx = self.x - autre.x
        let dy = self.y - autre.y
        return (dx * dx + dy * dy).sqrt()
    }

    fn deplacer(inout self, dx: f64, dy: f64) {
        self.x += dx
        self.y += dy
    }
}

enum Forme {
    Cercle(f64)
    Rectangle(f64, f64)
}

fn aire(f: Forme) -> f64 {
    return match f {
        Cercle(r) => 3.14159 * r * r
        Rectangle(l, h) => l * h
    }
}

fn main() {
    var p = Point.origine()
    p.deplacer(3.0, 4.0)
    println("distance = {p.distance(Point.origine())}")
    println("aire = {aire(Forme.Rectangle(2.0, 3.0))}")
}
```

- On construit une structure avec ses champs nommés, dans l'ordre : `Point(x: 1.0, y: 2.0)`.
- `self` est emprunté en lecture ; `inout self` permet de modifier l'objet.
- Un type récursif marque son stockage `indirect` :
  `enum Arbre { Feuille(i64), indirect Noeud(Arbre, Arbre) }`.

## 6. Absence de valeur et erreurs

```nexul
fn diviser(a: i64, b: i64) -> Result[i64, String] {
    if b == 0 {
        return Err("division par zéro")
    }
    return Ok(a / b)
}

fn calcul() -> Result[i64, String] {
    let x = diviser(10, 2)?      // en cas d'erreur, la fonction s'arrête ici
    let y = diviser(x, 0)?
    return Ok(y)
}

fn main() {
    match calcul() {
        Ok(v) => println("résultat : {v}")
        Err(e) => println("erreur : {e}")
    }
    let trouve: Option[i32] = Some(4)
    if let Some(n) = trouve {
        println("trouvé {n}")
    }
    println("{trouve.unwrap_or(0)}")
}
```

Il n'y a ni `null` ni exceptions : l'absence s'exprime par `Option[T]`, l'échec par
`Result[T, E]`, et `?` propage l'erreur.

## 7. Propriété : le cœur de la sûreté

Les types qui possèdent des ressources (`String`, `Vec`, et les structures qui les
contiennent) sont **déplacés** et non copiés :

```nexul
fn consommer(owned s: String) {
    println("j'ai pris {s}")
}

fn afficher(s: String) {
    println("je lis {s}")
}

fn main() {
    let a = "texte".clone()
    afficher(a)          // emprunt en lecture : `a` reste utilisable
    let b = a            // déplacement : `a` n'a plus de valeur
    consommer(b)         // `b` est transmis à la fonction
}
```

Les trois façons de passer un argument :

| Paramètre | Appel | Effet |
|---|---|---|
| `x: T` | `f(a)` | lecture seule (par défaut) |
| `inout x: T` | `f(&a)` | modification de la variable de l'appelant |
| `owned x: T` | `f(a)` | transfert de propriété |

Les règles qui garantissent la sûreté :
- on ne lit pas une variable après l'avoir déplacée (le compilateur indique où elle l'a été) ;
- pendant qu'on parcourt une collection, on ne la modifie pas ;
- on ne passe jamais la même variable deux fois à un appel si l'un des passages est `inout`.

Les valeurs sont détruites automatiquement à la fin de leur portée, dans l'ordre inverse
de leur création. Un type peut définir `fn drop(inout self)` pour libérer une ressource.

Pour modifier sur place le contenu d'une `Option` ou les éléments d'une collection, on
emprunte avec `&` :

```nexul
fn main() {
    var compteur: Option[i32] = Some(1)
    match &compteur {
        Some(n) => n += 1
        None => compteur = Some(0)
    }
    var notes = [12, 15, 9]
    for n in &notes {
        n += 1
    }
    println("{compteur.unwrap()} {notes[0]} {notes[2]}")
}
```

## 8. Collections et chaînes

```nexul
fn main() {
    var nombres: Vec[i32] = Vec.new()
    for i in 1..=5 {
        nombres.push(i * i)
    }
    nombres[0] = 100
    println("{nombres.len()} éléments, le dernier vaut {nombres[nombres.len() - 1]}")

    let phrase = "le vif renard brun"
    let mots = phrase.split(" ")
    for m in mots {
        print("[{m.to_uppercase()}] ")
    }
    println()
    println("{phrase.contains("renard")} {phrase.len()} octets, {phrase.char_count()} caractères")
}
```

Un accès hors bornes provoque une panique avec la position exacte dans le source ; il ne
corrompt jamais la mémoire.

## 9. Modules

Un fichier est un module. Le fichier `geometrie/formes.nx`, placé à côté du programme
principal, s'importe ainsi :

```
import geometrie.formes                  // utilisation : formes.Cercle(...)
import geometrie.formes.{Cercle, aire}   // noms importés directement
import std.io                            // bibliothèque standard
```

Seuls les éléments marqués `pub` sont visibles depuis les autres modules.

## 10. Tests

```nexul
fn factorielle(n: u64) -> u64 {
    if n <= 1 {
        return 1
    }
    return n * factorielle(n - 1)
}

@test
fn factorielle_de_cinq() {
    assert(factorielle(5) == 120)
}

fn main() {
    println("{factorielle(20)}")
}
```

`nxc test fichier.nx` exécute chaque fonction `@test` dans un processus séparé et
résume les résultats.

## 11. Code non sûr et C

```nexul
extern "C" fn abs(x: i32) -> i32

fn main() {
    let v = unsafe { abs(-5) }
    println("{v}")
}
```

Les appels à des fonctions C et les pointeurs bruts `Ptr[T]` exigent un bloc `unsafe` :
le compilateur ne peut pas les vérifier, et le mot-clé signale ces endroits au lecteur.

## 12. Compiler pour une autre machine

```
nxc build programme.nx --release --target aarch64-linux-gnu
nxc build programme.nx --target x86_64-linux-gnu
nxc build programme.nx --target aarch64-macos
nxc build programme.nx --target wasm32-wasi
```

## Pour aller plus loin

- [La spécification](spec/README.md) décrit chaque règle avec précision.
- `nxc explain E0402` explique un message d'erreur.
- Les programmes de `examples/` montrent des cas complets : calculatrice, arbre binaire de
  recherche, crible d'Ératosthène.
