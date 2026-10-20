# Mesures à faire sur le matériel

[English](mesures.en.md) · Français

Quatre mesures demandées par l'audit du 20 octobre 2026 ([audit-2026-10-20.md](audit-2026-10-20.md), section 16, ligne « Vérifications acceptées »). Chacune se fait sur le matériel de l'auteur et tranche une décision : sans son résultat, le chantier correspondant reste en attente. Rien n'a encore été exécuté.

| # | Mesure | Où | Tranche |
|---|---|---|---|
| 1 | Latence d'affichage de l'app Mac | Mac, en session | Garde « une image en vol » ; plein écran présenté directement ou composé (§3) |
| 2 | Droits Windows | Ordinateur | Droits de `ProgramData` (§7, chantier 2) ; niveau d'intégrité de l'icône (§7) |
| 3 | Wi-Fi saturé | Mac en Wi-Fi, ordinateur, un troisième appareil | File de 8 Mio du correctif 0007 (§3, §8) |
| 4 | Modes de l'écran virtuel SudoVDA | Ordinateur, en session | Politique « suivre / remettre » (§15) |

Matériel de référence : un Mac M4 Max ; un ordinateur Windows avec une carte NVIDIA, à l'adresse 192.168.1.100 sur le réseau local, avec un écran 4K 160 Hz.

Chaque mesure a son tableau à remplir. Les mesures 1 et 3 ont les colonnes condition, p50, p95, pertes et remarque. Les mesures 2 et 4 ne mesurent pas de durées : leurs tableaux ont les colonnes qui conviennent.

## Avant de commencer

- Note la date, la version de l'app Mac (KyberNow › À propos de KyberNow) et celle de la console (page **Mises à jour**, ou `C:\Program Files\KyberNow\kybernow.json`).
- Une seule session à la fois. Ferme l'app Apple TV et le client web : un second appareil qui se connecte prend la main et coupe le premier.
- Pour les mesures de latence, prépare sur l'ordinateur une scène qui bouge sans arrêt et que tu relances à l'identique : une démo de jeu en boucle, ou un test d'animation en plein écran. Pas d'écran fixe : Kyber n'envoie rien quand l'image ne change pas (§15).
- Garde un minuteur sous la main (téléphone).
- Ce fichier reste vierge. Les tableaux remplis vont dans l'audit (voir « Où noter les résultats »).

---

## 1. Latence d'affichage de l'app Mac

**But.** Mesurer ce que coûte la garde « une image en vol » du rendu (`VideoRenderer`) et savoir si le plein écran est présenté directement ou composé par le WindowServer. D'après le §3 de l'audit, lever la garde gagne 4 à 8 ms à 120 Hz et 8 à 16 ms à 60 Hz ; une présentation directe gagne environ 8 ms à 120 Hz.

