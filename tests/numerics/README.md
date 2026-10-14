# Validation numérique

Ces programmes comparent l'arithmétique flottante écrite en NEXUL (`std.fmt`, `std.math`)
à une référence indépendante : Python (formatage `repr`/`format` exact, `float()` correctement
arrondi, et la `libm` du système pour les fonctions mathématiques).

| Programme | Vérifie | Critère |
|---|---|---|
| `format.nx` + `format_verify.py` | forme la plus courte, `{x:.3}`, `{x:.0}`, `{x:e}`, `{x:.4e}` sur 10 000 valeurs aléatoires (tous exposants) | identité exacte |
| `parse.nx` + `parse_gen.py` | `parse_float` sur 15 000 chaînes difficiles (cas à mi-chemin, sous-normaux, 900 chiffres) | identité au bit près |
| `math.nx` + `math_verify.py` | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `ln`, `log2`, `log10`, `powf`, `hypot`, `floor`, `ceil`, `round`, `%` sur 40 000 arguments (jusqu'à 10²²) | écart ≤ 1 ulp |

Exécution (Python 3 requis) : `NEXUL_TEST_NUMERICS=1 cargo test --release numerics`.
