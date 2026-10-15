# NEXUL Studio natif — choix de la boîte à outils et architecture

Statut : **proposition à valider** (phase 2) — prototypes mesurés, recommandation au §5. Aucune migration irréversible n'est
engagée avant la validation de ce choix par l'utilisateur.

## 1. Objectif et contraintes

Remplacer l'interface actuelle (pages web affichées par Edge et servies par un serveur
HTTP local) par une **application de bureau native** :
- fenêtre native, sans navigateur ni moteur HTML/CSS/JavaScript pour l'interface
  principale, sans serveur HTTP obligatoire ;
- menus, raccourcis, boîtes de dialogue, intégration au système ; hors ligne ;
- démarrage rapide, faible latence de saisie, mémoire raisonnable ;
- un **éditeur de code professionnel** (curseurs multiples, recherche et remplacement,
  repliement, complétion, survol, diagnostics, Unicode, gros fichiers), de préférence un
  composant existant et mûr plutôt qu'un moteur de texte refait ;
- réutilisation des services Rust existants (fichiers, exécution, débogage, paquets,
  compilateur) ;
- Windows d'abord, puis Linux et macOS ;
- licences compatibles avec la distribution libre de NEXUL (MIT / Apache-2.0).

Exception prévue : l'**aperçu d'un site web** peut utiliser un moteur web isolé (fenêtre
séparée ou navigateur du système), sans que l'interface principale en dépende.

## 2. Critères

1. Composant d'édition de code disponible et sa qualité.
2. Rendu du texte : Unicode, écritures complexes (arabe, CJK), emoji, polices de repli,
   IME (saisie asiatique, touches mortes).
3. Clavier : raccourcis, menus, navigation.
4. Accessibilité (lecteurs d'écran).
5. Mise en page d'un IDE : panneaux redimensionnables, ancrables, onglets, listes
   virtualisées (gros arbres, gros fichiers).
6. Licence.
7. Construction et distribution : chaîne d'outils nécessaire, taille, dépendances
   système, multiplateforme.
8. Maintenance : activité, stabilité de l'API, communauté.
9. Performances mesurées sur un prototype.

## 3. Candidats

| | Qt 6 (cxx-qt) | Slint 1.18 | GPUI + gpui-component 0.7 | iced 0.14 | egui | Floem | Win32 / WinUI |
|---|---|---|---|---|---|---|---|
| Langage de l'interface | C++/QML + pont Rust | DSL `.slint` + Rust | Rust | Rust | Rust | Rust | Rust (FFI) |
| Éditeur de code fourni | QPlainTextEdit (base), KTextEditor (LGPL, lourd) ; QScintilla (GPL) | **non** (`TextEdit` simple) | **oui** : tampon en corde, curseurs multiples, sélection en colonne, recherche et remplacement, repliement, fournisseurs LSP (complétion, survol, définition, actions, jetons sémantiques), coloration enfichable (tree-sitter fourni), diagnostics | `text_editor` simple (un seul curseur) | `TextEdit` simple, lent sur gros textes | éditeur de Lapce, mais crate figée en 0.2.0 (2024), Lapce utilise git | contrôle RichEdit / Scintilla à intégrer |
| Mise en page IDE | dock Qt | à construire | **dock** : panneaux redimensionnables, onglets déplaçables, divisions, sérialisables ; listes et tables virtualisées | à construire | à construire | partielle | à construire |
| Accessibilité | excellente | AccessKit | AccessKit (annoncée) | limitée | AccessKit | faible | native |
| Licence | LGPLv3 / commerciale | GPLv3, ou « royalty-free » avec mention obligatoire (widget AboutSlint ou badge), ou commerciale | Apache-2.0 | MIT | MIT/Apache | MIT | — |
| Construction ici | **impossible sans outils C++** et SDK Qt (plusieurs Go) | Rust pur | Rust pur (tree-sitter optionnel, en C) | Rust pur | Rust pur | Rust pur | Rust pur, Windows seulement |
| Multiplateforme | oui | oui | oui (Windows, macOS, Linux) | oui | oui | oui | non |
| Maturité | très grande | grande (versions régulières, 1.18.1 en sept. 2026) | pré-1.0 ; gpui-component publie souvent (0.7.3 le 12 oct. 2026), utilisée en production (Longbridge Pro) ; GPUI vient de Zed | moyenne, versions espacées (0.14 en déc. 2025) | grande pour des outils | faible côté crate publiée | — |

Écartés d'emblée :
- **Tauri / WebView2 / Electron** : moteur web pour l'interface principale (interdit).
- **GTK 4 + GtkSourceView** : très bon composant d'édition, mais distribution lourde
  sous Windows (MSYS2, ~100 Mo de DLL) et rendu non natif sous Windows.
- **Win32 / WinUI** : pas de multiplateforme, et tout serait à construire.

## 4. Prototypes

Deux prototypes (fenêtre native, éditeur de texte chargé avec un fichier de 100 000
lignes et du texte Unicode) ont été construits sur la machine de développement, hors du
dépôt, pour mesurer la construction et l'exécution.

### 4.0 Chaîne de construction

Constat sur la machine de développement (Rust `windows-gnu`, sans compilateur C/C++) :
- les crates `windows` récentes importent les fonctions du système en `raw-dylib`, ce qui
  exige un `dlltool` complet : celui de rustup n'a pas d'assembleur ; `llvm-dlltool`
  convient pour les appels simples ;
- la MinGW minimale de rustup n'a pas toutes les bibliothèques d'importation
  (`shlwapi`, `d3d11`, `dwrite`…) ; en les générant depuis les DLL du système, le
  prototype Slint se construit mais **plante au démarrage** (exécution d'une adresse
  invalide : imports mal liés) ;
- GPUI compile ses shaders HLSL pendant la construction avec `fxc.exe`, fourni par le
  **Windows SDK**.

Conclusion : l'application native se construit avec la chaîne **MSVC** (outils C++ de
Visual Studio et Windows SDK), configuration standard des applications Rust de bureau
sous Windows. Le compilateur NEXUL n'est pas concerné (il n'a aucune dépendance).

