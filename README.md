# NEXUL — Next Universal Language

*One Language. Every Possibility.*

NEXUL est un langage de programmation compilé, sûr et lisible, conçu pour couvrir tous les
domaines du logiciel : outils système, serveurs, jeux, embarqué, calcul scientifique. Ce
dépôt contient la **version 0.2** : la spécification du langage, le compilateur natif
`nxc`, la bibliothèque standard et le runtime (écrits en NEXUL), le gestionnaire de
paquets `nxpm`, l'environnement de développement **NEXUL Studio** et le programme
d'installation pour Windows.

```nexul
struct Article {
    nom: String
    prix: f64
    quantite: u32
}

fn total(stock: Vec[Article]) -> f64 {
    var somme = 0.0
    for a in stock {
        somme += a.prix * a.quantite as f64
    }
    return somme
}

fn main() {
    var stock: Vec[Article] = Vec.new()
    stock.push(Article(nom: "clavier", prix: 49.90, quantite: 3))
    stock.push(Article(nom: "écran", prix: 189.0, quantite: 1))
    println("Valeur du stock : {total(stock):.2} €")
}
```

## Installer

**Windows.** Téléchargez `NEXUL-Setup.exe` depuis la page
[Releases](https://github.com/KAYTAY10ok/NEXUL/releases) et lancez-le : un assistant installe, pour votre compte et sans
droits d'administrateur, le compilateur, la chaîne LLVM, NEXUL Studio, le débogueur, les
exemples et la documentation. Il ajoute `nxc` et `nxpm` au PATH, crée les raccourcis et
associe les fichiers `.nx` à Studio. S'il trouve une version précédente de NEXUL, il vous
demande pour chacune de la **supprimer** ou de la **sauvegarder** (déplacée hors du PATH,
sans raccourci ni association : elle ne gêne pas la nouvelle version). Voir
[docs/installation.md](docs/installation.md), qui décrit aussi l'installation sans fenêtre.

**Depuis les sources** (Windows, Linux, macOS) : il faut [Rust](https://rustup.rs) et
[LLVM](https://llvm.org) 17 ou plus récent avec LLD (sur Windows, l'installateur officiel
de LLVM fournit `LLVM-C.dll` et `lld`).

```bash
cargo build --release
```

```bash
./target/release/nxc run examples/bonjour.nx
```

`nxc` trouve LLVM dans la variable `NEXUL_LLVM`, dans un dossier `llvm` installé à côté
de lui, puis dans les emplacements habituels (`C:\Program Files\LLVM`, `/usr/lib/llvm-*`,
Homebrew) et le `PATH`.

## Ce qui distingue NEXUL

- **Sûreté mémoire sans ramasse-miettes ni annotations de durée de vie.** NEXUL adopte la
  *sémantique de valeurs mutables* : chaque valeur a un propriétaire unique, les emprunts
  ne vivent que le temps d'un appel, d'une boucle ou d'un `match`, et la loi d'exclusivité
  interdit toute modification concurrente. Pas d'utilisation après libération, pas de
  double libération, pas de référence pendante — vérifié à la compilation
  ([spec, chapitre 05](docs/spec/05-memoire.md)), pour le code sans `unsafe` et sous les
  conditions de l'[ADR 0011](docs/adr/0011-perimetre-des-garanties.md) (pile suffisante,
  frontière C respectée).
- **Destruction déterministe (RAII)** dans un ordre spécifié, avec drapeaux de destruction
  générés automatiquement pour les déplacements conditionnels.
- **Abstractions sans coût caché** : traits et contraintes génériques (`[T: Ord]`),
  répartition dynamique (`dyn Forme`), `@derive(Eq, Ord, Clone, Display, Hash, Default)`,
  opérateurs sur les types utilisateur, fonctions anonymes non échappantes
  (`v.filter(fn(x) => x > 0)`) sûres sans durée de vie, `Map` et `Set`.
- **Aucun comportement surprenant** : pas de conversion numérique implicite, débordements
  détectés dans tous les modes (`+%` pour l'arithmétique modulaire explicite), indices
  vérifiés, conversions `as` vérifiées, pas de valeur nulle (`Option[T]`).
- **Diagnostics pédagogiques** : codes stables, extraits de source soulignés, suggestions
  (« did you mean `println`? »), aucune erreur en cascade, `nxc explain E0402`.
- **Sécurité dès le lexer** : caractères bidirectionnels « Trojan Source » interdits,
  identificateurs ASCII (pas d'homoglyphes).
- **Un compilateur autonome** : `nxc` produit lui-même l'IR LLVM, l'optimise et la
  compile en code machine, puis lie avec LLD. Aucun compilateur C n'intervient : le
  runtime (allocation, formatage des nombres, mathématiques, flottants logiciels,
  démarrage sans système) est écrit en NEXUL.
- **Toutes les machines** : une même commande produit des exécutables pour Windows,
  Linux et macOS (x86-64, ARM64, RISC-V), des modules WebAssembly (WASI) et des
  micrologiciels pour microcontrôleurs ARM Cortex-M et RISC-V, sans système
  d'exploitation (`nxc targets`).
- **Des outils complets** : formateur (`nxc fmt`), tests (`nxc test`), services d'éditeur
  (`nxc ide`), paquets avec versions figées et vérification SHA-256 (`nxpm`), débogueur
  intégré à NEXUL Studio.

## Commandes

| Commande | Effet |
|---|---|
| `nxc run prog.nx [-- args]` | compile et exécute |
| `nxc build prog.nx [--release] [-o sortie]` | produit un exécutable |
| `nxc build prog.nx --target aarch64-linux` | compilation croisée (`nxc targets` donne la liste) |
| `nxc build prog.nx --lib` / `--staticlib` | bibliothèque partagée ou statique des fonctions `@export` |
| `nxc check prog.nx` | vérifie sans générer de code |
| `nxc test prog.nx [nom...]` | exécute les fonctions `@test` |
| `nxc fmt prog.nx [--check]` | met le fichier en forme |
| `nxc explain E0402` | explique un code de diagnostic |
| `nxpm new projet` | crée un projet (`nexul.toml`, `src/main.nx`) |
| `nxpm add lib --git <url> --tag v1.0` | ajoute une dépendance (Git ou `--path`) |
| `nxpm build` / `run` / `test` | compile le projet avec ses dépendances |

Le [guide de démarrage](docs/guide.md) présente le langage pas à pas.

## NEXUL Studio

L'environnement de développement officiel ([studio/](studio/README.md)) : éditeur avec
coloration, complétion, documentation au survol, aller à la définition, références,
renommage, plan du fichier et mise en forme fournis par le compilateur ; erreurs
soulignées en direct ; exécution, tests et compilation croisée ; débogueur (points
d'arrêt, pas à pas, variables, pile d'appels) ; gestion des paquets ; aperçu web en
direct ; extensions en JavaScript isolées et soumises à permissions
([API des extensions](docs/studio/extensions.md)).

## Organisation du dépôt

| Chemin | Contenu |
|---|---|
| [`docs/cahier-des-charges.md`](docs/cahier-des-charges.md) | le cahier des charges fondateur |
| [`docs/spec/`](docs/spec/README.md) | la spécification de NEXUL |
| [`docs/guide.md`](docs/guide.md) | guide de démarrage |
| [`docs/roadmap.md`](docs/roadmap.md) | état d'avancement et feuille de route |
| [`docs/adr/`](docs/adr/README.md) | décisions d'architecture |
| `compiler/nxc_syntax` | sources, diagnostics, lexer, parser, AST |
| `compiler/nxc_sema` | modules, résolution, traits, types, inférence, exhaustivité, `@derive`, cibles |
| `compiler/nxc_nir` | représentation intermédiaire NIR, analyse de propriété, destructions |
| `compiler/nxc_codegen_llvm` | monomorphisation, génération d'IR LLVM, informations de débogage |
| `compiler/nxc` | `nxc` et `nxpm` : pilote, LLVM, édition de liens, services d'éditeur, paquets, suite de conformité |
| `lib/std/` | bibliothèque standard et runtime, écrits en NEXUL (`lib/std/sys/` : Windows, Linux, WASI, sans système) |
| `studio/` | NEXUL Studio (serveur Rust et interface web) et ses extensions d'exemple |
| `installer/` | le programme d'installation Windows (`NEXUL Setup.exe`) |
| `tools/` | construction de l'installateur, exécution WASI, scripts de génération |
| `tests/run/`, `tests/ui/`, `tests/numerics/` | programmes de conformité, diagnostics attendus, validation numérique |
| `examples/` | programmes d'exemple |

## Qualité

```bash
cargo test --release
```

La suite compile et exécute chaque programme de `tests/run/` avec un **détecteur de
fuites** (toute allocation non libérée fait échouer le test), compare chaque diagnostic de
`tests/ui/` à sa sortie attendue, vérifie que chaque code d'erreur est documenté et que
chaque exemple de la documentation compile, et compile les exemples. Les mêmes programmes
s'exécutent sur d'autres cibles (`NEXUL_TEST_TARGETS=x86_64-linux,wasm32-wasi`, Linux via
WSL, WebAssembly via Node, microcontrôleurs via QEMU). Le formatage et l'analyse des
flottants et la bibliothèque mathématique sont validés contre Python, et les flottants
logiciels bit à bit contre le processeur. Le compilateur n'a **aucune dépendance Rust
externe** et la génération de code est reproductible.

## Construire l'installateur

```bash
python -I tools/build_installer.py
```

compile l'ensemble en mode release, rassemble les composants (avec LLVM, LLD, LLDB et un
moteur Python pour le débogueur) et produit `NEXUL Setup.exe` sur le Bureau
(voir [docs/installation.md](docs/installation.md)).

## État

NEXUL 0.2 est utilisable pour des programmes complets en ligne de commande, des
bibliothèques, WebAssembly et des micrologiciels. La concurrence, le réseau,
l'interopérabilité Python et la compilation séparée des modules sont les prochaines
étapes de la [feuille de route](docs/roadmap.md).

## Licence

MIT ou Apache-2.0, au choix ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
