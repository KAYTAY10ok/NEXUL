# Étude — une interface native pour l'installateur Windows

Claude Outils, 15 octobre 2026. Statut : **étude**, aucune modification de code. Les
décisions demandées à l'utilisateur sont à la fin (§7).

La mission demande que le lancement de NEXUL Studio ne dépende plus d'Edge et d'étudier
une interface native pour l'installateur lui-même. Le premier point sera réglé par
l'empaquetage de l'application native de Studio (§6) ; cette étude porte surtout sur le
second.

## 1. L'installateur aujourd'hui (d'après le code)

`installer/src/main.rs` est un exécutable Rust **sans dépendance**. Son interface :

1. démarre un serveur HTTP sur `127.0.0.1`, port aléatoire, protégé par un jeton
   aléatoire et par la vérification de l'en-tête `Host` ;
2. ouvre `installer/ui/index.html` (30 Ko de HTML, CSS et JavaScript) dans
   **Microsoft Edge** en mode application (`msedge --app=… --user-data-dir=%TEMP%\nexul-setup-window`),
   et à défaut dans le **navigateur par défaut** (`cmd /C start`) ;
3. s'arrête 10 s après la dernière requête `ping` de la page, ou 90 s après le démarrage
   si la page ne s'est jamais connectée.

Pages : Bienvenue, Anciennes versions, Composants, Emplacement et options, Installation
en cours, Terminé ; Désinstaller pour le désinstallateur (le même programme sans ses
données). Le mode `--silent` installe et désinstalle sans fenêtre.

Les opérations sur le système passent par des **programmes externes** :

| Opération | Moyen actuel | Où |
|---|---|---|
| PATH et `NEXUL_HOME` de l'utilisateur | PowerShell (`[Environment]::SetEnvironmentVariable`) | `main.rs`, `old.rs` |
| Raccourcis du Bureau et du menu Démarrer | PowerShell + COM `WScript.Shell` | `main.rs` |
| Dossier du Bureau | PowerShell (`GetFolderPath`) | `main.rs` |
| Choix du dossier d'installation | PowerShell + Windows Forms (`FolderBrowserDialog`) | `main.rs` |
| Recherche des anciennes versions (cibles des raccourcis…) | script PowerShell + COM | `old.rs` |
| Signaler le changement d'association `.nx` | PowerShell + `Add-Type` (C# compilé à la volée) | `old.rs` |
| Droits d'administrateur pour les anciennes versions | PowerShell `Start-Process -Verb RunAs` | `old.rs` |
| Registre (association, inscription, nettoyage) | `reg.exe` (une quinzaine d'appels) | `main.rs`, `old.rs` |
| Arrêter Studio | `taskkill /F /IM "NEXUL Studio.exe"` | `main.rs`, `old.rs` |
| Effacer le désinstallateur après sa fin | PowerShell `Wait-Process` puis `Remove-Item` | `main.rs` |

Coût mesuré sur le PC de Claude Outils (Windows 11, à chaud) : PowerShell vide **202 ms**,
avec l'objet COM des raccourcis **338 ms**, avec `Add-Type` **600 ms**. Une installation
complète lance PowerShell environ huit fois, soit **2 à 3 s** d'attente due aux seuls
démarrages de PowerShell.

Le programme seul (sans données), construit avec `windows-gnu` en mode release, pèse
**768 Ko**.

## 2. Les problèmes

1. **Edge n'est pas garanti.** Depuis mars 2024, Edge peut être désinstallé par les
   utilisateurs de l'Espace économique européen ([Windows Central][edge-dma]). Il est
   aussi absent ou bloqué sur des postes gérés et sur certaines éditions de serveur. Le
   repli ouvre alors l'assistant dans un **onglet** du navigateur par défaut : on peut le
   fermer ou le perdre parmi les autres, et c'est le `ping` qui décide de la fin du
   programme.
2. **Un profil Edge temporaire à chaque lancement** (`%TEMP%\nexul-setup-window`) : premier
   démarrage plus lent et fichiers laissés dans `%TEMP%`.
3. **PowerShell lancé avec `-ExecutionPolicy Bypass` par un exécutable non signé** est
   un schéma typique de logiciel malveillant. Les antivirus et les outils de détection
   d'entreprise (EDR) le surveillent ou le bloquent. En **mode de langage restreint**
   (AppLocker ou WDAC), `New-Object -ComObject` et `Add-Type` sont refusés : raccourcis,
   choix du dossier, recherche des anciennes versions et notification d'association
   échouent alors. Ce PC est en `FullLanguage`, si bien que cette panne n'a pas été
   reproduite ici.
4. **Lenteur** : 2 à 3 s de démarrages de PowerShell, plus les processus `reg.exe`.
5. **Arrêt de Studio trop large** : `taskkill /IM "NEXUL Studio.exe"` arrête **toutes**
   les instances de ce nom, y compris celles d'une autre copie de NEXUL (par exemple un
   build de développement).
