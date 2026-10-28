# Temps réel — P8-G0 : choix de la première cible

Lot P8, étape G0 (tâche O6 du plan maître). Auteur : Claude (architecture, concurrence,
temps réel). Statut : **étude** (18/10/2026) ; **décision 3 prise par l'utilisateur le 28/10/2026 : cible A, Cortex-M4 sans système**. Elle prépare la **décision 3** de
l'utilisateur (plan maître §6) ; elle ne revendique aucune garantie.

## 1. Ce que « temps réel » voudra dire pour NEXUL

Classes de la passation (§7.1) : **souple** (un retard dégrade le service), **ferme**
(un résultat en retard est inutile), **strict** (un retard est inacceptable). NEXUL
annoncera exactement trois niveaux de preuve, sans les mélanger :

| Niveau | Ce que NEXUL peut affirmer | Comment |
|---|---|---|
| **Vérifié par le compilateur** | absence d'allocation, de blocage, de récursion, d'appel indirect dans une tâche ou un gestionnaire marqué ; pile bornée ; accès partagés sans course | effets (`with noalloc, nonblocking`, ADR 0010 §8) et analyses statiques ; erreurs de compilation |
| **Mesuré** | gigue, latence, temps d'exécution maximal **observé**, pile maximale observée | banc reproductible sur la cible, méthode publiée ; « observé » n'est **jamais** présenté comme un pire cas (WCET) |
| **Dépendant de la plateforme** | respect d'une échéance stricte | seulement pour une carte, une fréquence, une configuration et une application données, avec l'analyse correspondante |

Rien n'est garanti sur Windows, macOS, un navigateur ou un Linux générique.

## 2. Candidats

| Critère | **A. Cortex-M4 sans système** | B. Cortex-M + RTOS (FreeRTOS, Zephyr) | C. Linux PREEMPT_RT |
|---|---|---|---|
| Existant NEXUL | cible `thumbv7em-none`, démarrage, vecteurs, gestionnaires `@export`, flottants matériels, tests sous QEMU (`mps2-an386`) | aucun : il faudrait lier un RTOS écrit en C | cible `x86_64-linux`/`aarch64-linux`, appels système directs |
| Autonomie (ADR 0008 : pas de chaîne C) | totale | **rompue** : RTOS en C, à construire et à lier | totale (appels système) |
| Analysabilité des temps | bonne : pas de MMU, pipeline simple, peu ou pas de cache (accélérateur de flash à maîtriser) ; WCET de petites boucles réaliste | celle du RTOS, plus son interface | faible : caches, MMU, noyau ; latence pire cas seulement mesurée |
| Classe atteignable | ferme, et stricte pour une application analysée sur une carte donnée | ferme/stricte selon le RTOS | souple, ferme au mieux (latence de quelques dizaines de µs sur matériel réglé) |
| Instrumentation | compteur de cycles `DWT_CYCCNT`, broches GPIO + analyseur logique, trace SWO | idem + traces du RTOS | `clock_gettime`, `cyclictest` |
| Mesure en CI | fonctionnelle seulement (QEMU ne simule pas les temps) ; temps sur carte réelle | idem | impossible sur les machines virtuelles de GitHub |
| Matériel à acheter | une carte (ex. NUCLEO-F446RE, Cortex-M4F 180 MHz, sonde ST-LINK avec SWO intégrée, ≈ 15–25 €) | idem | un PC ou une Raspberry Pi avec noyau PREEMPT_RT |
| Ce que le langage doit apporter | tout : ordonnancement, ressources partagées, effets, pile | les effets ; l'ordonnancement vient du RTOS | les effets ; ordonnancement du noyau (`SCHED_FIFO`) |

## 3. Recommandation : A, Cortex-M4 sans système

1. C'est là que les promesses du langage ont le plus de sens et le moins de dépendances :
   NEXUL contrôle tout, du démarrage à l'ordonnancement, sans code C (ADR 0008).
2. La plateforme est analysable : une borne de pile **statique** et des temps mesurés au
   cycle près sont réalistes ; un pire cas formel reste possible plus tard avec un outil
   d'analyse dédié.
3. L'existant couvre déjà le démarrage, les vecteurs, les interruptions et les tests
   fonctionnels sous QEMU.
