# Feuille de route et état d'avancement

Ce document suit le tableau de conformité du [cahier des charges](cahier-des-charges.md)
(§19.1). Légende : ✅ réalisé et testé · 🟡 partiel · ⏳ planifié.

Chaque ✅ est adossé à des tests de `tests/run`, `tests/ui` ou `tests/numerics`, exécutés
par `cargo test --release`.

## Tableau de conformité (§19.1)

### Compilateur

| Exigence | État | Preuve / remarque |
|---|---|---|
| Compilation native sans interprétation obligatoire | ✅ | NEXUL → NIR → IR LLVM → code machine, lié par LLD ; aucun interpréteur, aucun compilateur C |
| Compilation séparée et édition de liens | 🟡 | édition de liens avec des bibliothèques natives (`@link`), bibliothèques NEXUL statiques et partagées (`--lib`, `--staticlib`) ; cache des binaires ; compilation **séparée** des modules prévue en 0.3 |
| Diagnostics et débogage | ✅ | près de 130 codes documentés (`nxc explain`), diagnostics JSON pour les éditeurs, informations de débogage (PDB, DWARF), débogueur LLDB intégré à NEXUL Studio, `@track_caller` |
| Optimisations configurables | ✅ | `-O0` à `-O3`, `-Os`, `-Oz`, `--release` ; LTO et PGO prévus |

### Langage

| Exigence | État | Preuve / remarque |
|---|---|---|
| Typage statique et inférence | ✅ | inférence par unification dans les corps de fonction (spec §3.5) |
| Programmation procédurale, objet et fonctionnelle | ✅ | structures et méthodes, enums, filtrage par motif, traits, contraintes génériques, `dyn`, `@derive`, opérateurs sur les types utilisateur, fonctions anonymes non échappantes, `while let` |
| Gestion explicite de la mémoire | ✅ | sémantique de valeurs mutables, RAII, `unsafe`, `Ptr[T]`, allocation explicite, variables statiques, accès `volatile` |
| Modules et génériques | ✅ | modules = fichiers, `pub`, imports ; génériques monomorphisés avec contraintes de traits |

### Portabilité

| Exigence | État | Preuve / remarque |
|---|---|---|
| Plusieurs systèmes d'exploitation | ✅ | Windows et Linux vérifiés par la suite de conformité ; macOS non validé (couche système Linux sélectionnée à tort, voir audit Core) |
| Cibles x86-64 et ARM64 | ✅ | `x86_64-*`, `aarch64-linux`, `aarch64-windows`, `aarch64-macos`, `aarch64-none` |
| WebAssembly | ✅ | `wasm32-wasi` : la suite de conformité s'exécute sous Node ; `wasm32-web` : premier contrat navigateur v1, exports/imports et UTF-8 testés (voir docs/refonte/interop-web.md) |
| Toolchains embarquées compatibles | ✅ | runtime sans système (`sys/bare.nx`), vecteurs, démarrage et scripts de liens ; la suite de conformité s'exécute sur Cortex-M4 sous QEMU ; Cortex-M3, RISC-V et ARM64 sans système produits ; flottants logiciels exacts au bit près |

### Écosystème

| Exigence | État | Preuve / remarque |
|---|---|---|
| Bibliothèque standard | 🟡 | `Option`, `Result`, `String`, `Vec`, `Map`, `Set`, traits standards, `std.io`, `std.system`, `std.time` (dates), `std.math`, `std.random`, `std.mem` ; réseau, crypto, concurrence prévus |
| Gestionnaire de paquets | ✅ | `nxpm` : dépendances locales et Git, `nexul.lock` (commit exact, SHA-256), `nxpm verify` ; registre central prévu |
| Tests et documentation | ✅ | `nxc test`, spécification, guide, exemples de la documentation compilés automatiquement |
| Outils | ✅ | formateur `nxc fmt`, services d'éditeur `nxc ide`, NEXUL Studio (IDE officiel, débogueur, extensions), installateur Windows |
| Interopérabilité C et Python | 🟡 | appel de C (`extern "C"`), exposition à C (`@export`, bibliothèques) ; génération d'en-têtes C et interopérabilité Python prévues |

