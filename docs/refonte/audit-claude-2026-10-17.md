# Audit de la conception du langage, de la sûreté et de l'architecture — 17/10/2026

Auteur : Claude (responsable de l'architecture fondamentale, de la sémantique, de la
sûreté ; mission maîtresse « NEXUL 1.0.0 », §2.2 et §6.1). Base : `main` à `c40acbd`.
Travaux : branche locale `refonte/core-surete-semantique` (non poussée à la date du
rapport ; voir §6).

Légende des constats : **démontré** (reproduit par une commande ou un test), **lu**
(établi par lecture du code, non exécuté), **hypothèse** (à vérifier). Gravité :
critique, haute, moyenne, basse.

## 0. Environnement et limites de cet audit

- PC sans LLVM (`C:\Program Files\LLVM` absent, `NEXUL_LLVM` vide) : `nxc check`, la
  suite UI, les tests du pilote, de l'IDE, du LSP et de la migration s'exécutent ;
  `nxc build/run/test` et `tests/run` **ne s'exécutent pas** ici (« LLVM was not found »).
  Tout ce qui touche à l'exécution est donc « lu » ou démontré seulement par l'analyse.
- Base de tests sur `main` sans LLVM (`cargo test --release -p nxc --no-fail-fast`) : les
  seuls échecs sont dus à l'absence de LLVM (70 occurrences du message) ; aucun échec
  d'un test qui ne dépend que du frontend.
- Une copie locale plus ancienne (`C:\NEXUL`, 0.1, backend C) a servi à des essais
  d'exécution ; ses résultats sont signalés comme tels.

## 1. Synthèse

Le cœur du langage est **cohérent et bien fondé** : propriétaire unique, références de
seconde classe, fonctions anonymes non échappantes, loi d'exclusivité étendue aux
captures, `static var` en `unsafe`. 20 sondes ciblées (aliasage par fonction anonyme,
gel des boucles et filtrages, arguments en deux phases, destruction conditionnelle) ont
toutes été refusées ou exécutées correctement, à **deux exceptions près, toutes deux
corrigées** dans la branche : une utilisation après libération dans du code sûr (gardes)
et la largeur de `isize`/`usize` sur les cibles 32 bits.

Les risques majeurs ne sont pas dans ce qui existe mais dans ce qui va être ajouté sans
fondations : concurrence, `Shared[T]`/`===` déjà réservés par l'ADR 0010, copiabilité des
types publics, erreurs de la bibliothèque standard. Trois ADR proposées les encadrent.

## 2. Constats priorisés

| ID | Gravité | Statut | Constat | Suite |
|---|---|---|---|---|
| S1 | **critique** | démontré, **corrigé** | Garde qui déplace une liaison d'un scrutin valeur | §2.1 |
| S2 | **haute** | démontré, **corrigé** | `isize`/`usize` vérifiés sur 64 bits pour toutes les cibles | §2.2 |
| S3 | haute | lu | Dépassement de pile : non détecté sans système, non signalé ailleurs, pile Wasm après les données, trames non sondées | ADR 0011, T1–T3 (Codex) |
| S4 | haute | lu | Gestionnaires d'interruption sûrs, runtime non réentrant (allocateur, sorties) | ADR 0011 C4, T5 (Codex) |
| S5 | haute | lu | La garantie de sûreté était annoncée sans périmètre ni conditions | ADR 0011, spec §5.8 réécrite |
| A1 | haute | lu | Copiabilité structurelle, champs privés compris : rupture d'API invisible ; `Copy` utilisé par l'ADR 0010 mais inexistant | ADR 0012 (décision utilisateur) |
| A2 | haute | lu | Concurrence sans fondations alors que l'ADR 0010 réserve `spawn`, canaux, `===`, `Shared[T]` | ADR 0013 (décision utilisateur) |
| A3 | moyenne | lu | Erreurs de la bibliothèque standard en `String` (`io.read_file` → `Result[String, String]`) et `?` sans conversion d'erreur : les changer après la 1.0 sera incompatible | §2.3, ADR 0015 (proposée) |
| A4 | moyenne | lu | Types distincts et contraints (`distinct`, `where it in …`) réservés sans sémantique des opérations | §2.4, ADR 0014 (proposée) |
| A5 | moyenne | lu | Monomorphisation et tables `dyn` dans le générateur LLVM : un second backend devrait les refaire | §2.5 |
| S6 | moyenne | lu | `@export` accepte `bool` et `char` venant de C sans vérification : un `char` invalide casse l'invariant UTF-8 de `String` | ADR 0011 T4 (Codex) |
| T1 | moyenne | lu, **corrigé** (lot 2) | `tests/run` exécuté seulement en debug (`BuildOptions::new(…, false)`) : rien ne vérifiait les garanties en `-O2` | test `run_programs_optimized` |
| D1 | basse | démontré, **corrigé** (lot 2) | E0402 annonçait « moved here, in a previous iteration of the loop » sans boucle (`f(s, s)`, arguments en deux phases) | `tests/ui/move_same_statement.nx` |
| D2 | basse | démontré, **corrigé** (lot 2) | Conflit d'exclusivité par capture : « this value is passed twice… », sans nommer la variable capturée | `tests/ui/exclusivity_closures.stderr` |
| D3 | basse | lu, **corrigé** (lot 2) | Le générateur détruisait inconditionnellement un `DropIfInit` résiduel au lieu de signaler une erreur interne | contrat K3.7 |
| D4 | basse | lu, **corrigé** | Spécification en retard : E0404 inexistant (§5.2), « 0.1 » dans les garanties, `static var` et captures absentes du ch. 05 | spec 03, 05 mises à jour |

