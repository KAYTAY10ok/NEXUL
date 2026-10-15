# Services d'éditeur de `nxc` — contrat v1

`nxc version --json`, `nxc check --stdin` et le serveur de langage `nxc lsp`.

| | |
|---|---|
| Statut | `CONÇU` — contrat **approuvé par Claude Main** le 15/10/2026 (lot P9) |
| Version du contrat | **1** |
| Implémentation | Codex Sol 6.1, sur mandat écrit (`docs/refonte/prompt-codex-sol.md`, règles) |
| Utilisateurs | NEXUL Studio (`studio/app` par `studio/lsp`), extension VS Code (`editors/vscode`) |

Les mots **DOIT**, **NE DOIT PAS** et **PEUT** sont normatifs. Toute question sur ce contrat
va à Claude Main avant d'écrire du code ; un changement du contrat est proposé à Main, qui
l'approuve et incrémente sa version.

## 0. Principes

1. **Une seule analyse.** Le serveur réutilise ce qui existe dans `compiler/nxc` :
   `compile_source` pour les diagnostics, `ide::analyze`, `ide::hover`, `ide::definition`,
   `ide::references`, `ide::rename`, `ide::symbols`, `ide::completion`,
   `ide::format_source`. Il NE DOIT PAS réinterpréter NEXUL autrement que `nxc check` et
   `nxc ide`.
2. **Aucune dépendance externe.** JSON-RPC, lecture et écriture JSON sont écrits dans la
   crate `nxc` (aucune nouvelle crate dans `Cargo.toml`).
3. **Compatibilité.** Les sorties existantes (`nxc check --json`, `nxc ide …`) NE DOIVENT
   PAS changer. Tout ce qui suit est un ajout.
4. **Honnêteté.** Une capacité n'est annoncée que si elle est implémentée et testée.
5. **Lecture seule.** Ni `nxc check --stdin` ni `nxc lsp` n'écrivent de fichier : le
   renommage et la mise en forme renvoient des modifications que le client applique.

## 1. Découverte : `nxc version --json`

`nxc version --json` DOIT écrire un seul objet JSON sur une ligne, puis terminer avec le
code 0 :

```json
{"nxc":"0.2.0","schemas":{"diagnostics":1,"ide":1},"lsp":1,"stdin":["check","ide"]}
```

| Champ | Sens |
|---|---|
| `nxc` | version du compilateur, la même que `nxc version` |
| `schemas.diagnostics` | version du format de `nxc check --json` (§2) |
| `schemas.ide` | version des sorties de `nxc ide` (formats actuels = 1) |
| `lsp` | version de ce contrat implémentée par `nxc lsp` ; `0` si le serveur n'existe pas |
| `stdin` | commandes qui acceptent `--stdin` |

Règles pour les clients :

- un client DOIT ignorer les champs inconnus ;
- un changement incompatible d'un format incrémente son numéro ; un client qui voit un
  numéro plus grand que ceux qu'il connaît DOIT se replier (par exemple garder `nxc ide`)
  au lieu de mal lire ;
- si `nxc version --json` échoue ou n'écrit pas un objet JSON (`nxc` 0.2.0), le client
  DOIT supposer `diagnostics = 1`, `ide = 1`, `lsp = 0`, `stdin = ["ide"]`.

## 2. Diagnostics JSON, format 1 (existant)

Ce paragraphe décrit le format **actuel** de `nxc check --json` ; il ne change pas. La
sortie est un tableau d'objets :

| Champ | Type | Sens |
|---|---|---|
| `level` | `"error"` ou `"warning"` | gravité |
| `code` | chaîne | code du diagnostic (`E0017`, `W0001`…), expliqué par `nxc explain` |
| `message` | chaîne | message principal |
| `file` | chaîne | fichier, tel que chargé par le compilateur |
| `line`, `col`, `end_line`, `end_col` | entiers | position du libellé principal : lignes et colonnes **à partir de 1**, colonnes comptées en **valeurs scalaires Unicode**, fin **exclue** ; `0` partout si le diagnostic n'a pas de position |
| `labels` | tableau | libellés : `primary` (booléen), `message`, `file`, `line`, `col`, `end_line`, `end_col` |
| `notes` | tableau de chaînes | notes et aides, préfixées par `note: ` ou `help: ` |

Les sorties de `nxc ide` utilisent les mêmes conventions de position.

## 3. `nxc check --stdin`

```
nxc check <fichier.nx> --stdin [--json] [autres options de check]
```

- Le texte du fichier est lu sur l'**entrée standard** (UTF-8) au lieu du disque.
  `<fichier.nx>` donne le nom affiché, le dossier où sont cherchés les modules importés et
  le champ `file` des diagnostics. Le fichier PEUT ne pas exister sur le disque.
