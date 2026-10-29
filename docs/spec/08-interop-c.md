# 08 — Interopérabilité C (ABI native)

Le cahier des charges (§12.1) demande que **l'ABI C soit la première interface stabilisée**.
NEXUL appelle des fonctions C et expose ses propres fonctions à C, sous forme
d'exécutable ou de bibliothèque statique ou partagée (§8.4).

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
| `fn R(A, B)` | `R (*)(A, B)` (pointeur de fonction de rappel) |

| structure `@repr("C")` (§8.5) | `struct` de mêmes champs, passée par valeur |

Tout autre type dans une signature `extern` est l'erreur E0503. *(Pas encore : variadiques ;
structures par valeur dans un pointeur de fonction de rappel.)*

**Fonctions de rappel.** Une valeur de type fonction passée par valeur est toujours une
fonction **nommée** (une fonction anonyme n'y est pas acceptée, E0248) : c'est un simple
pointeur de code, à la convention d'appel C, que C peut appeler. Dans l'autre sens, une
fonction `@export` qui reçoit un pointeur de fonction de C l'appelle comme une fonction
NEXUL. Testé avec un vrai compilateur C (`tests/interop`, §8.4).

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

## 8.4 Exposer des fonctions NEXUL (`@export`)

```nexul
@export
fn nx_addition(a: i32, b: i32) -> i32 {
    return a + b
}

@export("nx_moyenne")
fn moyenne(a: f64, b: f64) -> f64 {
    return (a + b) / 2.0
}

fn main() {
    println("{nx_addition(2, 3)} {moyenne(1.0, 2.0)}")
}
```

- Une fonction `@export` utilise la convention d'appel C et le nom donné (par défaut son
  propre nom). Ses paramètres et son résultat sont des types autorisés à la frontière
  (§8.2) ; elle n'est pas générique (E0504).
