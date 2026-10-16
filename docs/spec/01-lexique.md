# 01 — Structure lexicale

## 1.1 Encodage du source

- Un fichier source NEXUL porte l'extension `.nx` et **DOIT** être encodé en UTF-8 valide
  (erreur `E0001` sinon). Une marque d'ordre d'octets (BOM) initiale est ignorée.
- Les fins de ligne `LF` et `CRLF` sont acceptées. Un `CR` isolé est une erreur (`E0002`).
- **Sécurité (« Trojan Source », CVE-2021-42574)** : les caractères de contrôle
  bidirectionnels U+061C, U+200E, U+200F, U+202A–U+202E et U+2066–U+2069 sont **interdits
  partout**, y compris dans les commentaires et les chaînes (`E0003`). Ils restent
  représentables dans une chaîne par l'échappement `\u{...}`.
- Le caractère de tabulation est un espace blanc ; les autres caractères de contrôle
  (U+0000–U+001F sauf TAB, LF, CR, et U+007F) sont interdits hors des chaînes (`E0004`) et
  doivent être échappés dans les chaînes.

## 1.2 Espaces et commentaires

```
// commentaire de ligne
/// commentaire de documentation (attaché à l'élément suivant ; exploité par nxdoc, hors 0.1)
/* commentaire de bloc /* imbriquable */ */
```

Un commentaire de bloc non terminé est une erreur (`E0005`). Les commentaires se comportent
comme un espace blanc ; un commentaire de bloc contenant un saut de ligne compte comme un
saut de ligne pour la règle de fin d'instruction (§1.7).

## 1.3 Identificateurs

```
identifiant = ( lettre | "_" ) { lettre | chiffre | "_" } ;
lettre      = "A".."Z" | "a".."z" ;
chiffre     = "0".."9" ;
```

Les identificateurs sont **ASCII** en 0.1. Ce choix est délibéré : il élimine les attaques
par homoglyphes (`а` cyrillique contre `a` latin) dans le code relu. Les chaînes et les
commentaires acceptent tout Unicode. *(Hors 0.1 : identificateurs Unicode normalisés NFC
avec détection de confusables.)*

`_` seul n'est pas un identificateur : c'est le motif joker.

Conventions (vérifiées par `nxlint`, pas par le compilateur) : `TypesEnCamelCase`,
`fonctions_et_variables`, `CONSTANTES`.

## 1.4 Mots-clés

Mots-clés :

```
and     as      break   const   continue dyn    else    enum    extern  false
fn      for     if      impl    import   in     indirect inout  let     match
module  move    not     or      owned    pub    return  self    Self    static
struct  trait   true    type    unsafe   var    where   while
```

`dyn`, `static`, `trait` et `where` sont utilisés depuis la version 0.2.

Mots-clés **réservés** pour les versions futures (interdits comme identificateurs) :

```
async   await   class   defer   interface  loop   macro   ref   try   yield
```

## 1.5 Littéraux

### Entiers

```
entier      = decimal | "0x" hexa | "0o" octal | "0b" binaire ;
decimal     = chiffre { chiffre | "_" } ;
hexa        = chiffre_hexa { chiffre_hexa | "_" } ;
```

Exemples : `42`, `1_000_000`, `0xFF_FF`, `0o755`, `0b1010_0001`.
Un littéral entier n'a pas de signe : `-5` est l'opérateur `-` appliqué à `5`, mais le
compilateur traite `-` suivi d'un littéral comme une constante négative (ainsi
`-128` est un `i8` valide). Le type d'un littéral est déterminé par le contexte (§03) ;
une valeur qui ne tient pas dans le type est une erreur de compilation (`E0301`).
Pas de suffixe de type : on écrit `let x: u8 = 200` ou `200 as u8`.
Un `_` ne peut ni terminer le littéral ni suivre directement un préfixe (`E0006`).

### Flottants

```
flottant = decimal "." decimal [ exposant ] | decimal exposant ;
exposant = ( "e" | "E" ) [ "+" | "-" ] decimal ;
```

Exemples : `3.14`, `1.0e-9`, `6e23`. Un chiffre est obligatoire de chaque côté du point :
`1.` et `.5` sont invalides, ce qui laisse `0..10` (intervalle) et `t.0` (champ de
tuple) sans ambiguïté.