- Les modules importés sont lus sur le disque (limite de la v1).
- La sortie, le format (`--json` ou texte) et le **code de sortie** sont ceux de
  `nxc check` sur un fichier ayant ce contenu.
- `--stdin` NE DOIT PAS être accepté par `build`, `run` et `test` : message d'erreur clair
  et code de sortie non nul.

## 4. `nxc lsp`

### 4.1 Lancement et transport

- `nxc lsp`, sans autre argument (un argument inconnu : message sur la sortie d'erreur,
  code de sortie 2).
- Entrée et sortie standard, **JSON-RPC 2.0** avec le protocole de base de **LSP 3.17**
  (en-têtes `Content-Length`, contenu UTF-8).
- La sortie standard ne porte que le protocole. La sortie d'erreur PEUT porter un journal,
  seulement si la variable d'environnement `NXC_LSP_LOG=1`.
- Code de sortie : 0 après `shutdown` puis `exit` ; 1 après `exit` sans `shutdown`, ou si
  l'entrée se ferme sans `exit`.

### 4.2 Initialisation

- Réponse à `initialize` : les capacités du §4.3 et
  `serverInfo: {"name": "nxc", "version": "<version de nxc>"}`.
- **Encodage des positions** : si `capabilities.general.positionEncodings` du client
  contient `"utf-32"`, le serveur DOIT choisir `"utf-32"` ; sinon `"utf-16"` (défaut de
  LSP). Le choix est renvoyé dans `capabilities.positionEncoding`. Les positions de LSP
  partent de 0 ; celles de `nxc` partent de 1 et comptent des valeurs scalaires : le serveur
  DOIT convertir exactement, y compris pour les caractères hors du plan de base (deux unités
  UTF-16).
- Avant `initialize` : erreur `-32002` (ServerNotInitialized). Après `shutdown` : erreur
  `-32600` (InvalidRequest), sauf `exit`.

### 4.3 Capacités de la version 1

| Capacité annoncée | Méthode | Réalisation |
|---|---|---|
| `textDocumentSync: {"openClose": true, "change": 1, "save": {"includeText": false}}` | `didOpen`, `didChange` (texte complet), `didSave`, `didClose` | texte de chaque document ouvert gardé en mémoire |
| (diagnostics) | `textDocument/publishDiagnostics` | analyse de `compile_source` sur le texte en mémoire (§4.4) |
| `hoverProvider: true` | `textDocument/hover` | `ide::hover` → `MarkupContent` Markdown : signature dans un bloc ```` ```nexul ````, puis la documentation |
| `completionProvider: {"triggerCharacters": ["."]}` | `textDocument/completion` | `ide::completion` → `CompletionItem` (`label`, `kind` §4.5, `detail`, `documentation`) ; `isIncomplete: false` |
| `definitionProvider: true` | `textDocument/definition` | `ide::definition` → `Location`, ou `null` |
| `referencesProvider: true` | `textDocument/references` | `ide::references` → `Location[]` ; respecte `context.includeDeclaration` |
| `renameProvider: true` | `textDocument/rename` | `ide::rename` → `WorkspaceEdit` (`changes` par URI) ; refus de `nxc` → erreur `-32803` (RequestFailed) avec son message |
| `documentSymbolProvider: true` | `textDocument/documentSymbol` | `ide::symbols` → `DocumentSymbol[]` hiérarchiques (`children`) |
| `documentFormattingProvider: true` | `textDocument/formatting` | `ide::format_source` → un seul `TextEdit` qui remplace tout le document, ou `[]` si rien ne change |

**Pas en v1** (NE DOIT PAS être annoncé) : synchronisation incrémentale, symboles du
projet, jetons sémantiques, actions de code, aide à la signature, indications en ligne,
mise en forme d'une plage. Une méthode non annoncée : erreur `-32601` (MethodNotFound) ;
une notification inconnue est ignorée.

### 4.4 Documents, analyse et diagnostics

- Un document ouvert est analysé avec son texte en mémoire ; les autres fichiers (modules
  importés, documents non ouverts) sont lus sur le disque.
- Analyse après `didOpen`, après `didChange` (au plus tôt **200 ms** après la dernière
  modification ; une analyse dépassée par une version plus récente est abandonnée) et
  après `didSave`.
- `publishDiagnostics` porte l'URI, la `version` du document analysé et ses
  diagnostics :
  - `range` : position convertie (§4.2) ;
  - `severity` : 1 pour `error`, 2 pour `warning` ;
  - `code` : le code de `nxc` ; `source` : `"nxc"` ;
  - `message` : le message, suivi des `notes` (une par ligne) ;
  - `relatedInformation` : les libellés non principaux avec un message (emplacement +
    message).
- Les diagnostics situés dans un autre fichier sont publiés pour l'URI de ce fichier. Un
  fichier qui avait des diagnostics et n'en a plus reçoit une liste vide. `didClose`
  publie une liste vide pour le document fermé.
- Une requête sur un document non ouvert lit le fichier sur le disque ; fichier illisible :
  résultat vide (`null` ou `[]`), pas d'erreur fatale.

### 4.5 Correspondance des genres

Les genres de `nxc ide` deviennent ceux de LSP ; un genre inconnu prend la valeur de repli.

| `nxc ide` | `CompletionItemKind` | `SymbolKind` |
|---|---|---|
| `function`, `test` | 3 Function | 12 Function |
| `method` | 2 Method | 6 Method |
| `variable`, `parameter` | 6 Variable | 13 Variable |
| `field` | 5 Field | 8 Field |
| `struct` | 22 Struct | 23 Struct |
| `enum` | 13 Enum | 10 Enum |
| `variant` | 20 EnumMember | 22 EnumMember |
| `trait` | 8 Interface | 11 Interface |
| `module` | 9 Module | 2 Module |
| `constant`, `static` | 21 Constant | 14 Constant |
| `type` | 7 Class | 5 Class |
| `keyword` | 14 Keyword | — |
| autre (repli) | 1 Text | 13 Variable |

Un symbole `test` garde `detail` de `nxc` et ajoute `"test"` dans `detail` s'il n'y est pas.

### 4.6 URI et bibliothèque standard

- URI `file:` : décodage des `%XX`, lecteurs Windows (`file:///C:/…`, `file:///c%3A/…`,
  insensibles à la casse), chemins UNC (`file://serveur/partage/…`). Pour un document
  ouvert, le serveur DOIT renvoyer l'URI **telle que le client l'a envoyée**.
