# NEXUL — Cahier des charges officiel du langage de programmation universel

**NEXUL — Next Universal Language**
*One Language. Every Possibility.*

Document de vision, spécifications fonctionnelles et exigences techniques — Version 1.0 du cahier des charges.

> Ce fichier est la transcription Markdown du cahier des charges fondateur fourni par l'auteur du projet.
> Il sert de référence pour la traçabilité des exigences (voir `docs/spec/` et `docs/roadmap.md`).

---

## 1. Présentation générale du projet

### 1.1. Objet du projet

NEXUL est un projet de création d'un nouveau langage de programmation universel, conçu pour réunir dans un environnement cohérent la performance des langages compilés natifs, la simplicité des langages accessibles et la polyvalence des écosystèmes modernes.

L'objectif est de permettre à un développeur d'utiliser un même langage pour concevoir :

- des programmes système et des utilitaires en ligne de commande ;
- des applications de bureau et des applications mobiles ;
- des sites web, des interfaces web et des serveurs ;
- des jeux vidéo 2D et 3D, des moteurs graphiques et des simulations ;
- des logiciels industriels et scientifiques ;
- des systèmes embarqués et des micrologiciels pour microcontrôleurs ;
- des infrastructures réseau, des outils de cybersécurité et des systèmes cryptographiques ;
- des bases de données, des outils d'intelligence artificielle et des applications distribuées ;
- des logiciels nécessitant un accès très direct au matériel ;
- des programmes capables de communiquer avec les bibliothèques et les langages déjà existants.

NEXUL ne doit pas être limité à une seule catégorie de logiciels. Il doit constituer une plateforme de développement commune à plusieurs domaines, tout en laissant au développeur le contrôle nécessaire à chaque environnement.

### 1.2. Signification du nom

NEXUL = NEX (Next) + UL (Universal Language).

- **NEX — Next** : la prochaine génération de la programmation.
- **UL — Universal Language** : un langage conçu pour couvrir un très large éventail d'environnements et d'applications.

Le nom représente l'ambition de dépasser les limitations habituelles des langages spécialisés, sans sacrifier la performance, la lisibilité ou la maîtrise du système.

### 1.3. Vision fondamentale

NEXUL doit respecter cinq principes directeurs :

1. **Performance native** — Produire du code machine optimisé, avec un contrôle fin de la mémoire, des allocations et des ressources matérielles.
2. **Universalité** — Un langage commun pour les systèmes d'exploitation, les architectures matérielles et les domaines applicatifs pris en charge.
3. **Lisibilité** — Une syntaxe claire, régulière et compréhensible, adaptée aussi bien aux débutants qu'aux experts.
4. **Sécurité** — Des règles de types, de mémoire, de concurrence et d'interopérabilité conçues pour prévenir autant que possible les erreurs dangereuses.
5. **Interopérabilité** — La capacité à réutiliser les bibliothèques, outils, API et runtimes des autres écosystèmes de programmation.

### 1.4. Définition du résultat attendu

Le résultat ne sera pas uniquement une grammaire ou un compilateur. NEXUL devra être constitué de plusieurs éléments interdépendants :

- une spécification officielle du langage ;
- un compilateur ;
- un système de génération de code natif et de cibles portables ;
- un runtime minimal et modulaire ;
- une bibliothèque standard ;
- des bibliothèques spécialisées ;
- un gestionnaire de paquets ;
- des outils de débogage, de profilage et d'analyse ;
- une documentation technique et pédagogique ;
- un système de tests et de validation ;
- un processus de versionnement et de maintenance.

L'ensemble devra évoluer comme une plateforme cohérente.

## 2. Périmètre et universalité du langage

L'exigence principale est que NEXUL puisse être utilisé dans le plus grand nombre possible de contextes techniques.

Cependant, l'universalité ne signifie pas qu'un même binaire fonctionnera sans modification sur toutes les machines. Un exécutable dépend de son architecture, de son système d'exploitation, de son ABI et parfois de ses pilotes ou de son environnement d'exécution.

L'exigence est donc la suivante : **un langage et un écosystème communs, capables de produire des programmes adaptés à chaque cible officiellement supportée.**

### 2.1. Systèmes d'exploitation

Les cibles prioritaires seront : Windows ; Linux ; macOS ; Android ; iOS et iPadOS ; systèmes embarqués compatibles ; environnements web compatibles avec WebAssembly ; systèmes temps réel, selon les architectures et les pilotes disponibles.

À terme, l'architecture devra permettre d'ajouter d'autres systèmes sans réécrire le langage lui-même.

### 2.2. Architectures matérielles

| Famille | Objectif |
|---|---|
| x86-64 | Ordinateurs et serveurs modernes |
| ARM64 | Ordinateurs, serveurs et appareils mobiles |
| ARM 32 bits | Systèmes embarqués et microcontrôleurs compatibles |
| RISC-V | Processeurs et systèmes embarqués compatibles |
| WebAssembly | Environnements web et runtimes compatibles |
| Autres architectures | Extension possible via des backends de compilation |

