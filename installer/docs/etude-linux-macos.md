# Étude — distribuer NEXUL sous Linux et macOS

Claude Outils, 15 octobre 2026. Statut : **étude**, aucune modification de code. Rien
n'a été essayé sur Linux ni sur macOS : l'étude repose sur le code du dépôt, sur le
`Cargo.lock` de Studio et sur les sources citées. Décisions demandées à la fin (§7).

## 1. Où en sont les composants

| Composant | Linux | macOS | Source |
|---|---|---|---|
| `nxc`, `nxpm` (Rust, sans dépendance) | se construisent | se construisent | — |
| Exécutables **produits** par `nxc` | statiques, appels système directs ; suite de conformité **vérifiée** | **non valides** : la couche système Linux est choisie à tort pour macOS (`compiler/nxc_sema/src/load.rs`) | `docs/roadmap.md`, `docs/refonte/audit-core.md` |
| Chargement de LLVM par `nxc` | `libLLVM.so` (`/usr/lib/llvm-N/lib`, `/usr/lib`…) | `libLLVM.dylib` (Homebrew) | `compiler/nxc/src/toolchain.rs` |
| Édition de liens | `ld.lld` | `ld64.lld` (bibliothèque `libSystem` décrite par un `.tbd` écrit par `nxc`) | `compiler/nxc/src/link.rs` |
| Studio natif (GPUI) | rendu `wgpu` 29 (backends Vulkan `ash` et OpenGL ES `glow` dans le graphe), X11 (`x11rb`), Wayland, clavier `xkbcommon`, polices `cosmic-text` + FreeType + Fontconfig, portails XDG (`ashpd`) | rendu Metal (`metal`) | `studio/app/Cargo.lock` |
| Ancienne interface web de Studio | **non portable** (s'affiche dans Edge) | idem | `studio/src/main.rs` |
| Terminal intégré (`studio/terminal`) | `portable-pty` ; testé sous WSL par Codex Luna | non testé | `docs/refonte/codex-exec-status.md` |
| Débogueur (`studio/dap`) | `lldb-dap` | `lldb-dap` + `debugserver` signé (§4.4) | — |
| `studio/lsp` | chemins Unix prévus, **non testé** | idem | `studio/lsp/README.md` |
| Réglages de Studio | **`%APPDATA%` seulement** : sous Linux et macOS, repli sur le dossier temporaire | idem | `studio/app/src/settings.rs`, `studio/src/main.rs` |
| Cache et paquets | `%LOCALAPPDATA%\nexul` | idem | `studio/src/runner.rs` |

Remarque : GPUI (le projet Zed) a remplacé son ancien moteur de rendu Linux (`blade`) par
`wgpu` en février 2026 ([PR Zed n° 46758][zed-wgpu]). Dans le graphe verrouillé de Studio,
`gpui-kit` 0.7.3 répartit GPUI en crates par plate-forme :
- `gpui-fast-wgpu` et `gpui-pre-wgpu` dépendent de `wgpu` 29 ;
- `gpui-fast-apple` et `gpui-pre-apple` dépendent de `metal`.

**Conclusion** :
- **Linux** est proche : il manque l'empaquetage, les chemins de réglages et des essais.
- **macOS** est **bloqué** tant que la couche système dédiée de Codex Astra n'existe pas :
  sans elle, NEXUL pourrait s'installer sur un Mac, mais les programmes qu'il produit n'y
  fonctionneraient pas.

## 2. Ce qui est commun aux deux systèmes

### 2.1 Disposition installée

Le principe est le même que sous Windows : **installation pour l'utilisateur, sans
droits d'administrateur**, avec un dossier par version.

```
~/.local/share/nexul/            (Linux ; macOS : voir §4.2)
  versions/0.3.0/
    bin/        nxc, nxpm, nexul-studio
    llvm/       lib/libLLVM.so…, bin/ld.lld, lldb-dap…
    examples/   docs/   install.txt
  current -> versions/0.3.0      (lien symbolique : bascule atomique, retour arrière)
~/.local/bin/nxc, nxpm, nexul-studio -> ../share/nexul/current/bin/…
```

Le dossier par version et le lien `current` rendent les mises à jour atomiques et
réversibles (voir `etude-mises-a-jour.md`). Cette disposition ne contredit pas la
recherche actuelle de `nxc` : elle passe par `current_exe()`, puis par le `PATH`.

### 2.2 Emplacements des données

| Données | Windows (actuel) | Linux (XDG) | macOS |
|---|---|---|---|
| Réglages de Studio | `%APPDATA%\NEXUL Studio` | `$XDG_CONFIG_HOME/nexul-studio` (`~/.config/…`) | `~/Library/Application Support/NEXUL Studio` |
| Cache de compilation, paquets | `%LOCALAPPDATA%\nexul` | `$XDG_CACHE_HOME/nexul`, `$XDG_DATA_HOME/nexul` | `~/Library/Caches/nexul` |

Demandes à faire :
- à **Claude Studio** : les réglages (`studio/app/src/settings.rs`, `studio/src/main.rs`) ;
- à **Codex Astra** : le cache (`nxc`, `nxpm`), à vérifier dans le compilateur.

### 2.3 Construction et intégrité

- **Intégration continue** (fichiers de Codex Astra : `.github/**`) : une matrice
  Windows, Linux x86_64 et aarch64, macOS arm64. Les builds Linux se font dans un
  conteneur de la distribution la plus ancienne visée, pour que la version de la glibc
  requise reste basse (§3.1).
- **Intégrité** : un fichier `SHA256SUMS` et sa signature Ed25519 accompagnent chaque
  publication, avec la même clé que les mises à jour (`etude-mises-a-jour.md`).

## 3. Linux

### 3.1 Systèmes visés

Distributions à base de **glibc**, x86_64 et aarch64. La version minimale de la glibc
est fixée par le système de construction. Si l'on vise Ubuntu 22.04 (glibc 2.35),
Debian 12 (2.36) et RHEL 9 (2.34), on construit sous RHEL 9 ou un équivalent (glibc
2.34). Les distributions à musl (Alpine) sont hors du périmètre au départ.

Studio a besoin de bibliothèques système. D'après le graphe des dépendances : Fontconfig,
FreeType, xkbcommon, la bibliothèque cliente Wayland ou X11, et un pilote **Vulkan** ou
**OpenGL ES** (pour `wgpu`). La liste exacte (chargement dynamique ou édition de liens)
**reste à confirmer** sur un build Linux, avec `ldd` et un essai sans Vulkan.

### 3.2 Formats

| Format | Pour | Contre | Verdict |
|---|---|---|---|
| **Archive `.tar.xz` + script d'installation** | sans droits root, une seule archive pour toutes les distributions ; c'est ce que fait Zed ([docs Zed][zed-linux]) | pas de mise à jour par le gestionnaire de paquets | **à faire en premier** |
| `.deb` / `.rpm` | intégration au système, dépendances déclarées | droits root ; un dépôt signé à héberger pour les mises à jour ; une variante par famille | ensuite, si la demande existe |
| AppImage | un seul fichier | NEXUL est une **chaîne d'outils** (PATH, plusieurs exécutables), pas une application isolée ; FUSE 2 absent par défaut sur Ubuntu 24.04 (paquet `libfuse2t64` à installer, [AppImage][appimage-fuse]) | non |
| Flatpak | Flathub, bac à sable | un IDE doit lancer des compilateurs, LLDB et des terminaux **hors** du bac à sable ; Zed sort du sien au démarrage, ce qui en annule l'intérêt ([docs Zed][zed-linux]) | non |
| Snap | Snap Store | confinement « classic » obligatoire, soumis à approbation | non |

### 3.3 Script d'installation

`install.sh` (POSIX, aucune dépendance au-delà de `tar`, `xz` et `sha256sum`) fait le
pendant Linux de `NEXUL Setup.exe` :

- il vérifie l'archive avec `SHA256SUMS` (signature : voir l'étude des mises à jour) ;
- il installe dans `~/.local/share/nexul/versions/<v>`, met à jour `current` et crée les
  liens dans `~/.local/bin`, en avertissant si ce dossier n'est pas dans le `PATH` ;
