# 02 — Grammaire

## 2.1 Notation

EBNF ISO simplifiée : `"x"` terminal, `A B` séquence, `A | B` alternative, `[ A ]`
optionnel, `{ A }` répétition (zéro ou plus), `( ... )` groupement.
`FIN` désigne une fin d'instruction (saut de ligne significatif ou `;`, voir §1.7).
`Ident`, `Entier`, `Flottant`, `Caractere`, `Chaine` sont les jetons du chapitre 01.

La grammaire est **LL(1) à une exception près** documentée (§2.6) et ne nécessite aucune
information sémantique pour être analysée. Une accolade `{` ne commence **jamais** une
expression : un bloc n'apparaît qu'après un mot-clé (`if`, `else`, `while`, `for`,
`match`, `unsafe`, `fn`) ou une flèche `=>`. Il n'existe donc aucune ambiguïté entre
bloc et littéral.

## 2.2 Fichiers et éléments

```ebnf
Fichier          = { FIN } [ DeclModule ] { Import } { Element } ;
DeclModule       = "module" Chemin FIN ;
Import           = "import" Chemin [ "as" Ident | "." "{" ImportNom { "," ImportNom } [ "," ] "}" ] FIN ;
ImportNom        = Ident [ "as" Ident ] ;
Chemin           = Ident { "." Ident } ;

Element          = { Attribut } [ "pub" ] ( Fonction | Struct | Enum | Impl | Trait | Const
                                          | Static | AliasType | FonctionExterne ) FIN ;
Attribut         = "@" Ident [ "(" [ Litteral { "," Litteral } [ "," ] ] ")" ] { FIN } ;

Fonction         = [ "unsafe" ] "fn" Ident [ ParamsGeneriques ]
                   "(" [ Parametre { "," Parametre } [ "," ] ] ")" [ "->" Type ] [ Where ] [ Bloc ] ;
ParamsGeneriques = "[" ParamGenerique { "," ParamGenerique } [ "," ] "]" ;
ParamGenerique   = Ident [ ":" Contraintes ] ;
Contraintes      = CheminType { "+" CheminType } ;
Where            = "where" Ident ":" Contraintes { "," Ident ":" Contraintes } ;
Parametre        = [ Convention ] "self" | Ident ":" [ Convention ] Type ;
Convention       = "inout" | "owned" ;

Struct           = "struct" Ident [ ParamsGeneriques ] "{" { FIN } { Champ Sep } "}" ;
Champ            = [ "pub" ] [ "indirect" ] Ident ":" Type ;
Enum             = "enum" Ident [ ParamsGeneriques ] "{" { FIN } { Variante Sep } "}" ;
Variante         = [ "indirect" ] Ident [ "(" Type { "," Type } [ "," ] ")" ] ;
Sep              = ( "," | FIN ) { FIN } ;

Impl             = "impl" [ CheminType "for" ] TypeImpl "{" { FIN } { { Attribut } [ "pub" ] Fonction FIN } "}" ;
TypeImpl         = CheminType [ "[" ParamGenerique { "," ParamGenerique } [ "," ] "]" ] | Type ;
Trait            = "trait" Ident [ ":" Contraintes ] "{" { FIN } { { Attribut } Fonction FIN } "}" ;
Const            = "const" Ident ":" Type "=" Expr ;
Static           = "static" [ "var" ] Ident ":" Type "=" Expr ;
AliasType        = "type" Ident "=" Type ;
FonctionExterne  = "extern" Chaine "fn" Ident
                   "(" [ Ident ":" Type { "," Ident ":" Type } [ "," ] ] ")" [ "->" Type ] ;
```

Une `Fonction` sans corps n'est permise que dans un `trait` (méthode requise) et pour les
fonctions marquées `@intrinsic` (bibliothèque standard uniquement) ; sinon erreur `E0103`.
Dans `impl Trait for Vec[T: Clone]`, les paramètres du type portent les contraintes sous
lesquelles l'implémentation s'applique.

## 2.3 Types