La prise en charge effective de chaque architecture dépendra du backend, de l'ABI, du système d'exploitation et des outils de compilation disponibles.

### 2.3. Niveaux de programmation

- **Niveau 1 — Applicatif** : applications de bureau, mobiles, web, interfaces utilisateur, logiciels métiers et outils de productivité.
- **Niveau 2 — Système** : gestion des fichiers, processus, mémoire, réseau, threads, bibliothèques natives et interfaces système.
- **Niveau 3 — Bas niveau** : pointeurs, disposition mémoire, registres lorsque l'architecture le permet, assembleur intégré contrôlé, interruptions et accès au matériel via les mécanismes autorisés.
- **Niveau 4 — Matériel spécialisé** : microcontrôleurs, firmware, calcul GPU, systèmes temps réel et composants spécialisés, avec des toolchains et des contraintes propres à chaque cible.

Le développeur devra pouvoir choisir le niveau d'abstraction approprié sans être contraint de programmer systématiquement en bas niveau ou, à l'inverse, de subir un runtime lourd.

## 3. Architecture générale de NEXUL

L'architecture devra être conçue dès le départ pour permettre l'évolution du langage, l'ajout de nouvelles architectures et la création de bibliothèques sans remettre en cause ses fondations.

### 3.1. Architecture du compilateur

Le compilateur officiel aura pour nom provisoire **NEXUL Compiler**, avec la commande **`nxc`**. Il devra transformer le code source en un programme natif ou en un artefact adapté à la cible choisie.

Chaîne : code source NEXUL (syntaxe, modules, déclarations, annotations) → analyse lexicale et syntaxique (lexer, parser, AST) → analyse sémantique (résolution des noms, types, visibilité, règles mémoire) → représentation intermédiaire (IR typée et indépendante de la cible) → optimisation (locales, interprocédurales, guidées par profilage) → backend et édition de liens (code machine, WebAssembly, bibliothèques, linker) → **natif** (exécutables et bibliothèques) ou **portable** (Wasm et autres cibles compatibles).

**Exigences du compilateur.** Le compilateur devra :

- fournir des erreurs compréhensibles, avec localisation précise et suggestions de correction ;
- détecter les erreurs de syntaxe, de types et les violations des règles mémoire vérifiables statiquement ;
- prendre en charge la compilation séparée des modules ;
- permettre une compilation incrémentale pour accélérer le développement ;
- proposer des modes de compilation debug, release et profilage ;
- produire des symboles de débogage ;
- permettre le contrôle des options d'optimisation ;
- prendre en charge les bibliothèques statiques et partagées ;
- proposer des interfaces de programmation pour les outils externes ;
- permettre d'ajouter de nouvelles architectures sans modifier la syntaxe du langage.

**Technologie de départ.** Le choix recommandé est un frontend propre à NEXUL associé initialement à LLVM comme infrastructure de génération de code natif. Un backend spécialisé pourra être envisagé si des contraintes particulières le justifient. Il ne sera pas nécessaire de créer immédiatement un générateur de code machine entièrement nouveau. L'effort initial devra se concentrer sur la conception de NEXUL, son système de types, son modèle mémoire et sa sémantique.

### 3.2. Modèle de compilation

| Mode | Exigence |
|---|---|
| Compilation native | Produit du code machine pour la cible matérielle choisie |
| Compilation portable | Produit du WebAssembly ou un autre format portable pris en charge |
| Compilation bibliothèque | Produit des bibliothèques utilisables depuis d'autres langages |

La compilation native sera le mode principal pour les applications système, les serveurs et les programmes à hautes performances. Le mode portable permettra d'atteindre les environnements où un binaire natif classique n'est pas directement utilisable. La compilation de bibliothèques permettra de réutiliser les fonctions NEXUL dans des applications existantes.

### 3.3. Compilation croisée

Le compilateur devra permettre de construire un programme destiné à une machine différente de celle qui effectue la compilation. Cette capacité sera particulièrement importante pour : compiler un firmware depuis un ordinateur ; construire une application ARM64 depuis un poste x86-64 ; produire un programme Linux depuis Windows, lorsque la toolchain cible le permet ; générer des artefacts WebAssembly depuis un environnement de développement classique.

Chaque cible devra disposer d'une configuration documentée comprenant l'architecture, l'ABI, le système cible, le linker et les bibliothèques nécessaires.

## 4. Spécification de la syntaxe et du système de programmation

### 4.1. Objectif de lisibilité

La syntaxe devra être concise sans devenir ambiguë. Les règles du langage devront rester cohérentes d'un domaine à l'autre. Les exigences sont : noms explicites pour les fonctions et les types ; inférence des types lorsque le contexte est non ambigu ; déclarations de visibilité claires ; règles explicites de mutabilité ; conversions numériques maîtrisées ; messages d'erreur pédagogiques ; conventions uniformes pour les modules et les bibliothèques ; formatage automatique officiel ; absence de comportements implicites surprenants.

NEXUL devra éviter de multiplier les syntaxes pour accomplir une même opération.

### 4.2. Paradigmes de programmation

