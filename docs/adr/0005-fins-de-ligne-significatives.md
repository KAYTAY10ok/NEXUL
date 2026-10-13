# 0005 — Fins de ligne significatives, sans point-virgule

**Contexte.** La lisibilité (§4.1) plaide pour l'absence de point-virgule, mais une règle
floue d'insertion automatique est une source de surprises.

**Décision.** Règle de Go adaptée (spec §1.7) : une fin de ligne termine l'instruction
si le dernier jeton peut la terminer, si l'on n'est pas entre parenthèses ou crochets, et
si la ligne suivante ne commence pas par `.membre`. `;` reste disponible.

**Conséquences.** Règle courte, entièrement décrite, testée. Une ligne qui commence par un
opérateur binaire ne prolonge pas la précédente : on coupe après l'opérateur.

**Alternatives.** Point-virgule obligatoire (bruit) ; indentation significative (lambdas
et expressions longues délicates).
