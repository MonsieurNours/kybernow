# KyberNow Link: shared screens and texts

English · [Français](link-ui.md)

Every KyberNow Link app (Mac, Apple TV, iPhone and iPad, Windows, Android and Android TV, LG TVs, Linux, Steam Deck, Raspberry Pi and KyberNow Link OS, the `/link` web client) follows this document: same screens, same order, same words. Each platform keeps what is specific to it (remote, controller, touch, windows, QR code on the TV), but one thing has one name everywhere. The tone is the website's ([ton-du-site.en.md](ton-du-site.en.md)); names and typography come from [i18n.en.md](i18n.en.md).

## Principles

- In French, the reader is addressed as “vous”. Short sentences with a verb; a button is a verb and its object (“Restart the Station”).
- Nothing technical on the home screen: no graphics card, address, decoder or latency. Figures go into **Statistics**, settings into the **computer's settings**.
- Nothing over the picture when it arrives: connecting shows its progress, then the picture alone. Measurements show on demand (session bar or menu).
- Ellipses only for states in progress (“Connecting…”) and, on Mac, for commands that open a window (Apple's convention).
- Honest: say how long it takes and what will be cut off.
- In French, « de » elides before a name starting with a vowel or an h: « Réglages d'Atelier », « Réglages d'Hugo-PC », but « Réglages de Bureau-Salon ».

## Home screen

For the selected computer, in this order and nothing else:

| Element | Français | English |
|---|---|---|
| State | « En ligne », « Hors ligne », « Recherche… » | “Online”, “Offline”, “Searching…” |
| Name | the computer's name | |
| System | “Windows 11 Pro” (as the Station reports it) | |
| Display, virtual display on, sized for this device | « Écran de 2560 × 1440 à 120 Hz, à la taille de cet écran » | “2560 × 1440 display at 120 Hz, sized for this screen” |
| Display, custom | « Écran de 1920 × 1080 à 60 Hz » | “1920 × 1080 display at 60 Hz” |
| Display, no virtual display | « Les écrans de l'ordinateur, tels quels » | “The computer's own displays, as they are” |
| Main button, online | « Se connecter » | “Connect” |
| Main button, offline | « Réveiller l'ordinateur » when the app knows its MAC address, otherwise « Réessayer »; while waking: « Réveil de l'ordinateur… » | “Wake the computer” when the app knows its MAC address, otherwise “Try again”; while waking: “Waking the computer…” |
| Profiles | « Jeu », « Bureau », « Max », and « Personnalisé » when the settings differ | “Game”, “Desktop”, “Max”, “Custom” |
| Profile sentence | Jeu: « La réactivité d'abord, pour jouer. » Bureau: « Un texte net, pour travailler. » Max: « La meilleure image que cet appareil affiche. » Personnalisé: « Vos propres réglages. » | Game: “Responsiveness first, for gaming.” Desktop: “Sharp text, for work.” Max: “The best picture this device can show.” Custom: “Your own settings.” |
| Settings access | « Réglages » (gear icon; accessible label « Réglages de {nom} ») | “Settings” (“Settings for {name}”) |
| Living-room apps only (Apple TV, Android TV, LG TVs, Linux living room and Link OS) | « 2 manettes », « 1 manette », « Aucune manette » | “2 controllers”, “1 controller”, “No controller” |

The display shown is the one the session will ask for (same computation as the session: resolution, refresh rate, 4096-wide limit in H.264 on Windows); “as they are” when the “Virtual display” setting is off or the Station has none.

Errors under the name, when there are any: « Mot de passe refusé : mettez-le à jour dans les réglages. » / “Password refused: update it in the settings.”; « La Station a été réinstallée : vérifiez son empreinte dans les réglages. » / “The Station was reinstalled: check its fingerprint in the settings.”

The other computers (row or list): the name and the state (“Online”, “Offline”), nothing else. A computer announced on the network and not yet added: « Sur le réseau » / “On your network”. Adding: « Ajouter un ordinateur » / “Add a computer”.

At the top: « Réglages de l'app » / “App settings” (Link OS: « Réglages de l'appareil » / “Device settings”), and the app's own update when there is one: « Version {v} disponible » / “Version {v} available”.

## Connecting

The connection screen shows the steps, nothing else:

| Step | Français | English |
|---|---|---|
| 1 | « Préparation de l'écran » | “Preparing the display” |
| 2 | « Connexion » | “Connecting” |
| 3 | « Premières images » | “First frames” |
| Failure | « La connexion a échoué », then the reason in one sentence | “The connection failed”, then the reason |
| Button | « Annuler » | “Cancel” |

