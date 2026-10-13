# 0006 — Débordements arithmétiques toujours vérifiés

**Contexte.** Les débordements silencieux sont une source majeure de failles (§11.1).
Rust ne les vérifie qu'en mode debug ; C les rend indéfinis.

**Décision.** `+ - * / %`, la négation, les décalages et les conversions `as` sont vérifiés
dans **tous** les modes ; l'arithmétique modulaire s'écrit explicitement `+% -% *%`.

**Conséquences.** Un débordement provoque une panique localisée, jamais un résultat faux.
Le coût mesuré est faible (crible d'Ératosthène : environ 15 % d'écart avec le C non
vérifié, en comptant aussi les vérifications de bornes).

**Alternatives.** Vérification en debug seulement (comportement différent selon le mode) ;
enroulement silencieux (bogues invisibles).
