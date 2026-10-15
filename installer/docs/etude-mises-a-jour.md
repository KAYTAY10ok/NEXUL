# Étude — mécanisme de mise à jour de NEXUL

Claude Outils, 15 octobre 2026. Statut : **étude**, aucune modification de code.
Décisions demandées à la fin (§8).

## 1. Point de départ

- Les versions sont publiées sur **GitHub Releases** (`v0.2.0` avec `NEXUL-Setup.exe`).
- Pour mettre à jour, il faut aujourd'hui retélécharger l'installateur. Celui-ci remplace
  l'installation précédente : les fichiers listés dans `install.txt` sont supprimés,
  puis les nouveaux écrits (`installer/src/old.rs`).
- Le dépôt n'a **aucun code réseau** : `nxpm` télécharge les paquets avec `git`.
- L'installateur vérifie déjà le **SHA-256** de chaque fichier (manifeste de
  `payload.rs`). Il n'existe en revanche ni signature de code ni signature des
  publications.

## 2. Exigences

1. **Prévenir** qu'une version existe, et l'installer en un clic depuis Studio.
2. **Sûreté** : n'exécuter qu'un installateur publié par le projet, même si le
   téléchargement est détourné.
3. **Ne rien casser** : une mise à jour interrompue laisse la version en place
   utilisable ; retour arrière possible.
4. **Discrétion** : aucune télémétrie ; désactivable par l'utilisateur et par une
   politique d'entreprise ; jamais en intégration continue.
5. **Respect des gestionnaires de paquets** : une installation par `apt`, `dnf`,
   Homebrew ou `winget` se met à jour par eux.
6. **Canaux** : stable, et éventuellement préversion.

## 3. Menaces (d'après le modèle de TUF)

| Menace | Parade |
|---|---|
| Téléchargement modifié (miroir, proxy, compte GitHub compromis) | manifeste **signé** (Ed25519) par une clé gardée hors ligne ; chaque fichier vérifié par le SHA-256 que donne le manifeste signé |
| Retour à une ancienne version vulnérable (*rollback*) | refuser toute version inférieure ou égale à celle installée, et tout manifeste plus ancien que le dernier vu |
| Gel des mises à jour (*freeze*) : servir indéfiniment un vieux manifeste | champ `expires` (30 jours au plus) ; un manifeste périmé est ignoré et signalé |
| Clé de signature compromise | clé hors ligne ; liste `keys` permettant une rotation signée par la clé courante ; procédure d'urgence (nouvel installateur, nouvelle clé) |
| Téléchargement tronqué | taille et SHA-256 vérifiés avant toute exécution |

## 4. Options

