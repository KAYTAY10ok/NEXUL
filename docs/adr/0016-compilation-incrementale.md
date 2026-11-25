# 0016 — Compilation incrémentale et cache de la bibliothèque standard

Auteur : Codex Terra 6.1. **Statut : proposée**, le 29/10/2026 ; accord de Claude Main
requis avant implémentation. Cette proposition ne change ni la syntaxe ni les garanties du
langage et ne revendique aucun gain mesuré pour une implémentation qui n'existe pas encore.
Elle répond au critère de temps de compilation incrémentale de la 0.6.0.

## Problème et état actuel

`compile_source` analyse le programme entier et abaisse tous les corps en NIR, y compris
pour `nxc check`. Le chargement analyse la racine, précharge `std.core`, `std.string`,
`std.vec`, `std.mem` et `std.rt`, puis suit les imports ; il applique `@cfg` et les
expansions aux modules chargés. Le cache actuel dans `compiler/nxc/src/lib.rs` est un
cache de **résultat final** : il intervient après la génération de l'IR du programme entier
et évite LLVM et l'édition de liens si sa clé correspond. Une modification de l'IR du
programme manque donc ce cache. `LinkJob` accepte déjà une liste d'objets, mais `build`
n'en émet actuellement qu'un.

Les symboles de fonctions internes comportent un numéro attribué à la découverte ; le
même compteur sert notamment aux constantes, positions, statiques et vtables. Sur un petit
programme temporaire compilé en O0 avec `--emit llvm-ir --no-debug`, ajouter un appel à
une autre fonction avant `foo` change son symbole de `main.foo#304` à `main.foo#305`.
Même une fonction inchangée peut donc produire une IR différente. La source du générateur,
`compiler/nxc_codegen_llvm/src/lib.rs`, confirme le compteur partagé ; cet exemple ne
mesure pas l'impact sur un grand programme.

Sur la machine Linux x86-64 partagée du 29/10, sept lancements de `nxc check` sur
`fn main() {}` avec cache isolé donnent 54,299 ; 65,819 ; 48,392 ; 47,483 ; 47,412 ;
48,449 ; 52,315 ms, soit une médiane de **48,449 ms**. Cette mesure comprend tout le
processus et ne démontre pas quelle fraction provient de `std`. Il faut instrumenter ou
comparer les étapes avant d'attribuer le coût. Le LSP a déjà un cache d'analyse pour
certaines requêtes identiques ; la publication de diagnostics passe aussi par
`compile_source`. Latence CLI et latence LSP doivent être mesurées séparément.

## Solutions envisagées

| Option | Avantage | Limite |
|---|---|---|
| Cache du programme entier actuel | Simple, bon pour une entrée exactement identique | Une petite modification relance LLVM et le lien |
| Un objet par module source | Nombre d'unités limité, frontières lisibles | Un changement invalide tout le module ; les instances génériques peuvent être utilisées ailleurs |
| **Groupes déterministes de fonctions et unité commune** | Limite l'invalidation après modification d'une fonction ; taille ajustable | Nommage, dépendances, visibilité et ownership des données partagées à définir |
| Un objet par fonction sans regroupement | Invalidation fine | Beaucoup d'objets et coût de lien élevé ; les entités partagées demandent aussi un propriétaire |

## Décision proposée

### 1. Unités de génération et responsabilités

En **O0**, après les passes frontales et NIR du programme entier, découvrir les instances
atteignables et construire un plan déterministe. Regrouper les fonctions par module
propriétaire et par groupes de taille bornée, selon une règle stable fondée sur l'identité
des fonctions et non sur leur ordre de découverte. L'algorithme exact et le nombre de
fonctions par groupe restent à choisir et mesurer : une partition qui décale tous les
groupes à chaque insertion ne satisfait pas l'objectif. Une unité commune possède les
racines du programme et les entités dont l'unicité est nécessaire ; les autres unités
contiennent les définitions de leurs instances, leurs constantes privées et les
**déclarations** nécessaires aux références entre unités.

Les instances génériques, glue de destruction, vtables, statiques, chaînes, helpers,
wrappers `@export`, entrée, `@test`, interruptions et racines runtime doivent avoir une
règle de propriété explicite et déterministe. Les données modifiables et les objets dont
l'identité est observable ne doivent pas être dupliqués. Les constantes ne peuvent être
dupliquées que si leur identité et leur sémantique le permettent. Une instance ou une
référence ne peut avoir qu'une définition liée. Les fonctions aujourd'hui `internal` mais
référencées entre unités auront un lien et une visibilité compatibles avec cette
référence, sans ajouter d'exports publics. Chaque unité LLVM doit être vérifiable seule ;
les tables, attributs ABI, layout de types, debug et déclarations doivent rester cohérents.
Le plan doit aussi couvrir les cibles hébergées, WASI/web, bare, bibliothèques et ABI C ;
aucune cible n'est considérée prise en charge avant validation.