| Paradigme | Fonctionnalités requises |
|---|---|
| Procédural | Fonctions, procédures, structures de contrôle |
| Orienté objet | Classes, méthodes, interfaces et polymorphisme |
| Fonctionnel | Closures, fonctions d'ordre supérieur, pattern matching |
| Générique | Types paramétriques, contraintes et réutilisation du code |
| Système | Pointeurs, mémoire explicite et interfaces bas niveau |
| Événementiel | Gestionnaires d'événements et callbacks |
| Concurrent | Tâches, threads, canaux et primitives atomiques |
| Déclaratif | Descriptions de données, configurations et DSL lorsque pertinent |

Ces paradigmes devront pouvoir coexister dans un même projet, sans obliger le développeur à choisir une variante distincte du langage.

### 4.3. Système de types

Le système de types sera statique par défaut. Il devra prévoir : entiers signés et non signés de tailles définies ; nombres flottants ; booléens et caractères ; chaînes et vues de chaînes ; tableaux, vecteurs, maps et ensembles ; structures et énumérations ; classes et interfaces ; types génériques ; types optionnels ; types de résultats et de gestion d'erreurs ; types de fonctions ; références, pointeurs et types bas niveau ; types vectoriels adaptés au calcul scientifique.

Les règles de conversion, de débordement, d'alignement mémoire et de représentation binaire devront être documentées. L'inférence de types ne devra pas rendre le comportement du programme difficile à comprendre. Le développeur devra pouvoir annoter explicitement les types lorsque la précision est importante.

### 4.4. Programmation orientée objet

Fonctionnalités obligatoires : classes et instances ; constructeurs et destructeurs, avec des règles de durée de vie définies ; méthodes d'instance et méthodes statiques ; interfaces ou traits ; composition ; héritage lorsque nécessaire ; polymorphisme statique et dynamique ; encapsulation ; génériques ; propriétés et opérateurs personnalisables dans des limites définies.

La composition et les interfaces devront être privilégiées pour les abstractions qui n'ont pas besoin d'héritage.

### 4.5. Métaprogrammation

NEXUL devra prévoir progressivement : génération de code contrôlée ; macros ou mécanismes équivalents ; constantes évaluées à la compilation ; réflexion limitée et documentée ; spécialisations génériques ; génération de bindings ; descriptions de données permettant de générer des sérialiseurs et des adaptateurs.

Ces fonctionnalités ne devront pas rendre la compilation imprévisible ni compliquer excessivement l'analyse du code.

## 5. Gestion de la mémoire et programmation bas niveau

C'est une exigence centrale de NEXUL. Le langage doit être suffisamment proche du matériel pour le développement système, tout en offrant un environnement sûr pour les applications ordinaires.

### 5.1. Modèle mémoire

Le modèle recommandé est une gestion déterministe et sûre par défaut, avec la possibilité de contrôler explicitement les ressources. Il devra comprendre : vérification de propriété et de durée de vie ; références et emprunts lorsque cela est pertinent ; allocations dynamiques contrôlées ; allocateurs personnalisés ; arènes mémoire et pools ; gestion explicite des ressources système ; contrôle des accès mémoire ; mécanismes RAII ou équivalents ; absence de garbage collector obligatoire pour les programmes natifs ; possibilité d'utiliser des pointeurs bruts dans des contextes explicitement non sûrs.

Le modèle devra être spécifié avec précision avant la stabilisation du langage.

### 5.2. Accès bas niveau

NEXUL devra permettre, lorsque la cible le justifie : l'utilisation de pointeurs et d'adresses ; le contrôle de l'alignement des données ; le contrôle de la représentation mémoire des structures ; l'appel de fonctions système ; l'utilisation d'instructions SIMD ; l'intégration d'assembleur, selon l'architecture et le backend ; la manipulation de registres et de périphériques via les interfaces appropriées ; l'accès aux registres mémoire-mappés des microcontrôleurs ; l'utilisation de bibliothèques natives et de pilotes existants.

L'accès aux ressources matérielles devra rester dépendant des privilèges et des mécanismes du système cible. Un programme ordinaire ne doit pas pouvoir accéder librement à toute la mémoire physique d'un système protégé.

### 5.3. Runtime minimal

Le runtime devra être modulaire. Les programmes qui n'ont pas besoin d'allocation dynamique, de réseau, de threads ou de services complexes devront pouvoir être compilés sans embarquer ces fonctionnalités. Cela permettra de viser les environnements embarqués, les exécutables de petite taille et les systèmes soumis à des contraintes strictes de mémoire.

## 6. Performance et optimisation

### 6.1. Objectif de performance

NEXUL devra viser des performances comparables à celles de C et C++ pour les charges de travail pertinentes, tout en offrant des abstractions de haut niveau. Cet objectif devra être démontré par des benchmarks reproductibles. Il ne devra pas être présenté comme une garantie absolue avant validation.

### 6.2. Optimisations obligatoires

