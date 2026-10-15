# `nexul-studio-lsp` — client Language Server Protocol de NEXUL Studio

Crate Rust **sans dépendance** (ni GPUI, ni autre crate) : un client LSP générique, le
registre des langages, de leurs serveurs et des SDK, et leur **détection** sur
l'ordinateur, sans rien télécharger. Le serveur tourne dans un processus, lu par un fil ;
réponses et événements arrivent par des canaux, que l'interface attend sur ses propres
tâches de fond.

Responsable : **Claude Outils** (voir `docs/refonte/AGENTS.md`). Utilisateurs :
Claude Studio (adaptateurs GPUI dans `studio/app`) et Codex Luna (détection des SDK pour
`studio/templates`).

NEXUL n'en fait **pas** partie tant que `nxc lsp` n'existe pas : Studio garde `nxc ide`.

## Utilisation depuis `studio/app`

```toml
[dependencies]
nexul-studio-lsp = { path = "../lsp" }
```

Espace de travail Cargo propre (`[workspace]` vide, son propre `Cargo.lock`). Le binaire
`fake-lsp-server` de la crate ne sert qu'aux tests : il n'est pas construit quand la crate
est une dépendance.

```rust
use nexul_studio_lsp::registry::{self, Detector};
use nexul_studio_lsp::{Client, Event, Position, StartOptions};
use std::time::Duration;

// Au démarrage de Studio (quelques centaines de millisecondes : sur un fil de fond).
let detection = Detector::from_env().detect();
let rust = detection.language("rust").unwrap();
println!("Rust : {}", rust.label()); // « rust-analyzer 1.99.0 » ou « coloration seulement »

if let Some(server) = &rust.server {
    let (client, events) = Client::start(&server.command, &dossier, StartOptions::default())?;
    let langage = registry::language_for_path(&fichier).unwrap().lsp_language_id(&fichier);
    client.did_open(&fichier, langage, &texte)?;
    // Réponse attendue sur un fil de fond, avec une limite de temps :
    let survol = client.hover(&fichier, Position::new(3, 7))?.wait(Duration::from_secs(5))?;
    // Diagnostics, messages, fin du serveur : sur le canal `events`.
    for e in events.try_iter() {
        if let Event::Diagnostics { path, diagnostics, .. } = e { /* panneau */ }
    }
    client.shutdown(Duration::from_secs(5));
}
```

## Client (`client.rs`)