La monomorphisation reste globale pour décider des instances et de leurs propriétaires :
on ne suppose pas qu'un module source soit fermé sur ses génériques. Une modification de
signature, type, layout, constante, `@cfg`, implantation de trait ou racine peut changer
plusieurs unités : leur IR et leur clé changent. La modification d'un corps local sans
effet sur les autres unités devrait en revanche laisser leurs IR identiques ; c'est un
objectif à tester, pas une garantie déjà établie.

### 2. Noms de symboles stables

Construire pour chaque entité un identifiant canonique à partir de son genre, chemin de
paquet et de module, nom qualifié et, pour une instance, représentation canonique des
arguments de type et de son implantation. Définir séparément les identités des fonctions
synthétiques, closures, glue et données, à partir de leur propriétaire ; un index local
stable peut être utilisé quand il n'existe pas de nom explicite. L'identité doit
distinguer deux paquets ou deux types ayant le même nom affiché. Ne pas employer `FnId`,
`Ty`, les adresses, l'ordre d'itération d'une table ou le compteur global comme identité
persistante : leur stabilité entre compilations n'est pas assurée. L'encodage doit être
non ambigu, déterministe et compatible avec les formats d'objets et le débogueur ; si un
hachage raccourcit un nom, prévoir une stratégie de détection des collisions.

Les symboles explicitement exportés et leurs wrappers conservent leur contrat ABI. Les
noms et métadonnées privées de chaque unité, dont littéraux et positions, doivent eux aussi
être déterministes : stabiliser seulement les noms de fonctions ne suffit pas à stabiliser
l'IR. Insérer une fonction sans lien dans un autre groupe ne doit pas renommer les
symboles existants ; les changements d'identité causés par une modification à l'intérieur
d'un propriétaire doivent être documentés et testés.

### 3. Cache des objets par unité

Pour chaque unité, calculer une clé sur une représentation non ambiguë comprenant au moins
l'**empreinte de son IR complète** et un domaine/version du format de cache. Ajouter
l'identité de la construction du compilateur et de LLVM, la cible et son triple, CPU et
features, niveau d'optimisation, PIC, code model, options et métadonnées de débogage,
ainsi que toute autre option affectant l'objet. Ne pas supposer que la version du paquet
seule identifie une construction. Proposition : SHA-256 existant dans `nxc::pkg`, sans
nouvelle crate ; valider son utilisation et une identité fiable de LLVM avec Main. La
représentation de debug, notamment les chemins et positions, fait partie de l'IR ou de la
clé ; on ne la retire pas pour obtenir artificiellement un cache hit.

Au hit, réutiliser l'objet vérifié ; sinon, compiler l'IR, publier l'objet et son manifeste
par écriture temporaire puis renommage atomique. Prévoir concurrence entre processus,
troncature ou corruption, interruptions, version inconnue, échec d'écriture et nettoyage.
En cas de doute, ignorer l'entrée et reconstruire ; aucun hit ne doit masquer une erreur.
L'invalidation de l'objet ne remplace pas la vérification des entrées du lien : relier à
chaque construction avec la liste d'objets déterministe, les imports, exports, bibliothèques
et options du lien courants. Les sorties auxiliaires et contraintes des formats `.o`,
`.obj`, archives, DLL/PDB, Mach-O et WASM restent à tester. Respecter `NEXUL_NO_CACHE` et
le répertoire de cache isolable ; l'ancien cache final reste indépendant tant que sa
coexistence n'a pas été vérifiée.

### 4. Optimisation et inlining

Activer le découpage incrémental seulement pour une compilation **O0**. À O1/O2/O3/Os/Oz,
conserver le module unique actuel, afin de conserver les possibilités d'optimisation et
d'inlining entre fonctions. Le comportement exact lorsque `--release` et `-O0` sont
combinés doit être déterminé selon le niveau LLVM effectif et les autres effets de
`release`/`@cfg`, pas selon le seul nom de l'option. O0 perd l'accès à l'inlining entre
unités ; mesurer l'effet à l'exécution et le coût de lien. Aucune modification des
vérifications de sûreté ni des options d'optimisation n'est autorisée par cette ADR.

## Cache de la bibliothèque standard analysée

Le cache d'objets ne réduit pas l'analyse de `nxc check`. Proposer en premier une interface
de cache dans le chargeur, puis mesurer séparément : analyse lexicale/syntaxique de `std`,
expansion et configuration, analyse sémantique, NIR/ownership et rendu des diagnostics.
Un cache syntaxique peut être une étape ; il ne doit pas être présenté comme un cache de
`std` **analysée** ni comme une solution complète au critère avant mesure.