Le compilateur devra progressivement prendre en charge : élimination du code mort ; propagation de constantes ; inlining ; analyse et optimisation interprocédurales ; vectorisation SIMD ; optimisation du layout mémoire ; optimisation des appels de fonctions ; link-time optimization ; profilage guidant les optimisations ; compilation incrémentale ; cache de compilation ; sélection d'instructions selon la cible ; options permettant de contrôler le compromis entre vitesse, taille et temps de compilation.

Les optimisations devront préserver la sémantique du programme.

### 6.3. Absence d'interprétation obligatoire

Les applications natives classiques devront être compilées avant leur exécution. Un runtime pourra être utilisé lorsqu'il est nécessaire, par exemple pour les services asynchrones, l'interopérabilité ou certains environnements portables, mais il ne devra pas imposer une interprétation permanente du code source. Des modes de développement plus dynamiques pourront exister en complément, sans constituer le modèle d'exécution principal.

### 6.4. Méthode de validation

Les performances seront évaluées face à des implémentations optimisées en C, C++, Rust et, selon le domaine, d'autres langages. Les tests devront couvrir les calculs, la mémoire, le réseau, le multithreading, les démarrages à froid, la taille des exécutables et le temps de compilation. Les résultats devront être publiés avec la configuration matérielle, le système, les versions du compilateur et les options utilisées.

## 7. Microcontrôleurs, firmware et systèmes temps réel

NEXUL devra pouvoir descendre jusqu'aux environnements dans lesquels les ressources sont très limitées. Cette capacité est essentielle pour se distinguer d'un langage essentiellement destiné aux applications ou aux serveurs.

### 7.1. Microcontrôleurs

Le langage devra viser, à terme, les microcontrôleurs compatibles des familles ARM Cortex-M, RISC-V et d'autres architectures prises en charge. Exigences : génération de code sans système d'exploitation ; démarrage depuis le code de réinitialisation de la cible ; prise en charge des fichiers de démarrage et des scripts de linkage ; contrôle de la mémoire Flash et RAM ; prise en charge des interruptions ; accès aux registres des périphériques ; intégration des SDK et des HAL des fabricants ; compilation croisée ; génération des fichiers firmware adaptés à la cible ; analyse de la consommation mémoire ; prise en charge du débogage matériel lorsque la toolchain le permet.

Le langage devra permettre de travailler sans allocation dynamique, sans threads et sans runtime généraliste lorsque ces fonctionnalités ne sont pas nécessaires.

### 7.2. Temps réel

Fonctionnalités : délais d'exécution prévisibles ; contrôle des allocations ; synchronisation explicite ; gestion des interruptions ; priorités des tâches ; contrôle des sections critiques ; absence de pauses imprévues dues à un garbage collector obligatoire ; intégration avec les systèmes d'exploitation temps réel existants.

Une distinction devra être faite entre un langage permettant de développer un logiciel temps réel et un système réellement certifié pour une utilisation critique. Les certifications de sûreté et les garanties de temps d'exécution dépendront aussi du matériel, des bibliothèques, de la chaîne de compilation et des processus de validation.

## 8. Développement web et serveurs

### 8.1. Backend

La bibliothèque réseau devra proposer : serveurs HTTP ; clients HTTP ; WebSocket ; communication TCP et UDP ; prise en charge des protocoles TLS ; DNS ; sérialisation JSON et autres formats courants ; accès aux bases de données ; gestion des connexions ; opérations asynchrones ; journalisation et métriques ; gestion des délais d'attente, des erreurs et de l'annulation ; interfaces avec les systèmes de déploiement. Les fonctionnalités HTTP/2, HTTP/3 et QUIC seront ajoutées en fonction de la maturité des bibliothèques disponibles et des besoins du projet.

### 8.2. Frontend web

Le langage devra permettre de développer de la logique applicative exécutée dans un navigateur grâce à WebAssembly, avec des interfaces vers JavaScript et les API du navigateur : génération de modules WebAssembly ; intégration avec le DOM via les mécanismes disponibles ; gestion des événements utilisateur ; appels aux API web ; échanges asynchrones avec les serveurs ; sérialisation des données ; outils de débogage et de profilage adaptés au navigateur.

NEXUL ne devra pas prétendre supprimer les API JavaScript du navigateur. Il devra pouvoir s'y intégrer efficacement.

### 8.3. Serveurs haute performance

Le langage devra permettre de développer des serveurs capables de gérer de nombreuses connexions simultanées, tout en contrôlant : les allocations mémoire ; les files d'attente ; la contention entre threads ; les délais d'attente ; la consommation de ressources ; la latence ; les erreurs réseau ; les mécanismes de limitation de charge.

La bibliothèque standard devra fournir des abstractions simples, tout en laissant la possibilité de contrôler finement le comportement lorsque les performances sont prioritaires.

## 9. Applications de bureau et mobiles

### 9.1. Applications de bureau

NEXUL devra fournir des interfaces vers les fenêtres natives, les événements utilisateur, le système de fichiers, les notifications, l'audio et les API propres aux systèmes d'exploitation. La bibliothèque graphique pourra proposer une couche multiplateforme, tandis que les développeurs avancés conserveront la possibilité d'appeler directement les API natives.

### 9.2. Applications mobiles

