# 09 — Bibliothèque standard 0.1

La bibliothèque standard est **écrite en NEXUL** (dossier `lib/std/`), à l'exception d'un
petit nombre d'opérations intrinsèques fournies par le compilateur et d'un runtime C
minimal (`runtime/`). Elle est embarquée dans `nxc` : aucune installation séparée.

Le runtime est **modulaire** au sens du cahier des charges (§5.3) : un programme
n'embarque que les fonctions qu'il utilise ; un programme sans `String` ni `Vec`
n'effectue aucune allocation.

## 9.1 Prélude

### `Option[T]`
| Méthode | Description |
|---|---|
| `is_some(self) -> bool`, `is_none(self) -> bool` | test |
| `unwrap(owned self) -> T` | valeur, ou panique si `None` |
| `expect(owned self, message: String) -> T` | idem avec message personnalisé |
| `unwrap_or(owned self, owned defaut: T) -> T` | valeur ou valeur par défaut |
| `take(inout self) -> Option[T]` | extrait la valeur et laisse `None` à la place |

### `Result[T, E]`
| Méthode | Description |
|---|---|
| `is_ok(self)`, `is_err(self)` | test |
| `unwrap(owned self) -> T`, `unwrap_err(owned self) -> E` | extraction, ou panique |
| `expect(owned self, message: String) -> T` | extraction avec message |
| `unwrap_or(owned self, owned defaut: T) -> T` | valeur ou valeur par défaut |
| `ok(owned self) -> Option[T]`, `err(owned self) -> Option[E]` | conversion |

### `String` — texte UTF-8 possédé
Invariant : le contenu est **toujours de l'UTF-8 valide**. Les littéraux ne provoquent
aucune allocation (ils désignent la mémoire statique du programme) ; la première
modification d'une chaîne issue d'un littéral la recopie sur le tas.

| Méthode | Description |
|---|---|
| `String.new()`, `String.with_capacity(n: usize)` | chaîne vide |
| `len(self) -> usize` | longueur en **octets** |
| `char_count(self) -> usize` | nombre de caractères (parcours) |
| `is_empty(self) -> bool` | |
| `push(inout self, c: char)`, `push_str(inout self, s: String)` | ajout en fin |
| `clone(self) -> String` | copie explicite |
| `byte_at(self, i: usize) -> u8` | octet `i` (bornes vérifiées) |
| `substring(self, debut: usize, fin: usize) -> String` | sous-chaîne par octets ; panique si hors bornes ou hors frontière de caractère |
| `contains`, `starts_with`, `ends_with`(self, motif: String) -> bool | recherche |
| `find(self, motif: String) -> Option[usize]` | position en octets |
| `replace(self, de: String, vers: String) -> String` | remplacement de toutes les occurrences |
| `trim(self) -> String` | sans espaces blancs aux extrémités |
| `to_uppercase(self)`, `to_lowercase(self) -> String` | casse ASCII |
| `split(self, separateur: String) -> Vec[String]`, `lines(self) -> Vec[String]` | découpage |
| `repeat(self, n: usize) -> String` | répétition |
| `chars(self) -> Vec[char]`, `bytes(self) -> Vec[u8]` | décomposition |
| `parse_int(self) -> Option[i64]`, `parse_float(self) -> Option[f64]` | analyse numérique |
| `clear(inout self)` | vide la chaîne |
| `as_ptr(self) -> Ptr[u8]` | adresse des octets (§08) |

Opérateurs : `+` (concaténation), `==`, `!=`, `<`, `<=`, `>`, `>=`, itération
`for c in s` (caractères).

### `Vec[T]` — tableau dynamique possédé
| Méthode | Description |
|---|---|
| `Vec.new()`, `Vec.with_capacity(n: usize)` | vecteur vide |
| `len(self)`, `capacity(self) -> usize`, `is_empty(self) -> bool` | |
| `push(inout self, owned v: T)` | ajout en fin (croissance géométrique ×2) |
| `pop(inout self) -> Option[T]` | retrait en fin |
| `insert(inout self, i: usize, owned v: T)`, `remove(inout self, i: usize) -> T` | |
| `swap(inout self, i: usize, j: usize)` | échange deux éléments |
| `clear(inout self)`, `truncate(inout self, n: usize)`, `reserve(inout self, n: usize)` | |

Indexation `v[i]` (bornes vérifiées) : lecture, écriture `v[i] = x`, emprunt
`f(v[i])` et `g(&v[i])`. Itération `for x in v`.

## 9.2 Modules

| Module | Contenu 0.1 |
|---|---|
| `std.math` | constantes `PI`, `E`, `TAU`, `SQRT_2`, `LN_2`, `LN_10` (les fonctions sont des méthodes des flottants, §4.9) |
| `std.io` | `read_line() -> Option[String]`, `read_file(chemin: String) -> Result[String, String]`, `write_file(chemin: String, contenu: String) -> Result[(), String]`, `append_file(...)`, `flush()` |
| `std.system` | `args() -> Vec[String]`, `exit(code: i32) -> Never`, `env(nom: String) -> Option[String]` |
| `std.time` | `monotonic_ns() -> u64` (horloge monotone, pour mesurer des durées) |
| `std.mem` | `size_of[T]()`, `align_of[T]()`, `swap[T](inout a, inout b)`, `replace[T](inout dest, owned v) -> T`, `forget[T](owned v)`, `drop[T](owned v)`, et en `unsafe` : `alloc`, `realloc`, `free`, `copy[T]`, `drop_in_place[T]` |

*(Hors 0.1, dans l'ordre du cahier des charges §14.1 : `collections` (Map, Set),
`text` (Unicode complet), `net`, `crypto` (adossé à des implémentations auditées),
`async`, `testing` avancé, `logging`, `serialization`.)*
