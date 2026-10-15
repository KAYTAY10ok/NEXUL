# NEXUL WebAssembly navigateur — contrat v1

Date : 15 octobre 2026. Responsable : Codex Astra. Implémentation sur `refonte/codex-core`, proposée à intégration ; absente de l'installateur publié v0.2.0. Tests automatiques Node en debug et release, exemple exécuté dans le navigateur Codex/Chromium sous Windows. Pas de validation Safari/Firefox à ce stade.

## Construction et fichiers

Depuis la racine du dépôt, construire le nouveau compilateur puis l'exemple :

```powershell
cargo build --release -p nxc
.\target\release\nxc.exe check examples/web/calcul.nx --target wasm32-web --json
.\target\release\nxc.exe build examples/web/calcul.nx --target wasm32-web --lib --release -o examples/web/calcul.wasm
python -I examples/web/serve.py
```

Ouvrir `http://127.0.0.1:8766/examples/web/` : résultat attendu `42` puis `Bonjour, NEXUL 🌍 — été !`. Le serveur est lié à loopback et fixe les types MIME `.mjs` et `.wasm`, indépendamment du registre Windows. Il sert le dépôt pour le développement uniquement.

- Cible : `wasm32-web`, alias LLVM exact `wasm32-unknown-unknown` ; conditions `@cfg("web")`, `@cfg("wasm")`, `@cfg("ptr32")`. `wasi`, `bare` et `unix` sont faux.
- `--lib` produit un module sans point d'entrée, avec ses fonctions `@export` ; nom par défaut `<source>.wasm`.
- Sans `--lib`, `fn main()` est requis et l'export `_start` est appelé explicitement par `run()` du chargeur, une seule fois.
- `--release`, `-O0` à `-O3`, `-Os`, `-Oz`, `--no-debug` restent disponibles. `--emit llvm-ir` produit du texte LLVM. Aucune activation de fast-math.
- `nxc run/test --target wasm32-web` est refusé avec un message orientant vers le chargeur. Les tests web sont pilotés par JavaScript.
- LLVM-C et wasm-ld sont requis pour construire. Rust n'est pas requis pour exécuter le module. Aucun WASI, runtime C ou traduction en JavaScript.

## Diagnostics destinés aux IDE

`check --json` écrit les diagnostics sur stdout : 0 si l'analyse réussit, 1 sinon. `build` doit être lancé séparément et son statut vérifié : ajouter `--json` à build arrête le pilote après l'analyse, sans produire de binaire. Les erreurs de génération/link sont textuelles sur stderr. Le schéma JSON existant n'est pas encore versionné ; aucun changement incompatible dans ce lot. Détecter la cible avec `nxc targets`, pas avec le seul numéro de version 0.2.0.

## ABI v1 : scalaires, octets et exports

`@export("nom")` donne le nom JavaScript d'une fonction. `@link("app") extern "C" fn host_double(x: f64) -> f64` importe `app.host_double`. Les imports externes sont des opérations `unsafe` côté NEXUL ; l'application fournit explicitement leurs implémentations au chargeur.

| NEXUL | WebAssembly / JavaScript |
|---|---|
| i32/u32, usize/isize, Ptr[T] | i32 / Number ; pointeur et longueur interprétés non signés |
| i64/u64 | i64 / BigInt ; interpréter les bits non signés avec BigInt.asUintN si nécessaire |
| f32 | f32 / Number arrondi en simple précision |
| f64 | f64 / Number |
| aucun retour | void / undefined |

Rester sur ces types pour l'ABI publique v1. String, Vec, dyn, agrégats et fermetures ne doivent pas être exposés ; leurs représentations ne sont pas un contrat web. Ne pas utiliser les pointeurs de fonctions comme callbacks JavaScript : importer une fonction hôte nommée.

Exports runtime automatiquement ajoutés aux modules web :

| Export | Contrat |
|---|---|
| memory | mémoire linéaire privée, non partagée |
| nx_abi_version() -> i32 | retourne 1 |
| nx_alloc(len: usize) -> Ptr[u8] | allocation alignée à 16 octets ; octets non initialisés ; panique en cas d'échec |
| nx_free(p: Ptr[u8]) | libère une allocation une seule fois ; null accepté par l'ABI brute |
| nx_flush() | vide stdout |

La pile est de 8 Mio, comme WASI. L'allocateur NEXUL utilise memory.grow et réemploie les régions libérées ; la mémoire ne rétrécit pas. Toute vue JavaScript antérieure à memory.grow doit être recréée.

## Chargeur `lib/web/nexul.mjs`

`await loadNexul(source, options)` accepte une URL/string HTTP, un BufferSource ou un WebAssembly.Module. La récupération utilise fetch puis arrayBuffer ; pas de dépendance au streaming MIME Wasm. Les modules JS doivent néanmoins être servis avec un type MIME JavaScript. Héberger via HTTP(S), pas file://. La politique CSP de l'hôte doit autoriser ses modules et WebAssembly.

