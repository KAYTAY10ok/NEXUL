# 0013 — Fondations de la concurrence

Auteur : Claude (architecture, sémantique, sûreté). **Statut : proposée** (17/10/2026).
Lot P5 (conception) avant P7 (implémentation). Rien n'est implémenté. Les **invariants**
de la partie 1 protègent dès maintenant les choix futurs ; le **modèle** des parties 2 à 6
demande la validation de l'utilisateur.

## Problème

La passation V3 vise une « concurrence sûre » et l'ADR 0010 réserve déjà `spawn { … }`,
`channel[T](capacite: n)`, `===` sur les tâches et les canaux, et un type `Shared[T]`.
Aucune sémantique n'existe. Or plusieurs décisions déjà possibles aujourd'hui rendraient
une concurrence sans course de données **impossible à garantir plus tard** : un type de la
bibliothèque standard qui se modifie à travers un emprunt en lecture, un `Shared[T]` qui
autorise la modification de son contenu, des variables statiques lisibles sans `unsafe`.
Il faut fixer ces limites avant d'écrire la moindre primitive.

## Contexte (vérifié le 17/10 sur `main`)

- Propriétaire unique, références de seconde classe, fonctions anonymes non échappantes
  (E0248), loi d'exclusivité étendue aux captures (`tests/ui/exclusivity_closures.nx`).
- `static var` exige `unsafe` (E0501) ; une `static` ne contient pas de ressource (E0374).
- Aucune mutabilité intérieure dans le code sûr : aucune méthode `self` (emprunt en
  lecture) de `lib/std` n'écrit à travers un pointeur ou dans une statique (examen
  automatique des sources du 17/10, à confirmer par revue).
- Il existe **déjà** une concurrence : les gestionnaires d'interruption des cibles sans
  système (`@export("SysTick_Handler")`), qui interrompent le code principal ; le runtime
  n'y est pas réentrant (ADR 0011, condition C4).
- Une panique arrête le processus (ADR 0011 §4).

Conséquence clé : **dans du code sûr, un emprunt en lecture est profondément immuable**
et un emprunt `inout` est exclusif. C'est exactement ce qu'il faut pour partager des
données entre tâches sans course — à condition de ne jamais introduire de mutabilité
intérieure sans la marquer.

## Solutions envisagées

| | Modèle | Pour | Contre |
|---|---|---|---|
| A | Threads libres + verrous, comme C/Java | familier | courses de données non vérifiables avec le modèle actuel |
| B | Acteurs seuls (messages, aucune mémoire partagée) | sûr, simple | copies coûteuses, pas de parallélisme de données |
| C | `Send`/`Sync` et références à durée de vie, comme Rust | complet | exige des durées de vie que NEXUL a refusées (ADR 0003) |
| **D** | **Concurrence structurée** (tâches liées à une portée, captures par emprunt vérifiées par la loi d'exclusivité) + **tâches détachées par transfert de propriété** + canaux ; deux propriétés de types, `Transfer` et `Share` | réutilise les règles existantes ; aucune durée de vie ; parallélisme de données sûr | les tâches détachées ne capturent rien par emprunt |

## Décision proposée : D

### 1. Invariants applicables dès maintenant (à respecter par toute contribution)

- **I1. Pas de mutabilité intérieure dans le code sûr.** Aucune API sûre ne modifie une
  valeur à travers un emprunt en lecture (`self`, paramètre sans convention). Un futur
  type qui le fait (compteur atomique, verrou, cellule) le déclare avec la propriété
  `Share` explicite (partie 2), dans du code `unsafe` relu.
- **I2. `static var` reste `unsafe`.** Seule exception future : une `static` d'un type
  `Share` (atomique, verrou) pourra être utilisée sans `unsafe`.
- **I3. Pas de propriété partagée modifiable.** `Shared[T]` (ADR 0010), s'il est ajouté,
  ne donne que des emprunts en **lecture** de son contenu ; la modification passe par un
  contenu `Share` (verrou, atomique). `===` n'a de sens que pour ces valeurs partagées et
  pour les poignées de tâches et de canaux ; `any Interface` est une valeur à propriétaire
  unique, `===` y est refusé tant que cette ADR n'est pas étendue.
- **I4. Une panique arrête tout le processus** (ADR 0011 §4), tâches comprises.

### 2. Deux propriétés de types (noms à valider)

- **`Transfer`** : une valeur de ce type peut être **déplacée** vers une autre tâche.
- **`Share`** : un **emprunt en lecture** de ce type peut être utilisé par plusieurs tâches
  à la fois.

Elles sont **structurelles** (un type est `Transfer` si tous ses champs le sont), sauf :
`Ptr[T]` n'est ni `Transfer` ni `Share` ; un type qui contient un `Ptr` (directement ou
non) doit les **déclarer** dans un bloc `unsafe` relu (`Vec[T]`, `String`, `Map`
déclarent `Transfer` si `T` l'est, `Share` si `T` l'est). Elles s'écrivent comme des
contraintes (`T is Transfer`) mais n'ont pas de méthode.

