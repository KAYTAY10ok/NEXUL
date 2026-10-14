# Historique des versions

## 0.2.0 — « Autonomie et abstractions »

**Compilateur autonome**
- Backend LLVM : `nxc` produit l'IR LLVM, la fait optimiser et compiler par la
  bibliothèque LLVM chargée à l'exécution, et lie avec LLD. Le backend C, le runtime C et
  la dépendance à Zig sont supprimés.
- Runtime écrit en NEXUL : démarrage, allocateur, formatage et analyse exacts des
  flottants, mathématiques (fdlibm), flottants logiciels, accès direct à Windows, Linux,
  WASI et au matériel nu.
- Cibles : Windows, Linux, macOS (x86-64, ARM64, RISC-V), WebAssembly WASI, Cortex-M3/M4,
  RISC-V 32/64 et ARM64 sans système d'exploitation (`nxc targets`).
- Bibliothèques partagées et statiques (`--lib`, `--staticlib`), `@export`, niveaux
  d'optimisation `-O0` à `-Oz`, `--emit llvm-ir`, informations de débogage PDB/DWARF,
  cache des binaires.

**Langage**
- Traits, méthodes par défaut, super-traits, contraintes génériques (`[T: Ord]`,
  `where`), implémentations conditionnelles, répartition dynamique `dyn Trait`.
- Traits standards `Eq`, `Ord`, `Clone`, `Display`, `Hash`, `Default`, opérateurs
  (`Add`… `Neg`) sur les types utilisateur, `@derive`.
- Fonctions anonymes non échappantes, `while let`.
- Variables statiques, `@cfg`, accès `volatile`, gestionnaires d'interruption.

**Bibliothèque**
- `std.collections` (`Map`, `Set`), méthodes fonctionnelles de `Vec` (`map`, `filter`,
  `fold`, `sort_by`…), `std.time` (dates du calendrier), `std.random`.

**Outils**
- `nxc fmt` (formateur), `nxc ide` (survol, complétion, définition, références,
  renommage, plan, tests), `nxc test` avec filtre.
- `nxpm` : projets `nexul.toml`, dépendances locales et Git, `nexul.lock` (commit exact et
  SHA-256), `nxpm verify`.
- NEXUL Studio : édition assistée par le compilateur, débogueur LLDB, vue des tests,
  paquets, extensions isolées avec permissions.
- `NEXUL Setup.exe` : installation complète pour Windows, sans droits d'administrateur,
  avec détection des anciennes versions (suppression ou sauvegarde au choix), installation
  sans fenêtre et désinstallation.

## 0.1.0 — « Noyau »

Spécification formelle du noyau du langage ; compilateur `nxc` (lexer, parser,
résolution, inférence, exhaustivité, règles de places, NIR, analyse de propriété,
destructions, monomorphisation, génération de C) ; bibliothèque standard en NEXUL ;
suite de conformité avec détection de fuites.