- `call(nom, ...arguments)` appelle un export, puis vide stdout. Les noms nx_alloc/nx_free/_start sont réservés aux méthodes dédiées.
- `alloc(taille)` / `free(p)` suivent les allocations de l'hôte et refusent les doubles libérations ou les pointeurs non possédés. Éviter qu'une fonction NEXUL libère ces buffers elle-même.
- `writeBytes(p, Uint8Array)` copie dans un buffer possédé, avec vérification de capacité.
- `readBytes(p, n)` copie les octets ; `readText(p, n)` décode UTF-8 strictement. La vérification de bornes mémoire ne prouve pas que le pointeur est un objet vivant : l'appelant reste responsable de sa durée de vie.
- `withText(texte, callback)` alloue et copie les octets UTF-8 (sans terminateur zéro), appelle le callback **synchrone**, puis libère. La longueur compte les octets, pas les caractères. Le callback ne doit pas retenir le pointeur. Les NUL intégrés sont conservés.
- `run()` lance `_start` ; une sortie normale est absorbée. Une seconde exécution exige une nouvelle instance.
- `memory` et `module` permettent inspection et interop avancée ; ils ne constituent pas une protection contre un hôte malveillant.

Exemple d'aller-retour : `examples/web/calcul.nx` reçoit le nom via pointeur/longueur, construit un String NEXUL, puis appelle `app.host_text` avec des octets **empruntés pour la durée du callback**. Le callback les copie avec readText avant de retourner. Le String est ensuite détruit par NEXUL.

## Imports runtime fournis par le chargeur

Espace de noms réservé `nexul_v1` :

| Fonction | Comportement |
|---|---|
| nx_write(fd, p, len) -> i32 | fd 1/2, sortie UTF-8 ; 0 succès, 1 descripteur inconnu |
| nx_exit(code) -> Never | lève NexulExit(code) ; les paniques utilisent 101 |
| nx_now_ns(monotonic: i32) -> i64 | horloge hôte en nanosecondes, sans garantie de résolution nanoseconde |
| nx_random(p, len) -> i32 | crypto.getRandomValues par blocs ≤64 Kio ; 0 succès, 1 indisponible |

Options : `imports` pour les modules applicatifs, `write(fd, texte)` pour l'affichage, `nowNs(bool) -> BigInt` et `randomBytes(Uint8Array)` pour un hôte contrôlé/testable (false signale l'indisponibilité). Les callbacks sont synchrones, ne réentrent pas dans le module et ne retiennent pas les vues mémoire. La sortie par défaut passe à console.log/error par fragments ; l'intégrateur peut accumuler les fragments.

Une panique, un trap Wasm ou une exception d'import hôte peut interrompre NEXUL sans dérouler les destructeurs et sans restaurer sa pile. Le chargeur marque l'instance terminée et refuse de nouveaux appels. **Jeter l'instance entière**, ne pas tenter de récupérer ses buffers. Aucune promesse de récupération après panique.

## Limites explicites

Exécution mono-thread, synchrone. Aucun DOM implicite, async/await NEXUL, threads, mémoire partagée ou WASI. Les arguments sont vides, l'environnement absent, stdin est à EOF, les fichiers renvoient une erreur. Le fuseau est UTC. Un sommeil bloquant non nul panique ; organiser la temporisation côté hôte. L'aléa sécurisé échoue si crypto n'est pas disponible. Les imports applicatifs doivent fournir le DOM/réseau de manière explicite. Les entrées et pointeurs du code unsafe restent sous la responsabilité du développeur.

## Validation et intégration Studio

```text
cargo test --release -p nxc --test web -- --ignored --nocapture
```

Le test construit l'exemple et un programme de validation en debug/release, puis vérifie sous Node : imports absents, ABI, scalaires f32/f64 (NaN, infinis, zéro signé, sous-normal), BigInt, UTF-8/NUL/grande chaîne, croissance et réutilisation mémoire, propriété des buffers, erreurs, réentrance, horloge et aléa injectés, fichiers indisponibles, panic/overflow/trap et sortie de main. L'exemple a aussi été vérifié dans Chromium. La conformité native Windows, Linux/WSL et WASI reste testée séparément.

Claude Studio peut intégrer cette interface après intégration du lot Core : vérifier la cible, analyser avec check, construire avec build, servir les ressources et ouvrir l'aperçu isolé. Codex Luna peut proposer un modèle basé sur examples/web en conservant le chargeur et les chemins relatifs. Aucun changement d'interface graphique réalisé par Astra.

Références de conception : [API JavaScript WebAssembly](https://www.w3.org/TR/wasm-js-api-2/), [linker WebAssembly LLD](https://lld.llvm.org/WebAssembly.html).
