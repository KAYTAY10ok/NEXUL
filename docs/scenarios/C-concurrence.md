# Scénario C — Concurrence (passation §15.2)

> « Lancer tâches productrices/consommatrices ; communication canal borné ; annuler sous
> contention ; vérifier ressources ; refuser par compilation un partage mutable incorrect. »

Joué le 28/10/2026 par Claude Main, branche `claude/pensive-darwin-3e2tye`, sur Linux x86-64
(conteneur, 4 vCPU) ; rejoué par la CI à chaque PR sur Linux, Windows et macOS.

| Exigence | Preuve | Commande |
|---|---|---|
| Producteurs et consommateurs | `tests/run/task_channels.nx` (pipeline, producteur détaché, portée), `tests/run/task_channels_stress.nx` (8 producteurs, 3 consommateurs qui partagent un `Recepteur`, 2 places) | `cargo test --release -p nxc --test suite run_programs` (O0) et `run_programs_optimized` (O2) |
| Canal borné | capacités 1, 2, 16, 64 dans ces programmes ; `envoyer` attend tant que le canal est plein | idem |
| Annuler sous contention | `task_channels_stress.nx` : le récepteur est détruit pendant que son émetteur attend une place ; l'émetteur obtient `false`, la valeur est détruite ; `fermer()` termine les consommateurs (`recevoir` rend `None`) | idem |
| Vérifier les ressources | la suite exécute chaque programme avec `NEXUL_LEAK_CHECK=1` : le runtime vérifie à la sortie qu'aucune allocation ne reste (threads, canaux, valeurs restées dans un canal) | idem |
| Refuser un partage mutable incorrect | `tests/ui/tasks.nx`, `tests/ui/task_properties.nx`, `tests/ui/task_channels.nx`, et le corpus de courses `tests/ui/task_races.nx` (9 cas : paramètre `inout`, fonction anonyme imbriquée, méthode `inout`, champ, rappel d'un `Verrou`, boucle, affectation pendant une lecture, portées imbriquées, `dyn` sans `Transfer`) | `cargo test --release -p nxc --test suite ui_diagnostics` |
| Charge : 10⁶ messages, 64 tâches, résultat déterministe (critère de la 0.5.0) | `tests/stress/channels_1m.nx` : 64 producteurs détachés, 2 consommateurs, canal de 64 places ; 3,7 s ici en release | `NEXUL_STRESS=1 cargo test --release -p nxc --test suite stress_programs` |
| Pannes dans une tâche | une panique dans un thread termine le programme (code 101) ; un dépassement de pile dans un thread aussi (`tests/run/stack_overflow_thread.nx`) | `run_programs` |

**Non testé : ThreadSanitizer.** Sous Linux, NEXUL démarre ses threads par l'appel système
`clone` et attend par `futex`, sans bibliothèque C : ThreadSanitizer, qui intercepte
`pthread_create` et les fonctions de synchronisation de la libc, ne voit ni les threads ni
leurs synchronisations. L'absence de courses dans le code sûr repose sur les règles du
compilateur (ADR 0013, corpus ci-dessus) ; le code `unsafe` de `std.tache` (canaux, verrous,
atomiques) a été relu le 28/10 (`docs/refonte/revues-main.md`). Piste : un mode de
compilation qui instrumente les accès mémoire pour un détecteur écrit en NEXUL.

**Verdict de Main : scénario C passé** sur Linux, Windows et macOS (CI), sauf
ThreadSanitizer (non testé, raison ci-dessus).
