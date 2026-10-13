# 0007 — Identificateurs ASCII et caractères bidirectionnels interdits

**Contexte.** Les identificateurs Unicode permettent des attaques par homoglyphes
(`а` cyrillique contre `a` latin) ; les caractères de contrôle bidirectionnels permettent
d'afficher un code différent de celui qui est compilé (CVE-2021-42574).

**Décision.** En 0.1, les identificateurs sont ASCII ; les caractères bidirectionnels
sont interdits partout, y compris dans les commentaires et les chaînes (où ils restent
écrivables par échappement `\u{...}`). Les chaînes et commentaires acceptent tout Unicode.

**Conséquences.** Code relu = code compilé. Les diagnostics remplacent tout caractère
invisible par `�` dans les extraits affichés.

**Alternatives.** Identificateurs Unicode normalisés avec détection des confusables
(prévu, plus complexe) ; aucune protection (risque de sécurité).
