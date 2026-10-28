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
| Sécurité et cryptographie | 🟡 | sûreté mémoire et arithmétique vérifiées dans le périmètre et sous les conditions de l'[ADR 0011](adr/0011-perimetre-des-garanties.md), protection Trojan Source, aléa cryptographique du système (`std.random`) ; crypto prévue (sur implémentations auditées) |
| Microcontrôleurs et calcul spécialisé | 🟡 | micrologiciels sans système, registres mémoire-mappés (`read_volatile`, `write_volatile`), gestionnaires d'interruption ; **limites** : dépassement de pile non détecté, runtime non réentrant dans les interruptions (ADR 0011, C3 et C4) ; SIMD prévu |

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

### Trajectoire vers la 1.0.0 (proposition du 17/10/2026)

**Validée par l'utilisateur le 28/10/2026** : la 1.0.0 est la version majeure complète ; la 0.3.0 sort d'abord. État au 28/10, critères de sortie de la **0.4.0** :

| Critère | État |
|---|---|
| conditions C3–C4 de l'ADR 0011 levées (T1–T5), suite en `-O2` (T6) | fait : sondes, dépassement de pile (threads compris), pile WASM, `@export`, runtime réentrant ; `run_programs_optimized` |
| ADR 0012, 0014, 0015 | intégrées |
| matrice des plateformes publiée | [plateformes.md](plateformes.md) |
| macOS validé ou exclu | **validé** sur Apple silicon (CI, O0 et O2) ; Intel compilé seulement |
| `lib/` migrée | **fait** : toute la bibliothèque standard en édition 1 |
| édition 1 complète hors formes réservées | Q1, Q2, boucle typée et types à plusieurs interfaces faits (28/10) ; restent la contrainte sur le paramètre d'un type et les types distincts génériques (ADR 0010, fin) |
| `tests/ui` couvre chaque nouvelle règle | chaque code a son test (K7.1) |

Critères de sortie de la **0.5.0** (28/10) : corpus négatif (`tests/ui/task_races.nx`, 9 courses
refusées) et positif ; 10⁶ messages entre 64 tâches, résultat déterministe
(`tests/stress/channels_1m.nx`, en CI sur Linux, Windows et macOS) ; scénario C **passé**
([fiche](scenarios/C-concurrence.md)) ; ThreadSanitizer **non testé** (les threads de NEXUL
ne passent pas par la libc ; raison dans la fiche).

État de la **0.6.0** (28/10, en cours) : `std.fs` et `std.process` faits, testés par
`tests/run/fs.nx` (Linux x86-64, AArch64 et RISC-V, WASI ; Windows et macOS par la CI) et
`tests/run/process.nx` (Linux, Windows, macOS) ; `std.net` (TCP sur IPv4) fait, et le
critère « serveur TCP concurrent d'exemple testé » rempli sur Linux (trois architectures)
par `tests/run/tcp.nx` (un serveur, une tâche par connexion, 8 clients simultanés) ;
Windows et macOS : à confirmer par la CI. Restent la compilation séparée et le cache incrémental,
`@repr("C")` et les rappels C, les vues de chaînes, UDP, et deux critères de sortie (temps
de compilation incrémentale, appels C ↔ NEXUL).

Proposée par Claude (architecture) à partir de l'[audit du 17/10](refonte/audit-claude-2026-10-17.md)
et du [plan maître](refonte/master-plan.md) ; Codex la complète pour les outils, les
estimations et la validation opérationnelle ; l'utilisateur la valide. Elle remplace les
anciennes sections « 0.3 Système » et « 0.4 Réseau et outils », qui ne correspondaient
plus au plan maître (la 0.3.0 est la version de la refonte).

**Principe d'ordre.** Chaque version fait ce dont la suivante dépend, et rien de plus :
d'abord les fondations qui coûtent peu maintenant et beaucoup plus tard (garanties, `Copy`,
erreurs, types), puis la concurrence (qui change le modèle mémoire), puis le système et
l'interopérabilité (dont ont besoin les serveurs et l'auto-hébergement), puis le temps
réel (qui dépend de la concurrence et des effets), puis l'auto-hébergement (qui exige un
langage figé). Une version ne sort que si **tous** ses critères sont prouvés ; une
capacité non prête est reportée et le dit, plutôt que livrée partiellement.

