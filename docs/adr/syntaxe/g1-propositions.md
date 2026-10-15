# G1 — Trois propositions de syntaxe pour NEXUL

Livrable G1 de la refonte de la syntaxe (passation V3, §3.2 ; `master-plan.md`, P2).
Auteur : Claude Main (reprend le travail d'Outils depuis l'équipe réduite du 15/10/2026).
S'appuie sur l'audit [G0](g0-audit.md).

Statut : **propositions, à choisir par l'utilisateur** (décision 2 du plan). Tous les
programmes ci-dessous sont **illustratifs** : `nxc` ne les accepte pas. Rien n'est décidé
avant l'ADR G2, qui donnera la grammaire EBNF complète de la proposition retenue.

## 1. Ce qui est commun aux trois propositions

Ces points viennent de G0 ; ils ne dépendent pas du choix.

| Point | Règle commune |
|---|---|
| Piège §4.1 (ligne coupée en deux) | Une instruction qui calcule une valeur sans effet et n'est pas la dernière de son bloc est refusée (`E0110`). **Déjà dans `nxc` 0.2** (lot de Main, 15/10). Pour continuer une expression, la ligne se termine par l'opérateur. |
| Piège §4.2 (`Vret` devient une variable) | Corrigé par la grammaire : une variante et une nouvelle variable ne s'écrivent plus pareil dans un motif (forme propre à chaque proposition). |
| Propriétés à garder (G0 §3) | fins de ligne significatives ; aucune expression ne commence par `{` ; analyse sans information sémantique ; `and`, `or`, `not` ; bit à bit lie plus fort que les comparaisons ; arithmétique vérifiée et `+%` visible ; interpolation `"{x}"` ; construction nommée ; `if` et `match` sont des expressions ; attributs `@nom` ; identificateurs ASCII ; `indirect` ; pas de suffixe de type sur les littéraux. |
| `&` | Ne signifie plus que le ET bit à bit ; la modification a sa propre marque (forme propre à chaque proposition). |
| Boucle infinie | `loop` (remplace `while true`). |
| Place réservée (G0 §4.10) | concurrence (`spawn` ou `task`, canaux, type « transférable ») ; temps réel (fonctions périodiques, effets `noalloc`, `nonblocking`, `isr`) ; contrats `requires` et `ensures` ; types distincts et contraints ; visibilité `pub` et `pub(package)`, privé par défaut. Leur sémantique vient avec P5, P7, P8. |
| Transition | La 0.2 reste acceptée sous `--edition 0.2` ; `nxc migrate` réécrit les sources depuis l'AST (G0 §6), commentaires et mise en page gardés. |

Les 12 programmes de la passation (§3.2) : 1 Bonjour, 2 calcul, 3 structures et méthodes,
4 interface polymorphe, 5 génériques, 6 erreurs, 7 fichiers, 8 propriété des valeurs,
9 threads et canaux, 10 tâche périodique temps réel, 11 Web (WASM), 12 tests.

---

## 2. Proposition A — « Continuité »

**Idée** : garder la 0.2 et corriger ce que G0 a trouvé. Accolades, `fn`, `let` et `var`,
`return` obligatoire, `struct` / `trait` / `impl`, génériques entre crochets.

**Règles qui changent**

| Sujet | 0.2 | A |
|---|---|---|
| Variante dans un motif | `Rouge`, `Some(x)` | `.Rouge`, `.Some(x)` ; un nom seul est toujours une nouvelle variable |
| Argument modifié | `f(&x)` | `f(inout x)` (le même mot que le paramètre) |
| Boucle ou `match` qui modifie | `for x in &v`, `match &e` | `for inout x in v`, `match inout e` |
| Étiquette de boucle | `exterieur: for …` | `#exterieur for …`, `break #exterieur` |
| Contrats | — | `requires` / `ensures` entre la signature et `{` |
| Effets, temps réel | — | attributs `@no_alloc`, `@nonblocking`, `@periodic(…)` |

**1 — Bonjour**

```nexul
fn main() {
    println("Bonjour, NEXUL !")
}
```

**2 — Calcul**

