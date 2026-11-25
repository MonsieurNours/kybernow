# PyroWave : le client Android

[English](pyrowave-client-android.en.md) · Français

Section client de [pyrowave.md](pyrowave.md) (le contrat et la Station), à y fusionner. Concerne l'app Android (téléphone, tablette, téléviseur) ; règles et textes : [link-ui.md](link-ui.md), « PyroWave ».

## Construction (`apps/android/scripts/build-native.sh`)

- **PyroWave** : chemin Vulkan de l'amont, au commit épinglé de `server/kyber-patches/pyrowave.sh` (a32b05e, le même que la Station et le Mac), sources dans `third_party/pyrowave` avec le Granite partiel de `checkout_granite.sh` (`pyrowave_fetch` de la Station, rien n'est modifié dans ce dossier). Le projet CMake `apps/android/jni/pyrowave` ajoute Granite réduit à ce que demande le codec (ni rendu, ni compilateur de shaders, ni fenêtres, plateforme `null`), la bibliothèque `pyrowave`, l'API C (`pyrowave_c.cpp`) et le pont de l'app (`kn_pyrowave.cpp`), avec la chaîne CMake du NDK (`ANDROID_STL=c++_static`) et le `glslc` du NDK pour les deux shaders de l'affichage.
- **Sortie** : `libkybernow_pyrowave.so` (1,0 Mo pour arm64-v8a, 1,1 Mo pour x86_64, sans symboles), dans `jniLibs/<abi>/` à côté de `libkybernow.so`, qui s'y lie par la fonctionnalité Cargo `pyrowave` de `kybernow-android`. Seules les fonctions `kn_pw_*` sont exportées ; elle ne dépend que de Bionic, `liblog` et `libandroid` : Vulkan est chargé à la demande par volk, l'app démarre sur un appareil sans Vulkan.
- **ABI** : arm64-v8a et x86_64. armeabi-v7a reste sans PyroWave (sa sonde répond « indisponible »). Les appareils Android en 32 bits (Chromecast avec Google TV, téléviseurs d'entrée de gamme) n'ont de toute façon pas de Vulkan 1.3.
- **Correctifs recouverts** : un correctif de `apps/macos/kyber-patches` déjà posé mais recouvert par un correctif suivant du même dépôt (0002 sous 0004, quand le client Windows a été construit dans le même `third_party`) compte comme posé.
- **Rootfs à part** : `KYBERNOW_ANDROID_ROOTFS=<dossier>` construit libkynput (et prend libopus, `libvlc.pc`) dans `<dossier>/rootfs-<cible>` au lieu de `third_party/kyber-desktop/rootfs-<cible>`, pour construire contre les sources kynput du moment sans toucher au rootfs partagé.
- **Ce que Kyber demande** : le correctif kyclient 0025 (`VideoCodec::PyroWave`, `fallback_codec`) et la fonctionnalité `pyrowave` du cœur vont ensemble ; `third_party/kyber-desktop` est partagé par toutes les apps. Ils ne sont posés qu'avec `build-native.sh --kyber-pyrowave` (fonctionnalité `kyber-pyrowave` de `kybernow-android`), tant qu'ils ne sont pas allumés pour toutes les apps. Sans eux, l'app décode PyroWave et annonce `pyrowave` au cœur, mais le cœur ne le demande jamais à Kyber.

## La sonde du lancement

Au lancement de l'app, sur un fil d'arrière-plan (`PyroWave.probe`, `Native.pyrowaveProbe`, `kn_pw_probe`) :

1. Chargeur Vulkan (`volkInitialize`) en **1.3** au moins : le chargeur d'Android 11 s'arrête à 1.1 (la Shield n'ira pas plus loin).
2. Un GPU en Vulkan **1.3** avec ce qu'exige le décodeur de `pyrowave.h` : opérations de sous-groupe BASIC, VOTE, BALLOT, ARITHMETIC, SHUFFLE et SHUFFLE_RELATIVE en calcul, `subgroupSizeControl` et `computeFullSubgroups` ; et pour l'affichage `synchronization2`, `dynamicRendering`, une file graphique et de calcul, `VK_KHR_android_surface` et `VK_KHR_swapchain`.
3. Le device de l'app (créé par l'app, prêté à PyroWave par `pyrowave_create_device`), gardé pour les sessions ; le chemin par fragments quand `pyrowave_decoder_device_prefers_fragment_path` le dit (Qualcomm, Mali), sinon le calcul.
4. Un décodeur 128 × 128, l'image de test de la sonde du Mac (1 812 octets, rampe de Y de 16 à 235, chroma à 128) décodée et relue (`decode_cpu_buffer_synchronous`), à 4 près.

Le résultat (`PyroWave.decodes`) part au cœur dans les capacités (`pyrowave` de `Hdr.capabilities`, et pour un invité de KyberNow Share). Le journal dit le GPU et le chemin, ou la raison : `adb logcat -s KyberNow | grep PyroWave`.

## Décodage et affichage (`jni/src/video.rs`, `jni/src/pyrowave.rs`, `jni/pyrowave/kn_pyrowave.cpp`)

- Le fil du décodeur reconnaît `pwv1` (`0x7077_7631`) et passe à côté de MediaCodec, sur la même `SurfaceView` : changer de format ferme l'un avant d'ouvrir l'autre (une surface n'a qu'un producteur).
- **Paquet de configuration** `KNPW` (16 octets, `is_config` ou reconnu à sa signature) : chroma, couleur (SDR BT.709 ou HDR10 BT.2020 PQ), plage, définition. Sans lui : la taille de la session, 4:2:0, SDR plage pleine. Un changement recrée le décodeur, les trois plans et, si le HDR change, la swapchain.
- **Une image** : `pyrowave_decoder_clear`, tout le paquet dans `push_packet` ; incomplète (moins de 90 % des blocs ou bandes basses manquantes) ou illisible : sautée, rien n'est demandé à l'ordinateur. Si plusieurs images attendent, seule la plus récente est décodée.
- **Un seul tampon de commandes** par image : le décodage de PyroWave (`pyrowave_device_set_command_buffer`) dans trois plans R8 (SDR) ou R16 (HDR, si le GPU les prend ; sinon R8), en `COLOR_ATTACHMENT_OPTIMAL` (fragments) ou `GENERAL` (calcul), puis une passe de rendu (rendu dynamique) qui échantillonne les plans (chroma 4:2:0 agrandie par l'échantillonneur) et convertit YCbCr → RGB dans l'image de la swapchain : BT.709 en SDR, BT.2020 en HDR, plage pleine ou réduite (P010 en 10 bits) d'après le paquet de configuration. Soumis, présenté, et la fonction rend la main GPU terminé : l'instant du décodage pour les statistiques, et les tampons internes de PyroWave libres pour l'image suivante.
- **Swapchain** sur l'`ANativeWindow` de la session : MAILBOX si la surface le propose (la plus récente image au balayage suivant, comme MediaCodec), sinon FIFO ; image droite (`IDENTITY`, le compositeur tourne au besoin) ; refaite si la surface change de taille. La fréquence et le mode de l'écran restent ceux que l'app a choisis (`preferredDisplayModeId`, `Surface.setFrameRate`, réglés sur la fenêtre et la surface, pas par le décodeur).
- **HDR10** : swapchain `A2B10G10R10` en `HDR10_ST2084` quand la surface l'accepte (`VK_EXT_swapchain_colorspace`), métadonnées de l'écran par `VK_EXT_hdr_metadata` ; sinon l'image PQ est ramenée en SDR dans le shader (blanc à 203 nits, BT.2020 → BT.709, genou doux, gamma 2,2).
- **Statistiques** : `frame_displayed(reçue, décodée GPU terminé, montrée ≈ décodée + une période)`, donc « Décodage » et le trajet d'une image sans changement ; une image sautée n'est jamais montrée et compte dans « Images perdues ». Le nom du décodeur : « PyroWave (Vulkan, {GPU}, fragment|compute[, HDR10]) ». Trou dans le flux, surface refaite : rien à demander (pas d'image clé).

## Capacités et règle d'« Auto »

- `pyrowave` : la sonde (ci-dessus). `wired` : le réseau actif est en Ethernet (`ConnectivityManager`, `NetworkCapabilities.TRANSPORT_ETHERNET`), envoyé dans les réglages de la session ; le cœur prend PyroWave en « Auto » seulement par câble, hors relais, si la Station le propose (`profile::pyrowave_format`).
- L'annonce de la Station (`pyrowave.available` de `GET /api/stream/info`) est gardée avec l'ordinateur (`Host.pyrowave`, nulle pour une Station d'avant PyroWave).
- **Débit du lien** (`PyroWave.linkMbps`, `Native.sessionSetLinkMbps`) : donné au cœur au début de la session et à chaque changement du réseau (`registerDefaultNetworkCallback`), pour la définition ramenée (`pyrowave::ceiling`). En Wi-Fi, le débit de réception négocié (`WifiInfo.getRxLinkSpeedMbps`, sinon `getLinkSpeed`) ; en Ethernet, 0 (inconnu) : Android ne donne qu'une estimation fixe (`getLinkDownstreamBandwidthKbps`), pas la vitesse de la prise. Le plafond de la Station (« Débit maximal » d'Admin) et le débit réglé à la main comptent aussi.

## Interface (textes de link-ui, `scripts/strings.py`)

- **Réglages › Image › Format vidéo** : « PyroWave » après AV1, seulement quand la Station l'annonce ; inactif avec sa raison dans l'aide (« {nom} ne propose pas PyroWave. », « Cet appareil ne sait pas décoder PyroWave. »). Aides : PyroWave choisi, Auto par câble, Auto sans câble. Choisi en Wi-Fi ou hors de chez vous (ordinateur joint par le relais) : l'avertissement ambre sous le choix. « Texte net (4:4:4) » : « Avec PyroWave, presque sans coût : seul le débit monte. » quand PyroWave est choisi ou pris par « Auto ». Au téléviseur, les mêmes rangées au focus.
- **Session** : choisi à la main, en Wi-Fi ou par le relais, le bandeau du début (« PyroWave en Wi-Fi » ou « PyroWave hors de chez vous », « L'image peut saccader. », « Passer en HEVC », la croix ; parti après 8 s). « Passer en HEVC » relance l'image en HEVC pour cette session seulement (`Native.sessionSetSettings`). La puce ambre dans la barre (une tuile au menu du téléviseur), qui ouvre les statistiques.
- **Définition ramenée** (règle et calcul du cœur, `apps/core/src/pyrowave.rs`) : l'avis du cœur (`onNotice` 9, `SessionListener.NOTICE_RESOLUTION_LOWERED`, texte du cœur) donne le bandeau « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. » (8 s), repris dans le bandeau PyroWave du début quand il est là ; l'avis sans définition (revenue à celle de l'appareil) l'efface. Les Statistiques en font une ligne d'après `Second::lowered` (valeurs 18 à 21 d'`onSecond`).
- **Le réseau sature** ([debit-automatique.md](debit-automatique.md)) : l'avis du cœur (`onNotice` 10, `SessionListener.NOTICE_PYROWAVE_STRAINED`) montre le même bandeau, titré « Le réseau sature », avec « L'image peut saccader. », « Passer en HEVC » et la croix, parti après 8 s. Une fois par session, hôte seulement, par câble aussi ; il prend la place du bandeau du début s'il est encore là.
- **Statistiques** : « Images perdues » (en % des images de la seconde) et la définition ramenée.
- Tests : `PyroWaveTest` (annonce, choix, ordinateur gardé), `PyroWaveUiTest` (Compose : aides, avertissements, valeur inactive, bandeau, « Passer en HEVC », puce, « Images perdues », définition ramenée du cœur dans les Statistiques, son avis et son retour, repris dans le bandeau du début, bandeau « Le réseau sature » au téléphone et au téléviseur).

## Où PyroWave sera indisponible

- **Nvidia Shield TV** (Tegra X1, Android 11) : indisponible, le chargeur Vulkan d'Android 11 s'arrête à 1.1. Déduit, pas vérifié sur l'appareil : la sonde le dira dans le journal.
- Tout appareil en Android 12 ou avant, sans Vulkan 1.3, ou en 32 bits (armeabi-v7a).
- **Émulateur** : pas d'essai possible. Avec `-gpu host` (MoltenVK) ou `-gpu lavapipe`, gfxstream fait tomber l'émulateur dès la création du décodeur (« Failed to unbox VkSampler », dans gfxstream, sur les échantillonneurs de Granite) ; avec `-gpu swiftshader_indirect`, SwiftShader (sous-groupes de 4) refuse le décodeur et la sonde répond proprement « indisponible ». L'émulateur du Mac (puce Apple) est en arm64 : pas d'image x86_64.
- **Redmi A5** (Unisoc T7250, Mali-G57, Android 15) : probablement disponible, en chemin par fragments ; à essayer.

## Essai hors ligne sur un appareil (`apps/android/tests/pyrowave/run.sh`)

Sans ordinateur distant, sur l'appareil branché : la sonde, puis une mire (`testsrc2` de ffmpeg, 120 images, BT.709 plage réduite, 1,2 bit par pixel à 60 i/s) encodée **sur l'appareil** par l'encodeur de l'amont (une image = ses paquets bout à bout, comme un paquet média de la Station), puis décodée et présentée par le vrai chemin de l'app sur une surface `AImageReader` relue : PSNR de la première image en RGB contre la source, image tronquée sautée, temps moyen cadencé à 60 i/s ; en 4:2:0 et 4:4:4, puis étiquetée HDR10 (plans R16, PQ ramené en SDR). L'encodeur de l'amont demande plus que le décodeur : s'il refuse le GPU, l'essai s'arrête après la sonde. L'outil `kn_pyrowave_test` est construit par `build-native.sh` dans `target/pyrowave-android/<abi>/`.

### Marche à suivre pour l'auteur

Sur le Mac, dans le worktree de la branche (ou après fusion) :

1. `apps/android/scripts/build-native.sh` (les trois ABI ; PyroWave pour arm64 et x86_64).
2. Redmi A5 branché en USB (débogage USB) : `apps/android/tests/pyrowave/run.sh` (1080p) puis `apps/android/tests/pyrowave/run.sh 1280x720`. Attendu : « Sonde : PyroWave disponible (Mali-G57…, fragment) », « Encodé : 120 images … », « PyroWave (Vulkan, …, fragment), 1920x1080 4:2:0 », « Première image : PSNR … dB » (30 dB au moins), « Image tronquée au tiers : sautée », le temps moyen, puis « OK », pour 4:2:0, 4:4:4 et HDR10. À noter : toute la sortie.
3. L'app : `cd apps/android && ./gradlew assembleDebug && adb install -r app/build/outputs/apk/debug/app-debug.apk`, l'ouvrir, puis `adb logcat -d -s KyberNow | grep PyroWave` : la ligne de la sonde (« PyroWave : disponible (…) » ou la raison).
4. Shield branchée en réseau (`adb connect <adresse>:5555`) : `ANDROID_SERIAL=<adresse>:5555 apps/android/tests/pyrowave/run.sh` ; attendu : « Sonde : PyroWave indisponible (Vulkan loader 1.1, 1.3 needed) » (ou une autre raison), rien de plus. Puis l'app installée : la même ligne au journal, et pas de « PyroWave » dans Réglages › Image › Format vidéo… sauf si la Station l'annonce : il y est alors inactif, « Cet appareil ne sait pas décoder PyroWave. ».
5. Une session réelle, une fois la Station en PyroWave et le cœur construit avec `--kyber-pyrowave` : Redmi en Wi-Fi, PyroWave choisi dans Format vidéo : bandeau, puce ambre, « Passer en HEVC » ; Statistiques : « Décodage » et « Images perdues » ; le nom du décodeur.

## État des essais

| Quoi | État |
|---|---|
| `libkybernow_pyrowave.so` arm64-v8a et x86_64 (NDK 30, API 30) | construite (25/11/2026), liée à `libkybernow.so`, APK construit avec PyroWave |
| Sonde sur l'émulateur (SwiftShader) | répond « indisponible (… no decoder) » sans planter, app lancée (25/11/2026) |
| Décodage, affichage, HDR10 sur un vrai GPU | **à essayer** (Redmi A5 : marche à suivre ci-dessus) |
| Shield : sonde indisponible | à confirmer |
| Interface | tests Compose `PyroWaveUiTest` passés ; pas vue sur un appareil |
| Session complète vers la Station | à essayer (correctif 0025 et `--kyber-pyrowave`) |
