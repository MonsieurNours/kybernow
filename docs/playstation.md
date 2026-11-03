# KyberNow : jouer à sa PlayStation

[English](playstation.en.md) · Français

Les apps Link affichent aussi une PS5 ou une PS4 de la maison, comme la lecture à distance de Sony : l'image et le son de la console, la manette de l'appareil qui la pilote. La console apparaît à l'accueil à côté des ordinateurs.

Ce document est le contrat de ce chantier. Les textes exacts iront dans [link-ui.md](link-ui.md), section « Consoles PlayStation », quand les écrans seront écrits. État : **cadré le 03/11/2026 ; bloc 1 (découverte et réveil) écrit dans le cœur, découverte essayée sur la PS5 Pro, réveil à essayer après l'association** ([apps/core/README.md](../apps/core/README.md#consoles-playstation-srcplaystation)).

## Décisions

| Sujet | Choix |
|---|---|
| Consoles | PS5 (Pro comprise) et PS4, dès le départ |
| Protocole | Écrit en Rust dans le cœur (`apps/core`), d'après [chiaki-ng](https://github.com/streetpea/chiaki-ng) (AGPL-3.0, comme KyberNow), qui sert de référence et de témoin pendant les essais. Pas de lien avec la bibliothèque C de chiaki-ng |
| Première app | Mac. Les autres viennent après, sur le même cœur |
| Hors de chez soi | Par le relais de KyberNow ou un VPN (Tailscale, WireGuard). Pas de passage par les serveurs de Sony (« remote play » par PSN) |
| Compte PSN | L'identifiant du compte, demandé par la console à l'association, est lu par une connexion au compte Sony dans une fenêtre de l'app, **une seule fois**, puis gardé par l'app. Ce compte ne sert à rien d'autre (pas de connexion par les serveurs de Sony). En secours, si la page de Sony change : saisie de l'identifiant à la main |
| DualSense | Gâchettes adaptatives et vibrations HD dès la première version |
| Marque | « PS5 » et « PS4 » écrits en toutes lettres pour nommer la console ; aucun logo de Sony. Projet sans lien avec Sony, à dire dans la doc et sur le site |
| Essais | Une PS5 Pro sur le même réseau que le Mac. Pas de PS4 : sa partie est écrite d'après chiaki-ng et reste à essayer |

## Ce que voit l'utilisateur

- **Trouver la console** : les PS5 et PS4 allumées ou en mode repos apparaissent à l'accueil, avec leur nom et leur état. On peut aussi en ajouter une par son adresse.
- **L'associer, une fois** : l'app ouvre d'abord la page de connexion de Sony pour lire l'identifiant du compte PSN (une seule fois pour toutes les consoles de ce compte), puis demande le code à 8 chiffres affiché sur la console (Réglages › Système › Lecture à distance › Associer un appareil sur PS5), valable 5 minutes. L'association est gardée par l'app.
- **La réveiller** : une console en mode repos se réveille quand on la choisit, comme un ordinateur endormi.
- **Jouer** : l'image en plein écran, le son, la manette. Même barre de session, mêmes statistiques et même façon de quitter que pour un ordinateur.
- **La manette** : une DualSense branchée en USB ou en Bluetooth passe telle quelle, gâchettes adaptatives, vibrations HD, pavé tactile, gyroscope et barre lumineuse compris. Une autre manette est vue comme une manette PlayStation : boutons, sticks, vibrations simples.

## Ce que fait la console

- **Image** : H.264 ou H.265 (HEVC), décidé par la console. 1080p à 60 i/s sur PS5 d'après chiaki-ng ; la définition maximale de la PS5 Pro reste à relever au premier essai. HDR possible en HEVC sur PS5. PS4 : H.264 seulement, 720p ou 1080p (PS4 Pro).
- **Son** : Opus stéréo (déjà lu par le cœur, `audio/opus.rs`).
- **Réseau** : découverte par diffusion UDP (port 9302 sur PS5, 987 sur PS4), puis une session sur les ports 9295 à 9297. IPv4 seulement.
- **Une seule session à la fois** par console.

## Architecture

Le cœur garde Kyber pour les ordinateurs et ajoute une seconde sorte de source, la console. Blocs à écrire, dans l'ordre, chacun comparé à chiaki-ng sur la même console :

1. **Découverte et réveil** (`discovery` dans chiaki-ng).
2. **Association** : connexion au compte Sony pour lire l'identifiant (fenêtre web de l'app, comme chiaki-ng), puis le code (`regist`, `rpcrypt` : tables de clés propres à la PS4 et à la PS5).
3. **Session** : contrôle HTTP chiffré, transport Takion sur UDP, mesure du débit (`ctrl`, `session`, `takion`, `streamconnection`, `senkusha`). Messages en protobuf (`takion.proto`, lu avec prost).
4. **Image et son** : réassemblage, correction d'erreurs Reed-Solomon, déchiffrement (`videoreceiver`, `frameprocessor`, `fec`, `gkcrypt`, `audioreceiver`).
5. **Entrées et retour** : état de la manette, pavé tactile, gyroscope (`feedback`, `controller`), et dans l'autre sens les vibrations, les gâchettes et la barre lumineuse.

La connexion par les serveurs de Sony (`holepunch`, `rudp`, `stun` dans chiaki-ng, le plus gros morceau) n'est pas reprise.

**App Mac** : l'image de la console ne passe pas par VLC (`kyvlcplayer` est lié aux sessions Kyber). Décodage par VideoToolbox et affichage dans la même fenêtre que les sessions Kyber ; le choix exact de la couche d'affichage se fait au bloc 4. La DualSense passe par GameController (gâchettes adaptatives) ; les vibrations HD arrivent de la console comme un flux sonore, à jouer sur les canaux de vibration de la manette en USB, et à traduire en vibrations simples en Bluetooth (à vérifier sur macOS).

**Hors de chez soi** : une Station allumée sur le même réseau que la console fait suivre les paquets de la session dans le tunnel du relais, sans décoder l'image. Par un VPN, l'app joint la console directement.

## Points ouverts

- **Définition de la PS5 Pro** en lecture à distance : à relever au premier essai.
- **Vibrations HD sur Mac** : le flux de vibrations de la DualSense en USB est-il accessible par Core Audio ?
- **Relais** : faire suivre la session d'une console demande une Station sur le même réseau ; le cas « console seule, sans PC » ne passe que par un VPN.
