# Tests manuels de fumée

Ces tests complètent la suite de développement, ils ne la remplacent pas. Le lanceur appelle un compilateur installé et compare stdout à une référence. Les programmes tests/run/control_flow.nx et closures.nx sont lus tels quels ; ils ne sont ni copiés ni modifiés. hello_unicode.nx appartient à cette suite.

## Lancer

Depuis la racine du dépôt :

~~~powershell
python tests/manual/run_smoke.py
~~~

Si nxc n’est pas dans PATH :

~~~powershell
python tests/manual/run_smoke.py --nxc C:\chemin\vers\nxc.exe
~~~

Le script respecte également NEXUL_NXC. Chaque cas est lancé avec nxc run depuis la racine du dépôt ; stdout et code de sortie sont comparés. Le script ne modifie ni les tests sources ni les réglages de Studio. Utiliser un checkout de test : les tests/run peuvent changer au fil des révisions.

## Cas inclus

- hello_unicode : lancement minimal et chaîne Unicode.
- control_flow : fonctions, boucles, match, options et sortie déterministe.
- closures : fonctions anonymes, captures et collections.

Pour élargir la suite, choisir des tests/run sans stdin ni écriture de fichiers, puis comparer leur sortie à leur fichier .out. Ne pas utiliser NEXUL_BLESS ; ce lanceur n’a aucune commande de mise à jour des références.

## Premier audit

Non exécuté : nxc, Cargo et rustc étaient absents de l’environnement. Seule la syntaxe du script est vérifiée.