```nexul
fn moyenne(notes: Vec[f64]) -> Option[f64] {
    if notes.is_empty() {
        return None
    }
    var somme = 0.0
    for n in notes {
        somme += n
    }
    return Some(somme / notes.len() as f64)
}

fn main() {
    let ecart = 12.5 -
        3.0                                   // l'opérateur finit la ligne : une seule expression
    match moyenne(Vec.from([12.0, 15.5, 9.0])) {
        .Some(m) => println("moyenne : {m:.2}, écart : {ecart}")
        .None => println("aucune note")
    }
}
```

**3 — Structures et méthodes**

```nexul
pub struct Compte {
    titulaire: String                         // privé : seul ce module y accède
    solde: i64
}

impl Compte {
    pub fn ouvrir(titulaire: String) -> Compte {
        return Compte(titulaire: titulaire, solde: 0)
    }

    pub fn deposer(inout self, montant: i64)
        requires montant > 0
    {
        self.solde += montant
    }

    pub fn solde(self) -> i64 {
        return self.solde
    }
}

fn main() {
    var c = Compte.ouvrir("Ada")
    c.deposer(150)
    println("{c.solde()}")
}
```

**4 — Interface polymorphe**

```nexul
trait Forme {
    fn aire(self) -> f64
    fn nom(self) -> String
}

struct Cercle {
    r: f64
}

struct Rectangle {
    l: f64
    h: f64
}

impl Forme for Cercle {
    fn aire(self) -> f64 { return 3.14159 * self.r * self.r }
    fn nom(self) -> String { return "cercle" }
}

impl Forme for Rectangle {
    fn aire(self) -> f64 { return self.l * self.h }
    fn nom(self) -> String { return "rectangle" }
}

fn main() {
    let formes: Vec[dyn Forme] = Vec.from([Cercle(r: 1.0), Rectangle(l: 2.0, h: 3.0)])
    var total = 0.0
    for f in formes {
        println("{f.nom()} : {f.aire():.2}")
        total += f.aire()
    }
    println("total : {total:.2}")
}
```

**5 — Génériques**

```nexul
fn plus_grand[T: Ord + Copy](valeurs: Vec[T]) -> Option[T] {
    var meilleur: Option[T] = None
    for v in valeurs {
        if meilleur.is_none() or v > meilleur.unwrap() {
            meilleur = Some(v)
        }
    }
    return meilleur
}

struct Pile[T] {
    elements: Vec[T]
}

impl Pile[T] {
    fn empiler(inout self, x: T) { self.elements.push(x) }
    fn depiler(inout self) -> Option[T] { return self.elements.pop() }
}
```

**6 — Erreurs**

```nexul
fn diviser(a: i64, b: i64) -> Result[i64, String] {
    if b == 0 {
        return Err("division par zéro")
    }
    return Ok(a / b)
}

fn calcul() -> Result[i64, String] {
    let x = diviser(10, 2)?
    return Ok(diviser(x, 0)?)
}

fn main() {
    match calcul() {
        .Ok(v) => println("résultat : {v}")
        .Err(e) => println("erreur : {e}")
    }
}
```

**7 — Fichiers**

```nexul
import std.fs

fn compter_lignes(chemin: String) -> Result[u64, fs.Erreur] {
    let texte = fs.read_text(chemin)?
    return Ok(texte.lines().count())
}

fn main() {
    match compter_lignes("notes.txt") {
        .Ok(n) => fs.write_text("rapport.txt", "{n} lignes\n").expect("écriture du rapport")
        .Err(e) => println("lecture impossible : {e}")
    }
}
```

**8 — Propriété des valeurs**

```nexul
fn afficher(t: String) {                      // lecture
    println(t)
}

fn ajouter_point(inout t: String) {           // modification
    t.push_str(".")
}

fn garder(owned t: String) -> Vec[String] {   // prise de propriété
    var v: Vec[String] = Vec.new()
    v.push(move t)
    return v
}

fn main() {
    var texte = "Bonjour"
    afficher(texte)
    ajouter_point(inout texte)                // la modification se voit à l'appel
    let liste = garder(move texte)            // `texte` n'est plus utilisable
    println("{liste.len()}")
}
```

**9 — Threads et canaux**

