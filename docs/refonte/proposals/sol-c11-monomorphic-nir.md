# C11 — passe NIR monomorphe indépendante de LLVM

Statut : CONÇU. Proposition pour revue Claude avant tout code, selon le mandat C11.
Aucune crate, règle sémantique, NIR ou génération de code n'est modifiée ici.

## Résultat visé et frontière

`Program + NIR générique + racines + Target -> MonoProgram -> backend LLVM`.
MonoProgram ne contient ni texte LLVM, ni registres, ni disposition machine des agrégats.
Un second backend et SH5 peuvent le consommer sans dépendance à nxc_codegen_llvm.
Le typage, les règles d'exclusivité et l'élaboration des destructions restent en amont.

Données proposées :

- InstanceKey = (FnId, arguments Ty concrets), identifiants symboliques stables.
- MonoBody : paramètres, locaux et opérations avec substitution de tous les types ;
  appels statiques et références de fonctions résolus vers InstanceKey.
- DynTablePlan = (interface, type concret, liste ordonnée des méthodes concrètes,
  destructeur et besoins symboliques de taille/alignement). Les offsets, symboles et
  objets LLVM restent au backend. Préserver exactement l'ordre des slots actuel.
- MonoProgram : instances, tables dyn, racines et dépendances symboliques du runtime.
  Un ordre de découverte explicite remplace toute itération non triée de hash map.

Les bodies sont déjà élaborés : aucun DropIfInit ne peut apparaître, aucune destruction
n'est ajoutée ou retirée. Les blocs inaccessibles ne découvrent pas de nouvelles instances.
Les instances ne conservent pas de Param/Infer/IntVar/FloatVar non résolu.

## Piège principal pour K3.8

Gen utilise aujourd'hui un compteur partagé pour instances, chaînes, positions, types
et auxiliaires. Une passe qui nommerait avidement toutes les fonctions changerait les
symboles et donc l'IR, même si le code machine restait identique. Trier globalement la
nouvelle file ne convient pas non plus : le générateur actuel utilise une pile LIFO.

La passe mono doit donc produire des clés symboliques, sans attribuer les noms LLVM.
Pendant la première extraction, le backend conserve le moment d'allocation des symboles
et l'ordre d'émission existants. Il demande un MonoBody déjà substitué à la passe et
émet uniquement les instances réellement demandées. Une planification conservatrice peut
contenir plus de bodies, mais ils ne doivent pas être émis : pas de code mort supplémentaire.
La petite file d'émission qui reste dans LLVM ne fait plus de substitution ni de résolution
de dyn ; elle ne gère que placement et nommage. Une normalisation ultérieure des noms
serait une tâche distincte, après levée explicite du critère byte-identique.

## Dépendances à couvrir

- Racines main/tests et @export, fermetures et environnements, méthodes génériques,
  impl par défaut, conversions vers dyn et destructeurs de ressources génériques.
- Fonctions @lang appelées par les vérifications et points d'entrée spécifiques à la cible,
  y compris C1/C2/C5. La liste exacte dépend des opérations réellement générées.
- Fonctions externes : conserver conventions et validations C4 de Claude ; aucune nouvelle
  annotation d'alias, aucun changement des conversions char/bool ou types contraints.
- Reste au backend : LayoutCx, LLVM C ABI lowering, shims machine, symboles et debug info.
  Les signatures sémantiques concrètes appartiennent à MonoProgram ; les signatures
  machine appartiennent au backend.

## Extraction en étapes courtes après approbation

1. Capturer la référence d'IR complète avec options et racines identiques, avant extraction.
2. Introduire les clés et l'inventaire mono sans changer découverte ou émission ; comparer.
3. Déplacer la substitution de bodies et les décisions dyn dans la passe indépendante ;
   laisser l'adaptateur de nommage paresseux reproduire les événements existants.
4. Déplacer le driver vers le pipeline explicite ; tester MonoProgram sans charger LLVM.
5. Retirer les anciennes tables de monomorphisation du backend uniquement après équivalence.

Pas de dépendance ou changement du Cargo.toml racine avant accord du responsable transversal.
Une implantation dans nxc_nir::mono évite d'ajouter une crate lors de la première extraction ;
Claude doit confirmer cette frontière, notamment pour SH5.

## Porte de sortie obligatoire

IR identique à l'octet sur tests/run, tests/web, exemples et bibliothèques exportées ;
O0/O2, debug info activée/désactivée, toutes les cibles connues. Exécuter les deux compilateurs
sur les mêmes chemins de sources/options pour éviter de fausses différences de debug info.
La comparaison est sans normalisation de noms ou métadonnées et sans tolérance d'écart.
Vérifier aussi imports/exports/liens, diagnostics, tests d'exécution et robustesse. Les
consommateurs de compile_source utilisent une pile de 256 Mio.

Cette PR ne prétend pas avoir exécuté cette future matrice avant/après : aucun code n'est
extrait et il n'existe pas encore de candidat. Vérification actuelle : lecture des points
Gen::instance, fn_queue, lang_fn, vtables et du compteur commun ; git diff --check réussi.

Décisions demandées à Claude : frontière nxc_nir::mono, planification symbolique et
adaptateur de nommage paresseux, couverture exacte des racines et obligations d'équivalence.
