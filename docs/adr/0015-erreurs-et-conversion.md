# 0015 — Erreurs de la bibliothèque standard et conversion par `?`

Auteur : Claude (architecture, sémantique). **Statut : proposée** (17/10/2026). Change
des signatures publiques de `lib/std` : validation de l'utilisateur nécessaire. Constat
A3 de l'[audit du 17/10](../refonte/audit-claude-2026-10-17.md).

## Problème

- Toutes les fonctions faillibles de `std.io` rendent `Result[T, String]`
  (`read_file`, `read_bytes`, `write_file`, `write_bytes`, `append_file`). Un programme ne
  peut pas distinguer « fichier absent » de « accès refusé » sans comparer des textes
  (qui peuvent changer, être traduits, dépendre du système).
- `?` exige que le type d'erreur de l'expression soit **exactement** celui de la fonction
  (E0360). Dès qu'un programme combine deux sources d'erreurs (fichier puis analyse), il
  doit tout convertir à la main.
- Changer ces signatures **après** la 1.0 casserait tous les programmes. Avant, c'est une
  migration documentée.
- `nxc` n'a pas d'interfaces génériques : on ne peut pas écrire `interface From[E]`
  comme en Rust. La conversion ne peut donc pas reposer sur cette notion aujourd'hui.

## Solutions envisagées

| | Idée | Pour | Contre |
|---|---|---|---|
| A | Garder `String` | rien à faire | dette publique, comparaisons de textes |
| B | Interface générique `From[E]` utilisée par `?` | standard dans d'autres langages | exige des interfaces génériques (nouvelle notion du système de types) avant 0.4.0 |
| **C** | Énumérations d'erreurs de `std` ; `?` **emballe dans la variante** de l'énumération d'erreur de la fonction qui porte exactement ce type ; `any Error` pour les applications | aucune notion nouvelle ; la conversion se lit dans la déclaration du type | une règle de plus pour `?` |

## Décision proposée : C

### 1. Erreurs de la bibliothèque standard

```
type IoError = .NotFound(String path)
             | .PermissionDenied(String path)
             | .AlreadyExists(String path)
             | .InvalidData(String detail)        // ex. UTF-8 invalide
             | .Interrupted
             | .Other(i32 code, String message)   // code du système, message lisible
```

(Écriture de l'édition 1, illustrative.) `IoError` implémente `Display` (message lisible,
en anglais comme les diagnostics) et `Eq`. Toutes les fonctions faillibles de `std.io`
rendent `Result[T, IoError]`. Les futures bibliothèques (`std.fs`, `std.net`,
`std.process`) suivent le même modèle : une énumération par domaine, jamais `String`.

### 2. Interface `Error`

`interface Error is Display` (sans méthode obligatoire de plus en 1.0 ; une méthode
`source` pourra être ajoutée avec un corps par défaut, ce qui reste compatible). Les
erreurs de `std` l'implémentent.

### 3. Règle de `?`

Dans une fonction qui rend `Result[U, F]`, `e?` avec `e : Result[T, E]` :

1. si `E` = `F` : comportement actuel ;
2. sinon, si `F` est une énumération qui a **exactement une** variante dont l'unique
   valeur est de type `E` : l'erreur est emballée dans cette variante
   (`type ErreurApp = .Io(IoError) | .Analyse(ErreurAnalyse)` : `io.read_file(p)?` donne
   `ErreurApp.Io(e)`) ;
3. sinon, si `F` est `any Error` et que `E is Error` : l'erreur est rangée dans une valeur
   `any Error` (**allocation**, donc refusée sous `with noalloc`, P8) ;
4. sinon : E0360, avec la variante à ajouter proposée dans le message.

Aucune autre conversion. Pour `Option`, rien ne change.

### 4. Transition

- Les fonctions de `std.io` changent de type en 0.4.0. Pour que les programmes de
  l'édition 0.2 qui font `io.read_file(p)?` dans une fonction qui rend
  `Result[_, String]` continuent de compiler, une **règle transitoire** est réservée aux
  fichiers en édition 0.2 : si `F` est `String` et `E is Error`, `?` convertit par
  `to_string()`. Elle n'existe pas en édition 1. `nxc migrate` signale les fonctions
  concernées.
- Les programmes qui manipulent le texte de l'erreur (`Err(m) => m.len()`) cassent :
  le guide de migration montre `"{e}"` ou un filtrage sur les variantes.

### 5. Plus tard (compatible)

Si NEXUL ajoute des interfaces génériques (décision séparée, utile aussi pour un
protocole d'itération), une interface de conversion pourra compléter la règle 3.2. Les
programmes écrits avec la règle de la variante resteront valides.

## Conséquences

- Positives : erreurs filtrables (`when e { .NotFound(p) then … }`), messages stables,
  composition de sources d'erreurs sans code de conversion, aucune notion nouvelle du
  système de types, allocation seulement quand le programme choisit `any Error`.
- Négatives : changement de signatures de `std.io` (migration) ; `?` gagne une règle ;
  une énumération d'erreur par application.

## Composants concernés

`lib/std/io.nx`, `lib/std/sys/*.nx` (codes du système → variantes), `lib/std/core.nx`
(`Error`), `compiler/nxc_sema/src/check` (règle de `?`, E0360 enrichi),
`compiler/nxc_nir/src/build.rs` (`lower_try` : emballage), `compiler/nxc/src/migrate.rs`
(signalement), `docs/spec/07-erreurs.md`, `docs/spec/09-bibliotheque.md`, exemples et
`tests/run` qui utilisent `std.io`.

## Tests et preuves attendus

- `tests/run` : fichier absent → `.NotFound` sur Windows, Linux et WASI ; accès refusé
  → `.PermissionDenied` (Linux) ; UTF-8 invalide → `.InvalidData`.
- `tests/ui` : deux variantes de même type → E0360 ambigu ; aucune variante → E0360 avec
  suggestion ; `any Error` sous `with noalloc` refusé (quand P8 existe).
- `tests/run` en édition 0.2 : `?` vers `String` toujours accepté ; en édition 1, refusé.
- Aucun test existant ne perd de couverture (même nombre de cas d'erreur vérifiés).

## Risques

- Correspondance incomplète des codes d'erreur du système vers les variantes : `.Other`
  garde le code d'origine, rien n'est perdu.
- La règle 3.2 dépend de l'unicité de la variante : la rendre lisible dans le message
  d'erreur.
