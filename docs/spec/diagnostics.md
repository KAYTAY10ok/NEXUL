# Catalogue des diagnostics

Chaque diagnostic du compilateur a un code stable. `nxc explain <code>` affiche son
explication détaillée (en anglais, comme les messages du compilateur en 0.1).
Ce fichier est généré à partir du compilateur et vérifié par la suite de tests.

## Lexique (chapitre 01)

| Code | Signification |
|---|---|
| `E0001` | The source file is not valid UTF-8. |
| `E0002` | A carriage return (`\r`) appears without a following line feed. |
| `E0003` | A bidirectional control character appears in the source. |
| `E0004` | A control character appears where it is not allowed. |
| `E0005` | A block comment `/* ... */` is not closed. |
| `E0006` | An underscore is misplaced in a numeric literal. |
| `E0007` | Unknown escape sequence in a string or character literal. |
| `E0008` | A string literal is not closed on its line. |
| `E0009` | Invalid format specification in an interpolation. |
| `E0010` | A reserved keyword is used as a name. |
| `E0011` | A float literal is too large for `f64`. |
| `E0013` | An interpolation `{...}` in a string is not closed. |
| `E0014` | An interpolation `{}` is empty. |
| `E0015` | Invalid character literal. |
| `E0016` | Invalid digit or suffix in a numeric literal. |
| `E0017` | Unexpected character. |

## Syntaxe (chapitre 02)

| Code | Signification |
|---|---|
| `E0101` | The parser expected a different token. |
| `E0102` | An expression was expected. |
| `E0103` | A function has no body. |
| `E0104` | Comparison operators cannot be chained. |
| `E0105` | `_` is not a value. |
| `E0106` | A type was expected. |
| `E0107` | An import appears after other items. |
| `E0108` | An item was expected at module level. |
| `E0109` | A pattern was expected. |
| `E0111` | A form of edition 1 is planned but not implemented yet. |
| `E0112` | A form of NEXUL 0.2 does not exist in edition 1. |
| `E0113` | A construction is too deeply nested or too long. |
| `E0110` | An expression is computed, then its value is lost. |

## Noms, modules et attributs (chapitres 04 et 06)

| Code | Signification |
|---|---|
| `E0201` | A name is declared twice in the same scope. |
| `E0202` | A name cannot be found. |
| `E0203` | `self` is used outside of a method. |
| `E0207` | `break` or `continue` outside of a loop, or with an unknown label. |
| `E0212` | A type has a field and a method with the same name. |
| `E0213` | A built-in type name cannot be redefined (`i32`, `bool`, `Ptr`, `Never`...). |
| `E0220` | The `module` declaration does not match the file path. |
| `E0221` | An imported module cannot be found. |
| `E0222` | An imported item does not exist or is private. |
| `E0223` | The same name is imported twice, or is both imported and defined. |
| `E0224` | An `impl` block must be in the module that declares the type. |
| `E0226` | A private field is accessed from another module. |
| `E0227` | A private function or method is called from another module. |
| `E0230` | Missing or invalid `main` function. |
| `E0231` | This attribute is reserved for the standard library (`@lang`, `@intrinsic`, `@index`, `@track_caller`). |
| `E0232` | Unknown or misplaced attribute. |
| `E0233` | A `@test` function must be a free function without parameters, generics or return value. |
| `E0234` | Invalid `@cfg` condition. |
| `E0240` | A trait name is expected. |
| `E0241` | A function of an `impl Trait for Type` block does not match the trait. |
| `E0242` | An `impl Trait for Type` block does not implement every method of the trait. |
| `E0243` | A trait is implemented twice for the same type. |
| `E0244` | A trait requires another trait (its supertrait) that the type does not implement. |
| `E0245` | A method name is declared by several traits implemented by the type. |
| `E0246` | This trait cannot be used as `dyn Trait`. |
| `E0247` | An `impl Trait for Type` block is in an unrelated module. |
| `E0248` | An anonymous function is used where it could outlive the call that receives it. |
| `E0249` | A type does not implement a trait required by a bound. |
| `E0250` | A type declared `Copy` cannot be copied bit for bit. |
| `E0251` | Invalid distinct or constrained type (edition 1, ADR 0014). |
| `E0252` | A constant does not satisfy the condition of its constrained type. |

## Types, motifs et constantes (chapitres 03 et 04)