- Une définition dans la bibliothèque standard (`"std": true` dans `nxc ide`, fichier nommé
  `<std.vec>` par le compilateur) renvoie l'URI `nexul-std:///<module>`, par exemple
  `nexul-std:///std.vec`. Le client obtient son texte par la requête propre au serveur
  **`nexul/stdSource`** : `{"uri": "nexul-std:///std.vec"}` → `{"text": "…"}` (même contenu que
  `nxc ide std-source std.vec`) ; module inconnu : erreur `-32803`.

### 4.7 Robustesse

- JSON invalide : réponse d'erreur `-32700` (ParseError) avec `id: null`, puis le serveur
  continue.
- Erreur interne pendant une requête : erreur `-32603` (InternalError) avec un message ; le
  serveur continue. Une panique dans l'analyse NE DOIT PAS arrêter le serveur.
- `$/cancelRequest` : la requête annulée reçoit l'erreur `-32800` (RequestCancelled) si
  elle n'a pas encore répondu.
- Le serveur PEUT traiter les requêtes une à une ; il DOIT rester réactif à `shutdown`.

## 5. Critères d'acceptation (tests obligatoires)

Tests dans `compiler/nxc/tests/`, qui lancent le **vrai binaire** `nxc`
(`env!("CARGO_BIN_EXE_nxc")`) :

1. `nxc version --json` : objet conforme au §1 ; `lsp` vaut 1 une fois le serveur livré.
2. `nxc check --stdin --json` : texte différent du disque → diagnostics du texte reçu, même
   format et même code de sortie que `check` sur ce texte ; le fichier sur le disque n'est
   pas modifié ; `nxc build --stdin` refusé.
3. Cycle `initialize` → `initialized` → `shutdown` → `exit`, codes de sortie du §4.1 ;
   requête avant `initialize` → `-32002`.
4. `didOpen` d'un texte erroné → mêmes codes et positions que `nxc check --json` sur ce
   texte ; `didChange` qui corrige → liste vide avec la nouvelle `version` ; `didClose` →
   liste vide.
5. Survol, complétion, définition, références, renommage, symboles et mise en forme sur un
   programme d'exemple : mêmes résultats que `nxc ide` après conversion.
6. Positions en `utf-16` et en `utf-32`, avec `é` et `😀` avant le symbole visé.
7. Définition dans la bibliothèque standard → URI `nexul-std:` ; `nexul/stdSource` renvoie
   le texte.
8. Méthode inconnue → `-32601` ; JSON invalide → `-32700` puis réponse correcte à la
   requête suivante.
9. Temps mesuré de `didOpen` à `publishDiagnostics` sur `examples/` (valeur rapportée dans
   la PR, sans seuil imposé en v1).

## 6. Après la v1

- Main met à jour `docs/spec/10-compilateur.md` (ligne de commande) une fois la v1
  fusionnée.
- Studio (Main) et l'extension VS Code (Codex Sol) démarrent `nxc lsp` quand
  `nxc version --json` annonce `lsp >= 1`, et gardent `nxc ide` sinon.
- Pistes pour la v2 : synchronisation incrémentale, textes en mémoire pour les modules
  importés, symboles du projet, actions de code tirées des aides de `nxc`, jetons
  sémantiques (après l'ADR de syntaxe).
