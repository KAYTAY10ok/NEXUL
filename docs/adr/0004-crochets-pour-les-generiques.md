# 0004 — Arguments génériques entre crochets

**Contexte.** Avec des chevrons, `a < b > (c)` est ambigu entre une comparaison et un
appel générique ; les langages qui les utilisent ajoutent des règles de désambiguïsation
(`::<>` en Rust, heuristiques en C#).

**Décision.** Les arguments génériques s'écrivent entre crochets : `Vec[i32]`,
`Result[T, E]`, `size_of[u64]()`. Les symboles `<` et `>` sont uniquement des comparaisons.

**Conséquences.** La grammaire reste analysable sans information sémantique. Dans une
expression, `Vec[i32]` est d'abord lu comme une indexation, puis interprété selon ce que
désigne `Vec`.

**Alternatives.** Chevrons (ambiguïtés), turbofish `::<>` (lourd à lire).