```nexul
import std.task

fn main() {
    let (envoi, reception) = task.channel[i32](capacite: 8)
    let producteur = spawn fn() {             // `envoi` est transféré dans la tâche
        for i in 0..10 {
            envoi.send(i * i)
        }
    }
    var somme = 0
    while let .Some(x) = reception.recv() {
        somme += x
    }
    producteur.join()
    println("somme : {somme}")
}
```

**10 — Tâche périodique temps réel**

```nexul
import std.rt

@cfg("thumbv7em")
@periodic(periode: rt.ms(10), echeance: rt.ms(2))
@no_alloc @nonblocking
fn regulation(inout etat: Etat) {
    let mesure = rt.volatile_read(CAPTEUR)
    etat.commande = etat.pid.calcul(mesure)
    rt.volatile_write(MOTEUR, etat.commande)
}
```

**11 — Web (WebAssembly)**

```nexul
@export
fn ajouter(a: i32, b: i32) -> i32 {
    return a + b
}

@export
fn saluer(nom: String) -> String {
    return "Bonjour, {nom} !"
}
```

**12 — Tests**

```nexul
fn carre(x: i32) -> i32 {
    return x * x
}

@test
fn carre_positif() {
    assert(carre(3) == 9)
}

@test(panique)
fn depassement() {
    let _ = carre(100000)                     // arithmétique vérifiée : panique
}
```

**Bilan de A** : coût de migration le plus faible (surtout `&x` → `inout x` et les motifs),
aucune surprise pour qui connaît la 0.2. Mais l'allure générale reste celle de la 0.2, donc
proche de Rust : c'est le risque d'un « simple renommage » que la passation (§3.1) écarte.
L'ambiguïté des crochets (G0 §4.5) reste.

---

## 3. Proposition B — « Phrases »

**Idée** : la structure se lit en mots. Le code s'ouvre par `do`, les choix par `then`,
et tout bloc se ferme par `end`. Les déclarations (`struct`, `trait`, `enum`) s'ouvrent par
leur mot et se ferment par `end`. Un type porte ses méthodes dans son corps.

**Règles qui changent**

| Sujet | 0.2 | B |
|---|---|---|
| Blocs | `{ … }` | `do … end` (code), `then … else … end` (choix), déclaration … `end` |
| Fonction | `fn` | `func` |
| Méthodes | dans un `impl` séparé | dans le corps du type ; `struct Cercle is Forme` déclare l'interface |
| Variante dans un motif | `Some(x)` | `Some(let x)` : une **nouvelle variable** s'écrit avec `let` ; un nom seul est toujours une variante ou une constante |
| Filtrer | `match e { A => … }` | `match e` puis des lignes `case A then …`, puis `end` |
| Tester un motif | `if let Some(n) = e` | `if e is Some(let n) then` |
| Argument modifié | `f(&x)`, `inout x: T` | `f(var x)`, paramètre `var x: T` (le mot de la déclaration modifiable) |
| Erreur propagée | `f()?` | `try f()` |
| Étiquette de boucle | `exterieur: for …` | `for x in v named exterieur do`, `break exterieur` |
| Effets, temps réel | — | `with noalloc, nonblocking` ; `periodic func … every … within …` |
| Tests | `@test fn nom()` | `test "description" do … end` |

**1 — Bonjour**

```nexul
func main() do
    println("Bonjour, NEXUL !")
end
```

**2 — Calcul**

```nexul
func moyenne(notes: Vec[f64]) -> Option[f64] do
    if notes.is_empty() then
        return None
    end
    var somme = 0.0
    for n in notes do
        somme += n
    end
    return Some(somme / notes.len() as f64)
end

func main() do
    let ecart = 12.5 -
        3.0
    match moyenne(Vec.from([12.0, 15.5, 9.0]))
    case Some(let m) then println("moyenne : {m:.2}, écart : {ecart}")
    case None then println("aucune note")
    end
end
```

**3 — Structures et méthodes**

```nexul
pub struct Compte
    titulaire: String                         // privé : seul ce module y accède
    solde: i64

    pub func ouvrir(titulaire: String) -> Compte do
        return Compte(titulaire: titulaire, solde: 0)
    end

    pub func deposer(var self, montant: i64) requires montant > 0 do
        self.solde += montant
    end

    pub func solde(self) -> i64 do
        return self.solde
    end
end

func main() do
    var c = Compte.ouvrir("Ada")
    c.deposer(150)
    println("{c.solde()}")
end
```

