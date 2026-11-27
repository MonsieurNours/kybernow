# Vague 2 de l'audit du 27/11

Tout ce que la vague 1 a laissé parce que cela traverse plusieurs zones : remontée dans le cœur, factorisation, en-têtes communs de Kyber, scripts de construction, règles d'interface, traductions, docs. Relevé dans les douze fichiers de [`corrections/`](corrections/). La vague 2 se fait en deux temps pour que deux sessions ne touchent jamais les mêmes fichiers.

## Phase 2a : les fondations, en parallèle

| Session | Fichiers | Contenu |
|---|---|---|
| Cœur, API communes | `apps/core` seulement (et `apps/Cargo.lock`) | TRANS-34 (`short_fingerprint`), TRANS-35 (`parse_address`, juste en IPv6), TRANS-36 (`admin_url` exposé), TRANS-40 à 42 (débit en arrière-plan, bornes du débit à la main, constantes et règles des profils, du partage, de l'accueil, ports), COEUR-37 à 39, COEUR-41, TRANS-50 (dépendances `ndk-sys` et `kynput-sys` inutilisées) et TRANS-51 (`regist::PORT`, `takion::PORT`) (textes de l'enregistrement, `format_code`, lien d'invitation, « Définition ramenée »), WINAPP-11 (repli HEVC quand PyroWave ne s'ouvre pas), WINAPP-49 à 55, 56 (`MicUi`), 62 (`primary`, `session_hdr`), AND-30 (libellés du prêt, des erreurs et du chemin du relais), LINAPP-40 (réglages synchronisés, `PsSettings`), MAC-20 (règle d'empreinte), `ffi::guard` exporté pour les ponts. Livre un guide de migration par app. |
| Correctifs Kyber, contenu | `*.patch`, `server/macos/pyrowave` | KYB-44 à 47 (une seule source pour `kn_timing.h`, `kn_hdr_*.h` et le code commun de PyroWave), KYB-29 (en-tête `KNPW` v2 et métadonnées HDR), KYB-31 (`fallback_codec` par moteur), KYB-25 (`PacketSink` avec `Drop`, l'ancienne API gardée le temps de la phase 2b), KYB-17, KYB-37 (reste), KYB-39 à 43, borne de la mise en paquets de `kn_pyrowave_metal.mm`, STWIN-50 (renumérotation de 0085-kycontroller-micro-droits, avec ses citations), et la question laissée par la session « Réparer une perte sans image clé » : sur un écran figé, une image perdue n'est réparée qu'au prochain mouvement ; l'encodeur (txproto `encode.c`) garde sa dernière image et la réencode, avec un pts plus récent, quand une réparation ou une image clé est demandée et qu'aucune image n'arrive en 50 ms (sous Windows, la texture gardée sort de la réserve de la capture). |
| Scripts de construction | `scripts/`, `apps/*/scripts`, `server/*/scripts`, `server/kyber-patches/{apply,build}.sh` | TRANS-19 (commit de PyroWave en un seul endroit), TRANS-20 (pose des correctifs, `cargo_quiet`, identité et Team ID, versions d'outils en un seul endroit), MAC-31 (fonction commune de pose des correctifs client), STCOM-11 (les deux `build-kyber.sh` mis en commun), TRANS-03 (reste), KYB-48, TRANS-51 (marqueur effacé par `arbres.sh`). |
| Admin à distance et Admin des Stations Mac | `server/admin`, `server/relay`, `server/web` si besoin | STWIN-17, décidé le 27/11 : Admin à distance chiffré de bout en bout jusqu'à la Station (le relais ne déchiffre plus Admin ; il fait passer le TLS par le nom de la Station), en restant compatible avec les Stations et relais publiés. STMAC-06 (domaine launchd de la Station, pas celui de l'écran), STMAC-07 (`launchctl` impossible à lancer = échec), STMAC-08 (appels à l'agent des écrans avec délai, hors verrou), oubliés de la vague 1 (code d'Admin, entre deux sessions). |
| Docs, règles et site | `docs/`, `README*`, `PROJECT*`, `ROADMAP*`, `site/`, `brand/` | Règles proposées par les sessions, à écrire dans `docs/link-ui.md` (FR et EN) : textes de la caméra (cœur et Mac), « Envoyer le journal » (Android, pour toutes les apps), disposition du clavier (IOS-30), capsule caméra et micro (TVOS-24), barre en fenêtre étroite (LINAPP-19), pays et fuseau de Link OS (LINAPP-30), écarts de la barre (LINAPP-53), avertissement du prêt (LINAPP-54), WEB-25. Puis TRANS-22, 33, 43 à 47, COEUR-46 (PyroWave allumé par défaut dans les docs), MAC-35 et 40, KYB-49, STLIN-26, TRANS-49. |

## Phase 2b : chaque app sur ces fondations, en parallèle

Après la fusion de la phase 2a. Chaque session migre son app vers les API communes du cœur (guide de migration), applique les nouvelles règles de `link-ui.md`, retire ses traductions mortes et ses copies de règles, et pose `ffi::guard` dans ses ponts.

| Session | Contenu propre |
|---|---|
| Apple (Mac, iPhone, iPad, Apple TV) | IOS-21 à 27, TVOS-20 à 22 (fichier de session partagé), catalogues iOS et tvOS périmés, `catch_unwind` dans `apps/apple/src`. |
| Android | AND-30 sur le cœur, ponts JNI gardés. |
| Windows | WINAPP-49 à 55, 62 sur le cœur, PKCE dans l'installateur (`apps/windows/setup`). |
| Linux et Link OS | LINAPP-19, 30, 40 à 45, 53, 54. |
| webOS et web | WEBOS-26 et 27, WEB-21 et 22, TRANS-30 (reste), TRANS-34 côté Android et LG. |
| Stations | STWIN-44 à 46 (factorisation d'Admin et de l'installateur), STLIN-05 (aide root qui prend l'entrée pendant un prêt), STWIN-14 (fichiers Steam de l'hôte confiés à Admin), PKCE si une Station construit elle-même l'adresse. |
| Traductions et cohérence | TRANS-29, 31, 32 une fois les apps migrées (clés mortes, libellés divergents, espace fine) ; TRANS-38 (statistiques et « Trajet d'une image » complets sur Linux, Android et le web) et TRANS-39 (autres écarts à `link-ui.md`), avec chaque app. |

## Décisions de l'auteur (27/11)

- **STWIN-06** : les codes de prêt et de partage restent à 6 chiffres (la limite de 30 mauvais codes en 10 minutes, toutes adresses confondues, est posée).
- **STWIN-17** : oui, Admin à distance chiffré de bout en bout (phase 2a).
- **TRANS-48** : la vidéo du site reste suivie par git.