### 2.1 S1 — Utilisation après libération par une garde (critique, corrigé)

```nexul
fn prendre(owned s: String) -> bool {
    return false
}

fn main() {
    let o: Option[String] = Some("ab" + "c")
    match move o {
        Some(s) if prendre(s) => println("jamais")
        Some(s) => println(s)
        None => {}
    }
}
```

- Spec §5.6 : « une garde voit les liaisons comme des emprunts ». La NIR le fait
  (`bind_views` avant la garde), mais `places.rs` traitait les liaisons d'un scrutin
  *valeur* comme des variables possédées : la garde pouvait les déplacer.
  `prendre(s)` détruit la chaîne ; la garde échoue ; le bras suivant lie à nouveau le
  temporaire caché, qui contient la chaîne libérée → lecture après libération, puis
  double libération.
- Preuves : `main` accepte le programme (`nxc check`, code 0). Sur la copie 0.1 (backend
  C), la variante où la garde réussit plante à l'exécution sans aucune sortie.
- Correction (`6e764de`) : pendant la vérification d'une garde, les liaisons de son bras
  sont des vues ; les déplacer (argument `owned`, `move s`, receveur `owned self`) est
  E0403 avec une note qui renvoie à §5.6. Tests `tests/ui/guard_move.nx` et
  `tests/ui/edition1_guard_move.nx` (édition 1 : `when … if … then`).

### 2.2 S2 — `isize`/`usize` sur les cibles 32 bits (haute, corrigé)

- `IntTy::bits()` vaut 64 pour `isize`/`usize` ; l'analyse l'utilisait pour les
  littéraux (E0301), les constantes (E0370), `MIN`/`MAX`/`BITS` et le calcul des
  constantes, alors que le générateur écrit ces types en `i32` sur `wasm32-*`,
  `thumbv7*-none`, `riscv32-none` et réduit les constantes modulo 2³² (`int_const`).
- Démontré : `let a: usize = 5_000_000_000`, `const BIG: usize = 4_000_000_000 +
  1_000_000_000` et `let c: isize = -3_000_000_000` passaient `nxc check --target
  wasm32-wasi` ; à l'exécution, `isize.MAX` aurait valu −1 et `usize.BITS` 64 (lu). Sur la
  copie 0.1, le même programme compilé pour x86 32 bits affiche `705032704` pour
  `5_000_000_000 as usize` (démontré).
- Correction (`6de277d`) : largeur de la cible (`Program::int_bits/int_min/int_max`)
  partout dans l'analyse ; message qui nomme la cible et propose `u64`/`i64`. Test
  `tests/ui/size_types_32_bit_target.nx` (nouvelle directive `// target:`). La
  bibliothèque standard, les exemples et les 37 programmes de `tests/run` passent `nxc
  check` sur les 14 cibles.

### 2.3 A3 — Erreurs de la bibliothèque standard

`std.io` rend `Result[T, String]` ; `?` exige des types d'erreur identiques (E0360) ;
aucune interface de conversion. Conséquence : le code utilisateur compare des messages,
et passer plus tard à `Result[T, IoError]` cassera tous les programmes. Recommandation
(à décider avant de figer `std.io` pour la 1.0) : une énumération `IoError` (`NotFound`,
`PermissionDenied`, `InvalidData`, `Other(String)`…) avec `Display` ; une interface de
conversion utilisée par `?` (`From` ou équivalent de l'édition 1) ; le repli `else` de
l'ADR 0010 reste la forme courte.

### 2.4 A4 — Types distincts et contraints

`type Metres = distinct f64` et `type Pourcent = i32 where it in 0..=100` sont réservés
(refusés avec « pas encore pris en charge », bien). Il manque la sémantique des
opérations. Proposition pour l'ADR de P5 : un type distinct n'hérite d'aucune opération
sauf déclaration (`extend Metres is Add`) ; un type contraint vérifie sa condition à la
construction et à la conversion (`x as Pourcent` panique, `Pourcent.try(x)` rend
`Option`) ; l'arithmétique sur un type contraint rend le **type de base** (`p + 1` est un
`i32`), ce qui évite des vérifications cachées.

