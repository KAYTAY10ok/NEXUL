# 05 — Modèle mémoire : sémantique de valeurs mutables

Ce chapitre répond à l'exigence centrale du cahier des charges (§5, §11.1) : une gestion
**déterministe, sûre par défaut, sans ramasse-miettes**, avec un contrôle explicite des
ressources et des pointeurs bruts confinés aux contextes `unsafe`.

## 5.1 Principe

NEXUL adopte la **sémantique de valeurs mutables** (*mutable value semantics*, étudiée
notamment par les langages Hylo/Val et Swift) :

1. **Toute valeur a un unique propriétaire** : une variable, un champ ou élément d'une
   autre valeur, ou un temporaire.
2. **Les références sont de seconde classe** : on peut *emprunter* une valeur le temps
   d'un appel, d'une boucle ou d'un bras de `match`, mais une référence ne peut **jamais**
   être stockée dans une variable, un champ ou renvoyée par une fonction.
3. **Exclusivité** : pendant qu'une valeur est empruntée en écriture (`inout`), rien
   d'autre ne peut y accéder ; pendant qu'elle est empruntée en lecture, rien ne peut la
   modifier.

Conséquence : **aucune annotation de durée de vie n'est nécessaire**. Une référence ne
survit jamais à son référent puisqu'elle ne peut pas s'échapper de la construction qui
l'a créée. La sûreté est obtenue avec des règles locales, vérifiables fonction par
fonction, et des messages d'erreur simples.

Comparaison :

| | Rust | Swift | C/C++ | **NEXUL** |
|---|---|---|---|---|
| Sûreté mémoire sans GC | oui | ARC (comptage) | non | **oui** |
| Annotations de durée de vie | oui | non | non | **non** |
| Destruction déterministe | oui | oui (ARC) | manuelle/RAII | **oui** |
| Coût caché (compteurs, copies sur écriture) | non | oui | non | **non** |
| Références stockées | oui | (classes) | oui | **non** (seconde classe) |

La limite assumée : les structures à références partagées (graphes, listes doublement
chaînées) s'écrivent avec des indices dans un `Vec`, une arène, ou du code `unsafe`
encapsulé. *(À venir : projections et sous-scripts nommés ; un type à propriété partagée
(`Shared[T]`, ADR 0010) ne sera introduit qu'avec les règles de l'ADR 0013.)*

## 5.2 Copie et déplacement

Les types se divisent en **copiables** et **ressources** (§3.7).

```nexul
let a = 5
let b = a            // copie : a reste utilisable

let s = String.from("bonjour")   // ou simplement "bonjour"
let t = s            // déplacement : s n'est plus initialisée
println(s)           // erreur E0402 : utilisation de `s` après déplacement
                     //   note : `s` a été déplacée ici (ligne précédente)
```

Une ressource est **déplacée** lorsqu'une place qui la contient apparaît en **position
possédante** :

- initialisation d'une variable (`let`, `var`) et membre droit d'une affectation ;
- argument d'un paramètre `owned` ; receveur d'une méthode `owned self` ;
- valeur renvoyée (`return`) ;
- opérande d'une construction (structure, variante, tuple, tableau) ;
- opérande de `move`.

Seule une **variable locale entière** (ou un paramètre `owned`) peut être déplacée.
Déplacer un champ, un élément ou un paramètre emprunté est l'erreur E0403 ; le message
propose `move` de la variable entière, `Option.take()` ou une copie explicite
(`.clone()` pour `String`).