4. Le modèle de concurrence de l'ADR 0013 s'y prolonge naturellement (§4).

C (Linux PREEMPT_RT) viendra **ensuite**, pour le temps réel souple des services et de
la robotique, après la concurrence (0.5.0) : il ne demande que des appels système
(`sched_setscheduler`, `mlockall`, `clock_nanosleep` absolu). B n'est pas recommandé :
il réintroduit une chaîne C pour un gain qui disparaît dès que A existe.

## 4. Modèle proposé pour la cible A (pour l'ADR de G1)

- **Tâches à priorités fixes préemptives, sur une seule pile**, déclenchées par le
  temps (`every ms(10) within ms(2)`) ou par une interruption ; priorités déduites des
  périodes (monotone par taux) ou données explicitement. Le contrôleur d'interruptions
  (NVIC) fait l'ordonnancement : pas de noyau à écrire.
- **Ressources partagées à plafond de priorité** (politique de pile des ressources,
  modèle de RTIC en Rust) : une ressource partagée par plusieurs tâches est accédée dans
  une section qui élève la priorité au plafond de ses utilisateurs. Le compilateur
  vérifie statiquement que **toute** ressource partagée passe par là. Résultat : pas de
  course, pas d'interblocage, pas d'inversion de priorité non bornée, et une seule
  pile. C'est la loi d'exclusivité de NEXUL appliquée aux priorités.
- **Effets vérifiés** : une tâche ou un gestionnaire marqué `with noalloc, nonblocking`
  ne peut appeler (transitivement) ni l'allocateur, ni les sorties par
  semi-hébergement, ni une attente, ni une fonction récursive, ni un appel indirect
  (`any`, pointeur de fonction). Les fonctions de `lib/std` portent leurs effets.
- **Pile bornée** : taille de trame de chaque fonction (section `.stack_sizes` de LLVM)
  + graphe d'appels sans récursion ni appel indirect → borne par tâche, sommée par
  niveau de priorité ; refus de compiler si la borne dépasse la réserve. **Garde de
  pile** par une région MPU sans accès sous la pile (le Cortex-M4 de la carte en a une).
- **Dépassement d'échéance** : détecté (horodatage au cycle), compté, et traité par un
  gestionnaire choisi (arrêt, journal, mode dégradé) ; jamais ignoré en silence.
- **Hors profil** : `println` par semi-hébergement (bloque pendant des millisecondes)
  est refusé dans une tâche temps réel.

## 5. Prérequis dans les autres lots

| Prérequis | Lot | Statut |
|---|---|---|
| Runtime réentrant face aux interruptions (ADR 0011, C4, tâche T5) | 0.4.0 | à faire (Codex) |
| Garde de pile et détection du dépassement sur la cible | 0.4.0 / P8-G2 | à faire |
| `Transfer`/`Share` et règles de partage (ADR 0013) | 0.5.0 | proposé |
| Système d'effets (`with`) dans le compilateur | P8-G1 | à concevoir (ADR) |
| Accès `volatile` et atomiques sur la cible | existe en partie (`read_volatile`) | atomiques à faire |

## 6. Critères de réussite de G2–G3 (proposés)

Sur NUCLEO-F446RE à 180 MHz, une application témoin (boucle de régulation à 1 kHz, une
tâche capteur à 100 Hz, une interruption externe, une ressource partagée) :

- compilée avec le profil : aucune allocation ni attente (vérifié), pile bornée
  statiquement et inférieure à la réserve (vérifié) ;
- 10⁶ périodes mesurées : gigue maximale et temps de réponse maximal **observés**
  publiés avec la méthode (`DWT_CYCCNT`, broche + analyseur) ; aucune échéance
  manquée sous la charge nominale ;
- en surcharge provoquée : chaque échéance manquée est détectée et signalée ;
- tests fonctionnels dans la CI sous QEMU ; mesures de temps seulement sur carte, avec
  rapport daté.

## 7. Décision demandée (décision 3)

1. Valider la cible A pour le profil temps réel v1 (et C comme deuxième cible après la
   0.5.0).
2. Valider l'achat d'une carte (référence proposée : NUCLEO-F446RE), sans laquelle aucune
   mesure de temps n'est possible.
3. Le modèle du §4 sera écrit en ADR (P8-G1) avant toute implémentation.
