# G0 — Audit des usages de la syntaxe de NEXUL 0.2

Livrable G0 de la refonte de la syntaxe (passation V3, §3.2 ; `master-plan.md`, P2).
Auteur : Claude Outils / Core. Relecture : Claude Main.

Statut : **étude**. Ce document ne décide rien et ne propose aucune nouvelle syntaxe :
les propositions sont l'objet de G1, la décision de G2 (ADR validée par l'utilisateur).
Aucun fichier du lexer ni du parser n'a été modifié.

## 1. Méthode

**Corpus** : toutes les sources NEXUL du dépôt (`main` du 15/10/2026, `edb886c`), plus
les blocs de code NEXUL de la documentation.

| Famille | Fichiers ou blocs | Lignes | Jetons |
|---|---:|---:|---:|
| Bibliothèque standard et runtime (`lib/`) | 21 | 9 499 | 58 327 |
| Exemples (`examples/`) | 8 | 486 | 2 800 |
| Tests d'exécution (`tests/run/`) | 36 | 1 824 | 10 432 |
| Tests de diagnostics (`tests/ui/`) | 63 | 865 | 4 246 |
| Autres tests (`tests/numerics/`, `tests/web/`) | 5 | 206 | 1 316 |
| Blocs `nexul` de la documentation (guide, spécification, README) | 37 | 554 | 3 225 |
| **Total** | **170** | **13 434** | **80 346** |

**Outil** : `compiler/nxc_syntax/examples/g0_census.rs`. Il passe chaque source dans le
**vrai lexer et le vrai parser** de `nxc`, puis compte chaque forme dans l'arbre
syntaxique et dans les jetons. Il ne fait que lire. Pour le relancer :

```
cargo run -p nxc_syntax --example g0_census -- .
```

Le résultat complet est en annexe : [g0-recensement.md](g0-recensement.md).

Limites :

- Ce sont des comptes **syntaxiques**. Par exemple, `a[i]` et `Vec[i32]` en position
  d'expression forment le même nœud ; seule l'analyse sémantique les distingue.
- `while let` est réécrit par le parser en `while true { if let … }` : il est compté
  sur les jetons.
- 7 tests de `tests/ui/` contiennent volontairement des erreurs de syntaxe. 5 blocs de la
  spécification sont des fragments illustratifs (déclarations et instructions mêlées, ou
  `...`) ; ils sont comptés tels quels.
- Studio n'a pas de source `.nx`. Ses modèles de projet (`studio/templates`) contiennent
  5 petits fichiers NEXUL (3 modèles) et l'exemple web, dans des chaînes Rust ; ils sont
  lus à part (§6).
- Les deux pièges du §4.1 et du §4.2 ont été **vérifiés avec `nxc check`** sur ce PC
  (Rust 1.99, `nxc` 0.2 construit depuis `main`).

## 2. Vue d'ensemble

**La bibliothèque standard porte 73 % des jetons.** Toute modification de la syntaxe se
paie d'abord là. Elle a aussi son propre dialecte : `unsafe`, `extern`, `@lang`,
`@intrinsic`, conversions `as`, opérateurs bit à bit. À l'inverse, elle n'utilise **ni**
`?`, **ni** fonctions anonymes, **ni** `match &`, que l'on trouve dans les exemples et les
tests.

Les formes les plus fréquentes, sur tout le corpus :

| Forme | Nombre | Remarque |
|---|---:|---|
| appels de méthode ou de module `x.f(…)` / appels simples `f(…)` | 2 043 / 1 699 | le point sert à tout (§4.6) |
| `return valeur` | 1 250 | obligatoire dans un corps de fonction (§4.7) |
| fonctions déclarées | 1 143 | 678 libres, 442 dans un `impl`, 23 dans un `trait` |
| `let` / `var` | 1 106 / 584 | `var` porte un type dans 254 cas sur 584 |
| `if` / `else` / `else if` | 961 / 209 / 43 | 859 `if` dans `lib/` |
| paramètres `nom: T` (lecture) | 886 | `self` : 331 |
| `as` (conversion) | 569 | 509 dans `lib/` |
| types génériques appliqués `T[A]` | 527 | |
| chaînes interpolées `"{x}"` | 349 | 755 interpolations, dont 495 expressions plus complexes qu'un nom |
| `match` / bras | 123 / 338 | 8 gardes `if` |
| `while` / `for` | 122 / 179 | dont 37 `while true` (fichiers `.nx`) et 2 `while let` |
| `unsafe { … }` / `unsafe fn` | 111 / 103 | 94 et 98 dans `lib/` |
| appel avec `&x` (argument `inout`) | 135 | |

