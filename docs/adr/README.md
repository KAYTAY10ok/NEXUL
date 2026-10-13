# Décisions d'architecture

Chaque décision structurante est consignée ici (cahier des charges §16.1 : « documentation
des décisions architecturales »). Format : contexte, décision, conséquences, alternatives.

| N° | Décision |
|---|---|
| [0001](0001-compilateur-en-rust.md) | Le compilateur est écrit en Rust, sans dépendance externe |
| [0002](0002-backend-c-et-zig.md) | Premier backend : C11 portable, compilé par Zig |
| [0003](0003-semantique-de-valeurs-mutables.md) | Modèle mémoire : sémantique de valeurs mutables |
| [0004](0004-crochets-pour-les-generiques.md) | Arguments génériques entre crochets |
| [0005](0005-fins-de-ligne-significatives.md) | Fins de ligne significatives, sans point-virgule |
| [0006](0006-debordements-toujours-verifies.md) | Débordements arithmétiques toujours vérifiés |
| [0007](0007-identificateurs-ascii.md) | Identificateurs ASCII et caractères bidirectionnels interdits |
