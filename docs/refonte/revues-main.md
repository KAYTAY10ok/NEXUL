# Relectures de Claude Main

Une entrée par PR relue. « À corriger » devient une tâche de son propriétaire actuel
(équipe réduite du 15/10 : Codex Sol pour l'extension et l'installateur, Main ailleurs).

## PR #15 — audit G0 de la syntaxe (Outils) — 15/10/2026

**Acceptée.** Audit fait sur le vrai lexer et le vrai parser, recensement relançable, deux
pièges vérifiés avec `nxc check`. Les recommandations du §7 sont reprises dans G1.

Remarques :

1. Le piège §4.1 (ligne qui commence par `-`, `not`, `(`…) est grave : un programme mal
   découpé compile sans message. En plus de la grammaire G1, la 0.2 peut le signaler dès
   maintenant : une instruction-expression dont la valeur n'est pas `()` et n'est pas
   utilisée devrait être une erreur. → tâche de Main (compilateur), avant P6.
2. §4.2 (`Vret` devient une variable) : W0007 doit devenir une erreur dans la nouvelle
   syntaxe ; les trois propositions G1 le règlent par la grammaire.
3. Recommandation 4 (une seule source du lexique, `nxc ide lexicon`) : retenue ; à
   concevoir avec l'ADR G2.

## PR #16 — extension VS Code (Luna) — 15/10/2026

**Acceptée, deux corrections demandées** (propriétaire : Codex Sol).

1. **Sécurité — à corriger.** `nexul.nxcPath` n'a pas de `scope` : un dépôt peut le
   définir dans `.vscode/settings.json` et faire exécuter son propre programme à
   l'ouverture ou à l'enregistrement d'un `.nx` (`checkOnSave` est vrai par défaut). Corriger
   dans `package.json` : `"scope": "machine-overridable"` (ou `machine`) pour
   `nexul.nxcPath`, et déclarer `capabilities.untrustedWorkspaces`
   (`"supported": "limited"`, `"restrictedConfigurations": ["nexul.nxcPath"]`). Test :
   un réglage de dossier non approuvé n'est pas utilisé.
2. **Essais réels manquants** : `nxc` absent sur la machine de Luna ; diagnostics, build,
   run, test et format n'ont pas été essayés avec le vrai compilateur (tâche L4).
3. Bien : processus lancés sans shell (`shell: false`), formatage à l'enregistrement
   désactivé par défaut, aucune grammaire présentée comme finale.

## PR #14 — installateur, assistant Win32 (Outils) — 15/10/2026

**Acceptée** (fusionnée avant le transfert effectif à Luna puis à Codex Sol), **une
correction demandée** (propriétaire : Codex Sol).

1. **Sécurité — à corriger.** Pour les anciennes versions installées « pour la machine »,
   l'installateur écrit la liste des opérations (`del`, suppression de dossiers, registre)
   dans `%TEMP%\nexul-setup-<pid>\job.txt`, puis relance **le même programme avec
   élévation** qui exécute ce fichier (`old::run_elevated`, `old::job_main`). Le dossier
   est modifiable par tout programme de l'utilisateur : entre l'écriture et la lecture
   élevée, il peut remplacer les opérations et obtenir des suppressions avec les droits
   d'administrateur. Corriger : le processus élevé **refait lui-même l'inventaire** des
   anciennes versions (`old::scan`) et n'exécute que des opérations qu'il a calculées ; à
   défaut, il refuse toute opération hors des emplacements connus de NEXUL. Test : un
   `job.txt` modifié n'entraîne aucune suppression hors de ces emplacements.
2. Le mécanisme date de l'étape 1 (PR #11) ; l'étape 2 l'utilise tel quel.

## PR #17 — QA et documentation (Gemma) — 15/10/2026

**Acceptée.** Méthode honnête (ce qui est rapporté, non rejoué, à valider).