La prise en charge mobile devra permettre : de partager le code métier entre plateformes ; de construire les composants applicatifs communs ; d'intégrer les SDK Android et iOS ; d'accéder aux fonctions du système selon les permissions accordées ; de gérer les notifications, les capteurs, le réseau et le stockage ; de produire des artefacts compatibles avec les chaînes de compilation et de signature des plateformes.

Il ne sera pas nécessaire de réécrire tout le code pour chaque plateforme, mais certaines interfaces et intégrations devront rester spécifiques au système cible.

### 9.3. Interfaces graphiques

La bibliothèque UI devra couvrir : fenêtres et composants ; gestion des événements ; accessibilité ; rendu 2D ; gestion des polices ; mise en page ; gestion des entrées clavier, souris et tactile ; intégration avec des composants natifs ; outils de conception et de débogage. Une interface cohérente sera proposée, sans interdire les intégrations avec les frameworks existants.

## 10. Jeux vidéo, 3D, simulation et GPU

NEXUL devra être capable de servir de langage pour des jeux vidéo, des moteurs 3D, des outils de rendu et des applications de simulation. Il faut toutefois distinguer les capacités du langage et celles des bibliothèques graphiques : le langage doit rendre ces applications possibles, tandis que le moteur fournit les systèmes spécialisés.

### 10.1. Graphisme et rendu

Rendu 2D et 3D ; gestion des fenêtres et des entrées ; accès aux API graphiques adaptées à la plateforme ; gestion des textures, maillages et matériaux ; shaders ; calculs sur GPU ; gestion des ressources graphiques ; gestion des pipelines de rendu ; synchronisation CPU/GPU ; prise en charge progressive des techniques modernes de rendu. Les intégrations pourront s'appuyer sur Vulkan, Direct3D, Metal, OpenGL ou d'autres API en fonction des systèmes pris en charge.

### 10.2. Moteurs de jeu

Boucles de jeu ; systèmes d'entités et de composants ; animation ; physique ; audio spatial ; gestion des assets ; systèmes de particules ; navigation ; scripts et outils de création ; réseau multijoueur ; sauvegarde et chargement ; outils de profilage des performances. NEXUL devra aussi pouvoir être utilisé pour développer des extensions et des composants de moteurs existants.

### 10.3. Performance graphique

Le langage devra proposer un contrôle explicite des allocations, une gestion prévisible des ressources et des interfaces efficaces avec le GPU. Les abstractions graphiques ne devront pas obliger les développeurs à utiliser un moteur complet lorsqu'ils veulent seulement accéder à une API graphique.

## 11. Cybersécurité, cryptographie et protection des applications

La sécurité devra être une propriété transversale du langage, et non une bibliothèque ajoutée tardivement.

### 11.1. Sécurité mémoire

Les règles de base devront viser la prévention des erreurs courantes : accès mémoire invalide ; dépassements de capacité ; références pendantes ; double libération ; utilisation après libération ; courses de données ; conversions dangereuses ; gestion incorrecte des erreurs. Les mécanismes de vérification statique devront détecter autant de problèmes que possible avant l'exécution.

### 11.2. Cryptographie

NEXUL devra fournir des API de haut niveau pour utiliser des implémentations cryptographiques éprouvées : hachage ; chiffrement authentifié ; signatures numériques ; dérivation de clés ; génération aléatoire cryptographiquement sûre ; certificats ; gestion des clés ; intégration TLS ; mécanismes d'authentification. Les algorithmes cryptographiques ne devront pas être réinventés sans justification. Les bibliothèques utilisées devront être auditées, maintenues et intégrées selon des interfaces documentées.

### 11.3. Outils de cybersécurité

NEXUL devra pouvoir servir au développement d'outils de défense et d'audit : analyse de protocoles ; analyse de fichiers ; outils de diagnostic réseau ; tests de sécurité ; fuzzing ; analyse statique ; inspection de journaux ; outils d'investigation numérique ; automatisation de contrôles de sécurité ; analyse de logiciels suspects dans des environnements isolés. Les opérations nécessitant des privilèges particuliers devront rester explicites et respecter les permissions du système.

### 11.4. Sécurité de la chaîne logicielle

Le compilateur et le gestionnaire de paquets devront inclure des mécanismes pour : vérifier l'intégrité des dépendances ; enregistrer leur provenance ; détecter les versions vulnérables ; prendre en charge des signatures vérifiables ; produire des builds reproductibles lorsque l'environnement le permet ; analyser les dépendances transitives ; générer des rapports de sécurité.

## 12. Interopérabilité universelle avec les autres langages

Il ne faut pas confondre inclusion de code source, appel de bibliothèque, communication entre processus et intégration de runtime. Le langage devra prendre en charge ces différents mécanismes, avec des règles distinctes.

### 12.1. ABI native

La première interface à stabiliser sera une ABI C documentée. Elle devra permettre : d'appeler des fonctions C depuis NEXUL ; d'exposer des fonctions NEXUL à des applications C ; de créer des bibliothèques natives ; de définir des conventions d'appel ; de gérer les types compatibles ; de documenter la propriété et la durée de vie des ressources ; de produire des bindings à partir d'interfaces décrites. La compatibilité C++ sera assurée lorsque possible par des wrappers ou des interfaces C stables, sans promettre une ABI C++ universelle entre tous les compilateurs et systèmes.

