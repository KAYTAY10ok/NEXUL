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
