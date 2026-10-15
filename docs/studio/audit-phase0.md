# Audit de NEXUL Studio — phase 0 (octobre 2026)

Auteur : Claude (périmètre NEXUL Studio), branche `refonte/claude-studio`.
Point de départ : `main` à `d537a7b` (NEXUL 0.2.0).

Ce rapport décrit l'état **réellement constaté** de Studio : chaque problème listé a été
reproduit, et chaque correction a été vérifiée (tests unitaires et essais dans
l'interface). Les choix d'architecture native sont détaillés dans
[architecture-native.md](architecture-native.md).

## 1. Architecture actuelle

| Partie | Fichiers | Taille | Rôle |
|---|---|---|---|
| Serveur | `studio/src/main.rs`, `http.rs` | ~1 250 lignes | serveur HTTP sur `127.0.0.1` (jeton, contrôle de `Host`), API fichiers, recherche, compilation, services `nxc ide`, paquets, extensions, aperçu web (« live server ») |
| Exécution | `studio/src/runner.rs` | 180 | lance les programmes, sortie lue en continu, entrée clavier, arrêt |
| Débogage | `studio/src/debug.rs` | 110 | pont vers `lldb-dap` (protocole DAP) |
| Fichiers | `studio/src/workspace.rs` | nouveau | opérations sur les fichiers du dossier ouvert (voir §3) |
| Interface | `studio/ui/*.js`, `style.css`, `index.html` | ~6 800 lignes | atelier (`app.js`), éditeur (`editor.js`), services du compilateur, tests, débogueur, paquets (`ide.js`), langages (`langs.js`), extensions (`plugins.js`) |

Lancement : `NEXUL Studio.exe` démarre le serveur puis ouvre l'interface dans Microsoft
Edge en mode application (`--app`, profil dans `%APPDATA%\NEXUL Studio\window`). Sans
Edge, le navigateur par défaut est utilisé. Le serveur s'arrête 12 s après le dernier
signal de la fenêtre.

L'éditeur est un `<textarea>` transparent (caret, sélection, IME et presse-papiers natifs
du navigateur) au-dessus d'un rendu coloré des seules lignes visibles. Il n'a qu'un seul
curseur et une seule sélection.

Réglages et sessions : `%APPDATA%\NEXUL Studio\state.json`.

## 2. Composants réutilisables

