# Historique des versions

## Non publié (vers 0.4.0, 0.5.0 et 0.6.0)

Travail fait après la préparation de la 0.3.0, sur la même branche d'intégration ; les
critères de sortie de chaque version sont suivis dans la [feuille de route](docs/roadmap.md).

**Langage (édition 1, ADR 0010)**
- Blocs `extend Type[P is …]` : les méthodes d'un type générique contraintes sur un
  paramètre du type (Q1) ; mots réservés permis comme noms de membres (Q2) ; type sur la
  variable d'une boucle (`for T x in e`) ; fonctions dans un type qui a plusieurs
  interfaces. Toute la bibliothèque standard est écrite en édition 1 ; `nxc migrate`
  déplace les contraintes vers un bloc `extend` et garde les commentaires des variantes.

**Bibliothèque**
- `std.fs` (spec §9.3) : métadonnées, dossiers (lister, créer, supprimer, récursivement),
  renommer, copier, chemins (`join`, `parent`, `file_name`, `extension`) ; Linux
  (appels système directs), macOS (`libSystem`), Windows (`kernel32`), WASI.
- `std.process` : `run` (sorties capturées) et `status` (sorties partagées), recherche
  dans `PATH`, sans shell ; Linux, macOS, Windows.
- Canaux de `std.tache` : files d'attente par thread et réveil d'un seul thread ;
  `String.push` 1,75 fois plus rapide.

**Tests** : corpus de courses (`tests/ui/task_races.nx`), 10⁶ messages entre 64 tâches
sur Linux, Windows et macOS (`NEXUL_STRESS=1`), scénario C.

**Limites connues** : `std.process` hérite d'une course connue — un autre thread qui lance
un programme au même moment peut hériter du tube de capture (Windows, macOS) ; macOS
x86-64 (structures `stat` et `dirent` anciennes) compilé mais non testé ; WASI ne lance
pas de programmes.

## 0.3.0 — « Refonte »

Première version de la refonte. Le langage, les garanties et leurs limites sont décrits
par les ADR 0010 à 0015 et la [feuille de route](docs/roadmap.md) ; ce qui n'est pas
prouvé est écrit « non testé ».

**Langage**
- Édition 1 de la syntaxe (ADR 0010) : chaque fichier ou paquet choisit son édition
  (`// edition: 1`, `edition = "1"` dans `nexul.toml`) ; l'édition 0.2 reste acceptée ;
  `nxc migrate` réécrit un fichier 0.2 en édition 1 (il signale désormais les contraintes
  qu'il ne sait pas écrire au lieu de les perdre) ; 20 des 24 fichiers de la bibliothèque
  standard sont écrits en édition 1.
- Copiabilité explicite des types publics, `Copy` (ADR 0012).
- Types distincts et types contraints (`type Pourcent = i32 where it in 0..=100`),
  vérifiés à la construction et à l'entrée des `@export` (ADR 0014).
- Erreurs de la bibliothèque : `io.IoError`, l'interface `Error`, conversions par `?`
  (ADR 0015) ; `?` prend son opérande (plus de déplacement hors d'une place empruntée).
- Concurrence (ADR 0013) : `std.tache` — tâches liées à une portée (`tache.portee`,
  `p.lancer`), propriétés `Transfer` et `Share` vérifiées par le compilateur, tâches
  détachées (`lancer_detachee`), canaux bornés, `Verrou`, `Atomique` ; threads du système
  sur Linux et Windows, tâches exécutées l'une après l'autre ailleurs.

**Sûreté (ADR 0011)**
- Un dépassement de pile se termine comme une panique (code 101) sur Linux (x86-64,
  AArch64, RISC-V) et Windows, y compris dans un thread ; sondes de pile sur Linux et
  macOS (x86-64, AArch64).
- WebAssembly : la pile est placée avant les données statiques (`--stack-first`).
- Les valeurs `bool`, `char` et contraintes reçues de C par une fonction `@export` sont
  vérifiées.
- Runtime réentrant sur les cibles sans système (sections critiques autour de
  l'allocateur et des sorties).
- Une garde de `match` ne peut plus déplacer une variable liée (E0403) ; largeur de
  `isize`/`usize` vérifiée pour la cible.

**Plateformes** ([matrice](docs/plateformes.md))
- macOS : vraie couche système sur `libSystem` (entrées-sorties, fichiers, temps, threads
  POSIX, dépassement de pile) ; la suite de conformité passe sur Apple silicon dans la CI.
- La CI exécute aussi la suite sur AArch64 et RISC-V Linux (QEMU), WASI (Node) et
  Cortex-M4 (QEMU).

**Compilateur**
- `nxc` ne plante plus sur les sources pathologiques (test de robustesse, limite
  d'imbrication E0113) ; analyse des très grandes fonctions en secondes ; chaque code
  de diagnostic a son test.
- `nxc lsp` (ADR 0009), `nxc version --json`, texte non enregistré (`--stdin`), JSON
  versionné.

**Outils**
- NEXUL Studio natif (GPUI) : aucune perte de données, coloration des deux éditions,
  nouveau projet, recherche, exécuter/construire/tester ; terminal ConPTY et débogueur
  `lldb-dap` corrigés.
- Extension VS Code 0.3.0 : édition 1, LSP, formatage à l'enregistrement.
- `NEXUL Setup.exe` 0.3.0 : Studio natif, migration des réglages de la 0.2, versions des
  composants vérifiées avant la construction.
- CI : suite complète du compilateur sous Linux et Windows (LLVM 20.1.8), crates de
  Studio, ABI navigateur, PR empilées.

**Limites connues** : macOS Intel et Windows sur ARM compilés mais non exécutés ; en
édition 1, pas encore de méthode contrainte sur un paramètre de son type (`vec.nx`,
`string.nx`, `collections.nx`, `tache.nx` restent en 0.2) ; pas encore
d'`Atomique[bool]` ni d'ordres mémoire faibles.

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
