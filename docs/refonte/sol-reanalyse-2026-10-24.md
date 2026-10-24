# Réanalyse NEXUL — 24 octobre 2026

État examiné : Core #60 (`7d72bba65c7eb83be52a464be13df20ed5425f43`), puis
intégration de C1/C2/C3/C5/C6/C7/C8/C10 dans la PR #61. Les PR sont encore
distinctes de `main` ; cette branche sert à vérifier leur interaction. Elle ne
remplace ni la revue de sûreté de Claude ni la fusion par l'utilisateur.

## Défauts confirmés à corriger avant validation de sûreté

### P1 — Les exports contournent les invariants des bases distinct char/bool

Dans `compiler/nxc_codegen_llvm/src/func.rs`, branche `TyKind::Adt` du prologue
export (lignes 395–412 sur Core #60), le générateur reçoit la représentation de
base, vérifie éventuellement la contrainte, puis construit l'ADT. Il ne reprend
pas les contrôles bool/char appliqués aux paramètres primitifs.

Le cas `export_distinct.nx` expose `Letter = distinct char is Copy` et
`Flag = distinct bool is Copy` à JavaScript. Observations à O0 **et** O2 :

| Entrée | Résultat observé | Résultat requis |
|---|---|---|
| Letter 0xD800 | 55296, sans panique | Rejet du surrogate |
| Letter 0x110000 | 1114112, sans panique | Rejet hors Unicode |
| Flag 2 | O0 : 0 ; O2 : 2, sans panique | Rejet hors 0/1 |
| Flag 255 | O0 : 1 ; O2 : 255, sans panique | Rejet hors 0/1 |

Les entrées valides U+1F30D, 0 et 1 conservent leur résultat. Les tests officiels
des exports primitifs passent ; ils ne couvraient pas ces enveloppes. Corriger
la validation de base avant le conditionnement de l'ADT, y compris sans clause
`where`. Périmètre Core/C4 de Claude (#60), non modifié par cet audit.

### P1 — Une contrainte f32 diffère entre compilation et exécution

Dans `compiler/nxc_sema/src/check/newtype.rs`, l'évaluation des littéraux
flottants utilise `Num::Float(f64)` sans arrondi à la base f32. Le prédicat
exécuté utilise f32. Le programme suivant est accepté sans erreur :

```nx
// edition: 1
type Cut = f32 where it < 16777217.0
fn main() {
    let Cut value = 16777216.0
    println(Cut.check(value as f32).is_some())
}
```

Il affiche `false` à O0 et O2 : une valeur construite en code sûr échoue à son
propre prédicat. 16777217.0 est arrondi à 16777216.0 en f32. L'évaluation
constante et le prédicat généré doivent partager les règles de précision et
d'arrondi. Périmètre sémantique de Claude (#59), non modifié par cet audit.

### P2 corrigé — Les alias de chemins font disparaître des diagnostics LSP

La CI Windows a reproduit deux échecs sur un chemin `RUNNER~1` : diagnostic
d'import modifié non enregistré vide et diagnostic de manifeste envoyé sur une
autre URI. Le compilateur produit des chemins canoniques ; le serveur utilisait
l'égalité textuelle avec les URI `didOpen`.

La PR #62 associe les diagnostics par identité physique du fichier et conserve
l'URI du client, aussi pour le manifeste non ouvert. Six tests LSP passent sous
Linux, dont une nouvelle régression de protocole avec lien symbolique couvrant
les deux cas. Les tests Windows existants restent actifs ; validation de la
correction par Actions en cours au moment de cette première publication.

## Reproduction indépendante

Depuis la racine du dépôt, avec LLVM/LLD et Node disponibles :

```sh
python3 benchmarks/sol/reanalyse-2026-10-24/reproduce.py \
  --nxc target/release/nxc --output /tmp/nexul-invariants.json
```

Le script construit les deux cas à O0/O2, vérifie aussi les entrées valides et
sort avec le code **1** tant que les défauts persistent. Le relevé
`benchmarks/sol/reanalyse-2026-10-24/results.json` contient 10 violations :
quatre entrées d'export et un désaccord f32 à chaque niveau. Reproduit sur
Core #60 seul et sur l'intégration. Ces sondes ne sont pas masquées dans une
suite verte et ne modifient pas les tests de langage appartenant à Claude.

## Validation réellement exécutée

| Périmètre | Résultat |
|---|---|
| Core #60, workspace Linux hors installateur Windows | 75 tests réussis ; fuzz 40 tours, seed 2410 |
| Core #60, ABI navigateur explicitement `--ignored` | 1 test réussi |
| Intégration C1/C2/C3/C5/C6/C7/C8/C10, même workspace | 78 tests réussis avant ajout du nouveau test LSP |
| Intégration, ABI navigateur explicite | 1 test réussi |
| Intégration, corpus croisé wasm32-wasi | `run_programs_cross` réussi |
| Intégration, QEMU Cortex-M mps2-an386 | O0/O2 réussis : 100000 boucles, interruption SysTick, bilans d'allocations nuls |
| Correctif LSP sur intégration | 6 tests réussis, aucun ignoré |
| Assertions foundations après correction de chemin | 6 tests réussis |

Environnement local : Rust 1.99.1, LLVM 20, LLD 23, Node 24, QEMU 8.2.
Le runtime Rust fourni impose localement `CARGO_PROFILE_RELEASE_LTO=false` et
`CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1` pour éviter ses erreurs de liaison ;
aucun test désactivé. Les jobs Actions utilisent le profil release du dépôt.

## CI appliquée et limites de preuve

L'accord utilisateur pour C6 a été appliqué dans #52. Windows installe LLVM
20.1.8 depuis l'archive officielle, vérifie SHA256 et présence de LLVM-C.dll,
lld-link.exe, wasm-ld.exe. La suite compilateur release complète est lancée
avec `--no-fail-fast`. Les deux systèmes lancent explicitement le test
navigateur auparavant ignoré par défaut.

Contrairement au premier bilan, C7 a déjà été exécuté : run 40124607293,
Linux et Windows réussis. Le mécanisme fonctionne pour les PR empilées.
Les runs C6 40130961066 puis 40131454539 ont installé LLVM et passé Linux ;
Windows a révélé successivement l'assertion de chemin canonique (corrigée
sans retirer la comparaison du chemin complet), puis le défaut LSP ci-dessus.
Le premier run d'intégration 40131661105 reproduit seulement les deux échecs
LSP dans la suite compilateur ; son test `run_programs` réussit.

La relance intègre #62. La branche #61 ajoute les 20 répétitions Windows
du test de registre C8. Elles ne sont pas déclarées réussies avant un journal
les exécutant réellement. La construction/validation graphique de Studio
natif, un vrai lldb-dap et macOS ne sont pas couverts par ces jobs.

## État du projet et ordre conseillé

- Le sommet Core #60 comporte les nouveaux types distincts/contraints et les
  exports vérifiés primitifs, mais les deux cas P1 empêchent une validation
  globale des invariants. Les corriger puis ajouter leurs régressions Core.
- C9 et C11 ont maintenant une revue préalable approuvée dans #53/#54.
  C9 : domaine dense réduit aux locaux pertinents, oracle de domaine complet,
  comparaison diagnostics/IR et mesures RSS/temps. C11 : `nxc_nir::mono`, noms
  différés et inclusion des racines/conversions dyn Error de #56. Ils ne sont
  pas encore implémentés ; les anciens blocages de conception sont levés.
- C5 est vérifié pour les interruptions masquables sur Cortex-M. NMI, fautes
  et multicœur ne sont pas couverts par PRIMASK ; revue Claude toujours requise.
- C10 démontre un gain ASCII mesuré, pas une supériorité générale sur C ni
  un gain vectoriel établi. Les mesures Windows d'origine restent à reproduire.
- C12/macOS reste bloqué : pas de machine de validation, chemins runtime/entrée
  encore partagés avec Linux. Aucun résultat macOS revendiqué.
- `studio/app` est un workspace séparé : les tests racine ne valident pas GPUI.
  Il dépend de syntax/templates/lsp, sans dépendance terminal/dap. Le bouton
  « Sortie » ne prouve pas le raccordement terminal interactif/débogueur.
  Aucun nouveau test graphique ni paquet installable complet effectué ici.
- Les versions restent dispersées : compiler/installateur 0.2.0, application
  native 0.3.0, extension VS Code 0.1.0 ; les PR de distribution sont à intégrer.
- Concurrence, contrats et temps réel restent réservés par E0111 dans le
  parseur édition 1. Aucun compilateur auto-hébergé dans selfhost ou
  compiler/selfhost. La base examinée ne satisfait donc pas encore la mission
  complète de la refonte ; les anciens fichiers d'état ne suffisent pas à la
  déclarer livrée.

Demandes à Claude : revue des deux P1 avec ces reproductions, puis revue de
#62 et de la pile sûreté. Les fusions restent à l'utilisateur.