6. **Cohérence** : Studio devient une application native (GPUI), alors que son
   installateur reste une page web.

Le serveur local n'est pas un problème en soi (jeton et `Host` vérifiés, écoute sur
`127.0.0.1` seulement). Mais il disparaît naturellement avec une interface native.

## 3. Options

| | A. Win32 natif (FFI) | B. Boîtes `TaskDialog` | C. WebView2 | D. GPUI (comme Studio) | E. Slint, egui… |
|---|---|---|---|---|---|
| Dépendances Rust | **aucune** (comme aujourd'hui) | aucune | aucune avec un chargeur maison, sinon `webview2-com` | des centaines de crates (841 dans `studio/app/Cargo.lock`) | des dizaines |
| Chaîne de construction | `windows-gnu` ou MSVC | idem | idem | **MSVC + Windows SDK** (`fxc.exe`) | selon la crate |
| Prérequis sur le PC | aucun | aucun | runtime WebView2, c'est-à-dire **le moteur d'Edge** | DirectX 11 | GPU ou rendu logiciel |
| Taille | ~1 Mo estimé | ~0,8 Mo | ~1 Mo + runtime | ~20 Mo (prototype GPUI : 22,5 Mo) | 3 à 10 Mo |
| Reprend l'interface actuelle | non (à réécrire) | non, trop limitée (liste des composants, cartes des anciennes versions) | **oui** (HTML) | non | non |
| Accessibilité | contrôles standard, UI Automation | standard | navigateur | partielle | variable |
| Respecte « pas de dépendance sans Codex Astra » (`installer/` est dans le workspace racine) | oui | oui | oui ou non | **non** | non |

- **B** ne suffit pas pour l'assistant, mais reste utile pour les messages simples
  (erreur, confirmation).
- **C** garde le HTML. Mais WebView2 *est* le moteur d'Edge : l'esprit de la consigne
  « ne plus dépendre d'Edge » n'est pas respecté, et charger WebView2 sans dépendance
  demande d'écrire un chargeur COM.
- **D** donnerait la même apparence que Studio. En contrepartie, il faudrait la chaîne
  MSVC, un installateur d'environ 20 Mo et un espace de travail à part (pour ne pas
  ajouter de dépendances au workspace racine). C'est excessif pour un assistant de cinq
  pages.

## 4. Recommandation : A, Win32 natif sans dépendance

L'installateur appelle déjà Win32 directement (`RegGetValueW`, API de compression de
`cabinet.dll`). Il suffit d'étendre ce choix à l'interface et aux opérations système.

### 4.1 Correspondance des opérations

