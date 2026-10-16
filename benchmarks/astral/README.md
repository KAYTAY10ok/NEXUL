# Benchmarks natifs NEXUL / C / Rust / C#

Ce banc est la base P13 demandée pour le commit NEXUL figé dans chaque rapport. Il compare quatre programmes compilés qui exécutent les mêmes charges, vérifient leurs sorties et enregistrent les mesures brutes :

- boucles et calcul scientifique en `f64` (18 résultats : séries, intégrales, approximations de `e`, `sin(1)` et `cos(1)`, oscillateur) ;
- vecteur de `n` entiers `u64` parcouru et transformé par la même matrice 3×3 ;
- `n` allocations distinctes de petits blocs de quatre entiers, lus puis libérés ou rendus récupérables ;
- construction puis lecture d'une chaîne ASCII de `n` caractères, avec checksum et longueur vérifiés.

Les tailles par défaut sont 1, 10 et 100 millions d'itérations. Chaque exécutable fait dix passes de chauffe limitées à 10 000 éléments, puis une passe mesurée. Le pilote lance chaque couple charge/taille/langage cinq fois par défaut et publie médiane, écart-type et chaque échantillon. Le temps interne exclut le lancement du processus et l'impression. Le rapport contient aussi les temps muraux de compilation, leur pic mémoire, le pic du working set de chaque processus de mesure et les sorties validées.

## Modes de compilation

- C : Clang `-O2 -ffp-contract=off -fno-fast-math` avec l'éditeur de liens MSVC ;
- Rust : `rustc --edition=2021 -C opt-level=2`, cible `x86_64-pc-windows-msvc` ;
- C# : .NET `Release`, avec compilation à plusieurs niveaux et PGO activés ;
- NEXUL : `nxc build --release`, compilateur construit depuis le commit de la racine.

Le commit NEXUL est figé dans le JSON/Markdown généré. Le banc ne modifie pas le compilateur. Il requiert Windows x64, les outils C++ de Visual Studio, Clang/LLVM et LLD, Rust MSVC, .NET 10, Python 3.11+ et le runtime LLVM nécessaire à `nxc`.

## Exécution

Depuis la racine du dépôt, construire d'abord le compilateur du checkout, puis lancer le pilote :

```powershell
cargo build --release -p nxc
python benchmarks/astral/run.py
```

Pour réduire ou remplacer la matrice de mesures :

```powershell
python benchmarks/astral/run.py --sizes 1000000 10000000 --repetitions 3
```

Les programmes sont compilés sous `benchmarks/astral/build/` (ignoré par Git). Le rapport courant est écrit dans `benchmarks/astral/results/latest.json` et `latest.md` ; ces sorties sont aussi ignorées afin de ne pas écraser un relevé publié. Le relevé complet du commit `c40acbd` est dans [results/2026-10-16.md](results/2026-10-16.md), avec ses données brutes [JSON](results/2026-10-16.json). Les fichiers `2026-10-15.*` sont le relevé préliminaire à une seule charge et sont conservés comme historique, sans comparaison directe avec le nouveau banc.

## Limites d'interprétation

Le pic mémoire est le `PeakWorkingSetSize` fourni par Windows après la sortie du processus ; il inclut le runtime de chaque langage et peut varier selon le ramasse-miettes et l'encodage. C, NEXUL et Rust stockent ici la chaîne en octets UTF-8/ASCII, tandis que `string.Create` en C# utilise UTF-16. Les checksums rendent les résultats observables pour les optimiseurs. Pour les allocations C#, la récupération mémoire est contrôlée par le GC, donc le nombre d'objets alloués n'implique pas un `free` synchrone comme en C.

Ces résultats décrivent une machine, un système et ce corpus seulement. Ils ne permettent pas de conclure sur les performances générales, les autres plateformes, le temps réel ou la précision correctement arrondie de toute la bibliothèque mathématique.