- il installe un fichier `.desktop` pour NEXUL Studio, l'icône et le type MIME
  `text/x-nexul` (association `.nx` via `xdg-mime`), tout cela dans `~/.local/share` ;
- il traite les anciennes versions comme sous Windows : celles qu'il a installées sont
  remplacées, les autres signalées ;
- `install.sh --uninstall` retire exactement ce qui figure dans `install.txt`, le même
  format que sous Windows.

### 3.4 LLVM

`nxc` demande LLVM 17 ou plus récent. Deux voies :

1. **Livrer LLVM** (`libLLVM.so`, `ld.lld`, `lldb-dap`, `liblldb`) dans `llvm/` : versions
   connues et reproductibles, mais une archive plus grosse et des builds à faire pour
   une glibc ancienne. Les archives officielles de LLVM pourraient servir de source ;
   **leur contenu (présence de `libLLVM.so`) et leur glibc minimale restent à vérifier**
   avant de s'appuyer dessus.
2. **Utiliser le LLVM du système** (`llvm-N`, `lld-N`, `lldb-N`) : une archive légère,
   mais Ubuntu 22.04 fournit des versions trop anciennes par défaut, et le débogueur
   varie d'une distribution à l'autre.

Recommandation : **1 par défaut**, et 2 possible via `NEXUL_LLVM`. `toolchain.rs`
cherche déjà `NEXUL_LLVM` et un dossier `llvm` installé à côté de `nxc`.

