# Campagne de performance

La campagne T1 compare des programmes NEXUL, C et Rust sur les mêmes charges. Elle est
**en cours** : les résultats de ce banc ne constituent pas encore une preuve que NEXUL
atteint 1,1 fois le temps de C. Le relevé historique de Main est dans
[main/2026-10-28-linux.md](main/2026-10-28-linux.md) ; il utilise le meilleur de cinq
exécutions et n'est donc pas directement comparable aux médianes ci-dessous. Le banc
historique Windows et ses quatre charges sont conservés dans [astral/](astral/README.md).

## Mesurer sous Linux

Installer Python 3, un compilateur C compatible avec les options ci-dessous, Rust et une
version release de `nxc` construite depuis le commit étudié. Depuis la racine :

```sh
cargo build --release
python3 benchmarks/terra/run.py --output /tmp/nexul-bench-results
```

`--nxc`, `--cc` et `--rustc` permettent de choisir les exécutables ; `--workloads nom ...`
sélectionne les charges. Par défaut, le pilote découvre chaque fichier `.nx` de
`benchmarks/terra/` et exige un triplet de sources `<nom>.nx`, `<nom>.c`, `<nom>.rs`.
Chaque programme s'exécute sans argument et doit écrire une sortie déterministe identique
pour les trois langages. Une compilation échouée, une sortie vide ou différente arrête le
pilote sans écrire de nouveau rapport. Il refuse un dossier contenant déjà `latest.json`
ou `latest.md` : choisir un nouveau dossier pour préserver les mesures précédentes.

Le pilote compile séparément dans un répertoire temporaire avec `nxc build --release`,
`cc -std=c17 -O2 -ffp-contract=off -fno-fast-math -pthread` et `rustc --edition=2021 -C opt-level=2`.
Il exécute chaque programme sept fois, en alternant l'ordre des langages. Il mesure le
temps mural complet avec l'horloge monotone de Python : démarrage, calcul et sortie sont
inclus. La médiane et le ratio NEXUL/C figurent dans `latest.md` ; `latest.json` conserve
les sept échantillons, la sortie vérifiée, les empreintes des sources, les commandes, le
commit, l'état de l'arbre et les versions des outils. Les chemins des binaires temporaires
dans les commandes enregistrées indiquent les options utilisées ; ils ne subsistent pas
après l'exécution. Le dossier de sortie par défaut est `benchmarks/terra/results/` ; il
contient des données générées locales, à ne publier qu'avec leur contexte de mesure.

La comparaison de sorties détecte des divergences de résultat, mais ne prouve pas à elle
seule l'équivalence des opérations ou l'absence d'optimisation excessive. Vérifier les
sources et le coût relatif du démarrage, utiliser une machine aussi stable que possible et
conserver les données brutes. Les rapports de profil et les comparaisons avant/après
restent à produire. Un résultat isolé ne représente ni toutes les machines ni toutes les
charges. Le workflow Linux publie le tableau et les journaux en artefact sans bloquer la CI ;
son exécution GitHub Actions reste à valider.

## Charges et premier relevé

Douze triplets sont présents : recherche binaire, canal borné, entrées-sorties fichier,
flottants, table de hachage, calcul entier, matrice, tri fusion, balayage de source,
construction de chaîne, arbre indexé et vecteurs. Le canal compare les API réelles, dont
les implémentations diffèrent. Le balayage compte des mots ASCII dans `lib/std/io.nx` ;
il ne mesure pas le lexer complet. Les entrées-sorties utilisent `/tmp`, le cache du
système et aucune synchronisation sur disque ; le temps inclut la création et le nettoyage.
Les optimisations, vérifications et allocations peuvent différer entre langages.

Le [relevé Linux du 29 octobre](rapports/2026-10-29-linux.md) et ses
[échantillons bruts](rapports/2026-10-29-linux.json) sont exploratoires : machine partagée,
arbre de travail modifié, sans profilage et avec démarrage du processus inclus. Les écarts
ne constituent ni une preuve de leur cause ni une acceptation par Main. La dépendance du
balayage à `lib/std/io.nx` est identifiée par son empreinte dans le JSON.

| Charge | C (ms) | Rust (ms) | NEXUL (ms) | NEXUL / C |
|---|---:|---:|---:|---:|
| Recherche binaire | 101,207 | 94,601 | 98,610 | 0,974 |
| Canaux | 27,566 | 12,858 | 24,390 | 0,885 |
| Fichiers | 4,097 | 6,203 | 14,644 | 3,575 |
| Flottants | 19,543 | 20,258 | 19,359 | 0,991 |
| Table de hachage | 10,636 | 11,040 | 12,941 | 1,217 |
| Entiers | 99,569 | 94,325 | 99,527 | 1,000 |
| Matrice | 48,968 | 28,978 | 87,645 | 1,790 |
| Tri | 12,244 | 12,335 | 14,915 | 1,218 |
| Balayage de source | 3,509 | 4,083 | 20,914 | 5,960 |
| Chaîne | 7,762 | 9,898 | 9,648 | 1,243 |
| Arbre | 23,817 | 30,445 | 26,075 | 1,095 |
| Vecteurs | 16,975 | 17,549 | 24,881 | 1,466 |