Pour un cache d'analyse réutilisable, définir un instantané immuable de `std` indépendant
du programme utilisateur, ou un mécanisme explicite de remappage des identifiants et
références lors de sa réintégration. Les `FileId`, `Span`, `ModId`, `FnId`, types,
`SourceMap`, positions, provenance et diagnostics ne peuvent pas être supposés valides
dans une autre compilation. Les modules réellement chargés, leurs sources incluses, édition,
cible, `@cfg` (dont test/release), version et construction du compilateur, format du cache
et toutes dépendances influant l'analyse entrent dans l'identité ou provoquent une
invalidation conservatrice. Les imports du projet, dépendances de paquet et textes non
enregistrés du LSP restent propres à chaque analyse ; ils ne doivent jamais recevoir une
vue périmée. Les erreurs et avertissements, leur ordre, les chemins et positions ainsi que
l'ownership doivent correspondre à une analyse sans cache. Ne partager aucun état mutable
entre analyses concurrentes sans protocole vérifié.

Le cache mémoire du processus LSP et un cache persistant pour les invocations CLI sont
deux usages distincts : définir leur durée de vie, invalidation, schéma et comportement en
cas d'entrée illisible ou ancienne. Si la réutilisation sémantique indépendante ne peut pas
être prouvée, limiter l'implémentation à l'étape sûre et documenter le coût restant. Les
données chargées depuis un cache ne sont jamais une source d'autorité sur la correction du
programme : toute incompatibilité déclenche une analyse normale.

## Validation et mesure avant livraison

1. **Correctness** : comparer compilation complète monolithique et incrémentale O0 sur
   `tests/run` : code de sortie, stdout/stderr, diagnostics, paniques et positions, effets
   observables et compteur de fuites. Tester également échecs et diagnostics frontaux,
   génériques, méthodes de traits, closures, destructeurs, statiques, vtables, exports C,
   exceptions d'ABI, imports, `@cfg`, debug, cibles et bibliothèques. Une égalité binaire
   octet pour octet n'est pas exigée si les objets et métadonnées diffèrent ; l'équivalence
   observable et les symboles/exportations exigés le sont.
2. **Invalidation** : essais chaud/froid et sans cache, modifications successives d'un corps,
   signature, type, constante, trait, dépendance et fichier non enregistré ; comparer à une
   reconstruction complète. Vérifier source, options, LLVM, compilateur, cible et versions
   différentes ; collisions, fichier corrompu, publication interrompue et compilations
   simultanées ne doivent jamais réutiliser un objet incorrect.
3. **Temps** : générer un programme déterministe d'environ 20 000 lignes avec des fonctions
   effectivement atteignables, et un programme qui importe et utilise les parties
   applicables de `lib/`. Décrire la génération et les fonctions modifiées. Mesurer temps
   mural de compilation complète froide et après modification d'un corps, avec et sans
   cache, puis ventilation front-end, émission LLVM et lien ; médiane et échantillons de
   plusieurs répétitions, version/outils, machine, réglages et état du cache publiés.
   Vérifier le résultat fonctionnel à chaque scénario. Un simple cache hit sans modification
   ne prouve pas le critère. `lib/` n'est pas nécessairement compilable comme une unité
   autonome : définir le pilote et les modules réellement exercés, et signaler les manques.
4. **LSP** : mesurer séparément la latence des diagnostics après modification d'un document
   et des requêtes successives, avec texte non enregistré, imports et invalidation ;
   distinguer temps du compilateur, délai de debounce et transport. Comparer intégralement
   diagnostics et positions, y compris éditions et cibles différentes.
5. **Performance d'exécution** : sur les mêmes programmes, comparer O0 monolithique et
   découpé ; vérifier que le chemin optimisé est inchangé. Publier les chiffres et limites.

## Périmètre et demandes à Claude Main

Implémentation seulement après accord de Main. Chemins prévus : fonctions de construction
et cache dans `compiler/nxc/src/lib.rs` et `link.rs`, nouveau
`compiler/nxc_codegen_llvm/src/units.rs`, et uniquement nommage/répartition des unités
dans `compiler/nxc_codegen_llvm/src/lib.rs` et `func.rs`, annoncés avant modification.
L'interface nécessaire au cache de `std` touche vraisemblablement
`compiler/nxc_sema/src/load.rs` et peut exiger d'autres types ou passes de `nxc_sema` et
`nxc_syntax`, dont la propriété appartient à Main. **Demande explicite** : décider si Main
fournit cette interface, ou donner un mandat écrit et délimité avant que Terra modifie ces
fichiers. L'approbation de cette ADR ne constitue pas à elle seule une autorisation de
modifier les fichiers réservés.

Décisions demandées : stratégie de groupes et entités partagées ; représentation canonique
des identités ; cache `std` syntaxique puis/ou sémantique et propriétaire de son interface ;
identité de LLVM et format de cache ; matrice initiale de cibles et critères de publication.
Aucune de ces décisions n'est supposée acceptée par ce document.