| Option | Pour | Contre |
|---|---|---|
| **A. Mécanisme propre, bâti sur l'installateur existant** | réutilise la gestion des anciennes versions, le SHA-256 et le mode `--silent` ; aucune dépendance imposée | à écrire et à sécuriser |
| B. Velopack (Rust, multiplateforme, mises à jour différentielles, [docs][velopack]) | mûr, différentiel | impose sa propre disposition et son propre installateur (`Update.exe`) : il faudrait abandonner le nôtre et sa gestion des anciennes versions ; dépendance importante |
| C. MSIX avec mise à jour automatique (App Installer) | intégré à Windows | exige un certificat de confiance ; une application MSIX ne modifie pas le PATH (seulement des alias d'exécution) : mauvais choix pour une chaîne d'outils en ligne de commande |
| D. Gestionnaires seulement (`winget`, Homebrew, `apt`) | rien à écrire côté client | pas de mise à jour depuis Studio ; dépend de la soumission aux dépôts |
| E. Sparkle (macOS) | standard sur Mac | macOS uniquement, Objective-C |

**Recommandation : A**, complétée par **D** comme canal de distribution. Le mode
`--silent` permet déjà `winget install` et `winget upgrade` avec un manifeste soumis à
`winget-pkgs`.

## 5. Conception recommandée

### 5.1 Côté publication

Chaque publication sur GitHub ajoute deux fichiers. Le premier, `nexul-release.json` :

```json
{
  "schema": 1,
  "channel": "stable",
  "version": "0.3.0",
  "published": "2026-11-02T10:00:00Z",
  "expires": "2026-12-02T10:00:00Z",
  "notes": "https://github.com/KAYTAY10ok/NEXUL/releases/tag/v0.3.0",
  "assets": [
    { "os": "windows", "arch": "x86_64", "kind": "installer",
      "url": "https://github.com/KAYTAY10ok/NEXUL/releases/download/v0.3.0/NEXUL-Setup.exe",
      "size": 123456789, "sha256": "…" },
    { "os": "linux", "arch": "x86_64", "kind": "tarball", "url": "…", "size": 0, "sha256": "…" }
  ],
  "keys": ["<clé publique courante>", "<clé suivante, optionnelle>"]
}
```

- `nexul-release.json.minisig` contient la signature Ed25519 du fichier, au format
  [minisign][minisign] : un outil répandu, utilisable par le mainteneur sans
  développement.
- **Adresse stable** : `https://github.com/KAYTAY10ok/NEXUL/releases/latest/download/nexul-release.json`.
  GitHub redirige vers la dernière publication non préliminaire. Le canal préversion
  utilise une publication mobile (`preview`) mise à jour à chaque préversion.
- **Signature** : la clé privée reste **hors ligne**, chez le mainteneur, qui signe le
  manifeste avec `minisign` après la construction. L'intégration continue ne voit jamais
  la clé, au prix d'une étape manuelle à chaque publication. La clé publique est
  intégrée au programme de mise à jour.

### 5.2 Côté poste : le programme de mise à jour

L'installateur sans données sert déjà de désinstallateur (`Désinstaller NEXUL.exe`). Le
même programme, installé aussi sous `bin\nexul-update.exe` (lien physique, ou copie
d'environ 1 Mo), gagne deux modes :

- `nexul-update --check [--json]` : télécharge le manifeste et sa signature, vérifie la
  signature, l'expiration et le canal, compare les versions, puis répond « à jour » ou
  « 0.3.0 disponible (notes : …) » ;
- `nexul-update --apply` : télécharge l'installateur dans
  `%LOCALAPPDATA%\nexul\updates`, vérifie sa taille, son SHA-256 et, quand les
  publications seront signées, sa signature Authenticode (`WinVerifyTrust`). Il le lance
  ensuite en mode silencieux (§5.4).

**Studio** (Claude Studio) appelle `--check` en arrière-plan au plus une fois par jour.
Il affiche « NEXUL 0.3.0 est disponible » avec un lien vers les notes et un bouton
« Mettre à jour », qui appelle `--apply` puis propose de redémarrer.

Réglage `update.mode` = `notify` (par défaut), `off`, ou plus tard `auto`. La
vérification est désactivée si l'une des conditions suivantes est vraie :
- la variable `NEXUL_NO_UPDATE_CHECK` est définie ;
- la valeur de politique `HKCU\Software\Policies\NEXUL\DisableUpdateCheck` (ou `HKLM`)
  est présente ;
- la variable `CI` est présente.

La seule requête réseau est le téléchargement du manifeste, sans identifiant. GitHub
voit l'adresse IP, et la documentation le dira.

### 5.3 Réseau et cryptographie

- **Windows** : WinHTTP par appels directs à l'API système (`winhttp.dll`) : TLS du
  système, proxy de l'utilisateur, **aucune dépendance**.
- **Linux et macOS** : la commande `curl` (toujours présente sur macOS, presque toujours
  sur les distributions de bureau). Une bibliothèque TLS en Rust serait une dépendance.
- **Ed25519** : vérification par une crate auditée (par exemple `ed25519-dalek`), plutôt
  que de la cryptographie écrite à la main. `installer/` étant dans le workspace racine,
  c'est une **demande à Codex Astra** (règle « pas de dépendance sans lui »). La feuille
  de route prévoit d'ailleurs un `std.crypto` adossé à des implémentations auditées.

### 5.4 Appliquer une mise à jour sans rien casser

**Étape 1, disposition actuelle** : `NEXUL Setup.exe --silent --dir <dossier actuel> --old-versions auto`.
Cela fonctionne dès aujourd'hui, avec deux défauts :
- Studio doit être fermé, car Windows verrouille les exécutables en cours ;
- une panne au milieu, après la suppression de l'ancienne version, laisse une
  installation incomplète.

**Étape 2, un dossier par version** (même principe que sous Linux, voir
`etude-linux-macos.md` §2.1) :

```
%LOCALAPPDATA%\Programs\NEXUL\
  versions\0.2.0\   versions\0.3.0\     (bin\, llvm\, examples\… de chaque version)
  current\          jonction NTFS vers versions\0.3.0 (pas de droits d'administrateur)
```

Le PATH (`current\bin`), les raccourcis et l'association `.nx` désignent `current` : ils
ne changent pas d'une version à l'autre.

- La nouvelle version s'installe **à côté** de l'ancienne, pendant que Studio tourne ;
  la bascule de `current` intervient au redémarrage.
- **Retour arrière** : `nexul-update --rollback` remet `current` sur la version
  précédente, conservée jusqu'à la mise à jour suivante.
- **Garantie** : la nouvelle version n'est activée qu'après la vérification de tous ses
  fichiers.
- **Point à étudier** : il faut s'assurer que `nxc` trouve bien son `llvm/` à travers la
  jonction :
  `current_exe()` renvoie le chemin sans résoudre la jonction, et `toolchain.rs` cherche
  `llvm` à côté de l'exécutable, donc cela devrait fonctionner, à vérifier par un test.

Sous macOS, le paquet `.app` est remplacé en une fois : téléchargement, vérification,
copie à côté, puis échange. Une installation par Homebrew, `apt` ou `dnf` dépose un
fichier `install-source` qui **désactive** le programme de mise à jour (« mettez à jour
avec brew »).

### 5.5 Plus tard : ne télécharger que ce qui change

L'installateur est découpé en composants (`core`, `llvm`, `studio`, `debugger`,
`samples`). LLVM et le débogueur, les plus lourds, changent rarement. Une publication
peut proposer ses composants séparément, avec le SHA-256 de chaque fichier, comme dans
`payload.rs`. Il suffit alors que `install.txt` enregistre aussi le hachage de chaque
fichier installé : le programme de mise à jour ne télécharge que les composants dont un
fichier change, et reprend les autres de la version courante par liens physiques, sur le
même volume. C'est un gain proche du différentiel, sans format binaire de différences.

## 6. Tests prévus

- Serveur HTTP local de test (l'installateur en contient déjà un, `http.rs`) qui sert un
  manifeste et des fichiers.
- Cas vérifiés :
  - signature invalide ou faite avec une autre clé ;
  - SHA-256 faux ;
  - fichier tronqué ;
  - version inférieure ou égale ;
  - manifeste périmé ;
  - canal différent ;
  - rotation de clé valide et invalide ;
  - absence de réseau, sans erreur visible au démarrage de Studio.
- Bascule et retour arrière dans un dossier temporaire (règle 8 : ni le vrai
  `%LOCALAPPDATA%`, ni le vrai registre).
- Bout en bout : installer 0.2.x de test, publier un 0.2.y de test sur le serveur local,
  `--check`, `--apply`, vérifier `current` et le fonctionnement de `nxc`.

## 7. Plan

| Étape | Contenu | Dépend de |
|---|---|---|
| U1 | Clé minisign (créée par l'utilisateur), `nexul-release.json` et sa signature publiés avec la prochaine version | décision §8 ; intégration continue de Codex Astra pour l'automatiser |
| U2 | `nexul-update --check` et `--apply` (étape 1 du §5.4), WinHTTP, vérification Ed25519 | dépendance Ed25519 acceptée par Codex Astra |
| U3 | Notification et bouton dans Studio | Claude Studio |
| U4 | Un dossier par version, bascule, retour arrière (étape 2 du §5.4) | U2 ; essais de `nxc` à travers la jonction |
| U5 | Manifeste `winget-pkgs` (puis Homebrew) | une publication stable avec adresse fixe |
| U6 | Téléchargement par composant (§5.5) | U4 |

La signature Authenticode de l'installateur Windows est indépendante de ce plan, mais
elle évite l'avertissement SmartScreen. Depuis 2024, même un certificat EV ne donne plus
de réputation immédiate. Le service Azure Artifact Signing (ex-Trusted Signing),
disponible depuis janvier 2026, n'est ouvert aux **personnes** qu'aux États-Unis et au
Canada ; ailleurs, il faut un certificat OV ou passer par une organisation
([Microsoft Learn][ms-signing], [devclass][devclass]).

## 8. Décisions demandées

1. **Adopter l'option A** (mécanisme propre sur l'installateur) et publier sur `winget` ?
2. **Clé de signature** : l'utilisateur crée et garde la clé minisign hors ligne, et
   signe chaque publication à la main ?
3. **Comportement par défaut** : `notify` (prévenir, ne rien installer seul) ?
4. **Dépendance Ed25519** dans `installer/` : à demander à Codex Astra ?
5. **Signature Authenticode** : envisagée (certificat OV, ou Azure Artifact Signing selon
   le pays et le statut de l'éditeur) ?

## Sources

- [Velopack — signature et plates-formes][velopack]
- [minisign][minisign]
- [Microsoft Learn — options de signature de code et SmartScreen][ms-signing]
- [devclass — Azure Artifact Signing, janvier 2026][devclass]
- [The Update Framework (TUF)][tuf]

[velopack]: https://docs.velopack.io/packaging/signing
[minisign]: https://jedisct1.github.io/minisign/
[ms-signing]: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
[devclass]: https://devclass.com/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/
[tuf]: https://theupdateframework.io/