| Composant | Réutilisable dans une interface native ? |
|---|---|
| `workspace.rs` (fichiers) | **oui, tel quel** : aucune dépendance à l'interface, messages en français, tests |
| `runner.rs` (exécution) | oui ; à exposer sous forme de bibliothèque |
| `debug.rs` (DAP) | oui pour le transport ; la logique du client DAP est aujourd'hui en JavaScript (`ide.js`) et devra être portée en Rust |
| Appels à `nxc` (`check --json`, `ide`, `explain`, `std-source`, `fmt`) et `nxpm` | oui ; à regrouper dans un module « client du compilateur » |
| Aperçu web (live server) | oui : l'aperçu d'un site exige de toute façon un moteur web, isolé de l'interface principale (exception prévue par la mission) |
| Recherche dans les fichiers | oui |
| Interface JavaScript | non (l'interface native la remplace) ; seules les **règles** sont reprises : coloration de `langs.js`, extraits de code, raccourcis, textes |
| Extensions (API v1, Web Workers JavaScript) | non directement : une interface native n'a pas de moteur JavaScript. À reconcevoir (voir §6) |

## 3. Problèmes confirmés et corrections (phase 1)

Corrigés dans le commit `1492b45` (« Studio: fix file and folder creation… »).

| # | Problème constaté | Cause | Correction |
|---|---|---|---|
| 1 | **Créer un fichier ou un dossier depuis l'explorateur ne fait rien** : le champ du nom disparaît, rien n'est créé, aucun message | `startInlineNew()` appelait `showView('explorer')`, qui redessine l'arbre, puis le redessinait une seconde fois ; le second dessin retire le champ qui a le focus, Chromium émet alors `blur` pendant le retrait, et ce `blur` validait un nom vide | un seul dessin ; un dessin de l'arbre ne termine plus la saisie (le champ et son texte sont conservés) |
| 2 | **Renommer (F2, menu)** échouait de la même façon | même cause (`startRename()`) | idem |
| 3 | Un redessin pendant la saisie (diagnostics, actualisation) abandonnait le nom ou **validait un nom tronqué** | même mécanisme | idem |
| 4 | Renommer `main.nx` en `Main.nx` refusé (« ce nom existe déjà ») | le test d'existence ignore l'insensibilité à la casse de Windows | changement de casse autorisé |
| 5 | `abc.` créait `abc` en silence ; `CON.nx`, `NUL`, `COM1`… acceptés | aucune validation des noms côté serveur | validation complète (caractères interdits, point ou espace final, noms réservés, 255 caractères) |
| 6 | Messages bruts du système (« Paramètre incorrect. (os error 87) ») | erreurs transmises telles quelles | messages en français : lecture seule, fichier utilisé par un autre programme, accès refusé, introuvable, disque plein… |
| 7 | Déplacer un dossier dans lui-même : erreur 87 | non vérifié | refusé avec un message clair |
| 8 | Suppression lente (1,2 s) | PowerShell + `Microsoft.VisualBasic` à chaque suppression | `SHFileOperationW` (Corbeille de Windows) : 0,1 s ; avertissement de Windows si le disque n'a pas de Corbeille |
| 9 | Aucun déplacement de fichiers | non implémenté | **glisser-déposer** dans l'explorateur (Ctrl : copie), **couper / copier / coller** (Ctrl+X, C, V et menu) ; rien n'est jamais écrasé (`nom (2).ext`) ; les onglets ouverts suivent |
| 10 | Fichiers venant de l'Explorateur Windows non pris en charge, et **un fichier déposé sur la fenêtre remplaçait toute l'application** par ce fichier | comportement par défaut du navigateur | fichiers et dossiers déposés copiés dans le projet (dans le dossier visé, ou celui du fichier actif) ; navigation bloquée |
| 11 | Fichier ouvert supprimé en dehors de Studio : aucun signal | — | notification ; il est recréé à l'enregistrement |
| 12 | Session : un fichier renommé en dehors de Studio (casse) n'était pas rouvert | comparaison sensible à la casse | comparaison insensible à la casse |

Vérifications :
- `cargo test -p nexul-studio` : 5 tests (`workspace::tests`) — noms, création,
  enregistrement, lecture seule, renommage (casse, dans lui-même, collisions),
  déplacement, copie, import ; la mise à la Corbeille réelle est testée avec
  `NEXUL_STUDIO_TEST_TRASH=1` (elle utilise la Corbeille de l'utilisateur).
- Essais dans l'interface (clavier et souris réels, et événements de glisser-déposer
  simulés) : création de fichier et de dossier, saisie pendant deux redessins, F2 avec
  changement de casse, glisser-déposer, Ctrl+glisser, déplacement d'un dossier dans
  lui-même (refusé), couper/coller, copier/coller (`main (2).nx`), import d'un fichier
  texte et d'un fichier binaire (octets vérifiés), dépôt sur l'éditeur, suppression avec
  confirmation, restauration de la session après rechargement.

## 4. Problèmes restants (non corrigés en phase 1)

- ~~L'arbre complet du dossier était renvoyé toutes les 4 s et l'aperçu web parcourait
  le dossier toutes les 700 ms~~ : **corrigé** (commit `034e8c9`) par une surveillance
  du dossier (`ReadDirectoryChangesW`, `studio/src/watch.rs`) ; l'interface ne relit
  l'arbre et les fichiers ouverts que lorsqu'ils changent (moins d'une seconde).
- Les fichiers qui ne sont pas en UTF-8 (Windows-1252, UTF-16) ne sont pas détectés :
  ils s'affichent avec des caractères de remplacement et seraient abîmés à
  l'enregistrement.
- Un fichier modifié sur le disque alors qu'il a des modifications non enregistrées dans
  Studio est écrasé à l'enregistrement, sans avertissement de conflit.
- Sélection simple dans l'explorateur (pas de sélection multiple).
- Un seul curseur dans l'éditeur ; pas de sélection en colonne.
- Les gros fichiers (> 8 Mo) sont refusés par l'éditeur.
- Interface dépendante d'Edge : sans Edge, elle s'ouvre dans un onglet du navigateur.

## 5. Analyse de l'interface actuelle

Constats, au regard de la direction artistique demandée :
- **Dégradés** : logo, bouton principal et accents (`linear-gradient` dans `style.css`,
  et dans l'installateur).
- **Arrondis** nombreux et variés (2 à 10 px, 14 occurrences de 8 px), cartes de la page
  d'accueil et du concepteur web.
- **Densité** : barre d'activité, onglets, fil d'Ariane, barre d'état et panneaux
  simultanés ; la minimap et les statistiques sont toujours visibles par défaut.
- **Outils web** (aperçu, concepteur) mis au même niveau que l'édition de code.
- Points positifs à conserver : palette de commandes, ouverture rapide, raccourcis
  proches de VS Code, thèmes clair et sombre, coloration lisible.

## 6. Risques et dépendances

| Risque | Conséquence | Réponse |
|---|---|---|
| Pas de compilateur C/C++ sur la machine de développement (Visual Studio est installé sans les outils C++ ; la chaîne Rust est `windows-gnu`) | toute bibliothèque qui compile du C/C++ (Qt, grammaires tree-sitter, Skia) ne se construit pas ici | privilégier une pile 100 % Rust ; sinon, installer les outils C++ (décision de l'utilisateur) |
| `Cargo.lock` et `Cargo.toml` racine appartiennent à Codex | une interface native ajoute de nombreuses dépendances | l'application native vit dans son **propre espace de travail Cargo** sous `studio/` (son propre `Cargo.lock`), sans toucher aux fichiers de Codex ; son intégration au workspace racine sera demandée à Codex |
| Contrat WebAssembly de Codex (`docs/refonte/interop-web.md`) non publié ; Codex momentanément indisponible | l'intégration « Compiler pour le Web » (phase 4) est bloquée | rien n'est inventé : la fonction attendra le contrat |
| Pas de serveur LSP pour NEXUL (`nxc ide` est un protocole JSON propre) | un client LSP générique ne couvre pas NEXUL | Studio garde `nxc ide` ; demande à Codex d'un mode `nxc lsp` |
| Extensions JavaScript (API v1) sans moteur JavaScript dans une interface native | les extensions existantes ne fonctionneraient plus | conserver l'ancienne interface pendant la migration ; nouvelle API d'extensions à concevoir (par exemple des modules WebAssembly, que NEXUL sait produire) |
| Accessibilité des boîtes à outils Rust inégale | lecteurs d'écran | critère de choix (AccessKit) |
| Taille du chantier (un IDE complet) | risque de fonctionnalités factices | migration progressive, ancienne interface conservée jusqu'à parité |

## 7. Plan de réalisation

| Phase | Studio (Claude) | Validation |
|---|---|---|
| 1 — corrections | gestion des fichiers et dossiers (**fait**) | tests unitaires + essais interface |
| 2 — architecture | choix de la boîte à outils native (**à valider par l'utilisateur**) ; extraction des services Rust en bibliothèque (`studio/core`) utilisée par l'ancienne et la nouvelle interface ; squelette de l'application native | prototype mesuré : démarrage, mémoire, saisie, Unicode, IME |
| 3 — interface native | fenêtre, menus, palette de commandes, explorateur (surveillance des fichiers), onglets, éditeur, terminal intégré (ConPTY), diagnostics, barre d'état, paramètres, thèmes, mode minimaliste, dispositions mémorisées | parité avec l'ancienne interface pour NEXUL |
| 4 — avancé | client LSP (rust-analyzer, clangd, pyright, typescript-language-server…, détectés et configurés explicitement), DAP, projets web et WebAssembly (selon le contrat de Codex), nouvelle API d'extensions | essais par langage, en distinguant coloration seule et prise en charge complète |
| 5 — intégration | installateur sans Edge, retrait de l'ancienne interface, essais Windows complets | installation réelle, projets NEXUL, multilangage, NEXUL + WebAssembly + JavaScript |

## 8. Demandes à Codex

1. **`nxc lsp`** : un serveur Language Server Protocol (stdio) construit sur les services
   de `nxc ide` (survol, complétion, définition, références, renommage, symboles,
   diagnostics, mise en forme), pour que Studio utilise un seul client LSP pour tous les
   langages. Critère : `initialize`, `textDocument/didOpen|didChange`,
   `publishDiagnostics`, `hover`, `completion`, `definition`, `references`, `rename`,
   `documentSymbol`, `formatting`.
2. **Schéma versionné** des sorties JSON de `nxc check --json` et `nxc ide` (champ de
   version), pour détecter une incompatibilité au lieu d'afficher des erreurs muettes.
3. **Contrat WebAssembly** `docs/refonte/interop-web.md` (prévu par la mission).
4. **Intégration au workspace racine** de l'application native et de ses dépendances,
   ou accord pour qu'elle garde son propre espace de travail Cargo.