RÉSULTATS : voir §4.1.

### 4.1 Mesures

Machine : Windows 11, chaîne MSVC 14.51, Rust 1.99, profil `release`. Démarrage =
temps jusqu'à l'apparition de la fenêtre ; mémoire = mémoire privée 3 à 4 s après.
Pendant les mesures, l'installateur de Visual Studio tournait encore : les temps de
démarrage sont pessimistes.

| | Slint 1.18 (`TextEdit`) | GPUI + gpui-component 0.7 (`Editor`) |
|---|---|---|
| Construction (à froid) | 3 min 49 s | 5 min 51 s |
| Taille de l'exécutable | 13,7 Mo | 22,5 Mo |
| Démarrage, petit texte | 3,4 s | 1,5 s |
| Mémoire, petit texte | 32 Mo privés (77 Mo de *working set*) | 66 Mo privés (112 Mo) |
| Démarrage, fichier de 100 000 lignes (2,2 Mo) | 6,2 s | 0,9 s |
| Mémoire, 100 000 lignes | **650 Mo** privés | **71 Mo** privés |
| Accents, CJK, emoji en couleur | corrects | corrects |
| Arabe (écriture de droite à gauche) | lettres liées et ordre correct | lettres liées mais **ordre de gauche à droite** : pas de réordonnancement bidirectionnel |
| Éditeur | zone de texte simple : police proportionnelle, ni numéros de ligne, ni coloration, ni curseurs multiples | éditeur de code : police à chasse fixe, numéros de ligne, guides d'indentation, tampon en corde (gros fichiers), curseurs multiples, recherche et remplacement, repliement, fournisseurs LSP |