Sont **rares** (13 emplois ou moins dans tout le dépôt) : étiquettes de boucle (4),
`while let` (2), `for x in &v` (5), `if let … = &e` (5), `match &e` (5), gardes (8),
motifs `a | b` (11), `dyn` (5), `type` (2), `module` (1), `import … as` (4), `.0` (4),
`indirect` (12), `move` (13), `owned self` (12), super-traits (1). Ces formes coûtent peu à
changer, mais leur rareté signale aussi qu'elles sont peu éprouvées.

## 3. Formes à conserver

Chacune est motivée par la lisibilité ou par la sûreté, et non par l'habitude. Ce sont
des propriétés à garder ; leur écriture exacte reste à choisir en G1.

1. **Fins de ligne significatives, `;` facultatif** (ADR 0005). Le corpus ne contient que
   70 `;`, tous entre deux instructions sur une même ligne ; aucune ligne de code ne finit
   par `;`. La règle doit être revue sur un point (§4.1).
2. **Une accolade ne commence jamais une expression** (spécification §2.1). Pas
   d'ambiguïté bloc / littéral, analyse LL(1) sans information sémantique. C'est un
   atout pour le formateur, la coloration et la reprise sur erreur.
3. **Conventions de passage explicites** : lecture par défaut, `inout`, `owned`, et
   **marque obligatoire à l'appel** pour un argument modifié (135 emplois). Le lecteur
   voit quelles variables un appel modifie. C'est le cœur de la sûreté sans références
   stockées (§5 de la spécification) ; le futur modèle de concurrence en dépendra.
4. **Opérateurs logiques en mots** `and`, `or`, `not` (115, 122, 54). Lisibles, sans
   confusion avec les opérateurs bit à bit `&`, `|`, `~`.
5. **Les opérateurs bit à bit lient plus fort que les comparaisons**, et les comparaisons
   ne s'enchaînent pas (`a < b < c` refusé, E0104). Deux pièges du C en moins.
6. **Arithmétique vérifiée, et arithmétique modulaire visible** (`+%`, `-%`, `*%` : 34
   emplois, presque tous dans `lib/`). Le choix se lit dans le code (ADR 0006).
7. **Interpolation `"{expr}"` avec format `{x:.2}`**. Très employée (349 chaînes) ; 495
   interpolations sont plus que de simples noms (`{v.len()}`, `{a + b}`).
8. **Construction nommée et ordonnée** `Point(x: 1.0, y: 2.0)` : l'ordre d'évaluation et
   le sens de chaque valeur sont évidents (E0310).
9. **`if` et `match` sont des expressions** ; un `if` valeur exige son `else` (E0330).
10. **Attributs `@nom(…)`** : une seule forme pour les tests, la plateforme, l'exportation
    C et les liens avec le compilateur (12 noms, §5).
11. **Identificateurs ASCII et caractères bidirectionnels interdits** (ADR 0007).
12. **`indirect`** pour les types récursifs : le stockage sur le tas reste transparent à
    la lecture.
13. **Pas de suffixe de type sur les littéraux** : `let x: u8 = 200` ou `200 as u8`.

## 4. Faiblesses et ambiguïtés constatées

### 4.1 Une ligne qui commence par `-` devient silencieusement une autre instruction

La règle des fins de ligne (spécification §1.7) coupe après un identificateur. Ainsi :

```nexul
let total = a
    - b
```

donne deux instructions : `let total = a`, puis l'expression `-b`, dont la valeur est
perdue. **Vérifié** : `nxc check` accepte ce programme sans erreur ni avertissement
(code 0). La spécification ne cite que le cas `+ b`, refusé (E0102) parce que le `+`
préfixe n'existe pas. Une ligne qui commence par `not`, `move`, `(…)` ou `[…]` donne le
même résultat : **vérifié** aussi, chaque fois sans aucun message.

Impact : c'est le seul piège trouvé où un programme mal découpé compile **sans aucun
message**. G1 doit soit rendre ce cas impossible par la grammaire, soit le diagnostiquer.

