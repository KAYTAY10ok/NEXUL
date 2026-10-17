# 0011 — Périmètre des garanties de sûreté et modèle d'échec

Auteur : Claude (architecture, sémantique, sûreté). **Statut : proposée** (17/10/2026).
Les parties 1 à 3 décrivent l'état réel et corrigent la spécification : elles peuvent
être acceptées sans attendre. Les parties 4 et 5 engagent la version 1.0 et demandent la
validation de l'utilisateur.

## Problème

Le README et la spécification (§5.8 jusqu'au 17/10) annonçaient : « pour un programme sans
`unsafe`, les seules erreurs possibles à l'exécution sont des paniques déterministes ».
L'audit du 17/10 ([audit-claude-2026-10-17.md](../refonte/audit-claude-2026-10-17.md))
montre que cette phrase est fausse dans plusieurs cas, et qu'elle ne dit pas de quoi la
garantie dépend. La mission maîtresse interdit de qualifier NEXUL de « mémoire-sûr » sans
préciser le périmètre et les conditions.

## Contexte

- Le modèle (ADR 0003, spec ch. 05) est vérifié localement par `nxc_sema/src/places.rs`
  (E0403, E0405, E0410, E0420) et `nxc_nir/src/ownership.rs` (E0401, E0402, drapeaux de
  destruction).
- Défauts démontrés le 17/10, corrigés dans la branche `refonte/core-surete-semantique` :
  déplacement d'une liaison dans une garde (utilisation après libération dans du code sûr) ;
  largeur de `isize`/`usize` ignorée par l'analyse sur les cibles 32 bits (troncature
  silencieuse de constantes).
- Défauts relevés à la lecture, non corrigés : dépassement de pile non détecté sur les
  cibles sans système ; pile de WebAssembly placée après les données ; gestionnaires
  d'interruption sûrs alors que le runtime n'est pas réentrant ; valeurs `bool` et `char`
  reçues de C sans vérification par les fonctions `@export`.

## Solutions envisagées

1. Garder une phrase générale (« NEXUL est mémoire-sûr ») : rejetée, invérifiable.
2. **Énoncer la garantie avec ses conditions, ses exclusions et l'état de ses preuves**,
   et transformer chaque condition en tâche de correction : retenue.
3. Attendre que tous les défauts soient corrigés avant d'écrire la garantie : rejetée, la
   documentation resterait fausse pendant ce temps.

## Décision

### 1. La garantie (état au 17/10/2026)

Pour un programme dont le code ne contient pas d'`unsafe` (bibliothèque standard et
paquets exceptés), compilé par `nxc` pour une cible de `nxc targets`, et sous les
conditions de la partie 2 : pas d'accès hors bornes, pas d'utilisation après libération,
pas de double libération, pas de référence pendante, pas de lecture de mémoire non
initialisée, pas de débordement arithmétique silencieux, pas de conversion numérique
silencieusement destructrice. Toute violation détectée à l'exécution est une panique
(message, position, code 101).

### 2. Les conditions

| Condition | Pourquoi | Ce qui la rendra inutile |
|---|---|---|
| C1. Les blocs `unsafe` de la bibliothèque standard et des paquets respectent leur contrat | le compilateur ne les vérifie pas | revue, tests, contrat écrit de chaque bloc `unsafe` de `lib/std` |
| C2. Le code C respecte les signatures `extern` et passe des valeurs valides aux `@export` | frontière non vérifiable | vérification de `bool` et `char` à l'entrée des `@export` (tâche T4) |
| C3. La pile suffit | dépassement non détecté ou non signalé selon la cible (partie 3) | sondes de pile, gestionnaire de dépassement, analyse de pile (T1–T3) |
| C4. Sans système : pas d'allocation ni de sortie dans un gestionnaire d'interruption | le runtime (`heap.nx`, `rt.nx`) n'est pas réentrant | sections critiques dans le runtime (T5), puis profil temps réel `with noalloc` (P8) |

### 3. Le modèle d'échec (comportement observable)

