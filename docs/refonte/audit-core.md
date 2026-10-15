# Audit initial NEXUL Core

Date : 15 octobre 2026. Base : d537a7b. Branche : refonte/codex-core.
Cet audit porte sur le code lu et les tests exécutés, pas sur une certification de toutes les cibles.

## Architecture réutilisable

Le pipeline syntaxe → sémantique/THIR → NIR (ownership et drops) → LLVM IR → objet → LLD est déjà implémenté. La bibliothèque standard et les fonctions mathématiques sont écrites en NEXUL. Conserver ce pipeline.

`compiler/nxc/src/llvm.rs` charge LLVM par son API C, vérifie le module avant optimisation, puis applique les pipelines default<O0/O1/O2/O3/Os/Oz>. Les opérations flottantes ordinaires du backend ne portent pas de drapeaux fast-math. Le profil Cargo thin LTO optimise le compilateur Rust ; il ne constitue pas une fonctionnalité LTO des programmes NEXUL.

`compiler/nxc/src/link.rs` sélectionne les linkers natifs ou wasm-ld. La cible WASI dispose d'un runtime système et d'un lanceur Node. Les imports Wasm peuvent porter les attributs wasm-import-module et wasm-import-name. Les exports des bibliothèques sont transmis explicitement au linker.

## Défauts confirmés

- `compiler/nxc_sema/src/load.rs` sélectionne SYS_LINUX pour Os::MacOs. Un linker Mach-O ne rend pas les appels système Linux compatibles avec macOS. Ne pas annoncer cette cible comme validée ; il faut une couche système dédiée et des essais sur macOS.
- Les vérificateurs numériques acceptaient une sortie vide (format et math), ignoraient les lignes de format invalides et les colonnes mathématiques manquantes via zip. Le comparateur de parsing ignorait les lignes supplémentaires. Correction dans ce lot avec tests négatifs.
- Le test Rust numérique ne vérifiait pas le statut des exécutables avant d'analyser stdout. Correction dans ce lot.

## Limites des preuves

La comparaison mathématique utilise la libm de Python, pas un oracle multiprécision certifié. Une erreur ≤1 ULP par rapport à cette référence ne prouve pas un arrondi correct universel. Le corpus mathématique couvre 40 000 arguments déterministes ; compléter avec NaN, infinis, zéros signés, sous-normaux et frontières f32/f64.

Aucune comparaison de vitesse C/Rust/C#/NEXUL ni inspection de code machine n'est encore réalisée dans cette branche. Aucun gain de performance annoncé. Les tests croisés sont optionnels : un test run_programs_cross réussi sans NEXUL_TEST_TARGETS ne valide aucune cible supplémentaire.

## Plan incrémental

1. Fiabiliser les oracles et enregistrer une baseline native.
2. Ajouter un harness de benchmarks avec paramètres 1M/10M/100M, mêmes algorithmes, entrées runtime et checksum observable. Séparer compilation froide/cache, calcul, mémoire et exactitude ; publier versions, commandes, répétitions, médiane, dispersion et outils absents.
3. Examiner IR optimisé et code machine avant de changer l'inlining, la vectorisation ou les allocations. Maintenir les vérifications de sûreté sauf preuve de redondance.
4. Prototyper le contrat navigateur : imports hôte explicites, fonctions scalaires, mémoire et chaînes UTF-8, tests Node et navigateur. Aucun changement d'interface Studio.
5. Étudier LTO/PGO et un mode numérique rapide uniquement après mesures. Le mode actuel reste inchangé.

## Coordination

La demande Claude pour nxc lsp et le versionnement JSON est enregistrée dans son rapport. Elle reste à traiter ; nxc ide est l'interface existante. Le choix GUI et l'installateur appartiennent à Claude.