### 4.2 Un identificateur de motif a trois sens

Dans `match`, un nom seul désigne d'abord une variante du type filtré, puis une variante
ou une constante visible, et sinon **une nouvelle variable** (spécification §2.6). Une
faute de frappe (`Vret` au lieu de `Vert`) crée donc une variable qui accepte tout.
**Vérifié** : c'est seulement un avertissement (W0007, plus W0001 « variable inutilisée »),
et le programme compile. Le cas est fréquent : 150 motifs constructeurs et 109 motifs
littéraux dans le corpus, et les variantes sans données s'écrivent presque toujours en nom
seul (2 motifs qualifiés `a.B` seulement).

### 4.3 Le `&` a trois sens

| Emploi | Nombre | Sens |
|---|---:|---|
| `a & masque` | 132 | ET bit à bit |
| `f(&x)` | 135 | argument `inout` : `x` peut être modifié |
| `for x in &v`, `match &e`, `if let P = &e`, `while let P = &e` | 15 | les liaisons modifient `e` en place |

Pour un lecteur venu du C, de Rust ou de C++, `&x` signifie « adresse » ou « référence »,
alors qu'ici il signifie « modifiable ». Dans `for x in &v`, rien n'indique que `x` est
modifiable : c'est le `&` placé sur `v` qui le décide.

### 4.4 Le `:` a six sens

Annotation de type (`let x: T`, paramètres, champs), argument nommé et construction
(`Point(x: 1)`), étiquette de boucle (`exterieur: for …`), borne générique (`[T: Ord]`),
champ nommé dans un motif (`Point(x: a)`), format d'interpolation (`{x:.2}`). C'est le
symbole le plus fréquent après les parenthèses, les accolades, le point et le `=` (1 989
jetons). L'étiquette de boucle se lit comme une déclaration typée.

### 4.5 Les crochets : générique ou indexation

