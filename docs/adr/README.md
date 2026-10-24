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
| [0010](0010-syntaxe-nexul-1.md) | Syntaxe de NEXUL 1, édition 1 (acceptée par l'utilisateur le 16/10/2026) |
| [0011](0011-perimetre-des-garanties.md) | Périmètre des garanties de sûreté et modèle d'échec (acceptée le 24/10/2026) |
| [0012](0012-copiabilite-explicite.md) | Copiabilité explicite des types publics, `Copy` (acceptée le 24/10/2026) |
| [0013](0013-fondations-de-la-concurrence.md) | Fondations de la concurrence : invariants, concurrence structurée, `Transfer`/`Share` (acceptée le 24/10/2026) |
| [0014](0014-types-distincts-et-contraints.md) | Types distincts, types contraints et orientation des contrats (acceptée le 24/10/2026) |
| [0015](0015-erreurs-et-conversion.md) | Erreurs de la bibliothèque standard (`IoError`) et conversion par `?` (acceptée le 24/10/2026) |

## Travaux préparatoires : nouvelle syntaxe (refonte V3, P2)

Études préalables à l'ADR de la nouvelle syntaxe ; elles ne décident rien.

| Étape | Document |
|---|---|
| G0 | [Audit des usages de la syntaxe 0.2](syntaxe/g0-audit.md) et son [recensement](syntaxe/g0-recensement.md) |
| G1 | [Trois propositions](syntaxe/g1-propositions.md) ; l'utilisateur a choisi un mélange → ADR 0010 |

## Études

| Lot | Document |
|---|---|
| P12 — SH0 | [Auto-hébergement : inventaire et trajectoire](selfhost-sh0.md) |
| Contrats | [Contrats entre les composants du compilateur](../spec/contrats-composants.md) (proposés) |
