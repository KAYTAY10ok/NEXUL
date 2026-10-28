# Plateformes : ce qui est testé, où et comment

État au 28/10/2026 (version 0.3.0). Une cible n'est annoncée « testée » que si la suite de
conformité (`tests/run`, chaque programme comparé à sa sortie, son code de sortie et sa
sortie d'erreur attendus) s'y **exécute**. « Compilée seulement » veut dire que `nxc`
produit un exécutable valide, sans l'avoir lancé. Liste des cibles : `nxc targets`.

| Cible | Système | Exécution de `tests/run` | Par qui, sur quoi | Remarques |
|---|---|---|---|---|
| `x86_64-linux` | Linux | **O0 et O2**, programmes migrés en édition 1 | CI GitHub (`ubuntu-latest`) à chaque PR | référence ; threads, pile, ABI navigateur |
| `x86_64-windows` | Windows 10/11 | **O0 et O2** | CI GitHub (`windows-latest`, LLVM 20.1.8) | threads par `CreateThread`, attente par cession du processeur |
| `aarch64-macos` | macOS 11 et plus (Apple silicon) | **O0 et O2** | CI GitHub (`macos-latest`, LLVM 20 de Homebrew) depuis le 28/10 | couche `libSystem` (`sys/macos.nx`), threads POSIX, `__ulock_wait` |
| `aarch64-linux` | Linux | O0 | CI GitHub, QEMU en mode utilisateur | binaires statiques |
| `riscv64-linux` | Linux | O0 | CI GitHub, QEMU en mode utilisateur | dépassement de pile vérifié par Codex Sol (C2) |
| `wasm32-wasi` | WASI | O0 | CI GitHub, Node (`tools/wasi-run.mjs`) | tâches exécutées l'une après l'autre |
| `wasm32-web` | navigateur | ABI v1 (O0, O2) | CI GitHub, Node (`tests/web`) | pile placée avant les données |
| `thumbv7em-none` | sans système (Cortex-M4F) | O0, sauf les programmes qui demandent un système de fichiers ou beaucoup de mémoire | CI GitHub, QEMU (`mps2-an386`, semihosting) | runtime réentrant (C5) ; pas de threads |
| `x86_64-macos` | macOS (Intel) | compilée seulement | — | même couche que `aarch64-macos` ; aucune machine Intel en CI |
| `aarch64-windows` | Windows sur ARM | compilée seulement | — | |
| `thumbv7m-none`, `riscv32-none`, `riscv64-none`, `aarch64-none` | sans système | compilées seulement | — | démarrage et vecteurs produits ; à valider sur QEMU |

## Garanties qui dépendent de la plateforme (ADR 0011)

| Garantie | Linux | Windows | macOS | WebAssembly | Sans système |
|---|---|---|---|---|---|
| Dépassement de pile → panique (code 101) | oui (x86-64, AArch64, RISC-V), threads compris | oui (`__chkstk`, gestionnaire vectorisé), threads compris | oui (signal sur pile alternative), threads compris | piège de la machine, données intactes | **non détecté** |
| Sondes de pile | x86-64, AArch64 | par `__chkstk` | x86-64, AArch64 | sans objet | non |
| Threads de `std.tache` | oui | oui | oui | non (tâches l'une après l'autre) | non |
| Temps réel | non | non | non | non | profil prévu en 0.7.0 (Cortex-M4) |

## Ce qui manque pour une cible

- **macOS Intel** : une machine x86-64 en CI (`macos-13`).
- **Windows sur ARM** : une machine en CI.
- **Autres cartes sans système** : brancher QEMU (`virt`) dans la suite, comme pour le Cortex-M4.

Les commandes sont dans `.github/workflows/rust.yml` ; en local :
`NEXUL_TEST_TARGETS=aarch64-linux,riscv64-linux,wasm32-wasi,thumbv7em-none cargo test --release -p nxc --test suite run_programs_cross`
(QEMU et Node dans le `PATH`).