`Vec[i32]` et `v[i]` ont la même forme. En position de type, pas d'ambiguïté (527
emplois). En position d'expression (`Vec[i32].new()`, `taille[T]()`, 325 nœuds), le
parser produit un nœud « indexation » que l'analyse sémantique reclasse. Conséquences :
la coloration et les outils sans analyse sémantique ne peuvent pas distinguer les deux ;
`f[T](x)` et `tableau[i](x)` (appel d'une fonction rangée dans un tableau) ont la même
forme. L'ADR 0004 avait choisi les crochets pour libérer `<` et `>` : ce gain est réel et
reste à préserver.

### 4.6 Le point sert à tout

Le même `.` sépare : module et nom (`math.sqrt`), type et fonction associée
(`Point.origine()`), énumération et variante (`Forme.Cercle(r)`), valeur et champ, valeur
et méthode, tuple et position (`.0`), et il prolonge une instruction sur la ligne
suivante (règle 3 du §1.7). Pas d'ambiguïté pour le compilateur, qui résout les noms ;
mais le lecteur ne sait pas, sans connaître les déclarations, si `a.b(c)` est un appel de
module, de fonction associée ou de méthode (2 043 cas).

### 4.7 Le corps d'une fonction n'a pas de valeur, ses blocs si

`if` et `match` sont des expressions, mais le corps d'une fonction exige `return` (E0331).
D'où 1 250 `return valeur`, et la forme `return match … { … }` (exemples, bibliothèque).
La règle est défendable (sortie toujours visible), mais l'asymétrie se remarque entre un
bloc de `if` et un bloc de fonction.

### 4.8 Le mot `fn` et la flèche `=>` ont chacun deux rôles

- `fn` déclare une fonction (1 143), écrit un type de fonction (`fn(i32) -> i32`, 26) et
  une fonction anonyme (`fn(x) => x + 1` ou `fn(x) { … }`, 31).
- `=>` sépare le motif et le corps d'un bras de `match` (338) et le paramètre et le corps
  d'une fonction anonyme courte (24). Dans un bras dont le corps est une fonction
  anonyme, les deux se suivent.

### 4.9 Autres points relevés

- **`while true`** (37 emplois) faute de boucle infinie : `loop` est réservé, pas
  implémenté.
- **Deux places pour les bornes génériques** : en ligne `[T: Ord]` et `where` (32 clauses,
  toutes dans `lib/`), et une troisième pour les `impl` : `impl Display for Paire[A: Display]`.
- **`as` a deux sens** : conversion (569) et renommage à l'import (4).
- **Les constructions modifiantes sont rares et dispersées** (`inout self` 74, `match &`,
  `for … in &`) : la mutation n'a pas une écriture unique.
- **`var` sans valeur initiale** (4 emplois : 3 dans des tests d'erreur, 1 dans la
  spécification) : utile pour l'initialisation différée, mais presque inemployée.
- **`module`** (1 emploi) : le fichier fait déjà office de module.
- **`static var`** (39, presque tous dans `lib/`) : l'accès exige `unsafe` ; c'est le seul
  état global mutable.
- **Visibilité** : `pub` seulement (515 emplois), privé au module sinon ; rien entre les
  deux.

### 4.10 Manques pour les chantiers suivants

Ce ne sont pas des défauts de la 0.2, mais la grammaire de G2 doit leur réserver une place
(passation §4 à §7) :

- **concurrence** : tâches, canaux, partage ; rien aujourd'hui, `async` et `await` réservés ;
- **temps réel** : marquer une fonction ou un contexte sans allocation ni blocage ;
- **contrats et invariants**, types distincts, sous-types : aucune forme ;
- **visibilité et encapsulation** pour la POO : un seul niveau (`pub`) ;
- **`loop`, `defer`, `try`, `yield`, `class`, `interface`, `macro`, `ref`** : réservés,
  sans emploi défini. G1 devra dire lesquels garder.

## 5. Attributs et macros

**Macros** : NEXUL 0.2 n'en a pas. Il n'y a donc rien à migrer, mais aussi aucun mécanisme
d'extension de la syntaxe sur lequel s'appuyer.

**Attributs** (les 12 noms de la spécification §6.7 ; « std » = bibliothèque standard
uniquement, E0231 ailleurs ; `@inlin`, faute volontaire d'un test, n'est pas compté) :

| Attribut | Emplois | Où | Rôle |
|---|---:|---|---|
| `@cfg("…")` | 85 | `lib/`, tests | élément gardé selon la cible |
| `@lang("…")` | 67 | std | lien avec le compilateur |
| `@export` / `@export("…")` | 66 | `lib/`, exemples, tests web | visible depuis C ou JavaScript |
| `@link("…")` | 51 | `lib/`, exemples | bibliothèque native |
| `@intrinsic` | 20 | std | fonction fournie par le compilateur |
| `@track_caller` | 17 | std | position de l'appelant dans les paniques |
| `@cold` | 6 | std | indication à l'optimiseur |
| `@test` | 5 | exemples, tests | fonction de test |
| `@derive(…)` | 4 | tests, guide | implémentations générées |
| `@index` | 1 | std | opérateur `[]` |
| `@inline`, `@no_builtins` | 0 | — | indications à l'optimiseur |

Un changement de la forme `@…` ne touche que 323 jetons, mais il touche les attributs
« std », dont le compilateur dépend (`@lang`, `@intrinsic`, `@index`).

## 6. Impact d'un changement de syntaxe

| Composant | Ce qui dépend de la syntaxe | Propriétaire | Coût |
|---|---|---|---|
| Lexer et parser (`compiler/nxc_syntax`) | 934 et 1 768 lignes ; AST de 486 lignes | Core | réécriture partielle, AST en grande partie réutilisable |
| Messages du compilateur (`nxc_sema`, `nxc explain`) | une quarantaine d'aides citent du code (`` `&x` ``, `` `inout self` ``…) ; textes d'explication (`codes.rs`) | Core | à revoir mot à mot |
| Bibliothèque standard (`lib/`) | 9 499 lignes, 73 % des jetons | Core | **migration automatique indispensable** |
| Tests d'exécution (`tests/run/`) | 36 sources, 27 sorties attendues (`.out`) | Core | sources migrées ; sorties inchangées si la sémantique ne change pas |
| Tests de diagnostics (`tests/ui/`) | 62 sources et 62 `.stderr` qui **recopient les lignes et colonnes** du source | Core | sources migrées, `.stderr` régénérés et relus un par un |
| Formateur (`nxc fmt`, `nxc ide format`) | `format_source` travaille **ligne par ligne** sur l'indentation, pas sur l'arbre | Core | à réécrire sur l'AST (P6), quelle que soit la syntaxe retenue |
| Services d'éditeur (`nxc ide`) | complétion des mots-clés, survol, symboles | Core | tables à mettre à jour |
| Coloration | **5 listes de mots-clés** recopiées : lexer de `nxc`, `studio/syntax` (Luna), `studio/app/src/highlight.rs` (Main), `studio/ui/langs.js` (ancien Studio, 2 listes) ; plus la future extension VS Code (Luna) | Core, Luna, Main | voir recommandation 4 (§7) |
| Débogueur | les noms de types de DWARF reprennent l'écriture NEXUL (`Vec[i32]`) ; l'unité est déclarée `DW_LANG_C99`, donc LLDB évalue les expressions en C ; points d'arrêt par ligne non touchés | Core (DWARF), Luna (`studio/dap`) | faible, sauf si l'écriture des génériques change |
| Modèles de projet (`studio/templates`) | 5 petits fichiers (3 modèles) et l'exemple web | Luna | faible |
| Documentation | `docs/guide.md` (14 blocs), spécification (22 blocs), `README.md` (1 bloc) | Core | réécriture avec la spécification G2 |

Lecture du tableau :

- La migration de `lib/` et des tests est **mécanique si l'outil part de l'AST**. Le
  parser actuel garde la position exacte de chaque nœud (spécification §2.7) : un
  `nxc migrate` peut réécrire les sources par remplacements localisés, en gardant les
  commentaires et la mise en page. Cela suppose de **figer la grammaire 0.2** dans le
  compilateur pendant la transition (G4).
- Les `.stderr` de `tests/ui/` sont le coût le plus fin : chaque position change avec le
  texte. Ils se régénèrent, mais chaque diagnostic doit être relu.
- Le débogueur n'est touché que si l'écriture des types change, par exemple les crochets
  des génériques.

## 7. Recommandations pour G1

Ce sont des critères et des contraintes de méthode, pas des choix de syntaxe.

1. **Corriger les deux pièges vérifiés** (§4.1 et §4.2) dans chacune des propositions,
   par la grammaire ou par un diagnostic.
2. **Garder les propriétés du §3**, en particulier : pas d'expression commençant par `{`,
   analyse sans information sémantique, marque de modification visible à l'appel.
3. **Mesurer chaque proposition sur le corpus réel**, pas seulement sur les 12 programmes
   de la passation : la migration automatique d'un extrait de `lib/` (par exemple
   `lib/std/vec.nx`) et de `examples/calculatrice.nx` montrera le coût réel. Le
   recensement (`g0_census`) se relance sur le résultat pour comparer les nombres de
   jetons et de symboles.
