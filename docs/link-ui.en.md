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

During a session: « Se déconnecter » / “Disconnect” (bar, menu, shortcut), « Statistiques » / “Statistics” for live measurements, « Touches » / “Keys”, « Plein écran » / “Full screen”, « Clavier » / “Keyboard”.

With touch (iPhone, iPad, Android outside the TV):

- **Keyboard**: it opens from the bar; a touch on the picture folds it away (that touch does not click on the computer).
- **On-screen controller** / « Manette à l'écran »: a bar button, remembered for each computer. Two fixed layouts, for both thumbs in landscape: “Full” / « Complète », a whole Xbox controller (two sticks which click: L3, R3; the D-pad; A, B, X, Y; LB, RB, LT and RT, progressive; View, Menu and Guide); “Simple” / « Simple », the left stick, the D-pad, A, B, X, Y and Menu. A small button on the controller switches between them (accessible label “Simple controller” / “Full controller”, « Manette simple » / « Manette complète »), the choice remembered for each computer. The computer sees it as an Xbox 360 controller, in the first free player slot. While it is shown, the rest of the screen does not act as a mouse. Settings in Controls: “Layout” (Full, Simple), “Size” (Small, Medium, Large), “Opacity” and “Haptic feedback” (Off, Light, Medium, Strong; Medium by default). Every press can be felt and seen: the button lights up and sinks slightly; a crisp click when pressing a button, the D-pad or a stick; for the triggers, a light click at the start of the travel and a firm click at the end; for the sticks, a tick when the stick leaves the centre, a very light tick at each change of direction (eight directions) and a tick at the edge, never a continuous vibration. While the controller is shown, the game's rumble vibrates the device. Without a haptic engine (iPad), only the visual feedback remains.

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

## Icon next to the clock (Mac, Windows)

The Mac and Windows apps have an icon next to the clock: in the menu bar on the Mac, in the notification area on Windows. It stays as long as the app runs, window open or closed, and gives access to everything needed to connect quickly.

- **Drawing**: the KyberNow Link symbol alone (the `now` state of [brand/v2](../brand/v2/README.en.md)), in a single colour: a template image on the Mac (the system tints it), white or black depending on the taskbar theme on Windows. It does not look like the KyberNow Station icon (the coloured tile and its badge): a PC that has both shows two icons. In a session, the symbol lights up (a distinct variant, legible at small size). Tooltip: « KyberNow Link » / “KyberNow Link”, in a session « KyberNow Link · en session sur {nom} » / “KyberNow Link · in session on {name}”.
- **Click**: on the Mac, a click opens the menu. On Windows, a right or left click opens the same menu (on a left click, after Windows' double-click delay); a double click opens the window.

### Menu

From top to bottom. During a session, first:

| Item | Français | English |
|---|---|---|
| Header (inactive) | « En session sur {nom} » | “In session on {name}” |
| Measurements (inactive, updated while the menu is open) | « 12,4 ms · 60 i/s · 80 Mbit/s »: the latency as in the session bar, to the tenth, frames per second and bitrate; “—” for what is not measured yet; before the first frame, the connection step | “12.4 ms · 60 fps · 80 Mbit/s” |
| Full screen (checked in full screen) | « Plein écran » | “Full screen” |
| Mouse | « Souris des jeux ou du bureau » | “Game or desktop mouse” |
| Live measurements | « Statistiques » (Mac: the measurements panel; Windows: the session bar stays shown until the pointer passes over it, a second click hides it) | “Statistics” |
| End | « Se déconnecter » | “Disconnect” |

Then your computers, in the home screen's order: « {nom} — En ligne », « {nom} — Hors ligne », « {nom} — Recherche… » (“{name} — Online”, “Offline”, “Searching…”), each preceded by a small coloured dot: green online, red offline, amber while waking and while the Station restarts or updates, grey while searching (state colours from [brand/v2](../brand/v2/tokens/kybernow.css), `--kn-ok`, `--kn-bad`, `--kn-warn`, `--kn-faint`, in their light or dark version to match the menu); the state's text stays, colour alone is not enough for everyone. The session's computer is checked. Each one opens a submenu:

| Item | Français | English |
|---|---|---|
| Online (and “Searching…”, like the home screen's button) | « Se connecter ({profil}) », the computer's profile; inactive during a session on another computer (one session at a time); on the session's computer: « Se déconnecter » | “Connect ({profile})”; “Disconnect” |
| Offline | « Réveiller l'ordinateur » when the app knows its MAC address, otherwise « Réessayer »; while waking: « Réveil de l'ordinateur… », inactive; while the Station restarts or updates: « Se connecter ({profil}) », inactive (it comes back by itself) | “Wake the computer”, “Try again”, “Waking the computer…” |
| Profile | « Profil » ▸ « Jeu », « Bureau », « Max », the computer's one checked; « Personnalisé » checked and inactive when the settings depart from the profile. Picking a profile applies it as on the home screen (in a session, at once, like the bar's menu) | “Profile” ▸ “Game”, “Desktop”, “Max”, “Custom” |
| Settings | « Réglages » (Mac: « Réglages… »): the window opens on the computer's settings | “Settings” (Mac: “Settings…”) |
| Admin (online) | « Ouvrir Admin » | “Open Admin” |
| Restart (online, Station 0.5.0 and later) | « Redémarrer la Station »; meanwhile: « La Station redémarre… », inactive | “Restart the Station”; “The Station is restarting…” |
| Update (a version that installs from a device) | « Mettre à jour la Station ({v}) »; meanwhile: « Mise à jour de la Station… », inactive | “Update the Station ({v})”; “Updating the Station…” |

The menu's groups are separated by a line. During a session, “Settings” and “Add a computer” are inactive on the Mac (the window belongs to the session); on Windows, they show the home screen over the session.

Restarting and updating follow the **Station** section: the same confirmation when a device is in a session (“{device} is connected”, “Restart anyway”, “Cancel”), the same follow-up, the same texts at the end.

At the bottom:

| Item | Français | English |
|---|---|---|
| Window | « Ouvrir KyberNow Link » | “Open KyberNow Link” |
| Adding | « Ajouter un ordinateur » (Mac: « Ajouter un ordinateur… ») | “Add a computer” (Mac: “Add a computer…”) |
| App settings | « Réglages de l'app » (Mac: « Réglages de l'app… ») | “App settings” (Mac: “App settings…”) |
| App update | Mac: « Rechercher des mises à jour… »; Windows: « Version {v} disponible » when a version is waiting (opens the app settings), otherwise « Rechercher des mises à jour » (opens the app settings and starts the check) | “Check for updates…”; “Version {v} available”; “Check for updates” |
| Quit | « Quitter KyberNow Link »; a session in progress ends first | “Quit KyberNow Link” |

