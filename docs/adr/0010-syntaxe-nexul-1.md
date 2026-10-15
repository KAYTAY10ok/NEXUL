# 0010 — La syntaxe de NEXUL 1 (édition 1)

Lot P2, étape G2. Auteur : Claude Main. **Statut : proposée** (`CONÇU`), à valider par
l'utilisateur avant l'implémentation (G3). Tout le code de ce document est **illustratif**
tant que `nxc` ne l'accepte pas.

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

## La méthode : trois couches

Plutôt que de chercher des mots ou des signes inédits un par un (ce qui mène soit à copier
Rust, soit à l'illisible), la syntaxe est construite en **trois couches qui ne se mélangent
pas** :

1. **Les opérateurs et les littéraux sont ceux que tout le monde connaît** : `=`, `==`,
   `!=`, `<`, `+`, `"texte"`, `[1, 2]`, `0..10`. Aucune surprise.
2. **La structure et l'intention s'écrivent en mots** qui se lisent comme une phrase :
   `type … is …`, `when … then`, `if … is …`, `else` de repli, `repeat`, `test "…"`,
   `requires`, `with noalloc`, `every … within …`.
3. **Les effets ont un signe, et ce signe n'a qu'un sens** : `!` = « cette valeur est
   modifiée ici », `?` = « l'erreur remonte ici », `.Nom` = « variante du type attendu »,
   `|>` = « passe le résultat à ».

Un développeur lit la couche 1 sans rien apprendre, la couche 2 comme de l'anglais simple,
et n'a que quatre signes à retenir pour la couche 3. C'est cette combinaison, et non un mot
isolé, qui fait l'identité de NEXUL.

## Décision proposée

### 1. Égalités et comparaisons

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

### 2. Déclarations

| Écriture | Remplace (0.2) |
|---|---|
| `let x = 1`, `var y = 0` | inchangé |
| `fn nom(a: T) -> R { … return … }` | inchangé ; `return` obligatoire dans un bloc |
| `fn carre(x: i32) -> i32 = x * x` | **nouveau** : corps d'une expression |
| `type Point { x: f64, y: f64 … méthodes }` | `struct` + `impl` |
| `type Couleur = .Rouge \| .Vert \| .Rgb(r: u8, g: u8, b: u8)` | `enum` |
| `type Metres = distinct f64` | **nouveau** : type distinct (passation §4.2) |
| `type Pourcent = i32 where it in 0..=100` | **nouveau** : type contraint |
| `type Ligne = Vec[String]` | `type` (alias) |
| `interface Forme { fn aire(self) -> f64 }` | `trait` |
| `type Cercle is Forme { … }` | `impl Forme for Cercle` |
| `extend i32 is Affichable { … }` | `impl` sur un type existant |
| `any Forme` | `dyn Forme` |
| `fn f[T is Ord and Copy](…)` | `fn f[T: Ord + Copy](…)` |
| `test "description" { … }` | `@test fn nom() { … }` |

Un seul mot, `type`, déclare toutes les sortes de types ; les méthodes vivent dans le corps
du type. Une fonction du corps sans `self` est une fonction du type (`Compte.ouvrir(…)`).

### 3. Modification visible : `!`

Le `!` suit **toujours** la valeur modifiée :

| Où | Écriture |
|---|---|
| paramètre modifiable | `fn ajouter_point(t!: String)` |
| méthode qui modifie | `fn deposer(self!, montant: i64)` |
| argument | `ajouter_point(texte!)` |
| receveur | `compte!.deposer(150)`, `liste!.push(4)` |
| éléments d'une boucle | `for note! in notes { note += 1 }` |
| liaisons d'un filtre | `when arbre! { … }` |

Une affectation (`x = …`, `self.solde += …`) n'en a pas besoin : `=` montre déjà la
modification. Le `!` n'a aucun autre sens (la négation s'écrit `not`). `&` ne sert plus
qu'au ET bit à bit. Règle les points 4.3 et « receveur invisible » de G0 et G1.

### 4. Filtrer : `when … then`

