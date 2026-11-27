# PyroWave : le Raspberry Pi

[English](pyrowave-client-pi.en.md) · Français

Section client de [pyrowave.md](pyrowave.md), pour l'app Linux sur Raspberry Pi (`apps/linux`) : le paquet .deb sur Raspberry Pi OS (bureau labwc) et l'image KyberNow Link OS (`apps/linux/os`, KMS/DRM sans compositeur). Client Linux de bureau : [pyrowave-client-linux.md](pyrowave-client-linux.md) ; règles d'interface : [link-ui.md](link-ui.md), « PyroWave ».

**État (25/11/2026)** : étape 1 (étude d'après le code de PyroWave a32b05e, de Mesa et du noyau de Raspberry Pi, et le dépôt de Raspberry Pi OS ; compilation) et étape 2 (sonde du Pi, correctif du déquantificateur, chemin par fragments, interface) écrites. Vérifié dans la VM « KyberNow Linux » (plus bas). **Rien n'a tourné sur une carte** : la « Marche à suivre sur une carte » le fera.

## Verdict par carte

| Carte | Carte graphique | PyroWave |
|---|---|---|
| Pi 5, Pi 500, CM5 | V3D 7.1 (BCM2712) | **Probable**, à trois conditions : Raspberry Pi OS trixie à jour (Mesa 25.3 ou après ; 26.2.7 aujourd'hui), le chemin par fragments du décodeur, et un correctif d'une ligne des shaders de PyroWave (plus bas). Avec PyroWave a32b05e tel quel : impossible. Sur bookworm (Mesa 24.2) : impossible. Vitesse jamais mesurée. |
| Pi 4, Pi 400, CM4 | V3D 4.2 (BCM2711) | **Impossible** : le pilote n'a que les opérations de sous-groupe de base, le décodeur en exige six. Il faudrait réécrire le déquantificateur de l'amont sans sous-groupes. |
| Pi 3, Zero 2 W | VideoCore IV | **Impossible** : pas de pilote Vulkan (v3dv ne prend que V3D 4.2 et 7.1). |

## Ce que demande le décodeur (PyroWave a32b05e)

Lu dans le code de l'amont (`third_party/pyrowave`) ; l'encodeur demande plus, mais le Pi ne fait que décoder.

- **Vulkan 1.1** au moins (`pyrowave_c.cpp:115`, Granite `context.cpp:918`), avec le contrôle de la taille des sous-groupes et `computeFullSubgroups` (Vulkan 1.3 en pratique, Granite `device.cpp:5916`), une taille comprise entre 4 et 128 (`pyrowave_decoder.cpp:1016`).
- **Opérations de sous-groupe** BASIC, VOTE, BALLOT, ARITHMETIC, SHUFFLE et SHUFFLE_RELATIVE, sinon `pyrowave_decoder_create` refuse (`pyrowave_decoder.cpp:999`).
- **8 bits en stockage** (`storageBuffer8BitAccess`) **ou** des tampons de texels d'au moins 16 M éléments ; hors AMD, Intel et NVIDIA, l'amont prend d'office les tampons de texels (`pyrowave_common.cpp:268`).
- `shaderInt16` : l'encodeur seulement (`pyrowave_encoder.cpp:1254`). `shaderFloat16` : facultatif ; la compilation par défaut (`PYROWAVE_FP32_MATH`, précision 1) calcule en FP32 et ne garde que le stockage intermédiaire en 16 bits (`R16_SFLOAT`).
- **Écriture dans des images sans format** : le SPIR-V embarqué (`shaders/slangmosh.hpp`, désassemblé par `spirv-dis` le 25/11) déclare `StorageImageWriteWithoutFormat` dans les trois variantes du déquantificateur (`wavelet_dequant.comp`, image `R16_SFLOAT`) et dans l'IDWT en calcul (`idwt.comp`) ; pas dans l'IDWT par fragments (`idwt.frag`).
- **Chemin par fragments** (GPU mobiles faibles en calcul) : l'IDWT passe par des rendus (`R16_SFLOAT`, `R16G16_SFLOAT`, puis la sortie `R8_UNORM`) au lieu de shaders de calcul ; le déquantificateur reste en calcul. L'amont ne le choisit que pour Qualcomm, ARM et PanVK (`pyrowave_decoder.cpp:916`) ; l'app le choisit (`fragment_path` de `pyrowave_decoder_create_info`), et la relecture vers le processeur le connaît.

## Ce que donne Mesa v3dv

| Point | Pi 5 (V3D 7.1) | Pi 4 (V3D 4.2) |
|---|---|---|
| Mesa de Raspberry Pi OS (arm64) | trixie : **26.2.7-1~bpo13+0~rpt1** (dépôt de Raspberry Pi, au-dessus de la 25.0.7 de Debian 13) ; bookworm : 24.2.8-1~bpo12+rpt5 | pareil |
| Vulkan | 1.3 depuis Mesa 24.3 ; conforme 1.3 (Khronos, 24/07/2024, CTS 1.3.8.3) | 1.3 depuis Mesa 24.3 ; conforme 1.3 |
| Taille des sous-groupes | 16 (min = max), calcul et fragments | 16 |
| Contrôle de la taille, `computeFullSubgroups` | oui (calcul) | oui (calcul) |
| Opérations de sous-groupe | BASIC, VOTE, BALLOT, SHUFFLE, SHUFFLE_RELATIVE, QUAD ; **ARITHMETIC depuis Mesa 25.3** ; CLUSTERED depuis 26.2 | **BASIC seulement** |
| 8 bits et 16 bits en stockage | oui | oui |
| `maxTexelBufferElements` | 2^28 | 2^28 |
| `shaderInt16`, `shaderFloat16` | depuis Mesa 26.2 (le second émulé) | non |
| Écriture d'images sans format | **non** : `shaderStorageImageWriteWithoutFormat` faux, aucun format n'a `STORAGE_WRITE_WITHOUT_FORMAT`, et le compilateur exige un format à l'écriture | **non** |
| Images de stockage | tuilage `OPTIMAL` seulement (jamais `LINEAR`) | pareil |
| Mémoire | un seul type, `DEVICE_LOCAL \| HOST_VISIBLE \| HOST_COHERENT`, jamais `HOST_CACHED` ; le noyau mappe les tampons v3d en écriture combinée | pareil |
| Calcul | 256 invocations par groupe ; 32 Ko partagés depuis Mesa 26.2 (16 Ko avant) | pareil |
| dmabuf, modificateurs DRM, `VK_KHR_display`, `VK_EXT_acquire_drm_display` | oui | oui |

Conséquences :

- **Pi 4** : refusé à la création du décodeur (sous-groupes), proprement, sans planter.
- **Pi 5** : tout y est sauf l'écriture sans format. Le chemin par calcul l'utilise deux fois (déquantificateur, IDWT), le chemin par fragments une seule (le déquantificateur). D'où le correctif : `layout(set = 0, binding = 0, r16f) writeonly uniform image2DArray uDequantImg;` dans `wavelet_dequant.comp` (`r32f` en précision 2), shaders recompilés. Essayé le 25/11 sur une copie : les trois variantes compilées par glslang n'ont plus `StorageImageWriteWithoutFormat` (image `R16f`) et passent `spirv-val`. Ce format est celui de l'image, sur toutes les cartes : le correctif ne change rien ailleurs et peut être proposé à l'amont.
- **Sans correctif**, sur le Pi 5, le pipeline est invalide pour Vulkan ; Mesa compilé sans assertions écrirait n'importe quoi, ou planterait. La sonde doit donc vérifier la caractéristique avant de créer le décodeur, et tourner hors du processus de l'app (plus bas).
- **Relecture par le processeur** (ce que fait `pyrowave.c` sur Linux) : la mémoire du GPU n'est jamais en cache côté processeur, la lecture d'une image y est lente (pas mesurée). D'où la proposition d'affichage sans relecture.
- **Débit** : Ethernet 1 Gbit/s sur le Pi 4 et le Pi 5 (la définition ramenée du cœur tient le débit dans la vitesse de la prise) ; c'est la vitesse de décodage, inconnue, qui fixera la définition qu'on propose au Pi (1080p visé, comme le 4:4:4 du Pi).
- **HDR** : sans objet (le Pi est en SDR).

## Compilation (étape 1, fait)

- `apps/linux/scripts/build.sh` : l'étape 4 (PyroWave a32b05e, chemin Vulkan, `libpyrowave-shared.so.1`) se fait aussi avec `--rpi`. Les en-têtes posés dans le préfixe font compiler `pyrowave.c` au lieu de `pyrowave_none.c` (`build.rs`). Le déquantificateur y est corrigé (étape 2).
- `apps/linux/scripts/deb.sh` : la bibliothèque part dans `/usr/lib/libpyrowave-shared.so.1` (réduite par `strip`, 1,1 Mo), ses dépendances sont lues sur elle (`libstdc++6`, `libgcc-s1`) ; Vulkan est ouvert par la bibliothèque elle-même (volk), donc `libvulkan1` et `mesa-vulkan-drivers` sont **recommandés** (installés par défaut par `apt install ./…deb`). Options `--prefix DIR` (le préfixe des dépendances, passé à `build.sh`) et `--sans-pyrowave` (passée à `build.sh` : sans le correctif kyclient 0025 ni la fonctionnalité `pyrowave` du cœur, l'app décode PyroWave mais ne le demande jamais ; avec eux, le défaut, elle le demande) ; `--no-build` prend aussi la bibliothèque du préfixe.
- **KyberNow Link OS** : Raspberry Pi OS Lite n'a ni `libvulkan1` ni `mesa-vulkan-drivers` (listes de paquets de pi-gen, stages 0 à 2) et notre stage installe le .deb sans les recommandations : `stage-kybernow/00-packages` ajoute `libvulkan1`, `mesa-vulkan-drivers` et `vulkan-tools` (`vulkaninfo`, pour le diagnostic). Le compte `kybernow` est déjà dans le groupe `render` (`/dev/dri/renderD128`).
- **Vérifié le 25/11 dans la VM « KyberNow Linux »** (préfixe privé, pas `third_party`) : PyroWave compilé dans le préfixe du Pi (aarch64, `NEEDED` : libstdc++, libm, libgcc_s, libc) ; un .deb assemblé (`--no-build`, binaire du Pi d'une compilation précédente) avec la bibliothèque, `libstdc++6 (>= 12)` dans `Depends`, `libvulkan1, mesa-vulkan-drivers` dans `Recommends`. **Pas vérifié** : l'app elle-même (la branche `claude/pyrowave` ne compile pas aujourd'hui pour Linux : `build.sh` pose le correctif 0025, qui ajoute `fallback_codec` à kyclient, sans la fonctionnalité `pyrowave` du cœur, et l'app ne compile pas non plus avec elle ; travail en cours ailleurs), l'image (les paquets existent dans trixie).

## Affichage dans Link OS (proposition, pas écrite)

Aujourd'hui, sur le Pi, l'image HEVC du décodeur V4L2 est recopiée par le processeur (SAND en NV12 linéaire, `av_hwframe_transfer_data`) puis donnée à SDL (texture GLES sur KMS/DRM) ; le PyroWave de Linux relit ses plans de même. Sur le Pi, cette relecture lit une mémoire hors cache : à éviter.

**Recommandé : un plan DRM par dmabuf.**

1. **Tampons** : 3 ou 4 tampons linéaires par définition, alloués par GBM (ou en « dumb buffers ») au format `DRM_FORMAT_YUV420` (trois plans dans un tampon), exportés en dmabuf, importés dans Vulkan en **VkBuffer** (`VK_EXT_external_memory_dma_buf`) : v3dv n'accepte pas d'image linéaire en stockage ni en lecture, un tampon si.
2. **Décodage** sur notre propre `VkDevice` (`pyrowave_create_device` avec nos poignées, extensions dmabuf allumées), chemin par fragments, dans des images `R8_UNORM` de PyroWave (`pyrowave_image_create`, tuilage optimal) par `pyrowave_decoder_decode_gpu_buffer`, puis dans le même tampon de commandes `vkCmdCopyImageToBuffer` de chaque plan vers le dmabuf (copie par le GPU, jamais par le processeur). Fin signalée par une fence (ou un `sync_file` exporté).
3. **Montrer** : SDL kmsdrm donne son descripteur DRM (`SDL_GetWindowWMInfo`, `info.kmsdrm.drm_fd`, le même maître DRM) ; l'app pose le tampon sur un **plan overlay** (`drmModeSetPlane` ou commit atomique) avec `COLOR_ENCODING` = BT.709 et `COLOR_RANGE` = pleine (le contrat de PyroWave en SDR). Le HVS du Pi 4 et du Pi 5 prend YUV420 et NV12 linéaires sur tous ses plans, met à l'échelle et convertit en RGB au balayage : aucune passe de rendu. Le plan vidéo passe sous le plan principal de SDL (`zpos`), que l'interface de session (barre, statistiques, bandeaux) dessine transparent là où l'image doit se voir (ARGB8888, déjà accepté par vc4). Un tampon n'est rendu à la file qu'après le flip suivant.
4. **Le même chemin pour le HEVC** : le HVS prend aussi NV12 en SAND128 : les images `DRM_PRIME` du décodeur V4L2 peuvent aller sur ce plan sans la copie d'aujourd'hui (la « voie sans copie SAND → plan DRM » restée ouverte depuis le jalon 1 du Pi).
5. **Sur le bureau** (.deb sous labwc) : les plans sont au compositeur. Même côté Vulkan, puis import du dmabuf dans le GLES de SDL (`EGL_EXT_image_dma_buf_import`, YUV420), ou une sous-surface Wayland `linux-dmabuf` que wlroots peut balayer directement.

**Écarté : `VK_KHR_display`** (Vulkan maître de l'écran, `VK_EXT_acquire_drm_display`). SDL kmsdrm et Vulkan ne peuvent pas être maîtres ensemble : toute l'interface de Link (accueil, réglages, session) devrait passer par une chaîne d'échange Vulkan, pour Link OS seulement, et il faudrait encore une passe YUV → RGB sur le GPU. Beaucoup plus de travail, pour moins que le plan.

**Ordre proposé** : d'abord la relecture actuelle (rien à écrire de plus pour l'affichage), pour mesurer sur une carte le décodage et la relecture en 1080p60 ; le plan DRM ensuite, d'autant plus vite que la relecture coûte (au-delà de 2 ms, elle seule justifie le plan).

## Étape 2 (écrite)

1. **Correctif de PyroWave** (`apps/linux/pyrowave/v3dv-dequant.py`, lancé par `build.sh` sur son clone, `$PREFIX/build/pyrowave`, jamais `third_party`) : `layout(r16f)` sur `uDequantImg` dans `wavelet_dequant.comp`, et le même changement dans le SPIR-V déjà compilé de `shaders/slangmosh.hpp`, en place et à taille égale (format de l'image `Unknown` → `R16f`, capacité `StorageImageWriteWithoutFormat` → `StorageImageExtendedFormats`, celle que glslang pose pour la source corrigée) : 6 mots dans les trois variantes, rien d'autre ne bouge, `spirv-val` les accepte. Le script vérifie la forme du SPIR-V et refuse s'il ne la reconnaît pas (autre commit). Pour tous les Linux : sans effet hors du Pi. `PYROWAVE_PATCHES` (`build.sh`) refait une bibliothèque compilée sans lui (`$PREFIX/lib/.pyrowave-v3dv-dequant-1`). Régénérer les shaders par l'outil de l'amont (`slangmosh.sh`) aurait demandé de compiler glslang et SPIRV-Tools de Granite.
2. **Chemin par fragments forcé sur v3dv** (`pyrowave.c`, `fragment_path`) : le pilote est reconnu par `VkPhysicalDeviceDriverProperties` (`VK_DRIVER_ID_MESA_V3DV`) ; HDR10 refusé sur v3dv (le chemin par fragments n'a pas de sortie 10 bits). `KYBERNOW_PYROWAVE_FRAGMENT=1` (ou `0`) le force ailleurs, pour les essais.
3. **Sonde** (`device/system.rs`, `pyrowave_probe_start`) :
   - Pi 4, 400, CM4, Pi 3, Zero 2 W : indisponible sans rien essayer (journal : « PyroWave : indisponible sur Raspberry Pi 4 Model B (pas les opérations de sous-groupe de Vulkan (V3D 4.2) ; Raspberry Pi 5 seulement) ») ;
   - Pi 5, 500, CM5 : d'abord **dans un processus à part**, `kybernow --pyrowave-probe` (30 s au plus ; il écrit « ok » ou « no », la carte et la raison), pour qu'un pilote qui plante n'emporte pas l'app. Son résultat est gardé dans `pyrowave-probe` (dossier de l'app : `/var/lib/kybernow/config/kybernow` sous Link OS, `~/.config/kybernow` sous Raspberry Pi OS) tant que la version de l'app et le pilote Vulkan (fichiers de `/usr/share/vulkan/icd.d` et `libvulkan_broadcom.so`, taille et date) ne changent pas. Réussie, la sonde est refaite dans l'app (le device et ses pipelines sont prêts avant la première session) ;
   - partout, `pyrowave.c` vérifie aussi les opérations de sous-groupe et donne la raison (« opérations de sous-groupe manquantes (0x… au lieu de 0x7f…) ») ; la carte du journal dit le pilote (« V3D 7.1… (Vulkan 1.3, Mesa 26.2.7…) »).
4. **Interface** ([link-ui.md](link-ui.md), « PyroWave ») : Réglages › Image › Format vidéo propose PyroWave quand l'ordinateur le propose ; son étiquette est « carte graphique » (sur le Pi aussi : V3D, pas le décodeur du Pi) ; la carte « Cet appareil » du Pi ajoute « PyroWave par la carte graphique ». Valeur inactive : « Ce Raspberry Pi ne sait pas décoder PyroWave : il faut un Raspberry Pi 5. » (Pi 4, 3, Zero 2 W), « … : il lui faut Raspberry Pi OS « Trixie », à jour. » (Pi 5 sous Raspberry Pi OS), « Ce Raspberry Pi ne sait pas décoder PyroWave. » (Pi 5 sous Link OS). Captures : `--preview`, `rpi-46-pyrowave-carte`, `rpi-46b-pyrowave-image`, `rpi-46c-pyrowave-inactif-pi5`, `rpi-46d-pyrowave-inactif-pi4`.
5. **Affichage** : la relecture par le processeur (la voie du Linux de bureau, rien de plus à écrire), comme le prévoit l'ordre de la proposition : elle se mesure sur une carte (le kit ci-dessous) ; le plan DRM vient ensuite si elle coûte plus de 2 ms en 1080p.
6. **Demandé à Kyber** : par défaut (`deb.sh`, `build.sh`, `flatpak.sh` ; `--sans-pyrowave` pour s'en passer).

### Vérifié dans la VM (25/11/2026)

VM « KyberNow Linux » (Ubuntu 24.04 ARM64, lavapipe : pas de v3dv), préfixe privé du Pi (`build.sh --rpi --kyber-pyrowave --prefix …`) :

- l'app pour le Pi compile ; 153 tests de `kybernow-linux` passés ; le .deb assemblé (`/usr/lib/libpyrowave-shared.so.1` corrigée, `Recommends: … libvulkan1, mesa-vulkan-drivers`) ;
- `kybernow --pyrowave-probe` : « no », « rendu logiciel seulement (llvmpipe …) », code 1 ; avec `KYBERNOW_PYROWAVE_CPU=1` : « ok », code 0 ;
- le **chemin par fragments avec le déquantificateur corrigé** décode juste (`KYBERNOW_PYROWAVE_FRAGMENT=1`, flux de l'encodeur Metal du Mac) : 1920 × 1080 4:2:0, PSNR Y 57,1 dB (57,4 par le chemin en calcul) ; 1280 × 720 4:2:0, 51,8 dB ; 4:4:4, pixel du milieu juste ; image tronquée sautée, la suivante décodée. Les temps sont ceux du processeur (lavapipe), sans rapport avec le Pi.
- Pas vérifiable sans carte : v3dv lui-même (que le SPIR-V corrigé y compile et y tourne), la vitesse, la relecture en mémoire hors cache, l'image KyberNow Link OS.

## Marche à suivre sur une carte

Un Raspberry Pi 5 (ou 500, CM5) relié par **Ethernet**, et un Pi 4 si vous en avez un (pour voir le refus). Une Station qui propose PyroWave (correctif 0025 et `[kyavserver] pyrowave`).

**A. Préparer, sur le Mac et dans la VM**

1. Les flux d'essai 1080p, sur le Mac : `apps/linux/tests/pyrowave/samples-mac.sh ~/pw-samples-1080 1920x1080 60`.
2. Dans la VM : le paquet avec PyroWave demandé à Kyber, `apps/linux/scripts/deb.sh --kyber-pyrowave` (→ `apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb`).
3. Dans la VM : le kit de mesure, `apps/linux/tests/pyrowave/pi-kit.sh DOSSIER_DES_FLUX ~/pw-kit` (copier d'abord les flux du Mac dans la VM ; `--444` ajoute le 4:4:4, 370 Mo de plus).
4. Pour Link OS : `apps/linux/os/build-image.sh` avec ce .deb (vider d'abord le cache de pi-gen s'il contient une image précédente).

**B. Raspberry Pi OS (Trixie, 64 bits), sur le Pi 5**

1. `sudo apt update && sudo apt full-upgrade`, puis redémarrer.
2. `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb vulkan-tools` (le paquet tire `libvulkan1` et `mesa-vulkan-drivers`).
3. `vulkaninfo --summary` : attendu « V3D 7.1… », le pilote v3dv (`driverName`), `apiVersion = 1.3…`, Mesa 25.3 ou après. Noter la ligne `driverInfo`.
4. `kybernow --pyrowave-probe; echo $?` : attendu « ok », la carte, puis `0`. Autre chose : copier la sortie complète (c'est la réponse à la question « le SPIR-V corrigé passe-t-il sur v3dv ? »).
5. Le kit : `scp -r ~/pw-kit <utilisateur>@<pi>:` depuis la VM, puis sur le Pi `~/pw-kit/mesure.sh`. Noter pour chaque flux : PSNR, « décodage, relecture comprise, cadencé à 60 i/s : moyenne … médiane … maximum … ; à la chaîne : … i/s ». Pour comparer, la relecture seule n'est pas séparée : c'est la moyenne entière qui décide (sous 6 ms en 1080p60 : bon ; au-delà, le plan DRM).
6. Lancer KyberNow Link depuis le menu, Réglages de l'app : la carte « Cet appareil » dit « PyroWave par la carte graphique ». Le journal (`~/.config/kybernow/kybernow.log`) : « PyroWave : la sonde à part a réussi » puis « PyroWave : prêt sur V3D 7.1… ».
7. Réglages de l'ordinateur › Image › Format vidéo › PyroWave (étiquette « carte graphique »), puis une session par Ethernet : l'image, puis Statistiques (Décodage, Images perdues). Une session en Wi-Fi : le bandeau « PyroWave en Wi-Fi », « Passer en HEVC ».
8. Relancer l'app : le journal dit « (gardée) » (pas de nouvelle sonde à part).

**C. KyberNow Link OS, sur le Pi 5**

1. Flasher l'image de A.4, démarrer, passer le premier démarrage.
2. SSH (clé déposée sur `BOOTA`, voir le README de l'OS) : `vulkaninfo --summary`, `kybernow --pyrowave-probe; echo $?` (comme B.3 et B.4), `grep -i pyrowave /var/lib/kybernow/config/kybernow/kybernow.log`.
3. À la manette : Réglages de l'ordinateur › Image › Format vidéo › PyroWave, une session par Ethernet, les statistiques.

**D. Sur un Pi 4 (Raspberry Pi OS ou Link OS)**

1. `kybernow --pyrowave-probe` n'est pas lancé par l'app : le journal dit « PyroWave : indisponible sur Raspberry Pi 4 Model B (pas les opérations de sous-groupe de Vulkan (V3D 4.2) ; Raspberry Pi 5 seulement) ».
2. Format vidéo : PyroWave grisé, « Ce Raspberry Pi ne sait pas décoder PyroWave : il faut un Raspberry Pi 5. ». Lancé à la main, `kybernow --pyrowave-probe` doit dire « no » et « opérations de sous-groupe manquantes », sans planter.

À rapporter : les sorties de B.3, B.4, B.5 et C.2, et ce qu'on voit en session (image juste ou non, saccades).

## Sources

- PyroWave a32b05e : `pyrowave.h`, `pyrowave_c.cpp`, `pyrowave_decoder.cpp`, `pyrowave_common.cpp`, `shaders/wavelet_dequant.comp`, `shaders/slangmosh.hpp` ; Granite `vulkan/context.cpp`, `vulkan/device.cpp`.
- Mesa v3dv : https://gitlab.freedesktop.org/mesa/mesa/-/raw/mesa-26.2.7/src/broadcom/vulkan/v3dv_device.c (propriétés et caractéristiques : sous-groupes, 8/16 bits, `shaderStorageImageWriteWithoutFormat`, types de mémoire, limites de calcul ; refus des V3D avant 4.2), `…/mesa-26.2.7/src/broadcom/vulkan/v3dv_formats.c` (fonctions des formats : pas de `STORAGE_WRITE_WITHOUT_FORMAT`, stockage en `OPTIMAL` seulement), `…/mesa-26.2.7/src/broadcom/compiler/v3d_nir_lower_image_load_store.c` (écriture : format exigé), `…/mesa-25.0.7/src/broadcom/vulkan/v3dv_device.c` (avant ARITHMETIC), `…/main/src/broadcom/common/v3d_device_info.c` (V3D 4.2 et 7.1 seulement).
- Notes de version de Mesa : https://docs.mesa3d.org/relnotes/24.3.0.html (Vulkan 1.3 sur v3dv, Pi 4 et Pi 5), https://docs.mesa3d.org/relnotes/25.3.0.html (opérations arithmétiques de sous-groupe), https://docs.mesa3d.org/relnotes/26.2.0.html (CLUSTERED, float16/int8, 32 Ko partagés).
- Conformité : https://www.khronos.org/conformance/adopters/conformant-products/vulkan (Raspberry Pi 4 Model B et Raspberry Pi 5, Vulkan 1.3).
- Raspberry Pi OS : http://archive.raspberrypi.com/debian/dists/trixie/main/binary-arm64/Packages.gz et `…/bookworm/…` (versions de `mesa-vulkan-drivers`), https://github.com/RPi-Distro/pi-gen (branche `arm64`, listes de paquets des stages 0 à 2).
- Noyau de Raspberry Pi : https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/gpu/drm/vc4/vc4_plane.c (formats YUV des plans, SAND, `COLOR_ENCODING`, `COLOR_RANGE`), `…/drivers/gpu/drm/v3d/v3d_bo.c` (écriture combinée).
- Non lus : les rapports de vulkan.gpuinfo.org (liste chargée en JavaScript) ; tout ce qui précède vient du code, pas d'une carte.
