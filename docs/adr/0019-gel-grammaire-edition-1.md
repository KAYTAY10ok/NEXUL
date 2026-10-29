# ADR 0019 — Gel de la grammaire de l'édition 1 (G3)

- **Statut** : acceptée (Claude Main, 29/10/2026). C'est l'étape G3 du plan de la syntaxe,
  préalable à l'étape SH3 de l'auto-hébergement (parser écrit en NEXUL, Codex Terra).
  Aucune rupture : la grammaire figée est celle que `nxc` accepte déjà.
- **Grammaire normative** : spécification, chapitre 02, [§2.8](../spec/02-grammaire.md).

## Contexte

L'ADR 0010 a fixé la syntaxe de l'édition 1 avant son implémentation. Depuis :
- l'ADR 0013 a ajouté `unsafe extend` ;
- l'ADR 0014 a ajouté `distinct` et `where` ;
- l'ADR 0015 a ajouté `else` comme repli ;
- l'ADR 0017 a ajouté `@repr("C")` ;
- l'ADR 0018 a donné un sens à `with` et à `every … within …`.

Certaines formes de l'EBNF de l'ADR 0010 ne sont pas implémentées : `pub(package)`, les
interfaces génériques, `it` comme expression, `===`, `spawn`, les contrats. D'autres
existent sans y figurer : `owned self`, `[x; n]`, les blocs de portée, `type N is I = .A | .B`.

Un parser auto-hébergé ne peut pas viser une cible qui bouge (SH0 §5). Il faut une
grammaire écrite, exacte, et une règle qui l'empêche de changer en silence.

## Décision

1. **Référence** : la grammaire normative de l'édition 1 est celle de la spécification,
   chapitre 02, §2.8. Elle décrit ce que `compiler/nxc_syntax/src/parser/ed1.rs` accepte
   au commit qui la publie, rien de plus. L'EBNF de l'ADR 0010 reste l'historique de la
   décision ; en cas d'écart, §2.8 fait foi.
2. **Gel** : à partir de la 0.7.0, la grammaire de l'édition 1 ne change que par une ADR
   acceptée, et seulement de deux façons :
   - **corriger** le parser pour qu'il suive §2.8, quand il s'en écarte (un bogue) ;
   - **ajouter** une forme qui était refusée. Une forme acceptée garde son sens et son
     arbre.
   
   Retirer une forme, ou lui donner un autre sens, demande une nouvelle édition (ADR 0010,
   règle des éditions) et la décision de l'utilisateur.
3. **Formes réservées** : celles que §2.8 liste sous ce titre sont lues puis refusées
   (E0111). Ce sont les seules qui peuvent devenir acceptées sans nouvelle édition :
   - contrats (`requires`, `ensures`) ;
   - autres effets que `noalloc` et `nonblocking` ;
   - `===` et `!==` ;
   - `spawn` ;
   - `it` comme expression ;
   - bornes sur le paramètre d'un `type` ;
   - types `distinct` ou contraints génériques ;
   - type sur la variable d'un `for` qui modifie ou déstructure.
   
   `pub(package)` et les interfaces génériques ne sont **pas** réservés : ils ne sont pas
   reconnus. Les ajouter demande une ADR qui les place d'abord parmi les formes réservées.
4. **Obligations d'un changement** de `parser/ed1.rs` ou des parties de `parser.rs`
   propres à l'édition 1. Dans la même PR :
   - mettre à jour §2.8 ;
   - ajouter un test (`tests/ui` pour un refus, `tests/run` ou l'ensemble des `.nx` du
     dépôt pour une forme acceptée) ;
   - l'inscrire dans `claude-main-status.md`, que Codex Terra lit avant chaque tâche.
   
   Le parser de SH3 suit le même changement dans la PR suivante.
5. **Équivalence pour SH3** : deux parsers sont équivalents s'ils donnent, pour chaque
   fichier `.nx` du dépôt en édition 1, le même arbre (positions comprises) et, pour chaque
   fichier de `tests/ui`, les mêmes codes d'erreur de syntaxe aux mêmes positions. Le
   format de comparaison est la sortie JSON de l'arbre (mandat M-T4a ci-dessous). Ce n'est
   pas une grammaire formelle : un générateur de parser n'est pas exigé.

## Mandat écrit M-T4a (Codex Terra, SH3)

Codex Terra peut ajouter :
- la commande cachée `nxc debug ast --json` dans `compiler/nxc/src/main.rs`, à côté de
  `nxc debug tokens --json` ;
- un nouveau module `compiler/nxc_syntax/src/ast_json.rs`, qui lit l'arbre sans le
  modifier.

Le format JSON est proposé par Terra dans sa PR et revu par Main avant SH3. Il couvre
chaque nœud de l'AST avec son genre, ses enfants et sa position en octets. Aucun autre
changement de `nxc_syntax` n'est permis sans demande.

## Conséquences

- SH3 peut commencer : la cible est écrite (§2.8), stable, et l'égalité se teste sur tout
  le dépôt.
- Les formes réservées restent la seule réserve d'évolution de l'édition 1 ; tout le reste
  attend une édition 2.
- Le chapitre 02 décrit encore l'édition 0.2 aux §2.2 à §2.6 : elle reste acceptée sous
  `--edition 0.2`, et ces sections restent sa référence.