### 2.5 A5 — Place de la monomorphisation

La NIR reste générique ; la monomorphisation, la disposition et les tables `dyn` vivent
dans `nxc_codegen_llvm`. Acceptable avec un seul backend ; avant tout second backend (ou
le générateur NEXUL de l'auto-hébergement, SH5), extraire une passe « NIR monomorphe »
indépendante de LLVM, pour ne pas dupliquer les règles de sûreté (vérifications,
destructions) dans chaque backend.

### 2.6 Diagnostics (pour Codex)

- D1 : dans `ownership.rs::report`, une utilisation *avant* le déplacement dans le texte
  est supposée venir d'une boucle. Distinguer les arguments en deux phases : si le
  déplacement et l'utilisation appartiennent au même appel, dire « `s` est déplacée par
  un argument suivant du même appel ; les places empruntées sont prises après
  l'évaluation de tous les arguments (spec §4.1) ».
- D2 : quand l'accès en conflit est une capture, nommer la variable et la fonction
  anonyme (« `v` est modifiée par cette fonction anonyme, qui la capture »).

## 3. Ce qui a été vérifié et tient

| Propriété | Preuve |
|---|---|
| Aliasage par fonction anonyme refusé (7 formes) | `tests/ui/exclusivity_closures.nx` (nouveau) |
| Arguments en deux phases (§4.1) : `show(v[0], grow(&v))` sans pointeur pendant | exécuté sur la copie 0.1 avec détection de fuites ; même construction NIR sur `main` (lu) |
| `static var` en `unsafe` (E0501) | sonde `nxc check` |
| Déplacement conditionnel et réinitialisation en boucle (drapeaux) | exécuté sur la copie 0.1 ; accepté sur `main` |
| Ordre de destruction (inverse, conditionnel) | exécuté sur la copie 0.1 |
| Aucune API `self` de `lib/std` ne modifie à travers un emprunt en lecture | examen automatique des sources (hypothèse à confirmer par revue) |
| Aucun attribut `noalias`/`readonly` dans l'IR | `grep` sur `nxc_codegen_llvm` |

## 4. Décisions à faire valider par l'utilisateur

1. **ADR 0011** parties 4 et 5 : panique = arrêt du processus comme engagement 1.0 ;
   règle de communication sur la sûreté.
2. **ADR 0012** : `Copy` déclaré pour les types publics à partir de l'édition 1.
3. **ADR 0013** : modèle de concurrence (concurrence structurée, `Transfer`/`Share`,
   canaux bornés, verrous et atomiques) ; noms à choisir.
4. Erreurs de `std.io` (A3) : énumération d'erreurs avant le gel de l'API.

Les invariants I1 à I4 de l'ADR 0013 et les conditions de l'ADR 0011 décrivent l'existant :
ils s'appliquent dès maintenant à toute contribution.

## 5. Transfert à Codex (implémentation, sans conflit de fichiers)

| Tâche | Fichiers | Critère d'acceptation |
|---|---|---|
| T1 sondes de pile (`"probe-stack"="inline-asm"`, x86-64/AArch64) | `compiler/nxc_codegen_llvm` | récursion avec tableau local de 4 Mio : arrêt propre ; coût mesuré (P13) |
| T2 message de dépassement de pile, sortie standard conservée | `lib/std/sys/linux.nx`, `windows.nx`, `rt.nx` | `tests/run` : récursion infinie → `stack overflow`, code 101 |
| T3 `wasm-ld --stack-first` | `compiler/nxc/src/link.rs` | `wasm32-wasi` : piège, statiques intactes ; suite web inchangée |
| T4 vérification de `bool`/`char` à l'entrée des `@export` | `compiler/nxc_codegen_llvm` | bibliothèque appelée depuis C avec `char` 0xD800 → panique |
| T5 sections critiques sans système | `lib/std/heap.nx`, `rt.nx`, `sys/bare.nx` | essai QEMU interruption + allocation |
| ~~T6 `tests/run` aussi en `-O2`~~ | fait par Claude (lot 2, §7) | — |
| ~~T7 diagnostics D1, D2~~ | fait par Claude (lot 2, §7) | — |
| ~~T8 `DropIfInit` résiduel = erreur interne~~ | fait par Claude (lot 2, §7) | — |

`places.rs` reste sous la responsabilité de Claude pour les règles ; Codex peut en
modifier les messages (T7) en le signalant.

## 5 bis. Revue des branches ouvertes de Codex Sol (17/10, lecture seule)

Aucune ne touche l'analyse, la NIR, le générateur ni le runtime : pas de conflit avec
cette branche.

- `refonte/sol-a1-benchmarks` : le rapport est honnête sur ses limites (une machine, un
  corpus, commit figé). Un point à vérifier avant toute communication : NEXUL y est
  **plus rapide que C** sur « Boucles + maths » (1,12×) avec toutes ses vérifications.
  Inspecter l'IR optimisée de ce noyau (`--emit llvm-ir`) pour confirmer que les
  vérifications de débordement sont présentes (contrat K3.4) et comparer les fonctions
  mathématiques utilisées (implémentation NEXUL contre la libm de C) : l'écart peut
  venir de là et non du code produit. Les cas « chaîne ASCII » (0,2×) et « vecteurs »
  (0,5×) sont les vraies pistes d'optimisation (vues de chaînes, copies d'agrégats).
