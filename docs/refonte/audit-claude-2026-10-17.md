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
| A3 | moyenne | lu | Erreurs de la bibliothèque standard en `String` (`io.read_file` → `Result[String, String]`) et `?` sans conversion d'erreur : les changer après la 1.0 sera incompatible | §2.3 |
| A4 | moyenne | lu | Types distincts et contraints (`distinct`, `where it in …`) réservés sans sémantique des opérations | §2.4 |
| A5 | moyenne | lu | Monomorphisation et tables `dyn` dans le générateur LLVM : un second backend devrait les refaire | §2.5 |
| S6 | moyenne | lu | `@export` accepte `bool` et `char` venant de C sans vérification : un `char` invalide casse l'invariant UTF-8 de `String` | ADR 0011 T4 (Codex) |
| T1 | moyenne | lu | `tests/run` exécuté seulement en debug (`BuildOptions::new(…, false)`) : rien ne vérifie les garanties en `-O2` | contrat K7.3 (Codex) |
| D1 | basse | démontré | E0402 annonce « moved here, in a previous iteration of the loop » sans boucle (`f(s, s)`, arguments en deux phases) | §2.6 (Codex) |
| D2 | basse | démontré | Conflit d'exclusivité par capture : « this value is passed twice… », sans nommer la variable capturée | §2.6 (Codex) |
| D3 | basse | lu | Le générateur détruit inconditionnellement un `DropIfInit` résiduel au lieu de signaler une erreur interne | contrat K3.7 (Codex) |
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
  bibliothèque standard, les exemples et les 90 programmes de `tests/run` passent `nxc
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
| T6 `tests/run` aussi en `-O2` | `compiler/nxc/tests/suite.rs` | suite verte en debug **et** release |
| T7 diagnostics D1, D2 | `compiler/nxc_nir/src/ownership.rs`, `nxc_sema/src/places.rs` (messages seulement) | `tests/ui` mis à jour |
| T8 `DropIfInit` résiduel = erreur interne | `compiler/nxc_codegen_llvm/src/func.rs` | suite verte |

`places.rs` reste sous la responsabilité de Claude pour les règles ; Codex peut en
modifier les messages (T7) en le signalant.

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
  exemples et les 90 programmes de `tests/run` pour `wasm32-wasi` et `thumbv7em-none` :
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