```ebnf
Type        = CheminType [ "[" Type { "," Type } [ "," ] "]" ]      (* i32, Vec[i32], geo.Point *)
            | "[" Type ";" Expr "]"                                  (* tableau de taille fixe *)
            | "(" ")"                                                (* unité *)
            | "(" Type "," [ Type { "," Type } [ "," ] ] ")"         (* tuple *)
            | "fn" "(" [ [ Convention ] Type { "," [ Convention ] Type } [ "," ] ] ")" [ "->" Type ]
            | "dyn" CheminType ;                                     (* répartition dynamique *)
CheminType  = Ident { "." Ident } | "Self" ;
```

Les paramètres génériques s'écrivent entre **crochets** : `Vec[i32]`, `Result[T, E]`.
Ce choix (comme Go, Scala et Mojo) supprime l'ambiguïté classique de `a < b > (c)` : les
symboles `<` et `>` sont exclusivement des comparaisons.

## 2.4 Blocs et instructions

```ebnf
Bloc         = "{" { FIN } { Instruction ( FIN { FIN } | (* avant "}" *) ) } "}" ;
Instruction  = Let | Var | While | WhileLet | For | Bloc
             | Expr [ OpAffect Expr ] ;
Let          = "let" Motif [ ":" Type ] "=" Expr ;
Var          = "var" Ident [ ":" Type ] [ "=" Expr ] ;
OpAffect     = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "+%=" | "-%=" | "*%="
             | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
While        = [ Ident ":" ] "while" Expr Bloc ;
WhileLet     = [ Ident ":" ] "while" "let" Motif "=" [ "&" ] Expr Bloc ;
For          = [ Ident ":" ] "for" Motif "in" [ "&" ] Expr [ ( ".." | "..=" ) Expr ] Bloc ;
```

L'affectation est une **instruction**, jamais une expression : `if x = 3` est une erreur de
syntaxe, et `a = b = c` est impossible.

Un `Bloc` en position d'instruction ouvre une portée : les valeurs qui y sont déclarées sont
détruites à sa fin. `return`, `break` et `continue` sont des expressions de type `Never`,
utilisables comme corps de bras : `None => return 0`.

## 2.5 Expressions

Priorités, de la plus faible à la plus forte :

| Niveau | Opérateurs | Associativité |
|---|---|---|
| 1 | `or` | gauche |
| 2 | `and` | gauche |
| 3 | `==` `!=` `<` `<=` `>` `>=` | **aucune** (`a < b < c` est une erreur E0104) |
| 4 | `\|` | gauche |
| 5 | `^` | gauche |
| 6 | `&` | gauche |
| 7 | `<<` `>>` | gauche |
| 8 | `+` `-` `+%` `-%` | gauche |
| 9 | `*` `/` `%` `*%` | gauche |
| 10 | `as` | gauche |
| 11 | préfixes `-` `not` `~` `move` | droite |
| 12 | suffixes `.champ` `.0` `(...)` `[...]` `?` | gauche |

Contrairement à C, les opérateurs bit à bit lient **plus fort** que les comparaisons :
`a & masque == 0` signifie `(a & masque) == 0`.

```ebnf
Expr          = ExprBinaire ;                         (* selon le tableau ci-dessus *)
ExprPrefixe   = ( "-" | "not" | "~" | "move" ) ExprPrefixe | ExprSuffixe ;
ExprSuffixe   = Primaire { "." Ident | "." Entier | "(" [ Arguments ] ")"
                         | "[" Expr { "," Expr } [ "," ] "]" | "?" } ;
Arguments     = Argument { "," Argument } [ "," ] ;
Argument      = [ Ident ":" ] [ "&" ] Expr ;
Primaire      = Litteral | Ident | "self" | "Self"
              | "(" ")" | "(" Expr ")" | "(" Expr "," [ Expr { "," Expr } [ "," ] ] ")"
              | "[" [ Expr { "," Expr } [ "," ] ] "]" | "[" Expr ";" Expr "]"
              | Si | Match | "unsafe" Bloc | FonctionAnonyme
              | "return" [ Expr ] | "break" [ Ident ] | "continue" [ Ident ] ;
FonctionAnonyme = "fn" "(" [ ParamAnonyme { "," ParamAnonyme } [ "," ] ] ")"
                  ( "=>" Expr | [ "->" Type ] Bloc ) ;
ParamAnonyme  = [ Convention ] Ident [ ":" Type ] ;
Litteral      = Entier | Flottant | Caractere | Chaine | "true" | "false" ;
Si            = "if" ( Expr | "let" Motif "=" [ "&" ] Expr ) Bloc
                [ { FIN } "else" ( Si | Bloc ) ] ;
Match         = "match" [ "&" ] Expr "{" { FIN } { Bras Sep } "}" ;
Bras          = Motif [ "if" Expr ] "=>" ( Bloc | Expr [ OpAffect Expr ] ) ;
```