| Aujourd'hui | Remplacement natif |
|---|---|
| Edge + `index.html` | fenêtre `CreateWindowExW`, contrôles communs version 6 (dépendance `Microsoft.Windows.Common-Controls` à ajouter au manifeste ; voir §4.2 pour le cas où il n'est pas compilé) |
| `FolderBrowserDialog` (PowerShell) | `IFileOpenDialog` avec `FOS_PICKFOLDERS`, la boîte moderne de l'Explorateur |
| `WScript.Shell` (PowerShell) | `IShellLinkW` + `IPersistFile` |
| `GetFolderPath('Desktop')` | `SHGetKnownFolderPath(FOLDERID_Desktop / FOLDERID_Programs)` |
| `reg.exe add/delete` | `RegCreateKeyExW`, `RegSetValueExW`, `RegDeleteTreeW` |
| PATH et `NEXUL_HOME` (PowerShell) | valeur `REG_EXPAND_SZ` sous `HKCU\Environment`, puis `SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, "Environment")` |
| `Add-Type` + `SHChangeNotify` | `SHChangeNotify(SHCNE_ASSOCCHANGED)` appelé directement |
| `Start-Process -Verb RunAs` | `ShellExecuteExW` avec le verbe `runas`, puis `WaitForSingleObject` et `GetExitCodeProcess` |
| `taskkill /IM` | énumération des processus (`CreateToolhelp32Snapshot`), arrêt des seuls processus **dont l'exécutable est dans le dossier traité** |
| auto-effacement par PowerShell | méthode classique des désinstallateurs : le désinstallateur se copie dans `%TEMP%`, se relance depuis la copie et efface le dossier une fois l'original terminé |
| progression dans la page | barre de progression, et progression sur le bouton de la barre des tâches (`ITaskbarList3`) |

### 4.2 Interface

- **Les mêmes pages qu'aujourd'hui**, avec les boutons Précédent / Suivant / Installer /
  Annuler. Les composants s'affichent dans une liste à cases à cocher (avec leur taille),
  les anciennes versions sous forme de lignes avec leur choix (Supprimer / Sauvegarder /
  Conserver).
- **Haute résolution** : `PerMonitorV2` (le manifeste demande aujourd'hui `true/pm`),
  mise à l'échelle sur `WM_DPICHANGED`, police système (Segoe UI) prise dans
  `NONCLIENTMETRICS`.
- **Indépendance vis-à-vis des ressources** : `build.rs` ne compile l'icône et le
  manifeste que s'il trouve `llvm-rc` ; sans LLVM, le programme n'a ni l'un ni l'autre.
  C'est le cas du build fait sur le PC de Claude Outils. L'interface native s'en passe
  donc :
  - réglage DPI à l'exécution (`SetProcessDpiAwarenessContext`) ;
  - contrôles version 6 activés par un contexte d'activation (`CreateActCtxW`) ;
  - pour les builds de livraison, `build_installer.py` vérifie que les ressources sont
    bien présentes.
- **Thème sombre** : barre de titre sombre (`DwmSetWindowAttribute`, attribut documenté
  sous Windows 11) et fonds colorés via `WM_CTLCOLOR*`. Les boutons et cases standard
  n'ont **pas** de thème sombre documenté : on suit le thème clair, ou l'on accepte des
  contrôles clairs sur un fond sombre. Cette limite est assumée.
- **Clavier et accessibilité** : `IsDialogMessage` (Tab, Entrée, Échap), raccourcis
  soulignés, contrôles standard exposés à UI Automation (Narrateur).
- **Langue** : français, comme aujourd'hui. Les textes sont rassemblés dans un seul
  module pour pouvoir les traduire plus tard.

### 4.3 Structure du code

- `installer/src/win/ffi.rs` : déclarations `extern "system"` (environ 400 lignes), liées
  par `#[link(name = "...")]` comme le code actuel. On évite `raw-dylib`, qui pose
  problème avec la chaîne `windows-gnu` (problème déjà constaté par Claude Studio).
- `installer/src/win/system.rs` : raccourcis, dossiers connus, registre, environnement,
  processus, élévation (environ 600 lignes). Ce module remplace `powershell()`, `reg_add`,
  `reg_delete` et les `taskkill`.
- `installer/src/win/wizard.rs` : la fenêtre et ses pages (environ 1 200 lignes).
- **Inchangés** : `install()`, `uninstall()`, `old.rs` (hors ses appels à PowerShell),
  `payload.rs`, `compress.rs`, `pack.rs`, le mode `--silent`. La couche HTTP (`http.rs`,
  `api()`, `ui/index.html`) disparaît à la fin : les pages appellent les mêmes fonctions
  directement.

Estimation : **environ 2 200 lignes** ajoutées, environ 700 retirées (HTML, HTTP,
scripts). La taille du programme devrait rester proche du mégaoctet (estimation, à
mesurer).

### 4.4 Tests

- Les tests existants (`old.rs`) et ceux du mode `--silent` restent valables.
- Règle 8 d'AGENTS.md (ne pas toucher aux réglages réels) : un mode de test redirigera
  les écritures du registre sous une clé de test (`HKCU\Software\NEXUL-Tests\<id>\…`),
  les raccourcis et le PATH vers un dossier temporaire. Il sera activé par une variable
  d'environnement lue **uniquement** par les builds de test.
- Fonctions système : test aller-retour pour chaque opération (créer, relire, supprimer
  un raccourci, une clé, une valeur de PATH), plus un test de l'arrêt ciblé des processus
  (un faux « NEXUL Studio.exe » dans deux dossiers : seul celui du dossier traité
  s'arrête).
- Assistant : un mode `--ui-selftest` ouvre la fenêtre, parcourt les pages par messages,
  vérifie l'état des contrôles, puis ferme. On ajoutera un essai manuel sous le Narrateur.

## 5. Plan

| Étape | Contenu | Gain | Vérification |
|---|---|---|---|
| 1 — **fait** (15/10, `installer/src/win/`) | Remplacer PowerShell, `reg.exe` et `taskkill` par les appels natifs (§4.1), en gardant l'interface HTML | plus de blocage en mode restreint ni par les antivirus ; 2 à 3 s gagnées | 13 tests unitaires, test du choix du dossier, installation, mise à jour et désinstallation `--silent` réelles dans un bac à sable |
| 2 — **fait** (15/10, `installer/src/ui/`) | Assistant natif ; l'interface HTML reste disponible derrière `--web-ui` pendant une version | plus de dépendance à Edge | `--ui-selftest` de bout en bout (installation et désinstallation) avec captures à 100, 125 et 150 % ; essais manuels encore à faire (clavier, Narrateur, changement d'écran) |
| 3 | Retrait de `http.rs`, `index.html` et `--web-ui` | code plus simple | tests |

Les étapes 1 et 3 se construisent et se testent **sur ce PC** (`windows-gnu`, sans outils
C++). L'étape 2 aussi pour le code ; seul l'essai visuel demande quelqu'un devant l'écran.

### 5.1 Bilan de l'étape 2

Ce qui a été fait :

- `installer/src/ui/model.rs` : l'état de l'assistant, sans Windows. Il porte les pages,
  les règles et les textes de l'interface HTML : licences, choix par ancienne version,
  dépendances entre composants, validation du dossier, résumé. Il est couvert par des
  tests unitaires.
- `installer/src/ui/window.rs` : la fenêtre, ses pages et le fil d'installation ;
  `sys.rs` : les déclarations Win32 ; `selftest.rs` : le pilote des tests.
- Les hauteurs de texte sont **mesurées** (`DrawTextW` avec `DT_CALCRECT`) plutôt que
  fixées. Les polices ne grandissent pas exactement comme l'échelle : avec des hauteurs
  fixes, des lignes étaient coupées à 150 %, ce que les captures ont montré.
- Test `installer/tests/ui.rs` : le vrai installateur, construit par `nexul-pack`, piloté
  dans le bac à sable ; chaque page est capturée et vérifiée.

Écarts par rapport au §4.2, à reprendre si l'utilisateur le souhaite :

- pas de thème sombre (la fenêtre suit le thème clair) ;
- pas de raccourcis soulignés ;
- police Segoe UI fixée plutôt que lue dans `NONCLIENTMETRICS` ;
- la fenêtre ne se réduit pas sur un écran de moins de 640 pixels logiques de haut ;
- 200 % n'est pas testé automatiquement : la fenêtre ferait alors 1 200 pixels de haut,
  plus que l'écran de ce PC (Windows la tronque).

Essais manuels restant à faire devant l'écran :

- navigation au clavier ;
- lecture par le Narrateur ;
- déplacement de la fenêtre entre deux écrans d'échelles différentes.

## 6. Lancer Studio sans Edge : empaqueter l'application native

L'installateur empaquette aujourd'hui `target/release/NEXUL-Studio.exe`, c'est-à-dire
l'**ancienne** interface web de Studio, qui s'affiche elle-même dans Edge. Les
changements prévus dans `tools/build_installer.py` :

- prendre `studio/app/target/release/NEXUL-Studio-Native.exe` (construit avec la chaîne
  MSVC) et l'installer sous `bin\NEXUL Studio.exe` : raccourcis, association `.nx` et
  bouton « Lancer » de la page finale l'utilisent sans changement ;
- pendant la transition, livrer l'ancienne interface sous un autre nom
  (`bin\NEXUL Studio (web).exe`), comme le permet AGENTS.md ;
- **C runtime** : `studio/app` n'a pas de configuration `+crt-static`. Or un exécutable
  Rust construit avec MSVC dépend par défaut de `vcruntime140.dll`, absent d'un Windows
  neuf. **À vérifier** sur le PC équipé de MSVC
  (`dumpbin /dependents NEXUL-Studio-Native.exe`). Deux solutions :
  - construire avec `-C target-feature=+crt-static` (demande à Claude Studio, qui possède
    `studio/app`) ;
  - livrer les DLL du runtime à côté de l'exécutable, ce que Microsoft autorise.

  La première est plus simple.
- **Prérequis de construction** : l'outillage MSVC (Visual Studio Build Tools, charge
  « Développement Desktop en C++ » et Windows SDK) **n'est pas installé** sur le PC de
  Claude Outils. Deux possibilités : l'installer (plusieurs Go, accord de l'utilisateur),
  ou construire l'installateur sur l'autre PC.

## 7. Décisions demandées

1. **Adopter l'option A** (Win32 natif, sans dépendance) pour l'assistant ?
2. **Ordre** : commencer par l'étape 1 (retrait de PowerShell), utile tout de suite et
   testable ici ?
3. **Durée de vie de l'interface web** : une version derrière `--web-ui`, puis retrait ?
4. **Empaquetage de Studio natif** : construire ici (installer les outils C++ de Visual
   Studio et LLVM sur ce PC) ou sur l'autre PC ?

## Sources

- [Windows Central — Edge et Bing désinstallables dans l'EEE (DMA)][edge-dma]
- `docs/studio/architecture-native.md` (taille et dépendances du prototype GPUI)

[edge-dma]: https://www.windowscentral.com/software-apps/windows-11/microsoft-will-let-windows-11-users-in-europe-uninstall-edge-bing-and-disable-ads-in-eea-dma
