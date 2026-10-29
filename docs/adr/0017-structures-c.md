# ADR 0017 — Structures C par valeur (`@repr("C")`)

- **Statut** : acceptée (Claude Main, 29/10/2026), dans le périmètre de la 0.6.0 validé par
  l'utilisateur (feuille de route : « `@repr("C")`, rappels C ») ; pas de rupture de
  compatibilité : un nouvel attribut, rien ne change pour le code existant.
- **Numéro** : 0016 est réservé à la compilation incrémentale (proposition de Codex Terra).

## Contexte

Une fonction C qui prend ou rend une structure par valeur (`struct timespec`, `Vec2` d'une
bibliothèque graphique, `div_t`…) ne pouvait pas être appelée : la frontière C n'acceptait
que des scalaires et des pointeurs (E0503). Passer une structure par valeur suit des règles
propres à chaque cible (registres entiers ou flottants, morceaux de 8 octets, pile, mémoire
fournie par l'appelant) que le générateur doit appliquer lui-même : LLVM ne les connaît pas.

## Décision

1. **Attribut `@repr("C")` sur une structure** (édition 1 et 0.2). Il garantit la
   disposition de C et autorise la structure par valeur dans les signatures `extern "C"` et
   `@export`. Spécification : chapitre 08, §8.5.
2. **Champs** : seulement ce que C représente sans ambiguïté — entiers, flottants,
   `Ptr[T]`, rappels C, tableaux de ces types, autres structures `@repr("C")`. `bool` et
   `char` sont **refusés** : une valeur venue de C pourrait être invalide (2 pour un
   `bool`, une moitié de paire de substitution pour un `char`), ce que la sûreté de NEXUL
   interdit (ADR 0011). Pas de type paramétré, pas de destructeur : C copie la structure
   bit à bit. Erreur : **E0505**.
3. **Convention d'appel** : celle de clang pour la même déclaration C, cible par cible, dans
   un module unique du générateur (`compiler/nxc_codegen_llvm/src/cabi.rs`), qui décide pour
   chaque paramètre : morceaux en registres, agrégat, pointeur vers une copie (`byval` ou
   non), et pour le résultat : registres ou `sret`. Les règles qui dépendent des registres
   restants (x86-64 System V, convention flottante de RISC-V) les comptent sur toute la
   signature.
4. **Preuve** : `compiler/nxc/tests/c_abi.rs` compare les signatures à celles de clang sur
   toutes les cibles, et exécute des échanges réels NEXUL ↔ C sur toutes celles que la CI
   peut exécuter. Une règle qui diverge de clang est un bogue de `nxc`.
5. **Hors du périmètre, pour l'instant** : une structure par valeur dans le type d'un
   pointeur de fonction de rappel (une fonction NEXUL passée à C n'a pas la convention C
   pour ses structures : E0503, passer `Ptr[S]`) ; unions ; champs de bits ; variadiques.

## Conséquences

- Les bibliothèques C courantes deviennent utilisables sans fonction d'enrobage écrite en C.
- Sur ARM, le runtime fournit en plus les fonctions flottantes de l'ABI d'exécution
  (`__aeabi_dadd`…), appelées par le code C lié à un programme NEXUL sans `compiler-rt`.
- En corrigeant, un défaut existant : sur RISC-V 64, les entiers de 32 bits passés à C sont
  maintenant étendus par signe (`signext`) comme l'exige l'ABI, et un résultat C de moins de
  32 bits ne produit plus d'IR invalide.