Après un déplacement, la variable est **non initialisée** : la lire est une erreur
(E0402), mais on peut lui réaffecter une valeur (si elle est `var`). Un déplacement
conditionnel (dans une seule branche d'un `if`) rend la variable « peut-être
déplacée » : la lire est une erreur, et sa destruction en fin de portée est gérée par un
**drapeau de destruction** à l'exécution. Une variable déclarée hors d'une boucle peut
être déplacée dans la boucle seulement si elle reçoit une nouvelle valeur avant toute
lecture suivante, y compris au début de l'itération suivante et après la boucle ;
sinon c'est l'erreur E0402 (« déplacée à une itération précédente »). L'analyse suit
les chemins d'exécution sur la NIR (§10.2), pas la seule structure du texte.

## 5.3 Conventions de passage des paramètres

| Déclaration | Appel | Sémantique |
|---|---|---|
| `x: T` | `f(a)` | **emprunt en lecture** (défaut). `f` ne peut ni modifier ni déplacer `x`. Aucune copie de ressource. |
| `x: inout T` | `f(&a)` | **emprunt en écriture**. `a` doit être une place modifiable ; les modifications de `f` sont visibles après l'appel. |
| `x: owned T` | `f(a)` | **transfert de propriété**. Une ressource est déplacée dans `f`, qui la détruira ou la transmettra. Dans `f`, `x` est modifiable. |

Le `&` à l'appel est **obligatoire** pour `inout` (E0411) et interdit ailleurs (E0412) :
le lecteur voit immédiatement quelles variables un appel peut modifier.

Un argument emprunté peut être une valeur temporaire (`f(fabriquer())`) : le temporaire
vit jusqu'à la fin de l'instruction. Un argument `inout` doit être une place modifiable :
variable `var`, paramètre `inout`, receveur `inout self`, ou un champ ou élément de
celles-ci (E0410). Seule exception : le **receveur** d'une méthode `inout self` peut être
une valeur temporaire (`texte.chars().pop()`) ; le temporaire est modifié puis détruit à la
fin de l'instruction.

Pour le receveur des méthodes : `self` (emprunt en lecture), `inout self` (emprunt en
écriture, receveur implicite), `owned self` (consomme le receveur).

**Réalisation** : le compilateur choisit librement le mécanisme (valeur ou adresse) ;
la sémantique ne dépend pas de ce choix. Aujourd'hui, les emprunts de types copiables
de petite taille passent par valeur et les autres par adresse.

## 5.4 Loi d'exclusivité

**Règle E0420.** Deux accès simultanés à des places qui se recouvrent sont interdits
si l'un d'eux est une écriture. Concrètement, le compilateur vérifie :

1. **Arguments d'un même appel.** Si un argument `inout` désigne la place P, aucun autre
   argument du même appel ne peut désigner une place qui recouvre P. Deux places se
   recouvrent si elles ont la même racine et que le chemin de l'une est un préfixe de
   l'autre ; deux accès indexés au même tableau se recouvrent toujours
   (`echanger(&v[i], &v[j])` est refusé ; on écrit `v.swap(i, j)`).
2. **Boucles.** Pendant `for x in c`, la place `c` est **gelée** : elle ne peut être ni
   affectée, ni déplacée, ni passée `inout`, ni modifiée par une méthode `inout self`.
3. **Filtrage.** Quand le scrutin d'un `match` (ou d'un `if let`) est une place P :
   - pendant l'évaluation d'une **garde** `if`, P est toujours gelée (les bras suivants
     sont testés à travers la même place) ;
   - pendant le corps d'un bras dont le motif **emprunte** une partie de P (une liaison
     d'un type ressource), P est gelée de la même façon. Les liaisons de types copiables
     sont des copies : elles ne gèlent rien, et `match n { Some(k) => n = Some(k + 1) }`
     est valide.

```nexul
var v = Vec.new()
v.push(1)
for x in v {
    v.push(x)        // erreur E0420 : `v` est empruntée par la boucle `for` (ligne 3)
}
```

4. **Fonctions anonymes.** Une fonction anonyme utilise sur place les variables qu'elle
   capture (§4.12) : chaque capture compte comme un accès (lecture, ou écriture si la
   fonction la modifie) **pendant tout l'appel** qui reçoit la fonction. Les règles 1 à 3
   s'y appliquent : capturer une variable passée `inout` au même appel, la modifier
   pendant qu'un autre argument l'emprunte ou pendant qu'une boucle ou un filtrage la
   gèle, ou la modifier dans deux fonctions anonymes du même appel est l'erreur E0420
   (test de référence `tests/ui/exclusivity_closures.nx`).

