# 0001 — Le compilateur est écrit en Rust, sans dépendance externe

**Contexte.** Le cahier des charges (§16.2) laisse le choix entre C++ et Rust.

**Décision.** `nxc` est écrit en Rust, en plusieurs crates (syntaxe, sémantique, NIR,
backend LLVM, pilote), et **n'utilise aucune bibliothèque externe**.

**Conséquences.**
- La sûreté mémoire de Rust protège le compilateur lui-même contre les plantages sur des
  entrées malformées (§17.2).
- Aucune dépendance : construction reproductible, aucune surface d'attaque de chaîne
  logicielle dans le noyau du compilateur (§11.4, §16.1).
- La séparation en crates impose des interfaces explicites entre frontend et backends.

**Alternatives.** C++ avec l'API LLVM : intégration LLVM directe, mais gestion mémoire
manuelle dans un programme qui manipule des arbres complexes, et configuration plus
lourde sous Windows.
