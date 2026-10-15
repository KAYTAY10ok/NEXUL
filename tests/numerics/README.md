# Validation numÃ©rique

Ces programmes comparent l'arithmÃ©tique flottante Ã©crite en NEXUL (`std.fmt`, `std.math`)
Ã  une rÃ©fÃ©rence indÃ©pendante : Python (formatage `repr`/`format` exact, `float()` correctement
arrondi, et la `libm` du systÃ¨me pour les fonctions mathÃ©matiques).

| Programme | VÃ©rifie | CritÃ¨re |
|---|---|---|
| `format.nx` + `format_verify.py` | forme la plus courte, `{x:.3}`, `{x:.0}`, `{x:e}`, `{x:.4e}` sur 10 000 valeurs alÃ©atoires (tous exposants) | identitÃ© exacte |
| `parse.nx` + `parse_gen.py` | `parse_float` sur 15 000 chaÃ®nes difficiles (cas Ã  mi-chemin, sous-normaux, 900 chiffres) | identitÃ© au bit prÃ¨s |
| `math.nx` + `math_verify.py` | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`, `ln`, `log2`, `log10`, `powf`, `hypot`, `floor`, `ceil`, `round`, `%` sur 40 000 arguments (jusqu'Ã  10Â²Â²) | Ã©cart â‰¤ 1 ulp |

ExÃ©cution (Python 3 requis) : `NEXUL_TEST_NUMERICS=1 cargo test --release numerics`.

Les vérificateurs eux-mêmes disposent de tests négatifs :
`python -I tests/numerics/test_verifiers.py`. Ils refusent les sorties vides,
les lignes mal formées et les comptes de parsing incohérents. Le corpus mathématique
exige exactement 40 000 lignes. Le test Rust vérifie aussi le statut de chaque programme.
La libm Python sert de référence comparative, pas de preuve d'arrondi correct universel.