**4 — Interface polymorphe**

```nexul
trait Forme
    func aire(self) -> f64
    func nom(self) -> String
end

struct Cercle is Forme
    r: f64

    func aire(self) -> f64 do return 3.14159 * self.r * self.r end
    func nom(self) -> String do return "cercle" end
end

struct Rectangle is Forme
    l: f64
    h: f64

    func aire(self) -> f64 do return self.l * self.h end
    func nom(self) -> String do return "rectangle" end
end

func main() do
    let formes: Vec[dyn Forme] = Vec.from([Cercle(r: 1.0), Rectangle(l: 2.0, h: 3.0)])
    var total = 0.0
    for f in formes do
        println("{f.nom()} : {f.aire():.2}")
        total += f.aire()
    end
    println("total : {total:.2}")
end
```

**5 — Génériques**

```nexul
func plus_grand[T: Ord + Copy](valeurs: Vec[T]) -> Option[T] do
    var meilleur: Option[T] = None
    for v in valeurs do
        if meilleur.is_none() or v > meilleur.unwrap() then
            meilleur = Some(v)
        end
    end
    return meilleur
end

struct Pile[T]
    elements: Vec[T]

    func empiler(var self, x: T) do self.elements.push(x) end
    func depiler(var self) -> Option[T] do return self.elements.pop() end
end
```

**6 — Erreurs**

```nexul
func diviser(a: i64, b: i64) -> Result[i64, String] do
    if b == 0 then
        return Err("division par zéro")
    end
    return Ok(a / b)
end

func calcul() -> Result[i64, String] do
    let x = try diviser(10, 2)
    return Ok(try diviser(x, 0))
end

func main() do
    match calcul()
    case Ok(let v) then println("résultat : {v}")
    case Err(let e) then println("erreur : {e}")
    end
end
```

**7 — Fichiers**

```nexul
import std.fs

func compter_lignes(chemin: String) -> Result[u64, fs.Erreur] do
    let texte = try fs.read_text(chemin)
    return Ok(texte.lines().count())
end

func main() do
    match compter_lignes("notes.txt")
    case Ok(let n) then fs.write_text("rapport.txt", "{n} lignes\n").expect("écriture du rapport")
    case Err(let e) then println("lecture impossible : {e}")
    end
end
```

**8 — Propriété des valeurs**

```nexul
func afficher(t: String) do                   // lecture
    println(t)
end

func ajouter_point(var t: String) do          // modification
    t.push_str(".")
end

func garder(owned t: String) -> Vec[String] do
    var v: Vec[String] = Vec.new()
    v.push(move t)
    return v
end

func main() do
    var texte = "Bonjour"
    afficher(texte)
    ajouter_point(var texte)                  // la modification se voit à l'appel
    let liste = garder(move texte)
    println("{liste.len()}")
end
```

**9 — Threads et canaux**

```nexul
import std.task

func main() do
    let (envoi, reception) = task.channel[i32](capacite: 8)
    let producteur = task do                  // `envoi` est transféré dans la tâche
        for i in 0..10 do
            envoi.send(i * i)
        end
    end
    var somme = 0
    while reception.recv() is Some(let x) do
        somme += x
    end
    producteur.join()
    println("somme : {somme}")
end
```

**10 — Tâche périodique temps réel**

```nexul
import std.rt

@cfg("thumbv7em")
periodic func regulation(var etat: Etat) every rt.ms(10) within rt.ms(2) with noalloc, nonblocking do
    let mesure = rt.volatile_read(CAPTEUR)
    etat.commande = etat.pid.calcul(mesure)
    rt.volatile_write(MOTEUR, etat.commande)
end
```

**11 — Web (WebAssembly)**

```nexul
@export
func ajouter(a: i32, b: i32) -> i32 do
    return a + b
end

@export
func saluer(nom: String) -> String do
    return "Bonjour, {nom} !"
end
```

**12 — Tests**

