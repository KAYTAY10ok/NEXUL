# ADR 0018 — Profil temps réel v1 sur Cortex-M4 (P8-G1)

- **Statut** : acceptée (Claude Main, 29/10/2026), dans le périmètre de la 0.7.0 validé par
  l'utilisateur (décision 3 : cible A, Cortex-M4 sans système). Pas de rupture : `with` et
  `every … within …` sont réservés par l'ADR 0010 (§8, grammaire `clause`) et refusés
  jusqu'ici ; les attributs ajoutés n'existaient pas.
- **Étude préalable** : [temps-reel-g0.md](temps-reel-g0.md) §4 à §6.
- **Hors de cette ADR** : contrats (`requires`, `ensures`), effet `io`, pire temps
  d'exécution (WCET) calculé, Linux PREEMPT_RT (cible C), multicœur.

## Contexte

NEXUL tourne déjà sur `thumbv7em-none` (démarrage, table des vecteurs, gestionnaires
`@export("SysTick_Handler")`, flottants matériels, tests sous QEMU `mps2-an386`). Il lui
manque ce qui fait un profil temps réel : des tâches déclenchées par le temps, un partage
sans course ni interblocage, la preuve qu'une tâche n'alloue ni n'attend, une pile bornée,
et la détection des échéances manquées. Les trois niveaux de preuve de G0 §1 s'appliquent :
ce que cette ADR fait **vérifier par le compilateur** est listé en §7 ; le reste est
**mesuré** ou **dépendant de la plateforme**, et annoncé comme tel.

## Décision

### 1. Effets : `with noalloc, nonblocking`

```nexul
fn i32 filtre(Filtre f!, i32 x) with noalloc, nonblocking { … }
```

- Deux effets en v1. Toute autre étiquette (`io`…) reste refusée (« pas encore pris en
  charge »). La clause est permise sur toute fonction et sur toute cible ; elle est
  **obligatoire** sur une tâche (§2, §3) pour que le lecteur voie la garantie.
- **Vérification transitive** sur le graphe des **instances** (après monomorphisation) :
  une fonction `with noalloc` ne peut atteindre aucune racine d'allocation, une fonction
  `with nonblocking` aucune racine bloquante. Les appelées n'ont pas besoin d'être
  annotées : le compilateur suit les appels. L'erreur montre la chaîne d'appels jusqu'à la
  racine (E0601, E0602).