- `refonte/sol-p9-edition-matrix`, `sol-vscode-release`, `sol-v030-package-validation`,
  `sol-l1-terminal-followup`, `sol-l2-dap-followup` : outils et distribution, hors du
  périmètre sémantique ; rien à signaler sur la sûreté du langage.

## 6. Rapport de progression

**A. Travaux réalisés** (branche locale `refonte/core-surete-semantique`, 3 commits de code
et de tests + documentation) :
- `compiler/nxc_sema/src/places.rs` : S1 (E0403 dans les gardes).
- `compiler/nxc_sema/src/{ty,defs,consts}.rs`, `check/finalize.rs` : S2 (largeur des
  cibles).
- `compiler/nxc/tests/suite.rs` : directive `// target:` des tests UI.
- Tests : `tests/ui/guard_move`, `edition1_guard_move`, `size_types_32_bit_target`,
  `exclusivity_closures` (`.nx` + `.stderr`).
- Documentation : ADR 0011, 0012, 0013, `docs/adr/selfhost-sh0.md`,
  `docs/spec/contrats-composants.md`, spec §3.1 et ch. 05 (§5.1, §5.2, §5.4, §5.6,
  §5.8, §5.9 nouveau), ce rapport.

**B. Validation** — commandes réellement exécutées :
- `cargo build --release -p nxc` : réussi.
- `cargo test --release -p nxc --test suite ui_diagnostics` : réussi (tous les
  `tests/ui`, dont les 4 nouveaux).
- `cargo test --release -p nxc --no-fail-fast` (après les changements) : voir le message
  de commit final et la PR ; échecs attendus uniquement pour « LLVM was not found ».
- `nxc check --target <cible>` sur `examples/bonjour.nx` pour les 14 cibles, sur tous les
  exemples et les 37 programmes de `tests/run` pour `wasm32-wasi` et `thumbv7em-none` :
  aucun diagnostic.
- **Non exécutés** : `tests/run`, `editions.rs`, `migrated_programs_behave_the_same`,
  `web.rs` (LLVM absent sur ce PC) ; à exécuter sur un PC avec LLVM avant fusion.

**C. Intégration** : aucune interface publique modifiée. `IntTy::bits()` garde son sens
(largeur maximale) ; nouvelles méthodes `bits_on/min_on/max_on` et
`Program::int_bits/int_min/int_max`. Un programme qui utilisait une constante
`usize` > 2³²−1 est maintenant refusé sur les cibles 32 bits (il était tronqué en
silence). Les branches ouvertes de Codex Sol ne touchent aucun de ces fichiers
(vérifié le 17/10).

**D. Risques et limites** : S3 à S6 non corrigés ; ADR 0012 et 0013 non décidées ;
comportement à l'exécution des corrections non vérifié faute de LLVM (les deux
corrections sont dans l'analyse et ne changent pas le code produit des programmes
acceptés).

**E. Prochaines étapes** : validation des ADR 0011 à 0013 par l'utilisateur ; tâches T1 à
T8 par Codex ; ADR de P5 sur les types distincts et contraints et les erreurs (A3, A4) ;
SH1 seulement après le gel de la grammaire de l'édition 1.

## 7. Lot 2 (17/10, branche `refonte/core-feuille-de-route`)

**A. Travaux réalisés**
- [Feuille de route vers la 1.0.0](../roadmap.md) : versions 0.3.0 à 1.0.0, objectifs,
  contenus, critères de sortie, dépendances, reports explicites. Remplace les anciennes
  sections « 0.3 Système » et « 0.4 », contraires au plan maître ; le tableau de
  conformité ne présente plus les microcontrôleurs comme ✅ sans leurs limites.
- ADR 0014 (types distincts et contraints, orientation des contrats) et ADR 0015
  (erreurs de `std`, conversion par `?`), proposées.
- D1 : la classification « itération précédente » d'E0402 suit maintenant les chemins
  du graphe (arêtes de retour) au lieu de l'ordre du texte ; un déplacement plus loin
  dans la même instruction est expliqué par l'ordre d'évaluation (spec §4.1).
- D2 : les conflits d'exclusivité nomment la variable capturée et la fonction anonyme.
- D3 / K3.7 : un `DropIfInit` résiduel arrête le générateur (erreur interne).
- T6 / K7.3 : nouveau test `run_programs_optimized` (toute la suite `tests/run` en `-O2`,
  mêmes sorties attendues).