```nexul
func carre(x: i32) -> i32 do
    return x * x
end

test "carré d'un nombre positif" do
    assert(carre(3) == 9)
end

test "le dépassement panique" expects panic do
    let _ = carre(100000)
end
```

**Bilan de B** : l'identité la plus nette (ni Rust, ni C, ni Python), la plus lisible pour
un débutant ; chaque `end` ferme un mot connu, ce qui donne des erreurs précises
(« `end` de `func calcul` manquant ») et une bonne reprise sur erreur. Les effets et le
temps réel se lisent comme des phrases. Coût : **chaque bloc** est réécrit (mécanique avec
`nxc migrate`), le code est un peu plus long en caractères ; l'ambiguïté des crochets reste.

---

## 4. Proposition C — « Signes justes »

**Idée** : garder des accolades, mais **un signe n'a qu'un sens**. Le `!` marque toujours
une modification, `:` toujours un type, `[ ]` toujours des paramètres de type ; le corps
d'une fonction est une expression.

**Règles qui changent**

| Sujet | 0.2 | C |
|---|---|---|
| Argument modifié | `f(&x)` | `f(x!)` ; paramètre `x!: T` |
| Méthode qui modifie | `fn m(inout self)`, appel `v.m()` | `fn m!(self)`, appel `v.m!()` : la modification se voit aussi sur le receveur |
| Boucle ou `match` qui modifie | `for x in &v`, `match &e` | `for x! in v`, `match e! { … }` |
| Indexation | `v[i]` | `v.[i]` ; `[ ]` après un nom = paramètres de type seulement |
| Arguments nommés | `Point(x: 1.0)` | `Point(x = 1.0)` : `:` ne sert plus qu'aux types |
| Variante dans un motif | `Some(x)` | `.Some(x)` ; un nom seul est une nouvelle variable |
| Corps de fonction | `return` obligatoire | dernière expression = valeur ; `return` pour sortir plus tôt ; forme courte `fn f(x: i32) -> i32 = x * x` |
| Fonction anonyme | `fn(x) => x + 1` | `x => x + 1`, `(a, b) => a + b` |
| Enchaînement | — | `x |> f(y)` signifie `f(x, y)` ; une ligne peut commencer par `|>` |
| Étiquette de boucle | `exterieur: for …` | `#exterieur for …`, `break #exterieur` |

**1 — Bonjour**

```nexul
fn main() {
    println("Bonjour, NEXUL !")
}
```

**2 — Calcul**

```nexul
fn moyenne(notes: Vec[f64]) -> Option[f64] {
    if notes.is_empty() { return None }
    var somme = 0.0
    for n in notes { somme += n }
    Some(somme / notes.len() as f64)          // dernière expression : la valeur rendue
}

fn main() {
    let ecart = 12.5 -
        3.0
    match moyenne(Vec.from([12.0, 15.5, 9.0])) {
        .Some(m) => println("moyenne : {m:.2}, écart : {ecart}")
        .None => println("aucune note")
    }
}
```

**3 — Structures et méthodes**

```nexul
pub struct Compte {
    titulaire: String                         // privé : seul ce module y accède
    solde: i64
}

impl Compte {
    pub fn ouvrir(titulaire: String) -> Compte = Compte(titulaire = titulaire, solde = 0)

    pub fn deposer!(self, montant: i64) requires montant > 0 {
        self.solde += montant
    }

    pub fn solde(self) -> i64 = self.solde
}

fn main() {
    var c = Compte.ouvrir("Ada")
    c.deposer!(150)                           // `!` : `c` est modifié
    println("{c.solde()}")
}
```

**4 — Interface polymorphe**

```nexul
import std.iter.{map, sum}

trait Forme {
    fn aire(self) -> f64
    fn nom(self) -> String
}

struct Cercle {
    r: f64
}

struct Rectangle {
    l: f64
    h: f64
}

impl Forme for Cercle {
    fn aire(self) -> f64 = 3.14159 * self.r * self.r
    fn nom(self) -> String = "cercle"
}

impl Forme for Rectangle {
    fn aire(self) -> f64 = self.l * self.h
    fn nom(self) -> String = "rectangle"
}

fn main() {
    let formes: Vec[dyn Forme] = Vec.from([Cercle(r = 1.0), Rectangle(l = 2.0, h = 3.0)])
    for f in formes { println("{f.nom()} : {f.aire():.2}") }
    let total = formes
        |> map(f => f.aire())
        |> sum()
    println("total : {total:.2}")
}
```

