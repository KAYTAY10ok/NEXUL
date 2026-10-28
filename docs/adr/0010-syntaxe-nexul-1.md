# 0010 — La syntaxe de NEXUL 1 (édition 1)

Lot P2, étape G2. Auteur : Claude Main. **Statut : acceptée par l'utilisateur le
16/10/2026, avec ses retouches** (le type avant le nom partout ; plus de `->`). Tout le
code de ce document reste **illustratif** tant que `nxc` ne l'accepte pas (G3).

## Contexte

- G0 ([audit](syntaxe/g0-audit.md)) : ce qui est bon dans la 0.2, ses deux pièges, ses
  ambiguïtés.
- G1 ([trois propositions](syntaxe/g1-propositions.md)) : A « Continuité », B « Phrases »,
  C « Signes justes ».
- **Choix de l'utilisateur (15/10/2026)** : un mélange. Garder la logique habituelle des
  égalités (`=`, `==`, `!=`, `===`) ; écrire en mots les constructions et propriétés de
  base, comme B ; réduire l'écriture grâce à C ; obtenir quelque chose de vraiment propre à
  NEXUL, que n'importe quel développeur prend en main. Si une forme ressemble trop à un
  autre langage ou devient illisible, l'aborder autrement.
- **Retouches de l'utilisateur (16/10/2026)** : avec le typage renforcé, on écrira beaucoup
  de types ; **le type vient avant le nom**, comme `i32 x`. La flèche `->` (qui annonçait le
  type rendu par une fonction) n'a donc plus de raison d'être.

## La méthode : trois couches

