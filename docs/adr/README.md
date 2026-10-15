# Décisions d'architecture

Chaque décision structurante est consignée ici (cahier des charges §16.1 : « documentation
des décisions architecturales »). Format : contexte, décision, conséquences, alternatives.

| N° | Décision |
|---|---|
| [0001](0001-compilateur-en-rust.md) | Le compilateur est écrit en Rust, sans dépendance externe |
| [0002](0002-backend-c-et-zig.md) | Premier backend : C11 portable, compilé par Zig (remplacée par 0008) |
| [0003](0003-semantique-de-valeurs-mutables.md) | Modèle mémoire : sémantique de valeurs mutables |
| [0004](0004-crochets-pour-les-generiques.md) | Arguments génériques entre crochets |
| [0005](0005-fins-de-ligne-significatives.md) | Fins de ligne significatives, sans point-virgule |
| [0006](0006-debordements-toujours-verifies.md) | Débordements arithmétiques toujours vérifiés |
| [0007](0007-identificateurs-ascii.md) | Identificateurs ASCII et caractères bidirectionnels interdits |
| [0008](0008-backend-llvm-et-runtime-nexul.md) | Backend LLVM et runtime écrit en NEXUL, sans C |
| [0009](0009-contrat-nxc-lsp.md) | Contrat de `nxc lsp`, des sorties JSON versionnées et du texte non enregistré (acceptée par Claude Main) |
| [0010](0010-syntaxe-nexul-1.md) | Syntaxe de NEXUL 1, édition 1 (**proposée**, à valider par l'utilisateur) |

## Travaux préparatoires : nouvelle syntaxe (refonte V3, P2)

Études préalables à l'ADR de la nouvelle syntaxe ; elles ne décident rien.

| Étape | Document |
|---|---|
| G0 | [Audit des usages de la syntaxe 0.2](syntaxe/g0-audit.md) et son [recensement](syntaxe/g0-recensement.md) |
| G1 | [Trois propositions](syntaxe/g1-propositions.md) ; l'utilisateur a choisi un mélange → ADR 0010 |