**B. Validation** : voir la PR du lot. Sans LLVM sur ce PC, `run_programs_optimized`
n'est pas exécuté localement ; il l'est par le job Linux de la CI, qui a LLVM.

**C. Intégration** : fichiers de Claude seulement (`places.rs`, `ownership.rs`,
`suite.rs`, `func.rs` pour K3.7, docs). Les tâches T7 et T8 de la §5 sont faites ; la
§5 garde T1–T5 pour Codex.

**D. Limites** : si `run_programs_optimized` échoue en CI, c'est une information
précieuse (une optimisation change un comportement) : à analyser avant toute
désactivation.

## 8. Lot 3 (18/10, branche `refonte/core-robustesse`) — « le compilateur ne plante jamais »

La spec (§7.4) l'exige, mais **aucun test ne le vérifiait** (aucun fuzzing dans le
dépôt). Nouveau `compiler/nxc/tests/robustness.rs` :

- **mutations du corpus** (`tests/ui`, `tests/run`, `examples`, 231 fichiers) :
  troncature, suppression, duplication, échange de lignes, jeton inséré, mot remplacé ;
  dans les deux éditions, analysées par `nxc check` **et** jusqu'à l'IR LLVM
  (monomorphisation, génération : `Mode::Program`, sans la bibliothèque LLVM). Par défaut
  6 mutants par fichier (≈ 2 s) ; `NEXUL_FUZZ_ROUNDS`, `NEXUL_FUZZ_SEED` pour des
  campagnes. **Résultat** : 0 panique sur 11 campagnes de 231 fichiers × 200 mutants
  (≈ 500 000 compilations), dont 5 jusqu'à l'IR LLVM et 2 sur le code final de ce lot ;
