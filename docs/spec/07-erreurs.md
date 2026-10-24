# 07 — Gestion des erreurs et diagnostics

NEXUL distingue deux familles d'erreurs à l'exécution :

| | Erreurs **récupérables** | **Paniques** |
|---|---|---|
| Exemples | fichier absent, saisie invalide, réseau coupé | indice hors bornes, débordement, `unwrap` sur `None`, invariant violé |
| Mécanisme | valeur `Result[T, E]` ou `Option[T]` | arrêt immédiat du programme |
| Peut être ignorée ? | non : le type l'impose | non |

Il n'y a **pas d'exceptions** : le flux de contrôle est toujours visible dans le source.

## 7.1 `Option` et `Result`

```nexul
enum Option[T] { Some(T), None }
enum Result[T, E] { Ok(T), Err(E) }
```

Méthodes principales : `is_some`, `is_none`, `unwrap`, `expect(message)`,
`unwrap_or(defaut)`, `take` (pour `Option`) ; `is_ok`, `is_err`, `unwrap`, `unwrap_err`,
`expect(message)`, `unwrap_or(defaut)` (pour `Result`).

Ignorer une valeur `Result` dans une instruction-expression produit l'avertissement W0005
(« le résultat peut contenir une erreur ») ; on écrit `let _ = f()` pour l'ignorer
explicitement.

## 7.2 L'opérateur `?`

```nexul
enum ErreurConfig {
    Io(io.IoError)
    Syntaxe(ErreurSyntaxe)
}

fn lire_config(chemin: String) -> Result[Config, ErreurConfig] {
    let texte = io.read_file(chemin)?       // en cas d'erreur : return Err(ErreurConfig.Io(e))
    let config = analyser(texte)?           // ErreurSyntaxe : return Err(ErreurConfig.Syntaxe(e))
    return Ok(config)
}
```

- `e?` avec `e: Result[T, E]` dans une fonction qui renvoie `Result[U, F]` : vaut `t` si
  `e` est `Ok(t)` ; sinon la fonction renvoie `Err(f)` après avoir détruit les valeurs
  locales, où `f` est (ADR 0015) :
  1. l'erreur elle-même si `E` et `F` sont le même type ;
  2. sinon, si `F` est une énumération qui a **exactement une** variante dont l'unique
     valeur est de type `E`, l'erreur dans cette variante (exemple ci-dessus) ;
  3. sinon, si `F` est `dyn Error` (`any Error` en édition 1) et que `E` implémente
     `Error`, l'erreur rangée dans une valeur `dyn Error` (une allocation) ;
  4. en **édition 0.2 seulement**, si `F` est `String` et que `E` implémente `Error`, le
     texte de l'erreur (`to_string`). C'est une transition : les programmes écrits quand
     `std.io` rendait des erreurs `String` continuent de compiler. Elle n'existe pas en
     édition 1, et `nxc migrate` signale les `?` concernés.

  Il n'y a aucune autre conversion : sinon E0360, qui indique la variante à ajouter, les
  variantes entre lesquelles `?` ne peut pas choisir, ou l'interface manquante.
- `e?` avec `e: Option[T]` dans une fonction qui renvoie `Option[U]` : vaut `t` ou
  renvoie `None`.
- `?` prend la valeur de `e` : comme `let v = e`, `e` doit être une valeur possédée
  (variable locale, paramètre `owned`, résultat d'un appel). Une place empruntée, un
  champ ou un élément donnent E0403 ; pour un `Option` rangé dans une place, `.take()`
  sort la valeur et laisse `None`.

### Erreurs de la bibliothèque standard

Les fonctions faillibles de `std` rendent une énumération d'erreurs par domaine, jamais
`String` : on filtre la cause sans comparer de textes. `std.io` rend `io.IoError` :

| Variante | Cause |
|---|---|
| `NotFound(String)` | rien n'existe à ce chemin |
| `PermissionDenied(String)` | le système refuse l'accès à ce chemin |
| `AlreadyExists(String)` | quelque chose existe déjà à ce chemin |
| `InvalidData(String)` | contenu inattendu (fichier texte qui n'est pas en UTF-8) |
| `Interrupted` | opération interrompue par un signal |
| `Other(i32, String)` | autre erreur : code du système (-1 s'il n'en donne pas) et message |

`IoError` implémente `Display` (message en anglais, comme les diagnostics), `Eq`,
`Clone` et `Error`. L'interface `Error` (`trait Error: Display`, sans autre méthode) est
celle des erreurs que l'on montre aux utilisateurs ; elle est dans le prélude.

## 7.3 Paniques

Une panique écrit sur la sortie d'erreur :

```
panic: index out of bounds: the index is 10 but the length is 3
  --> src/main.nx:12:15
```

puis termine le processus avec le **code de sortie 101**, sans dérouler la pile ni
exécuter les destructeurs. Ce comportement est déterministe, n'a aucun coût sur le
chemin normal, et convient aux systèmes embarqués (pas de tables de déroulement).
Sur les cibles sans système d'exploitation, le message est écrit par semi-hébergement
et le programme s'arrête (§10.5). *(Hors 0.2 : gestionnaire de panique configurable.)*

Les messages des paniques du runtime sont en anglais (comme les diagnostics),
avec la position exacte `fichier:ligne:colonne`.

## 7.4 Diagnostics du compilateur

Chaque diagnostic comporte :

- une **sévérité** (`error`, `warning`, `note`) et un **code** stable (`E0402`, `W0001`) ;
- un message principal **court et précis** ;
- la **position** `fichier:ligne:colonne` et l'extrait de source souligné ;
- des étiquettes secondaires (« la valeur a été déplacée ici ») ;
- quand c'est possible, une **aide** proposant une correction concrète.

```
error[E0402]: use of moved value `s`
  --> main.nx:4:13
   |
 3 |     let t = s
   |             - value moved here
 4 |     println(s)
   |             ^ value used here after move
   |
   = help: `String` is not copyable; use `s.clone()` to keep a copy
```

`nxc explain E0402` affiche une explication détaillée avec exemples.
La liste des codes est tenue dans `docs/spec/diagnostics.md`.

Règles de qualité :
- une erreur ne doit jamais en provoquer d'autres en cascade (les types erronés sont
  absorbés silencieusement) ;
- le compilateur ne s'arrête pas à la première erreur : il en rapporte autant que
  possible, dans l'ordre du source ;
- le compilateur ne doit **jamais** planter, quel que soit le source donné ; tout plantage
  est un bogue du compilateur. Vérifié par `compiler/nxc/tests/robustness.rs` (mutations
  du corpus analysées jusqu'à l'IR LLVM, sources pathologiques) ; pour cela, l'imbrication
  des expressions, blocs, types et motifs est limitée à 1000 niveaux et une expression à
  10 000 opérateurs binaires à la suite (E0113) ;
- un extrait de source montre au plus 160 colonnes d'une ligne, autour de l'étiquette :
  la taille des messages reste proportionnelle à leur nombre, même pour une ligne
  engendrée de plusieurs centaines de kilo-octets.

Les messages du compilateur sont en anglais. *(Hors 0.2 : catalogue de messages
traduits, à commencer par le français.)*