- `&` devant le scrutin d'un `match`, d'un `if let` ou devant la collection d'un `for`
  donne aux liaisons un accès **modifiable** à la valeur (§5.6).
- Le corps d'un bras peut être une affectation unique : `Some(n) => total += n`.
- `Argument` : une étiquette `nom:` n'est permise que pour construire une structure
  (`Point(x: 1.0, y: 2.0)`). Le préfixe `&` marque un argument `inout` (§05).
- `"[" Expr { "," Expr } "]"` après une expression est soit une indexation (un seul
  argument), soit une instanciation générique explicite (`Vec[i32].new()`,
  `size_of[u64]()`), selon ce que désigne l'expression de base (§03).

## 2.6 Motifs

```ebnf
Motif         = MotifSimple { "|" MotifSimple } ;
MotifSimple   = "_"
              | Ident                                         (* liaison, ou variante/constante *)
              | MotifLitt [ ( ".." | "..=" ) MotifLitt ]
              | Chemin "(" [ ArgMotif { "," ArgMotif } [ "," ] ] ")"
              | "(" Motif "," [ Motif { "," Motif } [ "," ] ] ")" ;
ArgMotif      = Ident ":" Motif | Motif | ".." ;
MotifLitt     = [ "-" ] Entier | Caractere | Chaine | "true" | "false" ;
```

Exception LL(1) : dans `ArgMotif`, `Ident ":"` (champ de structure nommé) se distingue de
`Motif` par le second jeton.

Un identificateur seul dans un motif désigne, **dans cet ordre** :
1. une variante sans données de l'énumération attendue à cet endroit (résolution
   dirigée par le type : dans `match forme { Vide => ... }`, `Vide` est cherché parmi les
   variantes du type de `forme`) ;
2. une variante ou une constante visible dans la portée (`None`, `MAX`) ;
3. sinon, une **nouvelle liaison**.

De même `Cercle(r)` est cherché d'abord parmi les variantes du type attendu ; la forme
qualifiée `Forme.Cercle(r)` est toujours acceptée.

## 2.7 Grammaire et outils

L'analyseur officiel est un analyseur récursif descendant écrit à la main, avec
reprise sur erreur au niveau des instructions et des éléments : une seule erreur de
syntaxe ne doit jamais masquer les erreurs indépendantes qui suivent. L'arbre syntaxique
conserve la position exacte (fichier, octet de début et de fin) de chaque nœud.

## 2.8 Édition 1 (grammaire figée, ADR 0019)

Les sections 2.2 à 2.6 décrivent l'édition 0.2. La grammaire ci-dessous est celle de
l'**édition 1**, telle que l'implémente `compiler/nxc_syntax/src/parser/ed1.rs`. Depuis la
0.7.0, elle est **figée** (ADR 0019) : elle ne change que par une ADR acceptée, en
ajoutant une forme. Le parser auto-hébergé (SH3) doit donner le même arbre pour tout ce
qu'elle accepte, et refuser le reste.

Notation : celle de §2.1. `FIN` est la fin d'instruction (§1.7) ; `nom` est un `Ident`
qui n'est pas un mot réservé de l'édition 1. Les mots `when then is interface extend any
distinct it repeat test expects requires ensures with every within spawn loop` sont des
identificateurs pour le lexer et des mots-clés pour le parser (E0010 comme nom).

### Fichier et éléments

