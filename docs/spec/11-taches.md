# 11 — Tâches et concurrence

Ce chapitre fixe la concurrence de NEXUL (ADR 0013, acceptée le 24/10/2026 ; noms et
threads du système choisis par l'utilisateur le 25/10/2026). Le modèle est la
**concurrence structurée** : des tâches lancées dans une portée qui les attend, et dont
les règles réutilisent la loi d'exclusivité (§5.4). **Dans du code sûr, aucune course de
données n'est possible.**

## 11.1 Propriétés des types

Deux interfaces sans méthode du prélude (§3.7) disent ce qu'une valeur peut faire entre
tâches :

| Propriété | Sens | Exemples sans | Origine |
|---|---|---|---|
| `Transfer` | la valeur peut être **déplacée ou modifiée** par une autre tâche | `Ptr[T]`, fonctions anonymes | structure du type |
| `Share` | plusieurs tâches peuvent la **lire** en même temps | `Ptr[T]`, fonctions anonymes | structure du type |

Un type qui contient un `Ptr` et reste sûr d'une tâche à l'autre le déclare dans du code
relu : `unsafe impl Transfer for T {}` (`unsafe extend T is Transfer {}` en édition 1).
Sans `unsafe`, ou pour autre chose que ces deux propriétés, c'est l'erreur E0253.
`String`, `Vec[T]` (si `T` les a) et `rt.Location` les déclarent.

Invariants (ADR 0013 §1), à respecter par toute contribution :

- **I1** : aucune API sûre ne modifie une valeur à travers un emprunt en lecture ;
- **I2** : `static var` exige `unsafe` (E0501) ;
- **I3** : pas de propriété partagée modifiable sans type `Share` dédié (verrou, atomique) ;
- **I4** : une panique arrête tout le processus, tâches comprises (code 101).

## 11.2 `tache.portee` et `lancer`

```nexul
import std.tache

fn main() {
    var gauche: i64 = 0
    var droite: i64 = 0
    tache.portee(fn(inout p: tache.Portee) {
        p.lancer(fn() { gauche = 21 })
        p.lancer(fn() { droite = 21 })
    })
    println("{gauche + droite}")
}
```

- `tache.portee(f)` appelle `f` avec une `Portee`, puis **attend la fin de toutes les
  tâches** lancées avant de rendre la main.
- `p.lancer(fn() { … })` lance une tâche. Règles (E0254) : l'appel est écrit
  **directement** dans la fonction donnée à `tache.portee`, sur son paramètre ; la tâche
  est une **fonction anonyme écrite dans l'appel** ; elle n'utilise que des variables
  **déclarées avant `tache.portee`** (celles de la fonction donnée à `portee` sont
  détruites avant la fin des tâches) ; lancée dans une boucle, elle ne **modifie** aucune
  variable (la tâche du tour suivant la modifierait en même temps).
- **Exclusivité entre tâches** (E0420) : ce qu'une tâche utilise reste gelé jusqu'à la
  fin de la `portee` — en écriture (exclusif) si la tâche le modifie, en lecture sinon.
  Deux tâches ne modifient donc jamais la même variable ; une variable n'est jamais lue
  pendant qu'une tâche la modifie ; la fonction donnée à `portee` n'y touche plus.
- Une variable **lue** par une tâche est `Share`, une variable **modifiée** est
  `Transfer` (E0249).
- Après `tache.portee`, les variables sont de nouveau utilisables normalement, avec les
  valeurs écrites par les tâches.

## 11.3 Exécution

| Cibles | Tâches |
|---|---|
| Linux (x86-64, AArch64, RISC-V) | un thread du système par tâche (`clone`), pile de 8 Mio avec garde ; un débordement de pile dans une tâche finit comme une panique |
| Windows | un thread du système par tâche (`CreateThread`, pile réservée de 8 Mio) |
| macOS, WebAssembly, sans système | chaque tâche s'exécute **au moment où elle est lancée** (c'est un des ordres que les règles permettent) |

Le runtime (allocateur, sorties standard, table paresseuse de `math`) est protégé par un
verrou réentrant dès que le premier thread démarre (`rt.critical_section()`, le même
point de garde que les interruptions des cibles sans système, tâche C5) ; l'entrée
standard a son propre verrou, si bien qu'une tâche qui attend une ligne ne bloque pas
les autres. Sur les cibles sans système, ce point de garde masque les interruptions (cœur
unique) : un gestionnaire **non masquable** (NMI, faute matérielle) ne DOIT NI allouer NI
écrire sur les sorties standard, car il peut interrompre le runtime au milieu d'une
section protégée. Les opérations atomiques (`mem.atomic_*`, séquentiellement cohérentes) sont
réservées à la bibliothèque et au code `unsafe`.

*(Pas encore, lot 18 de l'ADR 0013 : canaux bornés `canal[T is Transfer](capacite)`,
`Verrou[T]`, `Atomique[…]`, `tache.lancer_detachee`.)*
