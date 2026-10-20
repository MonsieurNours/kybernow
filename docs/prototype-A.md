# Prototype A — Kyber 0.29 de bout en bout (PC Windows → Mac)

[English](prototype-A.en.md) · Français

Objectif : un serveur Kyber qui tourne sur le PC NVIDIA, le client macOS en ligne de commande qui s'y connecte, et une **mesure de latence de référence** avant d'écrire la moindre ligne de notre code.

## 1. Côté PC Windows

### 1.1 Préparer la machine
- Windows 10 ou 11, **pilote NVIDIA à jour** (GeForce Experience / NVIDIA App ou nvidia.com).
- **Câble Ethernet** pour le PC (et si possible pour le Mac) pendant les mesures ; le Wi-Fi viendra après, en comparaison.
- Un **écran branché** (ou un bouchon HDMI/DisplayPort) : le pilote d'écran virtuel de Kyber n'est pas encore publié, sans écran il n'y a rien à capturer.
- Noter l'adresse IP du PC : `ipconfig` dans un terminal → « Adresse IPv4 » (ex. `192.168.1.42`). Idéalement, lui réserver cette IP dans la box.

### 1.2 Installer Kyber 0.29.0
1. Télécharger l'installeur officiel :
   https://gitlab.com/api/v4/projects/78083474/packages/generic/kyber-installer/0.29.0/kyber-installer-win64-0.29.0.exe
2. L'exécuter en administrateur. S'il n'est pas signé, SmartScreen affiche un avertissement : « Informations complémentaires » → « Exécuter quand même ».
3. Dans l'assistant :
   - **Encoder** : `NVENC`
   - **TLS** : `Generate` (certificat auto-signé, suffisant pour le LAN)
   - **Authentication** : `Basic`, avec un identifiant et un mot de passe de ton choix (à garder pour le client)
   - **Startup** : `Windows service`
4. Installer le pilote manette **ViGEmBus** (nécessaire pour les manettes) : https://github.com/nefarius/ViGEmBus/releases (dernière version, `.exe`).

### 1.3 Réglages de performance
La doc Kyber mesure 2,5 ms d'encodage NVENC en 1080p avec les bons réglages, contre 8 ms quand le GPU économise de l'énergie.
- **Panneau de configuration NVIDIA** → Gérer les paramètres 3D → Mode de gestion de l'alimentation → **Performances maximales préférées**.
- **Plan d'alimentation Windows** : dans un PowerShell administrateur :
  ```powershell
  powercfg /setactive SCHEME_MIN
  ```
  (« Performances élevées » ; à remettre ensuite avec `powercfg /setactive SCHEME_BALANCED` si besoin).
- Résolution et fréquence de l'écran du PC : commencer en **1920×1080 à 60 Hz** pour la référence.

### 1.4 Vérifier
Dans un PowerShell :
```powershell
sc query kyber                                   # STATE : RUNNING
Get-NetFirewallRule -DisplayName *kyber* | Format-Table DisplayName, Enabled, Direction, Action
```
Si aucune règle de pare-feu n'apparaît, les ajouter (PowerShell administrateur) :
```powershell
New-NetFirewallRule -DisplayName "Kyber TCP 8080" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private
New-NetFirewallRule -DisplayName "Kyber UDP 8080" -Direction Inbound -Protocol UDP -LocalPort 8080 -Action Allow -Profile Private
```
Vérifier aussi que le réseau Windows est bien en profil **Privé** (Paramètres → Réseau → propriétés de la connexion).

Fichiers utiles :
- Config : `C:\Program Files\Kyber\kyber_config.toml` (redémarrer le service après modification : `net stop kyber` puis `net start kyber`)
- Logs : `C:\Program Files\Kyber\log\`

### 1.5 Premier test sans client natif
Depuis **Chrome** sur le Mac (pas Safari : écran noir) : `https://<IP-du-PC>:8080/webclient/` → accepter l'avertissement de certificat → se connecter avec l'identifiant Basic. Si l'image du PC s'affiche, le serveur est bon.

✅ Validé le 2026-10-19 : PC `192.168.1.100`, image reçue dans Chrome.

## 2. Côté Mac (client natif)

Le client est compilé depuis les sources (`third_party/kyber-desktop`, tag `0.29.0`, arm64) : `third_party/kyber-desktop/Kyber-Client.app` (voir `docs/build-notes.md` pour le contournement Xcode 27).

```bash
tools/latency/run_client.sh 192.168.1.100 matthieu            # options kyclient en plus si besoin
```
Le lanceur demande le mot de passe sans l'afficher, se connecte avec `--tls-tofu` (confiance au certificat du PC à la première connexion, comme SSH) et `--metrics true`, puis analyse la session à la fermeture. Chaque session est rangée dans `tools/latency/runs/<date>/` (`metrics.json`, `client.log`, `report.txt`). Raccourci : `Ctrl+Alt+F` pour le plein écran.

## 3. Mesurer la latence

### 3.1 Latence pipeline (automatique)
Le client horodate chaque image à chaque étape (capture → encodage → envoi → réception → décodage → affichage) et recale l'horloge du PC.
1. Sur le PC, ouvrir `tools/latency/timer.html` dans un navigateur en plein écran (F11) : il produit une image qui change en permanence, sinon la capture n'envoie que peu d'images.
2. Lancer le client avec `--metrics true`, laisser tourner **60 s**, fermer.
3. Analyser :
   ```bash
   python3 tools/latency/analyze_metrics.py metrics.json
   ```
   On obtient p50 / p95 / p99 par étape et au total.

### 3.2 Latence « de l'écran du PC à l'écran du Mac » (caméra)
La mesure automatique n'inclut ni le délai de rendu avant la capture, ni l'affichage par l'écran du Mac.
1. Le chronomètre plein écran sur le PC, le client Kyber plein écran sur le Mac, les deux écrans côte à côte.
2. Photographier les deux écrans ensemble (ou filmer au ralenti à 240 i/s) une dizaine de fois.
3. Latence = chiffre affiché sur le PC − chiffre affiché sur le Mac. Retenir la médiane.

### 3.3 Matrice de tests de référence
| # | Réseau | Codec | Débit | Notes |
|---|---|---|---|---|
| 1 | Ethernet | h264 | défaut | référence |
| 2 | Ethernet | hevc | défaut | `--video-codec hevc` |
| 3 | Ethernet | h264 | 50 Mb/s | `--bitrate` (unité indiquée par l'aide : octets) |
| 4 | Wi-Fi | h264 | défaut | comparaison |
| 5 | Ethernet | h264 | défaut | jeu en mouvement rapide + manette |

Consigner les résultats dans `docs/prototype-A-results.md`.