### Window and startup

- Closing the home window does not quit the app: it keeps running in the icon. It is quit with “Quit KyberNow Link” (Mac: also ⌘Q and the Dock). Closing the session window still means disconnecting.
- The first time the window closes, a notification says so, once only: « KyberNow Link est toujours là » / “KyberNow Link is still here”, « Vos ordinateurs vous attendent dans la barre des menus. » / “Your computers are waiting in the menu bar.” (Windows: « … dans la zone de notification, près de l'horloge (sous la flèche ^ si Windows l'a rangée). » / “… in the notification area, next to the clock (under the ^ arrow if Windows tucked it away).”: Windows 11 first puts a new icon under that arrow).
- Mac: the Dock icon is only there while a window is open (home, settings, session); without a window, the app lives in the menu bar only. Opening the app again (Finder, Launchpad, Spotlight) opens the home screen.
- **Ouvrir à l'ouverture de session** / “Open at login”, in the App settings, checked by default (from the first launch of the version that brings it; turning it off in the system, System Settings or Task Manager, is respected). Help: « KyberNow Link démarre avec votre Mac, sans fenêtre : vos ordinateurs vous attendent dans la barre des menus. » / “KyberNow Link starts with your Mac, without a window: your computers wait for you in the menu bar.” (Windows: « … démarre avec Windows, … dans la zone de notification. » / “… starts with Windows, … in the notification area.”). Launched this way, the app starts without a window, in the icon only.
- With the window closed, the app keeps following your computers (less often than with the window open, and at once when the menu opens): the menu and the notifications are up to date.

### Notifications

System notifications, only when the app is not in the foreground:

| When | Title | Text | Button |
|---|---|---|---|
| A Station version is ready (once per version and per computer) | « Mise à jour de la Station de {nom} » / “Station update for {name}” | « La version {v} est prête. Environ une minute ; une session en cours sera coupée. » / “Version {v} is ready. About a minute; a session in progress will be cut off.”; a version that needs the installer: « La version {v} s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe. » / “Version {v} installs on the computer, with KyberNow-Station-Setup.exe.” | « Mettre à jour » / “Update” (none for the installer) |
| The computer comes back after “Wake the computer” | « {nom} est en ligne » / “{name} is online” | « Vous pouvez vous connecter. » / “You can connect now.” | « Se connecter » / “Connect” |
| Still nothing after 2 min of waking | « {nom} ne répond toujours pas » / “{name} is still not responding” | « Vérifiez qu'il est branché au réseau et que le réveil est autorisé sur l'ordinateur (Admin › Système). » / “Check that it is plugged into the network and that waking is allowed on the computer (Admin › System).” | |
| A session stops without “Disconnect”: another device or Admin | « La session sur {nom} est terminée » / “The session on {name} has ended” | « Un autre appareil a pris la main, ou Admin y a mis fin. » / “Another device took over, or Admin ended it.” | « Reprendre la main » / “Take back control” |
| … an error | « La session sur {nom} s'est arrêtée » / “The session on {name} stopped” | the reason, in one sentence | « Reprendre » / “Resume” |
| End of a restart or update started from the menu, or a refusal | the computer's name | the Station section's texts (“The Station has restarted.”, “KyberNow Station {v} is installed.”, “The Station is taking a while to come back…”, “The update did not complete…”, “The Station has just restarted…”, Station too old) | |

A click on the notification opens the app on what it announces (the home screen on that computer, or its Station section).

## Where the texts live

Each app keeps its catalogues ([i18n.en.md](i18n.en.md), “Files”); the texts above are copied into them verbatim. A platform-specific text follows the same rules.