- `nxc build lib.nx --lib` produit une bibliothèque partagée (`.dll`, `.so`, `.dylib`)
  dont les symboles exportés sont exactement les fonctions `@export` ; `--staticlib`
  produit une bibliothèque statique (`.lib`, `.a`). Le runtime de NEXUL y est inclus ;
  hors Windows, les deux sont compilées en code indépendant de la position (un
  exécutable C est PIE par défaut). Dans une bibliothèque, chaque `print` est écrit
  aussitôt (aucun point d'entrée NEXUL ne viderait le tampon à la fin).
- **Testé** (0.6.0) : `tests/interop/main.c`, compilé par le compilateur C du système
  (`cc`, `clang` sous Windows), appelle `tests/interop/lib.nx` (entiers, chaîne C, `bool`
  vérifié, sortie standard), qui rappelle C par les pointeurs de fonction reçus ; lié une fois à la bibliothèque statique et une fois à la partagée, sur
  Linux, macOS et Windows dans la CI (`compiler/nxc/tests/interop.rs`).
- Une bibliothèque **partagée** n'appelle pas une fonction du programme qui la charge par
  une déclaration `extern` : une DLL Windows ne peut rien importer de son programme
  (l'appel échoue au chargement, 0xc0000139), et sur macOS le nom serait cherché dans
  `libSystem`. Le programme lui passe des pointeurs de fonction. Une bibliothèque
  statique, elle, peut le faire (le nom est résolu à l'édition de liens).
- Une panique dans une fonction exportée arrête le processus avec le message habituel
  (§07) : elle ne traverse jamais la frontière avec C.
- **Valeurs reçues de C** (ADR 0011 T4) : un `bool` doit être l'octet 0 ou 1, un `char`
  une valeur scalaire Unicode (ni 0xD800 à 0xDFFF, ni au-delà de 0x10FFFF). Une fonction
  `@export` qui en reçoit les vérifie à l'entrée et panique à la position du paramètre
  sinon (« value is not a valid `bool` (0 or 1) », « value is not a valid `char` »). Un
  type distinct ou contraint de l'édition 1 (§3.4) traverse la frontière comme sa base ;
  un type contraint est vérifié à l'entrée comme par `as`. Le symbole exporté est une
  petite fonction qui fait ces vérifications puis appelle la fonction ; le code NEXUL
  appelle la fonction directement, sans vérification (ses valeurs sont déjà valides).

## 8.5 Structures C (`@repr("C")`)

```nexul
// edition: 1
@repr("C")
type Vec2 {
    f32 x
    f32 y
}

extern "C" fn Vec2 vec2_add(Vec2 a, Vec2 b)

@export
fn Vec2 nx_scale(Vec2 v, f32 k) {
    return Vec2(x: v.x * k, y: v.y * k)
}
```

- **Disposition** : celle de C — champs dans l'ordre de déclaration, chacun aligné
  naturellement, taille arrondie à l'alignement de la structure. `@repr("C")` le garantit
  (sans l'attribut, l'ordre des champs n'est pas promis).
- **Champs permis** : entiers, flottants, `Ptr[T]`, pointeurs de fonction de rappel
  (§8.2), tableaux `[T; n]` de ces types, autres structures `@repr("C")`. **Refusés** :
  `bool` et `char`, car C pourrait donner une valeur invalide (`u8`, `u32` à la place), et
  tout type qui possède une ressource. La structure a au moins un champ, pas de paramètre
  de type, pas de destructeur. Toute autre règle enfreinte est l'erreur **E0505**, comme
  `@repr` avec un autre argument que `"C"`.
- **Par valeur** : une telle structure est un paramètre ou un résultat permis d'une
  fonction `extern "C"` et d'une fonction `@export`, passée avec **la convention d'appel C
  de la cible** — celle de clang :

  | Cible | Paramètre | Résultat |
  |---|---|---|
  | x86-64 System V (Linux, macOS) | jusqu'à 16 octets : un ou deux morceaux de 8 octets en registres entiers ou SSE, si assez de registres restent ; sinon sur la pile (`byval`) | registres (`rax`/`rdx`, `xmm0`/`xmm1`) ; au-delà de 16 octets, en mémoire fournie par l'appelant (`sret`) |
  | x86-64 Windows | 1, 2, 4 ou 8 octets : un entier ; sinon un pointeur vers une copie | 1, 2, 4 ou 8 octets : `rax` ; sinon `sret` |
  | AArch64 (Linux, macOS, Windows) | 1 à 4 flottants identiques (HFA) : registres flottants ; jusqu'à 16 octets : un ou deux registres entiers ; sinon un pointeur vers une copie | de même, ou `sret` au-delà de 16 octets |
  | RISC-V 64 (LP64D) | un flottant, deux flottants, ou un flottant et un entier : registres flottants et entiers, s'il en reste ; sinon un ou deux registres entiers ; au-delà de 16 octets, un pointeur | de même, ou `sret` |
  | RISC-V 32 (ILP32) | jusqu'à 8 octets : un ou deux registres ; sinon un pointeur | de même, ou `sret` |
  | WebAssembly | une structure à un seul champ scalaire : ce scalaire ; sinon un pointeur (`byval`) | ce scalaire, ou `sret` |
  | ARM Cortex-M (AAPCS) | flottants identiques (HFA, `thumbv7em`) : registres VFP ; sinon registres et pile par mots | jusqu'à 4 octets : `r0` ; HFA : registres VFP ; sinon `sret` |

- **Ce qui est vérifié** (`compiler/nxc/tests/c_abi.rs`, 30 formes de structures) :
  - les signatures produites par `nxc` sont celles de clang pour les mêmes déclarations C,
    sur les 13 cibles hébergées et sans système ;
  - un programme NEXUL et du code C compilé par clang échangent ces structures dans les
    deux sens (arguments, résultats, registres épuisés par huit structures, mélange avec
    des scalaires), exécutés sur Linux x86-64, AArch64 et RISC-V (QEMU), WASI (Node),
    Cortex-M4 (QEMU), macOS (Apple silicon) et Windows x86-64 dans la CI.
- Sur ARM, le runtime fournit aussi les fonctions flottantes de l'ABI d'exécution ARM
  (`__aeabi_dadd`…) que le code C appelle, pour lier du C à un programme NEXUL sans
  bibliothèque de support.
- **Pas encore** : une structure par valeur dans le type d'un pointeur de fonction de
  rappel (E0503 : passer `Ptr[S]`) ; les unions et les champs de bits.

