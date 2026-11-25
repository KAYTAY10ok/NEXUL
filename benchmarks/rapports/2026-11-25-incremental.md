# Compilation incrémentale en `-O0` : mesures du 25/11/2026 (ADR 0016, validation §3)

Machine : Linux x86-64, Intel Xeon à 2,10 GHz, 4 processeurs (conteneur partagé).
Compilateur : `nxc` 0.5.0 en version optimisée, construit à partir de la branche de cette
mesure, avec LLVM 20 chargé à l'exécution.

## Méthode

- **Programme** : `benchmarks/incremental/gen.py` écrit 1 000 fonctions, soit 21 006
  lignes, toutes atteignables depuis `main`. Chacune remplit un `Vec`, le parcourt,
  formate une `String` et appelle la précédente.
- **Modification** : le corps de `g500` change par une constante de boucle ; aucune ligne
  ne bouge.
- **Mesure** : `benchmarks/incremental/measure.py` lance `nxc build` et mesure le temps
  mural du processus entier.
  - À chaque répétition, le cache est vide au départ.
  - Le cache des exécutables entiers ne peut pas répondre après la modification, puisque
    le texte a changé.
  - Chaque exécutable est lancé, et sa sortie doit être celle de la construction en un
    seul module (`NEXUL_INCREMENTAL=0`).
- **Répétitions** : 7. La commande à lancer :

  ```
  python -I benchmarks/incremental/measure.py target/release/nxc <dossier> 1000 7
  ```

## Résultats

| Build | Median (s) | Samples (s) |
|---|---|---|
| cold, monolithic | 2.031 | 1.942, 1.960, 1.954, 2.031, 2.118, 2.342, 2.286 |
| cold, units | 0.942 | 1.024, 0.848, 0.934, 0.894, 0.942, 1.009, 0.965 |
| one body changed, monolithic | 2.156 | 1.780, 2.156, 2.349, 2.180, 2.489, 1.877, 2.121 |
| one body changed, units | 0.603 | 0.573, 0.627, 0.578, 0.603, 0.677, 0.531, 0.660 |


Après la modification, une unité sur 41 est recompilée.

**Ventilation** d'une reconstruction après la modification (`NEXUL_INCREMENTAL_REPORT=1`,
une exécution) :

| Étape | Temps |
|---|---|
| analyse (lexique, syntaxe, sémantique, NIR) | environ 0,3 s (`nxc check` : 0,42 s, processus compris) |
| génération de l'IR du programme entier | 0,16 à 0,21 s |
| découpage en unités | 58 à 78 ms |
| clés (SHA-256 des unités) | 23 à 33 ms |
| LLVM (une unité) | 40 à 58 ms |
| édition de liens (41 objets) | 21 à 31 ms |

## Lecture

- **Après la modification d'un corps** : 0,60 s au lieu de 2,16 s (médianes), soit
  3,6 fois moins.
- **Ce qui reste** : l'analyse et la génération de l'IR du programme entier. Le cache
  syntaxique de `std` (décision 3) n'est pas fait.
- **À froid** : 0,94 s au lieu de 2,03 s, parce que les unités sont compilées en
  parallèle sur 4 processeurs.

## Limites

- **Un seul programme**, synthétique, sur une seule machine. Il n'y a pas de mesure sur
  un programme qui utilise une grande part de `lib/`.
- **Lignes qui bougent** : une modification qui ajoute des lignes recompile aussi les
  unités des fonctions placées après elle dans le même fichier, parce que les
  informations de débogage portent leur ligne. Le test `a_new_function_does_not_rename_the_others`
  le vérifie, sans mesure de temps.
- **Non mesurés** :
  - la latence des diagnostics du LSP (validation §4) ;
  - le temps d'exécution d'un programme découpé par rapport à un seul module en `-O0`
    (validation §5).
- **Cibles** : Linux x86-64 et AArch64 seulement. AArch64 est testé sous QEMU par la
  suite, sans mesure de temps.
