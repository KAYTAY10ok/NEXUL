# 08 — Interopérabilité C (ABI native)

Le cahier des charges (§12.1) demande que **l'ABI C soit la première interface stabilisée**.
NEXUL 0.1 permet d'appeler des fonctions C ; l'exposition de fonctions NEXUL à C est
prévue pour 0.2 (mode de compilation « bibliothèque »).

## 8.1 Déclarer une fonction C

```nexul
@link("m")
extern "C" fn cbrt(x: f64) -> f64

extern "C" fn puts(texte: Ptr[u8]) -> i32

fn main() {
    let r = unsafe { cbrt(27.0) }
    println("{r}")
}
```

- Seule la convention `"C"` existe en 0.1 (E0502 pour une autre chaîne).
- Une fonction `extern` n'a pas de corps ; l'appeler exige un contexte `unsafe`, car
  le compilateur ne peut vérifier ni ce qu'elle fait de la mémoire, ni sa signature.
- Le nom NEXUL est le symbole C (pas de décoration).
- `@link("nom")` demande l'édition de liens avec la bibliothèque `nom`
  (`-lnom`). La bibliothèque C standard est toujours liée.

## 8.2 Types autorisés à la frontière

| NEXUL | C |
|---|---|
| `i8` `i16` `i32` `i64` | `int8_t` `int16_t` `int32_t` `int64_t` |
| `u8` `u16` `u32` `u64` | `uint8_t` `uint16_t` `uint32_t` `uint64_t` |
| `isize` `usize` | `intptr_t` `size_t` |
| `f32` `f64` | `float` `double` |
| `bool` | `_Bool` |
| `char` | `uint32_t` |
| `Ptr[T]` | `T*` |
| `()` (retour) | `void` |

Tout autre type dans une signature `extern` est l'erreur E0503. *(Hors 0.1 : structures
`@repr("C")`, pointeurs de fonctions de rappel, variadiques.)*

## 8.3 Pointeurs bruts `Ptr[T]`

| Opération | Sûre ? | Description |
|---|---|---|
| `Ptr[T].null()` | oui | pointeur nul |
| `p.is_null()` | oui | test de nullité |
| `p == q`, `p != q` | oui | comparaison d'adresses |
| `p.offset(n: usize)` | non | adresse de l'élément `n` (arithmétique en éléments) |
| `p.read() -> T` | non | lit la valeur pointée (copie bit à bit : pour une ressource, la propriété est dupliquée et le programmeur doit en tenir compte) |
| `p.write(owned v: T)` | non | écrit sans détruire l'ancienne valeur |
| `p.cast[U]() -> Ptr[U]` | non | réinterprète le type pointé |
| `p.addr() -> usize` | oui | adresse numérique |

Une `String` expose `s.as_ptr()` (sûr à appeler, dangereux à déréférencer) et la
bibliothèque fournit `std.ffi.CString` *(prévu)* pour produire des chaînes terminées par
zéro. En 0.1, les littéraux de chaîne sont toujours suivis d'un octet nul en mémoire, ce
qui permet `puts("bonjour".as_ptr())`.
