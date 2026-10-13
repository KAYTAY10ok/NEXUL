# Feuille de route et état d'avancement

Ce document suit le tableau de conformité du [cahier des charges](cahier-des-charges.md)
(§19.1). Légende : ✅ réalisé et testé dans la version 0.1 · 🟡 partiel · ⏳ planifié.

Chaque ✅ est adossé à des tests de `tests/run` ou `tests/ui`, exécutés par `cargo test`.

## Tableau de conformité (§19.1)

### Compilateur

| Exigence | État | Preuve / remarque |
|---|---|---|
| Compilation native sans interprétation obligatoire | ✅ | NEXUL → NIR → C11 → code machine ; aucun interpréteur |
| Compilation séparée et édition de liens | 🟡 | édition de liens et bibliothèques C (`@link`) ; compilation **séparée** des modules prévue en 0.3 (le programme est aujourd'hui compilé en une unité) |
| Diagnostics et débogage | ✅ | plus de 100 codes documentés (`nxc explain`), `#line` vers le source NEXUL, symboles `-g` en debug, `@track_caller` pour localiser les paniques |
| Optimisations configurables | 🟡 | `--release` (`-O2`) ; contrôle fin (taille/vitesse, LTO, PGO) prévu |

### Langage

| Exigence | État | Preuve / remarque |
|---|---|---|
| Typage statique et inférence | ✅ | inférence par unification dans les corps de fonction (spec §3.5) |
| Programmation procédurale, objet et fonctionnelle | 🟡 | procédural, structures + méthodes, enums, filtrage par motif, pointeurs de fonction ; **traits/interfaces et closures** prévus en 0.2 |
| Gestion explicite de la mémoire | ✅ | sémantique de valeurs mutables, RAII, `unsafe`, `Ptr[T]`, allocation explicite (`std.mem`) |
| Modules et génériques | ✅ | modules = fichiers, `pub`, imports ; génériques monomorphisés (sans contraintes en 0.1) |

### Portabilité

| Exigence | État | Preuve / remarque |
|---|---|---|
| Plusieurs systèmes d'exploitation | ✅ | exécutables produits pour Windows, Linux et macOS (`--target`) |
| Cibles x86-64 et ARM64 | ✅ | `x86_64-*`, `aarch64-linux-gnu`, `aarch64-macos` vérifiés |
| WebAssembly | 🟡 | `--target wasm32-wasi` produit un module ; l'intégration navigateur/JavaScript est prévue |
| Toolchains embarquées compatibles | ⏳ | RISC-V Linux vérifié ; runtime « sans système » (pas de libc, gestionnaire de panique) prévu en 0.4 |

### Écosystème

| Exigence | État | Preuve / remarque |
|---|---|---|
| Bibliothèque standard | 🟡 | `Option`, `Result`, `String`, `Vec`, `std.io`, `std.system`, `std.time`, `std.math`, `std.mem` ; `Map`/`Set`, réseau, crypto prévus |
| Gestionnaire de paquets | ⏳ | `nxpm` prévu en 0.5 |
| Tests et documentation | ✅ | `nxc test`, spécification, guide, exemples compilés automatiquement |
| Interopérabilité C et Python | 🟡 | appel de fonctions C (`extern "C"`) ; exposition à C et Python prévues |

### Domaines

| Exigence | État | Remarque |
|---|---|---|
| Applications et serveurs | 🟡 | applications en ligne de commande ; réseau et concurrence prévus |
| Web et réseau | ⏳ | WebAssembly de base ; bibliothèques réseau prévues |
| Jeux et graphisme 3D | ⏳ | nécessite l'interopérabilité C étendue (structures `@repr("C")`, rappels) |
| Sécurité et cryptographie | 🟡 | sûreté mémoire et arithmétique vérifiées, protection Trojan Source ; crypto prévue (sur bibliothèques auditées) |
| Microcontrôleurs et calcul spécialisé | ⏳ | runtime minimal sans libc, accès MMIO `volatile`, SIMD prévus |

## Versions

### 0.1 « Noyau » — réalisée

Spécification formelle complète du noyau ; compilateur `nxc` (lexer, parser, résolution,
inférence, exhaustivité, règles de places, NIR, analyse de propriété, élaboration des
destructions, monomorphisation, génération de C) ; bibliothèque standard en NEXUL ;
suite de conformité avec détection de fuites ; compilation croisée via Zig.

### 0.2 « Abstractions »

- Traits et contraintes génériques (`[T: Ord]`), avec les traits standards `Eq`, `Ord`,
  `Clone`, `Display`, `Hash`, `Default` ; `==`, `<` et `{x}` sur les types utilisateur.
- Closures non échappantes (sûres sans durée de vie) et fonctions d'ordre supérieur
  (`v.map(fn(x) { ... })`).
- Collections `Map[K, V]` et `Set[T]` ; vues de chaînes et de tableaux.
- Liaisons modifiables dans les motifs `let` ; `while let`.
- Messages du compilateur en français (catalogue traduit).

### 0.3 « Système »

- Compilation séparée des modules et cache de compilation incrémentale.
- Structures `@repr("C")`, exposition de fonctions NEXUL à C (`@export`), mode bibliothèque
  (statique et partagée), génération de bindings.
- Threads, canaux, atomiques et modèle mémoire de la concurrence (la loi d'exclusivité
  s'étend naturellement aux données partagées).

### 0.4 « Matériel »

- Runtime autonome sans libc ni système d'exploitation, gestionnaire de panique
  configurable, scripts de liens ; cibles ARM Cortex-M et RISC-V embarquées.
- Accès `volatile` aux registres mémoire-mappés, interruptions, SIMD.
- Backend LLVM IR direct en complément du backend C.

### 0.5 « Écosystème »

- Gestionnaire de paquets `nxpm` (versions, verrouillage, signatures, audit).
- Formateur `nxfmt`, linter `nxlint`, serveur de langage, débogueur et profileur.
- Interopérabilité Python.

## Limites connues de la version 0.1

- Pas de traits : les fonctions génériques ne peuvent pas comparer ou additionner leurs
  paramètres de type.
- Pas de closures ; les fonctions passées en valeur doivent être non génériques.
- Les références ne peuvent pas être stockées : les structures à références partagées
  (graphes) s'écrivent avec des indices ou du code `unsafe` encapsulé.
- Pas de concurrence.
- Le programme est compilé en une seule unité C (pas de compilation séparée).
- Les identificateurs sont ASCII.
