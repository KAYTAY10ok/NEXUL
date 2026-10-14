# 0008 — Backend LLVM et runtime écrit en NEXUL

**Remplace** [0002](0002-backend-c-et-zig.md).

**Contexte.** Le cahier des charges révisé impose l'**autonomie complète** de NEXUL : un
compilateur, un runtime, une bibliothèque et des outils propres, sans transpilation vers
C comme mécanisme principal. LLVM reste explicitement permis comme infrastructure de
génération de code. Le backend 0.1 traduisait la NIR en C11 compilé par `zig cc`, et le
runtime contenait du C (`runtime/nexul_rt.h`) : deux dépendances au langage C et à une
chaîne d'outils externe.

**Décision.**
- `nxc` traduit la NIR en **IR LLVM textuelle** (crate `nxc_codegen_llvm`) et la confie à
  la bibliothèque C de LLVM (`LLVM-C.dll`, `libLLVM.so`), **chargée à l'exécution** : le
  compilateur ne se lie pas à LLVM à la construction et reste sans dépendance Rust.
  L'optimisation passe par le gestionnaire de passes de LLVM (`default<O2>`…), le code
  objet par sa machine cible.
- L'édition de liens est faite par **LLD** (`lld-link`, `ld.lld`, `wasm-ld`, `ld64.lld`).
  Sur Windows, `nxc` écrit lui-même les bibliothèques d'importation des DLL du système :
  ni Visual Studio ni SDK Windows ne sont nécessaires.
- Le **runtime est écrit en NEXUL** (`lib/std/rt.nx`, `heap.nx`, `fmt.nx`, `math.nx`,
  `builtins.nx`, `sys/*.nx`) : il appelle directement l'API Windows, le noyau Linux, WASI
  ou le matériel (semi-hébergement, registres). Aucune bibliothèque C n'est liée.
- Les informations de débogage (CodeView ou DWARF) sont émises comme métadonnées LLVM ;
  LLDB (via `lldb-dap`) sert de débogueur à NEXUL Studio.

**Conséquences.**
- Plus aucun compilateur C dans la chaîne : NEXUL, LLVM et LLD suffisent, et
  l'installateur les fournit.
- Compilation croisée vers toutes les cibles de LLVM depuis une seule installation, y
  compris les microcontrôleurs sans système.
- Le formatage des nombres, les mathématiques et les flottants logiciels doivent être
  exacts par eux-mêmes : ils sont validés contre Python et contre le processeur
  (`tests/numerics/`).
- La version de LLVM peut varier (17 ou plus) : l'IR produite n'utilise que des
  constructions stables, et l'interface C de LLVM est stable.

**Alternatives.** Lier LLVM à la construction (dépendance lourde, version figée) ;
Cranelift (moins de cibles, pas de microcontrôleurs ni de WebAssembly objet natif) ;
générateur de code maison (effort démesuré au regard du cahier des charges §3.1) ;
garder le backend C (contraire à l'exigence d'autonomie).
