# PyroWave : le client Windows

[English](pyrowave-client-windows.en.md) · Français

Section client de [pyrowave.md](pyrowave.md) (le contrat et la Station), à y fusionner. Concerne l'app Windows (`apps/windows`, x64 et ARM64) ; le client Mac est dans [pyrowave-client-mac.md](pyrowave-client-mac.md). Règles et textes : [link-ui.md](link-ui.md), « PyroWave ».

## La bibliothèque

- Le chemin Vulkan de [PyroWave](https://github.com/Themaister/pyrowave) (licence MIT), au commit de `apps/macos/PyroWave/COMMIT` (a32b05e, le même que la Station et le Mac), avec le Granite partiel de `checkout_granite.sh` (volk, en-têtes Vulkan). Sources dans `third_party/pyrowave`, partagées avec la Station (qui compile dans `build-win64`).
- `apps/windows/scripts/build-pyrowave.sh [--arch x64|arm64]` (cmake et ninja) compile `libpyrowave-shared-1.dll` dans `third_party/pyrowave/build-client-<arch>` et la pose, allégée, dans `third_party/windows-client-<arch>/pyrowave/` avec son commit. x64 : MinGW de Homebrew, libstdc++ et libgcc liées en statique par le projet de l'amont ; ARM64 : llvm-mingw, libc++ et libunwind liées en statique (`-static`). Environ 2,2 Mo (x64) et 1,7 Mo (ARM64) ; elles ne dépendent que de Windows et de l'UCRT.
- `build.sh` la recompile si elle manque ou si le commit a changé, et la copie dans `dist/<arch>/` : elle part dans `KyberNow-Link-Setup.exe` avec le reste du dossier.
- **Vulkan vient du pilote.** La DLL n'est pas liée au chargeur Vulkan : volk ouvre `vulkan-1.dll` (installée par le pilote de la carte graphique) quand l'app crée le device PyroWave. Sans elle, ou sans la DLL de PyroWave, ou sur une carte qui ne le fait pas tourner, PyroWave est seulement indisponible : la sonde le dit au cœur, rien d'autre ne change.

## Décodage (`src/pyrowave.rs`)

- L'app ouvre la DLL elle-même (`LoadLibraryW`, une fois par processus), lit ses fonctions et vérifie l'API (1.x). Les types de `pyrowave.h` et le peu de Vulkan nécessaire y sont recopiés à la main (pas d'en-têtes Vulkan à la compilation).
- **Device.** Un device PyroWave par appareil Direct3D 11 de la session (`Gpu::pyrowave`, créé au premier flux PyroWave, partagé par tous les écrans) : `pyrowave_create_device_by_compat` avec le LUID de la carte de Direct3D (`IDXGIAdapter::GetDesc`), donc la même carte ; `pyrowave_device_confirm_interop_support`, file graphique (`pyrowave_device_set_queue_type`). Chemin par fragments (`fragment_path`) quand `pyrowave_decoder_device_prefers_fragment_path` le recommande (Adreno des PC Snapdragon).
- **Paquet de configuration** (`KNPW`, lu par le cœur : `kybernow_core::pyrowave::Config`, version 1 de 16 octets et tête de la version 2) : reconnu par `is_config` ou par sa signature, comme sur Mac. L'app demande la version 2 (`kyclient::set_pyrowave_header(2)`) et en lit les métadonnées HDR10 (`hdr::pyrowave_hdr`, testé sur le Mac), crête du flux pour le rendu ; une Station d'avant envoie la version 1. Un changement recrée le décodeur et ses textures. S'il manque, la définition et la chroma sont lues dans l'en-tête de séquence du flux (repli en SDR, plage pleine).
- **Textures partagées.** Trois textures Direct3D 11 (Y, Cb, Cr ; `R8_UNORM` en SDR, `R16_UNORM` en HDR10 ; chroma à demi-taille en 4:2:0), `SHARED | SHARED_NTHANDLE`, importées dans PyroWave par leur handle NT (`VK_EXTERNAL_MEMORY_HANDLE_TYPE_D3D11_TEXTURE_BIT`, `MUTABLE_FORMAT`, comme `create_pyrowave_image_from_d3d11` de l'essai d'interopérabilité de l'amont), une vue de stockage par plan.
- **Synchronisation.** Une fence Direct3D 11 partagée (`D3D11_FENCE_FLAG_SHARED`), importée comme semaphore timeline (`D3D12_FENCE_BIT`). Pour chaque image : le décodage attend la valeur posée par Direct3D après la copie de l'image précédente (images prises de `QUEUE_FAMILY_IGNORED` : contenu jeté) et pose la suivante en rendant les images à Direct3D (`QUEUE_FAMILY_EXTERNAL`) ; le fil de l'image attend cette valeur sur le processeur (au plus 250 ms, sinon l'image est sautée : Direct3D n'attend jamais une fence qui ne vient pas). Le rendu fait `ID3D11DeviceContext4::Wait`, copie les trois plans dans ses propres textures, puis `Signal` et `Flush`.
- **Une image = un paquet.** `pyrowave_decoder_clear`, tout le paquet dans `push_packet`, puis `decode_is_ready` (complète, ou partielle selon le seuil de l'amont) ; sinon l'image est sautée. Jamais d'image clé demandée : chaque image se décode seule. Quand d'autres images attendent derrière (rafale du réseau), celle-ci n'est pas décodée du tout.
- **Rendu** (`Renderer::draw_pyrowave`) : le mode « trois plans » du shader (celui du décodage par le processeur et du 4:4:4), sans rien de propre à PyroWave. Valeurs : PyroWave écrit la valeur normalisée v ; en 8 bits, le code v · 255 ; en 16 bits, la convention P010 de la Station (10 bits en haut de 16, unité 64/65 535). Matrice BT.709 (SDR) ou BT.2020 (HDR10), plage du paquet de configuration ; HDR10 : PQ, vers la chaîne d'échange 10 bits PQ ou ramené au SDR comme le HEVC Main 10 ; pas de métadonnées HDR10 dans le flux, crête par défaut (1 000 nits).
- **Temps.** `received` à l'arrivée du paquet, `decoded` quand le GPU a fini (fence vue par le processeur) : le temps de décodage des statistiques et le trajet d'une image en tiennent compte comme pour D3D11VA. Le format montré dans la barre et les statistiques est « PyroWave · carte graphique ».
- **Fil de l'image** (`src/video.rs`) : le format annoncé `pwv1` ouvre le décodeur PyroWave à la place de FFmpeg ; le format jamais annoncé mais demandé (`PyroWave` choisi) l'ouvre d'office, comme pour les autres formats.

## Capacités et câble

- **Sonde du lancement** (`pyrowave::probe`, dans le processus de sonde `--probe-json`, comme le décodage des autres formats) : device PyroWave sur la carte, décodeur 128 × 128, décodage pour de bon de l'image de test du Mac (`src/probe/pyrowave.bin`, 1 812 octets, rampe de Y de 16 à 235, Cb et Cr à 128, encodée par l'encodeur Metal du port), par le chemin de la session (textures partagées, fence), puis relecture de quelques points, à 4 près. `pyrowave` dans les capacités envoyées au cœur (`Gpu`, `ThisPc`, `Capabilities`) ; `kybernow --probe` dit « PyroWave : carte graphique (Vulkan) » ou la raison. Un pilote qui plante n'emporte que le processus de sonde.
- **Câble** (`system::wired_to`) : la carte réseau que Windows prend pour l'adresse de l'ordinateur (`GetBestInterfaceEx`, puis `GetIfEntry2`) est Ethernet (type 6, support 802.3 ou non dit) ; Wi-Fi, réseau mobile ou adresse introuvable : non. Relevé au lancement de la session et donné au cœur (`StreamSettings::wired`, gardé quand le profil change en session) ; « Auto » prend alors PyroWave si la Station le propose (règle du cœur, `profile::pyrowave_format`). Les réglages le demandent aussi (`computer_wired`) pour leurs aides.
- **Débit d'un PyroWave choisi à la main** (`plan.rs`, `sessionPlan` de l'interface) : celui qu'« Auto » prendrait par câble pour la définition demandée (`PYROWAVE_MBPS`, `PYROWAVE_MBPS_444` du cœur, suivant les pixels par seconde), pas celui du HEVC. Le 4:4:4 est permis en PyroWave.

## Interface

Textes de link-ui, dans les catalogues de l'app (`ui/src/lib/i18n`, rubrique `pyrowave` ; `src/i18n`, le texte du bandeau).

- **Réglages › Image › Format vidéo** : la valeur « PyroWave » quand la Station la propose (`info.pyrowave`, booléen ou `{ available }`) ou qu'elle est déjà choisie ; inactive avec sa raison (« {nom} ne propose pas PyroWave. », « Ce PC ne sait pas décoder PyroWave. »). Aides : PyroWave choisi ; « Auto » par câble ou sans câble. Avertissement sous le choix, PyroWave choisi : Wi-Fi, ou relais (ordinateur hors de chez vous ou session par le relais). « Texte net (4:4:4) » permis avec PyroWave, aide « presque sans coût ». Le curseur du débit va jusqu'à 1 500 Mbit/s en PyroWave, sans l'avertissement Wi-Fi général.
- **Bandeau du début de session** : à la première seconde d'image en PyroWave, sans câble ou par le relais, une fois par session : capsule de la barre « PyroWave en Wi-Fi » / « PyroWave hors de chez vous », « L'image peut saccader. », bouton « Passer en HEVC » (commande `session_hevc` : l'image repart en HEVC pour cette session, le choix de l'ordinateur reste), croix ; 8 s. App pas devant : la notification de Windows des autres avis.
- **Bandeau quand le réseau sature** ([debit-automatique.md](debit-automatique.md)) : le cœur envoie l'avis `PyroWaveStrained` après 5 s de réseau saturé (une fois par session, hôte seulement) ; `host.rs` le passe à l'interface (`pyrowave_strained`), qui montre le même bandeau titré « Le réseau sature », avec « Passer en HEVC » et la croix ; 8 s. Barre ouverte sur cette session : il attend qu'elle se cache, oublié si la session finit ou si l'image est déjà en HEVC. App pas devant : la notification de Windows.
- **Barre de session** : puce ambre « PyroWave en Wi-Fi » / « PyroWave hors de chez vous » quand l'image est en PyroWave (format des mesures) sans câble ou par le relais ; le format dit « PyroWave ».
- **Statistiques** : ligne « Images perdues » en part des images des 30 dernières secondes (« 0,8 % ») ; ligne « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. » prête (`Stats::lowered`), mais le cœur ne dit pas encore quand il baisse la définition : jamais montrée pour l'instant.
- Démo de l'interface (`npm run dev`) : `?pyrowave`, `?pyrowave=wifi`, `?pyrowave=pc`, avec `&relais` ; `bar.html?pyrowave=wifi` pour le bandeau, `bar.html?pyrowave=sature` pour celui du réseau qui sature.

## Essai hors ligne (`kybernow --pyrowave-test`)

Sur le PC, sans ordinateur ni réseau : la sonde du lancement, puis une mire animée (rampe qui défile, damier fin, bandes de couleur, plage réduite) encodée par **l'encodeur de l'amont** (celui de la DLL, `pyrowave_encoder_encode_cpu`, sur le même device), une image = paquets de 1 152 octets bout à bout comme un paquet média de la Station, décodée par **le décodeur de l'app** (`pyrowave::Decoder`, textures partagées, fence) et copiée comme le fait le rendu. Pour 1920 × 1080, 2560 × 1440 et 3840 × 2160, en 4:2:0 et 4:4:4, SDR et HDR10 (16 bits, convention P010) : débit à 60 i/s, temps de décodage (GPU terminé) cadencé à 60 i/s (moyenne, médiane, maximum), PSNR de la première image relue sur la carte, image tronquée sautée et la suivante décodée. Options : `--size 1920x1080`, `--420`/`--444`, `--sdr`/`--hdr`, `--frames 120`, `--bpp 1.2`. Code de sortie 1 si PyroWave est indisponible ou si le PSNR de Y tombe sous 30 dB.

**Pas encore lancé** : écrit et compilé sur le Mac (x64 et ARM64) le 25/11/2026, jamais exécuté sur Windows.

### Marche à suivre sur le PC x64

1. `apps/windows/scripts/build.sh`, puis copier `apps/windows/dist/x64/` sur le PC (ou l'installateur `KyberNow-Link-Setup.exe`, après `--arch arm64` et `build-installer.sh`).
2. Dans un terminal, dans ce dossier : `kybernow.exe --probe`. Attendu : « PyroWave : carte graphique (Vulkan) » ; sinon la raison (DLL absente, pas de Vulkan, carte refusée, image de test fausse).
3. `kybernow.exe --pyrowave-test`. Attendu : une ligne par cas, sans « ÉCHEC », PSNR de Y au-dessus de 40 dB environ, décodage de l'ordre de la milliseconde à 2 ms sur une carte de bureau, « image tronquée sautée, suivante décodée ». Garder la sortie et `%LOCALAPPDATA%\KyberNow\kybernow.log`.
4. En session (app construite par `build.sh`, qui pose par défaut le correctif kyclient 0025 et la fonctionnalité `pyrowave` du cœur ; Station qui propose PyroWave) : « Format vidéo » › PyroWave, puis le journal (« PyroWave : device prêt », « PyroWave : décodeur … »), la barre (« PyroWave · carte graphique ») et les Statistiques. Débrancher le câble (Wi-Fi) : avertissement des réglages, bandeau et puce ; « Passer en HEVC ».

### Dans la VM Windows ARM (Parallels)

Même chose avec `dist/arm64/`. La carte graphique virtuelle de Parallels ne donne probablement pas Vulkan : attendu alors « PyroWave : non (pas de Vulkan …) » à `--probe`, le code 1 de `--pyrowave-test`, et surtout une app qui démarre et des sessions HEVC qui marchent comme avant. Sur un vrai PC Snapdragon (Adreno), `--pyrowave-test` doit dire « Chemin par fragments ».

## Ce qui attend

- Rien côté code : « Définition ramenée » vient du cœur (`Second::lowered`, avis `ResolutionLowered`, lus par `host.rs`). Reste l'essai en session décrit plus haut.