| Version | Objectif | Contenu | Critères de sortie (preuves) | Dépend de |
|---|---|---|---|---|
| **0.3.0** « Refonte » | Première version de la refonte | édition 1 (ADR 0010, tranches 1–4), `nxc lsp`, Studio natif, extension VS Code, installateur 0.3.0 ; corrections de sûreté S1, S2 (PR #35) | plan maître §0.3.0 ; CI verte (Windows, Linux) ; scénario A en partie | — |
| **0.4.0** « Fondations sûres » | Fermer les écarts entre garanties annoncées et réelles ; figer ce qui serait incompatible plus tard | conditions C3–C4 de l'ADR 0011 levées sur les cibles hébergées et WebAssembly (tâches T1–T5), suite `tests/run` aussi en `-O2` (T6) ; ADR 0012 (`Copy`), ADR 0014 (types distincts et contraints), ADR 0015 (erreurs, `IoError`, conversion pour `?`) ; édition 1 complète hors formes réservées ; `lib/` migrée ; macOS validé **ou** exclu explicitement | chaque condition levée a son test ; `tests/ui` couvre chaque nouvelle règle ; matrice des plateformes publiée (cible, système, testé où, par quoi) | 0.3.0 ; décisions sur ADR 0011–0015 |
| **0.5.0** « Concurrence » | Concurrence sans course de données dans le code sûr | ADR 0013 : tâches liées à une portée, `Transfer`/`Share`, tâches détachées par transfert, canaux bornés, verrous, atomiques ; threads du système sur Windows et Linux | corpus négatif (courses refusées) et positif ; stress (10⁶ messages, 64 tâches) déterministe ; ThreadSanitizer sur Linux ou « non testé » écrit ; scénario C | 0.4.0 (`Copy`, `Share` reposent sur les mêmes mécanismes) |
| **0.6.0** « Système et interopérabilité » | Écrire des serveurs et des outils système ; préparer l'auto-hébergement | compilation séparée des modules et cache incrémental ; `@repr("C")`, rappels C, en-têtes C ; `std.fs` (dossiers, chemins, métadonnées), `std.process` ; `std.net` (TCP, UDP) ; vues de chaînes | serveur TCP concurrent d'exemple testé ; temps de compilation incrémentale mesuré ; programme C qui appelle NEXUL et inversement, testé sur deux systèmes | 0.5.0 (serveurs), SH1 |
| **0.7.0** « Temps réel » | Profil temps réel v1 sur **une** cible : Cortex-M4 sans système (décision 3, 28/10/2026) | `with noalloc, nonblocking`, tâches périodiques `every … within …`, analyse de pile, runtime réentrant (C4 levée sur cette cible) | budgets de latence mesurés et publiés avec leur méthode ; aucune allocation vérifiée par le compilateur ; scénario D. **Aucune** garantie de temps réel strict hors de la cible validée | 0.5.0, étude P8-G0 |
| **0.8.0** « Auto-hébergement » | Compilateur écrit en NEXUL | SH2–SH6 ([selfhost-sh0.md](adr/selfhost-sh0.md)) | critères A1–A5 (point fixe stage1 = stage2, construction sans Rust) ; scénario E | 0.6.0 ; grammaire de l'édition 1 gelée |
| **0.9.x** « Candidate » | Stabiliser | gel de l'API de `lib/std` (attributs de stabilité), politique de compatibilité (éditions, versions de `std`, paquets) écrite ; campagne de fuzzing (`nxc` ne plante jamais) ; références de performance (P13) ; documentation complète, guide de migration ; seulement des corrections | aucune régression sur deux candidates successives ; scénarios A–H passés ; chaque garantie du README renvoie à sa preuve | toutes |
| **1.0.0** | Version méritée | — | critères de la mission maîtresse §9, vérifiés un par un et consignés | 0.9.x |

**Reporté après la 1.0 (explicitement)** : interopérabilité Python, applications mobiles
et interfaces graphiques natives, tâches légères `async` (sauf mesures qui les
justifient avant), déroulement des paniques et isolation par tâche (ADR 0011 §4),
références stockées ou projections, registre central de paquets, LTO/PGO, SIMD portable.

**Parallélisme possible** : P8-G0 (étude de la cible temps réel) et SH1 peuvent avancer
pendant 0.5.0 ; les outils (formateur, LSP, Studio, VS Code) avancent à chaque version
sans bloquer le langage.

## Limites connues de la version 0.2

- Les références ne peuvent pas être stockées : les structures à références partagées
  (graphes) s'écrivent avec des indices ou du code `unsafe` encapsulé.
- Les fonctions anonymes ne peuvent pas être conservées (non échappantes) ; une fonction
  conservée dans une valeur est une fonction nommée.
- Pas de concurrence (hors interruptions sur les cibles sans système, voir ADR 0011, condition C4).
- Le programme est compilé en une seule unité (pas de compilation séparée).
- Les identificateurs sont ASCII ; les messages du compilateur sont en anglais.
