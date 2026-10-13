# NEXUL — Next Universal Language

*One Language. Every Possibility.*

NEXUL est un langage de programmation compilé, sûr et lisible, conçu pour couvrir à terme
tous les domaines du logiciel : outils système, serveurs, jeux, embarqué, calcul
scientifique. Ce dépôt contient la **version 0.1 « Noyau »** : la spécification formelle
du langage, le compilateur `nxc`, la bibliothèque standard et la suite de conformité.

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

## Ce qui distingue NEXUL

- **Sûreté mémoire sans ramasse-miettes ni annotations de durée de vie.** NEXUL adopte la
  *sémantique de valeurs mutables* : chaque valeur a un propriétaire unique, les emprunts
  ne vivent que le temps d'un appel, d'une boucle ou d'un `match`, et la loi d'exclusivité
  interdit toute modification concurrente. Pas d'utilisation après libération, pas de
  double libération, pas de référence pendante — vérifié à la compilation
  ([spec, chapitre 05](docs/spec/05-memoire.md)).
- **Destruction déterministe (RAII)** dans un ordre spécifié, avec drapeaux de destruction
  générés automatiquement pour les déplacements conditionnels.
- **Aucun comportement surprenant** : pas de conversion numérique implicite, débordements
  détectés dans tous les modes (`+%` pour l'arithmétique modulaire explicite), indices
  vérifiés, conversions `as` vérifiées, pas de valeur nulle (`Option[T]`).
- **Diagnostics pédagogiques** : codes stables, extraits de source soulignés, suggestions
  (« did you mean `println`? »), aucune erreur en cascade, `nxc explain E0402`.
- **Sécurité dès le lexer** : caractères bidirectionnels « Trojan Source » interdits,
  identificateurs ASCII (pas d'homoglyphes).
- **Compilation native et croisée** : `nxc` génère du C11 portable compilé par Zig/Clang ;
  une même commande produit des exécutables Windows, Linux, macOS, ARM64, RISC-V et
  WebAssembly.
- **Performance** : sur un crible d'Ératosthène (10 millions), NEXUL s'exécute à ~15 % du C
  optimisé **avec toutes les vérifications de sûreté actives**.

## Démarrage rapide

Prérequis : [Rust](https://rustup.rs) (pour construire le compilateur) et
[Zig](https://ziglang.org) (testé avec la version 0.17), qui sert de compilateur C et d'éditeur de liens.

```bash
cargo build --release
```

```bash
./target/release/nxc run examples/bonjour.nx
```

Commandes principales :

| Commande | Effet |
|---|---|
| `nxc run prog.nx [-- args]` | compile et exécute |
| `nxc build prog.nx [--release] [-o sortie]` | produit un exécutable |
| `nxc build prog.nx --target aarch64-linux-gnu` | compilation croisée |
| `nxc check prog.nx` | vérifie sans générer de code |
| `nxc test prog.nx` | exécute les fonctions `@test` |
| `nxc explain E0402` | explique un code de diagnostic |

Le [guide de démarrage](docs/guide.md) présente le langage pas à pas.

## Organisation du dépôt

| Chemin | Contenu |
|---|---|
| [`docs/cahier-des-charges.md`](docs/cahier-des-charges.md) | le cahier des charges fondateur |
| [`docs/spec/`](docs/spec/README.md) | la spécification formelle de NEXUL 0.1 |
| [`docs/guide.md`](docs/guide.md) | guide de démarrage |
| [`docs/roadmap.md`](docs/roadmap.md) | état d'avancement et feuille de route |
| [`docs/adr/`](docs/adr/) | décisions d'architecture |
| `compiler/nxc_syntax` | sources, diagnostics, lexer, parser, AST |
| `compiler/nxc_sema` | modules, résolution, types, inférence, exhaustivité, règles de places |
| `compiler/nxc_nir` | représentation intermédiaire NIR, analyse de propriété, destructions |
| `compiler/nxc_codegen_c` | monomorphisation et génération de C11 |
| `compiler/nxc` | le driver `nxc` et la suite de conformité |
| `runtime/nexul_rt.h` | runtime C minimal (embarqué dans `nxc`) |
| `lib/std/` | bibliothèque standard, écrite en NEXUL |
| `tests/run/`, `tests/ui/` | programmes de conformité et diagnostics attendus |
| `examples/` | programmes d'exemple |

## Qualité

```bash
cargo test
```

La suite compile et exécute chaque programme de `tests/run/` avec un **détecteur de
fuites** (toute allocation non libérée fait échouer le test), compare chaque diagnostic de
`tests/ui/` à sa sortie attendue, vérifie que chaque code d'erreur est documenté, et
compile les exemples. Le compilateur n'a **aucune dépendance externe** (exigence §16.1 du
cahier des charges), et la génération de code est reproductible à l'octet près.

## État

NEXUL 0.1 est un noyau complet et cohérent, pas encore un langage de production. Les
traits, les closures, la concurrence, les collections associatives, le gestionnaire de
paquets et les autres éléments du cahier des charges sont planifiés dans la
[feuille de route](docs/roadmap.md).

## Licence

MIT ou Apache-2.0, au choix.
