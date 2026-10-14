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
fn lire_config(chemin: String) -> Result[Config, String] {
    let texte = io.read_file(chemin)?       // en cas d'erreur, `return Err(e)` immédiat
    let config = analyser(texte)?
    return Ok(config)
}
```

- `e?` avec `e: Result[T, E]` dans une fonction qui renvoie `Result[U, E]` : vaut `t` si
  `e` est `Ok(t)`, sinon renvoie `Err(erreur)` après avoir détruit les valeurs locales.
- `e?` avec `e: Option[T]` dans une fonction qui renvoie `Option[U]` : vaut `t` ou
  renvoie `None`.
- En 0.1, le type d'erreur doit être **identique** (E0360). *(Hors 0.1 : conversion
  d'erreurs via un trait `From`.)*

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
  est un bogue du compilateur.

Les messages du compilateur sont en anglais. *(Hors 0.2 : catalogue de messages
traduits, à commencer par le français.)*