| Code | Signification |
|---|---|
| `E0301` | A literal or constant does not fit in its type. |
| `E0302` | Mismatched types. |
| `E0303` | An operator is applied to a type that does not support it. |
| `E0304` | Wrong number of arguments. |
| `E0305` | Named arguments are only used to build structures: `Point(x: 1.0, y: 2.0)`. |
| `E0306` | An enum variant is used incorrectly. |
| `E0307` | A method is called like an associated function, or the reverse. |
| `E0308` | No method with this name exists on the type. |
| `E0309` | No field with this name exists on the type. |
| `E0310` | Invalid structure construction. |
| `E0311` | A recursive type has infinite size. |
| `E0312` | An enum must have at least one variant. |
| `E0313` | `[x; n]` duplicates `x`, which must therefore be copyable. |
| `E0314` | A type is used as a value, or a value as a type. |
| `E0315` | Wrong number of generic arguments. |
| `E0316` | This function cannot be used as a value. |
| `E0317` | This value cannot be indexed. |
| `E0318` | This value cannot be iterated with `for`. |
| `E0319` | This value is not a function and cannot be called. |
| `E0320` | A type cannot be inferred. |
| `E0321` | A `var` declaration needs a type or an initial value: `var total: i64`. |
| `E0322` | This conversion is not possible with `as`. |
| `E0330` | An `if` used as a value needs an `else` branch, so that every case produces a value. |
| `E0331` | A function may finish without returning its value. |
| `E0340` | A `match` is not exhaustive. |
| `E0341` | Alternatives (`a \| b`) cannot bind variables in NEXUL 0.1. |
| `E0342` | A struct pattern must mention every field, or end with `..`: `Point(x: 0, ..)`. |
| `E0343` | The pattern of a `let` or a `for` must always match. |
| `E0344` | The same name is bound twice in one pattern. |
| `E0345` | Wrong number of sub-patterns for a variant or struct. |
| `E0346` | Invalid literal or range pattern. |
| `E0347` | A range loop binds a single variable: `for i in 0..n`. |
| `E0348` | `.Name` names a variant of the type the context expects (edition 1). |
| `E0349` | `else` after a value (edition 1) gives a fallback for an `Option` that is empty or a `Result` that is an error. |
| `E0350` | This value cannot be formatted. |
| `E0351` | This format specification does not apply to the value's type. |
| `E0360` | The `?` operator is applied to a value that is not `Option` or `Result`, or its error |
| `E0361` | The `?` operator is used in a function that does not return `Option` or `Result`. |
| `E0370` | Arithmetic overflow or division by zero while evaluating a constant. |
| `E0371` | This expression cannot be evaluated at compile time. |
| `E0372` | A constant depends on itself. |
| `E0373` | Invalid array length: it must be a constant integer between 0 and 4294967295. |
| `E0374` | A static variable cannot hold a resource. |

## Modèle mémoire (chapitre 05)

| Code | Signification |
|---|---|
| `E0401` | A variable is read before being initialized. |
| `E0402` | A value is used after being moved. |
| `E0403` | A resource cannot be moved out of a field, an element or a borrowed value. |
| `E0405` | An immutable place is modified. |
| `E0406` | `move` takes a whole variable that owns its value: `move v`. |
| `E0407` | Only places (variables, fields, elements) can be assigned. |
| `E0410` | An `inout` argument must be a modifiable place. |
| `E0411` | An argument passed to an `inout` parameter must be marked with `&`. |
| `E0412` | `&` is only used to pass `inout` arguments. |
| `E0420` | Law of exclusivity: a value cannot be modified while it is accessed elsewhere. |
| `E0430` | A destructor (`drop`) cannot be called explicitly. |
| `E0431` | A value whose type defines `drop` cannot be taken apart by a pattern. |
| `E0432` | A destructor must be declared `fn drop(inout self)`. |

## Code non sûr et interopérabilité C (chapitre 08)

| Code | Signification |
|---|---|
| `E0501` | This operation requires an `unsafe` block. |
| `E0502` | Unsupported ABI: NEXUL 0.1 only supports `extern "C"`. |
| `E0503` | This type cannot cross the C boundary. |
| `E0504` | Invalid `@export` function. |

## Erreurs internes

| Code | Signification |
|---|---|
| `E0900` | The standard library is incomplete (internal error). |

## Avertissements

| Code | Signification |
|---|---|
| `W0001` | A variable or parameter is never read. |
| `W0002` | A `match` arm can never be reached: earlier arms already match all its values. |
| `W0003` | A `var` is never modified: declare it with `let`. |
| `W0004` | An import is never used. |
| `W0005` | A `Result` is ignored, which may hide an error. |
| `W0006` | Code after `return`, `break`, `continue` or `panic` is never executed. |
| `W0007` | An uppercase name in a pattern is a new variable, not a variant. |
| `W0008` | A conversion with `as` converts a value to its own type. |