1. Vérifié par Main : `python tests/manual/run_smoke.py` → 3/3 réussis (`nxc` construit
   depuis `main`).
2. `quality-matrix.md` et `manual-studio-checklist.md` décrivent `main` à `548b1f6` : ils
   sont dépassés (fermeture avec dialogue, coloration et modèles raccordés, extension VS
   Code livrée). Main les met à jour avec l'essai manuel M19.

## 16/10/2026 — PR #20, #21, #25 (Codex Sol) : intégrées par Main

Revue outillée (un relecteur par PR, worktree propre, `main` fusionné, vrais tests MSVC).

**#20 L1 terminal ConPTY — INTÉGRÉE.** Le blocage est corrigé : sur `main`, les 2 tests
ConPTY dépassaient 60 s ; avec la PR, 5 suites et 90 exécutions directes (dont 6 processus
en parallèle) passent en 0,4–1,4 s ; 30 cycles lancement/arrêt sans fuite de handles ni
de threads ; un seul `Exited` par terminal. À reprendre par Sol (faible) :
1. Le lecteur écrit les réponses DSR sous le verrou d'entrée : risque d'interblocage Unix
   avec un grand collage. Passer par un fil d'écriture dédié (canal) ou `try_lock` + file.
2. Le surveillant interroge `try_wait` toutes les 10 ms sous le verrou du processus :
   préférer `clone_killer()` + `wait()` bloquant.
3. `lock().unwrap()` dans le nettoyage du surveillant → `unwrap_or_else(|e| e.into_inner())`.
4. Unix : `stop()` peut attendre tant qu'un travail d'arrière-plan garde le PTY (antérieur
   à la PR) : délai de grâce ou limite documentée.

**#21 L2 DAP avec le vrai `lldb-dap` — INTÉGRÉE.** Sur `main`, le test réel échoue
(`Timeout("launch")`, 16 s) ; avec la PR : 3 + 4 + 1 réel (point d'arrêt vérifié,
`answer = 42`, pas à pas), 30 + 10 exécutions stables ; corrélation par numéro de
séquence correcte. À reprendre par Sol :
1. (moyen) `launch()`/`attach()` bloquants restent publics mais attendent toujours 15 s avec
   `lldb-dap` 23 : `#[deprecated]` vers `begin_launch`/`begin_attach`, ou un test qui les couvre.