If Admin answers but the picture does not arrive despite the session's own repairs (keyframe, restart, lower bitrate, 4:2:0), or the connection to the streaming engine fails, the app restarts the Station, once, without ending another device's session, waits for it to come back (1 min 30 at most) and tries to connect once more. The connection screen says so: « La Station redémarre… » / “The Station is restarting…”, then resumes its steps. If the picture still does not come: “The connection failed”, and the reason.

From the first frame, the picture alone. No arrival capsule, no bar that shows then hides, no figure. The window title (Windows) is “{name} · KyberNow Link” (the command line keeps its measurements in the title, as it has no bar).

During a session: « Se déconnecter » / “Disconnect” (bar, menu, shortcut), « Statistiques » / “Statistics” for live measurements, « Touches » / “Keys”, « Plein écran » / “Full screen”.

## Computer settings

Title: « Réglages de {nom} » / “Settings for {name}”. At the top: « Profil » / “Profile” (Game, Desktop, Max, Custom). Then six sections, in this order (a platform may merge or hide one when it has nothing to put in it, never rename them):

| Section | Français | English | Contents |
|---|---|---|---|
| 1 | Image | Picture | resolution, refresh rate, virtual display, video format, sharp text (4:4:4), bitrate, frame display, keep the last image |
| 2 | Son | Sound | channels, sound on the computer, and what the platform offers (spatial audio, 5.1 Dolby…) |
| 3 | Commandes | Controls | controller seen by the computer, mouse, keyboard, touch, remote, rumble, shared clipboard |
| 4 | Réseau | Network | picture transport, reconnect automatically, wake on LAN |
| 5 | Statistiques | Statistics | last session, this computer |
| 6 | Station | Station | Admin, restart, update, account, remove this computer |