Images des prototypes : non versionnées (machine de développement). Non mesuré : latence
de frappe et saisie IME (à vérifier sur le prototype de l'application avant la parité).

## 5. Recommandation

**GPUI + gpui-component** (Apache-2.0), sous réserve de validation.

Raisons :
1. C'est la seule option qui fournit un **éditeur de code complet** (curseurs multiples,
   recherche/remplacement, repliement, LSP, gros fichiers) : la mission demande de
   réutiliser un composant mûr plutôt que de refaire un moteur de texte. Avec Slint ou
   iced, cet éditeur serait à écrire.
2. La **mise en page d'IDE** existe : panneaux ancrables et redimensionnables, onglets
   déplaçables, dispositions sérialisables, listes virtualisées, menus, dialogues,
   thèmes.
3. Mesures nettement meilleures sur les gros fichiers (71 Mo contre 650 Mo, 0,9 s contre
   6,2 s) et démarrage plus rapide.
4. Licence Apache-2.0, sans obligation d'attribution visible (Slint : GPL ou mention
   « Made with Slint »).
5. Rust pur côté application, rendu GPU (DirectX 11 sous Windows), multiplateforme.

Risques et parades :
- **Pré-1.0** : API qui change (GPUI via `gpui-pre`, figé par gpui-kit). Parade : versions
  figées dans `Cargo.lock`, services dans `studio/core` sans dépendance à GPUI, mises à
  jour regroupées.
- **Texte de droite à gauche** non réordonné dans l'éditeur. Parade : signaler la
  limite ; demande en amont ; impact faible pour du code (chaînes en arabe ou en hébreu).
- **Accessibilité** annoncée (AccessKit) mais à vérifier avec le Narrateur de Windows.
- **Construction** : chaîne MSVC + Windows SDK (shaders HLSL), 6 min à froid, exécutable
  de 22 Mo.
- Dépendance à deux projets (Zed pour GPUI, Longbridge pour les composants). Parade :
  code de l'interface isolé dans `studio/app`, remplaçable sans toucher aux services.

Repli : Slint (mûr, accessible, réordonnancement bidirectionnel correct), au prix d'un
éditeur de code à écrire.

## 6. Architecture cible

```
studio/
  core/        bibliothèque Rust sans interface : fichiers (workspace.rs), surveillance
               des fichiers, exécution (runner), client du compilateur (nxc check/ide/fmt/
               explain), nxpm, client DAP, client LSP, recherche, paramètres, sessions,
               modèle de projet
  app/         application native (son propre espace de travail Cargo et son Cargo.lock,
               pour ne pas modifier les fichiers de Codex) : fenêtre, panneaux, éditeur,
               terminal, paramètres, thèmes
  src/, ui/    ancienne interface (serveur + pages web), conservée jusqu'à la parité,
               puis retirée ; elle utilise déjà workspace.rs
```

- **Un seul processus** : l'interface et les services communiquent par appels de
  fonctions et canaux, sans HTTP. Les outils externes (`nxc`, `nxpm`, `lldb-dap`, serveurs
  LSP, programmes de l'utilisateur) restent des processus séparés.
- **Éditeur** : composant de la boîte à outils retenue ; coloration de NEXUL par un
  module Rust (lexique de NEXUL, puis jetons sémantiques de `nxc`), coloration des autres
  langages par grammaires.
- **Services du langage** : NEXUL via `nxc ide` (puis `nxc lsp` s'il est fourni par
  Codex) ; autres langages via LSP, serveurs détectés et configurés explicitement
  (rust-analyzer, clangd, pyright, typescript-language-server, csharp-ls…). Un langage
  sans serveur est annoncé « coloration seulement ».
- **Terminal intégré** : ConPTY (Windows) / pty, émulation VT, rendu dans l'interface.
- **Débogage** : DAP (lldb-dap pour NEXUL, C, C++, Rust).
- **Aperçu web** : navigateur du système ou fenêtre séparée isolée (exception).
- **Extensions** : l'API v1 (JavaScript) reste dans l'ancienne interface ; une API pour
  l'application native sera proposée séparément.
- **Paramètres** : fichier JSON dans `%APPDATA%\NEXUL Studio\`, reprise des réglages de
  l'ancienne interface.