- **sources pathologiques** (démontré avant correction) : 100 000 parenthèses
  imbriquées, une somme de 200 000 termes, 200 000 `{` non fermées **faisaient déborder
  la pile de `nxc`** (arrêt brutal, code 127, sur une pile de 256 Mio) → nouvelle
  limite **E0113** (1000 niveaux d'imbrication, 10 000 opérateurs à la suite) dans le
  parser commun aux deux éditions ;
- **sortie démesurée** (démontré) : 20 000 paramètres inutilisés sur une ligne de
  300 Ko produisaient **7,4 Go** de messages (chaque avertissement citait la ligne
  entière) ; écrits dans un tube, `nxc` paniquait. Les extraits montrent maintenant une
  fenêtre de 160 colonnes autour de l'étiquette : 7,2 Mo. Une sortie fermée
  (`nxc check … | head`) arrête `nxc` sans rapport de plantage.

**Constat pour Codex (performance, non corrigé)** : l'analyse d'un `match` est
**quadratique** en nombre de bras : 1000 bras 0,2 s, 4000 bras 1,1 s, 8000 bras 4,0 s,
50 000 bras plus de 2 min (énumération ou entiers, mesuré avec `nxc check`). Probablement
la recherche des bras inaccessibles (W0002), qui compare chaque bras aux précédents.
Critère : 50 000 bras en moins d'une seconde.

## 9. Lot 5 (18/10, branche `refonte/core-perf-match`) — temps d'analyse des grandes fonctions

En cherchant pourquoi un `match` de 8000 bras prenait 4 s (§8), un cas plus grave est
apparu : **20 000 `if` à la suite, chacun avec un `println`, dans une seule fonction :
plus de 10 minutes** pour `nxc check` (2000 : 7 s). Du code engendré (analyseurs,
tables, tests) a cette forme. Trois causes, corrigées :

1. `ownership.rs` : chaque initialisation parcourait **tous** les sites de déplacement de
   la fonction pour oublier ceux de la variable → index des sites par variable.
2. `ownership.rs` : la liste de travail du flot de données était traitée en pile : sur un
   grand graphe sans boucle, les blocs étaient revisités sans cesse → ordre postfixe
   inverse (chaque bloc une seule fois sans boucle ; même point fixe, mêmes diagnostics).
3. `exhaust.rs` : chaque bras était comparé à tous les précédents par l'algorithme
   général → chemin rapide pour les motifs plats (littéraux, booléens, variantes sans
   sous-motif ou avec `_`), en temps constant par bras ; algorithme général inchangé
   pour le reste.

| Fonction (release) | Avant | Après |
|---|---|---|
| 2000 `if` + `println` | 7,0 s | 0,18 s |
| 20 000 `if` + `println` | > 10 min | 3,7 s |
| `match` de 8000 bras entiers | 4,0 s | 0,6 s |
| `match` de 8000 variantes | 4,0 s | 0,4 s |

Reste (non corrigé, cas extrêmes) : 50 000 bras prennent encore 9 à 43 s ; le coût
restant vient des ensembles de bits copiés à chaque bloc (O(blocs × variables)). Test de
non-régression : `large_generated_functions_are_analyzed_quickly` (borne large de 60 s,
1,2 s mesurées en debug).

## 10. Lot 6 (18/10, branche `refonte/core-copy-et-never`) — deux plantages du générateur, `Copy`

En écrivant les tests de `T: Copy`, deux **plantages du générateur de code**, présents sur
`main`, sont apparus (démontrés, `nxc build` s'arrête sur une panique avant même LLVM) :

- **`println(7)`** : `print`/`println` vérifiaient leur argument contre le type erreur ;
  un littéral ou une variable inférée (`let x = 7`) prenait silencieusement le type
  `{error}` (`nxc check` muet), puis le générateur paniquait, sur toutes les cibles. La
  forme `println(v)` de la spec (§4.10) n'était testée qu'avec des `String`. Corrigé
  (variable de type neuve), test `tests/run/print_values.nx`.
- **argument ou opérande divergent** : `f(1, panic("…"))`, `v.push(return)`,
  `1 + return` faisaient paniquer le générateur (la « valeur » d'une expression `Never`
  dans un bloc inaccessible). Trouvé par le test de robustesse grâce à une nouvelle
  mutation. Corrigé : les blocs inaccessibles ne sont plus générés (contrat K3.10), test
  `tests/run/panic_in_argument.nx`.

Leçon : la campagne de mutations du lot 3 ne produisait jamais `println(7)`, parce que le
corpus n'en contient pas. Nouvelle mutation : une chaîne littérale remplacée par un
nombre, un caractère, un booléen ou un nom. Tous les programmes de `tests/run` passent
maintenant la génération de l'IR (sans LLVM) sans panique.

**Langage** : `Copy` devient une interface du prélude, satisfaite par la structure du
type, et une contrainte générique (`T: Copy`, `T is Copy`, l'exemple de l'ADR 0010
compile). Points 1 et 5 de l'ADR 0012 ; la déclaration obligatoire pour les types publics
attend la décision.

**Correction** : les rapports et la description de la PR #35 parlaient de « 90
programmes de `tests/run` » ; `tests/run` en contient 37 sur `main` (90 comptait tous les
fichiers, `.out` et `.exit` compris). Corrigé dans ce rapport et dans la PR.

## 11. Lot 7 (18/10, branche `refonte/core-couverture`) — couverture des diagnostics

24 des 126 codes de diagnostic n'étaient produits par **aucun** test de `tests/ui`, dont
des règles centrales : cohérence des implémentations (E0243, E0244, E0245, E0247),
traits utilisables par `dyn` (E0246), visibilité entre modules (E0226, E0227, E0223),
`@export` (E0504), tableaux `[x; n]` (E0313, E0373), et la plupart des erreurs lexicales.
Ajout de 10 tests (`traits_coherence`, `traits_names_and_dyn`, `privacy_modules`,
`arrays_and_export`, `lex_more_errors`, `items_and_assign`,
`module_declaration`, `pattern_expected`, `nesting_limit` ; E0001 est testé en mémoire par `invalid_utf8_is_reported`, car `tests/ui` doit rester en UTF-8 valide pour `studio/syntax`) et du test
`every_code_has_a_ui_test`, qui l'impose désormais (seule exception : l'erreur interne
E0900). Les fils de la suite compilent sur une pile de 256 Mio, comme `nxc` (K6.3).

Défauts vus en écrivant ces tests (lus et reproduits, non corrigés, pour Codex) :

- **D4, erreurs en double** (**corrigé au lot 9**) : `privacy.secret()` sur une fonction
  privée donnait E0227 **et** E0222 pour la même faute (spec §7.4 : pas d'erreur en
  cascade).
- **D5, `module` du fichier racine non vérifié** (**corrigé au lot 9**) : une déclaration
  `module autre.nom` dans le fichier passé à `nxc` était acceptée sans E0220, contrairement
  à la spec §6.1.

## 12. Lot 8 (18/10, branche `refonte/core-pile-linux`) — dépassement de pile sur Linux

Tâche T2 de l'ADR 0011, pour Linux x86-64 et AArch64. Sur ces cibles, une récursion
trop profonde arrêtait le processus par `SIGSEGV`, sans message et en perdant la sortie
standard tamponnée.

- **Runtime** (`lib/std/sys/linux.nx`) : au démarrage, les bornes de la pile principale
  sont relevées (pointeur de pile initial, `RLIMIT_STACK` par `prlimit64`), puis un
  gestionnaire de `SIGSEGV` et `SIGBUS` est installé sur une pile de secours de 64 Kio
  (`sigaltstack`, `rt_sigaction`). Une faute dans la pile ou juste sous elle (zone de
  garde) se termine comme une panique : sortie standard vidée,
  `panic: stack overflow (the recursion is probably too deep)`, code 101. Toute autre
  faute (possible seulement par `unsafe` ou une fonction étrangère) reprend l'action par
  défaut du système (`SA_RESETHAND`, puis le signal est relevé).
- **Générateur** : point d'entrée C `nx_linux_fault`, installé par `nx_linux_start`
  avant `main`. Test structurel `compiler/nxc/tests/codegen_ir.rs`, qui tourne sans LLVM.
- **Test d'exécution** : `tests/run/stack_overflow.nx` (Linux seulement, exécuté par la
  CI en debug et en `-O2`).
