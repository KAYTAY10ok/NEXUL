# 0002 — Premier backend : C11 portable, compilé par Zig

**Statut : remplacée** par [0008](0008-backend-llvm-et-runtime-nexul.md) (version 0.2). Le
backend C et le runtime C ont été supprimés.

**Contexte.** Le cahier des charges recommande LLVM (§3.1) et demande la compilation
croisée (§3.3). Lier le compilateur à la bibliothèque LLVM impose une version précise de
LLVM, compliquée à installer sur chaque plateforme.

**Décision.** La NIR est traduite en **C11 portable**, compilé par `zig cc` (Clang/LLVM
avec les bibliothèques C de toutes les cibles). Le C produit contient des directives
`#line` qui renvoient au source NEXUL. Un backend LLVM IR direct est prévu (0.4) sur la
même NIR.

**Conséquences.**
- LLVM optimise le code (via Clang) sans que `nxc` en dépende.
- Compilation croisée immédiate : Windows, Linux, macOS, ARM64, RISC-V, WebAssembly.
- Les débogueurs et les erreurs du compilateur C pointent vers le source NEXUL.
- Le C généré doit éviter tout comportement indéfini : arithmétique vérifiée par
  `__builtin_*_overflow`, décalages vérifiés, options `-fwrapv -fno-strict-aliasing`.

**Alternatives.** LLVM via ses API (dépendance lourde) ; Cranelift (moins de cibles,
pas de WebAssembly natif en sortie) ; générateur maison (effort démesuré, §3.1).