## 4. macOS

### 4.1 Prérequis bloquants

1. **Couche système macOS du runtime** (Codex Astra) : appels à `libSystem`, pas d'appels
   système Linux. Sans elle, aucun programme NEXUL ne fonctionne sur un Mac.
2. **Signature des programmes produits** : sur Apple Silicon, tout exécutable arm64
   doit porter au moins une signature *ad hoc*. `ld64.lld` l'ajoute **par défaut** pour
   arm64 et macOS ([LLD][lld-adhoc]) ; à **vérifier** sur un Mac (`codesign -dv`), car
   `nxc` passe ses propres options à `ld64.lld`.
3. **Chemins des réglages** (§2.2).

### 4.2 Forme du paquet

- **`NEXUL Studio.app`**, distribué dans un `.dmg` à glisser dans `Applications`.
  - `Contents/MacOS/` contient Studio, `Contents/Resources/` contient `nxc`, `nxpm` et
    `llvm/`.
  - `Info.plist` déclare le type de document `.nx` (UTI) pour l'association.
- **Commandes `nxc` et `nxpm` dans le terminal** : une action « Installer les commandes
  dans le PATH » de Studio crée des liens dans `~/.local/bin` (ou `/usr/local/bin` avec
  autorisation), à la manière de VS Code.
- **Archive pour la ligne de commande seule** (sans Studio), puis une formule Homebrew
  dans un dépôt dédié.
- **Architectures** : arm64 d'abord. x86_64 en option : binaire universel par `lipo`,
  et LLVM x86_64 en plus. La version minimale de macOS est à fixer avec GPUI ; les
  programmes produits visent macOS 11 (`link.rs`).

### 4.3 Signature et notarisation

Sans signature *Developer ID* ni **notarisation**, une application téléchargée est
bloquée par Gatekeeper. Depuis macOS Sequoia (15), le Ctrl-clic ne permet plus de passer
outre : l'utilisateur doit passer par *Réglages Système → Confidentialité et sécurité*
([Cult of Mac][sequoia]). Pour une distribution normale, il faut :

- le **programme Apple Developer** (abonnement annuel, compte de l'utilisateur ou d'une
  organisation) et un certificat *Developer ID Application* ;
- signer **chaque** binaire et bibliothèque du paquet avec le *hardened runtime*
  (`codesign --options runtime`). Les `.dylib` de LLVM chargées par `nxc` doivent être
  signées avec la même identité, sinon la validation des bibliothèques les refuse ;
- notariser avec `notarytool`, puis agrafer le ticket (`stapler`) au `.dmg`.

Ces opérations se font sur un Mac ou une machine macOS d'intégration continue, avec les
secrets de signature. Une formule Homebrew qui compile depuis les sources évite la
notarisation pour les utilisateurs avancés, mais ne remplace pas le `.dmg`.