- **Limites** : RISC-V (structure `sigaction` différente), macOS et Windows inchangés ;
  la pile des futurs threads devra installer sa propre pile de secours (ADR 0013) ; si
  la limite de pile est infinie (`ulimit -s unlimited`), la faute n'est pas reconnue et
  le comportement reste celui du système.

## 13. Lot 9 (24/10, branche `refonte/core-diagnostics-d4-d5`) — D4 et D5

- **D4** : une fonction libre privée d'un autre module n'est plus signalée qu'une fois,
  là où son nom est cherché (E0222, chemin `module.f` ou `import`). E0227 reste pour les
  méthodes et fonctions associées privées, que la recherche dans un module ne voit pas
  (`tests/ui/privacy_modules.nx` couvre maintenant une méthode privée).
- **D5** : le module racine porte le nom de son fichier ; une déclaration `module` qui
  ne correspond pas donne E0220 comme pour les autres modules
  (`tests/ui/root_module_declaration.nx`). Une déclaration conforme reste acceptée.


## 14. Lot 10 (24/10, branche `refonte/core-try-deplacement`) — `?` sur une place empruntée

- **Défaut démontré (sûreté mémoire)** : `?` lisait son opérande sans le prendre. `r?` sur
  un paramètre emprunté, `s.f?` ou `v[i]?` déplaçaient donc une ressource hors d'une place
  que la fonction ne possédait pas : la valeur était détruite deux fois (par la fonction,
  puis par son propriétaire). Les quatre fonctions de `tests/ui/try_moves.nx` étaient
  acceptées sans erreur.