```nexul
fn nom(c: Couleur) -> String = when c {
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

### 5. Conditions, boucles, repli

| Écriture | Sens |
|---|---|
| `if x > 0 { … } else { … }` | inchangé |
| `if valeur is .Some(n) { … }` | remplace `if let` |
| `while canal.recv() is .Some(x) { … }` | remplace `while let` |
| `for x in v { … }`, `for i in 0..10 { … }` | inchangé |
| `loop { … }` | remplace `while true` |
| `repeat 3 { … }` | **nouveau** : répéter n fois |
| `#exterieur for … { break #exterieur }` | étiquette (remplace `exterieur: for`) |
| `let n = texte.parse_int() else 0` | **repli** : si l'`Option` est vide ou le `Result` une erreur |
| `let f = fs.open(p) else { return }` | repli par un bloc qui doit sortir |
| `fs.write_text(p, t) else println("échec")` | repli en instruction |
| `let x = f()?` | inchangé : l'erreur remonte |

### 6. Fonctions anonymes et enchaînement

- `fn(x) => x * 2`, `fn(a, b) => a + b`, `fn(x) { … return … }` (0.2, analyse LL(1)).
- `valeurs |> map(fn(v) => v * 2) |> sum()` signifie `sum(map(valeurs, …))`. Une ligne
  peut commencer par `|>` ou par `.` : elle continue l'expression précédente.

### 7. Sûreté, concurrence, temps réel (formes réservées)

Leur sémantique vient avec P5, P7, P8. Jusque-là, `nxc` les **refuse** avec un message
« pas encore pris en charge » plutôt que de les accepter sans les vérifier.

| Écriture | Sens |
|---|---|
| `fn deposer(self!, m: i64) requires m > 0 ensures self.solde >= 0 { … }` | contrats |
| `fn lire(p: String) -> String with io` | effets déclarés et vérifiés |
| `fn regulation(etat!: Etat) every ms(10) within ms(2) with noalloc, nonblocking { … }` | tâche périodique temps réel |
| `let t = spawn { … }`, `t.join()` | tâche |
| `let (envoi, reception) = channel[i32](capacite: 8)` | canal borné (bibliothèque) |
| `pub`, `pub(package)` ; privé par défaut | visibilité |

### 8. Ce qui ne change pas (0.2)

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

Signes : ceux de la 0.2, plus `!` (postfixe, modification), `===`, `!==`, `|>`, `#nom`
(étiquette). `=>` ne sert plus qu'aux fonctions anonymes courtes.

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

fonction       = "fn" nom [ generiques ] "(" [ parametres ] ")" [ "->" type ] { clause } corps ;
parametres     = parametre { "," parametre } [ "," ] ;
parametre      = "self" [ "!" ] | [ "owned" ] nom [ "!" ] ":" type ;
generiques     = "[" generique { "," generique } "]" ;
generique      = nom [ "is" bornes ] ;
bornes         = type { "and" type } ;
clause         = "requires" expression | "ensures" expression
               | "with" nom { "," nom } | "every" expression [ "within" expression ] ;
corps          = bloc | "=" expression FIN ;

type_decl      = "type" nom [ generiques ] ( [ "is" bornes ] corps_type | "=" definition FIN ) ;
corps_type     = "{" { champ | { attribut } [ visibilite ] fonction } "}" ;
champ          = [ visibilite ] nom ":" type ( FIN | "," ) ;
definition     = variantes | "distinct" type [ restriction ] | type [ restriction ] ;
variantes      = [ "|" ] variante { "|" variante } ;
variante       = [ "indirect" ] "." nom [ "(" champ_v { "," champ_v } ")" ] ;
champ_v        = [ nom ":" ] type ;
restriction    = "where" expression ;                 (* « it » désigne la valeur *)
interface      = "interface" nom [ generiques ] [ "is" bornes ] "{" { { attribut } signature } "}" ;
signature      = "fn" nom [ generiques ] "(" [ parametres ] ")" [ "->" type ] { clause } [ corps ] FIN ;
extension      = "extend" type [ "is" bornes ] "{" { { attribut } [ visibilite ] fonction } "}" ;
test           = "test" chaine [ "expects" "panic" ] bloc ;
constante      = ( "const" | "static" [ "var" ] ) nom ":" type "=" expression FIN ;

type           = chemin [ "[" type { "," type } "]" ] | "any" chemin [ "[" type { "," type } "]" ]
               | "(" [ type { "," type } ] ")" | "[" type ";" expression "]"
               | "fn" "(" [ type { "," type } ] ")" [ "->" type ] | "Self" ;

bloc           = "{" { instruction FIN } "}" ;
instruction    = "let" motif [ ":" type ] "=" expression [ "else" repli ]
               | "var" nom [ ":" type ] [ "=" expression ]
               | expression affectation expression
               | [ etiquette ] boucle
               | expression [ "else" repli ] ;