```ebnf
fichier        = { FIN } [ "module" chemin FIN ] { import FIN } { element FIN } ;
import         = "import" chemin [ "." "{" nom_importe { "," nom_importe } "}" | "as" nom ] ;
nom_importe    = nom [ "as" nom ] ;
chemin         = nom { "." nom } ;
attribut       = "@" nom [ "(" [ litteral_attr { "," litteral_attr } ] ")" ] { FIN } ;
litteral_attr  = Chaine | Entier | Ident | "true" | "false" ;

element        = { attribut } [ "pub" ]
                 ( fonction | type_decl | interface | extension | "unsafe" extension
                 | test | constante | statique | externe ) ;
fonction       = [ "unsafe" ] "fn" [ type ] nom [ generiques ] "(" [ parametres ] ")"
                 { clause } [ corps ] ;
                 (* « un type puis un nom » : le type rendu ; un nom seul : rien n'est rendu *)
parametres     = parametre { "," parametre } [ "," ] ;
parametre      = [ "owned" ] "self" [ "!" ] | [ "owned" ] type nom [ "!" ] ;
generiques     = "[" generique { "," generique } [ "," ] "]" ;
generique      = nom [ "is" type { "and" type } ] ;
clause         = "with" nom { "," nom }                        (* noalloc, nonblocking *)
               | "every" expression [ "within" expression ] ;
corps          = bloc | "=" expression ;
test           = "test" Chaine [ "expects" "panic" ] bloc ;    (* chaîne sans interpolation *)
constante      = "const" type nom "=" expression ;
statique       = "static" [ "var" ] type nom "=" expression ;
externe        = "extern" Chaine "fn" [ type ] nom "(" [ type nom { "," type nom } ] ")"
                 [ "with" nom { "," nom } ] ;
```

### Types déclarés

```ebnf
type_decl      = "type" nom [ generiques ]
                 ( [ "is" type { "and" type } ] corps_type
                 | "is" type { "and" type } "=" variantes
                 | "=" ( variantes | [ "distinct" ] type [ "is" type { "and" type } ]
                                     [ "where" condition ] ) ) ;
corps_type     = "{" { FIN } { membre ( FIN | "," ) { FIN } } "}" ;
membre         = { attribut } [ "pub" ] ( fonction | [ "indirect" ] type nom ) ;
variantes      = [ "|" ] variante { "|" variante } ;
variante       = [ "indirect" ] "." nom [ "(" [ type [ nom ] { "," type [ nom ] } ] ")" ] ;
condition      = cond_et { "or" cond_et } ;
cond_et        = cond_non { "and" cond_non } ;
cond_non       = "not" cond_non | "(" condition ")"
               | "it" "in" unaire ( ".." | "..=" ) unaire
               | "it" comparateur unaire | unaire comparateur "it" ;
comparateur    = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
interface      = "interface" nom [ "is" type { "and" type } ] corps_fonctions ;
extension      = "extend" ( chemin [ generiques ] | type ) [ "is" type ] corps_fonctions ;
corps_fonctions= "{" { FIN } { { attribut } [ "pub" ] fonction FIN { FIN } } "}" ;
```

Une fonction d'un `corps_type` se distingue d'un champ de type fonction par ce qui suit le
type : `fn i32(i32) f` est un champ, `fn i32 f(…)` une fonction.

### Types

```ebnf
type           = chemin [ "[" type { "," type } "]" ] | "Self"
               | "any" type                                     (* objet d'interface *)
               | "(" [ type { "," type } ] ")"                  (* () et n-uplets *)
               | "[" type ";" expression "]"                    (* tableau de taille fixe *)
               | "fn" [ type ] "(" [ type_param { "," type_param } ] ")" ;
type_param     = [ "owned" ] type [ "!" ] ;
```

### Blocs et instructions

```ebnf
bloc           = "{" { FIN } { instruction ( FIN | "}" ) { FIN } } "}" ;
instruction    = "let" [ type ] motif "=" expression [ "else" repli ]
               | "var" [ type ] nom [ "=" expression ]
               | [ "#" nom ] boucle
               | bloc                                           (* portée *)
               | expression affectation expression
               | expression [ "else" repli ] ;
affectation    = "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" ;
repli          = bloc | expression ;
boucle         = "for" [ type ] motif [ "!" ] "in" expression [ ( ".." | "..=" ) expression ] bloc
               | "while" expression bloc
               | "loop" bloc
               | "repeat" expression bloc ;
```

### Expressions