### 12.2. Python

NEXUL devra pouvoir appeler des fonctions Python au moyen d'un adaptateur et du runtime Python, lorsque celui-ci est disponible, et exposer des fonctions natives NEXUL à des programmes Python.

### 12.3. Bash et autres shells

Lancement de processus ; passage d'arguments structurés ; capture des sorties ; récupération du code de retour ; gestion des délais d'attente ; gestion des signaux ; pipelines ; intégration de scripts Bash, PowerShell et d'autres shells disponibles sur la plateforme. Les scripts externes devront être considérés comme des programmes séparés, avec leurs dépendances, leurs permissions et leurs erreurs.

### 12.4. Autres langages

| Écosystème | Mécanisme principal envisagé |
|---|---|
| C | ABI native |
| C++ | Wrappers et interfaces natives |
| Rust | ABI C et bindings |
| Python | API native et runtime |
| JavaScript / TypeScript | WebAssembly et bindings web |
| Java / Kotlin | JNI ou interfaces natives adaptées |
| C# | Interfaces natives et P/Invoke |
| Go | Interfaces natives et appels via C |
| Bash / PowerShell | Processus et scripts externes |
| SQL | Pilotes et connecteurs de bases de données |

Chaque intégration devra définir les conversions de types, la gestion des erreurs, la durée de vie des ressources et les limites de performance.

## 13. Concurrence, parallélisme et calcul scientifique

Fonctionnalités requises : threads natifs ; tâches légères ; opérations asynchrones ; canaux de communication ; atomiques et modèles mémoire définis ; synchronisation ; annulation des tâches ; pools de threads ; SIMD ; accès aux GPU ; calcul scientifique ; bibliothèques de matrices et de vecteurs ; interfaces avec les bibliothèques de calcul existantes.

Le langage devra distinguer les opérations concurrentes, les opérations parallèles et les opérations asynchrones. Les abstractions ne devront pas masquer les coûts importants lorsque la performance est essentielle.

## 14. Bibliothèque standard et écosystème

### 14.1. Bibliothèque standard

| Module | Responsabilité |
|---|---|
| core | Types essentiels, erreurs, interfaces |
| collections | Tableaux, vecteurs, maps, ensembles |
| text | Chaînes, encodage, manipulation de texte |
| math | Mathématiques et opérations numériques |
| memory | Allocation et ressources |
| system | Interfaces système |
| io | Entrées-sorties |
| net | Réseau |
| crypto | Interfaces cryptographiques |
| async | Concurrence et asynchronisme |
| testing | Tests unitaires et outils de validation |
| logging | Journalisation |
| serialization | Formats de données et sérialisation |

La bibliothèque standard devra rester raisonnable en taille. Les fonctionnalités très spécialisées seront distribuées dans des bibliothèques indépendantes.

### 14.2. Bibliothèques spécialisées

Bases de données ; serveurs web ; interfaces utilisateur ; graphisme 2D et 3D ; audio et vidéo ; traitement d'images ; intelligence artificielle ; calcul scientifique ; outils industriels ; communications sécurisées ; systèmes embarqués ; outils de développement.

### 14.3. Gestionnaire de paquets

Nom provisoire : **`nxpm`**. Il devra gérer : installation et suppression de dépendances ; versions et fichiers de verrouillage ; dépendances transitives ; bibliothèques natives ; cache de compilation ; compilation reproductible ; dépôts publics et privés ; signatures et intégrité ; audit de sécurité ; génération de documentation ; compatibilité entre versions du compilateur. L'écosystème devra permettre à des développeurs tiers de publier leurs bibliothèques sans devoir modifier le compilateur lui-même.

## 15. Outils de développement officiels

### 15.1. Outils obligatoires

| Outil | Nom provisoire | Fonction |
|---|---|---|
| Compilateur | `nxc` | Compilation et diagnostics |
| Gestionnaire de paquets | `nxpm` | Dépendances et projets |
| Formateur | `nxfmt` | Formatage automatique |
| Linter | `nxlint` | Analyse du code |
| Testeur | `nxtest` | Tests automatisés |
| Débogueur | `nxdebug` | Inspection et débogage |
| Profileur | `nxprof` | Mesure des performances |
| Générateur de documentation | `nxdoc` | Documentation des API |
| Outil de projet | `nx` | Création et gestion des projets |
| Serveur de langage | NEXUL LSP | Autocomplétion et intégration IDE |

Les noms sont provisoires et devront être harmonisés avant la première version publique.

### 15.2. Environnement de développement

Autocomplétion ; navigation entre déclarations ; recherche de références ; affichage des erreurs en temps réel ; refactorisation ; débogage pas à pas ; inspection des variables ; profilage ; documentation contextuelle ; gestion des dépendances ; intégration des tests ; analyse de la mémoire.

### 15.3. Documentation