### 4.4 Débogueur

Déboguer un autre processus sur macOS passe par un `debugserver` **signé par Apple** :
celui de Xcode ou de ses outils en ligne de commande ([LLVM, 2013][lldb-sign]). Les
programmes produits par `nxc` ne sont pas en *hardened runtime* et restent donc
attachables. Il est plus simple d'utiliser le `lldb-dap` des outils de Xcode s'il est
fourni (**à vérifier** selon la version de Xcode) que de livrer et signer un LLDB.

## 5. Plan

| Étape | Contenu | Dépend de |
|---|---|---|
| L1 | Chemins XDG pour les réglages et le cache | Claude Studio, Codex Astra |
| L2 | Build Linux de `nxc`, `nxpm` et Studio natif dans un conteneur à glibc 2.34 ; liste réelle des bibliothèques (`ldd`) ; essai Vulkan, OpenGL ES, X11, Wayland | intégration continue (Codex Astra) |
| L3 | Archive `.tar.xz` + `install.sh` (§3.3), LLVM livré (§3.4), `SHA256SUMS` signé | L2, choix du §3.4 |
| L4 | Essais sur Ubuntu 22.04, 24.04, Fedora, Debian (machines virtuelles) | L3 |
| M1 | Couche système macOS du runtime, essais de conformité sur un Mac | **Codex Astra**, un Mac |
| M2 | `NEXUL Studio.app` + `.dmg`, signature et notarisation | M1, compte Apple Developer |
| M3 | Formule Homebrew, commandes dans le PATH | M2 |

Le travail de Claude Outils (L3, M2, M3) est l'empaquetage. Les étapes L1, L2 et M1
reviennent aux autres agents, comme demandes. **Aucun Linux natif ni Mac n'est
disponible** sur le PC de Claude Outils : WSL (Ubuntu) suffit pour L2 et L3, mais pas
pour le rendu de Studio, ni pour macOS.

## 6. Risques

- **Pilotes graphiques** sous Linux (machines virtuelles, sessions distantes) : `wgpu`
  a besoin de Vulkan ou d'OpenGL ES. Le repli logiciel reste à essayer.
- **Coût récurrent** de la signature Apple, et dépendance à un Mac pour publier.
- **Taille** : livrer LLVM pour chaque système et architecture multiplie la taille des
  publications.

## 7. Décisions demandées

1. **Priorité** : Linux d'abord (le plus proche du but), macOS seulement après la couche
   système de Codex Astra ?
2. **Format Linux** : archive + `install.sh` d'abord, `.deb`/`.rpm` plus tard ; AppImage,
   Flatpak et Snap écartés ?
3. **LLVM sous Linux** : livré avec NEXUL (recommandé) ou celui du système ?
4. **macOS** : un compte Apple Developer est-il envisagé (sans lui, pas de distribution
   normale par `.dmg`) ?
5. **Glibc minimale** : 2.34 (RHEL 9, Ubuntu 22.04, Debian 12) ?

## Sources

- [Zed, PR n° 46758 — le rendu Linux de GPUI passe à wgpu (février 2026)][zed-wgpu]
- [Zed — installation sous Linux, archive et Flatpak][zed-linux]
- [AppImage — FUSE et Ubuntu 24.04][appimage-fuse]
- [LLD — signature ad hoc par défaut sur arm64/macOS][lld-adhoc]
- [Cult of Mac — Gatekeeper sous macOS Sequoia][sequoia]
- [LLVM lldb-dev — debugserver signé][lldb-sign]

[zed-wgpu]: https://github.com/zed-industries/zed/pull/46758
[zed-linux]: https://zed.dev/docs/linux.html
[appimage-fuse]: https://github.com/AppImage/AppImageKit/wiki/FUSE
[lld-adhoc]: https://llvm.googlesource.com/llvm-project/lld/+/f87957fbe24e7cbdfc0351f5bef27542c2f06113%5E%21/
[sequoia]: https://www.cultofmac.com/864843/macos-sequoia-removes-handy-shortcut-bypass-gatekeeper-security/
[lldb-sign]: https://lists.llvm.org/pipermail/lldb-dev/2013-June/001946.html