| Échec | Comportement aujourd'hui | Comportement visé pour 1.0 |
|---|---|---|
| Panique (bornes, débordement, `unwrap`, `assert`…) | message, position, sortie standard vidée, code 101, **aucun destructeur exécuté** | inchangé |
| Mémoire épuisée | panique | inchangé |
| Dépassement de pile, Windows/Linux/macOS | arrêt par le système (page de garde), sans message de NEXUL ; sortie standard tamponnée perdue ; trames plus grandes que la zone de garde non sondées | message `stack overflow`, code 101 ; sondes de pile (`probe-stack`) |
| Dépassement de pile, WebAssembly | pile du moteur : piège ; pile en mémoire linéaire : **écrase les données statiques** | `wasm-ld --stack-first` : piège |
| Dépassement de pile, sans système | **non détecté** : écrase le tas | limite de pile matérielle quand elle existe (MPU, `MSPLIM`), sinon analyse statique de pile exigée par le profil temps réel |
| Panique dans un gestionnaire d'interruption | arrêt (semi-hébergement) | inchangé |

### 4. Panique = arrêt du processus (pour la 1.0)

Une panique **arrête le processus entier**, sans dérouler la pile ni exécuter les
destructeurs. C'est déjà le comportement de la 0.2 (spec §7.3) ; cette ADR propose d'en
faire un engagement de la 1.0, pour trois raisons : aucun coût sur le chemin normal ni
table de déroulement (embarqué, WebAssembly), aucune valeur à moitié détruite observable,
et aucune panique ne traverse la frontière C (§8.4).

Conséquences acceptées : un serveur s'arrête sur la première panique (il faut un
superviseur de processus et des API à `Result`) ; avec la concurrence (ADR 0013), la
panique d'une tâche arrête toutes les tâches ; les ressources externes (fichiers
temporaires, verrous de fichiers) ne sont pas libérées par le programme.

Alternative écartée pour la 1.0 : le déroulement avec récupération (`catch`). Il
changerait l'ordre des destructions, imposerait des tables de déroulement sur toutes les
cibles et une règle pour les paniques pendant une destruction. On peut l'ajouter après la
1.0 sous forme d'**isolation de tâche** sans changer cette règle pour le code existant.

### 5. Règle de communication

La documentation (README, guide, site, notes de version) ne dit jamais « mémoire-sûr »,
« sans course de données » ou « sécurisé » sans renvoyer à la partie 1 et à ses
conditions. Toute nouvelle construction qui touche à la mémoire ou à la concurrence met à
jour ce tableau avant d'être annoncée.

## Conséquences

- Positives : la spécification redevient exacte ; chaque condition devient une tâche
  mesurable ; les utilisateurs embarqués et WebAssembly connaissent les risques réels.
- Négatives : le discours devient moins simple ; certaines conditions (C3 sans système)
  ne disparaîtront qu'avec le profil temps réel.

## Composants concernés

`docs/spec/05-memoire.md` (§5.8, mis à jour), `docs/spec/07-erreurs.md`, `README.md`
(phrase « vérifié à la compilation »), `lib/std/rt.nx`, `lib/std/heap.nx`,
`lib/std/sys/*.nx`, `compiler/nxc/src/link.rs`, `compiler/nxc_codegen_llvm`.

## Tâches et preuves attendues

| Tâche | Responsable proposé | Preuve |
|---|---|---|
| T1 Sondes de pile : attribut LLVM `"probe-stack"="inline-asm"` sur x86-64 et AArch64 (Linux, macOS) | Codex | une fonction avec un tableau local de 4 Mio récursive se termine par un arrêt propre, pas par une écriture hors pile |
| T2 Message de dépassement de pile : pile de signal (`sigaltstack` + `SIGSEGV`) sur Linux, gestionnaire d'exception vectorisé sur Windows ; vidage de `OUT_BUF` par appel système direct | Codex | test `tests/run` : récursion infinie → `stack overflow` sur stderr, code 101, sortie déjà écrite conservée |
| T3 `wasm-ld --stack-first` | Codex | récursion avec agrégats sous `wasm32-wasi` : piège, données statiques intactes |
| T4 Vérification de `bool` (0 ou 1) et `char` (valeur scalaire) à l'entrée des `@export` | Codex, revue Claude | test de bibliothèque appelée depuis C avec des valeurs invalides → panique |
| T5 Sections critiques (masquage des interruptions) dans l'allocateur et les sorties sur les cibles sans système | Codex, revue Claude | test QEMU : interruption périodique qui alloue pendant que `main` alloue en boucle, sans corruption |
| T6 Phrase du README alignée sur la partie 1 | Claude | relecture |

## Risques

- Les sondes de pile ont un coût (une instruction par page de trame) : à mesurer (P13).
- Un gestionnaire de `SIGSEGV` doit distinguer le dépassement de pile des autres fautes
  (adresse dans la zone de garde), sinon il masquerait d'autres défauts.