Spécification formelle du langage ; guide de démarrage ; référence syntaxique ; référence de la bibliothèque standard ; guide de programmation système ; guide de gestion de la mémoire ; guide de compilation croisée ; guides par plateforme ; tutoriels web, serveur, jeu et embarqué ; documentation de l'interopérabilité ; guide de sécurité ; exemples complets ; guide de migration entre versions. La documentation devra être versionnée et vérifiée automatiquement autant que possible.

## 16. Architecture logicielle et organisation du dépôt

Grands ensembles : **Compilateur** (analyse syntaxique, types, IR, optimisations et backends) ; **Runtime et cibles** (mémoire, E/S, concurrence et intégrations système) ; **Bibliothèques** (standard, réseau, sécurité, interfaces, calcul et matériel) ; **Écosystème** (paquets, registre, dépendances et outils de publication) ; **Qualité et sécurité** (tests, fuzzing, audits, CI et builds reproductibles). Chaque composant devra disposer d'interfaces documentées et de tests automatisés.

### 16.1. Principes d'architecture

Séparation du frontend et des backends ; séparation du langage et de ses bibliothèques ; runtime optionnel lorsque cela est possible ; interfaces stables entre composants ; tests automatiques pour chaque fonctionnalité ; compatibilité vérifiée entre versions ; documentation des décisions architecturales ; absence de dépendances inutiles dans le noyau du compilateur ; possibilité de compiler le compilateur lui-même sur les plateformes supportées.

### 16.2. Langage d'implémentation du compilateur

Le premier compilateur pourra être développé en C++ ou en Rust, selon les compétences de l'équipe et les contraintes du projet. Le choix devra être motivé par la maturité des outils, l'intégration avec LLVM, la facilité de maintenance et la capacité à développer un compilateur robuste. Le bootstrapping ne devra pas être une condition préalable à la première version.

## 17. Tests, validation et assurance qualité

### 17.1. Tests du langage

Grammaire ; types ; erreurs de compilation ; règles de propriété ; génération de code ; optimisations ; linkage ; gestion des erreurs à l'exécution ; interopérabilité ; différentes architectures ; bibliothèques standard. Chaque nouvelle fonctionnalité devra être accompagnée de tests de non-régression.

### 17.2. Tests de sécurité

Fuzzing du lexer et du parser ; tests de robustesse du compilateur ; analyse statique ; tests des interfaces non sûres ; audit des dépendances ; tests de gestion des entrées non fiables ; tests de concurrence ; vérification de la provenance des artefacts publiés ; analyse des vulnérabilités des outils de compilation.

### 17.3. Tests de performance

Vitesse d'exécution ; latence ; débit ; consommation mémoire ; taille des binaires ; temps de compilation ; performances du réseau ; performances multithread ; performances des calculs vectoriels ; coût des abstractions et des appels entre langages. Les résultats devront être suivis au fil des versions afin d'identifier les régressions.

### 17.4. Tests de compatibilité

Chaque version devra être validée sur une matrice de plateformes et d'architectures officiellement supportées, en debug et release, avec bibliothèques natives, compilations croisées et environnements où certaines fonctionnalités sont indisponibles. Une fonctionnalité ne sera déclarée portable que si ses contraintes et ses limitations sont documentées.

## 18. Développement du projet : phases et jalons

| Phase | Nom | Contenu |
|---|---|---|
| 0 — Conception | Spécification formelle | Syntaxe, types, modèle mémoire, modules, erreurs, cibles, interopérabilité. Documents de référence avant de figer les choix fondamentaux. |
| 1 — Prototype | Première chaîne de compilation | Lexer, parser, système de types initial, génération de code minimale. Fonctions, structures de contrôle, diagnostics. |
| 2 — Natif | Premier compilateur exploitable | Exécutables natifs, linkage, bibliothèque de base, tests, outils de débogage. |
| 3 — Système | Mémoire et interopérabilité | Modèle mémoire stabilisé, ABI C, bibliothèques natives, premières fonctionnalités de concurrence. |
| 4 — Écosystème | Outils et bibliothèques | Gestionnaire de paquets, outils IDE, réseau, bases de données, premiers frameworks. |
| 5 — Portabilité | Multiples cibles | Compilation croisée, WebAssembly, mobile, premières cibles embarquées. |
| 6 — Maturité | Plateforme industrielle | Performances, audits de sécurité, documentation, compatibilité ascendante, écosystèmes graphiques, scientifiques et industriels. |

Les délais devront être fixés après évaluation des ressources humaines, des compétences disponibles, des plateformes prioritaires et de la complexité réelle du prototype. La prise en charge complète de tous les domaines représente un effort de plusieurs années.

## 19. Exigences fonctionnelles finales

### 19.1. Tableau de conformité

- **Compilateur** : compilation native sans interprétation obligatoire ; compilation séparée et édition de liens ; diagnostics et débogage ; optimisations configurables.
- **Langage** : typage statique et inférence ; programmation procédurale, objet et fonctionnelle ; gestion explicite de la mémoire ; modules et génériques.
- **Portabilité** : plusieurs systèmes d'exploitation ; cibles x86-64 et ARM64 ; WebAssembly ; toolchains embarquées compatibles.
- **Écosystème** : bibliothèque standard ; gestionnaire de paquets ; tests et documentation ; interopérabilité C et Python.
- **Domaines** : applications et serveurs ; web et réseau ; jeux et graphisme 3D ; sécurité et cryptographie ; microcontrôleurs et calcul spécialisé.

