# NEXUL — PASSATION MAÎTRE V3
## Refonte complète du langage, NEXUL Studio natif, auto-hébergement du compilateur, extension VS Code et coordination de cinq agents

> **Statut : cahier des charges et document de reprise proposé, non encore exécuté.**  
> **Date de référence : 15 octobre 2026**  
> **Dépôt :** https://github.com/KAYTAY10ok/NEXUL  
> **Révision `main` observée :** `548b1f6fdd9b9cdd4c3040906cc25ba7b6b98fb3`  
> **Sources consolidées :** `NEXUL - consignes des agents.md` fourni par l'utilisateur, passation V2, fichiers publics du dépôt, PR fusionnées #1 et #3–#8, rapports des agents, exigences les plus récentes de l'utilisateur.  
> **Portée :** transmission intégrale à une nouvelle session de Claude Code, Codex ou autre agent. Le présent document prime sur les anciennes répartitions *uniquement après validation de la nouvelle gouvernance par le responsable humain*.

---

# 0. MANDAT EXÉCUTIF : CE QUI DOIT ÊTRE LIVRÉ À LA FIN

**Ce projet ne consiste PAS en une simple amélioration de l'IDE. Le travail demandé est une refonte profonde, ambitieuse et achevée du langage NEXUL lui-même et de son écosystème.**

À la fin du développement, NEXUL doit constituer une **version majeure stable et complète dans le périmètre de sa spécification**, pas une collection de démonstrations partielles, de mots-clés non implémentés ou de prototypes qui ne peuvent pas compiler de vrais programmes. Il doit être possible d'écrire, compiler, tester, déboguer, exécuter et distribuer des programmes NEXUL substantiels en s'appuyant prioritairement sur le langage NEXUL lui-même.

La vision de NEXUL est la suivante :

1. **Langage indépendant à 100 % sur le plan de la sémantique :** lexer, grammaire, système de types, modèle mémoire, règles de concurrence, runtime et bibliothèques NEXUL. Pas de transpilation obligatoire en C/Rust/JS ; backend LLVM autorisé. Un outil ou IDE écrit initialement en Rust ne change pas cette identité.
2. **Syntaxe propre et immédiatement reconnaissable :** design original, cohérent, lisible, concis sans obscurité, agréable pour débutants comme professionnels. Une véritable refonte de syntaxe, pas un simple renommage cosmétique des mots-clés de Rust/C. Prévoir migration des programmes 0.2 et diagnostics de compatibilité.
3. **Typage statique fort et sécurité renforcée :** contraintes de types pertinentes, conversions contrôlées, invariants, références et ressources sûres, pas de comportement caché ; API expressive et compatible avec la philosophie de NEXUL.
4. **Programmation orientée objet complète et moderne :** encapsulation, méthodes, interfaces/traits, abstraction, polymorphisme, composition, génériques, contrôle de visibilité, dispatch statique et dynamique, construction/destruction sûre, sans obligation d'imiter l'héritage classique.
5. **Concurrence sûre :** threads/tâches, communication, atomiques et mécanismes de synchronisation, contrôle statique des transferts/partages, absence de courses de données dans le code sûr selon un modèle mémoire formalisé.
6. **Programmation temps réel sérieuse :** outils et profils déterministes, contraintes explicites sur mémoire/latence/ordonnancement/interruptions, mesures sur cibles définies ; garanties hard real-time uniquement si elles sont réellement démontrées dans un environnement matériel/RTOS précis.
7. **Rapidité, efficacité et précision :** performances compétitives sur des charges natives représentatives, forte fiabilité numérique, modes de calcul aux compromis explicités, benchmarks comparatifs honnêtes.
8. **Compilation autonome, puis *self-hosting* :** aboutir à un compilateur NEXUL écrit principalement en NEXUL, capable de compiler sa propre source, avec bootstrap vérifiable et reproductible.
9. **Utilisation dans plusieurs domaines :** programmes natifs, outillage, WebAssembly navigateur/WASI, applications système, web, composants embarqués et temps réel sur les cibles annoncées. Aucun besoin de faire croire que toutes les cibles sont identiques.
10. **Deux environnements d'édition :** NEXUL Studio, une vraie application de bureau native et professionnelle, **et une extension officielle pour Visual Studio Code**, permettant d'écrire et de développer en NEXUL depuis VS Code.
11. **Qualité de livraison :** tests réels, compatibilité documentée, installation opérationnelle, critères de finition satisfaits et limitations résiduelles explicites. Le mot « terminé » n'est autorisé que lorsque les portes de qualité sont franchies.

**Directive de pilotage : CLAUDE OPUS 5.5 — MAIN devient le principal acteur du projet et le responsable technique global.** L'ancien rôle « Claude Studio » est renommé **Claude Main**. Il conserve l'autorité sur NEXUL Studio natif et devient l'architecte et intégrateur principal de l'ensemble. **Claude Opus 5.5 — Outils** est le propriétaire d'implémentation des changements sémantiques critiques, mais chaque changement de langage est conçu et approuvé par les deux Opus. Codex Astral ne les aide que ponctuellement ; ni Luna ni Gemma n'écrivent de règles sémantiques critiques.

## 0.1 Ce que signifie « fini »

« Fini » veut dire **toutes les fonctionnalités déclarées obligatoires dans une version majeure définie sont complètes et testées**. On ne peut techniquement pas garantir qu'un langage traite sans aucune limite *toute* application imaginable, que son temps réel dur fonctionne sur n'importe quel système, ni qu'il surpasse C dans tous les programmes. L'objectif est donc un langage de production cohérent, avec des garanties **mesurables, prouvables lorsque nécessaire et explicitement bornées par plateforme et profil**. Toute fonction indispensable impossible à réaliser devient un **blocage documenté**, pas une case cochée fictivement.

La livraison finale ne se résume pas à « on a écrit du code » : compilation de projets non triviaux, tests, outils installables et expérience utilisateur démontrée sont obligatoires.

---

# 1. ÉTAT RÉEL À LA PASSATION — NE PAS REFAIRE CE QUI EXISTE

