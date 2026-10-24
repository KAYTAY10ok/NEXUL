# C9 — réduire les copies de l'état de propriété

Statut : CONÇU, revue Claude requise avant modification de ownership.rs.
Aucune règle ni ligne de ce fichier n'est modifiée dans cette PR.

## Mesure de départ

Base a824c76, compilateur Rust release avec LTO=false ; Linux x86-64 partagé.
20 000 if indépendants : 8,765 / 4,927 / 5,307 s, sorties sans diagnostic.
50 000 bras : processus tué par SIGKILL après 27,126 s ; cause du signal non établie.
Ni « 50 000 bras < 2 s » ni « 20 000 if < 1 s » ne sont atteints.
Données brutes : benchmarks/sol/c9-analysis-baseline-2026-10-24.json.
Des compilations ont aussi tourné pendant ces relevés : référence locale exploratoire,
pas comparaison contrôlée de machines ou preuve de régression depuis les anciens rapports.

Reproduction des sources (édition 0.2, puis `nxc check`, délai maximal 60 s) :

```python
ifs = 'fn main() {\n let x = 3\n' + ''.join(
    f'if x == {i} {{ println({i}) }}\n' for i in range(20000)) + '}\n'
arms = 'fn main() {\n let x = 3\n match x {\n' + ''.join(
    f'{i} => println({i})\n' for i in range(50000)) + '_ => {}\n}\n}\n'
```

## Constat dans les sources

BitSet possède un Vec<u64> dense. State contient maybe_init, maybe_uninit et moves.
solve clone l'état d'entrée à chaque visite et lors de la découverte d'un successeur ;
check le clone à nouveau. Les temporaires de bras augmentent la largeur de chaque état.
L'ordre inverse postordre du lot 5 évite déjà les visites inutiles : ne pas le changer.

## Proposition à relire

Introduire un bitset persistant par pages, sans changer Effects, State::apply, join,
les sites de déplacement ou le calcul des drapeaux de destruction. Un arbre radix de
pages partagé permet un clone O(1), une mise à jour sur O(log pages) nœuds et des unions
qui ignorent les sous-arbres identiques. Les bits non initialisés à l'entrée peuvent être
représentés par un défaut 1 dans un domaine fini, au lieu de matérialiser toutes les pages.
Les sites de moves ont un défaut 0. Les itérateurs restent en ordre croissant des indices.

Un simple Arc<Vec<u64>> avec copy-on-write copie encore tout le vecteur à la première
écriture de chaque bloc : insuffisant. Un BTreeMap copié en entier pour chaque état peut
également redevenir quadratique. Évaluer la représentation sur les formes dense et creuse.
Une limitation aux seules variables vivantes est une autre option, mais demande une preuve
séparée que les temporaires ressources et les informations de diagnostic restent suivis.

## Conditions avant adoption

- Oracle de bitset dense : insert/remove/contains/union/itération identiques sur un corpus
  déterministe, notamment pages limites et domaines non multiples de 64.
- États de boucles, branches, réinitialisation et déplacements partiels : mêmes diagnostics,
  spans et ordre des notes. Même élaboration des destructions et aucun DropIfInit résiduel.
- Corpus UI complet identique à l'octet et exécution O0/O2, mutations robustesse ; pile 256 Mio.
- IR identique avant/après sur toutes les cibles (K3.7, K3.8) ; aucune annotation d'alias.
- Mesures contrôlées avec médiane/dispersion et mémoire ; critères C9 toujours à démontrer.

Demande à Claude : approuver la représentation et ces obligations avant le code, ou
privilégier une réduction de domaine accompagnée de sa preuve de conservation des états.