Plutôt que de chercher des mots ou des signes inédits un par un (ce qui mène soit à copier
Rust, soit à l'illisible), la syntaxe est construite en **trois couches qui ne se mélangent
pas** :

1. **Les opérateurs, les littéraux et l'ordre « type puis nom » sont ceux que tout le monde
   connaît** (C, C#, Java) : `=`, `==`, `!=`, `<`, `+`, `"texte"`, `[1, 2]`, `0..10`,
   `i32 x`. Aucune surprise.
2. **La structure et l'intention s'écrivent en mots** qui se lisent comme une phrase :
   `type … is …`, `when … then`, `if … is …`, `else` de repli, `repeat`, `test "…"`,
   `requires`, `with noalloc`, `every … within …`.
3. **Les effets ont un signe, et ce signe n'a qu'un sens** : `!` = « cette valeur est
   modifiée ici », `?` = « l'erreur remonte ici », `.Nom` = « variante du type attendu »,
   `|>` = « passe le résultat à ».

Un développeur lit la couche 1 sans rien apprendre, la couche 2 comme de l'anglais simple,
et n'a que quatre signes à retenir pour la couche 3. C'est cette combinaison, et non un mot
isolé, qui fait l'identité de NEXUL.

## Décision

### 1. Le type avant le nom

| Écriture | Sens | 0.2 |
|---|---|---|
| `let i32 x = 5` | valeur fixe, typée | `let x: i32 = 5` |
| `var f64 somme = 0.0` | variable modifiable, typée | `var somme: f64 = 0.0` |
| `let total = 3` | type déduit de la valeur (ici `i32`) | inchangé |
| `fn i32 carre(i32 x) = x * x` | fonction : **type rendu, nom, paramètres** | `fn carre(x: i32) -> i32` |
| `fn main() { … }` | sans type rendu : la fonction ne rend rien | inchangé |
| `type Point { f64 x  f64 y }` | champs : type puis nom | `x: f64` |
| `fn(i32 x) => x * 2` | fonction anonyme | `fn(x: i32) => x * 2` |
| `fn i32(i32, i32)` | **type** d'une fonction (comme sa déclaration, sans les noms) | `fn(i32, i32) -> i32` |
| `const i32 MAX = 100` | constante | `const MAX: i32 = 100` |

Les types sont **obligatoires** sur les fonctions, les champs, les constantes et les
paramètres. Sur une variable locale, ils sont facultatifs quand la valeur les donne
(`let total = 3`) ; Studio et l'extension VS Code peuvent afficher le type déduit.

Le `:` n'a plus qu'un sens : **« nom : valeur »**, dans une construction ou un argument
nommé (`Point(x: 1.0, y: 2.0)`), et dans le format d'une interpolation (`{x:.2}`). La
flèche `->` disparaît du langage (G0 §4.4 et §4.8 réglés).

### 2. Égalités et comparaisons

| Écriture | Sens |
|---|---|
| `x = valeur` | affectation (et `+=`, `-=`, `*=`, …) |
| `a == b`, `a != b` | même valeur (interface `Eq`) |
| `a === b`, `a !== b` | **même objet** : seulement pour les valeurs partagées (`Shared[T]`, `any Interface`, tâches, canaux) ; sur une valeur simple, erreur qui conseille `==` |
| `<`, `<=`, `>`, `>=` | ordre (interface `Ord`) ; jamais enchaînées (`E0104`, comme en 0.2) |
| `x in 0..10`, `x in liste` | appartenance |
| `x is .Some(n)`, `f is Cercle` | filtre par motif, test de type ; introduit les noms du motif |

Logique en mots : `and`, `or`, `not` (0.2). Bit à bit : `&`, `|`, `^`, `~`, `<<`, `>>`,
plus prioritaires que les comparaisons (0.2).

### 3. Déclarations

| Écriture | Remplace (0.2) |
|---|---|
| `type Compte { String titulaire  i64 solde … méthodes }` | `struct` + `impl` |
| `type Couleur = .Rouge \| .Vert \| .Rgb(u8 r, u8 g, u8 b)` | `enum` |
| `type Metres = distinct f64` | **nouveau** : type distinct (passation §4.2) |
| `type Pourcent = i32 where it in 0..=100` | **nouveau** : type contraint |
| `type Ligne = Vec[String]` | `type` (alias) |
| `interface Forme { fn f64 aire(self) }` | `trait` |
| `type Cercle is Forme { … }` | `impl Forme for Cercle` |
| `extend i32 is Affichable { … }` | `impl` sur un type existant |
| `any Forme` | `dyn Forme` |
| `fn T plus_grand[T is Ord and Copy](Vec[T] v)` | `fn plus_grand[T: Ord + Copy](v: Vec[T]) -> T` |
| `test "description" { … }` | `@test fn nom() { … }` |

Un seul mot, `type`, déclare toutes les sortes de types ; les méthodes vivent dans le corps
du type. Une fonction du corps sans `self` est une fonction du type (`Compte.ouvrir(…)`).
Le corps d'une fonction est un bloc (avec `return` pour rendre une valeur) ou, en une
ligne, `= expression`.

### 4. Modification visible : `!`

Le `!` suit **toujours** la valeur modifiée :

| Où | Écriture |
|---|---|
| paramètre modifiable | `fn ajouter_point(String t!)` |
| méthode qui modifie | `fn deposer(self!, i64 montant)` |
| argument | `ajouter_point(texte!)` |
| receveur | `compte!.deposer(150)`, `liste!.push(4)` |
| éléments d'une boucle | `for note! in notes { note += 1 }` |
| liaisons d'un filtre | `when arbre! { … }` |

Une affectation (`x = …`, `self.solde += …`) n'en a pas besoin : `=` montre déjà la
modification. Le `!` n'a aucun autre sens (la négation s'écrit `not` ; `!=` et `!==` sont
des opérateurs entiers). `&` ne sert plus qu'au ET bit à bit.

### 5. Filtrer : `when … then`

```nexul
fn String nom(Couleur c) = when c {
    .Rouge then "rouge"
    .Vert, .Bleu then "froid"
    .Rgb(r, g, b) if r > 200 then "rougeâtre"
    else "autre"
}
```

- Une variante s'écrit **toujours** avec son point (`.Rouge`) ; un nom seul est **toujours**
  une nouvelle variable : le piège G0 §4.2 disparaît.
- Un bras par ligne ; plusieurs motifs séparés par `,` ; garde `if` ; bras final `else`.
- Le corps d'un bras est une expression ou un bloc.
- Dans une expression, `.Some(x)`, `.None`, `.Ok(v)`, `.Err(e)` s'écrivent aussi avec le
  point quand le type est connu ; `Some(x)`, `None`, `Ok`, `Err` restent acceptés en
  expression (prélude).

### 6. Conditions, boucles, repli

| Écriture | Sens |
|---|---|
| `if x > 0 { … } else { … }` | inchangé |
| `if valeur is .Some(n) { … }` | remplace `if let` |
| `while canal.recv() is .Some(x) { … }` | remplace `while let` |
| `for x in v { … }`, `for i32 i in 0..10 { … }` | type de la variable facultatif |
| `loop { … }` | remplace `while true` |
| `repeat 3 { … }` | **nouveau** : répéter n fois |
| `#exterieur for … { break #exterieur }` | étiquette (remplace `exterieur: for`) |
| `let i64 n = texte.parse_int() else 0` | **repli** : si l'`Option` est vide ou le `Result` une erreur |
| `let f = fs.open(p) else { return }` | repli par un bloc qui doit sortir |
| `fs.write_text(p, t) else println("échec")` | repli en instruction |
| `let x = f()?` | inchangé : l'erreur remonte |

### 7. Fonctions anonymes et enchaînement

- `fn(i32 x) => x * 2`, `fn(a, b) => a + b` (types facultatifs quand le contexte les
  donne), `fn i32(i32 x) { … return … }` avec un bloc.
- `valeurs |> map(fn(v) => v * 2) |> sum()` signifie `sum(map(valeurs, …))`. Une ligne
  peut commencer par `|>` ou par `.` : elle continue l'expression précédente.

### 8. Sûreté, concurrence, temps réel (formes réservées)

Leur sémantique vient avec P5, P7, P8. Jusque-là, `nxc` les **refuse** avec un message
« pas encore pris en charge » plutôt que de les accepter sans les vérifier.

| Écriture | Sens |
|---|---|
| `fn deposer(self!, i64 m) requires m > 0 ensures self.solde >= 0 { … }` | contrats |
| `fn String lire(String p) with io` | effets déclarés et vérifiés |
| `fn regulation(Etat etat!) every ms(10) within ms(2) with noalloc, nonblocking { … }` | tâche périodique temps réel |
| `let t = spawn { … }`, `t.join()` | tâche |
| `let (envoi, reception) = channel[i32](capacite: 8)` | canal borné (bibliothèque) |
| `pub`, `pub(package)` ; privé par défaut | visibilité |

### 9. Ce qui ne change pas (0.2)

Fins de ligne significatives (`;` seulement entre deux instructions d'une même ligne) ;
`E0110` pour une valeur perdue ; aucune expression ne commence par `{` ; construction
nommée `Point(x: 1.0, y: 2.0)` ; génériques entre crochets `Vec[i32]` et indexation `v[i]`
(l'analyse sémantique distingue, comme en 0.2) ; `as` pour les conversions ; arithmétique
vérifiée et `+%` ; interpolation `"{x}"`, `"{x:.2}"` ; attributs `@export`, `@cfg(…)` ;
`unsafe { … }` ; `owned`, `move` ; `indirect` ; identificateurs ASCII ; commentaires.

## Lexique

Mots-clés de l'édition 1 :

```
fn let var type interface extend is any distinct where it
if else when then for in while loop repeat break continue return
and or not as move owned self Self pub unsafe extern indirect const static
import test expects requires ensures with every within spawn true false
```

Retirés : `struct`, `enum`, `trait`, `impl`, `dyn`, `match`, `inout`, `module`. Les mots
nouveaux (`when`, `then`, `is`, `with`, `every`, `within`, `repeat`, `test`, `expects`,
`interface`, `extend`, `any`, `distinct`, `it`, `spawn`) ne sont réservés qu'en édition 1 :
un programme 0.2 qui les emploie comme noms reste valide sous `--edition 0.2`.

Signes : ceux de la 0.2 sauf `->`, plus `!` (postfixe, modification), `===`, `!==`, `|>`,
`#nom` (étiquette). `=>` ne sert plus qu'aux fonctions anonymes courtes ; `:` qu'à « nom :
valeur ».

## Grammaire (EBNF)

Notation du chapitre 02 de la spécification ; `FIN` est la fin d'instruction (§1.7).

```ebnf
programme      = { import } { element } ;
import         = "import" chemin [ "." "{" nom { "," nom } "}" ] [ "as" nom ] FIN ;
chemin         = nom { "." nom } ;
element        = { attribut } [ visibilite ]
                 ( fonction | type_decl | interface | extension | test | constante | externe ) ;
visibilite     = "pub" [ "(" "package" ")" ] ;
attribut       = "@" nom [ "(" arguments ")" ] ;

fonction       = "fn" [ type ] nom [ generiques ] "(" [ parametres ] ")" { clause } corps ;
                 (* un type suivi d'un nom : le type rendu ; un nom seul : rien n'est rendu *)
parametres     = parametre { "," parametre } [ "," ] ;
parametre      = "self" [ "!" ] | [ "owned" ] type nom [ "!" ] ;
generiques     = "[" generique { "," generique } "]" ;
generique      = nom [ "is" bornes ] ;
bornes         = type { "and" type } ;
clause         = "requires" expression | "ensures" expression
               | "with" nom { "," nom } | "every" expression [ "within" expression ] ;
corps          = bloc | "=" expression FIN ;

type_decl      = "type" nom [ generiques ] ( [ "is" bornes ] corps_type | "=" definition FIN ) ;
corps_type     = "{" { champ | { attribut } [ visibilite ] fonction } "}" ;
champ          = [ visibilite ] type nom ( FIN | "," ) ;
definition     = variantes | "distinct" type [ "is" bornes ] [ restriction ] | type [ restriction ] ;  (* ADR 0014 *)
variantes      = [ "|" ] variante { "|" variante } ;
variante       = [ "indirect" ] "." nom [ "(" champ_v { "," champ_v } ")" ] ;
champ_v        = type [ nom ] ;
restriction    = "where" condition ;                  (* « it » désigne la valeur ; condition : ADR 0014 §2.2 *)
interface      = "interface" nom [ generiques ] [ "is" bornes ] "{" { { attribut } signature } "}" ;
signature      = "fn" [ type ] nom [ generiques ] "(" [ parametres ] ")" { clause } [ corps ] FIN ;
extension      = "extend" type [ "is" bornes ] "{" { { attribut } [ visibilite ] fonction } "}" ;
test           = "test" chaine [ "expects" "panic" ] bloc ;
constante      = ( "const" | "static" [ "var" ] ) type nom "=" expression FIN ;

type           = chemin [ "[" type { "," type } "]" ] | "any" chemin [ "[" type { "," type } "]" ]
               | "(" [ type { "," type } ] ")" | "[" type ";" expression "]"
               | "fn" [ type ] "(" [ type { "," type } ] ")" | "Self" ;

bloc           = "{" { instruction FIN } "}" ;
instruction    = "let" [ type ] motif "=" expression [ "else" repli ]
               | "var" [ type ] nom [ "=" expression ]
               | expression affectation expression
               | [ etiquette ] boucle
               | expression [ "else" repli ] ;
affectation    = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
repli          = expression | bloc ;
etiquette      = "#" nom ;
boucle         = "for" [ type ] motif [ "!" ] "in" expression bloc | "while" expression bloc
               | "loop" bloc | "repeat" expression bloc ;

expression     = enchainement ;
enchainement   = disjonction { "|>" postfixe } ;
disjonction    = conjonction { "or" conjonction } ;
conjonction    = comparaison { "and" comparaison } ;
comparaison    = intervalle [ ( "==" | "!=" | "===" | "!==" | "<" | "<=" | ">" | ">=" | "in" ) intervalle
                            | "is" motif ] ;
intervalle     = bit_ou [ ( ".." | "..=" ) bit_ou ] ;
bit_ou         = bit_xou { "|" bit_xou } ;
bit_xou        = bit_et { "^" bit_et } ;
bit_et         = decalage { "&" decalage } ;
decalage       = additif { ( "<<" | ">>" ) additif } ;
additif        = multiplicatif { ( "+" | "-" | "+%" | "-%" ) multiplicatif } ;
multiplicatif  = conversion { ( "*" | "/" | "%" | "*%" ) conversion } ;
conversion     = unaire { "as" type } ;
unaire         = ( "-" | "not" | "~" | "move" ) unaire | postfixe ;
postfixe       = primaire { "." ( nom | entier ) | "(" [ arguments ] ")" | "[" expression { "," expression } "]"
                          | "?" | "!" } ;
arguments      = argument { "," argument } [ "," ] ;
argument       = [ nom ":" ] expression ;
primaire       = litteral | chaine | nom | "self" | "Self" | "it"
               | "." nom [ "(" arguments ")" ]                  (* variante du type attendu *)
               | "(" [ expression { "," expression } ] ")" | "[" [ expression { "," expression } ] "]"
               | si | quand | anonyme | "spawn" bloc | "unsafe" bloc
               | "return" [ expression ] | "break" [ etiquette ] | "continue" [ etiquette ] ;
si             = "if" expression bloc [ "else" ( si | bloc ) ] ;
quand          = "when" expression [ "!" ] "{" { bras FIN } [ "else" valeur FIN ] "}" ;
bras           = motif { "," motif } [ "if" expression ] "then" valeur ;
valeur         = expression | bloc ;
anonyme        = "fn" [ type ] "(" [ param_a { "," param_a } ] ")" ( "=>" expression | bloc ) ;
param_a        = [ type ] nom ;

motif          = motif_simple { "|" motif_simple } ;
motif_simple   = "_" | litteral | nom [ "!" ] | "." nom [ "(" motifs ")" ] | chemin "." nom [ "(" motifs ")" ]
               | "(" motifs ")" | intervalle_litteral ;
motifs         = motif_champ { "," motif_champ } ;
motif_champ    = [ nom ":" ] motif ;
```

Précédence, de la plus forte à la plus faible : suffixes (`.`, appel, `[…]`, `?`, `!`) ;
unaires (`-`, `not`, `~`, `move`) ; `as` ; `*` `/` `%` `*%` ; `+` `-` `+%` `-%` ; `<<` `>>` ;
`&` ; `^` ; `|` ; `..` `..=` ; comparaisons, `in`, `is` (non associatives) ; `and` ; `or` ;
`|>`. Le repli `else` n'existe qu'au niveau de l'instruction et de `let`.

**Reconnaître un type** : après `fn`, `let`, `var`, `for`, dans un paramètre ou un champ, le
parser lit d'abord un type ou un nom, puis regarde le jeton suivant : **un nom suit** → ce
qui précède était un type ; **`(`, `=`, `!` ou `in` suit** → c'était le nom (type absent ou
déduit). Un motif entre parenthèses (`let (a, b) = …`) se reconnaît de même au `=` qui le
suit. C'est un retour arrière borné par les crochets et parenthèses, sans information
sémantique. Un `else` qui suit le bloc d'un `if` appartient toujours à ce `if`.
Continuation de ligne : la ligne finit par un opérateur binaire, `,`, `=`, ou un délimiteur
ouvert ; ou la suivante commence par `.`, `|>`, `|` (variantes), `else`.

## Les 12 programmes en édition 1

**1 — Bonjour**

```nexul
fn main() {
    println("Bonjour, NEXUL !")
}
```

**2 — Calcul**

```nexul
fn Option[f64] moyenne(Vec[f64] notes) {
    if notes.is_empty() { return .None }
    var f64 somme = 0.0
    for n in notes { somme += n }
    return .Some(somme / notes.len() as f64)
}

fn main() {
    var Vec[f64] notes = Vec.new()
    for n in [12.0, 15.5, 9.0] { notes!.push(n) }
    let f64 m = moyenne(notes) else 0.0
    println("moyenne : {m:.2}")
    repeat 3 { print("*") }
}
```

**3 — Structures et méthodes**

```nexul
pub type Compte {
    String titulaire                          // privé
    i64 solde

    pub fn Compte ouvrir(owned String titulaire) = Compte(titulaire: titulaire, solde: 0)

    pub fn deposer(self!, i64 montant) requires montant > 0 {
        self.solde += montant
    }

    pub fn i64 solde(self) = self.solde
}

fn main() {
    var c = Compte.ouvrir("Ada")
    c!.deposer(150)
    println("{c.solde()}")
}
```

**4 — Interface polymorphe**

```nexul
import std.iter.{map, sum}

interface Forme {
    fn f64 aire(self)
    fn String nom(self)
}

type Cercle is Forme {
    f64 r
    fn f64 aire(self) = 3.14159 * self.r * self.r
    fn String nom(self) = "cercle"
}

type Rectangle is Forme {
    f64 l
    f64 h
    fn f64 aire(self) = self.l * self.h
    fn String nom(self) = "rectangle"
}

fn main() {
    var Vec[any Forme] formes = Vec.new()
    formes!.push(Cercle(r: 1.0))
    formes!.push(Rectangle(l: 2.0, h: 3.0))
    for f in formes { println("{f.nom()} : {f.aire():.2}") }
    let f64 total = formes |> map(fn(f) => f.aire()) |> sum()
    println("total : {total:.2}")
}
```

**5 — Génériques**

```nexul
fn Option[T] plus_grand[T is Ord and Clone](Vec[T] valeurs) {
    var Option[T] meilleur = .None
    for v in valeurs {
        var bool plus = true
        if meilleur is .Some(m) { plus = v > m }
        if plus { meilleur = .Some(v.clone()) }
    }
    return meilleur
}

type Pile[T] {
    Vec[T] elements
    fn empiler(self!, owned T x) { self.elements!.push(move x) }
    fn Option[T] depiler(self!) = self.elements!.pop()
}
```

**6 — Erreurs**

```nexul
fn Result[i64, String] diviser(i64 a, i64 b) {
    if b == 0 { return .Err("division par zéro") }
    return .Ok(a / b)
}

fn Result[i64, String] calcul() {
    let x = diviser(10, 2)?
    return .Ok(diviser(x, 0)?)
}

fn main() {
    when calcul() {
        .Ok(v) then println("résultat : {v}")
        .Err(e) then println("erreur : {e}")
    }
}
```

**7 — Fichiers**

```nexul
import std.fs

fn main() {
    let String texte = fs.read_text("notes.txt") else {
        println("lecture impossible")
        return
    }
    fs.write_text("rapport.txt", "{texte.lines().count()} lignes\n") else println("écriture impossible")
}
```

**8 — Propriété des valeurs**

```nexul
fn afficher(String t) = println(t)                       // lecture

fn ajouter_point(String t!) { t!.push_str(".") }        // modification

fn Vec[String] garder(owned String t) {                 // prise de propriété
    var Vec[String] v = Vec.new()
    v!.push(move t)
    return v
}

fn main() {
    var texte = "Bonjour"
    afficher(texte)
    ajouter_point(texte!)
    let liste = garder(move texte)                       // `texte` n'est plus utilisable
    println("{liste.len()}")
}
```

**9 — Threads et canaux**

```nexul
fn main() {
    let (envoi, reception) = channel[i32](capacite: 8)
    let producteur = spawn {                             // `envoi` est transféré dans la tâche
        for i in 0..10 { envoi.send(i * i) }
    }
    var i32 somme = 0
    while reception.recv() is .Some(x) { somme += x }
    producteur.join()
    println("somme : {somme}")
}
```

**10 — Tâche périodique temps réel**

```nexul
import std.rt.{ms, volatile_read, volatile_write}

@cfg("thumbv7em")
fn regulation(Etat etat!) every ms(10) within ms(2) with noalloc, nonblocking {
    let mesure = volatile_read(CAPTEUR)
    etat.commande = etat.pid!.calcul(mesure)
    volatile_write(MOTEUR, etat.commande)
}
```

**11 — Web (WebAssembly)**

```nexul
@export
fn i32 ajouter(i32 a, i32 b) = a + b

@export
fn String saluer(String nom) = "Bonjour, {nom} !"
```

**12 — Tests**

```nexul
fn i32 carre(i32 x) = x * x

test "carré d'un nombre positif" {
    assert(carre(3) == 9)
}

test "le dépassement panique" expects panic {
    let _ = carre(100000)
}
```

## Passer de la 0.2 à l'édition 1 (`nxc migrate`)

| 0.2 | Édition 1 | Remarque |
|---|---|---|
| `let x: T = v`, `var x: T = v` | `let T x = v`, `var T x = v` | |
| `fn f(a: A, b: B) -> R` | `fn R f(A a, B b)` | sans `-> R` : `fn f(A a, B b)` |
| `x: T` (champ), `fn(A) -> R` (type) | `T x`, `fn R(A)` | |
| `struct X { … }` + `impl X { … }` | `type X { champs … fonctions }` | l'`impl` rejoint le type s'il est dans le même module, sinon `extend X` |
| `enum E { A, B(T) }` | `type E = .A \| .B(T)` | |
| `trait T`, `impl T for X` | `interface T`, `type X is T` ou `extend X is T` | |
| `dyn T` | `any T` | |
| `match e { P => v }` | `when e { P then v }` | |
| motif `A`, `Some(x)` | `.A`, `.Some(x)` | les noms seuls restent des variables |
| `if let P = e`, `while let P = e` | `if e is P`, `while e is P` | |
| `f(&x)`, `inout x: T`, `inout self` | `f(x!)`, `T x!`, `self!` | |
| `v.m()` où `m` prend `inout self` | `v!.m()` | l'analyse sémantique dit quelles méthodes modifient |
| `for x in &v`, `match &e` | `for x! in v`, `when e!` | |
| `while true` | `loop` | |
| `a: for …`, `break a` | `#a for …`, `break #a` | |
| `[T: A + B]` | `[T is A and B]` | les clauses `where` passent en ligne |
| `@test fn nom() { … }` | `test "nom" { … }` | |

Le reste ne change pas. La migration part de l'AST, qui garde les positions (G0 §6) ;
commentaires et mise en page sont conservés. La 0.2 reste compilable sous `--edition 0.2`
pendant toute la transition ; l'édition se choisit par `edition = "1"` dans `nexul.toml`, par
`// edition: 1` en tête du fichier, ou par `--edition 1`.

## Plan d'implémentation (G3)

1. **Lexer** : mots de l'édition 1 réservés selon l'édition ; `!` postfixe, `===`, `!==`,
   `|>`, `#nom`.
2. **Parser de l'édition 1** vers **le même AST** que la 0.2 quand la sémantique existe
   déjà (`type` → `struct`/`enum` + `impl`, types avant les noms, `when` → `match`, `is` →
   `if let`, `x!` → argument `inout`, `extend` → `impl`, `test` → `@test`, `repeat` →
   `for`, `|>` → appel, `else` de repli → `match`) : la sémantique, le NIR et LLVM ne
   changent pas.
3. **Nouveau sens** : `===`, `in`, `distinct`, `where` (types contraints) ; les formes du §8
   sont refusées avec « pas encore pris en charge » jusqu'à P5, P7, P8.
4. **`nxc migrate`** : réécriture 0.2 → 1 ; mesure sur `lib/std/vec.nx` et
   `examples/calculatrice.nx` (recommandation 3 de G0), puis migration de `lib/`, des tests
   et des exemples.
5. **Formateur sur l'AST** (P6), **lexique fourni par le compilateur** (`nxc ide lexicon`)
   pour Studio et l'extension VS Code ; diagnostics et `nxc explain` revus.
6. **Tests** : chaque programme de cette ADR devient un test ; les suites `tests/run` et
   `tests/ui` passent dans les deux éditions pendant la transition.

## État de l'implémentation (G3)

**Tranche 1, 16/10/2026** : `nxc --edition 1` accepte `type` (structure, méthodes,
variantes, alias), `type … is`, `interface`, `extend`, `test "…"`, le type avant le nom
partout (`let`, `var`, `fn T f`, paramètres, champs, constantes, types de fonction
`fn R(A)`), `= expression`, `when … then`, `is` (dans `if`, `while` et comme valeur),
`.Nom` en motif et en expression, `x!` (argument, receveur, boucle), `any`, `|>`, `loop`,
`repeat`, `#étiquette`, fonctions anonymes `fn(T x) => …`. Les formes de la 0.2 retirées
sont refusées avec la nouvelle écriture (`E0112`) ; celles du §8, le repli `else`, `===`,
`in`, `distinct`, `where`, `spawn`, `expects panic` sont refusées (`E0111`). Tests :
`tests/run/edition1.nx`, `tests/ui/edition1_*.nx`.

Précisions apportées par l'implémentation :

- Continuation de ligne : une ligne qui commence par `.minuscule` (méthode, champ) continue la
  précédente ; une ligne qui commence par `.Majuscule` (variante) commence une instruction ou
  un bras de `when`.
- `type X is I { … }` : les fonctions du corps implémentent `I` ; une fonction propre au
  type va dans `extend X { … }` (pour l'instant).
- Les noms des valeurs d'une variante (`.Rgb(u8 r, …)`) sont acceptés mais pas encore
  utilisés : les motifs restent positionnels.
- Un nom seul dans un motif est pris pour une variante s'il en existe une du même nom (comme en
  0.2) ; `.Nom` est l'écriture recommandée.

**Tranche 2, 16/10/2026** : `nxc migrate fichier.nx… [--write]` réécrit la 0.2 en édition 1 à
partir de l'arbre (commentaires et mise en page gardés) ; les noms devenus réservés sont
renommés (`repeat` → `repeat_`) quand c'est sûr ; ce qui reste est listé avec sa ligne. Le
parser accepte aussi `e! is P` (liaisons qui modifient `e`) et `extend Type[A is Borne]`.
Mesures : les 33 programmes 0.2 de `tests/run` (un seul fichier) migrés donnent **la même
sortie et le même code de sortie** ; les 21 fichiers de `lib/` et les 8 exemples migrent sans
rien à reprendre ; les 7 exemples migrés compilent sans avertissement (tests `migrated_programs_behave_the_same`, `migrated_examples_and_library`).

**Tranche 3, 16/10/2026** : l'édition se choisit sans option : `--edition` s'il est donné,
sinon `// edition: X` sur l'une des trois premières lignes du fichier, sinon `edition = "1"`
dans le `nexul.toml` du projet, sinon 0.2 (`nxc check/build/run/test`, `nxc ide`, `nxpm`).
`studio/syntax` colore l'édition 1 (langage `nexul1` : mots, `!`, `===`, `|>`, `#`,
type rendu avant le nom), vérifié jeton par jeton contre le lexer de l'édition 1 ; NEXUL Studio
choisit l'édition de chaque fichier de la même façon et change de coloration dès qu'on écrit
`// edition: 1`.

**Revue du 16/10** : chaque fichier importé et chaque dépendance gardent leur édition (en-tête,
sinon le `nexul.toml` de leur paquet ; `--edition` ne vaut que pour le projet racine) ; un BOM
UTF-8 n'empêche plus de lire l'en-tête ni le `nexul.toml` ; `.Nom` dans un `else` ou un bras
suivant prend le type de la première branche ; le formateur garde les bras `.Variante`.

**Tranche 4, 16/10/2026** : le repli `else` (`let i64 n = t.parse_int() else 0`, `else { return }`,
en instruction `f() else println(…)`), compris comme `match valeur { Some(value) => value,
_ => repli }` (ou `Ok`) ; une variable qui possède sa valeur la donne (`match move`) ;
`E0349` si la valeur n'est ni `Option` ni `Result`. `x in a..b`, `x in a..=b` (bornes
comparées, `x` lu une fois) et `x in c` (`c.contains(x)`, `Map.contains` ajouté).
`test "…" expects panic` : le test passe s'il panique (code 101), échoue sinon. Tests :
`tests/run/edition1_fallback.nx`, `tests/ui/edition1_fallback.nx`, `compiler/nxc/tests/editions.rs`.