- **Correction** : `x?` est une position de déplacement (`compiler/nxc_sema/src/places.rs`),
  comme `let v = x`. Une place empruntée, un champ ou un élément donnent E0403 (avec
  l'aide `.take()` pour un `Option`) ; un paramètre `owned`, une variable locale, un appel
  ou une charge utile `Copy` restent acceptés. Un second `r?` après le premier donne E0402.
- **Test** : `tests/ui/try_moves.nx` (cas refusés et cas acceptés).

## 15. Lot 11 (24/10, branche `refonte/core-adr0015-erreurs`) — ADR 0015 mise en œuvre

- **Erreurs de `std.io`** : `io.IoError` remplace `String` dans les cinq fonctions
  faillibles ; la couche système rend le code d'erreur du système au lieu d'un booléen,
  ce qui distingue « absent » de « refusé » sans comparer de textes (constat A3).
- **`?` convertit** dans la variante qui contient exactement l'erreur, dans `dyn Error`
  (`any Error`), ou, en édition 0.2 seulement, en texte. Rien d'autre (E0360 enrichi).
- **Migration** : `nxc migrate` signale les `?` concernés ; le test des programmes migrés
  exclut le seul programme qui teste exprès la règle de transition (`// no-migrate:`).
- Détails, fichiers et limites : [ADR 0015, « Mise en œuvre »](../adr/0015-erreurs-et-conversion.md).

## 16. Lot 12 (24/10, branche `refonte/core-adr0012-copy`) — ADR 0012 mise en œuvre

- **Copiabilité des types publics** : en édition 1, un type `pub` n'est copié que s'il
  déclare `Copy` ; la déclaration est vérifiée (E0250). L'édition 0.2 ne change pas.
- **Parseur de l'édition 1** : `type T is Copy = .A | .B` ; les fonctions de
  `type T is Copy { … }` restent celles du type (avant : rattachées à `Copy`, E0241).
- **Migration** : `nxc migrate` ajoute `is Copy` ; E0402 propose la déclaration.
- `random.Rng` laissé sans déclaration (décision à prendre à la migration de `std`).
- Détails : [ADR 0012](../adr/0012-copiabilite-explicite.md).

## 17. Lot 13 (24/10, branche `refonte/core-adr0014-types`) — ADR 0014 mise en œuvre

- **Types distincts et contraints** (édition 1) : `distinct` et `where` ne sont plus
  refusés (E0111). Un tel type est une structure à un champ caché ; conversions par `as`,
  littéraux vérifiés à la compilation (E0252), `as` vérifié à l'exécution avec la position
  exacte, `T.check(x)`, interfaces de la base réutilisées par leur nom.
- **Aucun changement du générateur** : la disposition est celle d'une structure à un
  champ, de même taille que la base (`mem.size_of` testé).
- Restent : C4 (`@export`), types distincts génériques, constantes nommées dans les
  conditions. Détails : [ADR 0014](../adr/0014-types-distincts-et-contraints.md).

## 18. Lot 14 (24/10, branche `refonte/core-c4-exports`) — C4 : valeurs reçues de C

- **Défaut** : un `bool` différent de 0/1 ou un `char` invalide (0xD800, > 0x10FFFF) passé
  par C (ou par l'hôte JavaScript) à une fonction `@export` cassait les invariants (UTF-8
  des `String`, filtrage exhaustif). Un `bool` reçu comme `i1` était même réinterprété en
  silence (seul le bit de poids faible comptait).
- **Correction** : le symbole exporté d'une telle fonction est une enveloppe qui reçoit un
  `bool` comme octet et vérifie `bool`, `char` et types contraints (ADR 0014 §2.7), puis
  appelle la fonction ; le code NEXUL appelle la fonction directement. Nouveau cas de
  panique 15 (`bool`) dans `rt.panic_arith`.
- **Tests** : `codegen_ir.rs` (structure, trois cibles, exécuté partout) ;
  `tests/web/interop.mjs` (comportement, exécuté seulement à la main : LLVM, wasm-ld, Node).
- `compiler/nxc_codegen_llvm` est partagé avec Codex : seules `instance()` (nom de
  l'export) et la fin de `emit_fn` changent, loin des sections modifiées par #44, #46, #51.

## 19. Lot 15 (25/10, branche `refonte/core-adr0013-proprietes`) — ADR 0013, propriétés des tâches

- L'utilisateur a validé les noms de l'ADR 0013 et les threads du système d'abord.
- **`Transfer` et `Share`** : interfaces du prélude satisfaites par la structure (comme
  `Copy`) ; `Ptr` et les fonctions anonymes n'en ont aucune ; un type qui contient un
  `Ptr` les déclare par `unsafe impl` (nouvelle syntaxe, et `unsafe extend` en édition 1),
  sinon E0253. `String`, `Vec`, `Location` les déclarent.
- Rien n'exécute encore de tâche : ces propriétés seront exigées par `std.tache` (lot 17)
  et les canaux (lot 18). Plan des lots : [ADR 0013, « Mise en œuvre »](../adr/0013-fondations-de-la-concurrence.md).

## 20. Lot 16 (25/10, branche `refonte/core-adr0013-threads`) — tâches de `std.tache`

- **Threads** : Linux par `clone` (assembleur en ligne pour x86-64, AArch64, RISC-V ; pile
  de 8 Mio, garde, pile de signal par thread : un débordement de pile dans une tâche
  finit comme une panique), Windows par `CreateThread` ; ailleurs, chaque tâche
  s'exécute quand elle est lancée.
- **Runtime sûr entre threads** : la tâche C5 de Codex (#51) a été fusionnée dans ce lot ;
  son point de garde `rt.critical_section()` devient, sur les cibles avec threads, un
  verrou réentrant (identité du thread, profondeur) actif dès le premier thread. Entrée
  standard sous un verrou à part. Table paresseuse de `math` sous le verrou.
- **`std.tache`** : `portee` et `lancer` ; règles E0254 ; gel des captures par tâche
  jusqu'à la fin de la `portee` (E0420) ; `Share` / `Transfer` exigés des captures.
- Opérations atomiques (`mem.atomic_*`) et intrinsèques des fonctions anonymes dans le
  générateur (partagé avec Codex : sections nouvelles, aucune modification de ses lignes).
- Reste de l'ADR 0013 : canaux, `Verrou`, `Atomique`, `lancer_detachee` (lot 18).

## 21. Lot 18 (25/10, branche `refonte/core-adr0013-canaux`) — canaux, verrous, atomiques, tâches détachées

- **Canaux bornés** : `tache.canal[T](n)` → `Emetteur` / `Recepteur` ; attente quand le
  canal est plein ou vide (futex sur Linux) ; `None` quand tous les émetteurs sont fermés
  ou détruits ; poignées clonables, `Transfer` et `Share`.
- **`Verrou`** (`avec`), **`Atomique`** (entiers, séquentiellement cohérentes),
  **`lancer_detachee`** / `Tache.rejoindre` : une fonction nommée et des données
  déplacées, exécutées par un `dyn` interne (le point d'entrée d'un thread n'est pas
  générique).
- La CI du lot 16 avait trouvé un IR invalide que le test structurel ne voyait pas : un
  script de contrôle des types d'opérandes est désormais passé sur l'IR de cinq cibles
  avant de pousser (zéro incohérence pour ce lot).
- ADR 0013 : toutes les parties sont mises en œuvre ; restent `Atomique[bool]`, les ordres
  mémoire faibles et l'attente par `WaitOnAddress` sous Windows.
