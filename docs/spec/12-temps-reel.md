# 12 — Effets et temps réel

Décision : [ADR 0018](../adr/0018-profil-temps-reel.md). Ce chapitre décrit ce que le
compilateur vérifie. Ce qui n'est pas encore implémenté est signalé « à venir » ; le
compilateur le refuse (E0111) plutôt que de l'accepter sans le vérifier.

## 12.1 Effets : `with noalloc, nonblocking`

```nexul
fn i64 filtrer(Filtre f!, i64 x) with noalloc, nonblocking {
    f.somme += x - f.fenetre[f.pos]
    f.fenetre[f.pos] = x
    f.pos = (f.pos + 1) % 4
    return f.somme / 4
}
```

- La clause suit les paramètres, avant le corps. Effets : `noalloc` et `nonblocking`, une
  fois chacun (E0604). Une autre étiquette est refusée (E0111).
- **`noalloc`** : la fonction ne peut ni allouer ni libérer de mémoire du tas, directement
  ou par une fonction qu'elle appelle (E0601). Cela comprend `Vec.push` et les chaînes qui
  grandissent, l'interpolation de chaînes, la conversion vers `any`, les parties
  `indirect` et la **destruction** d'une valeur qui possède du tas (`String`, `Vec`…).
- **`nonblocking`** : la fonction ne peut pas attendre (E0602) : console (`println`),
  fichiers, semi-hébergement, verrous et canaux de `std.tache`, `join`, `sleep`, appels
  système.
- Dans les deux cas, la fonction ne peut faire ni appel **indirect** (valeur de fonction,
  appel à travers `any`) ni **récursion** (E0603).
- La vérification suit tous les appels, dans les instances génériques réelles
  (`premier[i32]`) et les destructeurs. Les fonctions appelées n'ont pas besoin d'être
  annotées. L'erreur montre la chaîne d'appels.
- **Arrêts** : une panique (débordement, indice hors bornes, `panic`, `assert`) et un appel
  à une fonction qui ne rend jamais la main (`Never`) ne sont pas des effets : le
  programme s'arrête.
- **Fonctions C** : un appel à une fonction `extern "C"` alloue et bloque, sauf si sa
  déclaration porte la clause. C'est alors une promesse du programmeur, que le compilateur
  ne vérifie pas :

  ```nexul
  extern "C" fn i32 abs(i32 x) with noalloc, nonblocking
  ```

- La bibliothèque standard marque ses racines par `@allocates` (allocateur) et `@blocks`
  (appel système, semi-hébergement, attente). Ces deux attributs lui sont réservés (E0231).
- Les effets sont vérifiés sur toutes les cibles ; ils ne promettent rien sur la durée.

## 12.2 Tâches périodiques, ressources, interruptions

Sur une cible Cortex-M sans système (`thumbv7em-none`, `thumbv7m-none`) ; ailleurs, E0604.

```nexul
// edition: 1
// target: thumbv7em-none
import std.rt
import std.rt.{ms, us}

type Etat {
    u32 n
}

@resource
static var Etat etat = Etat(n: 0)

fn regulation(Etat etat!) every ms(1) within us(400) with noalloc, nonblocking {
    etat.n += 1
    if etat.n == 1000 {
        rt.stop()
    }
}

@clock(25_000_000)
fn main() {
    rt.run()
    println("{etat.n} périodes, {rt.overruns(regulation)} retards")
}
```

- **Tâche périodique** : `every` et, si l'échéance est plus courte que la période,
  `within`. Leurs durées sont `ms(n)`, `us(n)` ou `s(n)` de `std.rt` avec un entier
  littéral. Une tâche est une fonction libre, sans résultat ni générique, déclarée
  `with noalloc, nonblocking` (E0604).
- **Interruption** : `@interrupt(n)` (ligne 0 à 31 du NVIC) et `@priority(p)` obligatoire.
- **Priorités** : de 1 (la plus basse) à 7. Une tâche périodique sans `@priority` reçoit
  la sienne de sa période (monotone par taux) : la plus courte a la priorité la plus
  haute ; à période égale, même niveau, dans l'ordre de déclaration. SysTick, qui libère
  les tâches, est au-dessus de toutes.
- **Ressources** : une `static var` marquée `@resource`. Une tâche la reçoit par un
  paramètre du même nom et du même type (`Etat etat!` pour la modifier). Personne d'autre
  ne l'utilise, sauf `main`, sans `unsafe`, puisqu'aucune tâche ne tourne en dehors de
  `rt.run()` (E0605). Une tâche s'exécute au **plafond** de ses ressources, c'est-à-dire
  la plus haute priorité des tâches qui les reçoivent, par `BASEPRI`. Pas de course, pas
  d'interblocage, une seule pile.
- **Exécution** : `main` prépare le matériel puis appelle `rt.run()`. La fonction rend la
  main quand une tâche appelle `rt.stop()`. Sans appel à `rt.run()`, W0009.
- **Horloge** : `@clock(hz)` sur `main`, obligatoire avec des tâches (E0604). Les périodes
  sont des nombres entiers de cycles ; leur PGCD donne le top de SysTick, d'au moins 1000
  cycles (E0606).
- **Dépassements** : une libération alors que l'instance précédente n'a pas fini est
  comptée et **sautée** ; une fin après l'échéance est comptée. `rt.overruns(tache)`
  donne le compte, et la fonction marquée `@overrun` (sans paramètre,
  `with noalloc, nonblocking`) est appelée à chaque dépassement.
- **Instant** : `rt.now_cycles()` donne les cycles depuis `rt.run()`, d'après SysTick.
- Un appel à une tâche, ou une tâche prise comme valeur, est refusé (E0605), sauf comme
  argument de `rt.overruns` (E0608 pour un autre argument).

## 12.3 Pile bornée

- **Réserve** : `@stack(octets)` sur `main`, 8192 par défaut, un multiple de 32 d'au moins
  1024 (E0604). La pile est en haut de la RAM ; le tas s'arrête sous la réserve.
- **Garde** : une région MPU sans accès, de 32 octets, sous la réserve. Un débordement, dans
  une tâche comme dans `main`, provoque une faute MemManage : arrêt sûr, avec le message
  `panic: stack overflow` et le code de sortie 101. Les autres fautes (HardFault, BusFault,
  UsageFault) arrêtent de même.
- **Borne statique** : `nxc` la calcule sur l'exécutable lié, en code machine, après
  optimisation et inlining.
  - Les trames sont lues dans les prologues Thumb-2 et les appels dans les `bl`.
  - La borne additionne :
    - la chaîne de démarrage jusqu'à `rt.run()` ;
    - pour chaque niveau de priorité, le plus profond de ses gestionnaires et le cadre
      d'exception (112 octets avec le contexte flottant, 40 sans) ;
    - SysTick.
  - Un appel par un registre, une autre écriture de `sp` ou un cycle rendent la borne
    inconnue : la compilation échoue (E0607), sans estimation.
  - Une borne supérieure à la réserve échoue aussi (E0607), et aucun exécutable n'est
    laissé.
  - Les fonctions qui ne rendent jamais la main (paniques) n'en font pas partie : si une
    panique déborde, la garde arrête le programme.
- `nxc build --stack-report` écrit la borne, niveau par niveau.
- `rt.stack_used()` donne la pile la plus profonde **observée** pendant `rt.run()`, en
  peignant la pile libre. C'est une mesure, pas une borne. Le test
  `compiler/nxc/tests/rt_stack.rs` vérifie sous QEMU que la borne la couvre, en O0, O2 et
  Os.