Reste : `===`/`!==` (avec les valeurs partagées), types distincts et contraints (`distinct`,
`where`), contrats (`requires`, `ensures`), puis la migration effective de `lib/` et des tests.

## Conséquences

- NEXUL a une signature reconnaissable : `type … is …`, `when … then`, `x!`, `else` de
  repli, `test "…"`, le type avant le nom, sans `->` ni `:` de typage.
- La sémantique de la 0.2 est réutilisée : la nouvelle syntaxe ne demande pas de réécrire
  l'analyse ni la génération de code.
- Coût : toute la bibliothèque standard et les tests passent par `nxc migrate` ; les
  `.stderr` des tests de diagnostics sont régénérés et relus.
- Le parser doit reconnaître un type à sa place (retour arrière borné) : c'est le prix de
  « type avant le nom », comme en C# ou en Java.

## Alternatives écartées

- **A seule** : trop proche de la 0.2 et de Rust (passation §3.1).
- **B seule** (`do … end`) : la plus lisible, mais plus longue à écrire, ce que
  l'utilisateur veut réduire.
- **C seule** : la plus courte, mais `v.[i]`, le retour implicite et les arguments `x = 1`
  surprennent ; le corps d'une expression (`= x * x`) en est gardé, pas le reste.
- **Type après le nom** (`x: i32`, `-> i32`) : écarté à la demande de l'utilisateur.
- **Indentation significative** : écartée (passation §3.1 : pas de règle dépendant
  d'espaces mal définis ; copier-coller fragile).

## Questions du 28/10/2026 — **décidées par l'utilisateur le 28/10/2026** (A pour Q1, la proposition pour Q2)

Constatées en migrant la bibliothèque standard (Claude Main). **Mises en œuvre le
28/10/2026** : `extend Type[P is …] { … }` sans interface est accepté quand il nomme les
paramètres du type dans l'ordre (sinon E0224) ; ses contraintes s'ajoutent à chacune de ses
méthodes ; un mot réservé peut nommer une méthode (dans `extend`, `interface` ou un type),
un champ ou un argument nommé, jamais une fonction libre (E0010). `nxc migrate` regroupe
les méthodes à `where` par jeu de contraintes et ne renomme plus les membres. Toute la
bibliothèque standard est écrite en édition 1. Tests : `tests/run/edition1_members.nx`,
`tests/ui/edition1_members.nx`.

**Q1. Une méthode contrainte sur un paramètre de son type.** En 0.2,
`impl Vec { fn contains(self, value: T) -> bool where T: Eq { … } }` ; en édition 1, ni
l'en-tête de la méthode ni `extend Vec[T is Eq] { … }` (refusé : E0224) ne peuvent l'écrire.
Un programme en édition 1 ne peut donc pas définir un conteneur générique complet.

- **Proposition A (recommandée)** : `extend Vec[T is Eq] { pub fn bool contains(self, T value) … }`
  — un bloc conditionnel, comme les `extend … is Interface` existants ; une seule règle à
  apprendre ; les méthodes n'existent que si la contrainte est satisfaite (message clair
  sinon). `nxc migrate` regroupe les méthodes par contrainte.
- **Proposition B** : `pub fn bool contains(self, T value) where T is Eq { … }` — plus
  proche de la 0.2, migration ligne à ligne, mais `where` y prend un second sens (il
  désigne déjà la condition d'un type contraint, ADR 0014).

**Q2. Une méthode publique dont le nom devient réservé** (`repeat`, `any`, `extend`,
`test`) : `v.any(f)` s'écrit déjà en édition 1 (un nom après un point n'est jamais un mot
clé), mais `fn bool any(…)` ne se déclare pas.

- **Proposition (recommandée)** : accepter un mot réservé comme **nom de méthode ou de
  champ** dans sa déclaration (le mot suit `fn T` ou un type, jamais en tête
  d'instruction), sans le permettre ailleurs. Aucun renommage de l'API, aucune rupture.