### Domaines

| Exigence | État | Remarque |
|---|---|---|
| Applications et serveurs | 🟡 | applications en ligne de commande complètes ; réseau et concurrence prévus |
| Web et réseau | 🟡 | WebAssembly (WASI), aperçu web dans Studio ; bibliothèques réseau prévues |
| Jeux et graphisme 3D | ⏳ | nécessite structures `@repr("C")` et rappels C |
| Sécurité et cryptographie | 🟡 | sûreté mémoire et arithmétique vérifiées, protection Trojan Source, aléa cryptographique du système (`std.random`) ; crypto prévue (sur implémentations auditées) |
| Microcontrôleurs et calcul spécialisé | ✅ | micrologiciels sans système, registres mémoire-mappés (`read_volatile`, `write_volatile`), gestionnaires d'interruption ; SIMD prévu |

## Versions

### 0.1 « Noyau » — réalisée

Spécification formelle complète du noyau ; compilateur `nxc` (lexer, parser, résolution,
inférence, exhaustivité, règles de places, NIR, analyse de propriété, élaboration des
destructions, monomorphisation) ; bibliothèque standard en NEXUL ; suite de conformité
avec détection de fuites.

### 0.2 « Autonomie et abstractions » — réalisée

- **Compilateur autonome** : backend LLVM (IR produite par `nxc`, LLVM chargé à
  l'exécution), édition de liens par LLD, bibliothèques d'importation Windows écrites par
  `nxc` ; plus de compilateur C ni de Zig.
- **Runtime écrit en NEXUL** : démarrage, allocateur, formatage et analyse exacts des
  flottants, bibliothèque mathématique (fdlibm), flottants logiciels, accès à Windows,
  Linux (appels système directs), WASI et au matériel nu.
- **Cibles** : Windows, Linux et macOS (x86-64, ARM64, RISC-V), WebAssembly, Cortex-M,
  RISC-V et ARM64 sans système.
- **Langage** : traits, contraintes génériques, `dyn`, `@derive`, opérateurs sur les types
  utilisateur, fonctions anonymes non échappantes, `while let`, variables statiques,
  `@cfg`, `@export`, bibliothèques statiques et partagées.
- **Bibliothèque** : `Map`, `Set`, méthodes fonctionnelles de `Vec`, dates, aléa.
- **Outils** : `nxc fmt`, `nxc ide`, `nxpm`, NEXUL Studio (édition assistée par le
  compilateur, débogueur, tests, paquets, extensions), installateur Windows qui gère les
  anciennes versions.

### 0.3 « Système »

- Compilation séparée des modules et cache de compilation incrémentale.
- Structures `@repr("C")`, rappels C, génération d'en-têtes C ; interopérabilité Python.
- Threads, canaux, atomiques et modèle mémoire de la concurrence (la loi d'exclusivité
  s'étend naturellement aux données partagées).
- Messages du compilateur en français (catalogue traduit).

### 0.4 « Réseau et outils »

- `std.net` (TCP, UDP, HTTP), `std.crypto` adossé à des implémentations auditées.
- Linter `nxlint`, générateur de documentation `nxdoc`, profileur.
- Serveur de langage (LSP) pour les autres éditeurs, à partir de `nxc ide`.
- Registre central de paquets avec signatures.

## Limites connues de la version 0.2

- Les références ne peuvent pas être stockées : les structures à références partagées
  (graphes) s'écrivent avec des indices ou du code `unsafe` encapsulé.
- Les fonctions anonymes ne peuvent pas être conservées (non échappantes) ; une fonction
  conservée dans une valeur est une fonction nommée.
- Pas de concurrence.
- Le programme est compilé en une seule unité (pas de compilation séparée).
- Les identificateurs sont ASCII ; les messages du compilateur sont en anglais.