| Élément | Rôle |
|---|---|
| `Client::start(commande, dossier, options)` | lance le serveur (sans fenêtre de console sous Windows ; un `.cmd` d'npm fonctionne), fait `initialize` puis `initialized` ; renvoie le client et le `Receiver<Event>` |
| `capabilities()` | ce que le serveur annonce : nom, version, `position_encoding`, `sync` (`None`/`Full`/`Incremental`), `open_close`, `save`, `hover`, `completion` (+ caractères déclencheurs), `definition`, `references`, `rename`, `formatting`, et la réponse brute |
| `did_open(chemin, languageId, texte)` | ouvre (version 1) ; rouvrir un document ouvert envoie son texte comme modification |
| `did_change(chemin, texte)` | texte entier (toujours valable) ; la version est incrémentée par le client |
| `did_change_incremental(chemin, &[TextEdit])` | plages, appliquées l'une après l'autre (règle du protocole) ; seulement si `sync == Incremental` |
| `did_save(chemin, Some(texte))`, `did_close(chemin)` | le texte n'est envoyé que si le serveur le demande |
| `hover`, `completion`, `definition`, `references`, `rename`, `formatting` | requêtes typées → `Pending<Option<Hover>>`, `Pending<Completions>`, `Pending<Vec<Location>>`, `Pending<WorkspaceEdit>`, `Pending<Vec<TextEdit>>` ; `Error::Unsupported` si le serveur n'annonce pas la fonction |
| `request(méthode, params)`, `notify(méthode, params)` | toute autre méthode (`Json` brut) |
| `shutdown(délai)` | arrêt propre : `shutdown`, `exit`, attente ; le processus est tué (avec ses enfants) s'il ne s'arrête pas ; renvoie le code de sortie |
| `version(chemin)`, `is_running()`, `process_id()`, `position_encoding()` | état |

**`Pending<T>`** : `wait(délai)` (après le délai, la requête est annulée par
`$/cancelRequest` et `Error::Timeout` est renvoyée), `try_take()` (sans attendre, pour
une boucle d'interface), `cancel()`, `id()`. **Abandonner un `Pending` annule la
requête** : un survol devenu inutile n'occupe plus le serveur.

**`Event`** : `Diagnostics { uri, path, version, diagnostics }`, `Log`, `Message`,
`Notification` (autres, `$/progress`…), `Stderr` (lignes d'erreur du serveur),
`Malformed` (message illisible, le flux continue), `Exited { code, expected }` (fin du
serveur ; `expected` après `shutdown` ou l'abandon du client : sinon, c'est un plantage).

**`Error`** : `Spawn`, `Io`, `Response { code, message, data }` (dont
`codes::REQUEST_CANCELLED`, `codes::CONTENT_MODIFIED`), `Timeout`, `ServerExited`,
`Closed`, `Unsupported`, `Protocol`, `NotOpen`.

Comportements :

- **Positions** : le client propose UTF-8 et UTF-16, le serveur choisit
  (`position_encoding()`) ; `offset_to_position` / `position_to_offset` convertissent
  avec les octets de l'éditeur (fins de ligne `\n`, `\r\n`, `\r`).
- **Requêtes du serveur**, toutes répondues pour qu'aucun serveur n'attende :
  `workspace/configuration` (depuis `StartOptions::configuration`, sections pointées
  suivies membre par membre), `workspace/workspaceFolders`, enregistrements de capacités et
  rafraîchissements (`null`), `window/showMessageRequest` (sans choix, et signalé en
  `Event::Message`), `workspace/applyEdit` (refusé), le reste par `MethodNotFound`.
- **Plantage** : toutes les requêtes en attente échouent (`ServerExited`), `Exited` est
  émis avec le code de sortie, rien n'attend indéfiniment.
- **Abandon du client** sans `shutdown` : `exit` est envoyé, le processus est tué après
  une seconde, sans bloquer l'appelant.
- **Windows** : arrêt de toute l'arborescence (`taskkill /T`), sinon le serveur lancé par
  un `.cmd` (node…) survivrait à `cmd.exe`.

## Types (`types.rs`)

`Position`, `Range`, `Location` (+ `path()`), `Diagnostic` (`Severity`), `TextEdit`,
`Hover` (Markdown ou texte, toutes les formes du protocole), `CompletionItem`
(`kind_name()`, `text()`, `snippet`), `Completions`, `WorkspaceEdit` (`files` par document,
`documentChanges` et `changes`), `FormattingOptions`.

Fonctions : `apply_edits(texte, &edits, encodage)` (refuse des modifications qui se
chevauchent), `offset_to_position`, `position_to_offset`, `path_to_uri`
(`C:\a b\x.rs` → `file:///C:/a%20b/x.rs`, chemins UNC), `uri_to_path` (lettre de lecteur
en majuscule).

`json.rs` : valeur `Json`, analyse (profondeur limitée) et sérialisation ;
`rpc.rs` : en-têtes `Content-Length`.

## Registre et détection (`registry.rs`)

| Langage | Extensions | Serveurs, par préférence |
|---|---|---|
| Rust | `rs` | rust-analyzer |
| C, C++ | `c h` ; `cpp cc cxx c++ hpp hh hxx h++` | clangd |
| Python | `py pyi pyw` | Pyright (`pyright-langserver`, `basedpyright-langserver`), pylsp, `python -m pylsp` |
| JavaScript, TypeScript | `js mjs cjs jsx` ; `ts mts cts tsx` | typescript-language-server |
| HTML, CSS, JSON | `html htm` ; `css scss less` ; `json jsonc` | serveurs de VS Code (`vscode-langservers-extracted`) |
| C# | `cs` | csharp-ls, OmniSharp |
| SQL | `sql` | sqls, sql-language-server |

SDK (identifiants = noms des exécutables, ceux de `studio/templates`) : `nxc`, `nxpm`,
`python`, `node`, `gcc`, `g++`, `clang`, `clang++`, `cl`, `cargo`, `rustc`, `dotnet`.

`Detector::from_env()` cherche dans le `PATH` puis dans les dossiers usuels : NEXUL
(`%LOCALAPPDATA%\Programs\NEXUL\bin` et `llvm\bin`), LLVM, Node, .NET, npm, Cargo, outils
.NET, Go, `~/.local/bin`. Sous Windows, il essaie les extensions `exe`, `cmd`, `bat` et
`com`. Chaque candidat est **vérifié** :

- une commande de version, qui doit réussir (un proxy rustup sans son composant est
  écarté) ;
- ou une bannière (`cl.exe`, cherché aussi dans les installations de Visual Studio via
  `vswhere`) ;
- ou sa simple présence, pour les serveurs qui n'ont pas d'option de version.

Chaque sonde a une limite de temps (10 s), au-delà de laquelle le processus est tué. Les
**alias du Microsoft Store pour Python** ne sont jamais exécutés, car ils ouvrent le
Store. `detect()` fait tout en parallèle (0,5 s sur le PC de développement).

`LanguageStatus::label()` donne « rust-analyzer 1.99.0 » ou **« coloration seulement »** ;
`install_hints()` propose la commande d'installation (jamais exécutée par Studio).
`DetectedServer::command` est prêt pour `Client::start`.

### Pont vers `studio/templates` (Codex Luna)

`studio/templates` ne détecte rien lui-même ; Studio lui fournit les exécutables :

```rust
struct Sdks(nexul_studio_lsp::Detection);

impl nexul_studio_templates::ToolchainProvider for Sdks {
    fn executable(&self, tool: Tool) -> Option<PathBuf> {
        let id = match tool {
            Tool::Nxc => "nxc", Tool::Nxpm => "nxpm", Tool::Python => "python", Tool::Node => "node",
            Tool::Gcc => "gcc", Tool::Gxx => "g++", Tool::Clang => "clang", Tool::Clangxx => "clang++",
            Tool::Cl => "cl", Tool::Cargo => "cargo", Tool::Rustc => "rustc", Tool::Dotnet => "dotnet",
        };
        self.0.sdk(id).map(|s| s.path.clone())
    }
}
```

`cl.exe` trouvé hors d'une invite développeur a besoin de l'environnement de Visual
Studio (`vcvars64.bat`) pour compiler.

## Tests

```
cargo test --manifest-path studio/lsp/Cargo.toml
```

- **16 unitaires** : JSON (valeurs, Unicode et paires de substitution, erreurs, profondeur,
  nombres), transport (longueur en octets, en-têtes, corps illisible sans perte de
  synchronisation), positions UTF-8/16/32 et fins de ligne, `apply_edits`, URI Windows et
  UNC, lecture des résultats du protocole, catalogue, numéros de version.
- **10 contre le faux serveur** (`src/bin/fake_server.rs`, qui tient le texte des documents
  et répond depuis lui) : session complète (capacités, `initialize`, configuration,
  ouverture, diagnostics, survol, complétion, modifications incrémentales enchaînées et
  complètes, définition, références, renommage, formatage, enregistrement, fermeture,
  arrêt) ; positions UTF-8 négociées ; délai, annulation (explicite, par délai, par
  abandon) et interrogation sans blocage ; plantage du serveur ; message illisible et
  `stderr` ; erreurs (programme absent, `initialize` sans réponse, synchronisation
  complète, méthode inconnue) ; abandon du client ; 8 fils × 50 requêtes simultanées ;
  serveur lancé par un `.cmd` ; arrêt de toute l'arborescence d'un serveur lancé par un
  `.cmd`.
- **5 de détection** : dossier d'outils factices (versions, proxy en échec, présence
  seule, bannière de `cl`, SDK), sonde bloquée tuée, alias du Store ignorés, ordre de
  recherche et extensions, détection réelle de l'ordinateur (affichée).
- **1 contre de vrais serveurs** (`tests/real_servers.rs`) : rust-analyzer sur un petit
  projet Cargo (diagnostic E0061, survol, définition, références, complétion, renommage,
  formatage par rustfmt, arrêt). Il est ignoré, avec un message, si rust-analyzer n'est
  pas installé.

## Limites

- Fonctions LSP couvertes par des méthodes typées : celles de la liste ci-dessus. Les
  autres (actions de code, aide à la signature, symboles, jetons sémantiques…) passent par
  `request` en `Json` brut.
- L'enregistrement dynamique de capacités est accepté mais ignoré (le client annonce
  `dynamicRegistration: false`).
- `workspace/applyEdit` (modifications demandées par le serveur) est refusé.
- Sous Linux et macOS, tuer un serveur ne tue pas ses enfants (pas de groupe de
  processus) ; ces systèmes n'ont pas été testés ici.
- Seul rust-analyzer a été essayé en vrai ; les autres serveurs du catalogue ne sont
  vérifiés que par la détection.