- **Racines**, déclarées dans `lib/std` seulement par deux attributs réservés à `std` :
  `@allocates` (allocation **et** libération : `heap.alloc`, `realloc`, `free`, donc
  `Vec.push`, `String` qui grandit, destruction d'une valeur possédante…) et `@blocks`
  (semi-hébergement, `println`, attentes, verrous et canaux de `std.tache`, `join`,
  `sleep`). Un appel `extern "C"` est une racine des deux effets, sauf si sa déclaration
  porte elle-même `with noalloc, nonblocking` : c'est alors une promesse du programmeur,
  comme `unsafe`.
- **Refusés dans une fonction à effets** (E0603) : un appel **indirect** (pointeur de
  fonction, appel à travers `any`), sauf si l'instance appelée est connue statiquement
  après monomorphisation ; la **récursion** (cycle du graphe). Les deux rendent la pile et
  les effets invérifiables.
- **Paniques** : les chemins de panique (débordement, indice hors bornes…) ne comptent pas
  comme effets, ni un appel à une fonction qui ne rend jamais la main (type `Never` :
  `panic`, `exit`). Une panique est un **arrêt sûr** (§5) : le système s'arrête, il ne
  continue pas en retard. `rt.stop` (§2) a le même statut.

### 2. Tâches périodiques : `every … within …`

```nexul
import std.rt.{ms, us}

@resource
static var Etat etat = Etat(consigne: 0, commande: 0)

fn regulation(Etat etat!) every ms(1) within us(400) with noalloc, nonblocking {
    etat.commande = etat.pid!.calcul(capteur.lire())
}
```

- **Cibles** : les cibles Cortex-M sans système (`thumbv7em-none` en v1). Ailleurs : E0604.
- **Durées** : `every` et `within` prennent un appel de `std.rt.ms`, `us` ou `s` dont
  l'argument est une expression constante entière (E0604 sinon). Le compilateur les
  convertit en cycles avec la fréquence de l'horloge (§6). `within` est facultatif et vaut
  alors la période ; `within` > `every` est refusé (échéance contrainte, E0604).
- **Signature** : pas de résultat ; les paramètres sont des **ressources** (§3), rien
  d'autre (E0605). La tâche n'est pas appelable comme une fonction (E0605).
- **Priorités** : monotones par taux, c'est-à-dire que la période la plus courte a la
  priorité la plus haute ; à période égale, même niveau, exécutées dans l'ordre de
  déclaration. `@priority(n)` (1 = la plus basse) remplace la priorité calculée.
- **Exécution** : un `main` ordinaire prépare le matériel, puis appelle `rt.run()`. Les
  tâches s'exécutent jusqu'à ce que l'une appelle `rt.stop()` ; `rt.run()` rend alors la
  main à `main`, qui peut afficher un bilan. Sans appel à `rt.run()`, aucune tâche ne
  démarre (avertissement W0009).

### 3. Ressources partagées à plafond de priorité

- Une ressource est une `static var` marquée **`@resource`**. Son type suit les règles des
  statiques (pas de ressource système, E0374) ; sa valeur initiale est constante.
- Une tâche **reçoit** une ressource par un paramètre **du même nom** et du même type :
  `Etat etat!` pour la modifier, `Etat etat` pour la lire. Dans la tâche, la loi
  d'exclusivité habituelle s'applique (ADR 0003, 0013) ; la tâche peut la transmettre aux
  fonctions qu'elle appelle.
- **Aucun autre accès** : lire ou modifier une `@resource` ailleurs que par un paramètre
  de tâche est refusé (E0605), y compris dans `unsafe`. Exception : `main`, **avant**
  `rt.run()` et **après** son retour, où aucune tâche ne tourne (vérifié sur le graphe
  d'appels de `main`, E0605 sinon).
- **Plafond** : priorité maximale des tâches qui reçoivent la ressource. En v1, une tâche
  dont un paramètre a un plafond supérieur à sa priorité s'exécute **entièrement** au
  plafond (registre `BASEPRI`). C'est la politique de pile des ressources à grain
  grossier : pas de course, pas d'interblocage, une seule pile, et une inversion de
  priorité bornée par la durée d'**un** corps de tâche. Des sections plus fines
  (verrouiller une ressource dans une partie du corps) demandent une syntaxe, à proposer à
  l'utilisateur en v2.

### 4. Interruptions matérielles

```nexul
@interrupt(6)
@priority(3)
fn bouton(Compteur appuis!) with noalloc, nonblocking { appuis.n += 1 }
```

- `@interrupt(n)` lie la fonction à la ligne `n` du NVIC ; `@priority(p)` est
  **obligatoire** pour une interruption (E0604). Les paramètres sont des ressources (§3),
  et l'interruption entre dans le calcul des plafonds.
- `@export("SysTick_Handler")` et les lignes utilisées par l'ordonnanceur (§5) sont
  refusés quand le programme a des tâches (E0606). Un `@export("IRQn_Handler")` sans
  tâche fonctionne comme avant.

### 5. Ordonnancement et échéances

Le NVIC ordonne, sans noyau : aucune pile par tâche, aucun changement de contexte écrit à
la main.

- **SysTick** a la priorité la plus haute. Il compte les tops ; la durée d'un top est le
  PGCD des périodes (en cycles). Si ce PGCD dépasse le compteur de 24 bits de SysTick,
  c'est son plus grand diviseur qui tient (E0606 si aucun ne dépasse 1 000 cycles). À
  chaque échéance de libération, SysTick note l'**instant de libération** et rend en
  attente la ligne du **répartiteur** du niveau de la tâche.
- **Un répartiteur par niveau de priorité**, généré par le compilateur, sur une ligne
  d'interruption libre (prise depuis la dernière ligne vers le bas) : il exécute les
  tâches libérées de son niveau, au plafond voulu (§3).
- **Instant** : nombre de tops × recharge + position dans le top (registre `SYST_CVR`),
  au cycle près ; il ne dépend pas de `DWT_CYCCNT`, que QEMU n'émule pas.
- **Dépassement** : une libération alors que l'instance précédente n'est pas finie (la
  libération est alors sautée), ou une fin plus tard que libération + `within`, incrémente
  le compteur de la tâche,
  `rt.overruns(regulation)` (l'argument doit nommer une tâche, E0608). Si le programme
  déclare `@overrun fn f() with noalloc, nonblocking`, `f` est appelée après chaque
  dépassement, par l'ordonnanceur : depuis SysTick pour une libération sautée, depuis le
  répartiteur pour une fin tardive. Un dépassement n'est **jamais** ignoré en
  silence.
- **Niveaux** : 2^bits − 1 niveaux pour les tâches (SysTick garde le plus haut). La v1
  suppose 3 bits de priorité, le minimum des Cortex-M4 visés (`mps2-an386`), soit
  7 niveaux. Au-delà : E0606.
- **Arrêt sûr** : panique, faute (HardFault, MemManage) ou garde de pile (§6). Toutes les
  interruptions sont masquées, le message est écrit par semi-hébergement s'il est
  disponible, puis le processeur s'arrête. Le programme ne repart jamais en silence.

### 6. Pile bornée et configuration de la carte

- `@clock(hz)` sur `main` donne la fréquence du cœur. Il est obligatoire avec des tâches
  (E0604) : 25 000 000 pour `mps2-an386`, 180 000 000 pour la NUCLEO-F446RE.
- `@stack(octets)` sur `main` donne la réserve de pile, 8 192 octets par défaut. Le script
  de lien place la pile en haut de la RAM et le tas **au-dessous** de sa réserve.
- **Garde** : une région MPU sans accès de 32 octets au bas de la réserve. Un débordement
  provoque MemManage, donc un arrêt sûr (§5).
- **Borne statique**, calculée par `nxc` sur l'exécutable lié, en code machine, après
  optimisation et inlining (nouveau module `compiler/nxc/src/stack.rs`) :
  - taille de trame de chaque fonction, lue dans son prologue Thumb-2 (`push`, `vpush`,
    `sub sp`) ;
  - graphe d'appels lu dans les `bl`.
  
  Toute autre modification de `sp`, un `blx` vers un registre, ou un cycle dans le code
  atteint par une tâche rend la borne inconnue : c'est une erreur, pas une estimation
  (E0607).
  
  La borne vaut :
  - celle de `main`, jusqu'à `rt.run()` compris ;
  - plus, pour chaque niveau de priorité, le maximum sur ses tâches, plus un cadre
    d'exception de 104 octets (contexte flottant) ;
  - plus le cadre de SysTick et celui du répartiteur.
  
  Si elle dépasse la réserve, la compilation échoue (E0607, avec le détail par niveau).
  `nxc build --stack-report` écrit la borne de chaque tâche.

### 7. Ce que la v1 permet d'affirmer

| Niveau | Affirmation |
|---|---|
| Vérifié par le compilateur | pas d'allocation, d'attente, de récursion ni d'appel indirect dans une tâche, une interruption à ressources ou une fonction `with` ; ressources partagées sans course ni interblocage ; pile bornée et inférieure à la réserve |
| Vérifié à l'exécution | toute échéance manquée est comptée et signalée ; tout débordement de pile arrête le système |
| Mesuré | gigue, temps de réponse et pile **observés**, sur carte, avec la méthode de G0 §6 ; jamais présentés comme un pire cas |
| Dépendant de la plateforme | respect d'une échéance stricte : pour une carte, une fréquence et une application analysées |

## Codes d'erreur

| Code | Sens |
|---|---|
| E0601 | une fonction `with noalloc` peut allouer ou libérer (chaîne d'appels montrée) |
| E0602 | une fonction `with nonblocking` peut bloquer (chaîne d'appels montrée) |
| E0603 | récursion ou appel indirect dans une fonction à effets |
| E0604 | tâche ou interruption mal formée : cible, durée non constante, `within` > `every`, effets ou `@priority` manquants, `@clock` manquant, résultat |
| E0605 | ressource : paramètre de tâche qui n'en est pas une, accès hors d'un paramètre de tâche, tâche appelée comme une fonction |
| E0606 | ordonnancement impossible : trop de niveaux, top impossible, ligne d'interruption en conflit |
| E0607 | pile : borne inconnue, ou supérieure à la réserve |
| E0608 | `rt.overruns` dont l'argument n'est pas une tâche |
| W0009 | des tâches sont déclarées mais `rt.run()` n'est jamais appelée |

## Plan de réalisation (G2) et preuves exigées

1. **Syntaxe** : `with` et `every … within …` acceptés en édition 1 (`parser/ed1.rs`,
   AST) ; nouveaux attributs vérifiés à leur place. Tests `tests/ui`.
2. **Effets** : attributs `@allocates` et `@blocks` posés dans `lib/std` ; vérification
   sur le graphe des instances. Tests `tests/ui` pour E0601 à E0603, avec chaînes
   d'appels ; test que `lib/std` entière se compile avec ses racines.
3. **Ressources, tâches, plafonds** (E0604 à E0606, E0608) ; répartiteurs générés ;
   runtime dans `lib/std/rt.nx` et `lib/std/sys/bare.nx` (SysTick, NVIC, `BASEPRI`,
   instants, compteurs, `rt.run`, `rt.stop`, `rt.overruns`).
4. **Pile** : `stack.rs`, réserve, garde MPU, `--stack-report` (E0607). Test : la borne
   calculée est supérieure ou égale au maximum **observé** (motif écrit dans la pile au
   démarrage puis relu) sur chaque programme de test.
5. **Tests d'exécution sous QEMU** (`tests/run`, `// targets: thumbv7em-none`) :
   - ordre et nombre des libérations ;
   - préemption avec ressource partagée : compteur cohérent ;
   - interruption déclenchée par logiciel ;
   - surcharge provoquée : chaque dépassement est compté ;
   - garde de pile : arrêt sûr.
6. **Spécification** : nouveau chapitre `docs/spec/12-temps-reel.md`, codes dans
   `codes.rs`, et `docs/plateformes.md`.
7. **Mesures sur carte** (G3, NUCLEO-F446RE) : « non testé » tant que la carte n'est pas
   disponible. QEMU ne simule pas les temps : il ne prouve que le comportement.

## Conséquences

- Les fonctions de `lib/std` utilisables dans une tâche se lisent dans la documentation :
  elles n'atteignent aucune racine. `nxc doc` affichera les effets inférés.
- Le grain grossier des plafonds (§3) est la principale limite de la v1. Une tâche longue
  qui partage une ressource avec une tâche rapide la retarde de toute sa durée ; la
  mesure le montrera, et la v2 proposera des sections.
- Les tâches et les interruptions à ressources ne sont pas appelables, ce qui garde les
  plafonds exacts. Le code commun se met dans des fonctions ordinaires.