Pourquoi deux propriétés : grâce à I1, presque tous les types sont `Share` ; seuls les
types à mutabilité intérieure non synchronisée (une future cellule mono-tâche) ou à
pointeur non protégé ne le sont pas. Elles coûtent peu à inférer et évitent les règles
de durée de vie.

### 3. Tâches liées à une portée (cas principal)

Forme illustrative (syntaxe définitive dans P6) :

```
tache.portee(fn(p) {
    p.lancer(fn() { somme_gauche = somme(gauche) })
    p.lancer(fn() { somme_droite = somme(droite) })
})            // attend toutes les tâches avant de rendre la main
```

- La fonction reçue par `portee` est non échappante (E0248) et `portee` attend toutes les
  tâches lancées : les captures par emprunt restent valides.
- **Règle d'exclusivité entre tâches** (extension de la règle 4 de la spec §5.4) :
  chaque fonction passée à `p.lancer` gèle ses captures jusqu'à la fin de la portée,
  en écriture (exclusif) pour les captures modifiées, en lecture sinon. Deux tâches ne
  peuvent donc pas modifier la même place, ni l'une la lire pendant que l'autre la
  modifie ; le code de la portée lui-même ne peut plus y toucher. C'est la machinerie des
  places gelées de `nxc_sema/src/places.rs`, pas une nouvelle analyse.
- Une capture en lecture exige `Share`, une capture modifiée exige `Transfer`.

### 4. Tâches détachées et canaux

- `tache.lancer_detachee(travail, owned donnees)` : une **fonction nommée** et des
  arguments **déplacés**, qui doivent être `Transfer`. Aucune capture par emprunt ne
  peut s'échapper (les fonctions anonymes restent non échappantes).
- `canal[T is Transfer](capacite: n)` : canal **borné** ; `envoyer` déplace la valeur,
  `recevoir` rend `Option[T]` (`None` quand tous les émetteurs sont détruits).
- `rejoindre()` rend la valeur de la tâche ; pas de `Result` pour les paniques (I4).

### 5. Mémoire partagée explicite

`Verrou[T is Transfer]` (`Share`, accès par une fonction anonyme non échappante :
`v.avec(fn(x!) { … })`, ce qui évite les gardes stockées), `Atomique[i32]`, `Atomique[u64]`,
`Atomique[bool]`, `Atomique[usize]`. Modèle mémoire des atomiques : celui de LLVM
(C++20) restreint à `Relaxed`, `Acquire`, `Release`, `AcqRel`, `SeqCst` (défaut
`SeqCst`). Une course de données n'est atteignable qu'en `unsafe`.

### 6. Interruptions (cibles sans système)

Un gestionnaire d'interruption est une tâche préemptive : il ne peut utiliser sans
`unsafe` que des statiques `Share` (atomiques). Le profil temps réel (P8, `with
noalloc, nonblocking`) vérifiera qu'il n'alloue pas et ne bloque pas. D'ici là, la
condition C4 de l'ADR 0011 s'applique.

## Conséquences

- Positives : aucune annotation de durée de vie ; le parallélisme de données (diviser un
  `Vec` en tâches) est sûr et vérifié par les règles déjà connues des utilisateurs ; les
  canaux couvrent les serveurs et les pipelines.
- Négatives : pas de tâche détachée qui emprunte ; pas de `Shared[T]` modifiable sans
  verrou ; `Transfer`/`Share` à déclarer pour les types à `Ptr` de la bibliothèque.
- Dette évitée : I1 à I3 empêchent dès maintenant d'ajouter une API qui rendrait la
  partie 3 fausse.

## Composants concernés

`compiler/nxc_sema` (propriétés, contraintes, gel par tâche), `lib/std` (déclarations
`Transfer`/`Share`, module `tache`, canaux, verrous, atomiques), `lib/std/sys/*`
(threads du système, futex/`WaitOnAddress`), `compiler/nxc_codegen_llvm` (atomiques,
TLS si nécessaire), `docs/spec/05-memoire.md`, nouveau chapitre de spécification.

## Tests et preuves attendus (P7)

- Corpus négatif (`tests/ui`) : deux tâches qui modifient la même variable ; lecture
  pendant une modification ; capture d'un type non `Share` ; envoi d'un type non
  `Transfer` ; `===` sur une valeur simple.
- Corpus positif (`tests/run`) : somme parallèle, producteur/consommateur borné,
  arrêt quand tous les émetteurs sont détruits, panique dans une tâche → code 101.
- Stress : 10⁶ messages, 64 tâches, résultats déterministes vérifiés ; exécution sous
  ThreadSanitizer sur Linux quand la chaîne le permet (sinon « non testé »).

## Risques et questions ouvertes

- Ordonnanceur : threads du système d'abord (simple, prévisible) ; tâches légères
  (`async`) seulement après mesures. À trancher dans P7.
- Coût de la règle « gel jusqu'à la fin de la portée » si la portée est longue :
  à mesurer sur le corpus.
- Noms définitifs (`Transfer`, `Share`, `portee`, `lancer`) : P6 et l'utilisateur.