4. **Une seule source pour le lexique** : le compilateur fournit la liste des mots-clés,
   opérateurs et classes de jetons (par exemple `nxc ide lexicon` en JSON, à concevoir),
   et `studio/syntax`, Studio et l'extension VS Code la testent par conformité, comme le
   fait déjà le test de conformité de `studio/syntax` contre le lexer de `nxc`.
5. **Un formateur sur l'AST** (P6) avant la migration de masse : il rendra le résultat de
   `nxc migrate` uniforme.
6. **Transition** : accepter la 0.2 sous une édition déclarée (`--edition 0.2` ou
   équivalent) jusqu'à la fin de la migration ; aucune suppression silencieuse (passation
   §3.2, G4).
7. **Réserver la place des chantiers suivants** (§4.10) dans la grammaire G2, même si leur
   sémantique vient plus tard, pour ne pas casser la syntaxe une seconde fois.

## Annexe

- [g0-recensement.md](g0-recensement.md) : recensement complet, par famille de sources,
  avec les emplacements des constructions rares.
- Programmes de vérification des §4.1 et §4.2 (essais avec `nxc check`) :

```nexul
// §4.1 : accepté sans message ; deux instructions, `let total = a` puis `-b`
fn main() {
    let a = 5
    let b = 3
    let total = a
        - b
    println("{total}")
}
```

```nexul
// §4.2 : `Vret` est une nouvelle variable (W0007, W0001), le programme compile
enum Couleur {
    Rouge
    Vert
}

fn nom(c: Couleur) -> String {
    return match c {
        Rouge => "rouge"
        Vret => "vert"
    }
}

fn main() {
    println(nom(Couleur.Vert))
}
```