**Référence Git publique vérifiée :** `main` à `548b1f6` le 15/10/2026 ; 7 PR de refonte fusionnées (#1, #3, #4, #5, #6, #7, #8), environ 30 commits et 105 fichiers affectés entre la base `d537a7b` et cette référence. Ces chiffres ne prouvent pas la qualité ni l'achèvement de toutes les fonctions. Des versions locales non poussées peuvent être plus avancées : relire Git et les fichiers d'état avant toute édition.

| Composant | Ce qui est implémenté dans le dépôt | Ce qui manque / doit être vérifié |
|---|---|---|
| NEXUL 0.2 | Lexer/parser/typage statique/inférence, NIR, backend LLVM natif, runtime et bibliothèque standard en NEXUL, structures/traits/génériques/`dyn`, ownership et RAII, `nxc`/`nxpm` | Spécification majeure révisée, concurrence, garanties RT, nouvelles contraintes de typage, syntaxe distinctive finalisée, self-hosting |
| WebAssembly navigateur | `wasm32-web`, ABI hôte v1, `lib/web/nexul.mjs`, exemple `examples/web` ; tests rapportés dans la PR #1 | Intégration graphique et modèle de projet dans Studio ; tests navigateurs supplémentaires, limite des appels synchrones et du DOM |
| Studio natif | `studio/app` : première application GPUI, éditeur, onglets, explorateur, thèmes, panneaux, F5 et services NEXUL | Parité avec ancien Studio, finitions UX, intégration de toutes les nouvelles crates, installer natif, scénarios de fermeture sûre |
| Fichiers et dossiers Studio | Création, renommage, déplacement, copier/coller, Corbeille et glisser-déposer ajoutés | Tester tous les cas réels et les scénarios de perte de données |
| Coloration NEXUL | `studio/syntax`, analyse lexicale incrémentale, tests de conformité déclarés | Raccorder à `studio/app` et préserver diagnostics/couleurs |
| LSP générique | `studio/lsp`, client + détection SDK/langages, tests déclarés | Raccorder à la fenêtre GPUI ; `nxc lsp` encore à développer selon demandes historiques |
| Terminal | `studio/terminal`, moteur PTY et émulation ANSI | Vraie vue interactive dans Studio, essais Windows ConPTY en conditions réelles |
| DAP | `studio/dap`, client débogage et adaptateurs | UI native, validation avec vrai nxc/LLDB |
| Modèles | `studio/templates`, NEXUL, sites web, Python/C/C++/Rust/C# | Assistant GPUI ; modèle NEXUL + navigateur fondé sur l'ABI v1 |
| Installateur | L'ancien pipeline est opérationnel selon sa documentation | `tools/build_installer.py` copie encore l'ancien `NEXUL-Studio.exe` ; distribuer le nouveau binaire natif correctement |
| Benchmarks | Tests de précision numérique existants et vérificateurs renforcés | Référence unifiée NEXUL/C/Rust/C#, 1M/10M/100M, analyse LLVM, performances sur même machine |
| Self-hosting | Compilateur écrit en Rust et fonctionnel | Compilateur NEXUL écrit en NEXUL, bootstrap en plusieurs étages et conformité |
| VS Code | Pas d'extension NEXUL officielle identifiée dans les chemins publics inspectés | Créer et tester `editors/vscode` (ou chemin retenu dans la nouvelle charte) |

### Anomalies concrètes du Studio à régler d'abord

- `studio/app/src/studio.rs` sauvegarde un document modifié lors de la fermeture d'onglet sans présenter le choix normal **Enregistrer / Ne pas enregistrer / Annuler** ; `studio/app/src/main.rs` termine la fenêtre sans stratégie de récupération manifestement complète : audit P0.
- `studio/app/Cargo.toml`, à la date de référence, ne raccorde pas encore `studio/syntax`, `studio/lsp`, `studio/terminal`, `studio/dap` et `studio/templates`.
- L'installateur copie le binaire de l'ancien Studio, pas `NEXUL-Studio-Native.exe`.
- Ancienne interface Edge/HTTP dans `studio/src`, `studio/ui` : ne supprimer qu'après parité vérifiée ; une WebView n'est acceptable que dans un panneau d'aperçu web isolé, pas comme coquille de l'IDE.
- `nxc check` sur un tampon non sauvegardé, la version des diagnostics JSON, `nxc lsp` et le formatage ont fait l'objet de demandes : vérifier leur statut réel avant de répondre.
- Fonctionnement du runtime macOS signalé comme incorrect (mauvaise couche système sélectionnée) dans l'audit Core ; pas de prétendue compatibilité macOS complète avant correctif et tests.

### Fichiers et historique à relire avant la reprise

`docs/refonte/AGENTS.md`, `docs/refonte/{claude-status,claude-outils-status,codex-status,codex-exec-status,audit-core,interop-web}.md`, `docs/studio/{audit-phase0,architecture-native}.md`, `docs/roadmap.md`, `docs/spec/**`, `studio/{app,lsp,syntax,terminal,dap,templates}/README.md`, `examples/web/**` ; ancien fichier utilisateur `NEXUL - consignes des agents.md` ; passation V2.

**Attention au caractère historique :** les rapports d'état peuvent encore dire qu'une PR n'est pas fusionnée ou que certains agents n'ont pas commencé. Les vérifications Git courantes priment sur ces mentions anciennes. Les tests déclarés par les agents ne sont pas automatiquement une validation CI ou indépendante.

---

# 2. GOUVERNANCE : CLAUDE MAIN DIRIGE CINQ AGENTS

## 2.1 Noms officiels et niveaux de responsabilité

| Agent | Fonction | Autorité et périmètre |
|---|---|---|
| **Claude Opus 5.5 — MAIN** *(ancien Claude Studio)* | **Chef de projet technique, architecte en chef, intégrateur principal, responsable Studio et codécideur du langage** | Définit l'ordre des travaux, tranche les questions de conception après analyse, approuve toutes les changements de sémantique avec Claude Outils, coordonne les PR, valide les tests transversaux. Possède `studio/app/**` et l'interface. Ne modifie pas les fichiers critiques de Claude Outils sans transfert écrit. |
| **Claude Opus 5.5 — OUTILS / CORE** | **Ingénieur principal du compilateur et de la sûreté NEXUL** | Seul auteur régulier de `compiler/**`, `lib/**`, `docs/spec/**` et des tests du langage après transfert. Conçoit avec Claude Main syntaxe, typage, ownership, OOP, concurrence, RT et self-hosting ; implémente par étapes revues. |
| **Codex ASTRAL** | **Expert ponctuel en LLVM, analyse et performance** | Lecture, notes techniques, benchmarks séparés et revues sur mandat explicite de Claude Main ou Claude Outils. Un patch critique uniquement si sous-périmètre autorisé, revu et signé par les deux Opus. Ne peut pas changer seul la syntaxe, la sûreté, le modèle mémoire ou le RT. |
| **Codex LUNA** | **Ingénieur composants Studio, intégrations périphériques, VS Code et distribution** | Maintient terminal/DAP/templates, implémente l'extension VS Code dans un dossier neuf réservé, raccordements auxiliaires non-GPUI, packaging/installateur après transfert. Fournit API et preuves à Claude Main. Aucun accès d'écriture au langage critique. |
| **Gemma 4:31B — Claude Code** | **Assistant QA/documentation/corpus et tâches non critiques** | Élabore checklists, guides, matrices et exemples proposés, tests de fumée isolés, inventaires ; aucun pouvoir de fusion, pas d'édition du compilateur ou du code de sûreté, pas de réécriture de specs normative sans revue Opus. |

**Important :** ce tableau est une assignation de responsabilités et non une affirmation mesurée sur les capacités exactes des modèles. Le travail risqué est confié aux deux Opus selon la décision du propriétaire du projet ; les activités secondaires sont limitées et vérifiables.

**Claude Main est le point d'entrée unique.** Tous les agents lui soumettent leurs livrables, rapports, risques et demandes. Claude Main propose les arbitrages ; le propriétaire humain du dépôt décide des changements de cap majeurs et des fusions.

## 2.2 Propriété des fichiers — zéro collision

- **Claude Main :** `studio/app/**`, `studio/src/**`, `studio/ui/**`, `studio/plugins/**`, `studio/assets/**`, `studio/Cargo.toml`, `studio/README.md`, `docs/studio/**`, `docs/refonte/claude-main-status.md` (ou transition temporaire depuis `claude-status.md`). **Seul propriétaire de l'UI GPUI**, des raccordements GUI et de l'intégration VS Code au niveau UX/documentation Studio.
- **Claude Outils/Core :** `compiler/**`, `lib/**`, `tests/run/**`, `tests/ui/**`, `tests/numerics/**`, `tests/web/**`, `docs/spec/**`, `docs/guide.md`, `docs/roadmap.md` pour le langage, `docs/adr/**` relatifs au langage, `docs/refonte/claude-core-status.md`, les sources du **compilateur auto-hébergé** sous `selfhost/**` ou sous `compiler/selfhost/**` selon ADR approuvée. Il maintient aussi les aides NEXUL nécessaires à l'extension, sans écrire son interface VS Code.
- **Codex Astral :** dossiers de revue/mesures explicitement attribués (`docs/refonte/reviews/astral/**`, `benchmarks/astral/**` si autorisés), en lecture seule partout ailleurs par défaut.
- **Codex Luna :** `studio/terminal/**`, `studio/dap/**`, `studio/templates/**`, **`editors/vscode/**`** (extension officielle), `docs/refonte/codex-luna-status.md`. Après mandat écrit, `installer/**`, `tools/build_installer.py`, `docs/installation.md`, et maintenance technique de `studio/lsp/**` et `studio/syntax/**` *sans modification simultanée par Main*.
- **Gemma :** `docs/refonte/gemma/**`, `docs/refonte/gemma-status.md`, éventuellement `tests/manual/**` attribués en propre. Pour toucher un autre chemin, rédiger une proposition et laisser le responsable appliquer ou déléguer le patch.
- **Fichiers transversaux :** `Cargo.toml`, `Cargo.lock` racine, `README.md` racine, `.github/**`, `CHANGELOG.md` et `docs/refonte/AGENTS.md` sont **gérés par Claude Main comme responsable d'intégration** ; si une modification requiert un correctif du compilateur, Main fait une PR d'intégration après les PR des propriétaires. Le compiler peut conserver son propre lockfile ; aucun agent ne modifie concurrentiellement ces fichiers. Le transfert depuis l'ancienne propriété Astral **doit être consigné**.

**Gouvernance et transférabilité :** au moment de l'audit, `docs/refonte/AGENTS.md` et l'ancien document de passation donnent `compiler/**` à Astral, `installer/**` et `studio/lsp|syntax/**` à Claude Outils. **Avant la première écriture** dans un de ces chemins réattribués : Claude Main propose une mise à jour d'`AGENTS.md`, la fait valider par le responsable humain, ferme les branches concurrentes et acquiert l'accord des propriétaires précédents ou consigne le transfert. Ne jamais démarrer avec deux propriétaires.

Pour éviter les conflits sur `docs/refonte/` : chacun a **son fichier** et ne modifie pas celui d'un autre. Les demandes croisées sont des issues GitHub / PR / fichiers de contrat, pas des modifications surprises chez le voisin.

## 2.3 Procédure de travail

1. Récupérer le `main` actuel, identifier la révision de base, les PR ouvertes/fusionnées et les fichiers modifiés en local. Chaque agent utilise son worktree ou clone indépendant et une branche dédiée par lot.
2. Lire les contrats et les API existantes ; vérifier quels modules sont *implémentés*, *raccordés* et *testés*. Aucun agent ne doit réécrire une crate simplement parce que sa fonctionnalité est absente de l'écran.
3. Claude Main tient un tableau maître des lots et des propriétaires. Un seul responsable d'écriture par lot et par fichier ; chaque lot a des critères d'acceptation et une dépendance éventuelle.
4. Développer une PR courte et isolée, avec description, tests réellement exécutés, limitations, impact sur les anciennes applications, preuves du comportement final et demande de revue.
5. Changement sémantique / risque de sécurité : **deux revues obligatoires** (Claude Outils propose et implémente ; Claude Main approuve) + validation humaine pour une rupture de compatibilité ou un nouveau mot-clé fondamental.
6. Mettre à jour le rapport personnel après chaque PR ; Main actualise la matrice de suivi et organise l'intégration.
7. Ne jamais publier directement dans `main` sans revue ; pas de `git reset --hard`, `git push --force`, suppression du travail d'autrui ni `git stash` partagé à l'aveugle.
8. Si un outil n'est pas disponible ou une preuve manque, signaler *non testé*. Ne pas inventer une réussite, un benchmark ou une certification.

---

# 3. REFONTE MAJEURE DE LA SYNTAXE NEXUL — OBLIGATOIRE

## 3.1 Une identité linguistique propre

NEXUL 0.2 emprunte visuellement certaines conventions connues (`fn`, `let`, `var`, blocs `{}`, traits, `impl`). La nouvelle version majeure doit **étudier, définir et adopter une grammaire originale** qui donne au langage une personnalité reconnaissable, **sans copier mot à mot** la structure extérieure de Rust, C++, Java ou Python. L'unicité absolue de tous les symboles n'est ni mesurable ni souhaitable : des signes communs comme `+` ou `=` resteront probablement les plus lisibles. L'exigence porte sur **la cohérence et la signature globale de la syntaxe**.

Contraintes incontournables :

- Un programme NEXUL doit rester **évident à lire** : noms explicites, déclaration claire des données, effets visibles, erreurs compréhensibles. Ne pas fabriquer une syntaxe « extraterrestre » par simple recherche d'originalité.
- Définir un **style rédactionnel NEXUL**, avec des conventions de modules, procédures/fonctions, expressions, déclarations, conditions, boucles, propriétés, méthodes, traits, erreurs, gestion de ressources, concurrence et temps réel.
- Les formes courantes doivent pouvoir être enseignées par un guide officiel. Favoriser cohérence orthographique et régularité plutôt que multiplication d'exceptions.
- Le nouveau langage ne doit pas être un front-end décoratif déléguant l'essentiel des règles à un autre langage ; lexer/parser et analyse sémantique restent sous le contrôle de `nxc`.
- La syntaxe doit être compatible avec la sûreté des ressources et les profils temps réel : les allocations, opérations bloquantes et comportements exceptionnels doivent être identifiables par le compilateur dans les contextes concernés.
- Éviter l'ambiguïté lexicale, les grammaires impossibles à parser de manière fiable, les règles dépendantes d'espaces indéfinis et les raccourcis qui cachent des effets destructifs.

## 3.2 Procédure de création de la grammaire

**Aucune modification irréversible du lexer ou parser avant le livrable G0.**

**G0 — Audit des usages de NEXUL 0.2** : inventorier les formes déjà utilisées par le runtime, tests, exemples, guides et Studio. Identifier les formes suffisamment bonnes à garder, les faiblesses de lisibilité et les ambiguïtés. Évaluer l'impact de chaque changement sur les programmes, le débogueur, les macros/attributs éventuels et les outils d'édition.

**G1 — Trois propositions de style syntaxique** : Claude Main et Claude Outils conçoivent au moins trois propositions crédibles, chacune couvrant **les mêmes 12 programmes** : Bonjour, calcul, structures et méthodes, interface polymorphe, génériques, gestion des erreurs, fichiers, ownership, threads et canaux, tâche périodique RT, programmation web WASM, tests. Pour chaque proposition : analyse de lisibilité, nombre de symboles/termes, risques et complexité du parser.

**G2 — Spécification de la proposition retenue** : document d'architecture signé des deux Opus et validé par l'utilisateur, puis lexique, grammaire formelle EBNF, précédence des opérateurs, règles de nommage et commentaires, style standard et traitement des ambiguïtés. Les exemples de la spécification doivent devenir des tests du compilateur.

**G3 — Implémentation complète** : lexer/parser → AST/THIR → typage/NIR → génération LLVM → messages d'erreur/positions source → formateur/Studio/VS Code. **Ne jamais laisser une grammaire acceptée qui n'est pas réellement compilable.**

**G4 — Transition** : choisir et documenter une stratégie explicite : `nxc migrate`, compatibilité `--edition` ou outil de migration avec diagnostics ; préserver les projets 0.2 jusqu'à la fin de la transition. Les anciennes constructions sont soit migrées fidèlement soit rejetées avec explication. Pas de suppression silencieuse.

**G5 — Stabilisation** : parser robuste sur grands projets, fichiers malformés, préfixes incomplets, erreurs multiples non en cascade, formateur déterministe et éditeur cohérent. Fuzzer le lexer/parser et conserver un corpus de régression.

**Exemples de syntaxe dans les études :** toutes les propositions sont **illustratives et non exécutables** tant que `nxc` ne les accepte pas. Ne jamais présenter une esquisse comme la syntaxe officielle.

---

# 4. NEXUL COMME LANGAGE FORTEMENT TYPÉ ET EXPLICITE

## 4.1 Garanties existantes à préserver

NEXUL 0.2 possède déjà des types statiques, de l'inférence, des conversions explicites, des `Option`/`Result`, des génériques, des traits, une loi d'exclusivité, l'ownership et RAII. Ne pas réinventer ces éléments ni annoncer qu'ils sont absents.

## 4.2 Renforcements demandés

- **Types distincts / newtypes** : ne pas mélanger implicitement des valeurs de même représentation ayant des sens différents (ID utilisateur / ID projet, secondes / millisecondes, degrés / radians).
- **Sous-types contraints** : plages et prédicats simples, constantes vérifiées à la compilation, vérifications d'entrées au runtime quand la preuve statique n'est pas possible.
- **Invariants de structures/objets** : construction et mutation préservant des conditions documentées ; visibilité et API empêchant le contournement sûr.
- **Contrats de fonctions** : préconditions, postconditions, invariants de boucle quand cela apporte une garantie pratique ; définir ce qui est statique, dynamique ou réservé aux profils.
- **Effets explicites** : réfléchir à un système déclarant allocation, blocage, I/O, panique, accès matériel, interruption, partage d'état, et exceptions de sécurité. L'effet `no_alloc` et `nonblocking` doit être réellement vérifié s'il existe.
- **Typage des ressources** : ownership, transfert, exclusivité, portée, destruction déterministe ; aucun alias mutable non contrôlé dans le code sûr.
- **Concurrence** : capacité d'un type à être transféré/partagé déterminée statiquement ; pas d'usage dangereux après déplacement ; protection des données partagées.
- **Numérique** : conversions vérifiées, débordements définis, unités/dimensions si retenues, gestion de l'overflow et du flottant explicitement documentée.
- **Exhaustivité** : filtrage par motifs total, enums typés, valeurs optionnelles explicites, erreurs non ignorées silencieusement si une politique de diagnostic l'exige.
- **Diagnostics** : erreurs de type pédagogiques avec source, chemin du raisonnement, proposition de correction et localisation précise dans Studio/VS Code.

**Limiter le coût de complexité :** un bon système de types rend le code plus sûr *et reste compréhensible*. Chaque nouveauté doit démontrer une valeur concrète et des messages d'erreur exploitables. Ne pas ajouter un type dépendant général ou un solveur complexe sans nécessité, architecture ou plan de validation.

## 4.3 Cas d'acceptation

- Uniquement les programmes sûrs et valides compilent sans `unsafe`.
- Mélanger deux unités distinctes par erreur échoue à la compilation (ou exige une conversion explicitement contrôlée).
- Une précondition violée est diagnostiquée statiquement lorsqu'elle est démontrable, sinon contrôlée conformément au contrat.
- Le compilateur refuse les usages invalides après transfert de propriété et les emprunts incompatibles.
- Il ne doit pas exister de « mode release » supprimant silencieusement les protections essentielles.
- Les programmes de référence 0.2 migrés doivent produire des résultats fonctionnellement équivalents.

---

# 5. ORIENTATION OBJET COMPLÈTE SANS IMITER UN AUTRE LANGAGE

L'ambition « OOP complet » doit être définie par les **capacités** suivantes et validée par des applications réelles :

- Types avec état privé et invariants ; visibilité module/type ; méthodes d'instance et de type.
- Interfaces/traits et implémentations contrôlées ; polymorphisme statique et dynamique (`dyn` ou construction propre à la nouvelle syntaxe).
- Composition, délégation, extensions et génériques ; dispatch explicite ou compréhensible ; relation claire entre objets, ressources et destruction.
- Constructeurs/usines, validation à la construction, gestion des erreurs ; cycle de vie sûr.
- Collections hétérogènes via interface, sans fuite ni double destruction.
- Abstraction et programmation par contrats, interfaces ouvertes à des implémentations futures dans le respect de la compatibilité ABI.
- Introspection/réflexion **limitée et explicite** si nécessaire ; éviter un runtime dynamique caché qui dégrade déterminisme et performance.
- Définir les frontières avec le temps réel : allocation implicite par polymorphisme dynamique, dispatch imprévisible et destructeurs complexes doivent être contrôlés ou refusés en profil RT.

**Point d'architecture :** l'héritage de classes multiple, la réflexion totale et un garbage collector ne sont pas des conditions nécessaires à un OOP solide. Ne les introduire que si les cas d'usage démontrent un avantage et que les invariants mémoire tiennent.

**Programmes d'acceptation OOP :** petite bibliothèque graphique typée, moteur de simulation, système d'événements, bibliothèque de pilotes, application serveur et hiérarchie de services — construits en NEXUL, documentés, testés en debug/release et sur les cibles pertinentes.

---

# 6. CONCURRENCE SÛRE ET MODÈLE MÉMOIRE

**Définir les garanties avant d'écrire les primitives.** La concurrence doit être cohérente avec les règles NEXUL de propriété, d'exclusivité et de destruction déterministe.

### 6.1 Capacités minimales de la version majeure

1. Threads système et/ou tâches NEXUL avec cycle de vie clair, lancement, jonction, arrêt et propagation des erreurs.
2. Transfert sûr de valeurs entre threads et règles de partage des ressources.
3. Canaux bornés et non bornés **hors profil RT**, fermetures et annulation explicites ; files bornées privilégiées en RT.
4. Synchronisation : mutex/RW locks si retenus, variables de condition, événements/sémaphores, contention, gestion du poison/erreurs selon la politique du langage.
5. Atomiques : opérations, ordres mémoire définis, barrières, cibles réellement prises en charge et interdiction des usages invalides.
6. Exclusion statique des **data races dans le code sûr** avec un modèle de transfert/partage des types et des `unsafe` confinés.
7. Synchronisation/ordonnancement cohérents avec les interruptions et le profil RT.
8. Erreurs de cancellation, ressources abandonnées et destruction des objets déterministes et documentées.
9. Comportement par cible : natif Windows/Linux/macOS, embarqué et Wasm peuvent proposer des capacités différentes, documentées explicitement.

### 6.2 Validations obligatoires

- Tests de compilation négatifs : mutable partagé sans synchronisation, invalidité après move, partage non transférable, référence pendante, mutation concurrente interdite.
- Tests de stress : création/destruction répétées, contention, starvation détectable, fermeture de canal, annulation, timeouts, ressources libérées, panics contrôlées.
- Tests avec détecteurs ou instrumentation adaptés : ne pas confondre un test stress réussi avec une preuve universelle d'absence de race.
- Vérification des primitives runtime et des ordres mémoire sur plusieurs architectures où l'exécution est possible.

**Garanties limitées :** un système de types peut prévenir les courses de données en code sûr, mais ne garantit pas automatiquement l'absence de deadlock, de priorité inversée, de famine, de bug logique ou de dépassement de deadline.

---

# 7. PROGRAMMATION TEMPS RÉEL : UNE VRAIE CAPACITÉ DU LANGAGE

Cette refonte est un **chantier majeur du noyau**, pas un simple ajout de `sleep()` et de threads. S'inspirer de la rigueur d'Ada et des profils temps réel/hautement fiables, **sans copier la syntaxe d'Ada** et sans prétendre à sa certification.

## 7.1 Définition des classes de temps réel

- **Soft real-time** : les dépassements de délais dégradent la qualité du service mais ne rendent pas nécessairement l'application incorrecte.
- **Firm real-time** : un résultat produit après sa deadline n'a plus de valeur fonctionnelle.
- **Hard real-time** : le dépassement est inacceptable dans le système considéré ; exige une conception matériel/OS/ordonnanceur, des bornes de ressources et une vérification appropriée.

NEXUL doit fournir une **programmation temporelle explicite et vérifiable**, mais la mention « hard RT assuré » n'est permise que pour une **cible et un environnement mesurés/analysés**. Windows desktop, navigateur et environnement générique ne fournissent pas à eux seuls une garantie hard RT.

## 7.2 Profil RT contrôlé

Définir un profil NEXUL temps réel (`rt` ou autre nom proposé par les deux Opus, non figé avant ADR) avec :

- Allocation dynamique interdite ou soumise à règles strictes après l'initialisation, pools bornés et budgets d'objets.
- Interdiction des boucles, récursions et attentes potentiellement non bornées dans les portions où une preuve de borne est requise ; possibilité de budgets statiques/annotations vérifiées.
- Destruction déterministe et absence de GC/pauses imprévisibles.
- Fonctions et effets classés : ISR-safe, nonbloquant, no-alloc, déterministe, accès matériel et I/O ; erreurs claires à la compilation.
- Tâches périodiques, événementielles, priorités, délais/deadlines, horloges monotones, alarmes, files bornées, événements, watchdog.
- Coordination avec un ordonnanceur temps réel ou un environnement bare metal selon le profil ; contrat précis entre langage/runtime/OS.
- Protocoles ou stratégies contre l'inversion de priorité si locks et priorités sont fournis.
- Contrôle des accès `volatile`, atomiques et barrières pour registres/interruptions, zones critiques et interactions ISR/tâches.
- Budget pile, heap, nombre de tâches, temps de réaction, comportements en surcharge et procédures de repli.
- Profilage et instrumentation : jitter, latence, deadline manquée, temps max observé, pile maximale et allocation. **Une durée maximale observée n'est pas une preuve de WCET** ; une telle preuve exige une méthodologie formelle adaptée à la plateforme.

## 7.3 Choix de plateforme et livrable RT

G0 : étude comparée d'une première cible de démonstration (ex. Cortex-M sans OS, Cortex-M + RTOS approprié ou Linux PREEMPT_RT), avec contraintes de programmation et possibilité d'instrumentation. **Claude Main propose le choix et l'utilisateur valide avant revendication d'une garantie spécifique.**

G1 : API et profil RT spécifiés, exemples compilés ; tests de refus d'allocations/blocages/interruptions dangereuses.

G2 : runtime et modèle d'ordonnancement sur cible choisie, banc de test reproductible, rapport mémoire et timing.

G3 : applications témoins (contrôle périodique, capteur/actuateur, communication entre tâches, ISR contrôlée) et tests de surcharge.

G4 : documentation énonçant **ce qui est prouvé**, **ce qui est seulement mesuré**, et **ce qui dépend de l'OS ou matériel**. Le profil RT peut être livré graduellement, mais la fonctionnalité obligatoire annoncée pour la version finale doit être réellement opérationnelle sur au moins **une cible définie** et validée.

NEXUL doit permettre au développeur d'utiliser le temps réel sans devoir déléguer toute sa logique à du C ou du Rust. Des ABI explicites vers l'OS/RTOS restent autorisées.

---

# 8. PERFORMANCES, PRÉCISION ET EFFICACITÉ

**Objectif :** performance native compétitive et exactitude mathématique démontrée, sans compromettre sûreté ou visibilité des effets.

- Construire des références NEXUL/C/Rust/C# équivalentes sur la même machine : boucle 1M, 10M et 100M, fonctions mathématiques, vecteurs/matrices, allocations, chaînes, tâches et synchronisation, IO séparées des calculs.
- Compiler en modes optimisés documentés et mesurer temps de compilation, exécution, mémoire, variabilité et précision ; éviter dead-code elimination/constant folding qui annulent les travaux. Rapport de médianes, dispersion et mesures brutes.
- Tester et expliquer le comportement de `f32`/`f64`, NaN, infinis, zéro signé, subnormaux, arrondis, erreur relative/absolue, cas limites, calculs conditionnellement mal conditionnés.
- Distinguer un **mode strict** des optimisations numériques **rapides optionnelles** et explicites ; pas de `fast-math` silencieux ni suppression invisible de contrôle de débordement.
- Profiler l'IR/assembleur LLVM, vérifier inlining, SIMD/vectorisation, optimisation des boucles, runtime math, élimination prouvée des vérifications redondantes, LTO/PGO si rentables ; mesurer avant/après.
- Préciser l'impact des nouvelles abstractions OOP, contrats, typage, concurrence et RT sur les coûts. Déclarer les compromis.
- Conserver des garde-fous automatiques contre la régression des performances et de la correction.

Ne pas remplacer secrètement les calculs NEXUL par des implémentations étrangères uniquement pour embellir un benchmark. Un appel système ou FFI explicitement déclaré est différent d'une transpilation cachée.

---

# 9. SELF-HOSTING — UN COMPILATEUR NEXUL ÉCRIT EN NEXUL

**Cet objectif fait partie des livrables majeurs obligatoires du projet**, mais il doit être atteint **sans casser le compilateur existant**.

## 9.1 Définition concrète

Un compilateur *self-hosted* de NEXUL possède son cœur écrit en NEXUL et peut compiler sa propre source. Le compilateur Rust historique peut rester comme **bootstrap initial (stage 0)** pendant la transition, et LLVM demeure un backend autorisé. Ceci n'en fait pas un « faux langage ». À terme, un développeur doit pouvoir reconstruire le compilateur NEXUL à partir d'une chaîne d'amorçage documentée.

**Ne pas confondre :** `nxc` qui compile un programme NEXUL, `nxc` écrit en NEXUL, et la propriété **de self-compilation** qui requiert de compiler lui-même sa propre source : ce sont trois étapes différentes.

## 9.2 Plan d'auto-hébergement

- **SH0 — Dépendances :** identifier les capacités manquantes dans NEXUL pour implémenter lexer, parser, diagnostics, typage, fichiers, mémoire, structures de données, compilation, linking et appels LLVM. Définir les bindings système explicites nécessaires, sans transpilation cachée.
- **SH1 — Infrastructure :** développer en NEXUL les bibliothèques d'outillage requises (tokenisation, collections, gestion de chaînes/UTF-8, diagnostics, API fichiers, utilitaires CLI) avec tests de conformité et budgets de performance.
- **SH2 — Premier compilateur NEXUL :** transférer progressivement les passes du compilateur dans du NEXUL en conservant une spécification de comportement partagée. Compiler ce nouveau compilateur avec le compilateur bootstrap écrit en Rust, puis comparer l'acceptation/refus des mêmes programmes.
- **SH3 — Chaîne d'amorçage :** `stage0` (nxc historique) compile les sources du compilateur NEXUL → binaire `stage1`; `stage1` recompile ces sources → binaire `stage2`; `stage2` recompile de nouveau si nécessaire pour stabilité. Exécuter la même suite de conformité et comparaisons. Documenter différences non sémantiques d'artefacts (timestamps, chemins, sections debug), options, versions LLVM.
- **SH4 — Reproductibilité :** vérifier l'équivalence comportementale des compilateurs et, lorsque possible, la reproductibilité binaire après normalisation appropriée ; mesurer performance, diagnostiques, sécurité et stabilité de `stage1/stage2`.
- **SH5 — Transition :** `nxc` auto-hébergé devient la chaîne recommandée **uniquement après** la réussite de la suite complète, compatibilité de CLI, sécurité, plateformes cibles annoncées et stratégie de rollback. Conserver une procédure bootstrap documentée.

### Tests obligatoires du self-hosting

- Compiler un jeu substantiel de programmes NEXUL avec stage0/stage1/stage2 ; comparer exécution et diagnostics essentiels.
- Faire passer les tests lexer, parser, sémantique, modèle mémoire, OOP, concurrence, RT, WebAssembly, erreurs et bibliothèques.
- Refuser correctement les programmes mal formés et dangereux, sans crash ni corruption.
- Reconstruire à froid sur machine vierge documentée ; dépendances identifiées et versions figées.
- Démontrer un **vrai cycle stage1 → stage2** : présenter seulement un `nxc` qui accepte des `.nx` n'est pas suffisant.

Le self-hosting est techniquement un **grand projet** ; ne pas annoncer son achèvement avant la boucle d'amorçage réellement démontrée. Il peut être le dernier jalon de la version majeure plutôt que de bloquer immédiatement la sûreté ou le Studio.

---

# 10. EXTENSION OFFICIELLE VISUAL STUDIO CODE POUR NEXUL

**Livrable obligatoire indépendant de NEXUL Studio** : une extension installable pour VS Code permettant de créer, éditer, compiler, lancer, tester, formater et déboguer des programmes NEXUL dans les limites des capacités disponibles.

## 10.1 Identité et architecture

- Développer l'extension dans un emplacement réservé, proposé : `editors/vscode/**`, avec son propre `package.json`, extension entrypoint et tests.
- Nom de travail : **NEXUL Language Support** ; extension `.nx` et reconnaissance des projets `nexul.toml`.
- L'extension peut être écrite en TypeScript/JavaScript **uniquement comme plugin de l'hôte VS Code**. **Le programme NEXUL n'est jamais transpillé en JavaScript** : compiler via `nxc` et exécuter le binaire/wasm réel.
- Réutiliser `nxc` CLI, `nxc ide` pendant la transition puis le serveur **`nxc lsp` officiel** lorsqu'il est réellement disponible. Ne pas écrire un compilateur NEXUL bis dans l'extension.
- Localiser `nxc` installé ou configuré ; chemins personnalisés, sélection de projet/SDK, message clair si absent. Aucun téléchargement ou exécution arbitraire silencieuse ; respecter Workspace Trust et les options de sécurité de VS Code.

## 10.2 Fonctions indispensables de la première version complète

1. Détection `.nx`, icônes de fichier, association de langage et thèmes de coloration.
2. Grammaire de coloration TextMate ou équivalent alignée sur la **nouvelle syntaxe officielle**, avec tests de tokens, chaînes, interpolation, attributs, commentaires et erreurs.
3. Snippets, indentation, appariement, commentaires, plissage et conventions `nxc fmt`.
4. Diagnostics et squiggles depuis `nxc check` ou `nxc lsp` ; numéros de ligne et colonnes corrects, gestion Unicode, respect du texte non sauvegardé quand l'API le permet.
5. Complétion, hover, navigation définition/références, renommage, symboles de document/workspace, formatage et refactorings **quand fournis** par le langage serveur. Annoncer honnêtement les commandes non disponibles.
6. Commandes : nouveau projet, build debug/release, run, test, check, format, clean si CLI existante, consulter diagnostics `nxc explain`.
7. Tâches VS Code (`tasks.json`) pour compiler et tester, configurations lancement/débogage (`launch.json`) via LLDB/DAP ou extension de débogage compatible ; ne pas simuler un vrai debugger en bricolant des `println`.
8. Gestion des projets `nxpm`, choix des cibles (`nxc targets`) et lancement des builds WebAssembly.
9. Documentation install/configuration/raccourcis, dépannage Windows/Linux/macOS, compatibilité versions `nxc`, licences et changelog.
10. Package **`.vsix`** installable et reproductible, tests automatisés extension host, smoke tests réels avec `nxc`, possibilité de publication Marketplace **avec décision utilisateur**.

## 10.3 Suivi de la nouvelle syntaxe

Luna implémente l'extension. Claude Outils définit la grammaire et les capacités LSP. Claude Main en garantit l'expérience et valide le comportement global. Pour éviter le double travail : commencer l'ossature VS Code, les commandes CLI et les tests indépendants immédiatement, mais **ne finaliser ni la grammaire ni les snippets tant que l'ADR syntaxe majeure n'est pas approuvée**. Synchroniser avec `nxc lsp` et `nxc fmt` plutôt que maintenir trois interprétations concurrentes de NEXUL.

## 10.4 Définition de terminé pour l'extension

L'extension est installée à partir du VSIX dans une instance propre de VS Code. Un développeur ouvre un projet NEXUL, voit la coloration, obtient de vrais diagnostics, compile, exécute, lance les tests, formate et utilise la navigation LSP **si le serveur le prend en charge** ; la fonction de débogage est testée réellement avec l'adaptateur sélectionné. Les cas d'outil absent sont correctement expliqués. Les versions de langage supportées sont déclarées.

---

# 11. NEXUL STUDIO NATIF — FINITION ET RACCORDEMENTS

**Responsable de produit et auteur de l'interface : Claude Main.** Le langage et le Studio sont deux chantiers distincts : ne pas bloquer les correctifs UX sur la refonte de syntaxe, mais anticiper la migration du lexique et des diagnostics.

### P0 — Protection des données des utilisateurs

- Boîte de dialogue **Enregistrer / Ne pas enregistrer / Annuler** lors de fermeture d'onglet et de fenêtre en présence de modifications, avec politique d'autosave configurable et explicite.
- Ne jamais écraser un fichier modifié sur disque sans conflit/résolution ; sauvegarde atomique et avertissement en cas d'échec.
- Restaurer les onglets et données non sauvegardées après crash quand possible ; tests de scénario coupure/disque plein/permissions/conflit de noms.
- Corriger définitivement nouveau fichier/nouveau dossier, renommer, déplacement, Corbeille, glisser-déposer, chemins longs, encodages, UNC et symlinks selon politique de sécurité.

### P1 — Raccorder les cinq crates existantes au Studio GPUI

1. **`studio/syntax`** : coloration incrémentale qui coexiste avec diagnostics NEXUL ; adaptation de la nouvelle grammaire sous contrat Claude Outils.
2. **`studio/lsp`** : vrai cycle LSP multi-langage, détection SDK, diagnostics/hover/complétion/définitions/renommage, arrêt propre, présence/absence de serveurs ; `nxc ide` en attendant `nxc lsp`.
3. **`studio/terminal`** : rendre et piloter le PTY, clavier interactif, scrollback, copies, shells et plusieurs terminaux ; sortie de build/test distincte de console interactive.
4. **`studio/dap`** : interface points d'arrêt, pas à pas, variables, pile, requêtes, vrai test LLDB NEXUL et diagnostics de connectivité.
5. **`studio/templates`** : assistant Nouveau projet et SDK ; modèle NEXUL+Web de Luna lorsque prêt, créable et exécutable.

### P1 — Ergonomie professionnelle et installation

- Fenêtre principale vraiment native, éditeur au centre, menus sobres, panneaux masquables/déplaçables, thème clair/sombre, mode minimaliste, raccourcis personnalisables, palette de commandes, recherche globale, paramètres clairs.
- Prioriser fluidité clavier, Unicode, performances sur gros fichiers, gestion des sessions, accessibilité et limitations textuelles connues.
- Projets multi-langages : édition et LSP grâce aux outils **externes détectés**, pas de compilos factices ; terminal et profils exécution par type de projet.
- Web : aperçus isolés qui peuvent utiliser un moteur de navigateur **dans leur panneau seulement** ; `nxc targets` + `check --json` + `build --target wasm32-web --lib` selon `docs/refonte/interop-web.md`, erreurs et rechargement propre.
- Ne retirer l'ancienne interface (`studio/ui`) que lorsque les fonctions importantes sont disponibles et testées dans la nouvelle, avec migration des préférences.
- Claude Main construit le binaire natif sur Windows/MSVC ; **Codex Luna** empaquette le bon binaire dans l'installateur *après transfert de fichiers approuvé*, assure raccourcis, association `.nx`, désinstallation et migration. Ne pas laisser l'ancien Edge devenir le Studio installé par défaut.

### P2 — Avancé après stabilité

- Centre NEXUL optionnel : benchmarks/profilage, inspection IR/assembleur, précision numérique et outils de simulation temps réel, panneau concurrence et diagnostics du nouveau typage ; ne pas imposer ces panneaux au quotidien.
- Plugins/extensions, projets multiples et dispositions personnalisées, collaboration Git, documentation intégrée, observation des threads et des deadlines RT lorsque les outils de runtime le permettent.

---

# 12. TRAVAIL DÉTAILLÉ DES CINQ AGENTS, SANS CHEVAUCHEMENT

## 12.1 CLAUDE OPUS 5.5 — MAIN (ancien « Claude Studio »)

**Rôle prioritaire et autorité : directeur technique/architecte/intégrateur.** Il ne devient pas simple chef administratif ; c'est **l'acteur principal du projet**, le détenteur du plan de livraison et le réviseur obligatoire des changements de langage.

Mandats exclusifs :

1. Mettre à jour la charte de coordination `docs/refonte/AGENTS.md` **après validation humaine** et faire signer les transferts de répertoires ; centraliser l'état des lots dans `docs/refonte/master-plan.md`.
2. Diriger les discussions de **nouvelle syntaxe**, comparer les propositions, arrêter le style cible avec l'utilisateur et co-approuver la grammaire avec Claude Outils.
3. Co-diriger les architectures de typage, OOP, concurrence sûre, mémoire, RT et self-hosting ; exercer un **veto sécurité/compatibilité** sur les PR Core.
4. Réaliser personnellement les correctifs P0 du Studio et raccorder syntax/LSP/terminal/DAP/templates, puis WebAssembly et les nouveaux diagnostics du langage.
5. Arbitrer les conflits de dépendances, imports, branches et format de CLI ; approuver le contrat `nxc lsp` destiné à Studio et VS Code.
6. Réviser l'extension VS Code de Luna, ses tests de conformité et son UX, sans écrire en parallèle le code de Luna.
7. Organiser les intégrations régulières, la vérification Windows et le suivi des limitations Linux/macOS/embarqué.
8. Tenir l'acceptation finale : toute fonctionnalité obligatoire doit avoir une preuve de fin. Si la refonte n'est pas complètement validée, ne pas dire « terminé ».

**Claude Main ne code PAS simultanément les passes critiques du compilateur modifiées par Claude Outils**. Il peut ouvrir des prototypes syntaxiques ou ADR sur une branche documentaire séparée avec un seul propriétaire et obtenir ensuite un transfert contrôlé.

## 12.2 CLAUDE OPUS 5.5 — OUTILS / CORE

L'ancien Claude Outils entretenait la coloration, LSP et l'installateur ; les briques sont en grande partie déjà produites et proposées au transfert à Luna. **Sa mission prioritaire change complètement : développer le nouveau cœur de NEXUL sous supervision étroite de Main.**

Mandats :

1. Auditer et spécifier la nouvelle grammaire, écrire le lexer/parser, AST, typage, NIR et codegen qui la supportent jusqu'au bout.
2. Renforcer le système de types (distinct, sous-types, invariants, effets et contrats) et préserver les sûretés existantes.
3. Compléter OOP/traits/polymorphisme/encapsulation par des scénarios vérifiables.
4. Construire le modèle de concurrence sûre, le runtime threads/tâches/canaux/atomiques et les preuves/tests positifs/négatifs nécessaires.
5. Concevoir et implémenter le profil RT et les primitives déterministes sur cible validée, avec rapports de timing/budget et documentation des limites.
6. Développer `nxc lsp` et sorties diagnostics versionnées avec accès au texte non enregistré ; préserver la CLI du Studio et du VS Code plugin.
7. Réaliser le **self-hosting** à étapes stage0/1/2 avec soutien architectural de Main : développer un compilateur NEXUL en NEXUL, qualifier et basculer quand les preuves sont suffisantes.
8. Maintenir WebAssembly navigateur/WASI, backend LLVM, exactitude numérique et cibles sans régression.
9. Définir les éditions et migration `nxc migrate`/`--edition` ou équivalent, tenir la spécification normative, corpus positif/négatif et exemple final.

Chaque ajout à `compiler/**` ou au runtime doit être examiné par Main avant intégration. Sur les sous-systèmes dangereux (`unsafe`, atomiques, FFI, ISR, ordonnancement), ajouter revue spécialisée et tests sur matériel/cible lorsqu'ils sont requis.

## 12.3 CODEX ASTRAL — SOUTIEN TECHNIQUE LÉGER ET PONCTUEL

- Vérifications LLVM, ABI, IR, diagnostics techniques, atomiques et interop sur demande cadrée.
- Benchmarks comparatifs isolés et études performances/numérique, analyses de résultats, régressions ; ne pas inventer temps ou sorties.
- Aide ponctuelle à une décision difficile de backend sans devenir l'auteur permanent de la sémantique, du modèle mémoire, du temps réel ou de la nouvelle grammaire.
- Par défaut lecture seule du noyau ; patch possible **uniquement avec périmètre écrit, branche dédiée et revue des deux Opus**.
- Consigner résultats avec matériel, toolchain et conditions ; n'exécuter aucun changement silencieux.

## 12.4 CODEX LUNA — INTEGRATIONS ET EXTENSION VS CODE

- Continuer `studio/terminal`, `studio/dap`, `studio/templates` : améliorer tests Windows ConPTY, vrais LLDB/NEXUL, templates et robustesse ; livrer APIs à Main.
- Produire modèle NEXUL + WebAssembly + JavaScript à partir de l'ABI v1 existante, sans dupliquer le chargeur.
- Après transfert, gérer `installer/**` et scripts de packaging du Studio natif, compatibilité Windows, association `.nx`, tests installation/désinstallation et évolutions hors plateforme documentées.
- Assurer maintenance contrôlée de `studio/lsp` et `studio/syntax` après transfert des anciens dossiers de Claude Outils.
- **Nouvelle mission importante : extension VS Code officielle** dans `editors/vscode/**` : grammaire, snippets, tâches nxc, intégration nxc lsp/ide, formateur, projets nxpm, VSIX, tests hôte, documentation. Ne pas inventer de syntaxe avant approbation Main/Outils.
- N'écrit pas les compilateurs, les contrats mémoire/concurrence/RT, ni l'interface native GPUI.

## 12.5 GEMMA 4:31B (CLAUDE CODE) — QUALITÉ, DOCS, CHECKLISTS

- Tenir un tableau de statut honnête : `existant / à raccorder / testé unitairement / testé E2E / certifié ou non / limitations / preuve / propriétaire`.
- Créer docs utilisateur, guide d'installation VS Code, préparation de snippets et exemples **en proposition**, guides de syntaxe une fois validée, tableaux comparatifs et checklists manuelles de Studio.
- Préparer des tests black-box de programmes NEXUL non critiques dans un dossier réservé ; remonter les plantages et les incohérences de messages d'erreur à Claude Main/Core.
- Générer un index des nouvelles fonctionnalités de langage et des cibles supportées, vérifié par les propriétaires ; ne pas annoncer de fonctionnalité « complète » si elle n'a pas de test bout en bout.
- Résumer les PR et maintenir le contexte de passation ; activités courtes et réversibles, faciles à relire.
- **Interdictions :** aucune modification de `compiler/**`, `lib/**`, `studio/app/**`, `installer/**`, `editors/vscode/**`, noyau sémantique, `unsafe` ou mémoire partagée sans une dérogation très précise. Pas de décision autonome sur la sécurité, le temps réel ou la grammaire.

---

# 13. FEUILLE DE ROUTE ET DÉPENDANCES — PRIORITÉ À L'ACHÈVEMENT

**Les projets Studio et Core avancent parallèlement, mais pas les mêmes fichiers.** Chaque jalon possède un propriétaire unique de code, des tests et une PR.

| Jalon | Urgence | Travail et preuve de sortie | Auteur principal | Dépendances |
|---|---|---|---|---|
| P0 | **Immédiat** | Audit des branches et transferts ; `AGENTS.md` révisé, propriétaire par dossier, documents de contexte classés | Claude Main | Accord utilisateur |
| P1 | **Immédiat** | Pas de perte de données Studio, dialogue de fermeture, récupération et tests E2E | Claude Main | P0 pour fichiers transférés seulement |
| P2 | **Immédiat** | Spécification nouvelle syntaxe : trois propositions et ADR validée ; plan migration 0.2 | Claude Main + Outils (un auteur par fichier) | Décision architecture utilisateur |
| P3 | **Élevée** | Raccordement syntax/LSP/terminal/DAP/templates dans GPUI ; UI native stable | Claude Main | Modules déjà fusionnés |
| P4 | **Élevée** | Installer natif Windows testé avec un vrai binaire installé | Codex Luna | Transfert, build Studio P3 minimal |
| P5 | **Élevée** | Conception typage/OOP et modèle mémoire concurrence, tests d'acceptation et compatibilité | Claude Outils, revue Main | P2 |
| P6 | **Élevée** | Nouveau lexer/parser + règles de types/formatage + diagnostics ; migration source | Claude Outils | ADR P2, tests existants gelés |
| P7 | **Élevée** | Concurrence sûre compilateur/runtime, canaux/tests négatifs et stress | Claude Outils | P5/P6 |
| P8 | **Élevée** | Profil RT v1 sur cible définie avec bornes d'effets/budgets et tests timing | Claude Outils | P7, validation cible |
| P9 | **Élevée** | `nxc lsp`, JSON versionné, tampon non enregistré, IDE support | Claude Outils | Interfaces nxc négociées |
| P10 | **Élevée** | Extension VS Code installable VSIX, actions nxc, LSP, format, tests | Codex Luna, revue Main | P2 pour coloration finale ; P9 pour LSP avancé |
| P11 | **Élevée** | Studio web NEXUL : template + preview isolée avec wasm32-web | Luna template, Main UI | ABI v1 existante |
| P12 | **Majeur** | Self-hosting, stage0/1/2, parité et reproductibilité documentée | Claude Outils, revue Main | Noyau assez stable + APIs outillage |
| P13 | **Majeur** | Benchmarks, précision, optimisation et contrôles cross-target | Outils ; Astral mandat limité | Programmes représentatifs et base stable |
| P14 | **Bloquant pour « fini »** | Batteries E2E NEXUL/Studio/VS Code ; docs exactes ; release stable, migration et rollback | Claude Main et chaque propriétaire | Tous les jalons obligatoires |

**Séquençage important :** ne pas réécrire immédiatement tout le compilateur pour obtenir la nouvelle syntaxe, la concurrence et le self-hosting d'un seul coup. La suite de tests doit rester verte jalon après jalon. Les changements de syntaxe doivent être planifiés avant le plugin final et les garanties RT doivent être spécifiées avant l'implémentation des primitives.

### Actions possibles en parallèle dès maintenant

- Main corrige les pertes de données Studio et branche les crates existantes.
- Outils prépare l'ADR de syntaxe, le modèle de types et la conception de concurrence/RT **sans modifier simultanément les mêmes fichiers**.
- Luna prépare la structure de l'extension VS Code, les scripts CLI, le packaging et les tests de base, et finalise l'installateur sur transfert.
- Astral mesure le compilateur existant sur une base figée **sur mandat écrit**.
- Gemma recense les écarts code/docs, prépare les scénarios de test et tient la matrice des preuves.

---

# 14. MATRICE DE LIVRAISON FINALE — TOUS LES BOUTS DE LA CHAÎNE

Une fonctionnalité doit passer par **spécification → implémentation → tests unitaires et négatifs → test réel bout en bout → documentation → validation du responsable** avant d'être déclarée livrée.

| Exigence non négociable | Preuve attendue pour la version majeure |
|---|---|
| Indépendance de NEXUL | Un programme NEXUL réel passe par le front-end NEXUL → NIR → LLVM → binaire/Wasm sans conversion en un autre langage source |
| Syntaxe propre | EBNF officielle, style guide, corpus et diagnostics ; au moins trois candidats évalués, syntaxe retenue validée ; exemple d'application cohérent |
| Typage fort | Tests positifs/négatifs pour conversions, plages, types distincts, invariants et effet(s) retenus ; protections présentes aussi en release |
| OOP complet | Bibliothèque OOP non triviale construite uniquement en NEXUL, polymorphisme sûr et prédictible, encapsulation et génériques opérationnels |
| Concurrence sûre | Pas de data race admise dans le code sûr pour le modèle annoncé ; tests statiques de refus, tests de stress/arrêt/transfert/atomiques, limites documentées |
| Temps réel | Profil RT compilable et runtime réel sur cible choisie, tests d'effets/budgets, instrumentations deadlines/jitter/stack et rapport des garanties exactes |
| Précision | Comparaisons mathématiques documentées, cas limites NaN/infini/sous-normal/overflow, tolérances appropriées, modes strict/rapide clairement séparés |
| Performances | Benchmarks NEXUL/C/Rust/C# comparables et reproductibles, régressions suivies ; aucune promesse de dépasser tous les concurrents |
| Self-hosting | stage0 produit stage1 ; stage1 produit stage2 ; stage1/stage2 valident le corpus ; bootstrap documenté et faisable à froid |
| Web/WASM | Exemple NEXUL compilé en Wasm navigateur, JS hôte explicite, UTF-8, erreurs contrôlées, aperçu Studio et template fonctionnels |
| NEXUL Studio | Exécutable natif, installation Windows, F5/build/test, explorateur et fichiers sûrs, terminal/DAP/LSP/templates actifs, fermeture sans perte de données |
| Extension VS Code | VSIX installable, `.nx` reconnu, coloriage, diagnostics, build/run/test/format, LSP et DAP dans la limite documentée, tests réels et docs |
| CI et compatibilité | CI multi-composants (y compris workspaces séparés), matrice plateformes, migration depuis 0.2, tests E2E et artefacts de release |
| Documentation | Guide du langage et spec correspondant exactement au compilateur ; références pour Studio, extension et déploiement |

### Niveaux de statut obligatoires

- `CONÇU` : ADR/spécification seulement.
- `IMPLÉMENTÉ` : code présent mais tests ou intégration incomplets.
- `TESTÉ_UNITAIREMENT` : tests locaux passés, pas forcément utilisable dans le produit.
- `INTÉGRÉ` : raccordé au compilateur/IDE ou outil correspondant.
- `VALIDÉ_E2E` : scénario utilisateur complet exécuté avec résultat vérifié.
- `LIVRÉ` : version distribuée, installée et documentée, limitations annoncées.
- `BLOQUÉ` : information/manque technique empêchant la preuve.

**Jamais « terminé » sur la seule base d'une capture, d'un extrait de code ou d'un test qui a été ignoré.**

---

# 15. PROCÉDURES D'ACCEPTATION PRATIQUES

## 15.1 Tests de conformité et commandes existantes

Depuis la racine du dépôt, sur environnement configuré :

```powershell
cargo test --release --workspace
cargo test -p nexul-studio
cargo +stable-x86_64-pc-windows-msvc test --manifest-path studio/app/Cargo.toml -- --test-threads=1
cargo +stable-x86_64-pc-windows-msvc build --release --manifest-path studio/app/Cargo.toml
cargo test --manifest-path studio/syntax/Cargo.toml
cargo test --manifest-path studio/lsp/Cargo.toml
cargo test --manifest-path studio/terminal/Cargo.toml
cargo test --manifest-path studio/dap/Cargo.toml
cargo test --manifest-path studio/templates/Cargo.toml
```

Ce sont des **commandes proposées à exécuter**, non la preuve qu'elles réussissent maintenant. Certaines exigent Windows MSVC, LLVM, LLD, Windows SDK, LSP réels, LLDB ou une cible particulière. Ajouter ultérieurement les commandes **réelles** des tests de concurrence/RT/self-hosting/extension VS Code. `studio/app` et les crates périphériques étant dans des workspaces Cargo séparés, ne pas croire que `cargo test --workspace` racine suffit à les valider.

## 15.2 Tests de fin de projet (scénarios utilisateurs)

**Scénario A — Premier programme :** installer NEXUL ; ouvrir Studio ; créer projet ; saisir nouveau NEXUL ; erreurs utiles ; compiler et exécuter ; interrompre ; debug ; fermer/réouvrir sans perte.

**B — Bibliothèque OOP forte :** types avec invariants, génériques et interfaces ; compilation debug/release ; erreurs refusées et tests de non-fuite.

**C — Concurrence :** lancer tâches productrices/consommatrices ; communication canal borné ; annuler sous contention ; vérifier ressources ; refuser par compilation un partage mutable incorrect.

**D — Temps réel :** compiler en profil RT pour cible explicitée ; exécuter des tâches périodiques sur environnement test ; mesurer deadlines/jitter/stack ; démontrer refus des opérations interdites et décrire garanties/limitations.

**E — Self-host :** compiler `nxc` en NEXUL avec bootstrap, puis avec lui-même ; lancer suite complète avec stage1 et stage2 ; documenter sorties et différences.

**F — VS Code :** installer VSIX propre ; ouvrir `.nx` ; coloration et diagnostic ; auto-complétion/hover si LSP disponible ; format, build, test, run, débogage, erreurs d'outil absent.

**G — Web :** NEXUL → `wasm32-web` → module chargé dans page HTML/JS et aperçu Studio ; textes Unicode et erreurs/tentatives de réentrance gérés.

**H — Régression :** programmes 0.2 migrés, matrice de cibles, CPU/mémoire et précision surveillés, CI complète et artefact installable.

## 15.3 Politique de sortie de version

- Ne pas annoncer une version « complète » tant que syntaxe et sémantique sont instables ou que le compilateur ne s'auto-compile pas si cet objectif est dans le périmètre obligatoire.
- Une sortie intermédiaire `alpha`/`preview` peut être publiée **avec son statut exact**, sans la présenter comme une finalisation.
- Pour une version majeure stable, lister clairement toutes les fonctionnalités obligatoires, plateformes garanties, tests réellement exécutés, limitations tolérées, vulnérabilités ouvertes et stratégie de rollback.
- Si la conformité du profil RT sur cible n'est pas prouvée, le profil ne doit pas être décrit comme « hard real-time garanti ».
- Si un livrable obligatoire (VSIX, auto-compilation, protection des données, compilation sûre) manque, **le projet n'atteint pas les critères de fin** demandés dans ce document.

---

# 16. RISQUES ET DÉCISIONS À FAIRE VALIDER PAR L'UTILISATEUR

Les audits et corrections P0 ne doivent pas attendre ces décisions, mais toute **rupture d'architecture ou revendication forte** requiert leur traitement.

1. **Édition du langage :** nouvelle syntaxe incompatible par défaut ou édition opt-in `edition 2` avec migrateur ? Recommandation initiale : version majeure + outil/compatibilité transitoire, pas casse silencieuse.
2. **Premier environnement RT :** Cortex-M bare metal, RTOS spécifique ou Linux PREEMPT_RT ? Recommandation : définir **une première cible** avant de promettre hard RT.
3. **Niveau de contrats :** sous-types/plages/effets intégrés partout ou profil strict optionnel ? Choix fondé sur ergonomie et régression.
4. **Concurrence par cible :** natif dans un premier temps, embarqué avec ordonnanceur plus tard, Wasm selon les capacités hôte.
5. **Self-hosting :** périmètre initial des passes du compilateur à écrire en NEXUL et stratégie de bootstrap conservant LLVM.
6. **Extension VS Code :** nom définitif, identifiant éditeur, publication Marketplace et propriétaire du compte ; **publication publique demande une action explicite du propriétaire**.
7. **Transfert ancien AGENTS :** mise à jour du tableau de propriété, notamment Core Astral → Claude Outils, ancien Claude Studio → Claude Main, installateur Outils → Luna, lsp/syntax → Luna.
8. **CI de confiance :** tests réels sur Windows, Linux et cibles embarquées selon accès matériels disponibles ; pas d'auto-certification.

---

# 17. CONSIGNE COPIABLE POUR RELANCER UNE NOUVELLE SESSION D'AGENT

> **TU TRAVAILLES SUR NEXUL.** Lis ce document intégralement avant tout code, puis lis la version actuelle de `docs/refonte/AGENTS.md`, les derniers rapports d'état et les PR GitHub. Confirme ton identité parmi : **Claude Opus 5.5 Main (ancien Studio)**, **Claude Opus 5.5 Outils/Core**, **Codex Astral**, **Codex Luna**, **Gemma 4:31B via Claude Code**. **Claude Main est le responsable technique principal et le réviseur obligatoire de la refonte du langage.** La nouvelle grammaire NEXUL, le typage renforcé, l'OOP complet, la concurrence sûre, le temps réel et l'auto-hébergement du compilateur sont des objectifs majeurs ; leur code critique revient à **Claude Outils**, co-conception/revue à **Claude Main**, assistance Astral uniquement sur demande. Luna détient les services Studio, la distribution et **l'extension VS Code** ; Gemma aide pour QA/docs. Ne modifie aucun fichier réattribué avant mise à jour validée de la charte. Un seul propriétaire par dossier, une branche et une PR par lot, résultats réellement vérifiés. Distingue écrit/raccordé/testé/livré. La finalisation doit produire un **langage NEXUL autonome, à syntaxe distinctive, lisible, fortement typé, OOP, concurrent, performant, précis, avec profil RT vérifié sur cible définie, compilateur auto-hébergé et plugin officiel VS Code**, ainsi qu'un NEXUL Studio natif entièrement fonctionnel. **Commence par l'audit courant et une feuille de route avec dépendances, puis exécute les correctifs de ton périmètre sans réécrire les composants existants.**

---

# 18. RÉFÉRENCES ET TRAÇABILITÉ

- Dépôt : https://github.com/KAYTAY10ok/NEXUL
- Charte historique : https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/refonte/AGENTS.md
- État Studio : https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/refonte/claude-status.md
- État Claude Outils : https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/refonte/claude-outils-status.md
- État Codex Core : https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/refonte/codex-status.md
- État Codex Exécution : https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/refonte/codex-exec-status.md
- Architecture GUI : https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/studio/architecture-native.md
- Contrat Web : https://github.com/KAYTAY10ok/NEXUL/blob/main/docs/refonte/interop-web.md
- PR #1 WebAssembly : https://github.com/KAYTAY10ok/NEXUL/pull/1
- PR #3 terminal : https://github.com/KAYTAY10ok/NEXUL/pull/3
- PR #4 DAP : https://github.com/KAYTAY10ok/NEXUL/pull/4
- PR #5 modèles : https://github.com/KAYTAY10ok/NEXUL/pull/5
- PR #6 syntaxe Studio : https://github.com/KAYTAY10ok/NEXUL/pull/6
- PR #7 Studio natif : https://github.com/KAYTAY10ok/NEXUL/pull/7
- PR #8 LSP : https://github.com/KAYTAY10ok/NEXUL/pull/8
- Issue #2 Web : https://github.com/KAYTAY10ok/NEXUL/issues/2

**Origine des décisions :** les objectifs et le changement de rôle de Claude Main viennent des demandes explicites de l'utilisateur ; les constats de code viennent du dépôt et des rapports. Les futures décisions sur RT, syntaxe finale, version majeure, extension Marketplace et transferts de propriété doivent être consignées, approuvées et vérifiées avant d'être considérées comme réalisées.

---

## DERNIÈRE INSTRUCTION — NE PAS RÉDUIRE L'AMBITION, NE PAS TRICHER SUR L'ACHÈVEMENT

**La priorité stratégique est maintenant la création d'un NEXUL véritablement fini dans sa version cible, possédant une syntaxe unique dans son esprit, un typage fort, une OOP solide, une concurrence sûre, des capacités temps réel vérifiées, une compilation efficace et précise, la possibilité de compiler son propre compilateur et des outils professionnels dans NEXUL Studio et Visual Studio Code.**

Réaliser ces ambitions par paliers mesurés, pas par promesses vagues. Ne pas faire passer un prototype, une cible non testée ou un test ignoré pour une livraison. **Claude Opus 5.5 MAIN est le pilote central et le principal responsable technique de l'ensemble du projet.**