```ebnf
expression     = binaire { "|>" postfixe } ;
binaire        = (* opérateurs binaires par précédence croissante, ci-dessous *) ;
conversion     = unaire { "as" type } ;
unaire         = ( "-" | "not" | "~" | "move" ) unaire | postfixe ;
postfixe       = primaire { "." ( nom | Entier ) | "(" [ arguments ] ")"
                          | "[" expression { "," expression } "]" | "?" | "!" } ;
arguments      = argument { "," argument } [ "," ] ;
argument       = [ nom ":" ] expression ;                       (* `x!` : valeur modifiée *)
primaire       = litteral | Chaine | nom | "self" | "Self" | "." nom
               | "(" [ expression { "," expression } [ "," ] ] ")"
               | "[" [ expression ( ";" expression | { "," expression } ) ] "]"
               | si | quand | anonyme | "unsafe" bloc
               | "return" [ expression ] | ( "break" | "continue" ) [ "#" nom ] ;
si             = "if" expression bloc [ "else" ( si | bloc ) ] ;
quand          = "when" expression [ "!" ] "{" { FIN } { bras FIN { FIN } } "}" ;
bras           = "else" valeur | motif { "," motif } [ "if" expression ] "then" valeur ;
valeur         = bloc | expression [ affectation expression ] ;
anonyme        = "fn" [ type ] "(" [ param_a { "," param_a } ] ")" ( "=>" expression | bloc ) ;
param_a        = [ "owned" ] [ type ] nom [ "!" ] ;
```

Précédence de `binaire`, de la plus faible à la plus forte (toutes associatives à gauche,
sauf les comparaisons, non associatives : E0104) :

| Niveau | Opérateurs |
|---|---|
| 1 | `or` |
| 2 | `and` |
| 3 | `==` `!=` `<` `<=` `>` `>=` ; `e is motif` ; `e in a..b`, `e in a..=b`, `e in c` (bornes et `c` au niveau 4) |
| 4 | `\|` |
| 5 | `^` |
| 6 | `&` |
| 7 | `<<` `>>` |
| 8 | `+` `-` `+%` `-%` |
| 9 | `*` `/` `%` `*%` |
| 10 | `as` (`conversion`) |

`|>` est plus faible que tout : `a |> f(b)` est `f(a, b)`, `a |> f` est `f(a)`. `..` et
`..=` n'existent que dans `for`, après `in` et dans les motifs. `if e is P` et `while e is P`
lient les noms de `P` dans le bloc ; `e! is P` les lie à `e` lui-même.

### Motifs

```ebnf
motif          = motif_simple { "|" motif_simple } ;
motif_simple   = "_" | litteral_motif [ ( ".." | "..=" ) litteral_motif ]
               | nom [ "!" ]                                     (* liaison *)
               | "." nom [ "(" args_motif ")" ]                  (* variante du type examiné *)
               | chemin [ "(" args_motif ")" ]                   (* variante ou constante qualifiée *)
               | "(" [ motif { "," motif } [ "," ] ] ")" ;
args_motif     = [ arg_motif { "," arg_motif } [ "," ] ] ;
arg_motif      = nom ":" motif | motif | ".." ;
litteral_motif = [ "-" ] Entier | Caractere | Chaine | "true" | "false" ;
```

### Reconnaissance d'un type

Après `fn`, `let`, `var`, `for`, dans un paramètre, un champ ou un paramètre d'une fonction
anonyme, le parser lit un type **seulement si** les jetons qui suivent forment un type
puis un `nom` (autre que `is` et `then`). C'est un retour arrière borné par les crochets
et parenthèses, sans information sémantique. Un `else` qui suit le bloc d'un `if`, même à
la ligne, appartient à ce `if`.

### Formes réservées

Lues puis refusées (E0111), jamais acceptées sans être vérifiées : `requires` et
`ensures` ; un effet autre que `noalloc` et `nonblocking` ; `===` et `!==` ; `spawn` ;
`it` hors d'une condition `where` ; une borne sur le paramètre d'un `type` ; un type
`distinct` ou contraint générique ; un type sur la variable d'un `for` qui modifie ou
déstructure.

Retirées de l'édition 0.2, refusées avec la nouvelle écriture (E0112) : `struct`, `enum`,
`trait`, `impl`, `dyn`, `match`, `&x`, `->`, `name: Type`, `[T: I]`, `if let`, `while let`,
`=>` dans `when`, `label: for`.