**Ce que l'app écrit.**
- `KN_PRESENT_STATS=1` : chaque seconde, une ligne dans le journal unifié (sous-système `fr.leslignesclaires.kybernow`, catégorie `video`) avec le p50 et le p95 de « presentedTime − commit » (le temps entre l'envoi de l'image à Metal et son apparition à l'écran), le nombre d'images écartées par Core Animation et le nombre d'images rendues.
- `KN_INFLIGHT_GUARD=1` : remet l'ancienne garde, pour comparer. Sans cette variable, la garde est levée.

### Préparer

1. Vérifie que ton app contient ces variables. Elles viennent d'une autre session de travail : tant que la commande suivante ne répond rien, il n'y a rien à mesurer.

```bash
cd ~/Sites/kyber-apps
grep -rn KN_PRESENT_STATS apps/macos/KyberNow apps/macos/KyberKit
```

2. Quitte KyberNow (⌘Q), puis recompile l'app de développement. `pgrep` ne doit rien afficher.

```bash
pgrep KyberNow
cd ~/Sites/kyber-apps/apps/macos
xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Debug -derivedDataPath build/dev build
```

3. Dans les réglages de l'ordinateur (app Mac) : profil **Jeu**, **synchro verticale activée** (sans elle, la garde ne joue aucun rôle), lecteur VLC désactivé (Réglages, Avancé : VLC n'utilise pas le rendu de l'app).
4. Sur l'ordinateur, lance la scène (voir « Avant de commencer »).

### Lancer l'app avec les variables

Ouvre deux fenêtres du Terminal.

Fenêtre 1, le journal :

```bash
log stream --predicate 'subsystem == "fr.leslignesclaires.kybernow" && category == "video"' --level info
```

Fenêtre 2, l'app. Lance l'une des deux commandes, jamais les deux :

```bash
cd ~/Sites/kyber-apps
APP="$PWD/apps/macos/build/dev/Build/Products/Debug/KyberNow.app"

# Sans garde (comportement par défaut)
KN_PRESENT_STATS=1 "$APP/Contents/MacOS/KyberNow"

# Avec l'ancienne garde (pour comparer)
KN_PRESENT_STATS=1 KN_INFLIGHT_GUARD=1 "$APP/Contents/MacOS/KyberNow"
```

L'app reste attachée à la fenêtre 2. ⌘Q ou ⌃C la quitte. Lance toujours le binaire comme ci-dessus : `open` ne transmet pas les variables.

Dès la première image de la session, une ligne par seconde apparaît dans la fenêtre 1 : p50, p95, écartées, rendues (le libellé exact est celui de l'app). Rien après 5 s : l'app n'a pas les variables (étape 1 de « Préparer »), ou le lecteur est VLC.

### Protocole

À faire d'abord à 120 Hz, puis à 60 Hz.

1. Règle l'écran du Mac : Réglages Système › Moniteurs › Taux de rafraîchissement. Sur un écran ProMotion, « ProMotion » suit le flux jusqu'à 120 Hz et « 60 Hertz » fixe 60 Hz. Sur un écran externe, prends 120 Hz puis 60 Hz.
2. Lance l'app **avec garde** (la seconde commande ci-dessus). Ouvre la session sur l'ordinateur en profil Jeu, puis passe en plein écran (⌃⌥⌘F). Barre et panneau fermés, souris immobile et loin du haut de l'écran.
3. Lance la scène sur l'ordinateur. Laisse tourner 10 s, puis compte **60 s** au minuteur sans toucher à rien.
4. Déconnecte-toi (⌃⌥⌘D), quitte l'app (⌘Q), puis arrête le journal (⌃C dans la fenêtre 1).
5. Garde les lignes de la mesure, pour les relire sans te presser :

```bash
log show --last 2m --info --style compact --predicate 'subsystem == "fr.leslignesclaires.kybernow" && category == "video"' > ~/Desktop/affichage-120hz-avec-garde.log
```

6. Relance l'app **sans garde** (la première commande ci-dessus) et refais les étapes 2 à 5. Change le nom du fichier.
7. Passe l'écran à 60 Hz et refais les étapes 2 à 6.

Si deux mesures identiques diffèrent de plus de 1 ms au p50, refais la paire.

### Ce qu'il faut noter

Dans le fichier de chaque mesure, prends les 60 lignes de la minute et note :
- **p50** : la valeur du milieu des 60 p50 ;
- **p95** : la valeur du milieu des 60 p95 (note le plus haut dans la remarque) ;
- **pertes** : le total des images écartées et le total des images rendues sur 60 s ;
- en remarque : la médiane de « Dernière session » sur l'accueil de l'app Mac, qui doit baisser de la même valeur que le p50.

| Condition | p50 (ms) | p95 (ms) | Pertes (écartées / rendues) | Remarque |
|---|---|---|---|---|
| 120 Hz, avec garde | | | | |
| 120 Hz, sans garde | | | | |
| Écart à 120 Hz (avec − sans) | | | | |
| 60 Hz, avec garde | | | | |
| 60 Hz, sans garde | | | | |
| Écart à 60 Hz (avec − sans) | | | | |

### Interpréter

- **Une baisse du p50 de 4 à 8 ms à 120 Hz (8 à 16 ms à 60 Hz) confirme l'audit.** Garde la garde levée, c'est le comportement par défaut.
- Une baisse de 1 à 4 ms est un vrai gain, plus petit que prévu : garde la garde levée si le p95 n'augmente pas.
- Une baisse de moins de 1 ms, ou une hausse : l'hypothèse de l'audit est fausse. Ne lève pas la garde pour la latence.
- Les images **écartées** et **rendues** montent sans la garde. Ce n'est pas une perte : le flux arrive à 160 images par seconde sur un écran à 120 Hz, Core Animation montre la plus récente (le panneau les appelle « Remplacées »). Un problème serait un p95 qui monte de plus que le p50 ne baisse, ou des gels visibles.

### Le HUD Metal

Il dit si la couche Metal de la session est présentée directement à l'écran ou composée par le WindowServer. D'après le §3, elle est composée, parce qu'une vue SwiftUI est posée par-dessus la couche pendant toute la session.

1. Quitte l'app. Relance-la avec le HUD (et les mesures, pour avoir le p50 de la même minute) :

```bash
MTL_HUD_ENABLED=1 KN_PRESENT_STATS=1 "$APP/Contents/MacOS/KyberNow"
```

2. Ouvre la session, passe en plein écran (⌃⌥⌘F), lance la scène. Le HUD s'affiche sur l'image (appareil, images par seconde, durée d'une image, temps GPU…).
3. Cherche dans le HUD la ligne qui dit comment la couche est présentée. Son libellé change avec la version de macOS : « Direct » ou « Composited » (composée), parfois une mention de la composition. Ce qui compte : **la couche est-elle présentée directement, sans passer par le compositeur ?** Si tu ne trouves aucune ligne de ce genre, fais une capture (⇧⌘3) et note-le en remarque.
4. **Barre fermée** : souris loin du haut de l'écran, panneau fermé. Attends 10 s, relève la ligne de présentation, fais une capture (⇧⌘3) et note le p50 et le p95 des 10 dernières lignes du journal.
5. **Barre ouverte** : amène la souris en haut de l'écran jusqu'à ce que la barre apparaisse, laisse-la 10 s, relève et fais une capture. Même relevé du journal.
6. Facultatif, **panneau des mesures ouvert** (⌃⌥⌘S), barre fermée : même relevé.

Le HUD coûte un peu de temps GPU : compare des mesures avec HUD à des mesures avec HUD, jamais à celles du tableau précédent.

| Condition | Ligne de présentation du HUD | p50 (ms) | p95 (ms) | Remarque |
|---|---|---|---|---|
| Barre fermée | | | | |
| Barre ouverte | | | | |
| Panneau des mesures ouvert (facultatif) | | | | |

**Interpréter.**
- « Composited » barre fermée : la composition du plein écran est confirmée. Le chantier du §3 (couche Metal = couche de la vue de contenu, barre et panneaux dans une fenêtre enfant ordonnée seulement quand elle est visible) est justifié. Son gain réel, environ un rafraîchissement, se mesurera après le chantier avec la même procédure.
- « Direct » barre fermée et « Composited » barre ouverte : la composition ne coûte que pendant que la barre ou un panneau est visible, le reste du temps le plein écran est déjà direct. Le chantier n'apporte rien de plus ; l'écart de p50 entre les deux lignes dit ce que coûte la barre ouverte.
- « Direct » partout : la composition n'est pas en cause. Abandonne ce chantier.

---

## 2. Droits Windows

**But.** Vérifier sur l'ordinateur ce que la phase 0 a livré (§17 de l'audit) : le dossier `C:\ProgramData\KyberNow` n'est plus accessible qu'aux administrateurs, et l'icône relancée par une mise à jour de la console ne tourne pas avec un jeton élevé (§7, point « à confirmer »).

### Le dossier de données

Ouvre PowerShell **en administrateur** (clic droit › Exécuter en tant qu'administrateur) sur l'ordinateur (ou depuis une session ouverte sur le Mac). N'ouvre jamais ce dossier avec l'Explorateur : devant un accès refusé, il propose « Continuer » et ajoute alors ton compte aux droits, ce qui fausse la mesure.

```powershell
icacls C:\ProgramData\KyberNow
icacls C:\ProgramData\KyberNow /T | Select-Object -First 40
```

**Attendu après la phase 0**, sur le dossier lui-même (les noms suivent la langue de Windows) :

```text
C:\ProgramData\KyberNow NT AUTHORITY\SYSTEM:(OI)(CI)(F)
                        BUILTIN\Administrateurs:(OI)(CI)(F)
```

Avec `/T`, les sous-dossiers et les fichiers donnent les mêmes deux lignes, marquées héritées : `(I)(OI)(CI)(F)` pour un dossier, `(I)(F)` pour un fichier. C'est correct.

La mesure échoue si une ligne cite `Utilisateurs` (`Users`), `Tout le monde` (`Everyone`), `Utilisateurs authentifiés` (`Authenticated Users`) ou `CREATEUR PROPRIETAIRE` (`CREATOR OWNER`), ou si un droit n'est pas `(F)`. Note alors le chemin concerné, puis `Restart-Service kyber-admin` (la console repose les droits à chaque démarrage) et recommence.

Trois vérifications de plus :

```powershell
(Get-Acl C:\ProgramData\KyberNow).Owner
(Get-Acl C:\ProgramData\KyberNow).AreAccessRulesProtected
Select-String -Path C:\ProgramData\KyberNow\kyber-admin.log -Pattern 'droits de' | Select-Object -Last 3
```

Attendu : le propriétaire est `BUILTIN\Administrateurs` (`BUILTIN\Administrators`) ; `True` ; au moins une ligne « droits de C:\ProgramData\KyberNow : SYSTEM et Administrateurs seulement », datée du dernier démarrage de la console.

### Depuis un compte standard

1. Ouvre une session Windows avec un compte standard (Paramètres › Comptes › Autres utilisateurs, si tu n'en as pas ; change d'utilisateur sans fermer l'autre).
2. Ouvre PowerShell (pas en administrateur) et lance :

```powershell
dir C:\ProgramData\KyberNow
New-Item -ItemType Directory C:\ProgramData\KyberNow\update\essai
```

**Attendu** : les deux commandes sont refusées (« Accès refusé » / « Access to the path … is denied »). Aucun nom de fichier ne s'affiche.
3. Dans la même session, clique sur l'icône KyberNow puis sur l'ouverture de la console : elle s'ouvre toujours.

Sans compte standard, le même `dir` dans une fenêtre PowerShell **non élevée** de ton compte administrateur doit aussi être refusé (le groupe Administrateurs y est en refus seul). La preuve est plus faible : elle ne couvre pas un autre compte.

### L'icône après une mise à jour

Une icône relancée par la console avec le jeton complet d'un administrateur tourne en haute intégrité. Le test n'a de sens que sur un compte **administrateur** : sur un compte standard, le jeton est déjà limité.

1. Avant la mise à jour, mesure l'icône lancée à l'ouverture de session (référence) avec l'un des outils ci-dessous : elle doit être en intégrité moyenne.
2. Lance une mise à jour depuis la console (page **Mises à jour › Installer**). S'il n'y a pas de version plus récente, sers un paquet depuis le Mac (voir [releases.md](releases.md), « Pour essayer un paquet servi depuis le Mac »). Attends environ une minute : l'icône réapparaît.
3. Vérifie que tu regardes l'icône relancée par la mise à jour, pas l'ancienne :

```powershell
Get-Process kybernow-tray | Select-Object Id,Path,StartTime
```

`StartTime` doit être celle de la fin de la mise à jour. Note l'`Id`.
4. Clique sur l'icône pour ouvrir son panneau (il lance des processus `msedgewebview2.exe` fils de l'icône).
5. Lis le niveau d'intégrité de `kybernow-tray.exe` (et de ses fils `msedgewebview2.exe`) avec l'un de ces outils :
   - **Process Explorer** (Sysinternals), lancé en administrateur. View › Select Columns, ajoute la colonne « Integrity Level ». Ou double-clique le processus, onglet Security : le groupe « Mandatory Label\… Mandatory Level ». C'est la méthode la plus sûre.
   - **Gestionnaire des tâches**, lancé en administrateur. Onglet Détails, clic droit sur un en-tête › Sélectionner les colonnes › « Élevé » (« Elevated » en anglais, « Niveau d'élévation » selon la version). La colonne dit « Oui » ou « Non » pour chaque processus.

**Attendu : intégrité moyenne (Medium), colonne « Élevé » à « Non ».** Une intégrité haute (High) ou « Oui » confirme le problème du §7 : la console relance l'icône avec le jeton complet (`launch_as_user`, `server/admin/src/display/agent.rs`, via `WTSQueryUserToken`). La correction est de lui donner le jeton limité lié (`TokenLinkedToken`).

| Condition | Attendu | Obtenu | Remarque |
|---|---|---|---|
| `icacls` sur le dossier | SYSTEM et Administrateurs, `(OI)(CI)(F)`, rien d'autre | | |
| `icacls /T`, 40 premières lignes | les mêmes deux noms, héritage `(I)` ; aucune ligne Utilisateurs / Users | | |
| Propriétaire, héritage protégé | Administrateurs ; `True` | | |
| Journal « droits de » | une ligne par démarrage de la console | | |
| Compte standard, `dir` | refusé | | |
| Compte standard, création d'un dossier | refusé | | |
| Compte standard, icône → console | s'ouvre | | |
| Icône à l'ouverture de session (référence) | intégrité moyenne | | |
| Icône après une mise à jour de la console | intégrité moyenne, « Élevé » : Non | | |
| Fils `msedgewebview2.exe` du panneau | intégrité moyenne | | |

---

## 3. Wi-Fi saturé

**But.** Savoir si la file de 8 Mio du correctif 0007 fait du bufferbloat quand le Wi-Fi sature, puis mesurer la politique « file courte » qui la remplacera. L'audit (§3, §8) estime que 0,7 à 2,2 s d'images périmées partent avant d'être jetées : 8 Mio, c'est 67 Mbit, soit 0,67 s à 100 Mbit/s et 2,2 s à 30 Mbit/s.

### Matériel et réglages

- Le Mac en Wi-Fi, au même point d'accès que l'ordinateur. L'ordinateur de préférence en Ethernet : le Wi-Fi est alors le seul goulot. Débranche l'Ethernet du Mac et coupe tout VPN.
- Un troisième appareil en Wi-Fi sur le même point d'accès (portable, téléphone) avec `iperf3`.
- Note le Wi-Fi du Mac : Option-clic sur l'icône Wi-Fi donne le canal, la bande, le débit de liaison et le signal.
- L'app Mac avec le lecteur de l'app (pas VLC), profil **Jeu**, définition 4K. Ouvre la session, lance la scène, passe en plein écran, puis ouvre le panneau des **mesures détaillées** (⌃⌥⌘S).

### Préparer la saturation

Sur l'ordinateur, dans PowerShell :

```powershell
iperf3 -s
```

Autorise `iperf3` dans le pare-feu (réseau privé) à la première question. Retire cette règle quand tu as fini.

Sur le troisième appareil, **sans session ouverte**, vérifie d'abord que le lien sature :

```bash
iperf3 -c 192.168.1.100 -u -b 300M -t 10 -R
```

`-R` inverse le sens : l'ordinateur envoie, comme pour l'image. Lis le résumé. Si le débit reçu est nettement sous 300 Mbit/s ou si des datagrammes sont perdus, le lien est saturé. Sinon monte `-b` (500M, 800M) jusqu'à y arriver, et note la valeur retenue. À défaut d'iperf3, un gros téléchargement lancé depuis le troisième appareil fait l'affaire, en moins reproductible.

### Protocole

Une session, trois phases de 60 s, enchaînées sans fermer la session.

1. **Avant (60 s).** Session ouverte depuis au moins 30 s, scène en cours, panneau ouvert, aucun trafic ailleurs. Toutes les 10 s, relève le tableau ci-dessous. Au début de la phase, note la ligne « Session : N images, M paquets » de la zone Pertes. Reprends-la à la fin de la phase.
2. **Pendant (60 s).** Lance sur le troisième appareil :

```bash
iperf3 -c 192.168.1.100 -u -b 300M -t 60 -R
```

Mêmes relevés toutes les 10 s, et le compteur de session au début et à la fin.
3. **Après (60 s).** Dès la fin d'iperf3. Mêmes relevés. Note aussi le temps que met la latence à revenir à moins de 2 ms au-dessus de la valeur d'avant.

À chaque relevé de 10 s, note : la **latence** (le grand nombre en haut du panneau, médiane de la dernière seconde), la ligne **Réseau** de la cascade et l'**aller-retour** (zone Détails). À 30 s et à 60 s de chaque phase, note aussi les quatre cases de la zone « Pertes · 30 s » : « Images perdues », « Paquets perdus », « Images clés » (images clés redemandées) et « Remplacées ». Ces cases couvrent 30 s glissantes : les deux relevés se suivent sans se recouvrir. Les images clés d'une phase sont leur somme.

| Phase | 10 s | 20 s | 30 s | 40 s | 50 s | 60 s |
|---|---|---|---|---|---|---|
| Latence, avant (ms) | | | | | | |
| Latence, pendant (ms) | | | | | | |
| Latence, après (ms) | | | | | | |
| Aller-retour, avant (ms) | | | | | | |
| Aller-retour, pendant (ms) | | | | | | |
| Aller-retour, après (ms) | | | | | | |

Pour le tableau final :
- **p50** : la valeur du milieu des 6 relevés de latence de la phase ;
- **p95** : le plus haut des 6 relevés (six relevés ne permettent pas mieux) ; le p95 de toute la session est sur l'accueil de l'app, sous « Dernière session », une fois la session fermée ;
- **pertes** : la différence du compteur « Session » entre le début et la fin de la phase (images et paquets), puis les images clés.

### Les deux séries

Fais la série complète avec :
1. **la version 0.2.5 publiée** (file de 8 Mio), celle que l'ordinateur a sans doute aujourd'hui (vérifie dans « Applications installées »). Pour la réinstaller si besoin, en PowerShell administrateur :

```powershell
$env:KYBERNOW_UPDATE_BASE = 'https://github.com/MonsieurNours/kybernow/releases/download/v0.2.5'
irm https://github.com/MonsieurNours/kybernow/releases/download/v0.2.5/update.ps1 | iex
```

2. **la nouvelle version avec la politique « file courte »**, quand elle sera disponible. Elle se règle dans `kyber_config.toml` : le nom du réglage et ses valeurs seront dans [kyber-changes.md](kyber-changes.md). Installe la version, applique le réglage, relance le service comme indiqué là, et note la valeur dans la remarque. L'app Mac reste la même pour les deux séries.

| Condition | p50 (ms) | p95 (ms) | Pertes (images / paquets / images clés) | Remarque |
|---|---|---|---|---|
| 0.2.5, avant | | | | |
| 0.2.5, pendant | | | | |
| 0.2.5, après | | | | |
| File courte, avant | | | | |
| File courte, pendant | | | | |
| File courte, après | | | | |

En remarque : le débit `-b` retenu et le débit reçu par iperf3, le Wi-Fi du Mac, le débit du flux (courbe « Débit »), le temps de retour à la normale après la saturation, et tout événement (bandeau de repli, image figée, session coupée).

### Interpréter

- **Si la latence médiane « pendant » dépasse de plusieurs centaines de millisecondes celle d'« avant », la file profonde fait du bufferbloat.** C'est ce que prévoit l'audit. Une latence qui reste haute longtemps « après » est la même signature : la file se vide au rythme du lien.
- Avec la file courte, la latence « pendant » doit rester proche de celle d'« avant ». Le prix est plus de pertes et d'images clés redemandées : compare-les. À titre indicatif, la file courte est la bonne si la latence « pendant » reste à moins de 100 ms de celle d'« avant » sans gel d'image de plus d'une seconde.
- Pas d'écart entre 0.2.5 et l'« avant » : le lien n'était pas saturé pour le flux. Monte `-b` ou rapproche le Mac de la limite du Wi-Fi.

---

## 4. Modes de l'écran virtuel SudoVDA

**But.** Savoir quels modes l'écran virtuel expose : un seul, ou une liste. Cela décide si un jeu peut changer la définition de l'écran virtuel, donc si le scénario « le mode de l'écran capturé change » du §15 existe sur l'écran virtuel.

Ne change aucun mode pendant les relevés : la session peut s'arrêter selon le moteur de capture (§15).

### Dans les Paramètres de Windows

1. Ouvre une session depuis le Mac, l'écran virtuel en place sur l'ordinateur. Tu peux tout faire depuis la fenêtre de l'ordinateur sur le Mac.
2. Sur l'ordinateur : Paramètres › Système › Affichage. Sélectionne l'écran virtuel en haut de la page (si les écrans physiques sont éteints, il est seul ; son moniteur se nomme sans doute « KyberNow »).
3. Descends à **Résolution d'affichage** et déroule la liste. Note les modes proposés : un seul, ou plusieurs (fais une capture avec ⇧⌘4 sur le Mac). Ferme la liste avec Échap, sans rien choisir.
4. Ouvre **Affichage avancé** pour cet écran. Déroule **Choisir une fréquence d'actualisation** et note les fréquences. Même règle : ne choisis rien.
5. Facultatif : Affichage avancé › Propriétés de l'adaptateur d'affichage › **Lister tous les modes** donne la liste complète du pilote.

### Avec un script PowerShell

Lance-le dans une fenêtre PowerShell de la session de l'utilisateur connecté (pas dans un service ni une connexion à distance : la liste y serait vide). Il n'a pas besoin d'être administrateur. Colle le bloc entier.

```powershell
# Liste les modes de chaque ecran avec EnumDisplaySettingsEx.
$source = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class KnDisplay
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODE
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public ushort dmSpecVersion;
        public ushort dmDriverVersion;
        public ushort dmSize;
        public ushort dmDriverExtra;
        public uint dmFields;
        public int dmPositionX;
        public int dmPositionY;
        public uint dmDisplayOrientation;
        public uint dmDisplayFixedOutput;
        public short dmColor;
        public short dmDuplex;
        public short dmYResolution;
        public short dmTTOption;
        public short dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public ushort dmLogPixels;
        public uint dmBitsPerPel;
        public uint dmPelsWidth;
        public uint dmPelsHeight;
        public uint dmDisplayFlags;
        public uint dmDisplayFrequency;
        public uint dmICMMethod;
        public uint dmICMIntent;
        public uint dmMediaType;
        public uint dmDitherType;
        public uint dmReserved1;
        public uint dmReserved2;
        public uint dmPanningWidth;
        public uint dmPanningHeight;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DISPLAY_DEVICE
    {
        public uint cb;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceString;
        public uint StateFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceID;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceKey;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplayDevices(string lpDevice, uint iDevNum, ref DISPLAY_DEVICE lpDisplayDevice, uint dwFlags);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplaySettingsEx(string lpszDeviceName, int iModeNum, ref DEVMODE lpDevMode, uint dwFlags);

    static DEVMODE NewMode()
    {
        DEVMODE dm = new DEVMODE();
        dm.dmSize = (ushort)Marshal.SizeOf(typeof(DEVMODE));
        return dm;
    }

    static string Describe(DEVMODE dm)
    {
        return dm.dmPelsWidth.ToString() + " x " + dm.dmPelsHeight.ToString()
            + " @ " + dm.dmDisplayFrequency.ToString() + " Hz";
    }

    public static string[] Report(uint flags)
    {
        List<string> lines = new List<string>();
        for (uint i = 0; ; i++)
        {
            DISPLAY_DEVICE adapter = new DISPLAY_DEVICE();
            adapter.cb = (uint)Marshal.SizeOf(typeof(DISPLAY_DEVICE));
            if (!EnumDisplayDevices(null, i, ref adapter, 0)) break;

            string monitorName = "(aucun)";
            DISPLAY_DEVICE monitor = new DISPLAY_DEVICE();
            monitor.cb = (uint)Marshal.SizeOf(typeof(DISPLAY_DEVICE));
            if (EnumDisplayDevices(adapter.DeviceName, 0, ref monitor, 0)) monitorName = monitor.DeviceString;

            bool attached = (adapter.StateFlags & 0x1) != 0;
            bool primary = (adapter.StateFlags & 0x4) != 0;
            lines.Add("");
            lines.Add(adapter.DeviceName + " | " + adapter.DeviceString + " | ecran : " + monitorName
                + (attached ? " | actif" : " | inactif") + (primary ? " | principal" : ""));

            DEVMODE current = NewMode();
            if (EnumDisplaySettingsEx(adapter.DeviceName, -1, ref current, 0))
                lines.Add("  mode courant : " + Describe(current));

            List<string> modes = new List<string>();
            for (int n = 0; ; n++)
            {
                DEVMODE dm = NewMode();
                if (!EnumDisplaySettingsEx(adapter.DeviceName, n, ref dm, flags)) break;
                string mode = Describe(dm);
                if (!modes.Contains(mode)) modes.Add(mode);
            }
            lines.Add("  " + modes.Count.ToString() + " mode(s) :");
            foreach (string line in modes) lines.Add("    " + line);
        }
        return lines.ToArray();
    }
}
'@

if (-not ('KnDisplay' -as [type])) { Add-Type -TypeDefinition $source }

'=== Modes proposes par le pilote ==='
[KnDisplay]::Report([uint32]0)

'=== Modes bruts (EDS_RAWMODE), meme ceux que le moniteur ne declare pas ==='
[KnDisplay]::Report([uint32]2)
```

Il écrit, pour chaque `\\.\DISPLAYn` : la carte, le moniteur, s'il est actif, son mode courant, puis la liste des modes en `largeur x hauteur @ fréquence Hz` (sans doublon, toutes profondeurs de couleur confondues). L'écran virtuel est celui dont le moniteur se nomme « KyberNow » ou dont la carte est celle de SudoVDA ; à défaut, celui dont le mode courant est celui de la session. Un écran inactif peut afficher « 0 mode(s) ».

| Source | Modes relevés | Nombre | Remarque |
|---|---|---|---|
| Paramètres › Résolution d'affichage | | | |
| Paramètres › Affichage avancé › fréquences | | | |
| Script, modes proposés par le pilote | | | |
| Script, modes bruts | | | |
| « Lister tous les modes » (facultatif) | | | |

### Interpréter

- **Un seul mode** (celui que la console a demandé à la création de l'écran) : un jeu ne peut pas changer la définition de l'écran virtuel. Le scénario « le mode change » du §15 n'existe alors que sur les écrans physiques. La politique « suivre / remettre » de la console n'a pas d'objet pour l'écran virtuel : ne la construis pas.
- **Une liste** : un jeu peut changer la définition de l'écran virtuel. Le scénario du §15 est réel : l'image continue à la nouvelle taille (moteur txproto) ou la session coupe (moteur libavconv), et la politique « suivre / remettre » (§15 : laisser le jeu choisir le mode, ou le figer) a un sens. Note si la liste contient la définition de ton écran Mac et la 4K, et plusieurs fréquences pour une même taille.
- Les modes bruts plus nombreux que les modes proposés : le moniteur virtuel en cache. Note-les, un jeu qui énumère les modes comme le script verra la liste du pilote.

---

## Où noter les résultats

Dans [audit-2026-10-20.md](audit-2026-10-20.md), **section 16** (« Décisions prises le 20 octobre 2026 »), ligne « Vérifications acceptées », dont la fin dit « Procédures à préparer » : ces procédures sont ce fichier. Sous la table de la section 16, ajoute un sous-titre par mesure avec la date, les versions, le tableau rempli et la décision prise. Fais ensuite pointer la ligne vers ces sous-titres.

| Mesure | Qui lit le résultat dans l'audit | Décision à écrire |
|---|---|---|
| 1. Latence d'affichage | §3, lignes « Affichage » et « Composition du plein écran » | Garde levée ou remise ; chantier de la couche Metal directe lancé ou abandonné |
| 2. Droits Windows | §7 (ACL, icône en haute intégrité) ; §17, liste « Mise à jour et installation » | Phase 0 validée ou reprise ; correction `TokenLinkedToken` à faire ou non |
| 3. Wi-Fi saturé | §3, ligne « Envoi (serveur) » ; §8, correctif 0007 | Politique « file courte » par défaut ou non, et sa valeur |
| 4. Modes SudoVDA | §15, « Questions propres à ce sujet » et « Choix de politique » | Un ou plusieurs modes ; politique « suivre / remettre » construite ou non |
