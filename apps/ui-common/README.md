# kybernow-ui — l'interface commune de Linux et des téléviseurs LG

[English](README.en.md)

Les apps KyberNow Link pour Linux (`apps/linux` : bureau, Steam Deck, salon, KyberNow Link OS) et pour les téléviseurs LG (`apps/webos`) dessinent leur interface elles-mêmes, et cette interface se compile sans le cœur ni SDL (captures, essais sur le Mac). Ce qu'elles écrivaient chacune, mot pour mot, est ici (audit du 27/11, LINAPP-42 et WEBOS-26, phase 2c).

| Module | Contenu |
|---|---|
| `i18n` | La langue de l'app (`english`, `set_english`, `resolve` du réglage Auto / Français / English, `force_english` pour les essais), `tr("…", "…")`, `de` (« d'Atelier », « de Bureau-Salon »), `settings_for`, `decimal` ; **`core_text(clé, paramètres)`** : un texte du catalogue du cœur (`apps/core/src/i18n/*.json`, lu tel quel), le même mot pour mot que dans les autres apps |
| `time` | Heure locale (`localtime_r`), noms des jours et des mois, `hour_label` (« 18 h 30 »), `weekday_label`, `date_label` (« 26 novembre », « 1er décembre »), `relative_day` (« aujourd'hui », « hier ») |
| `autotest` | Le panneau du test du profil Auto : `StepState`, `Step`, `Choice`, les étapes en attente et le relais (catalogue du cœur), « Choisi par le test du {date} ({réseau}). », les textes de la carte « Débit » et du bandeau |
| `station_offer` | La mise à jour des Stations proposée par l'app : ce que le cœur calcule, gardé par l'interface (`StationOffer`, `OfferBanner`), le bandeau de l'accueil, l'avis après une mise à jour de l'app, leurs textes |
| `share` | Le rôle d'un invité et ses droits (`ShareRole`, `ShareMove`), la qualité de l'image, l'état du relais d'un partage et son attente (`ShareRelay`, `relay_wait`, `RELAY_LINK_WAIT`), la disponibilité, les petits textes |
| `lend` | Le prêt : l'état de l'ordinateur d'un ami, l'emprunt et sa session, la vue de l'hôte et ses demandes, le code et son lien, les invitations, leurs textes |
| `from_core` | Fonctionnalité `core`, pour la couche appareil : le rôle, la qualité et le relais du cœur dans ces types |

Ce qui reste dans chaque app : ce qui touche à son écran, à son focus et à ses actions (`App`, `Target`, `Screen`), et le dessin (le canevas de Linux suit l'échelle de l'écran et le HiDPI, celui de LG la sortie 1080p du téléviseur : seules quelques briques sont semblables).

Les règles sans texte viennent de `kybernow-rules` (`apps/core/rules`) ; les fonctions de la couche appareil qui ne dépendent que du cœur sont dans le cœur (`share::join_relays`, `relay::session_end_text`, `input::keymap::scancode_of_usage`).

Essais : `cargo test -p kybernow-ui` (et `--features core`).
