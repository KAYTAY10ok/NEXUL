# 09 — Bibliothèque standard

La bibliothèque standard est **entièrement écrite en NEXUL** (dossier `lib/std/`), à
l'exception d'un petit nombre d'opérations intrinsèques fournies par le compilateur. Le
runtime en fait partie : démarrage du programme (`rt.nx`), allocateur (`heap.nx`),
formatage et analyse exacts des nombres (`fmt.nx`), bibliothèque mathématique
(`math.nx`), flottants logiciels pour les processeurs sans unité flottante
(`builtins.nx`) et accès au système (`sys/windows.nx`, `sys/linux.nx`, `sys/wasi.nx`,
`sys/bare.nx`). Elle est embarquée dans `nxc` : aucune installation séparée, aucune
bibliothèque C.

Le runtime est **modulaire** au sens du cahier des charges (§5.3) : un programme
n'embarque que les fonctions qu'il utilise ; un programme sans `String` ni `Vec`
n'effectue aucune allocation.

## 9.1 Prélude

Le prélude, visible sans import, contient `Option`, `Result`, `String`, `Vec`, les
fonctions intégrées (§4.10) et les traits standards (§3.9) : `Eq`, `Ord` et `Ordering`,
`Clone`, `Display`, `Hash`, `Default`, `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`. Les types
primitifs, `String`, `Vec`, `Option` et `Result` les implémentent (pour `Vec`, `Option` et
`Result`, quand leurs éléments les implémentent). Le prélude contient aussi `Error`,
l'interface des erreurs que l'on montre aux utilisateurs (§7.2), implémentée par les
erreurs de `std` (`io.IoError`).

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
| `into_bytes(owned self) -> Vec[u8]` | les octets, sans copie |
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
| `first(self)`, `last(self) -> Option[T]` | premier, dernier élément (`T: Clone`) |
| `reverse(inout self)`, `extend(inout self, autre: Vec[T])` | |
| `contains(self, x: T) -> bool`, `index_of(self, x: T) -> Option[usize]` | recherche (`T: Eq`) |
| `sort(inout self)`, `binary_search(self, x: T) -> Option[usize]` | tri et recherche dichotomique dans un vecteur trié (`T: Ord`) |
| `min(self)`, `max(self) -> Option[T]`, `sum(self) -> T`, `dedup(inout self)` | |
| `join(self, separateur: String) -> String` | texte des éléments (`T: Display`) |
| `map(self, f: fn(T) -> U) -> Vec[U]`, `filter(self, f: fn(T) -> bool) -> Vec[T]` | transformations |
| `for_each`, `any`, `all`, `count`, `find`, `position`, `fold(init, f)` | parcours avec une fonction |
| `retain(inout self, f: fn(T) -> bool)`, `sort_by(inout self, f: fn(T, T) -> Ordering)` | |

Indexation `v[i]` (bornes vérifiées) : lecture, écriture `v[i] = x`, emprunt
`f(v[i])` et `g(&v[i])`. Itération `for x in v`.

## 9.2 Collections associatives (`std.collections`)

`Map[K, V]` (table de hachage, clés `K: Hash + Eq`) et `Set[T]` (ensemble) :

```nexul
import std.collections.{Map, Set}

fn main() {
    var ages: Map[String, i32] = Map.new()
    let _ = ages.insert("alice", 31)
    let _ = ages.insert("bob", 25)
    let _ = ages.update("alice", fn(inout a) {
        a += 1
    })
    println("{ages.get("alice")} {ages.get_or("zoé", 0)} {ages.len()}")

    var vus: Set[i32] = Set.new()
    for x in [3, 1, 3, 2] {
        if not vus.insert(x) {
            println("doublon : {x}")
        }
    }
}
```

| `Map[K, V]` | Description |
|---|---|
| `new()`, `len`, `is_empty`, `clear` | |
| `insert(inout self, owned k: K, owned v: V) -> Option[V]` | ajoute ou remplace ; renvoie l'ancienne valeur |
| `get(self, k: K) -> Option[V]`, `get_or(self, k: K, owned defaut: V) -> V` | lecture (copie, `V: Clone`) |
| `contains_key(self, k: K) -> bool`, `remove(inout self, k: K) -> Option[V]` | |
| `update(inout self, k: K, f: fn(inout V)) -> bool` | modifie la valeur sur place |
| `keys`, `values`, `entries`, `for_each(f: fn(K, V))` | parcours |

| `Set[T]` | Description |
|---|---|
| `new()`, `len`, `is_empty`, `clear`, `items` | |
| `insert(inout self, owned x: T) -> bool` | `false` si déjà présent |
| `contains`, `remove` | |
| `union`, `intersection`, `difference` | opérations ensemblistes |

Les deux implémentent `Clone`, `Display` et `Default`.

## 9.3 Modules

| Module | Contenu |
|---|---|
| `std.math` | constantes `PI`, `E`, `TAU`, `SQRT_2`, `LN_2`, `LN_10` ; `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `ln`, `log2`, `log10`, `pow`, `hypot`, `floor`, `ceil`, `round`, `trunc`, `fmod`, `scalbn` (aussi méthodes des flottants, §4.9), précises au dernier bit près (portage de fdlibm) |
| `std.io` | `read_line() -> Option[String]`, `read_file`, `read_bytes`, `write_file`, `write_bytes`, `append_file` (résultats `Result[_, IoError]`, §7.2), `IoError`, `flush()` |
| `std.system` | `args() -> Vec[String]`, `exit(code: i32) -> Never`, `env(nom: String) -> Option[String]` |
| `std.time` | `monotonic_ns()`, `unix_time()`, `unix_time_ns()`, `sleep(ms)`, `sleep_ns(ns)`, `utc_offset(...)` ; dates du calendrier : `Date` (`new`, `today`, `parse`, `add_days`, `add_months`, `weekday`, `days_until`…), `DateTime` (`now`, `now_utc`, `from_unix`…), `Weekday`, `is_leap_year`, `days_in_month` |
| `std.random` | aléa cryptographique du système : `secure_bytes`, `secure_u64`, `secure_range`, `secure_token` ; générateur reproductible `Rng` (`new(graine)`, `from_entropy`, `range`, `float`, `chance`, `shuffle`) |
| `std.collections` | `Map[K, V]`, `Set[T]` (§9.2) |
| `std.mem` | `size_of[T]()`, `align_of[T]()`, `swap`, `replace`, `forget`, `drop`, et en `unsafe` : `alloc`, `realloc`, `free`, `copy[T]`, `set_bytes`, `drop_in_place[T]`, `addr_of`, `read_volatile`, `write_volatile` (registres matériels), `zeroed[T]` |

*(Hors 0.2, dans l'ordre du cahier des charges §14.1 : `text` (Unicode complet), `net`,
`crypto` (adossé à des implémentations auditées), `async`, `testing` avancé, `logging`,
`serialization`, concurrence.)*