Cette liste constitue un suivi de projet, et non une certification automatique. Chaque élément devra être associé à des tests et à des preuves vérifiables. (Suivi : voir `docs/roadmap.md`.)

### 19.2. Critères d'acceptation

Le projet devra être considéré comme prêt pour une publication stable uniquement lorsque :

1. Le langage dispose d'une spécification publique et versionnée.
2. Le compilateur produit des exécutables natifs sur les plateformes officiellement annoncées.
3. Le système de types et les règles mémoire sont documentés et testés.
4. La compilation, le linkage, les tests et le débogage fonctionnent de manière reproductible.
5. Les bibliothèques et outils essentiels sont disponibles et maintenus.
6. L'interopérabilité annoncée est démontrée par des projets de référence.
7. Les performances sont mesurées et documentées.
8. Les vulnérabilités critiques identifiées font l'objet d'un processus de correction.
9. La documentation permet à un développeur externe de créer, compiler, tester et distribuer un projet.
10. Les limitations de chaque plateforme sont clairement publiées.

## 20. Contraintes, risques et limites à anticiper

| Risque | Mesure préventive |
|---|---|
| Périmètre trop large | Définir des versions successives et des cibles prioritaires |
| Compilateur trop complexe | S'appuyer initialement sur LLVM et limiter la première grammaire |
| Difficulté du modèle mémoire | Spécifier et tester les règles avant de stabiliser le langage |
| Performances insuffisantes | Benchmarks continus et analyse des régressions |
| Bibliothèques peu nombreuses | Faciliter les bindings et la réutilisation des bibliothèques existantes |
| Fragmentation des plateformes | Tests automatisés et matrice de compatibilité officielle |
| Vulnérabilités des dépendances | Audit, signatures, provenance et mises à jour |
| Ruptures de compatibilité | Versionnement du langage, des paquets et des ABI |
| Difficulté d'adoption | Documentation pédagogique, outils fiables et exemples concrets |

**Une limite fondamentale à respecter.** NEXUL doit pouvoir servir à tout type de développement, mais cela ne signifie pas qu'il remplacera immédiatement tous les langages ou toutes les plateformes. Un microcontrôleur sans système d'exploitation, un navigateur web, un moteur 3D et un serveur Linux n'offrent pas les mêmes capacités. Certaines fonctions dépendront nécessairement du matériel, des pilotes, des SDK et des API du système. L'exigence réaliste est donc de partager le langage, la syntaxe, les outils et autant de code que possible, tout en fournissant des mécanismes adaptés à chaque environnement.

## 21. Gouvernance et évolution du langage

À définir : une spécification officielle du langage ; une politique de versionnement ; un processus de proposition et d'approbation des nouvelles fonctionnalités ; une politique de compatibilité ascendante ; un calendrier de maintenance ; une politique de correction des vulnérabilités ; des règles de contribution ; une politique de publication des compilateurs ; une procédure de dépréciation des fonctionnalités ; une feuille de route publique.

Les changements affectant la syntaxe, les types, le modèle mémoire ou l'ABI devront être examinés avec une attention particulière. Le compilateur, la bibliothèque standard et les outils devront évoluer de manière coordonnée, sans obliger les utilisateurs à réécrire constamment leurs applications.

## 22. Définition du produit final NEXUL

- **Un langage compilé** — code natif optimisé, avec des cibles portables lorsque nécessaire.
- **Un langage universel** — programmation applicative, système, web, jeux, 3D, sécurité et embarqué.
- **Un langage multiparadigme** — procédural, objet, fonctionnel, générique, concurrent et bas niveau.
- **Un langage interopérable** — interfaces natives, bindings, runtimes externes et processus système.
- **Une plateforme complète** — compilateur, bibliothèques, gestionnaire de paquets, outils, documentation et processus de sécurité.

### Conclusion

La mission de NEXUL est de réunir la performance native, la proximité du matériel, la simplicité d'utilisation et l'universalité des applications au sein d'une plateforme de programmation cohérente.

Trois priorités non négociables :

1. **La qualité du langage lui-même** : syntaxe cohérente, système de types solide, mémoire maîtrisée et sémantique bien définie.
2. **La qualité du compilateur** : génération de code efficace, optimisation mesurable, diagnostics fiables et prise en charge progressive des plateformes.
3. **La qualité de l'écosystème** : bibliothèques, interopérabilité, documentation, outils et maintenance à long terme.

Ce cahier des charges constitue la base fonctionnelle et technique de NEXUL. La prochaine étape est de le transformer en une spécification formelle, en fixant précisément la grammaire, les types, le modèle mémoire, l'architecture du compilateur et les critères de validation de la toute première version.

*NEXUL — One Language. Every Possibility.*