affectation    = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
repli          = expression | bloc ;
etiquette      = "#" nom ;
boucle         = "for" motif [ "!" ] "in" expression bloc | "while" expression bloc
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
anonyme        = "fn" "(" [ parametres_a ] ")" ( "=>" expression | [ "->" type ] bloc ) ;
parametres_a   = nom [ ":" type ] { "," nom [ ":" type ] } ;

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

Analyse : LL(1) partout sauf la distinction `x = …` / expression (déjà en 0.2) ; un `else`
qui suit le bloc d'un `if` appartient toujours à ce `if`. Continuation de ligne : la ligne
finit par un opérateur binaire, `,`, `=`, ou un délimiteur ouvert ; ou la suivante commence
par `.`, `|>`, `|` (variantes), `else`.

## Les 12 programmes en édition 1

**1 — Bonjour**

```nexul
fn main() {
    println("Bonjour, NEXUL !")
}
```

**2 — Calcul**

```nexul
fn moyenne(notes: Vec[f64]) -> Option[f64] {
    if notes.is_empty() { return .None }
    var somme = 0.0
    for n in notes { somme += n }
    return .Some(somme / notes.len() as f64)
}

fn main() {
    let m = moyenne(Vec.from([12.0, 15.5, 9.0])) else 0.0
    println("moyenne : {m:.2}")
    repeat 3 { print("*") }
}
```

**3 — Structures et méthodes**

```nexul
pub type Compte {
    titulaire: String                         // privé
    solde: i64

    pub fn ouvrir(owned titulaire: String) -> Compte = Compte(titulaire: titulaire, solde: 0)

    pub fn deposer(self!, montant: i64) requires montant > 0 {
        self.solde += montant
    }

    pub fn solde(self) -> i64 = self.solde
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
    fn aire(self) -> f64
    fn nom(self) -> String
}

type Cercle is Forme {
    r: f64
    fn aire(self) -> f64 = 3.14159 * self.r * self.r
    fn nom(self) -> String = "cercle"
}

type Rectangle is Forme {
    l: f64
    h: f64
    fn aire(self) -> f64 = self.l * self.h
    fn nom(self) -> String = "rectangle"
}

fn main() {
    let formes: Vec[any Forme] = Vec.from([Cercle(r: 1.0), Rectangle(l: 2.0, h: 3.0)])
    for f in formes { println("{f.nom()} : {f.aire():.2}") }
    let total = formes |> map(fn(f) => f.aire()) |> sum()
    println("total : {total:.2}")
}
```

**5 — Génériques**

```nexul
fn plus_grand[T is Ord and Copy](valeurs: Vec[T]) -> Option[T] {
    var meilleur: Option[T] = .None
    for v in valeurs {
        if meilleur is .None or v > meilleur.unwrap() { meilleur = .Some(v) }
    }
    return meilleur
}

type Pile[T] {
    elements: Vec[T]
    fn empiler(self!, x: T) { self.elements!.push(x) }
    fn depiler(self!) -> Option[T] = self.elements!.pop()
}
```

**6 — Erreurs**

```nexul
fn diviser(a: i64, b: i64) -> Result[i64, String] {
    if b == 0 { return .Err("division par zéro") }
    return .Ok(a / b)
}

fn calcul() -> Result[i64, String] {
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
    let texte = fs.read_text("notes.txt") else {
        println("lecture impossible")
        return
    }
    fs.write_text("rapport.txt", "{texte.lines().count()} lignes\n") else println("écriture impossible")
}
```

**8 — Propriété des valeurs**

```nexul
fn afficher(t: String) = println(t)                      // lecture

fn ajouter_point(t!: String) { t!.push_str(".") }       // modification

fn garder(owned t: String) -> Vec[String] {             // prise de propriété
    var v: Vec[String] = Vec.new()
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
    var somme = 0
    while reception.recv() is .Some(x) { somme += x }
    producteur.join()
    println("somme : {somme}")
}
```

**10 — Tâche périodique temps réel**

```nexul
import std.rt.{ms, volatile_read, volatile_write}

@cfg("thumbv7em")
fn regulation(etat!: Etat) every ms(10) within ms(2) with noalloc, nonblocking {
    let mesure = volatile_read(CAPTEUR)
    etat.commande = etat.pid!.calcul(mesure)
    volatile_write(MOTEUR, etat.commande)
}
```