What applies to the whole app (language, iCloud, the app's updates, controller test, what the device can decode) goes into the **App settings**, not here.

### Setting labels

| Français | English | Values (FR / EN) |
|---|---|---|
| Définition | Resolution | Comme cet écran / Match this display; Au plus 4K / 4K at most; Sur mesure / Custom |
| Fréquence | Refresh rate | “120 Hz” |
| Écran virtuel | Virtual display | help: « L'ordinateur crée un écran à la taille de cet appareil pendant la session ; les siens s'éteignent, puis reviennent à la fin. » / “The computer creates a display sized for this device during the session; its own displays switch off, then come back at the end.” |
| Format vidéo | Video format | Auto; HEVC; H.264; AV1 |
| Texte net (4:4:4) | Sharp text (4:4:4) | help: « Couleurs complètes : le texte reste net, au prix d'un débit plus élevé. » / “Full colour: text stays sharp, at the cost of a higher bitrate.” |
| Débit | Bitrate | Auto; “80 Mbit/s” |
| Affichage des images | Frame display | Synchronisé / Synced; Immédiat / Immediate |
| Garder la dernière image | Keep the last image | |
| Son | Sound | Auto; Stéréo / Stereo; 5.1; 7.1; Sur l'ordinateur / On the computer |
| Manette vue par l'ordinateur | Controller seen by the computer | Xbox 360; Fidèle / As is |
| Souris | Mouse | Auto; Souris du bureau / Desktop mouse; Souris des jeux / Game mouse |
| Presse-papiers partagé | Shared clipboard | |
| Vibrations | Rumble | |
| Transport de l'image | Picture transport | Rapide / Fast; Fiable / Reliable; help: « Rapide convient au réseau local et au jeu. Fiable renvoie ce qui se perd, au prix d'un peu de délai. » / “Fast suits your home network and gaming. Reliable resends what gets lost, at the cost of a little delay.” |
| Reconnexion automatique | Reconnect automatically | |
| Réveil par le réseau | Wake on LAN | « Prêt » / “Ready” with the MAC address learnt from the Station, or « Pas encore d'adresse : connectez-vous une fois à cet ordinateur. » / “No address yet: connect to this computer once.”; help: « Allume l'ordinateur à distance quand il est en veille. Il faut aussi l'autoriser sur l'ordinateur (Admin › Système). » / “Turns the computer on remotely when it is asleep. It must also be allowed on the computer (Admin › System).” |

## Statistics

**Last session**, modelled on the Mac app's card (`FrameRuler`):

- large, the median: “8.9 ms” and “median”;
- a ruler in milliseconds, from 0 to 20 ms (more if the measurement goes beyond), with the marks “160 Hz”, “120 Hz” and “60 Hz” at the duration of one frame at those rates (6.25; 8.33; 16.67 ms), and a band from the median to the 95th percentile;
- “95% of frames under 14.2 ms”;
- the duration and when: “42 min · yesterday”;
- what was sent: “2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s”;
- help: “The delay between the computer and this screen. Below a mark, the picture arrives before the next frame at that refresh rate.”
- with no session: “No session on this computer yet.”

Data kept by the app for each computer, at the end of every session of at least 3 s: median, 95th percentile, duration, date, resolution, refresh rate, format, bitrate.

**This computer**: “System”, “Graphics card”, “Address” (Admin's address and port), “Fingerprint” (first four groups).

## Station

| Element | Français | English |
|---|---|---|
| Version | « KyberNow Station 0.5.0 », then « À jour » or « Version 0.5.1 disponible » | “KyberNow Station 0.5.0”, “Up to date” or “Version 0.5.1 available” |
| Admin, desktop or handheld device | button « Ouvrir Admin »; help: « Admin s'ouvre dans votre navigateur et demande son mot de passe. Le navigateur peut signaler le certificat : la Station signe elle-même sa connexion, c'est normal. » | “Open Admin”; “Admin opens in your browser and asks for its password. The browser may warn about the certificate: the Station signs its own connection, this is expected.” |
| Admin, TV and living room | QR code of `https://address:port/`; « Scannez ce code avec votre téléphone pour ouvrir Admin. » and the address in plain text | “Scan this code with your phone to open Admin.” |
| Restart | button « Redémarrer la Station »; help: « Environ 30 secondes. Une session en cours sera coupée. » | “Restart the Station”; “About 30 seconds. A session in progress will be cut off.” |
| Update (installable version) | button « Mettre à jour la Station »; help: « Version {v}. Environ une minute. Une session en cours sera coupée. » | “Update the Station”; “Version {v}. About a minute. A session in progress will be cut off.” |
| Version that needs the installer | « La version {v} s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe. » | “Version {v} installs on the computer, with KyberNow-Station-Setup.exe.” |
| A device is in a session | title « {appareil} est connecté », message « Redémarrer la Station coupera sa session. » (or « La mise à jour coupera sa session. »), buttons « Redémarrer quand même » / « Mettre à jour quand même » and « Annuler » | “{device} is connected”, “Restarting the Station will end its session.” / “The update will end its session.”, “Restart anyway” / “Update anyway”, “Cancel” |
| During | « La Station redémarre… », « Mise à jour de la Station… » | “The Station is restarting…”, “Updating the Station…” |
| After | « La Station a redémarré. », « KyberNow Station {v} est en place. » | “The Station has restarted.”, “KyberNow Station {v} is installed.” |
| Too soon | « La Station vient de redémarrer. Réessayez dans une minute. » | “The Station has just restarted. Try again in a minute.” |
| The Station is slow to return | « La Station tarde à revenir. Ouvrez Admin pour en savoir plus. » | “The Station is taking a while to come back. Open Admin to find out more.” |
| The update fails | « La mise à jour n'a pas abouti. Ouvrez Admin pour en savoir plus. » | “The update did not complete. Open Admin to find out more.” |
| Station too old | « Pour redémarrer ou mettre à jour la Station depuis cet appareil, mettez-la d'abord à jour depuis Admin. » | “To restart or update the Station from this device, first update it from Admin.” |
| Account | « Compte »: the username; device added with a code: « Accès propre à cet appareil » | “Account”; “This device's own access” |
| Remove | button « Retirer cet ordinateur »; confirmation « Retirer {nom} de cet appareil ? Vous pourrez l'ajouter à nouveau. », buttons « Retirer » and « Annuler » | “Remove this computer”; “Remove {name} from this device? You can add it again later.”, “Remove”, “Cancel” |

Restart and update go through Admin (`POST /api/stream/station/restart` and `/update`, see [server/README.en.md](../server/README.en.md)); the app then follows the Station's state through `GET /api/stream/info` until it is back. Every added device can do it.

Wake on LAN: Admin gives the computer's network cards in `GET /api/stream/info` (MAC address, IPv4 address, broadcast address); the app keeps them for each computer and the core sends the wake packet. On iPhone, iPad and Apple TV, sending to a broadcast address needs an Apple entitlement: the app also sends the packet straight to the last known address, which does not always wake it.

## Where the texts live

Each app keeps its catalogues ([i18n.en.md](i18n.en.md), “Files”); the texts above are copied into them verbatim. A platform-specific text follows the same rules.
