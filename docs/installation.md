# Installer NEXUL

## Windows : `NEXUL Setup.exe`

`NEXUL Setup.exe` est un assistant d'installation complet, sans aucun prérequis. Il
installe NEXUL **pour votre compte**, sans droits d'administrateur, par défaut dans
`%LOCALAPPDATA%\Programs\NEXUL`.

| Composant | Contenu |
|---|---|
| Compilateur NEXUL *(requis)* | `nxc` et `nxpm`, avec la bibliothèque standard intégrée |
| Génération de code LLVM *(requis)* | `LLVM-C.dll` et l'éditeur de liens LLD (`lld-link`, `ld.lld`, `wasm-ld`, `ld64.lld`) |
| NEXUL Studio | l'environnement de développement et ses extensions d'exemple |
| Débogueur | LLDB (`lldb-dap`, `liblldb.dll`) et le moteur Python qu'il utilise |
| Exemples et documentation | `examples\`, `docs\` (guide, spécification, API des extensions) |

Dossier installé :

```
NEXUL\
  bin\        nxc.exe, nxpm.exe, NEXUL Studio.exe, plugins\
  llvm\       bin\ (LLVM, LLD, LLDB, python\)
  examples\   docs\
  Désinstaller NEXUL.exe   install.txt   nexul.ico
```

Options de l'assistant : ajout de `bin\` au **PATH** de l'utilisateur (avec la variable
`NEXUL_HOME`), raccourci sur le **Bureau**, dossier **NEXUL** dans le menu Démarrer
(Studio, guide, désinstallation), ouverture des fichiers **`.nx`** avec NEXUL Studio.
L'installation est inscrite dans *Paramètres → Applications installées*.

Chaque fichier est vérifié (SHA-256) après décompression : un programme d'installation
endommagé est détecté avant toute écriture incorrecte.

### Anciennes versions

Au démarrage, l'assistant recherche les versions précédentes de NEXUL :

- les installations faites par un programme d'installation de NEXUL (inscrites dans les
  applications installées, pour l'utilisateur ou la machine) ;
- les copies de NEXUL Studio ou de NEXUL dans `Program Files` et
  `%LOCALAPPDATA%\Programs` (par exemple les premières versions de NEXUL Studio, dans
  `C:\Program Files\NEXUL Studio`) ;
- tout ce qui désigne une version : entrées du PATH (utilisateur et système), raccourcis
  du Bureau et du menu Démarrer, association des fichiers `.nx`, variable `NEXUL_HOME`.

S'il en trouve, une étape **Anciennes versions** les présente, avec leur emplacement,
leur taille et ce qui s'y rattache, et vous demande pour **chacune** :

- **Supprimer** : ses fichiers sont effacés (pour une installation de NEXUL, seuls les
  fichiers qu'elle avait installés : vos fichiers ajoutés dans son dossier restent), et
  ses entrées du PATH, raccourcis, association et inscription sont retirés ;
- **Sauvegarder** : son dossier est déplacé dans
  `%USERPROFILE%\NEXUL - anciennes versions\<nom> <version> - <date>`, et elle est
  **détachée** du système : retirée du PATH, sans raccourci, sans association de fichiers
  ni inscription, et sans son programme de désinstallation. Elle ne peut donc pas
  interférer avec la nouvelle version ; un fichier `LISEZ-MOI - sauvegarde.txt` indique
  d'où elle vient. Pour s'en débarrasser plus tard, il suffit de supprimer son dossier.

Les éléments isolés (raccourci dont la cible n'existe plus, entrée du PATH vers une
version de développement, association des `.nx` vers un autre programme) se
**retirent** ou se **conservent**.

Une version placée dans `Program Files` ne peut être modifiée qu'avec les droits
d'administrateur : sa carte l'indique, et Windows demande votre accord au moment de la
traiter (seule cette opération est faite avec ces droits ; NEXUL reste installé pour votre
compte). Si vous refusez, l'installation continue et la version reste en place ; la page
finale le signale.

### Installation sans fenêtre

```
"NEXUL Setup.exe" --silent [--dir <dossier>] [--components core,llvm,studio,debugger,samples]
                  [--no-path] [--no-shortcuts] [--no-associate]
                  [--old-versions auto|remove|backup|keep]