2. `PendingRequest::wait` consomme la requête : ajouter `try_wait` (sondage sans blocage
   pour l'interface, M7).
3. Une requête envoyée après l'arrêt du lecteur attend son délai au lieu de
   `AdapterExited` (antérieur) ; le README l'affirme pourtant.
4. Avec `NEXUL_NXC`/`NEXUL_LLVM` définis, un outil absent doit faire échouer le test réel
   au lieu de le sauter.

**#25 L3 installateur natif + CI — INTÉGRÉE avec corrections de Main.** Conflits résolus :
le CI de `main` (PR #28, déjà vert) est gardé, plus l'étape des tests bac à sable de
l'installateur et des étapes Windows pour `studio/syntax`, `terminal`, `dap`, `lsp` ;
l'étape LLVM 19 est abandonnée (inutile depuis `f8fc0d3`). Corrigé par Main dans
`installer/src/migrate.rs` :
- l'analyseur JSON était quadratique (1 Mio : 193 s) → copie des suites de caractères,
  linéaire ; test `large_strings_parse_in_linear_time` ;
- un dossier `NEXUL Studio` ou un `state.json` qui est un lien ou une jonction, ou qui
  sort de `%APPDATA%`, annule la migration (installateur élevé) ;
- deux dossiers ne différant que par la casse gardent la session la plus récente (`t`) ;
  test `folders_differing_only_in_case_keep_the_most_recent_session`.
Tests : `cargo test --manifest-path installer/Cargo.toml --features test-sandbox --test
silent --bins` (24 + 4) et `--test ui` (2) verts sur cette machine (session administrateur).
Non testé : `tools/build_installer.py` complet avec Studio natif MSVC (tâche 4 de Sol).
## 28/10/2026 — PR #29 à #67 : relecture et intégration par Main

Reprise après le 16/10 : 39 PR ouvertes. La pile Core #35 → #66 est linéaire (chaque PR
contient la précédente ; #51 et le correctif P1 de #63 y sont déjà fusionnés). Les PR de
Codex Sol du 24/10 sont empilées sur #43 ; #61 les réunit (C1–C10), #57 réunit C9, C11 et
C12. Les PR du 16/10 (#29 à #34) partent de `main`. Tout est fusionné sur la branche
`claude/pensive-darwin-3e2tye`, une seule PR vers `main` : sa fusion ferme les autres.

Machine de revue : conteneur Linux x86-64 (4 vCPU), Rust 1.97, LLVM 18 et 20, LLD,
Node 22. Non disponibles : Windows, MSVC, macOS, matériel ARM ou RISC-V.

### Tests exécutés

| Où | Commande | Résultat |
|---|---|---|
| `main` (`c40acbd`) | `cargo test --release --workspace --exclude nexul-installer` | vert |
| sommet Core #66 (`a1370b4`) | idem, `--no-fail-fast` | vert (`run_programs` O0/O2 compris) |
| branche intégrée | idem | vert : `codegen_ir` 7, `lsp` 9, `robustness` 4, `suite` 13 |
| branche intégrée | `cargo test --release -p nxc --test web -- --ignored` | 1/1 (ABI navigateur, `@export` vérifiés, pile WASI/web avant les données) |
| branche intégrée | `cargo test --manifest-path studio/{syntax,lsp,terminal,dap,templates}/Cargo.toml` | vert après le correctif ETXTBSY ci-dessous |
| `editors/vscode` | `node --test test/cli.test.js test/edition.test.js test/trust.test.js` | 15/15 |

CI GitHub de #64, #65, #66 : verte sous Linux et Windows (25/10).

### Pile Core (#35 → #66) — ACCEPTÉE

Lu en priorité ce qui touche la sûreté : `std.tache` et les threads (`sys/linux.nx`,
`sys/windows.nx`, intrinsèque `linux_clone`), propriétés `Transfer`/`Share`
(`defs.rs`), règles des tâches (`check/call.rs`, `places.rs`), ordre de travail de
l'analyse de propriété (`ownership.rs`, lot 5).

**Essais adverses** (programmes écrits pour cette revue, tous refusés comme attendu) : deux
tâches qui modifient un paramètre `inout` de la fonction englobante ; une modification
cachée dans une fonction anonyme imbriquée (`v.sort_by(fn …)`) ; une méthode `inout self`
pendant qu'une autre tâche lit ; deux champs différents d'une même structure (refus
conservateur, par la racine) ; un `Vec[dyn Show]` dont l'interface ne demande pas
`Transfer` envoyé à une tâche détachée (E0249) ; une modification dans le rappel d'un
`Verrou.avec` en concurrence avec une autre tâche ; une tâche lancée dans une boucle
(E0254) ; une affectation après le lancement d'une tâche qui lit (E0420) ; une fonction
anonyme passée ailleurs qui modifie une variable d'une tâche ; deux `portee` imbriquées.

**Exécution réelle** : panique dans un thread → message et code 101 (`exit_group`) ;
dépassement de pile dans un thread (trames de 512 o et de 320 Kio) → `panic: stack
overflow`, code 101 ; 40 producteurs détachés, 10⁶ valeurs : résultat exact.

À reprendre :
1. **(corrigé ici) Débit des canaux** : un seul mot de signal pour tout changement et un
   réveil de tous les threads à chaque valeur. Avec 40 producteurs sur 4 cœurs,
   100 000 valeurs prenaient 53 s en `--release` (Rust `std::sync::mpsc` : 1,9 s). Chaque
   thread qui attend dort maintenant sur son propre mot, dans une file du canal ; le
   verrou réveille un seul thread (`sys.wake_one`) ; attente active courte avant de
   dormir. Mesures : 100 000 valeurs 0,9 s, 10⁶ valeurs 8,8 s (Rust std : 17,8 s).
   Test ajouté : `tests/run/task_channels_stress.nx`.
2. **(corrigé ici) Windows** : `SetThreadStackGuarantee` vaut pour un seul thread ; les
   threads de `std.tache` ne réservaient rien pour le gestionnaire de C2. Non testé sur
   Windows (pas de machine) : la CI le dira.
3. **Sondes de pile et threads** : sans les sondes de C1, une trame plus grande que la
   garde de 64 Kio d'un thread peut atteindre la pile d'un autre thread. C1 est dans cette
   intégration : à garder obligatoire pour toute cible avec threads. Windows sonde par
   `__chkstk`.
4. `Verrou.avec` réentrant (un rappel qui reprend le même verrou) bloque : pas une faute
   de sûreté, mais à signaler dans la spécification §11.5 ou à détecter.
5. Sous Windows, l'attente est une cession du processeur en boucle (`WaitOnAddress`
   prévu) : correct, mais coûteux pour des tâches qui attendent longtemps.

### Codex Sol, C1–C12 (#44 à #62) — ACCEPTÉES

- C1 sondes (`"probe-stack"="inline-asm"` sur toutes les définitions Linux/macOS
  x86-64/AArch64), C2 (Windows et RISC-V), C3 `--stack-first`, C5 (déjà dans la pile Core),
  C8 (réessai de 1018, clés uniques), C7 (CI sur toute base), #62 (alias de chemins du
  LSP), C6 (suite du compilateur sous Windows avec LLVM 20.1.8 vérifié par SHA-256) :
  lus et testés ici quand c'était possible.
- C10 : l'explication de l'écart « 1,12× C » est honnête (pas de contrôle entier dans la
  boucle flottante, aucune vérification retirée) ; gain de `String.push` ASCII (−12,7 %
  à 10 M) accepté.
- C9, C11 : propositions acceptées comme documents ; leur code passera par une revue de
  sûreté (`ownership.rs`, NIR monomorphe : IR identique à l'octet exigée).
- C12 : macOS **BLOQUÉ** (pas de Mac) ; la cible reste annoncée « non validée ».
- La garde de couverture à 90 % de #63 (`14ce81d`) n'est **pas** intégrée (décision du
  25/10 maintenue) ; #67 reste un brouillon de mesure, à fermer.

### Codex Sol, 0.3.0 (#29 à #34) — ACCEPTÉES

- #31 (DAP) et #32 (terminal) traitent toutes les remarques du 16/10 (fil d'écriture
  dédié pour les réponses DSR, `wait()` bloquant sur `clone_killer`, `try_wait`,
  `#[deprecated]` sur `launch`/`attach`, `AdapterExited` immédiat, outil absent avec
  `NEXUL_NXC` défini = échec).
- #30 (VSIX 0.3.0, formatage à l'enregistrement) : **corrigé ici**, un document non vide
  n'est jamais remplacé par une sortie vide de `nxc` (constaté sans risque aujourd'hui :
  sur une erreur de syntaxe, `nxc ide format --stdin` rend le texte inchangé).
- #33 (versions de distribution alignées avant de construire l'installateur), #34 (tests
  LSP d'éditions mélangées), #29 (références P13 brutes) : acceptées.

### Correctifs de Main dans cette intégration

- `studio/lsp` : `detection_in_a_folder_of_fake_tools` échouait environ une fois sur 12
  sous Linux (outil copié pendant qu'un autre thread `fork` : ETXTBSY) ; la sonde
  réessaie. Avant : 1 échec sur 12 ; après : 0 sur 12. Le job Linux de la CI teste
  maintenant les crates de Studio sans GPUI (il ne les lançait pas).
