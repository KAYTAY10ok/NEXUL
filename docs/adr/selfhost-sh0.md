# Auto-hébergement — SH0 : inventaire et trajectoire

Lot P12, étape SH0 (tâche O8 du plan maître). Auteur : Claude (architecture).
Statut : **étude** (17/10/2026) — elle ne décide rien. Les choix marqués « proposé » seront
repris dans une ADR quand SH1 commencera. Base : `main` à `c40acbd`.

## 1. Objectif et critères de fin

L'auto-hébergement est **déclaré effectif** seulement quand les cinq critères suivants
sont démontrés et publiés avec leurs commandes :

| N° | Critère | Preuve |
|---|---|---|
| A1 | `stage1` = compilateur écrit en NEXUL, construit par `stage0` (le `nxc` Rust actuel) | journal de construction |
| A2 | `stage2` = le même source construit par `stage1` | journal |
| A3 | **Point fixe** : `stage2` et `stage1` produisent une IR LLVM identique à l'octet pour toute la suite `tests/run`, les exemples et le source du compilateur lui-même | comparaison automatique en CI |
| A4 | `stage2` passe toute la suite de conformité (UI, run, numériques) sur les cibles officielles | rapport de suite |
| A5 | Construction **sans Rust** à partir d'un binaire `stage0` publié, procédure documentée ; dépendances restantes listées (LLVM-C, LLD) | procédure rejouée sur une machine propre |

Tant que A3 n'est pas atteint, aucune communication ne parle d'auto-hébergement.

## 2. Ce que le compilateur Rust utilise, et ce que NEXUL offre

Le compilateur (`compiler/`, 29 946 lignes de Rust au 17/10) est déjà écrit dans un style
compatible avec le modèle de NEXUL : identifiants entiers (`FnId`, `AdtId`, `Local`),
tableaux et tables de hachage, presque aucun pointeur partagé (`Rc` seulement dans
l'interneur de types et les diagnostics). Ce style se transpose sans références stockées.

| Besoin du compilateur | NEXUL 0.2 | Écart | Priorité |
|---|---|---|---|
| Énumérations à données, filtrage exhaustif | oui | — | — |
| Arbres récursifs (AST, THIR) | `indirect` | destruction récursive : dépassement de pile possible sur des arbres très profonds (ADR 0011, C3) | moyenne |
| Tables de hachage, ensembles | `Map`, `Set` | performances non mesurées | moyenne |
| Interfaces et génériques | traits, monomorphisation | — | — |
| Chaînes UTF-8, construction de texte | `String` (`push_str`, `substring`…) | pas de **vue** de chaîne : `substring` copie ; coût pour le lexer | haute |
| Tampons d'octets | `Vec[u8]` | — | — |
| Fichiers | `io.read_file`, `write_file`, `read_bytes` | pas de liste de dossier, d'existence, de métadonnées, de chemins | haute |
| Lancer l'éditeur de liens (LLD), `nxc run` | — | **aucun lancement de processus** | haute |
| Appeler la bibliothèque LLVM-C | `extern "C"`, `@link` | chargement dynamique impossible (pas de conversion `Ptr` → `fn`) ; lien direct à `LLVM-C` possible (bibliothèque d'importation écrite par `nxc` sur Windows) | haute |
| Entiers 128 bits (évaluation des constantes, `ConstValue::Int(i128)`) | — | pas de `i128` ; `std.fmt.Big` existe | moyenne |
| Arguments, variables d'environnement, code de sortie | `std.system` | — | — |
| Tri, recherche | `Vec.sort`, `binary_search` | — | — |
| SHA-256 (`nxpm`) | — | à écrire en NEXUL ou garder `nxpm` en Rust | basse |
| Pile profonde (analyses récursives : le pilote Rust lance une pile de 256 Mio) | pile du système | pas de choix de taille de pile ni de thread | moyenne |

## 3. Ce qui reste en Rust (proposé)

| Composant | Décision proposée | Raison |
|---|---|---|
| NEXUL Studio (`studio/**`) | Rust, sans échéance | GPUI, écosystème graphique |
| Installateur (`installer/**`) | Rust | Win32, aucun bénéfice |
| `nxc lsp` / `nxc ide` | Rust jusqu'à A4, puis réévaluation | latence mesurée (ADR 0009), propriété de Codex |
| `nxpm` | Rust jusqu'à A4 | réseau, SHA-256 |
| Chargement de LLVM, LLD | dépendances externes conservées | ADR 0008 : LLVM est l'infrastructure permise |

## 4. Étapes proposées

| Étape | Contenu | Oracle de vérification | Dépend de |
|---|---|---|---|
| SH1 | Bibliothèque : processus, dossiers et chemins, vues de chaînes, `i128` ou équivalent, appel de LLVM-C par lien direct | tests `tests/run` par fonction | édition 1 figée (G3) |
| SH2 | **Lexer** en NEXUL ; outil `nxc-lex-nx` qui écrit les jetons en JSON | jetons **identiques** au lexer Rust sur tous les `.nx` du dépôt (différentiel) | SH1 |
| SH3 | Parser et AST ; sortie JSON de l'AST | AST identique au parser Rust (différentiel) | SH2 |
| SH4 | Analyse sémantique, THIR, diagnostics | diagnostics identiques sur `tests/ui` | SH3, ADR 0012 et 0013 tranchées |
| SH5 | NIR, propriété, génération de l'IR LLVM **textuelle** | IR identique à l'octet au générateur Rust | SH4 |
| SH6 | Pilote, édition de liens, point fixe A1–A5 | §1 | SH5 |

Pendant SH2 à SH5, les deux implémentations coexistent : le compilateur Rust reste la
référence, et chaque composant NEXUL est comparé au composant Rust correspondant sur tout
le corpus avant de le remplacer. L'IR textuelle (ADR 0008) rend la comparaison de SH5
exacte, ce qui est l'atout principal de cette trajectoire.

## 5. Risques d'une transition prématurée

- **Syntaxe mouvante** : l'édition 1 est en cours d'implémentation (G3) ; réécrire le
  compilateur avant qu'elle soit figée imposerait une double migration. SH2 ne commence
  pas avant le gel de la grammaire de l'édition 1.
- **Double maintenance** : chaque correction du compilateur Rust devra être reportée
  jusqu'à SH6 ; limiter la durée de coexistence par étape.
- **Performances** : aucune mesure de NEXUL sur un programme de la taille du compilateur ;
  mesurer SH2 (lexer) contre le lexer Rust avant d'aller plus loin (P13).
- **Garanties** : le compilateur NEXUL hérite des conditions de l'ADR 0011 (pile).

## 6. Reproductibilité

- La génération de code est déterministe à l'octet (spec §10.4) : à vérifier en CI pour
  `stage1` et `stage2` (deux constructions successives, mêmes octets).
- Version exacte de LLVM et de LLD consignée avec chaque `stage0` publié ; une IR
  identique peut donner des binaires différents d'une version de LLVM à l'autre.
- `stage0` publiés avec leur empreinte SHA-256.