```

- Sans `--components`, les composants proposés par défaut sont installés (tous).
- `--old-versions auto` (par défaut) remplace les installations précédentes faites par
  l'installateur et sauvegarde les autres copies ; `remove` supprime tout, `backup`
  sauvegarde toutes les versions, `keep` ne touche à rien.
- Code de sortie 0 en cas de succès, 1 en cas d'échec ; le message d'erreur (ou les
  avertissements) est écrit dans `%TEMP%\nexul-setup-error.txt`.

### Désinstaller

Depuis *Applications installées*, le menu Démarrer (*NEXUL → Désinstaller NEXUL*) ou
`Désinstaller NEXUL.exe` dans le dossier d'installation (`--silent` pour le faire sans
fenêtre). Sont retirés : les fichiers installés, l'entrée du PATH et `NEXUL_HOME`, les
raccourcis, l'association des fichiers `.nx` et l'inscription. Ne sont pas touchés : vos
projets, les réglages de NEXUL Studio (`%APPDATA%\NEXUL Studio`), le cache de compilation
et les paquets téléchargés (`%LOCALAPPDATA%\nexul`), les sauvegardes d'anciennes versions,
ni les fichiers que vous avez ajoutés dans le dossier d'installation (il est conservé s'il
n'est pas vide).

Avant de remplacer ou de retirer une installation, les programmes qui tournent **depuis son
dossier** (NEXUL Studio, `nxc`, le débogueur) sont arrêtés ; ceux d'une autre copie de
NEXUL ne sont pas touchés. Un programme ne pouvant pas s'effacer pendant qu'il tourne, le
désinstallateur laisse une copie de lui-même dans `%TEMP%`
(`nexul-uninstall-<numéro>.exe`). Cette copie attend sa fin, efface
`Désinstaller NEXUL.exe` et le dossier, puis est elle-même supprimée au prochain lancement
de l'installateur ou du désinstallateur.

## Depuis les sources

Prérequis : [Rust](https://rustup.rs) (stable) et [LLVM](https://llvm.org) 17 ou plus
récent, avec LLD. Sur Windows, l'installateur officiel de LLVM fournit `LLVM-C.dll` et
`lld.exe` dans `C:\Program Files\LLVM\bin` ; sur Linux, les paquets `llvm` et `lld` de la
distribution conviennent.

```
cargo build --release
target/release/nxc run examples/bonjour.nx
```

`nxc` trouve LLVM dans `NEXUL_LLVM`, dans un dossier `llvm` installé à côté de lui, puis
dans les emplacements habituels et le `PATH` (spécification §10.4).

## Construire `NEXUL Setup.exe`

```
python -I tools/build_installer.py ["chemin\NEXUL Setup.exe"]
```

Le script compile l'espace de travail en mode release, rassemble les composants dans
`target/installer-staging/` puis les empaquette avec `nexul-pack` (compression LZMS de
Windows, SHA-256 de chaque fichier). Il prend LLVM, LLD et LLDB dans `NEXUL_LLVM` ou
`C:\Program Files\LLVM`, et le moteur Python du débogueur dans la plus récente
installation de Python 3 (ou `NEXUL_PYTHON`) : seules la bibliothèque standard de Python
et ses DLL sont copiées (`tools/make-python-runtime.py`). Par défaut, l'installateur est
écrit sur le Bureau.

Le programme d'installation (`installer/`) est un exécutable Rust sans dépendance : il
affiche son interface (`installer/ui/index.html`) dans une fenêtre d'application de
Microsoft Edge, servie sur `127.0.0.1` avec un jeton aléatoire. Les données sont ajoutées
à la fin de l'exécutable ; sans elles, le même programme sert de désinstallateur.

Ses opérations sur le système passent directement par l'API de Windows
(`installer/src/win/`). Il ne lance ni PowerShell, ni `reg.exe`, ni `taskkill` : il
fonctionne donc aussi quand les scripts sont bloqués (stratégie d'exécution, mode de
langage restreint, AppLocker). Ces opérations sont :

- registre (inscription, association des `.nx`) ;
- PATH et `NEXUL_HOME` (forme brute conservée, programmes prévenus du changement) ;
- raccourcis (`IShellLinkW`) et dossiers connus (Bureau, menu Démarrer) ;
- choix du dossier (la boîte de l'Explorateur) ;
- droits d'administrateur pour les anciennes versions (`ShellExecuteEx`, verbe `runas`) ;
- arrêt des programmes d'un dossier.

Tests :

```
cargo test -p nexul-installer                                 # tests unitaires
cargo test -p nexul-installer --features test-sandbox         # + installation complète
cargo test -p nexul-installer -- --ignored picker             # choix du dossier (ouvre une fenêtre)
```

Le test `silent` construit un vrai installateur avec `nexul-pack`. Il l'installe, le met à
jour par-dessus une installation précédente et une ancienne copie de NEXUL Studio, puis le
désinstalle, et vérifie le résultat avec `reg.exe` et Windows Script Host. Tout cela se
passe dans un **bac à sable** : la fonctionnalité `test-sandbox` et la variable
`NEXUL_SETUP_SANDBOX` redirigent le registre sous `HKCU\Software\NEXUL-Tests\…` et le
Bureau et le menu Démarrer dans un dossier temporaire ; les réglages réels ne sont jamais
touchés. Cette redirection n'existe pas dans l'installateur livré.

## Évolutions à l'étude

Trois études préparent les prochaines versions de l'installation ; elles ne décrivent pas
encore le fonctionnement actuel :

- [Interface native de l'installateur](../installer/docs/etude-interface-native.md) :
  remplacer Edge par une interface Windows native (le retrait de PowerShell, étape 1, est
  fait), et livrer l'application native de NEXUL Studio ;
- [Distribution sous Linux et macOS](../installer/docs/etude-linux-macos.md) ;
- [Mécanisme de mise à jour](../installer/docs/etude-mises-a-jour.md).
