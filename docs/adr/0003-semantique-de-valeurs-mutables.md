# 0003 — Modèle mémoire : sémantique de valeurs mutables

**Contexte.** Le cahier des charges exige une mémoire sûre, déterministe, sans
ramasse-miettes obligatoire (§5.1), lisible par des débutants (§1.3).

**Décision.** NEXUL adopte la sémantique de valeurs mutables : propriétaire unique,
déplacements explicites dans leur effet, références de **seconde classe** (paramètres,
éléments de boucle, liaisons de motifs) qui ne peuvent jamais être stockées ni renvoyées,
et loi d'exclusivité vérifiée localement. Voir [spec chapitre 05](../spec/05-memoire.md).

**Conséquences.**
- Sûreté mémoire sans annotations de durée de vie ni comptage de références.
- Règles locales : chaque fonction se vérifie seule ; les messages restent simples.
- Destruction déterministe (RAII) avec drapeaux de destruction calculés par flot de
  données sur la NIR.
- Limite : pas de références stockées. Les graphes utilisent des indices, des arènes ou
  du code `unsafe` encapsulé. Des projections plus riches sont étudiées pour la suite.

**Alternatives.** Emprunts de Rust avec durées de vie (expressif mais difficile
d'accès) ; comptage de références de Swift (coût caché, cycles) ; ramasse-miettes
(interdit par le cahier des charges pour le natif).