### Caractères

`'a'`, `'é'`, `'\n'`, `'\u{1F600}'`. Un littéral caractère contient exactement une valeur
scalaire Unicode (U+0000–U+10FFFF hors U+D800–U+DFFF).

### Chaînes

```
"Bonjour, {nom} !"          // chaîne avec interpolation
"accolade littérale : \{"   // échappement de l'accolade
r"C:\chemin\sans\échappement {pas d'interpolation}"   // chaîne brute
```

Échappements (chaînes et caractères) : `\n` `\r` `\t` `\0` `\\` `\'` `\"` `\{` `\}`
et `\u{H...}` (1 à 6 chiffres hexadécimaux, valeur scalaire Unicode valide).
Un échappement inconnu est une erreur (`E0007`). Une chaîne ne peut pas contenir de saut
de ligne brut en 0.1 (`E0008`) ; on utilise `\n`. *(Hors 0.1 : chaînes multilignes `"""`.)*

**Interpolation.** Dans une chaîne non brute, `{` ouvre une expression interpolée fermée
par l'accolade correspondante. Une spécification de format optionnelle suit le premier `:`
de profondeur zéro :

```
"{x}"        "{prix:.2}"     "{n:>8}"     "{octet:02x}"     "{nom:-<20}"
```

Grammaire de la spécification de format :
`[[remplissage] alignement] [signe] ["0"] [largeur] ["." précision] [type]` avec
`alignement ∈ { "<", "^", ">" }`, `signe = "+"`, `type ∈ { "x", "X", "o", "b", "e" }`.
Une expression interpolée peut contenir des chaînes littérales, elles-mêmes interpolées :
`"{nom.replace("-", " ")}"`.

### Booléens

`true` et `false` sont des mots-clés.

## 1.6 Ponctuation et opérateurs

```
+   -   *   /   %   +%  -%  *%  &   |   ^   ~   <<  >>
=   +=  -=  *=  /=  %=  &=  |=  ^=  <<= >>=
==  !=  <   <=  >   >=
(   )   [   ]   {   }   ,   .   ..  ..= :   ;   ->  =>  ?   @
```

Les opérateurs logiques sont les mots-clés `and`, `or` et `not`. Le lexème le plus long
l'emporte (`<<=` est un seul jeton).

## 1.7 Fins d'instruction (sauts de ligne significatifs)

NEXUL n'exige pas de point-virgule. Le lexer transforme un saut de ligne en jeton
`FIN` lorsque **toutes** les conditions suivantes sont réunies :

1. le délimitateur ouvert le plus interne est une accolade `{`, ou aucun délimitateur
   n'est ouvert (les sauts de ligne à l'intérieur de `(...)` et `[...]` sont ignorés) ;
2. le dernier jeton émis peut terminer une instruction : identificateur, littéral,
   `true`, `false`, `self`, `Self`, `return`, `break`, `continue`, `)`, `]`, `}` ou `?` ;
3. la ligne suivante non vide ne commence pas par `.` suivi d'un identificateur ou d'un
   chiffre (chaînage de méthodes sur plusieurs lignes).

Plusieurs sauts de ligne consécutifs produisent au plus un `FIN`. Le point-virgule `;`
est un `FIN` explicite (utile pour plusieurs instructions sur une ligne).

Conséquences, à connaître :

```
let total = a +
    b               // OK : la ligne se termine par `+`, pas de FIN
let total = a
    + b             // deux instructions : `let total = a` puis `+b` (erreur E0102)
let ecart = a
    - b             // deux instructions : `let ecart = a` puis `-b` (erreur E0110)
let r = valeur
    .methode()      // OK : règle 3
```

Une instruction qui calcule une valeur sans effet (opérateurs, littéraux, noms, parenthèses,
tableaux, sans appel ni `?`) et qui n'est pas la dernière de son bloc est refusée (`E0110`) :
c'est presque toujours une ligne coupée en deux. Pour abandonner une valeur exprès :
`let _ = expression`.

Le parseur ignore un `FIN` situé juste avant `else`, ce qui permet le style
`}` / `else {` sur deux lignes.
