# Tâches pour Codex — 24/10/2026

Rédigé par Claude (architecture, sémantique, sûreté) à partir de l'[audit du 17/10](audit-claude-2026-10-17.md)
et des lots 1 à 9 (PR #35 à #43). Chaque tâche relève du périmètre d'implémentation de
Codex (mission maîtresse §2.3) ; Claude relit les PR qui touchent la sûreté (contrats
[K1–K7](../spec/contrats-composants.md)).

## Avant de commencer

1. **Base de travail.** Les PR #35 → #43 sont empilées. L'utilisateur les fusionne dans
   l'ordre. Partez de `main` une fois qu'elles sont fusionnées, ou de la branche
   `refonte/core-diagnostics-d4-d5` (sommet de la pile) en attendant.
2. **Règles** (AGENTS.md §4) : une branche `refonte/sol-…` et une PR `[sol]` par tâche ;
   tests **réellement exécutés**, écrits dans la PR avec leur résultat ; « non testé »
   quand un outil manque ; aucun test désactivé pour obtenir une CI verte.
3. **Contrats à respecter** ([contrats-composants.md](../spec/contrats-composants.md)) :
   - aucune vérification entière retirée, à aucun niveau d'optimisation (K3.4) ;
   - pas de `fast-math` (K3.5) ;
   - aucun attribut d'alias tiré de la loi d'exclusivité sans ADR (K3.6) ;
   - destructions inchangées (K3.7) ;
   - IR déterministe (K3.8) ;
   - blocs inaccessibles non générés (K3.10) ;
   - pile de 256 Mio pour compiler (K6.3) ;
   - un test UI par code de diagnostic (K7.1).
4. **Ne pas modifier sans revue de Claude** : les règles de `compiler/nxc_sema/src/places.rs`,
   `compiler/nxc_nir/src/ownership.rs`, `compiler/nxc_sema/src/exhaust.rs`, `docs/spec/**`,
   `docs/adr/**`. Les messages de diagnostic peuvent changer, en le signalant dans la PR.
5. **Outils utiles** :
   - `cargo test --release -p nxc --test robustness` : mutations et sources pathologiques ;
   - `NEXUL_FUZZ_ROUNDS=200 NEXUL_FUZZ_SEED=<n>` pour une campagne plus longue ;
   - `compiler/nxc/tests/codegen_ir.rs` : tests de l'IR sans LLVM ;
   - `// target:` et `// edition: 1` en tête d'un test UI ;
   - `// targets: linux` en tête d'un test `tests/run`.

## Priorité 1 — sûreté (ADR 0011)

### C1. Sondes de pile (`probe-stack`) — tâche T1

- **Problème.** Une trame plus grande que la zone de garde (un tableau local de plusieurs
  Mio dans une fonction récursive) peut sauter par-dessus la page de garde et écrire dans
  une autre zone mémoire, depuis du code sûr.
- **Travail.** Attribut LLVM `"probe-stack"="inline-asm"` sur les fonctions générées,
  pour x86-64 et AArch64 (Linux, macOS ; Windows sonde déjà avec `__chkstk`).
- **Fichiers.** `compiler/nxc_codegen_llvm/src/lib.rs` (attributs de fonction).
- **Critère.**
  - `tests/run/stack_probe.nx` (`// targets: linux`) : une fonction récursive avec un
    tableau local de 4 Mio se termine par `panic: stack overflow…`, code 101 (le
    gestionnaire du lot 8 fait le reste), et non par une écriture hors pile.
  - Coût mesuré sur les références P13 et publié dans la PR.

### C2. Message de dépassement de pile sur les autres systèmes — suite de T2

- **Contexte.** Linux x86-64 et AArch64 sont faits (PR #42 : `sigaltstack` + `SIGSEGV`,
  `lib/std/sys/linux.nx`, entrée C `nx_linux_fault`).
- **Windows.** `AddVectoredExceptionHandler` pour `EXCEPTION_STACK_OVERFLOW`
  (0xC00000FD), avec `SetThreadStackGuarantee` pour que le gestionnaire ait de la pile.
  Vidage de `OUT_BUF` par `WriteFile`, puis `ExitProcess(101)`, avec le même message
  que sur Linux.
- **Linux RISC-V.** La structure `sigaction` du noyau n'a pas de `sa_restorer` :
  adapter `install_fault_handler` (aujourd'hui sans effet sous `@cfg("riscv64")`).
- **macOS.** Dépend de C12.
- **Critère.**
  - `tests/run/stack_overflow.nx` passe aussi sur Windows (changer la ligne
    `// targets:`), ce qui demande C6.
  - Test d'exécution RISC-V sous QEMU, ou « non testé » écrit.

### C3. Pile WebAssembly placée avant les données — tâche T3

- **Problème.** `wasm-ld` place la pile en mémoire linéaire après les données statiques :
  un dépassement les écrase en silence.
- **Travail.** Ajouter `--stack-first` dans `link_wasm` (`compiler/nxc/src/link.rs`).
- **Critère.**
  - Sous `wasm32-wasi` (Node, `NEXUL_TEST_TARGETS=wasm32-wasi`), une récursion avec
    agrégats se termine par un piège, avec les statiques intactes.
  - La suite `tests/web` et le contrat navigateur (`docs/refonte/interop-web.md`) sont
    inchangés.

### C4. Valeurs reçues de C par les fonctions `@export` — tâche T4

- **Problème.** Un `char` invalide (0xD800, > 0x10FFFF) ou un `bool` différent de 0/1
  passé par C à une fonction `@export` casse les invariants du langage (UTF-8 de
  `String`, filtrage exhaustif).
- **Travail.** Vérifier ces paramètres à l'entrée des `@export` (panique avec position)
  dans le générateur. Plus tard, faire de même pour les types contraints (ADR 0014).
- **Critère.** Test par l'hôte JavaScript (`tests/web`, `wasm32-web`) : appeler un export
  avec un `char` 0xD800 donne une panique ; avec une valeur valide, résultat inchangé.

### C5. Runtime réentrant sur les cibles sans système — tâche T5

- **Problème.** Un gestionnaire d'interruption (`@export("SysTick_Handler")`) écrit en
  code sûr peut appeler l'allocateur ou les sorties pendant que `main` les utilise :
  corruption de `heap.nx` / `rt.nx`.
- **Travail.** Sections critiques (masquage des interruptions : `cpsid i`/`cpsie i` sur
  Cortex-M, `csrrci mstatus` sur RISC-V) autour de l'allocateur et du tampon de sortie.
- **Fichiers.** `lib/std/heap.nx`, `lib/std/rt.nx`, `lib/std/sys/bare.nx`, intrinsèques
  du générateur si nécessaire.
- **Critère.** Test QEMU (`thumbv7em-none`) : une interruption périodique qui alloue
  pendant que `main` alloue en boucle ; aucune corruption, aucune fuite.

## Priorité 2 — CI et fiabilité

### C6. Tests d'exécution aussi sous Windows

- **Problème.** Seul le job Linux exécute `tests/run` (avec LLVM) ; le job Windows ne
  teste que l'installateur et les crates de Studio.
- **Travail.** Installer LLVM dans le job Windows et y lancer
  `cargo test --release -p nxc` (dont `run_programs` et `run_programs_optimized`).
- **Fichiers.** `.github/workflows/*.yml`. C'est un fichier transversal : demandez
  l'accord de l'utilisateur.
- **Critère.** Les deux jobs exécutent la suite complète.

### C7. CI des PR empilées

- **Problème.** Le workflow ne se déclenche que pour les PR vers `main`. Une PR empilée
  sur une autre branche n'a aucune CI, ce qui oblige à viser `main` en dépendant d'une
  autre PR.
- **Travail.** Ajouter `workflow_dispatch` et le déclenchement sur les PR de toute base.
- **Critère.** Une PR basée sur une branche `refonte/…` lance les deux jobs.

### C8. Test instable de l'installateur

- **Constat.** `win::reg::tests::keys_tree` a échoué une fois sur la CI de #35, puis a
  réussi au second essai, sans changement de code. Erreur : « Illegal operation
  attempted on a registry key that has been marked for deletion (os error 1018) ».
- **Travail.** Utiliser un nom de clé unique par test et par exécution, et attendre la
  suppression ou réessayer.
- **Critère.** 20 exécutions consécutives sans échec, en local ou en CI.

### C9. Temps d'analyse : cas extrêmes

- **Contexte.** Le lot 5 (PR #39) a corrigé l'essentiel : 20 000 `if` passent de plus de
  10 min à 3,7 s.
- **Reste.** Un `match` de 50 000 bras prend 9 à 43 s : les ensembles de bits de
  `ownership.rs` sont copiés à chaque bloc, en O(blocs × variables).
- **Travail.** Ensembles creux, ou analyse limitée aux variables vivantes dans la portée.
  Revue de Claude obligatoire (`ownership.rs`).
- **Critère.** 50 000 bras en moins de 2 s, 20 000 `if` en moins de 1 s (release). Mêmes
  diagnostics sur toute la suite.

## Priorité 3 — performances et architecture

### C10. Vérifier les références de performance (A1, PR #29)

- **Problème.** NEXUL y apparaît **plus rapide que C** sur « Boucles + maths » (1,12×)
  avec toutes ses vérifications.
- **Travail.**
  - Inspecter l'IR optimisée de ce noyau (`--emit llvm-ir`) : les vérifications de
    débordement sont-elles présentes (K3.4) ?
  - Comparer les fonctions mathématiques : NEXUL contre la libm de C.
  - Optimiser ensuite, mesures à l'appui, les cas « chaîne ASCII » (0,2×) et
    « vecteurs » (0,5×).
- **Critère.** Explication écrite de l'écart ; gains mesurés, médiane et dispersion
  publiées.

### C11. Monomorphisation hors du générateur LLVM (constat A5)

- **Travail.** Extraire la monomorphisation et les tables `dyn` en une passe « NIR
  monomorphe » indépendante de LLVM, préalable à tout second backend et à l'étape SH5
  de l'auto-hébergement.
- **Préalable.** Proposition écrite, puis revue de Claude avant le code.
- **Critère.** IR identique à l'octet sur toute la suite avant et après (K3.8).

### C12. Couche système macOS (O7)

- **Problème.** macOS utilise aujourd'hui les appels système de Linux.
- **Travail.** Écrire une vraie couche (`lib/std/sys/macos.nx`), ou exclure explicitement
  la cible dans `nxc targets` et la documentation.
- **Critère.** `tests/run` sur un Mac, ou « BLOQUÉ : pas de Mac » écrit.

## Ce que Claude garde

Conception et règles de sûreté :
- décisions sur les ADR 0011 à 0015, puis implémentation des règles de langage
  correspondantes ;
- ADR du profil temps réel (P8-G1) une fois la cible choisie ;
- revue de toutes les PR qui touchent la NIR, le générateur ou le runtime sous l'angle
  de la sûreté.
