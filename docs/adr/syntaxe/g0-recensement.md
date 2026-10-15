# G0 — Recensement de la syntaxe de NEXUL 0.2

Annexe de [g0-audit.md](g0-audit.md). Fichier **généré** par :

```
cargo run -p nxc_syntax --example g0_census -- .
```

sur `main` du 15/10/2026 (`edb886c`). Ne pas modifier à la main : relancer la commande.

Colonnes : `lib` (bibliothèque standard), `examples`, `tests/run`, `tests/ui`,
`tests/autres` (`tests/numerics`, `tests/web`), `docs` (blocs `nexul` des fichiers
Markdown, hors `docs/refonte/` et `docs/adr/syntaxe/`). Les lignes « jeton » comptent les
jetons du lexer ; les autres, les nœuds de l'arbre syntaxique.

| | lib | examples | tests/run | tests/ui | tests/autres | docs | total |
|---|---|---|---|---|---|---|---|
| fichiers ou blocs | 21 | 8 | 36 | 63 | 5 | 37 | 170 |
| lignes | 9499 | 486 | 1824 | 865 | 206 | 554 | 13434 |
| lignes de code | 7912 | 414 | 1593 | 770 | 177 | 488 | 11354 |
| jetons | 58327 | 2800 | 10432 | 4246 | 1316 | 3225 | 80346 |
| avec erreur de syntaxe | 0 | 0 | 0 | 7 | 0 | 5 | 12 |
| accès: .0 (tuple) | 0 | 0 | 4 | 0 | 0 | 0 | 4 |
| accès: .champ ou module.nom | 1882 | 132 | 635 | 69 | 96 | 108 | 2922 |
| affectation: &= | 4 | 0 | 0 | 0 | 0 | 0 | 4 |
| affectation: *= | 13 | 2 | 2 | 0 | 0 | 0 | 17 |
| affectation: += | 123 | 19 | 50 | 5 | 2 | 13 | 212 |
| affectation: -= | 46 | 1 | 1 | 0 | 0 | 0 | 48 |
| affectation: /= | 3 | 0 | 0 | 0 | 0 | 0 | 3 |
| affectation: <<= | 20 | 0 | 0 | 0 | 0 | 0 | 20 |
| affectation: = | 342 | 11 | 26 | 14 | 3 | 5 | 401 |
| affectation: >>= | 2 | 0 | 0 | 0 | 0 | 0 | 2 |
| affectation: ^= | 5 | 0 | 0 | 0 | 0 | 0 | 5 |
| affectation: \|= | 10 | 0 | 0 | 0 | 0 | 0 | 10 |
| appel: argument &x (inout) | 96 | 11 | 17 | 9 | 2 | 0 | 135 |
| appel: argument nommé (nom: valeur) | 74 | 11 | 66 | 21 | 0 | 24 | 196 |
| appel: f(…) | 881 | 89 | 452 | 130 | 51 | 96 | 1699 |
| appel: x.f(…) (méthode ou module) | 1249 | 79 | 487 | 50 | 93 | 85 | 2043 |
| attribut: @cfg | 0 | 0 | 0 | 1 | 0 | 0 | 1 |
| attribut: @cfg(…) | 73 | 0 | 9 | 2 | 0 | 0 | 84 |
| attribut: @cold | 6 | 0 | 0 | 0 | 0 | 0 | 6 |
| attribut: @derive(…) | 0 | 0 | 3 | 0 | 0 | 1 | 4 |
| attribut: @export | 0 | 0 | 0 | 0 | 0 | 1 | 1 |
| attribut: @export(…) | 51 | 3 | 0 | 0 | 10 | 1 | 65 |
| attribut: @index | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| attribut: @inlin | 0 | 0 | 0 | 1 | 0 | 0 | 1 |
| attribut: @intrinsic | 19 | 0 | 0 | 1 | 0 | 0 | 20 |
| attribut: @lang(…) | 67 | 0 | 0 | 0 | 0 | 0 | 67 |
| attribut: @link(…) | 47 | 2 | 0 | 1 | 0 | 1 | 51 |
| attribut: @test | 0 | 3 | 0 | 1 | 0 | 1 | 5 |
| attribut: @track_caller | 17 | 0 | 0 | 0 | 0 | 0 | 17 |
| bloc: unsafe { … } | 94 | 6 | 7 | 2 | 0 | 2 | 111 |
| boucle: for avec motif déstructurant | 0 | 0 | 1 | 1 | 0 | 0 | 2 |
| boucle: for x in &v (modification en place) | 0 | 1 | 1 | 2 | 0 | 1 | 5 |
| boucle: for x in a..=b | 0 | 2 | 4 | 0 | 0 | 2 | 8 |
| boucle: for x in a..b | 55 | 3 | 19 | 2 | 3 | 2 | 84 |
| boucle: for x in v | 31 | 5 | 29 | 5 | 0 | 12 | 82 |
| boucle: étiquette (nom: while/for) | 0 | 0 | 1 | 3 | 0 | 0 | 4 |
| contrôle: break | 35 | 0 | 5 | 1 | 1 | 2 | 44 |
| contrôle: break étiquette | 0 | 0 | 1 | 1 | 0 | 0 | 2 |
| contrôle: continue | 11 | 2 | 3 | 1 | 1 | 0 | 18 |
| contrôle: continue étiquette | 0 | 0 | 1 | 0 | 0 | 0 | 1 |
| contrôle: else | 189 | 3 | 8 | 2 | 3 | 4 | 209 |
| contrôle: else if | 36 | 3 | 4 | 0 | 0 | 0 | 43 |
| contrôle: if | 859 | 22 | 44 | 9 | 19 | 8 | 961 |
| contrôle: if let / while let | 12 | 1 | 5 | 0 | 0 | 3 | 21 |
| contrôle: if let P = &e / while let (modification) | 1 | 1 | 1 | 2 | 0 | 0 | 5 |
| contrôle: match | 45 | 9 | 37 | 18 | 3 | 6 | 118 |
| contrôle: match &e (modification) | 0 | 1 | 1 | 1 | 0 | 2 | 5 |
| contrôle: return | 27 | 0 | 0 | 2 | 0 | 1 | 30 |
| contrôle: return valeur | 1034 | 31 | 103 | 28 | 25 | 29 | 1250 |
| contrôle: while (source) | 95 | 8 | 10 | 4 | 1 | 2 | 120 |
| contrôle: while let (source) | 0 | 0 | 1 | 0 | 0 | 1 | 2 |
| crochets: a[i] ou instanciation générique F[T] | 224 | 17 | 67 | 10 | 2 | 5 | 325 |
| crochets: plusieurs arguments a[x, y] | 0 | 0 | 2 | 2 | 0 | 0 | 4 |
| enum: variante | 14 | 11 | 27 | 11 | 0 | 11 | 74 |
| enum: variante avec données | 3 | 4 | 10 | 3 | 0 | 9 | 29 |
| enum: variante indirect | 0 | 2 | 3 | 0 | 0 | 1 | 6 |
| expression: () | 3 | 0 | 0 | 0 | 0 | 0 | 3 |
| expression: tableau [a, b] | 11 | 3 | 13 | 1 | 1 | 3 | 32 |
| expression: tableau répété [x; n] | 20 | 0 | 6 | 0 | 0 | 0 | 26 |
| expression: tuple (a, b) | 29 | 1 | 8 | 2 | 0 | 0 | 40 |
| fin d'instruction: ; entre deux instructions | 55 | 0 | 12 | 1 | 2 | 0 | 70 |
| fonction anonyme: fn(x) => expr | 0 | 0 | 14 | 3 | 0 | 7 | 24 |
| fonction anonyme: fn(x) { … } | 0 | 0 | 4 | 0 | 0 | 3 | 7 |
| fonction anonyme: paramètre inout/owned | 0 | 0 | 2 | 0 | 0 | 1 | 3 |
| fonction anonyme: paramètre typé | 0 | 0 | 1 | 1 | 0 | 0 | 2 |
| fonction anonyme: type de retour | 0 | 0 | 1 | 0 | 0 | 0 | 1 |
| fonction: dans un trait | 12 | 0 | 3 | 2 | 0 | 6 | 23 |
| fonction: libre | 388 | 27 | 93 | 103 | 22 | 45 | 678 |
| fonction: méthode ou fonction associée (impl) | 374 | 6 | 34 | 13 | 0 | 15 | 442 |
| fonction: sans corps | 31 | 0 | 2 | 3 | 0 | 4 | 40 |
| fonction: type de retour (->) | 678 | 16 | 81 | 31 | 14 | 32 | 852 |
| fonction: unsafe fn | 98 | 2 | 0 | 1 | 1 | 1 | 103 |
| générique: clause where (source) | 32 | 0 | 0 | 0 | 0 | 0 | 32 |
| générique: enum paramétrée | 2 | 0 | 0 | 0 | 0 | 2 | 4 |
| générique: fonction avec bornes (inline ou where) | 33 | 0 | 4 | 1 | 0 | 2 | 40 |
| générique: fonction paramétrée | 16 | 0 | 9 | 4 | 0 | 3 | 32 |
| générique: impl avec bornes sur les paramètres du type | 17 | 0 | 0 | 0 | 0 | 0 | 17 |
| générique: struct paramétrée | 3 | 0 | 2 | 0 | 0 | 1 | 6 |
| import: liste .{a, b} | 0 | 0 | 3 | 2 | 0 | 3 | 8 |
| import: module | 45 | 2 | 8 | 6 | 9 | 1 | 71 |
| import: module as alias | 0 | 0 | 1 | 0 | 0 | 1 | 2 |
| import: élément renommé (x as y) | 0 | 0 | 1 | 0 | 0 | 1 | 2 |
| instruction: expression | 2560 | 108 | 506 | 129 | 63 | 116 | 3482 |
| instruction: let | 739 | 26 | 148 | 135 | 20 | 38 | 1106 |
| instruction: let avec motif déstructurant | 26 | 0 | 2 | 2 | 0 | 1 | 31 |
| instruction: let avec type | 7 | 0 | 33 | 24 | 0 | 1 | 65 |
| instruction: var | 420 | 24 | 89 | 17 | 7 | 27 | 584 |
| instruction: var avec type | 173 | 12 | 41 | 12 | 1 | 15 | 254 |
| instruction: var sans valeur initiale | 0 | 0 | 0 | 3 | 0 | 1 | 4 |
| interpolation: avec format {x:…} | 11 | 4 | 37 | 4 | 4 | 4 | 64 |
| interpolation: expression | 57 | 25 | 485 | 56 | 73 | 59 | 755 |
| interpolation: expression complexe (pas un simple nom) | 19 | 13 | 355 | 15 | 60 | 33 | 495 |
| jeton: ; | 55 | 0 | 12 | 1 | 2 | 0 | 70 |
| jeton: Amp | 219 | 14 | 20 | 15 | 10 | 3 | 281 |
| jeton: AmpEq | 4 | 0 | 0 | 0 | 0 | 0 | 4 |
| jeton: And | 108 | 1 | 3 | 0 | 1 | 0 | 113 |
| jeton: Arrow | 734 | 17 | 90 | 36 | 14 | 36 | 927 |
| jeton: As | 509 | 3 | 10 | 6 | 11 | 3 | 542 |
| jeton: At | 281 | 8 | 12 | 7 | 10 | 5 | 323 |
| jeton: Break | 35 | 0 | 5 | 2 | 1 | 2 | 45 |
| jeton: Caret | 13 | 0 | 0 | 0 | 0 | 0 | 13 |
| jeton: CaretEq | 5 | 0 | 0 | 0 | 0 | 0 | 5 |
| jeton: Colon | 1414 | 62 | 264 | 137 | 15 | 97 | 1989 |
| jeton: Comma | 1410 | 84 | 214 | 38 | 28 | 69 | 1843 |
| jeton: Const | 105 | 6 | 11 | 7 | 0 | 0 | 129 |
| jeton: Continue | 11 | 2 | 4 | 1 | 1 | 1 | 20 |
| jeton: Dot | 1909 | 128 | 422 | 74 | 64 | 104 | 2701 |
| jeton: DotDot | 55 | 3 | 19 | 2 | 3 | 12 | 94 |
| jeton: DotDotEq | 0 | 2 | 7 | 2 | 0 | 5 | 16 |
| jeton: Dyn | 0 | 0 | 2 | 1 | 0 | 2 | 5 |
| jeton: Else | 225 | 6 | 11 | 2 | 3 | 3 | 250 |
| jeton: Enum | 4 | 2 | 8 | 6 | 0 | 5 | 25 |
| jeton: Eq | 1653 | 70 | 291 | 183 | 31 | 77 | 2305 |
| jeton: EqEq | 389 | 9 | 13 | 1 | 10 | 5 | 427 |
| jeton: Extern | 47 | 2 | 3 | 3 | 0 | 3 | 58 |
| jeton: False | 101 | 6 | 11 | 8 | 0 | 0 | 126 |
| jeton: FatArrow | 140 | 31 | 106 | 41 | 6 | 26 | 350 |
| jeton: Fn | 836 | 35 | 150 | 128 | 22 | 78 | 1249 |
| jeton: For | 264 | 11 | 57 | 11 | 3 | 28 | 374 |
| jeton: Ge | 121 | 3 | 4 | 0 | 3 | 1 | 132 |
| jeton: Gt | 132 | 2 | 15 | 8 | 0 | 3 | 160 |
| jeton: If | 872 | 24 | 52 | 15 | 19 | 14 | 996 |
| jeton: Impl | 192 | 1 | 15 | 11 | 0 | 7 | 226 |
| jeton: Import | 45 | 2 | 12 | 8 | 9 | 5 | 81 |
| jeton: In | 86 | 11 | 53 | 9 | 3 | 24 | 186 |
| jeton: Indirect | 0 | 4 | 6 | 0 | 0 | 2 | 12 |
| jeton: Inout | 85 | 8 | 19 | 8 | 1 | 5 | 126 |
| jeton: LBrace | 2380 | 105 | 348 | 216 | 54 | 146 | 3249 |
| jeton: LBracket | 666 | 45 | 146 | 46 | 5 | 37 | 945 |
| jeton: LParen | 4077 | 224 | 933 | 333 | 119 | 260 | 5946 |
| jeton: Le | 44 | 2 | 2 | 0 | 2 | 1 | 51 |
| jeton: Let | 752 | 28 | 154 | 144 | 20 | 45 | 1143 |
| jeton: Lt | 232 | 5 | 5 | 5 | 1 | 5 | 253 |
| jeton: Match | 45 | 10 | 38 | 19 | 3 | 8 | 123 |
| jeton: Minus | 529 | 4 | 19 | 4 | 10 | 4 | 570 |
| jeton: MinusEq | 46 | 1 | 1 | 0 | 0 | 0 | 48 |
| jeton: MinusWrap | 11 | 0 | 0 | 0 | 1 | 0 | 12 |
| jeton: Module | 0 | 0 | 1 | 0 | 0 | 0 | 1 |
| jeton: Move | 9 | 0 | 2 | 2 | 0 | 0 | 13 |
| jeton: Not | 48 | 1 | 3 | 0 | 0 | 1 | 53 |
| jeton: NotEq | 101 | 2 | 4 | 0 | 2 | 0 | 109 |
| jeton: Or | 108 | 2 | 3 | 0 | 7 | 0 | 120 |
| jeton: Owned | 27 | 0 | 8 | 4 | 0 | 2 | 41 |
| jeton: Percent | 51 | 0 | 5 | 0 | 4 | 2 | 62 |
| jeton: Pipe | 83 | 5 | 8 | 1 | 2 | 1 | 100 |
| jeton: PipeEq | 10 | 0 | 0 | 0 | 0 | 0 | 10 |
| jeton: Plus | 436 | 7 | 18 | 9 | 2 | 7 | 479 |
| jeton: PlusEq | 123 | 19 | 50 | 5 | 2 | 13 | 212 |
| jeton: PlusWrap | 10 | 0 | 0 | 1 | 1 | 0 | 12 |
| jeton: Pub | 477 | 2 | 14 | 5 | 10 | 7 | 515 |
| jeton: Question | 0 | 15 | 19 | 4 | 0 | 4 | 42 |
| jeton: RBrace | 2380 | 105 | 348 | 214 | 54 | 146 | 3247 |
| jeton: RBracket | 666 | 45 | 146 | 46 | 5 | 37 | 945 |
| jeton: RParen | 4077 | 224 | 933 | 333 | 119 | 260 | 5946 |
| jeton: Return | 1061 | 31 | 103 | 30 | 25 | 30 | 1280 |
| jeton: SelfType | 21 | 0 | 0 | 1 | 0 | 0 | 22 |
| jeton: SelfValue | 1016 | 42 | 96 | 24 | 0 | 35 | 1213 |
| jeton: Shl | 92 | 0 | 1 | 0 | 1 | 0 | 94 |
| jeton: ShlEq | 20 | 0 | 0 | 0 | 0 | 0 | 20 |
| jeton: Shr | 102 | 0 | 0 | 0 | 3 | 0 | 105 |
| jeton: ShrEq | 2 | 0 | 0 | 0 | 0 | 0 | 2 |
| jeton: Slash | 98 | 2 | 7 | 2 | 1 | 2 | 112 |
| jeton: SlashEq | 3 | 0 | 0 | 0 | 0 | 0 | 3 |
| jeton: Star | 346 | 4 | 31 | 2 | 4 | 17 | 404 |
| jeton: StarEq | 13 | 2 | 2 | 0 | 0 | 0 | 17 |
| jeton: StarWrap | 7 | 0 | 0 | 0 | 0 | 0 | 7 |
| jeton: Static | 34 | 1 | 9 | 4 | 1 | 0 | 49 |
| jeton: Struct | 12 | 3 | 18 | 19 | 0 | 11 | 63 |
| jeton: Tilde | 6 | 0 | 0 | 1 | 0 | 0 | 7 |
| jeton: Trait | 12 | 0 | 1 | 1 | 0 | 2 | 16 |
| jeton: True | 90 | 6 | 19 | 15 | 1 | 1 | 132 |
| jeton: Type | 0 | 0 | 2 | 0 | 0 | 0 | 2 |
| jeton: Unsafe | 192 | 8 | 6 | 3 | 1 | 4 | 214 |
| jeton: Var | 454 | 25 | 92 | 18 | 7 | 28 | 624 |
| jeton: Where | 32 | 0 | 0 | 0 | 0 | 0 | 32 |
| jeton: While | 95 | 8 | 11 | 4 | 1 | 3 | 122 |
| jeton: chaîne avec interpolation | 33 | 20 | 212 | 31 | 16 | 37 | 349 |
| jeton: fin de ligne significative | 5847 | 313 | 1255 | 570 | 132 | 374 | 8491 |
| littéral: booléen | 191 | 12 | 36 | 20 | 1 | 2 | 262 |
| littéral: caractère | 77 | 7 | 18 | 4 | 0 | 1 | 107 |
| littéral: chaîne interpolée "{x}" | 33 | 20 | 212 | 31 | 16 | 37 | 349 |
| littéral: chaîne simple | 95 | 22 | 227 | 51 | 12 | 37 | 444 |
| littéral: entier | 2462 | 84 | 467 | 127 | 79 | 90 | 3309 |
| littéral: flottant | 254 | 6 | 97 | 22 | 16 | 25 | 420 |
| match: bras | 140 | 31 | 100 | 39 | 6 | 22 | 338 |
| match: bras à bloc => { … } | 36 | 5 | 8 | 11 | 3 | 4 | 67 |
| match: garde (if) | 0 | 0 | 3 | 3 | 0 | 2 | 8 |
| motif: _ | 99 | 6 | 31 | 16 | 1 | 8 | 161 |
| motif: alternative a \| b | 4 | 2 | 3 | 1 | 0 | 1 | 11 |
| motif: champ nommé (nom: motif) | 0 | 0 | 2 | 4 | 0 | 0 | 6 |
| motif: chemin qualifié a.B | 0 | 0 | 2 | 0 | 0 | 0 | 2 |
| motif: constructeur V(…) | 57 | 12 | 49 | 18 | 3 | 11 | 150 |
| motif: intervalle a..=b | 0 | 0 | 3 | 2 | 0 | 2 | 7 |
| motif: littéral | 62 | 11 | 20 | 11 | 0 | 5 | 109 |
| motif: tuple (a, b) | 27 | 7 | 13 | 3 | 0 | 1 | 51 |
| opérateur: != | 101 | 2 | 7 | 0 | 3 | 0 | 113 |
| opérateur: % | 52 | 0 | 9 | 0 | 5 | 2 | 68 |
| opérateur: & | 122 | 0 | 1 | 1 | 8 | 0 | 132 |
| opérateur: * | 346 | 4 | 36 | 3 | 6 | 18 | 413 |
| opérateur: *% | 7 | 0 | 1 | 0 | 0 | 0 | 8 |
| opérateur: + | 418 | 7 | 27 | 8 | 4 | 8 | 472 |
| opérateur: +% | 10 | 0 | 1 | 1 | 1 | 0 | 13 |
| opérateur: - | 413 | 5 | 7 | 0 | 7 | 4 | 436 |
| opérateur: - (préfixe) | 116 | 1 | 33 | 4 | 5 | 1 | 160 |
| opérateur: -% | 11 | 0 | 1 | 0 | 1 | 0 | 13 |
| opérateur: / | 99 | 2 | 13 | 2 | 6 | 3 | 125 |
| opérateur: < | 232 | 5 | 10 | 1 | 3 | 5 | 256 |
| opérateur: << | 92 | 0 | 4 | 0 | 1 | 0 | 97 |
| opérateur: <= | 44 | 2 | 2 | 0 | 4 | 1 | 53 |
| opérateur: == | 389 | 9 | 26 | 1 | 12 | 4 | 441 |
| opérateur: > | 132 | 2 | 20 | 5 | 2 | 2 | 163 |
| opérateur: >= | 121 | 3 | 6 | 0 | 4 | 1 | 135 |
| opérateur: >> | 102 | 0 | 2 | 0 | 3 | 0 | 107 |
| opérateur: ? (propagation) | 0 | 15 | 19 | 4 | 0 | 4 | 42 |
| opérateur: ^ | 13 | 0 | 1 | 0 | 0 | 0 | 14 |
| opérateur: and | 108 | 1 | 5 | 0 | 1 | 0 | 115 |
| opérateur: as (conversion) | 509 | 3 | 26 | 6 | 23 | 2 | 569 |
| opérateur: move | 9 | 0 | 2 | 2 | 0 | 0 | 13 |
| opérateur: not | 48 | 1 | 4 | 0 | 0 | 1 | 54 |
| opérateur: or | 108 | 2 | 5 | 0 | 7 | 0 | 122 |
| opérateur: \| | 72 | 0 | 2 | 0 | 2 | 0 | 76 |
| opérateur: ~ | 6 | 0 | 1 | 1 | 0 | 0 | 8 |
| paramètre: inout self | 54 | 5 | 10 | 2 | 0 | 3 | 74 |
| paramètre: nom: T (emprunt en lecture) | 749 | 18 | 58 | 24 | 12 | 25 | 886 |
| paramètre: nom: inout T | 30 | 3 | 6 | 5 | 1 | 1 | 46 |
| paramètre: nom: owned T | 16 | 0 | 7 | 4 | 0 | 2 | 29 |
| paramètre: owned self | 11 | 0 | 1 | 0 | 0 | 0 | 12 |
| paramètre: self | 279 | 1 | 22 | 13 | 0 | 16 | 331 |
| pub: méthode | 180 | 0 | 3 | 1 | 0 | 3 | 187 |
| pub: élément | 286 | 2 | 7 | 2 | 10 | 0 | 307 |
| struct: champ | 44 | 8 | 32 | 21 | 0 | 20 | 125 |
| struct: champ indirect | 0 | 2 | 3 | 0 | 0 | 1 | 6 |
| struct: champ pub | 11 | 0 | 4 | 2 | 0 | 4 | 21 |
| trait: avec super-traits | 1 | 0 | 0 | 0 | 0 | 0 | 1 |
| type: () | 3 | 0 | 1 | 0 | 0 | 0 | 4 |
| type: Self | 21 | 0 | 0 | 1 | 0 | 0 | 22 |
| type: chemin qualifié (a.B) | 1 | 0 | 0 | 1 | 1 | 0 | 3 |
| type: dyn Trait | 0 | 0 | 2 | 1 | 0 | 2 | 5 |
| type: fn avec convention inout/owned | 1 | 0 | 1 | 0 | 0 | 0 | 2 |
| type: fn(…) -> T | 15 | 0 | 7 | 2 | 0 | 2 | 26 |
| type: générique appliqué T[A] | 359 | 29 | 82 | 30 | 1 | 26 | 527 |
| type: tableau [T; N] | 31 | 0 | 7 | 1 | 1 | 0 | 40 |
| type: tuple | 13 | 0 | 2 | 0 | 0 | 0 | 15 |
| élément: const | 105 | 6 | 11 | 7 | 0 | 0 | 129 |
| élément: enum | 4 | 2 | 8 | 6 | 0 | 5 | 25 |
| élément: extern "C" fn | 47 | 2 | 3 | 2 | 0 | 3 | 57 |
| élément: extern "Rust" fn | 0 | 0 | 0 | 1 | 0 | 0 | 1 |
| élément: impl Trait for Type | 178 | 0 | 4 | 2 | 0 | 4 | 188 |
| élément: impl Type (inhérent) | 14 | 1 | 11 | 9 | 0 | 3 | 38 |
| élément: module (déclaration) | 0 | 0 | 1 | 0 | 0 | 0 | 1 |
| élément: static | 0 | 0 | 6 | 3 | 1 | 0 | 10 |
| élément: static var | 34 | 1 | 3 | 1 | 0 | 0 | 39 |
| élément: struct | 12 | 3 | 18 | 19 | 0 | 11 | 63 |
| élément: trait | 12 | 0 | 1 | 1 | 0 | 2 | 16 |
| élément: type (alias) | 0 | 0 | 2 | 0 | 0 | 0 | 2 |

