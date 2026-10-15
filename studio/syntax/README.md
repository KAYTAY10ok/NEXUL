# `nexul-studio-syntax` — coloration lexicale de NEXUL

Crate Rust **sans dépendance** (ni GPUI, ni autre crate) qui découpe un texte NEXUL en
jetons classés ; l'éditeur de NEXUL Studio associe chaque classe à une couleur de son
thème. Elle suit le lexique de la spécification (`docs/spec/01-lexique.md`) et, jeton
par jeton, le lexer du compilateur (`compiler/nxc_syntax/src/lexer.rs`), ce que vérifie
un test de conformité.

Responsable : **Claude Outils** (voir `docs/refonte/AGENTS.md`). Utilisateur : Claude
Studio, dans `studio/app` (adaptateur GPUI).

## Utilisation depuis `studio/app`

```toml
[dependencies]
nexul-studio-syntax = { path = "../syntax" }
```

La crate a son propre espace de travail Cargo (`[workspace]` vide, son propre
`Cargo.lock`) : elle ne touche ni au workspace du compilateur ni à celui de `studio/app`.
Elle se construit avec les chaînes `windows-gnu` et `windows-msvc`.

## API

```rust
use nexul_studio_syntax::{tokens, Highlighter, Kind, Token};

// Tout le texte d'un coup.
let toks: Vec<Token> = tokens("nexul", "let x = 42 // réponse");
// Token { range: 0..3, kind: Keyword }, Token { range: 4..5, kind: Variable }, ...

// Incrémental, pour l'éditeur.
let mut h = Highlighter::new("nexul", &texte);
let lignes_a_redessiner = h.replace(debut..fin, "nouveau texte"); // octets dans le texte
for jeton in h.line_tokens(12) { /* plages relatives au début de la ligne 12 */ }
```

### `Token` et ses garanties

`Token { range: Range<usize>, kind: Kind }` — plage en **octets** (UTF-8) dans le texte.
Les jetons d'un résultat :

- sont triés, ne se chevauchent pas et ne sont jamais vides ;
- commencent et finissent sur des frontières de caractères ;
- ne contiennent jamais de saut de ligne (un commentaire de bloc sur trois lignes donne
  trois jetons) ;
- ne perdent aucun texte : entre deux jetons, il n'y a que des espaces, tabulations et
  sauts de ligne (et une marque d'ordre d'octets en tout début de fichier).

### `Kind`

| Classe | Contenu |
|---|---|
| `Keyword` | mots-clés de déclaration et autres : `fn`, `let`, `var`, `struct`, `pub`, `and`, `or`, `not`, `as`, `in`, `self`… et les mots réservés (`async`, `yield`…) |
| `Control` | `if`, `else`, `match`, `while`, `for`, `return`, `break`, `continue` |
| `Type` | types primitifs (`i32`, `f64`, `bool`, `char`, `Never`…), `Self`, noms en `CamelCase`, nom déclaré après `struct`, `enum`, `trait`, `type` |
| `Function` | nom après `fn`, nom suivi de `(` ou de `[...](` (`taille[u64]()`) |
| `Variable` | autres identificateurs (variables, champs, modules, étiquettes) |
| `Constant` | `true`, `false`, noms en `MAJUSCULES` (`MAX`, `TAILLE_MAX`) |
| `String` | chaînes et caractères, guillemets compris, hors échappements et interpolations |
| `Escape` | échappement valide : `\n`, `\{`, `\u{1F600}` |
| `Interpolation` | accolades `{` `}` d'une interpolation et sa spécification de format (`:>8.2`) ; l'expression interpolée reçoit les classes ordinaires |
| `Number` | entiers et flottants (`0xFF_FF`, `1.0e-9`) |
| `Comment` | `// …` et `/* … */` (imbriqués) |
| `DocComment` | `/// …` (mais `//// …` est un commentaire ordinaire) |
| `Attribute` | `@test`, `@intrinsic` (le `@` et le nom) |
| `Operator` | `+ - * / % +% -% *% & \| ^ ~ << >>`, affectations, comparaisons, `.. ..= -> => ?` |
| `Punctuation` | `( ) [ ] { } , ; . :`, `@` seul |
| `Invalid` | ce que le compilateur refuse (voir ci-dessous) |

`Kind::name()` donne un nom stable en minuscules (`"doc_comment"`…) pour les fichiers
de thème, `Kind::from_name()` l'inverse, `Kind::ALL` la liste.

La classification est **lexicale** : `Type`, `Function` et `Constant` reposent sur les
conventions de nommage (§1.3) et sur le contexte immédiat. Une coloration sémantique
exacte demandera les jetons sémantiques de `nxc` (futur `nxc lsp`, Codex Astra).

### Fonctions

| Fonction | Rôle |
|---|---|
| `tokens(lang, texte) -> Vec<Token>` | tokenise tout le texte ; langage inconnu : aucun jeton (texte brut) |
| `tokenize_line(lang, ligne, état) -> (Vec<Token>, LineState)` | une ligne, à partir de l'état en début de ligne ; plages relatives à la ligne |
| `language_for_path(chemin) -> Option<&Language>` | langage d'un fichier d'après son extension (`.nx`) |
| `is_supported(lang) -> bool` | le langage est-il coloré par cette crate ? |
| `LANGUAGES` | `[Language { id: "nexul", name: "NEXUL", extensions: ["nx"] }]` |
| `nexul::KEYWORDS`, `CONTROL`, `RESERVED`, `LITERAL_KEYWORDS`, `PRIMITIVE_TYPES` | listes de mots (utiles aussi pour la complétion) |

