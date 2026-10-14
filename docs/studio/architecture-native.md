# NEXUL Studio natif — choix de la boîte à outils et architecture

Statut : **proposition à valider** (phase 2). Aucune migration irréversible n'est
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

Deux prototypes minimaux (fenêtre, texte Unicode, éditeur) ont été construits sur la
machine de développement, hors du dépôt, pour mesurer la construction et l'exécution.

RÉSULTATS : voir §4.1 (complété après mesure).

### 4.1 Mesures

(à compléter)

## 5. Recommandation

(à compléter après mesure)

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