Comme une référence ne peut pas être stockée et qu'une fonction anonyme ne peut pas
survivre à l'appel qui la reçoit (E0248), ces quatre règles suffisent : il n'existe
aucun autre moyen d'obtenir deux chemins vers la même mémoire dans du code sûr. Les
variables statiques modifiables (§5.9) sont l'exception, d'où leur accès en `unsafe`.

## 5.5 Destruction (RAII)

- Une valeur ressource est **détruite exactement une fois** : quand son propriétaire
  sort de portée sans l'avoir cédée, quand une variable qui la contient est réaffectée
  (l'ancienne valeur est détruite juste avant l'écriture), ou quand un temporaire
  arrive en fin de vie.
- **Ordre** : les variables d'une portée sont détruites dans l'**ordre inverse** de leur
  déclaration ; les temporaires d'une instruction à la fin de celle-ci, dans l'ordre
  inverse de leur création ; les champs d'une structure dans l'ordre de déclaration,
  après la méthode `drop` de la structure ; les éléments d'un tableau ou d'un `Vec`
  par indices croissants.
- Les sorties anticipées (`return`, `break`, `continue`, `?`) détruisent tout ce qui
  doit l'être entre le point de sortie et la destination.
- **Destructeur personnalisé** : une méthode `fn drop(inout self)` dans un bloc `impl`
  est appelée automatiquement. Elle ne peut pas être appelée explicitement (E0430).
  Un type qui définit `drop` ne peut pas être déstructuré par déplacement (E0431).
- Une panique termine le processus **sans** exécuter les destructeurs (§07) : le système
  d'exploitation récupère la mémoire et les descripteurs.

```nexul
struct Fichier { descripteur: i32 }
impl Fichier {
    fn drop(inout self) {
        fermer(self.descripteur)     // appelé automatiquement en fin de portée
    }
}
```

## 5.6 Filtrage et propriété

- Scrutin **place** : les liaisons empruntent en lecture ; aucune possession n'est
  transférée ; la place est gelée pendant le bras.
- Scrutin **valeur** : la valeur est placée dans un temporaire caché. Les liaisons
  prennent possession des parties qu'elles désignent ; les parties ressources non liées
  sont détruites **au début du bras**, avant l'exécution de son corps. Une garde `if`
  voit les liaisons comme des emprunts, et les transferts n'ont lieu qu'une fois la
  garde validée : déplacer une liaison dans la garde (`Some(s) if prendre(s)`,
  `move s`, receveur `owned self`) est l'erreur E0403, car si la garde échouait, les
  bras suivants filtreraient une valeur déjà détruite.
- Pour consommer une variable dans un `match`, on écrit `match move v { ... }`.
- Scrutin **`&place`** (`match &x`, `if let P = &x`, `for e in &v`) : la place doit être
  modifiable ; les liaisons sont des emprunts en **écriture**. Pendant un bras qui lie des
  variables (et pendant les gardes), la place est gelée de façon **exclusive** : elle ne
  peut être ni lue ni modifiée autrement que par les liaisons. Un bras sans liaison
  (`None => x = Some(1)`) peut modifier la place librement.

```nexul
fn prendre(owned s: String) -> bool {
    return s.len() > 1
}

fn main() {
    let o: Option[String] = Some("ab" + "c")
    match move o {
        Some(s) if prendre(s) => println("long")   // erreur E0403 : la garde emprunte `s`
        Some(s) => println(s)
        None => {}
    }
}
```

## 5.7 Code non sûr

Le mot-clé `unsafe` délimite le code dont la sûreté mémoire est garantie par le
programmeur et non par le compilateur :

```nexul
unsafe fn lire_brut(p: Ptr[u8]) -> u8 { ... }    // l'appelant doit être en contexte unsafe

let octet = unsafe { p.offset(4).read() }
```