`LineState` est opaque (`Copy`, `Eq`, `Default` = début de fichier) : en NEXUL, seul un
commentaire de bloc s'étend sur plusieurs lignes, l'état est sa profondeur
d'imbrication. Une ligne tokenisée à partir de deux états égaux donne les mêmes jetons.

### `Highlighter` (incrémental)

Il garde, pour chaque ligne, son texte, ses jetons et l'état en début et en fin de ligne.
Après une modification, il retokenise à partir de la première ligne modifiée et s'arrête
dès qu'une ligne non modifiée retrouve son état d'origine : une frappe dans une ligne ne
retokenise que cette ligne ; ouvrir `/*` retokenise ce qui suit.

| Méthode | Rôle |
|---|---|
| `Highlighter::new(lang, texte)` | coloration initiale |
| `replace(plage_octets, texte) -> Option<Range<usize>>` | applique une modification de l'éditeur ; renvoie les lignes recalculées (numérotées après la modification), ou `None` sans rien changer si la plage n'est pas valide |
| `edit_lines(première, retirées, nouvelles_lignes) -> Range<usize>` | même chose, ligne par ligne |
| `set_text(texte)` | remplace tout |
| `line_count()`, `line_text(n)`, `line_tokens(n)` | lecture ligne par ligne (plages relatives à la ligne) |
| `tokens()`, `text()` | tout le document (`tokens()` = `tokens(lang, text())`) |

Les fins de ligne CRLF sont conservées ; le `\r` final d'une ligne n'appartient à aucun
jeton.

## Texte en cours de frappe et erreurs

La tokenisation **n'échoue jamais** et ne panique pas (testé sur du texte aléatoire) :

- chaîne, caractère ou interpolation non fermés : jusqu'à la fin de la ligne, en
  `String` (une chaîne NEXUL ne peut pas s'étendre sur plusieurs lignes) ;
- commentaire de bloc non fermé : jusqu'à la fin du texte ;
- imbrication de chaînes dans des interpolations limitée à 64 niveaux (au-delà, le reste
  de la ligne est du texte de chaîne) : aucune récursion profonde sur un fichier hostile.

Sont signalés en `Invalid`, à l'endroit où le compilateur les signale (test de
conformité) : caractère de contrôle bidirectionnel **même dans une chaîne ou un
commentaire** (« Trojan Source », E0003), autre caractère de contrôle (E0004), retour
chariot isolé (E0002), échappement inconnu ou `\u{…}` invalide (E0007), spécification de
format invalide (E0009), interpolation vide (E0014), caractère vide ou multiple (`''`,
`'ab'`, E0015), préfixe sans chiffres ou lettres collées à un nombre (`0x`, `10u8`,
E0016), caractère inattendu hors chaîne et commentaire (`!` seul, `$`, `é` dans un nom,
E0017).

Ne sont **pas** signalés, volontairement : construction non fermée (texte en cours de
frappe), `_` mal placé dans un nombre (E0006), mot réservé employé comme nom (coloré en
mot-clé, E0010), littéral trop grand (E0301, E0011). Le panneau de diagnostics
(`nxc check --json`) les montre.

## Tests

```
cargo test --manifest-path studio/syntax/Cargo.toml
cargo test --release --manifest-path studio/syntax/Cargo.toml -- --nocapture   # avec les mesures
```

- **Unitaires** (17) : chaque classe, opérateurs (plus long lexème), nombres, chaînes,
  échappements, interpolations imbriquées et formats, caractères, commentaires
  imbriqués sur plusieurs lignes, attributs, Trojan Source, texte en cours de frappe,
  imbrication hostile, `Highlighter`.
- **Corpus** (7) : tous les fichiers `examples/*.nx`, `tests/run/*.nx` et `lib/**.nx`
  (59 fichiers) sans jeton invalide ni perte de texte ; les 128 fichiers `.nx` du dépôt
  (y compris `tests/ui`, faux exprès) avec les garanties ; chaque préfixe des exemples
  (frappe depuis le début) ; frappe puis effacement caractère par caractère dans le
  `Highlighter` ; 5 000 textes aléatoires ; 1 500 modifications aléatoires comparées à
  une tokenisation complète ; un document de 100 000 lignes.
- **Conformité** (2) : sur les 126 fichiers que `nxc_syntax` lexe sans diagnostic, chaque
  jeton du compilateur est un jeton ici, de classe compatible, et inversement (hors
  parties de chaînes et commentaires) ; sur les 2 fichiers d'erreurs lexicales, chaque
  erreur listée ci-dessus est marquée `Invalid` ; les listes de mots-clés sont celles du
  compilateur. `nxc_syntax` n'est qu'une dépendance de développement, en lecture seule.

## Performances

Mesurées le 15 octobre 2026 (Windows 11, Rust 1.99.0 `x86_64-pc-windows-gnu`,
`--release`), sur un document de 105 463 lignes (2,8 Mo, 608 301 jetons) fait du code
du dépôt :

| Opération | Durée |
|---|---|
| `tokens` sur tout le document | 20 à 24 ms (environ 120 à 140 Mo/s) |
| `Highlighter::new` | 45 à 48 ms |
| une frappe au milieu (`replace`, une ligne retokenisée) | 0,4 ms |
| ouvrir `/*` en tête (tout ce qui suit retokenisé) | 9 ms |

## Limites

- Seul NEXUL est coloré ici. Pour les autres langages, `docs/studio/architecture-native.md`
  prévoit les grammaires de l'éditeur (tree-sitter de gpui-component) ; un langage peut
  être ajouté à cette crate à la demande de Claude Studio (le paramètre `lang` et
  `LANGUAGES` sont prévus pour cela).
- Coloration lexicale seulement (voir `Kind`).