**11 — Web (WebAssembly)**

```nexul
@export
fn ajouter(a: i32, b: i32) -> i32 = a + b

@export
fn saluer(nom: String) -> String = "Bonjour, {nom} !"
```

**12 — Tests**

```nexul
fn carre(x: i32) -> i32 = x * x

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
| `struct X { … }` + `impl X { … }` | `type X { champs … fonctions }` | l'`impl` rejoint le type s'il est dans le même module, sinon `extend X` |
| `enum E { A, B(T) }` | `type E = .A \| .B(T)` | |
| `trait T`, `impl T for X` | `interface T`, `type X is T` ou `extend X is T` | |
| `dyn T` | `any T` | |
| `match e { P => v }` | `when e { P then v }` | |
| motif `A`, `Some(x)` | `.A`, `.Some(x)` | les noms seuls restent des variables |
| `if let P = e`, `while let P = e` | `if e is P`, `while e is P` | |
| `f(&x)`, `inout x: T`, `inout self` | `f(x!)`, `x!: T`, `self!` | |
| `v.m()` où `m` prend `inout self` | `v!.m()` | l'analyse sémantique dit quelles méthodes modifient |
| `for x in &v`, `match &e` | `for x! in v`, `when e!` | |
| `while true` | `loop` | |
| `a: for …`, `break a` | `#a for …`, `break #a` | |
| `[T: A + B]` | `[T is A and B]` | les clauses `where` passent en ligne |
| `@test fn nom() { … }` | `test "nom" { … }` | |

Le reste ne change pas. La migration part de l'AST, qui garde les positions (G0 §6) ;
commentaires et mise en page sont conservés. La 0.2 reste compilable sous `--edition 0.2`
pendant toute la transition ; l'édition se choisit par `edition = "1"` dans `nexul.toml`
ou `--edition 1`.

## Plan d'implémentation (G3)

1. **Lexer** : mots de l'édition 1 réservés selon l'édition ; `!` postfixe, `===`, `!==`,
   `|>`, `#nom`.
2. **Parser de l'édition 1** vers **le même AST** que la 0.2 quand la sémantique existe
   déjà (`type` → `struct`/`enum` + `impl`, `when` → `match`, `is` → `if let`, `x!` →
   argument `inout`, `extend` → `impl`, `test` → `@test`, `repeat` → `for`, `|>` → appel,
   `else` de repli → `match`) : la sémantique, le NIR et LLVM ne changent pas.
3. **Nouveau sens** : `===`, `in`, `distinct`, `where` (types contraints) ; les formes du §7
   sont refusées avec « pas encore pris en charge » jusqu'à P5, P7, P8.
4. **`nxc migrate`** : réécriture 0.2 → 1 ; mesure sur `lib/std/vec.nx` et
   `examples/calculatrice.nx` (recommandation 3 de G0), puis migration de `lib/`, des tests
   et des exemples.
5. **Formateur sur l'AST** (P6), **lexique fourni par le compilateur** (`nxc ide lexicon`)
   pour Studio et l'extension VS Code ; diagnostics et `nxc explain` revus.
6. **Tests** : chaque programme de cette ADR devient un test ; les suites `tests/run` et
   `tests/ui` passent dans les deux éditions pendant la transition.

## Conséquences

- NEXUL a une signature reconnaissable : `type … is …`, `when … then`, `x!`, `else` de
  repli, `test "…"`, sans changer les opérateurs que tout développeur connaît.
- La sémantique de la 0.2 est réutilisée : la nouvelle syntaxe ne demande pas de réécrire
  l'analyse ni la génération de code.
- Coût : toute la bibliothèque standard et les tests passent par `nxc migrate` ; les
  `.stderr` des tests de diagnostics sont régénérés et relus.

## Alternatives écartées

- **A seule** : trop proche de la 0.2 et de Rust (passation §3.1).
- **B seule** (`do … end`) : la plus lisible, mais plus longue à écrire, ce que
  l'utilisateur veut réduire.
- **C seule** : la plus courte, mais `v.[i]`, le retour implicite et les arguments `x = 1`
  surprennent ; le corps d'une expression (`= x * x`) en est gardé, pas le reste.
- **Indentation significative** : écartée (passation §3.1 : pas de règle dépendant
  d'espaces mal définis ; copier-coller fragile).
