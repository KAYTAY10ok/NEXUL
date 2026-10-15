# Comparatif scientifique natif NEXUL / C / Rust / C#

Ce banc compare le même calcul séquentiel en double précision dans quatre langages.
Il reprend les accumulateurs du programme scientifique NEXUL 0.1 fourni avec la tâche,
avec un nombre d'itérations paramétrable. Les variantes renvoient les mêmes 18 valeurs,
dont des séries, des intégrales numériques, des approximations de `e`, `sin(1)` et
`cos(1)`, et une simulation d'oscillateur.
La somme de Taylor de `sin(1)` inclut son terme initial `1`; il manquait dans le fichier
d'origine fourni, ce qui faisait mesurer et afficher une valeur décalée de `-1`.

Les configurations sont compilées en mode optimisé, et les temps de compilation sont
rapportés séparément des temps de calcul. Chaque processus exécute dix petites passes de
chauffe, puis mesure une passe; le pilote relance cinq processus par langage et taille,
puis rapporte médiane et dispersion. Le temps mesuré est interne au programme : il exclut
le lancement du processus et l'impression. Les tailles prévues sont 1M, 10M et 100M.

Le C utilise Clang `-O2 -ffp-contract=off`, Rust `-C opt-level=2` sur la cible x86-64 générique,
NEXUL `nxc build --release`, et C# le JIT .NET en configuration Release avec PGO.
Ce comparatif est une mesure d'une charge scientifique séquentielle sur une machine; il
ne certifie ni les performances générales du langage, ni le temps réel, ni l'exactitude
correctement arrondie de toutes les fonctions mathématiques.

## Exécution

```powershell
python benchmarks/astral/run.py
```

Le pilote détecte les compilateurs, construit dans `benchmarks/astral/build/`, vérifie
les résultats numériques entre langages avant de publier `benchmarks/astral/results/latest.json`
et le tableau Markdown `benchmarks/astral/results/latest.md`. Les exécutables locaux sont
ignorés par Git. Le rapport conserve les versions des compilateurs, la CPU, les arguments,
les mesures individuelles, la médiane et l'écart-type.

Les paramètres de l'exécution peuvent être remplacés :

```powershell
python benchmarks/astral/run.py --sizes 1000000 10000000 100000000 --repetitions 5
```

Les résultats validés du 15 octobre 2026 sont dans [results/2026-10-15.md](results/2026-10-15.md),
avec les mesures brutes dans [results/2026-10-15.json](results/2026-10-15.json). Sur cette
charge seulement, NEXUL est proche de C et plus rapide que Rust et C# sur la machine mesurée.
Une exécution, une machine et un seul profil de calcul ne permettent pas de conclure sur les
performances générales; répéter sur d'autres charges et cibles avant toute conclusion produit.