**5 — Génériques**

```nexul
fn plus_grand[T: Ord + Copy](valeurs: Vec[T]) -> Option[T] {
    var meilleur: Option[T] = None
    for v in valeurs {
        if meilleur.is_none() or v > meilleur.unwrap() { meilleur = Some(v) }
    }
    meilleur
}

struct Pile[T] {
    elements: Vec[T]
}

impl Pile[T] {
    fn empiler!(self, x: T) { self.elements.push!(x) }
    fn depiler!(self) -> Option[T] = self.elements.pop!()
    fn premier(self) -> T = self.elements.[0]
}
```

**6 — Erreurs**

```nexul
fn diviser(a: i64, b: i64) -> Result[i64, String] {
    if b == 0 { return Err("division par zéro") }
    Ok(a / b)
}

fn calcul() -> Result[i64, String] {
    let x = diviser(10, 2)?
    Ok(diviser(x, 0)?)
}

fn main() {
    match calcul() {
        .Ok(v) => println("résultat : {v}")
        .Err(e) => println("erreur : {e}")
    }
}
```

**7 — Fichiers**

```nexul
import std.fs

fn compter_lignes(chemin: String) -> Result[u64, fs.Erreur] {
    let texte = fs.read_text(chemin)?
    Ok(texte.lines().count())
}

fn main() {
    match compter_lignes("notes.txt") {
        .Ok(n) => fs.write_text("rapport.txt", "{n} lignes\n").expect("écriture du rapport")
        .Err(e) => println("lecture impossible : {e}")
    }
}
```

**8 — Propriété des valeurs**

```nexul
fn afficher(t: String) {                      // lecture
    println(t)
}

fn ajouter_point(t!: String) {                // modification
    t.push_str!(".")
}

fn garder(owned t: String) -> Vec[String] {   // prise de propriété
    var v: Vec[String] = Vec.new()
    v.push!(move t)
    v
}

fn main() {
    var texte = "Bonjour"
    afficher(texte)
    ajouter_point(texte!)                     // la modification se voit à l'appel
    let liste = garder(move texte)
    println("{liste.len()}")
}
```

**9 — Threads et canaux**

```nexul
import std.task

fn main() {
    let (envoi, reception) = task.channel[i32](capacite = 8)
    let producteur = spawn {                  // `envoi` est transféré dans la tâche
        for i in 0..10 { envoi.send(i * i) }
    }
    var somme = 0
    while let .Some(x) = reception.recv() { somme += x }
    producteur.join()
    println("somme : {somme}")
}
```

**10 — Tâche périodique temps réel**

```nexul
import std.rt

@cfg("thumbv7em")
@periodic(periode = rt.ms(10), echeance = rt.ms(2))
@no_alloc @nonblocking
fn regulation(etat!: Etat) {
    let mesure = rt.volatile_read(CAPTEUR)
    etat.commande = etat.pid.calcul!(mesure)
    rt.volatile_write(MOTEUR, etat.commande)
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

@test
fn carre_positif() {
    assert(carre(3) == 9)
}

@test(panique)
fn depassement() {
    let _ = carre(100000)
}
```

**Bilan de C** : le plus court en lignes et en caractères (pas en jetons, voir §5) ; la modification se voit partout (`!`), y compris sur le
receveur d'une méthode, ce que ni la 0.2 ni A ni B ne montrent ; les crochets ne sont plus
ambigus (G0 §4.5) ; `:` retrouve un seul sens (G0 §4.4). Risques : le retour implicite
cache des sorties de fonction (G0 §4.7 défendait `return`), `!` peut rappeler les macros de
Rust ou la négation du C, `v.[i]` surprend. Coût de migration moyen : marques `!`,
arguments nommés, indexations, corps de fonction.

---

## 5. Comparaison

**Problèmes de G0 réglés**