## Emplacements (constructions rares, total <= 12)
- accès: .0 (tuple) (4) : tests/run/collections.nx:50, tests/run/collections.nx:50, tests/run/statics.nx:32, tests/run/structs_enums.nx:116
- affectation: &= (4) : lib/std/builtins.nx:78, lib/std/builtins.nx:509, lib/std/math.nx:174, lib/std/math.nx:185
- affectation: /= (3) : lib/std/fmt.nx:106, lib/std/fmt.nx:506, lib/std/rt.nx:118
- affectation: >>= (2) : lib/std/fmt.nx:951, lib/std/math.nx:214
- affectation: ^= (5) : lib/std/random.nx:151, lib/std/random.nx:152, lib/std/random.nx:153, lib/std/random.nx:154
- affectation: |= (10) : lib/std/builtins.nx:200, lib/std/builtins.nx:212, lib/std/builtins.nx:363, lib/std/builtins.nx:368
- attribut: @cfg (1) : tests/ui/cfg_errors.nx:5
- attribut: @cold (6) : lib/std/rt.nx:28, lib/std/rt.nx:204, lib/std/rt.nx:227, lib/std/rt.nx:252
- attribut: @derive(…) (4) : tests/run/collections.nx:4, tests/run/traits.nx:53, tests/run/traits.nx:71, docs/guide.md:413
- attribut: @export (1) : docs/spec/08-interop-c.md:65
- attribut: @index (1) : lib/std/vec.nx:175
- attribut: @inlin (1) : tests/ui/attributes.nx:1
- attribut: @test (5) : examples/tests_demo.nx:14, examples/tests_demo.nx:21, examples/tests_demo.nx:26, tests/ui/attributes.nx:8
- boucle: for avec motif déstructurant (2) : tests/run/collections.nx:51, tests/ui/patterns_misc.nx:36
- boucle: for x in &v (modification en place) (5) : examples/arbre.nx:81, tests/run/mutable_views.nx:81, tests/ui/mutable_views.nx:15, tests/ui/mutable_views.nx:19
- boucle: for x in a..=b (8) : examples/premiers.nx:8, examples/premiers.nx:27, tests/run/closures.nx:36, tests/run/control_flow.nx:69
- boucle: étiquette (nom: while/for) (4) : tests/run/control_flow.nx:83, tests/ui/loops_labels.nx:4, tests/ui/loops_labels.nx:7, tests/ui/loops_labels.nx:8
- contrôle: break étiquette (2) : tests/run/control_flow.nx:89, tests/ui/loops_labels.nx:5
- contrôle: continue étiquette (1) : tests/run/control_flow.nx:86
- contrôle: if let P = &e / while let (modification) (5) : examples/arbre.nx:54, lib/std/collections.nx:173, tests/run/mutable_views.nx:54, tests/ui/mutable_views.nx:11
- contrôle: match &e (modification) (5) : examples/arbre.nx:11, tests/run/mutable_views.nx:11, tests/ui/mutable_views.nx:3, docs/guide.md:227
- contrôle: while let (source) (2) : tests/run/closures.nx:80, docs/spec/04-expressions.md:259
- crochets: plusieurs arguments a[x, y] (4) : tests/run/errors.nx:49, tests/run/errors.nx:54, tests/ui/generics_errors.nx:11, tests/ui/indexing_iteration.nx:9
- enum: variante indirect (6) : examples/calculatrice.nx:17, examples/calculatrice.nx:18, tests/run/calculator.nx:17, tests/run/calculator.nx:18
- expression: () (3) : lib/std/io.nx:49, lib/std/io.nx:57, lib/std/io.nx:65
- fonction anonyme: fn(x) { … } (7) : tests/run/closures.nx:30, tests/run/closures.nx:44, tests/run/collections.nx:17, tests/run/collections.nx:43
- fonction anonyme: paramètre inout/owned (3) : tests/run/collections.nx:17, tests/run/collections.nx:43, docs/spec/09-bibliotheque.md:107
- fonction anonyme: paramètre typé (2) : tests/run/closures.nx:44, tests/ui/traits_errors.nx:48
- fonction anonyme: type de retour (1) : tests/run/closures.nx:44
- générique: enum paramétrée (4) : lib/std/core.nx:6, lib/std/core.nx:59, docs/spec/07-erreurs.md:16, docs/spec/07-erreurs.md:17
- générique: struct paramétrée (6) : lib/std/collections.nx:22, lib/std/collections.nx:275, lib/std/vec.nx:7, tests/run/generics.nx:4
- import: liste .{a, b} (8) : tests/run/collections.nx:2, tests/run/modules/util.nx:1, tests/run/modules.nx:3, tests/ui/imports_errors.nx:2
- import: module as alias (2) : tests/run/modules.nx:4, docs/spec/06-modules.md:21
- import: élément renommé (x as y) (2) : tests/run/modules.nx:3, docs/spec/06-modules.md:22
- instruction: var sans valeur initiale (4) : tests/ui/uninit_read.nx:2, tests/ui/uninit_read.nx:7, tests/ui/var_without_type.nx:2, docs/spec/04-expressions.md:156
- jeton: AmpEq (4) : lib/std/builtins.nx:78, lib/std/builtins.nx:509, lib/std/math.nx:174, lib/std/math.nx:185
- jeton: CaretEq (5) : lib/std/random.nx:151, lib/std/random.nx:152, lib/std/random.nx:153, lib/std/random.nx:154
- jeton: Dyn (5) : tests/run/traits.nx:114, tests/run/traits.nx:117, tests/ui/traits_errors.nx:47, docs/guide.md:430
- jeton: Indirect (12) : examples/arbre.nx:6, examples/arbre.nx:7, examples/calculatrice.nx:17, examples/calculatrice.nx:18
- jeton: MinusWrap (12) : lib/std/builtins.nx:475, lib/std/builtins.nx:731, lib/std/math.nx:189, lib/std/math.nx:199
- jeton: Module (1) : tests/run/modules/geometry/shapes.nx:1
- jeton: PipeEq (10) : lib/std/builtins.nx:200, lib/std/builtins.nx:212, lib/std/builtins.nx:363, lib/std/builtins.nx:368
- jeton: PlusWrap (12) : lib/std/builtins.nx:30, lib/std/builtins.nx:30, lib/std/builtins.nx:103, lib/std/builtins.nx:514
- jeton: ShrEq (2) : lib/std/fmt.nx:951, lib/std/math.nx:214
- jeton: SlashEq (3) : lib/std/fmt.nx:106, lib/std/fmt.nx:506, lib/std/rt.nx:118
- jeton: StarWrap (7) : lib/std/core.nx:231, lib/std/core.nx:232, lib/std/random.nx:108, lib/std/random.nx:109
- jeton: Tilde (7) : lib/std/core.nx:274, lib/std/core.nx:318, lib/std/core.nx:362, lib/std/core.nx:406
- jeton: Type (2) : tests/run/consts.nx:11, tests/run/structs_enums.nx:90
- match: garde (if) (8) : tests/run/control_flow.nx:7, tests/run/control_flow.nx:45, tests/run/control_flow.nx:133, tests/ui/exclusivity_guard.nx:10
- motif: alternative a | b (11) : examples/calculatrice.nx:70, examples/calculatrice.nx:71, lib/std/fmt.nx:95, lib/std/math.nx:1178
- motif: champ nommé (nom: motif) (6) : tests/run/structs_enums.nx:85, tests/run/structs_enums.nx:85, tests/ui/drop_rules.nx:23, tests/ui/patterns_misc.nx:27
- motif: chemin qualifié a.B (2) : tests/run/statics.nx:34, tests/run/statics.nx:35
- motif: intervalle a..=b (7) : tests/run/control_flow.nx:6, tests/run/control_flow.nx:141, tests/run/control_flow.nx:142, tests/ui/non_exhaustive.nx:20
- opérateur: *% (8) : lib/std/core.nx:231, lib/std/core.nx:232, lib/std/random.nx:108, lib/std/random.nx:109
- opérateur: ~ (8) : lib/std/core.nx:274, lib/std/core.nx:318, lib/std/core.nx:362, lib/std/core.nx:406
- paramètre: owned self (12) : lib/std/core.nx:28, lib/std/core.nx:37, lib/std/core.nx:45, lib/std/core.nx:81
- struct: champ indirect (6) : examples/arbre.nx:6, examples/arbre.nx:7, tests/run/mutable_views.nx:6, tests/run/mutable_views.nx:7
- trait: avec super-traits (1) : lib/std/core.nx:162
- type: () (4) : lib/std/io.nx:47, lib/std/io.nx:55, lib/std/io.nx:63, tests/run/generics.nx:84
- type: chemin qualifié (a.B) (3) : lib/std/math.nx:698, tests/numerics/softfloat.nx:17, tests/ui/impl_errors.nx:9
- type: dyn Trait (5) : tests/run/traits.nx:114, tests/run/traits.nx:117, tests/ui/traits_errors.nx:47, docs/guide.md:430
- type: fn avec convention inout/owned (2) : lib/std/collections.nx:170, tests/run/fn_values.nx:18
- élément: extern "Rust" fn (1) : tests/ui/ffi_errors.nx:1
- élément: module (déclaration) (1) : tests/run/modules/geometry/shapes.nx:1
- élément: static (10) : tests/numerics/softfloat.nx:10, tests/run/statics.nx:14, tests/run/statics.nx:15, tests/run/statics.nx:16
- élément: type (alias) (2) : tests/run/consts.nx:11, tests/run/structs_enums.nx:90

## Blocs de documentation non analysables : 5
- docs/spec/03-types.md:41
- docs/spec/03-types.md:59
- docs/spec/04-expressions.md:77
- docs/spec/04-expressions.md:168
- docs/spec/05-memoire.md:181
