# 06 — Modules, visibilité et prélude

## 6.1 Fichiers et modules

- **Un fichier = un module.** Le nom d'un module est le chemin du fichier relatif à la
  **racine des sources** (le dossier du fichier passé à `nxc`), avec `.` comme séparateur
  et sans l'extension : `geometrie/formes.nx` est le module `geometrie.formes`.
- La déclaration `module chemin` en tête de fichier est **facultative**. Si elle est
  présente, elle doit correspondre au chemin du fichier (E0220) : elle documente le
  module sans pouvoir le contredire.
- L'ordre des déclarations dans un module n'a pas d'importance : une fonction peut
  appeler une fonction définie plus bas, deux types peuvent se référencer mutuellement.
- Les imports cycliques entre modules sont permis.
- Les noms de modules sont des identificateurs (§1.3) ; un fichier dont le nom n'est pas
  un identificateur valide ne peut pas être importé.

## 6.2 Imports

```nexul
import std.math                     // utilisation : math.sqrt(2.0)
import geometrie.formes as f        // utilisation : f.Cercle(1.0)
import std.io.{read_line, read_file as lire}   // noms importés directement
```

- `import a.b.c` rend le module visible sous son dernier segment `c`.
- `import a.b.c as x` le rend visible sous le nom `x`.
- `import a.b.c.{n1, n2 as m}` importe des éléments publics du module.
- Importer un module inexistant (E0221), un élément non public (E0222) ou un nom déjà
  défini dans le module (E0223) est une erreur.
- Un import inutilisé produit l'avertissement W0004.

Les modules `std.*` sont fournis par le compilateur ; les autres sont cherchés sous la
racine des sources, puis dans les dépendances du projet. Un projet est décrit par
`nexul.toml` (nom, version, fichier principal, dépendances locales ou Git) ; `nxpm`
récupère les dépendances et fige dans `nexul.lock` le commit exact et l'empreinte SHA-256
de chacune. Le module racine d'une dépendance `geo` s'importe avec `import geo`.

## 6.3 Visibilité

- Par défaut, un élément (fonction, type, constante, alias) et un champ de structure sont
  **privés à leur module**.
- `pub` rend un élément visible depuis tout module qui l'importe.
- Les méthodes et fonctions associées d'un bloc `impl` sont privées au module du bloc
  sauf si elles sont marquées `pub`.
- Un bloc `impl` doit se trouver dans le **même module** que le type qu'il étend (E0224).
  Cette règle garantit qu'on trouve toutes les méthodes d'un type au même endroit.
- Une signature publique ne peut pas exposer un type privé (E0225).

## 6.4 Portées et résolution des noms

Un nom est cherché, dans l'ordre :
1. dans les portées locales (de la plus interne à la plus externe) ;
2. parmi les paramètres génériques de la fonction, puis du bloc `impl` ;
3. parmi les éléments du module et les noms importés ;
4. dans le **prélude**.

Une déclaration locale peut masquer un élément du module ou du prélude. Un élément du
module ne peut pas porter le nom d'un import (E0223).

## 6.5 Prélude

Visibles sans import dans tout module :

- les types primitifs, `Never`, `Ptr` ;
- `String`, `Vec`, `Option` (avec `Some`, `None`), `Result` (avec `Ok`, `Err`) ;
- les fonctions intégrées : `print`, `println`, `eprint`, `eprintln`, `panic`,
  `assert`, `unreachable`.

## 6.6 Point d'entrée

Le programme commence par la fonction `fn main()` du module racine (le fichier donné à
`nxc`). `main` ne prend pas de paramètres, ne renvoie rien et n'est pas générique (E0230).
Les arguments de la ligne de commande s'obtiennent avec `std.system.args()` et le code de
sortie se choisit avec `std.system.exit(code)`.

## 6.7 Attributs

| Attribut | S'applique à | Effet |
|---|---|---|
| `@test` | fonction sans paramètre | fonction de test exécutée par `nxc test` |
| `@link("nom")` | fonction `extern` | lie la bibliothèque native `nom` |
| `@lang("...")`, `@intrinsic`, `@index` | bibliothèque standard uniquement | relient le compilateur aux types et opérations fondamentaux (E0231 ailleurs) |
| `@track_caller` | bibliothèque standard uniquement | une panique dans la fonction indique la position de l'**appelant** (`v[10]`, `x.unwrap()`) |
| `@derive(Eq, Ord, ...)` | structure, énumération | génère les implémentations des traits standards (§3.9) |
| `@cfg("windows")` | tout élément | ne garde l'élément que si la condition est vraie pour la cible : système (`"windows"`, `"linux"`, `"macos"`, `"unix"`, `"wasi"`, `"bare"` sans système), architecture (`"x86_64"`, `"aarch64"`, `"arm"`, `"riscv"`, `"riscv32"`, `"riscv64"`, `"wasm"`, `"ptr32"`, `"ptr64"`), mode (`"debug"`, `"release"`, `"test"`), flottants (`"hard_float"`, `"soft_float"`) ; `"!linux"` (négation), `"windows\|unix"` (l'une ou l'autre) — E0234 si la condition est invalide |
| `@export` / `@export("nom")` | fonction | rend la fonction visible depuis C sous ce nom (§8.4) |
| `@inline`, `@cold`, `@no_builtins` | fonction | indications à l'optimiseur |

Un attribut inconnu est l'erreur E0232.