| G0 | A | B | C |
|---|---|---|---|
| §4.1 ligne coupée en deux | E0110 (commun) | E0110 (commun) | E0110 (commun) |
| §4.2 variante / variable | oui (`.Vert`) | oui (`let x` pour une variable) | oui (`.Vert`) |
| §4.3 `&` à trois sens | oui (`inout`) | oui (`var`) | oui (`!`) |
| §4.4 `:` à six sens | en partie (étiquettes) | en partie (étiquettes) | **oui** (sauf format de l'interpolation) |
| §4.5 crochets générique / index | non | non | **oui** (`v.[i]`) |
| §4.6 le point sert à tout | non | non | non |
| §4.7 corps de fonction sans valeur | non (choix : `return` visible) | non (même choix) | oui (dernière expression) |
| §4.8 `fn` et `=>` à deux rôles | non | en partie (`func`, pas de `=>` dans `match`) | en partie (`fn` déclare seulement) |
| §4.9 `while true` | `loop` | `loop` | `loop` |
| Modification du receveur visible | non | non | **oui** |

**Critères** (++ très bon, + bon, − faible, −− mauvais ; appréciation de Main, à discuter)

| Critère | A | B | C |
|---|---|---|---|
| Identité propre (passation §3.1) | − | ++ | + |
| Lisibilité pour un débutant | + | ++ | + |
| Concision (lignes, caractères) | + | − | ++ |
| Analyse sans information sémantique (LL(1)) | + | + | ++ |
| Messages et reprise sur erreur | + | ++ | + |
| Effets, contrats, temps réel lisibles | + | ++ | + |
| Coût de migration de `lib/` et des tests | ++ | −− | − |
| Risque de surprise pour un développeur Rust/C | + | + | − |

**Mesures** : nombre approximatif de jetons des 12 programmes, découpage lexical simple
(mots, nombres, chaînes, symboles), commentaires exclus. Mesure indicative ; la vraie mesure
(G0, recommandation 3) se fera en G2 sur `lib/std/vec.nx` et `examples/calculatrice.nx`
migrés par un prototype de `nxc migrate`.

| Proposition | Programmes | Lignes | Caractères | Jetons | dont mots | dont symboles |
|---|---:|---:|---:|---:|---:|---:|
| A | 12 | 175 | 3325 | 1035 | 460 | 525 |
| B | 12 | 161 | 3518 | 994 | 557 | 385 |
| C | 12 | 152 | 3280 | 1045 | 451 | 544 |

Lecture : B a le moins de jetons et de symboles, mais le plus de mots et de caractères ; C a le moins de lignes et de caractères, mais le plus de symboles ; A a le plus de lignes. Les écarts restent faibles (moins de 7 %) : le choix se fait sur la lisibilité et l'identité plus que sur la longueur.

## 6. Recommandation de Claude Main

**B « Phrases »**, pour trois raisons :

1. C'est la seule qui répond pleinement à la passation (§3.1) : une syntaxe propre,
   reconnaissable et lisible, et pas une 0.2 retouchée à l'allure de Rust.
2. Les chantiers qui viennent (effets, contrats, concurrence, temps réel) s'y écrivent en
   mots clairs (`with noalloc`, `every … within …`, `requires`) au lieu de s'empiler en
   attributs et en signes.
3. Son coût principal, la réécriture des blocs, est **mécanique** : `nxc migrate` part de
   l'AST, qui garde les positions (G0 §6) ; il ne change pas la sémantique.

Deux éléments de C méritent d'être repris quel que soit le choix, à trancher en G2 :
la marque de modification sur le receveur d'une méthode, et l'enchaînement `|>`.

Si l'utilisateur préfère garder les accolades, **C** est le meilleur second choix ; **A**
n'est recommandée que si le coût de migration doit être minimal.

## 7. Suite

1. L'utilisateur choisit A, B, C ou un mélange (décision 2 du plan).
2. G2 (Main) : ADR et grammaire EBNF de la proposition retenue, précédences, style,
   lexique fourni par le compilateur (`nxc ide lexicon`), stratégie d'édition.
3. Prototype : parser de la nouvelle syntaxe derrière `--edition 2`, et `nxc migrate` sur
   `lib/std/vec.nx` et `examples/calculatrice.nx` pour mesurer le coût réel.
4. Puis G3 (implémentation complète), coloration de Studio et de l'extension VS Code.