Nécessitent un contexte `unsafe` (bloc `unsafe { }` ou corps de `unsafe fn`) (E0501) :
appeler une `unsafe fn` ou une fonction `extern`, et les opérations `read`, `write`,
`offset`, `cast` et `as` sur `Ptr[T]`. Créer un pointeur nul (`Ptr.null()`), tester
`is_null()`, copier ou comparer des pointeurs est sûr.

Une API sûre construite sur du code `unsafe` (comme `Vec[T]`) **doit** garantir que
toute utilisation depuis du code sûr respecte les règles de ce chapitre. C'est le contrat
documenté de chaque bloc `unsafe` de la bibliothèque standard.

## 5.8 Garanties et leur périmètre

Le périmètre exact, les conditions et l'état des preuves sont fixés par
l'[ADR 0011](../adr/0011-perimetre-des-garanties.md). En résumé :

**Garantie.** Pour un programme dont le code ne contient pas d'`unsafe` (bibliothèque
standard et paquets exceptés), compilé par `nxc` pour une cible prise en charge, et
sous les conditions ci-dessous, il ne se produit ni accès hors bornes (vérification à
l'exécution), ni utilisation après libération, ni double libération, ni référence
pendante, ni lecture de mémoire non initialisée, ni débordement arithmétique silencieux,
ni conversion numérique silencieusement destructrice. Une violation de ces règles que
le compilateur ne peut pas refuser devient une **panique** déterministe (message,
position dans le source, code de sortie 101).

**Conditions** (hors d'elles, la garantie ne s'applique pas) :

1. Chaque bloc `unsafe` de la bibliothèque standard et des paquets respecte son
   contrat (§5.7) : la garantie est relative à ce code, que le compilateur ne vérifie pas.
2. Les fonctions externes (`extern "C"`) respectent leur signature, et le code C qui
   appelle une fonction `@export` lui passe des valeurs valides de leur type (§8.4).
3. La pile suffit au programme. Un dépassement de pile n'est **pas** une panique :
   - sur Linux x86-64 et AArch64, il se termine comme une panique : sortie standard
     vidée, `panic: stack overflow` sur la sortie d'erreur, code 101 (gestionnaire de
     signal sur une pile de secours ; test `tests/run/stack_overflow.nx`) ;
   - sur Windows, macOS et Linux RISC-V, il arrête le processus par le système (page de
     garde), sans message de NEXUL ni vidage de la sortie standard tamponnée ;
   - une trame plus grande que la zone de garde n'est pas encore sondée ;
   - sur WebAssembly, la pile d'appels du moteur est protégée (piège de l'hôte), mais
     pas la pile placée en mémoire linéaire (agrégats, variables dont l'adresse est
     prise) : `wasm-ld` la place aujourd'hui après les données statiques, qu'un
     dépassement peut écraser ;
   - sur les cibles sans système (`*-none`), il **n'est pas détecté** : la pile peut
     écraser le tas. La garantie y suppose une pile dimensionnée par l'analyse.
4. Sur les cibles sans système, aucun gestionnaire d'interruption (`@export` d'un
   vecteur) n'appelle l'allocateur, les sorties ou toute fonction de la bibliothèque
   standard qui utilise une variable statique, si le code principal peut être
   interrompu pendant ces opérations (le runtime n'est pas encore réentrant).

**Ce qui n'est pas garanti** : l'absence de fuite (`std.mem.forget`, valeurs statiques,
paniques qui n'exécutent pas les destructeurs), l'absence de panique, la terminaison,
les propriétés temporelles (temps réel) et l'absence de course de données dès qu'il y
aura de la concurrence (rien n'en offre encore en dehors des interruptions ; voir
l'ADR 0013).

## 5.9 Variables statiques

Une variable `static` vit pendant toute l'exécution et n'est jamais détruite : elle ne
peut pas contenir de ressource (E0374). Lire une `static` immuable est sûr. Lire ou
modifier une `static var` exige un contexte `unsafe` (E0501), car le compilateur ne peut
pas vérifier la loi d'exclusivité sur une valeur accessible de partout : une fonction
appelée pendant un emprunt pourrait la modifier, et un gestionnaire d'interruption peut
s'exécuter à tout moment.
