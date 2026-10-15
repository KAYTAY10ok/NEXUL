//! Explanations of the diagnostic codes (`nxc explain E0402`).

pub fn explain(code: &str) -> Option<&'static str> {
    let code = code.to_ascii_uppercase();
    CODES.iter().find(|(c, _)| *c == code).map(|(_, t)| *t)
}

pub fn all_codes() -> impl Iterator<Item = &'static str> {
    CODES.iter().map(|(c, _)| *c)
}

const CODES: &[(&str, &str)] = &[
    ("E0001", "The source file is not valid UTF-8.\n\nNEXUL source files must be encoded in UTF-8. Re-save the file with the UTF-8 encoding."),
    ("E0002", "A carriage return (`\\r`) appears without a following line feed.\n\nLines must end with `\\n` or `\\r\\n`."),
    ("E0003", "A bidirectional control character appears in the source.\n\nThese invisible characters can make code look different from what the compiler sees\n(\"Trojan Source\", CVE-2021-42574). They are forbidden everywhere, including comments\nand strings. Inside a string, write them as escapes: \"\\u{202E}\"."),
    ("E0004", "A control character appears where it is not allowed.\n\nWrite control characters in strings as escapes, for example \"\\t\" or \"\\u{1B}\"."),
    ("E0005", "A block comment `/* ... */` is not closed.\n\nBlock comments nest: every `/*` needs its own `*/`."),
    ("E0006", "An underscore is misplaced in a numeric literal.\n\nUnderscores separate digits: `1_000_000` is valid, `1_`, `0x_FF` and `1._5` are not."),
    ("E0007", "Unknown escape sequence in a string or character literal.\n\nValid escapes: \\n \\r \\t \\0 \\\\ \\' \\\" \\{ \\} and \\u{1F600}."),
    ("E0008", "A string literal is not closed on its line.\n\nStrings cannot contain a raw line break in NEXUL 0.1; write \\n instead."),
    ("E0009", "Invalid format specification in an interpolation.\n\nFormat: {value:[[fill]align][+][0][width][.precision][x|X|o|b|e]}\nExamples: {x:>8} {price:.2} {n:08x} {name:-^20}"),
    ("E0010", "A reserved keyword is used as a name.\n\nThese words are reserved for future versions: async await class defer interface loop\nmacro ref try yield."),
    ("E0011", "A float literal is too large for `f64`."),
    ("E0013", "An interpolation `{...}` in a string is not closed.\n\nTo write a literal brace in a string, escape it: \"\\{\"."),
    ("E0014", "An interpolation `{}` is empty.\n\nWrite an expression between the braces, or escape the brace: \"\\{\"."),
    ("E0015", "Invalid character literal.\n\nA character literal contains exactly one character: 'a', '\\n', '\\u{E9}'.\nUse double quotes for strings."),
    ("E0016", "Invalid digit or suffix in a numeric literal.\n\nNEXUL has no literal suffixes: write `let x: u8 = 200` or `200 as u8`."),
    ("E0017", "Unexpected character.\n\nIdentifiers are ASCII in NEXUL 0.1. Logical operators are written `and`, `or`, `not`."),
    ("E0101", "The parser expected a different token.\n\nCheck the syntax around the indicated position. Statements end at the end of the line;\na line ending with an operator continues on the next line."),
    ("E0102", "An expression was expected.\n\nNote that a block `{ ... }` is not an expression on its own: structures are built with\nparentheses, `Point(x: 1.0, y: 2.0)`."),
    ("E0103", "A function has no body.\n\nOnly `@intrinsic` functions of the standard library and `extern \"C\"` declarations\nhave no body."),
    ("E0104", "Comparison operators cannot be chained.\n\n`a < b < c` is ambiguous in most languages; write `a < b and b < c`."),
    ("E0105", "`_` is not a value.\n\n`_` is the wildcard pattern. To discard a value, write `let _ = expression`."),
    ("E0106", "A type was expected.\n\nGeneric arguments use square brackets: `Vec[i32]`, `Option[String]`.\nArrays are written `[T; N]`."),
    ("E0107", "An import appears after other items.\n\nAll `import` declarations come first in a file."),
    ("E0108", "An item was expected at module level.\n\nA module contains `fn`, `struct`, `enum`, `trait`, `impl`, `const`, `static`, `type`\nand `extern` declarations. Variables only exist inside functions; use `const` for constants."),
    ("E0109", "A pattern was expected."),
    ("E0110", "An expression is computed, then its value is lost.\n\nA statement such as `- b`, `not ok` or `(x + 1)` has no effect. It is almost always a line cut in two: a new line ends the statement, so\n\n    let total = a\n        - b\n\nis `let total = a` followed by the statement `-b`. To continue an expression on the next line, end the first line with the operator:\n\n    let total = a -\n        b\n\nTo compute a value and drop it on purpose, write `let _ = expression`."),
    ("E0201", "A name is declared twice in the same scope.\n\nNames must be unique within a module, a block, a parameter list or a pattern.\nShadowing is allowed in a nested block."),
    ("E0202", "A name cannot be found.\n\nCheck the spelling, and import the module that declares it: `import std.io`."),
    ("E0203", "`self` is used outside of a method.\n\nMethods are declared in `impl Type { ... }` with `self` as their first parameter."),
    ("E0207", "`break` or `continue` outside of a loop, or with an unknown label."),
    ("E0212", "A type has a field and a method with the same name.\n\nRename one of them: `value.name` and `value.name()` must not be confused."),
    ("E0213", "A built-in type name cannot be redefined (`i32`, `bool`, `Ptr`, `Never`...)."),
    ("E0220", "The `module` declaration does not match the file path.\n\nThe module `a.b` lives in the file `a/b.nx` under the source root."),
    ("E0221", "An imported module cannot be found.\n\nUser modules are files next to the main file: `import geometry.shapes` loads\n`geometry/shapes.nx`. Standard modules: std.io, std.system, std.time, std.math, std.mem."),
    ("E0222", "An imported item does not exist or is private.\n\nOnly items marked `pub` can be used from other modules."),
    ("E0223", "The same name is imported twice, or is both imported and defined."),
    ("E0224", "An `impl` block must be in the module that declares the type.\n\nThis keeps all the methods of a type in one place."),
    ("E0226", "A private field is accessed from another module.\n\nMark the field `pub`, or provide a method."),
    ("E0227", "A private function or method is called from another module.\n\nMark it `pub` to export it."),
    ("E0230", "Missing or invalid `main` function.\n\nA program starts with `fn main()` (no parameters, no return value) in the main file.\nArguments: `std.system.args()`. Exit code: `std.system.exit(code)`."),
    ("E0231", "This attribute is reserved for the standard library (`@lang`, `@intrinsic`, `@index`, `@track_caller`)."),
    ("E0232", "Unknown or misplaced attribute.\n\nAttributes: `@test` on functions, `@link(\"lib\")` on `extern` functions."),
    ("E0233", "A `@test` function must be a free function without parameters, generics or return value."),
    ("E0234", "Invalid `@cfg` condition.

`@cfg(\"condition\")` keeps an item only when the condition holds for the target being
compiled. Conditions: windows, linux, macos, wasi, bare, unix, x86_64, aarch64, arm,
riscv64, riscv32, riscv, wasm32, wasm, ptr32, ptr64, debug, release, test, hard_float,
soft_float. Negate with `!` and combine alternatives with `|`:

    @cfg(\"windows\")
    fn path_separator() -> char { return '\\' }

    @cfg(\"!windows\")
    fn path_separator() -> char { return '/' }"),
    ("E0240", "A trait name is expected.\n\nBounds (`[T: Display]`, `where T: Ord`), supertraits (`trait Ord: Eq`), `impl Trait for\nType` and `dyn Trait` name traits declared with `trait` (or imported)."),
    ("E0241", "A function of an `impl Trait for Type` block does not match the trait.\n\nEvery function must be declared by the trait, with the same parameters, conventions and\nreturn type, `Self` being replaced by the implementing type:\n\n    trait Shape { fn area(self) -> f64 }\n    impl Shape for Circle { fn area(self) -> f64 { return 3.14 * self.r * self.r } }"),
    ("E0242", "An `impl Trait for Type` block does not implement every method of the trait.\n\nThe methods without a default body in the trait must all be implemented."),
    ("E0243", "A trait is implemented twice for the same type.\n\nA type implements a trait at most once in a program."),
    ("E0244", "A trait requires another trait (its supertrait) that the type does not implement.\n\n`trait Ord: Eq` means every type implementing `Ord` implements `Eq` too: add the\nmissing `impl` (or `@derive(Eq)`)."),
    ("E0245", "A method name is declared by several traits implemented by the type.\n\nRename one of the methods."),
    ("E0246", "This trait cannot be used as `dyn Trait`.\n\nThrough `dyn`, methods are called on a value whose type is only known at run time: each\nmethod must take `self` or `inout self`, have no generic parameters, and not use `Self`\nelsewhere in its signature. Use a generic parameter `[T: Trait]` instead."),
    ("E0247", "An `impl Trait for Type` block is in an unrelated module.\n\nIt must be in the module that declares the trait or the one that declares the type, so\nthat a type implements a trait at most once in a program."),
    ("E0248", "An anonymous function is used where it could outlive the call that receives it.\n\nAn anonymous function (`fn(x) => x * 2`) uses the variables around it in place: it can\nonly be passed directly as an argument to a parameter of function type, and that\nparameter cannot be stored, returned or put in a collection. To keep a function in a\nvalue, use a named function."),
    ("E0249", "A type does not implement a trait required by a bound.\n\nThe called function declares `[T: Trait]` (or `where T: Trait`): its type arguments must\nimplement the trait. Implement it (`impl Trait for Type`, or `@derive(Trait)` for `Eq`,\n`Ord`, `Clone`, `Display`, `Hash`, `Default`), or add the bound to your own generic\nparameter."),
    ("E0301", "A literal or constant does not fit in its type.\n\nUse a wider type (`i64`, `u64`), or check the value."),
    ("E0302", "Mismatched types.\n\nNEXUL never converts values implicitly: convert numbers with `as`\n(`x as f64`), wrap optional values in `Some(...)`, build strings with interpolation."),
    ("E0303", "An operator is applied to a type that does not support it.\n\nArithmetic works on numbers, `+` also concatenates strings, comparisons work on\nnumbers, `char`, `bool`, `String` and enums without data."),
    ("E0304", "Wrong number of arguments."),
    ("E0305", "Named arguments are only used to build structures: `Point(x: 1.0, y: 2.0)`."),
    ("E0306", "An enum variant is used incorrectly.\n\nVariants with values are called: `Shape.Circle(1.0)`. Variants without values are\nwritten without parentheses: `Shape.Empty`."),
    ("E0307", "A method is called like an associated function, or the reverse.\n\nMethods (with `self`) are called on a value: `v.len()`. Associated functions are called\non the type: `Vec.new()`."),
    ("E0308", "No method with this name exists on the type."),
    ("E0309", "No field with this name exists on the type.\n\nTuple fields are numbered: `t.0`, `t.1`."),
    ("E0310", "Invalid structure construction.\n\nEvery field must be given, by name, in declaration order:\n`Person(name: \"Ada\", age: 36)`."),
    ("E0311", "A recursive type has infinite size.\n\nMark the recursive field or variant `indirect`: its value is then stored on the heap,\ntransparently.\n\n    enum Tree {\n        Leaf(i64)\n        indirect Node(Tree, Tree)\n    }"),
    ("E0312", "An enum must have at least one variant."),
    ("E0313", "`[x; n]` duplicates `x`, which must therefore be copyable."),
    ("E0314", "A type is used as a value, or a value as a type."),
    ("E0315", "Wrong number of generic arguments."),
    ("E0316", "This function cannot be used as a value.\n\nIn NEXUL 0.1, only non-generic functions can be passed as values; call the others."),
    ("E0317", "This value cannot be indexed.\n\nArrays and `Vec` are indexed with `v[i]`. Strings are not indexed by integer because\na character may take several bytes: use `s.byte_at(i)`, `s.chars()` or `for c in s`."),
    ("E0318", "This value cannot be iterated with `for`.\n\n`for` iterates over ranges (`0..n`), arrays, `Vec` and `String` (characters)."),
    ("E0319", "This value is not a function and cannot be called."),
    ("E0320", "A type cannot be inferred.\n\nAdd a type annotation: `let v: Vec[i32] = Vec.new()`. A method can only be called\non a value whose type is already known at that point."),
    ("E0321", "A `var` declaration needs a type or an initial value: `var total: i64`."),
    ("E0322", "This conversion is not possible with `as`.\n\n`as` converts between numbers, from `bool` and `char` to integers, and from integers\nto `char`. For strings, use `value.to_string()` and `text.parse_int()`."),
    ("E0330", "An `if` used as a value needs an `else` branch, so that every case produces a value."),
    ("E0331", "A function may finish without returning its value.\n\nNEXUL has no implicit return: every execution path of a function that returns a value\nmust end with `return value` (or diverge with `panic`)."),
    ("E0340", "A `match` is not exhaustive.\n\nEvery possible value must be handled. Add the missing cases, or a final `_ => ...` arm.\nArms with an `if` guard do not count."),
    ("E0341", "Alternatives (`a | b`) cannot bind variables in NEXUL 0.1."),
    ("E0342", "A struct pattern must mention every field, or end with `..`: `Point(x: 0, ..)`."),
    ("E0343", "The pattern of a `let` or a `for` must always match.\n\nUse `if let` or `match` for patterns that may fail."),
    ("E0344", "The same name is bound twice in one pattern."),
    ("E0345", "Wrong number of sub-patterns for a variant or struct."),
    ("E0346", "Invalid literal or range pattern."),
    ("E0347", "A range loop binds a single variable: `for i in 0..n`."),
    ("E0350", "This value cannot be formatted.\n\nInterpolation and `print` accept integers, floats, `bool`, `char` and `String` in\nNEXUL 0.1. Format the fields of a structure individually."),
    ("E0351", "This format specification does not apply to the value's type.\n\nPrecision applies to floats and strings; `x`, `X`, `o`, `b` to integers; `e` to floats."),
    ("E0360", "The `?` operator is applied to a value that is not `Option` or `Result`, or the error\ntypes differ.\n\nIn NEXUL 0.1, the error type of `?` must be exactly the error type of the function."),
    ("E0361", "The `?` operator is used in a function that does not return `Option` or `Result`."),
    ("E0370", "Arithmetic overflow or division by zero while evaluating a constant."),
    ("E0371", "This expression cannot be evaluated at compile time.\n\nConstants may use literals, other constants, operators and `as`."),
    ("E0372", "A constant depends on itself."),
    ("E0373", "Invalid array length: it must be a constant integer between 0 and 4294967295."),
    ("E0374", "A static variable cannot hold a resource.

Static variables live for the whole program and are never destroyed, so their type must
be copyable (numbers, `bool`, `char`, pointers, and arrays, tuples and structures of
them). Use `const` for a constant string, or keep the resource in a local variable."),
    ("E0401", "A variable is read before being initialized.\n\nEvery path must assign a value before the variable is read."),
    ("E0402", "A value is used after being moved.\n\nTypes that own resources (`String`, `Vec`, structures containing them...) are moved,\nnot copied, when assigned or passed `owned`. After the move, the variable has no value.\n\n    let s = \"text\".clone()\n    let t = s          // `s` is moved into `t`\n    println(s)         // error: `s` has no value any more\n\nUse `s.clone()` to keep a copy, or pass the value as a borrowed parameter."),
    ("E0403", "A resource cannot be moved out of a field, an element or a borrowed value.\n\nOnly whole variables that own their value can be moved. Alternatives: copy the value\n(`.clone()`), take it out of an `Option` with `.take()`, or move the whole variable."),
    ("E0405", "An immutable place is modified.\n\nVariables declared with `let` are immutable: declare them with `var`. Parameters are\nborrowed read-only by default: declare them `inout` to modify the caller's value."),
    ("E0406", "`move` takes a whole variable that owns its value: `move v`."),
    ("E0407", "Only places (variables, fields, elements) can be assigned."),
    ("E0410", "An `inout` argument must be a modifiable place.\n\nPass a variable declared with `var` (or an `inout` parameter), with `&`: `f(&x)`."),
    ("E0411", "An argument passed to an `inout` parameter must be marked with `&`.\n\nThe `&` shows the reader that the function may modify the variable: `swap(&a, &b)`."),
    ("E0412", "`&` is only used to pass `inout` arguments.\n\nNEXUL has no reference values: parameters borrow their arguments automatically."),
    ("E0420", "Law of exclusivity: a value cannot be modified while it is accessed elsewhere.\n\nDuring a call, a value passed `inout` cannot also be passed as another argument.\nDuring a `for` loop, the iterated collection cannot be modified. During a `match` arm\nthat borrows parts of a value, the value cannot be modified.\n\n    for x in v {\n        v.push(x)      // error: `v` is being iterated\n    }"),
    ("E0430", "A destructor (`drop`) cannot be called explicitly.\n\nIt runs automatically when the value is destroyed. To destroy a value early, use\n`std.mem.drop(value)`."),
    ("E0431", "A value whose type defines `drop` cannot be taken apart by a pattern.\n\nIts destructor must run on the whole value."),
    ("E0432", "A destructor must be declared `fn drop(inout self)`."),
    ("E0501", "This operation requires an `unsafe` block.\n\nCalling external C functions, unsafe functions, and reading or writing through raw\npointers cannot be checked by the compiler. Wrap the operation in `unsafe { ... }` after\nverifying that it is correct."),
    ("E0502", "Unsupported ABI: NEXUL 0.1 only supports `extern \"C\"`."),
    ("E0503", "This type cannot cross the C boundary.\n\nAllowed: integers, floats, `bool`, `char`, `Ptr[T]` and `fn` types whose signature is\nitself allowed (and `()` as return type)."),
    ("E0504", "Invalid `@export` function.\n\nAn exported function is called from other languages with the C calling convention: it\nmust be a free function without generic parameters, and its symbol name may contain only\nletters, digits and `_`:\n\n    @export(\"nexul_add\")\n    pub fn add(a: i32, b: i32) -> i32 { return a + b }"),
    ("E0900", "The standard library is incomplete (internal error)."),
    ("W0001", "A variable or parameter is never read.\n\nIf this is intentional, start its name with `_`."),
    ("W0002", "A `match` arm can never be reached: earlier arms already match all its values."),
    ("W0003", "A `var` is never modified: declare it with `let`."),
    ("W0004", "An import is never used."),
    ("W0005", "A `Result` is ignored, which may hide an error.\n\nHandle it (`match`, `?`, `unwrap()`), or ignore it explicitly: `let _ = f()`."),
    ("W0006", "Code after `return`, `break`, `continue` or `panic` is never executed."),
    ("W0007", "An uppercase name in a pattern is a new variable, not a variant.\n\nIt matches every value. Check the spelling of the variant."),
    ("W0008", "A conversion with `as` converts a value to its own type."),
];

/// Every (code, explanation) pair, in order.
pub fn entries() -> &'static [(&'static str, &'static str)] {
    CODES
}

/// The Markdown catalogue of `docs/spec/diagnostics.md`.
pub fn catalogue_markdown() -> String {
    let sections: &[(&str, &str)] = &[
        ("E00", "Lexique (chapitre 01)"),
        ("E01", "Syntaxe (chapitre 02)"),
        ("E02", "Noms, modules et attributs (chapitres 04 et 06)"),
        ("E03", "Types, motifs et constantes (chapitres 03 et 04)"),
        ("E04", "Modèle mémoire (chapitre 05)"),
        ("E05", "Code non sûr et interopérabilité C (chapitre 08)"),
        ("E09", "Erreurs internes"),
        ("W00", "Avertissements"),
    ];
    let mut out = String::from(
        "# Catalogue des diagnostics\n\nChaque diagnostic du compilateur a un code stable. `nxc explain <code>` affiche son\nexplication détaillée (en anglais, comme les messages du compilateur en 0.1).\nCe fichier est généré à partir du compilateur et vérifié par la suite de tests.\n",
    );
    for (prefix, title) in sections {
        let rows: Vec<&(&str, &str)> = CODES.iter().filter(|(c, _)| c.starts_with(prefix)).collect();
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!("\n## {}\n\n| Code | Signification |\n|---|---|\n", title));
        for (code, text) in rows {
            let first = text.lines().next().unwrap_or("").replace('|', "\\|");
            out.push_str(&format!("| `{}` | {} |\n", code, first));
        }
    }
    out
}
