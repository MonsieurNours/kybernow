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
| Display, several displays (Mac, Windows, Linux) | « 3 écrans, à la taille de ceux de cet appareil » | “3 displays, sized for this device's screens” |
| Display, custom | « Écran de 1920 × 1080 à 60 Hz » | “1920 × 1080 display at 60 Hz” |
| Display, no virtual display | « Les écrans de l'ordinateur, tels quels » | “The computer's own displays, as they are” |
| Main button, online | « Se connecter » | “Connect” |
| Main button, computer already in a session (Mac, Windows) | « Afficher la session »: its window comes to the front | “Show session” |
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
| 1 | « Préparation de l'écran » ; avec plusieurs écrans : « Préparation des écrans » | “Preparing the display”; with several displays: “Preparing the displays” |
| 2 | « Connexion » | “Connecting” |
| 3 | « Premières images » | “First frames” |
| Failure | « La connexion a échoué », then the reason in one sentence | “The connection failed”, then the reason |
| Button | « Annuler » | “Cancel” |

If Admin answers but the picture does not arrive despite the session's own repairs (keyframe, restart, lower bitrate, 4:2:0), or the connection to the streaming engine fails, the app restarts the Station, once, without ending another device's session, waits for it to come back (1 min 30 at most) and tries to connect once more. The connection screen says so: « La Station redémarre… » / “The Station is restarting…”, then resumes its steps. If the picture still does not come: “The connection failed”, and the reason.

From the first frame, the picture alone. No arrival capsule, no bar that shows then hides, no figure. The window title (Windows) is “{name} · KyberNow Link” (the command line keeps its measurements in the title, as it has no bar).

**Several displays** (Mac, Windows and Linux apps, see [multi-ecrans.en.md](multi-ecrans.en.md)): the connection screen shows on the device's main display; the others show its background (its colour, without the home screen photo), with no text; each display switches to its picture at its first frame. If the connection fails, the message is on the main display and the other windows close. A display the computer could not create keeps that background, with a single line: « L'ordinateur n'a pas pu créer cet écran. » / “The computer could not create this display.” When the device's displays change during the session, or a profile picked during the session changes their number, the connection starts again at its first step on the main display; if the computer then refuses the new displays, the session ends with the reason, like a failed connection. A display created but left without a picture (never announced, or its encoder did not open) shows the same line as a display not created. With a Station that handles only one display, the other displays' windows do not open, with no message. A single session bar, on the display the pointer is on; the window it opens in becomes the active window; its commands apply to the whole session. The “Displays” setting changed in Settings during a session applies to the next session; the profile picked in the bar or the icon's menu applies at once. “Full screen” is not offered (bar, menu, shortcut): the windows stay full screen; to work in a window, choose “This display”. Every window has the same title.

**Several computers** (Mac, Windows and Linux apps in Desktop mode, see [plusieurs-ordinateurs.en.md](plusieurs-ordinateurs.en.md)): during a session, the bar has a « Ordinateurs » / “Computers” menu, presented like the other menus of the app's bar (a computer icon; alone where the bar shows its menus as icons, like “Keys” on Windows and Linux; with its label on the Mac, the icon alone in a narrow bar) and absent when you have no other computer: your other computers, in the home screen's order, with their colour dot and state as in the icon's menu (“{name} — Online”…); a click on an online computer opens its session, on an offline computer wakes it (“Wake the computer”, like the home screen); those already in a session are checked and offer « Afficher » / “Show”. The first session opens as usual; each further session opens in a window on top, on the active window's display, centred, two thirds of that display, with a single display, and its connection shows in that window. Each window bears its computer's name. The session of the foreground window gets the keyboard, the mouse, the controllers, the clipboard and the sound; the others keep their picture, without sound. With at least two sessions open, the bar's sound chip becomes a button, accessible label « Garder le son » / “Keep sound” (a speaker icon, lit when the session stays audible even when not active; off by default, for the current session); with a single session, it stays as before. « Ordinateur suivant » / “Next computer” shortcut: ⌃⌥⌘O on the Mac, Ctrl+Alt+Shift+O on Windows and Linux, to the next computer's window in opening order. “Connect” on a computer already in a session brings its window to the front. Four computers at most: beyond that, “Connect” is inactive, help « Quatre ordinateurs au plus en même temps. » / “Four computers at most at the same time.” The click that brings a session window to the front also goes to its computer, as with a single session. A session that stops without “Disconnect” (another device, Admin, an error) while others go on always gives its notification, even with the app in front and even if it was the active session, except when its message stays shown in its window (a failed connection, with “Close”): no duplicate (Linux, without notifications: the message stays in its window until “Close”). If a further session's connection fails, the message stays in its window with a single button, « Fermer » / “Close”, which closes that window (instead of going back to the home screen: the other sessions go on). Closing a window or “Disconnect” only ends its own session; quitting the app ends them all.

During a session: « Se déconnecter » / “Disconnect” (bar, menu, shortcut), « Statistiques » / “Statistics” for live measurements, « Touches » / “Keys”, « Plein écran » / “Full screen”, « Clavier » / “Keyboard”, « Ordinateurs » / “Computers” and « Garder le son » / “Keep sound” (several computers, below).

**Session bar from the pointer** (Mac and Windows apps, Linux app in Desktop mode, web client): it opens when the pointer stays a quarter of a second against the top edge of the picture (2 points at most), in a centred zone as wide as the bar, and only if the app is in the foreground, the pointer is on the session window and the mouse is in desktop mode. Passing quickly does not open it; nor does a pointer on another display or another app, or a mouse button held down (a window on the computer dragged to the top). With the game mouse, the pointer is captured: the bar comes through the app's other means (the icon's menu, the controller). A short line (220 points), centred at the top of the picture, marks the spot, only when the gesture is possible (app in the foreground, desktop mouse, bar closed); the web client also makes it tappable. The bar closes 0.4 s after the pointer has left it, never while one of its menus is open. With several displays, it opens on the display where the gesture happens.

With touch (iPhone, iPad, Android outside the TV):

- **Keyboard**: it opens from the bar; a touch on the picture folds it away (that touch does not click on the computer).
- **On-screen controller** / « Manette à l'écran »: a bar button, remembered for each computer. Two fixed layouts, for both thumbs in landscape: “Full” / « Complète », a whole Xbox controller (two sticks which click: L3, R3; the D-pad; A, B, X, Y; LB, RB, LT and RT, progressive; View, Menu and Guide); “Simple” / « Simple », the left stick, the D-pad, A, B, X, Y and Menu. A small button on the controller switches between them (accessible label “Simple controller” / “Full controller”, « Manette simple » / « Manette complète »), the choice remembered for each computer. The computer sees it as an Xbox 360 controller, in the first free player slot. While it is shown, the rest of the screen does not act as a mouse. Settings in Controls: “Layout” (Full, Simple), “Size” (Small, Medium, Large), “Opacity” and “Haptic feedback” (Off, Light, Medium, Strong; Medium by default). Every press can be felt and seen: the button lights up and sinks slightly; a crisp click when pressing a button, the D-pad or a stick; for the triggers, a light click at the start of the travel and a firm click at the end; for the sticks, a tick when the stick leaves the centre, a very light tick at each change of direction (eight directions) and a tick at the edge, never a continuous vibration. While the controller is shown, the game's rumble vibrates the device. Without a haptic engine (iPad), only the visual feedback remains.

## Sharing (KyberNow Share)

Contract: [kybernow-share.en.md](kybernow-share.en.md). Every Link app and the web client, the same texts everywhere; on the same network only for now.

**The host, in their session**: « Partager » / “Share” in the bar (presented like the other menus of the app's bar, a share icon). It opens a panel:

| Item | French | English |
|---|---|---|
| Title | « Partager {nom} » (the computer) | “Share {name}” |
| Code | « 123 456 » (six digits, in groups of three) | |
| Code, QR code | label « Code »; QR code of the apps' link (`link`, `kybernow://join…`), accessible label « QR code du partage » | “Code”; “QR code for the share” |
| Link | « Copier le lien »: copies the web client's address (`web`); then « Lien copié. », or « Copie impossible : sélectionnez le lien à la main. » with the address to select; no copy without a useful clipboard (Apple TV, TVs, living room) | “Copy link”; “Link copied.”; “Could not copy: select the link by hand.” |
| Help | « Sur le même réseau : « Rejoindre avec un code » dans KyberNow Link, ou ce lien dans un navigateur. » | “On the same network: “Join with a code” in KyberNow Link, or this link in a browser.” |
| List | title « Invités »; none: « Personne pour l'instant. » | “Guests”; “Nobody yet.” |
| A guest | « {prénom} · Spectateur » (no right), « {prénom} · Joueur » (controllers), « {prénom} · Clavier et souris », « {prénom} · Joueur, clavier et souris »; below, their app, their picture quality when it is not the host's (“1080p · 60 Hz”) and « depuis {durée} » | “{name} · Spectator”, “{name} · Player”, “{name} · Keyboard and mouse”, “{name} · Player, keyboard and mouse”; “for {duration}” |
| A guest's app (`app`) | Mac, Windows, Linux, « iPhone ou iPad », Apple TV, Android, « Téléviseur LG », « Navigateur » | Mac, Windows, Linux, “iPhone or iPad”, Apple TV, Android, “LG TV”, “Browser” |
| Duration (`since`, the Station's Unix seconds) | « moins d'une minute », « 3 min », « 1 h 12 »; never negative (clocks out of step) | “under a minute”, “3 min”, “1 h 12” |
| Actions on a guest | two independent rights, each given or taken back without touching the other: « Donner les manettes » / « Reprendre les manettes »; « Donner le clavier et la souris » / « Reprendre le clavier et la souris »; then « Retirer » | “Give controllers”; “Take back controllers”; “Give keyboard and mouse”; “Take back keyboard and mouse”; “Remove” |
| New code | « Nouveau code » (guests already there stay) | “New code” |
| End | « Arrêter le partage » (everyone leaves) | “Stop sharing” |
| Limit | « Quatre invités au plus. », only when the four places are taken | “Four guests at most.” |
| Station too old | « Le partage demande KyberNow Station 0.8.0 ou plus récente. » | “Sharing needs KyberNow Station 0.8.0 or later.” |
| Sharing turned off on the computer | « Le partage est coupé sur cet ordinateur (Admin › Appareils Link). » | “Sharing is turned off on this computer (Admin › Link devices).” |

Sharing impossible (Station too old, sharing turned off): “Share” dimmed; with a mouse, inactive with the reason as a tooltip; on touch and with a remote, it stays selectable and opens the panel reduced to the reason.

An open share shows in the bar (a « 2 invités » / “2 guests” chip, from one guest on, never “0 guests”; it reopens the panel), in its always-visible part. In tile menus (Apple TV, TVs, living room, Deck, Android TV), the “Share” tile's subtitle is the number of guests as soon as there is one, otherwise the code (“123 456”) when the share is open, otherwise « Code et QR code » / “Code and QR code”. Closing the session stops the share.

**The PC itself** (KyberNow Station: the icon's menu next to the clock, and Admin's home page): « Partager un écran… » / “Share a screen…”, then the choice of display (« Écran 1 — 3840 × 2160 », the main display first); the same panel (code, link, guests, actions), titled « Partage de l'écran 1 » / “Sharing display 1”. The PC stays usable as usual; no virtual display, no display switched off.

**Station refusals** (Admin, the icon, and the host routes' `error`): « {nom} est en session : le partage se lance depuis cet appareil. » / “{name} is in a session: share from that device.”; « Une connexion est en cours sur cet ordinateur : partagez un écran une fois qu'elle est finie. » / “A connection is in progress on this computer: share a screen once it has ended.”; « Cet écran n'est plus branché. » / “This display is no longer connected.”; « Cet invité n'est plus dans le partage. » / “This guest is no longer in the share.”; « Seul l'appareil en session peut partager cet ordinateur. » / “Only the device in a session can share this computer.”; « Aucun partage en cours. » / “No share in progress.”; restart or update during a share: « Un partage est en cours avec {prénoms} : le redémarrage l'arrêtera » / “A share is in progress with {names}: restarting will stop it” (« l'installation l'arrêtera » / “installing will stop it”).

**Station panel** (Admin: home and Link devices; the icon's panel), besides the texts above:

| Item | French | English |
|---|---|---|
| Card help (Admin) | « Montrez un écran de cet ordinateur à quelqu'un sur votre réseau : il le voit, entend le son, et peut jouer si vous le laissez. Vous continuez d'utiliser l'ordinateur normalement. » | “Show one of this computer's screens to someone on your network: they see it, hear the sound, and can play if you let them. You keep using the computer as usual.” |
| Choice of display | « Choisissez l'écran à partager. »; « Recherche des écrans… »; « Annuler » | “Choose the screen to share.”; “Looking for screens…”; “Cancel” |
| A device's share | « Ouvert depuis la session de {nom}. » | “Opened from {name}'s session.” |
| List | « Participants » when the host is in it; « {nom} · Hôte » | “Participants”; “{name} · Host” |
| Stopped | « Partage arrêté. » | “Sharing stopped.” |
| Admin's header, top of the panel | « Partage en cours · {n} invités »; « Partage en cours » | “Sharing · {n} guests”; “Sharing” |
| Icon, status line | « Partage de l'écran {n} · {n} invités » | “Sharing display {n} · {n} guests” |
| Icon, confirmations | « Un partage est en cours avec {prénoms} : arrêter la Station le coupera. Arrêter la Station ? »; « … : le redémarrage l'arrêtera. Redémarrer la Station ? »; when quitting: « Un partage est en cours avec {prénoms}. La Station peut rester en marche : le partage continue, et vous pourrez rouvrir l'icône depuis le menu Démarrer. » | “A share is in progress with {names}: stopping the Station will end it. Stop the Station?”; “… restarting will stop it. Restart the Station?”; “A share is in progress with {names}. The Station can keep running: the share goes on, and you can reopen the tray icon from the Start menu.” |
| Icon, failure | « Admin n'a pas pu changer le partage ({erreur}). » | “Admin could not change the share ({error}).” |

**The guest**:

| Item | French | English |
|---|---|---|
| Home screen, next to “Add a computer” | « Rejoindre avec un code » | “Join with a code” |
| Screen | title « Rejoindre un partage »; help « Le code s'affiche chez la personne qui partage. »; « Votre prénom » (suggested: the device's name, remembered); button « Rejoindre » | “Join a share”; “The code is shown by the person sharing.”; “Your name”; “Join” |
| With a camera (iPhone, iPad, Android phones and tablets) | at the top of the screen, the choice « Code » / « QR code »; a share's QR code read in “Add a computer › QR code” also leads here | “Code” / “QR code” |
| Link | `kybernow://join…` (and the QR code) opens this screen, code and computer filled in; nothing is sent before “Join”; unreadable link: « Ce lien de partage est illisible. » | “This share link cannot be read.” |
| Incomplete code | « Le code a six chiffres. » | “The code has six digits.” |
| Camera | denied: « KyberNow Link n'a pas accès à la caméra. Autorisez-la dans les Réglages, ou tapez le code. »; missing: « Pas de caméra disponible sur cet appareil. Tapez le code. » | “KyberNow Link can't access the camera. Allow it in Settings, or type the code.”; “No camera available on this device. Type the code.” |
| Searching | « Recherche du partage… » | “Looking for the share…” |
| Unknown code | « Aucun partage avec ce code sur le réseau. » | “No share with this code on your network.” |
| Full | « Ce partage est complet (quatre invités au plus). » | “This share is full (four guests at most).” |
| Too many tries (10 a minute) | « Trop d'essais de code : réessayez dans une minute. » | “Too many code attempts: try again in a minute.” |
| Unreadable format | « Cet appareil ne lit pas l'image de cet ordinateur ({format}). » | “This device cannot play this computer's picture ({format}).” |
| Sharing unavailable (the Station cannot sign Kyber's token, momentarily) | « Le partage n'est pas disponible pour le moment : réessayez dans un instant. » | “Sharing is not available right now: try again in a moment.” |
| Connecting | the connection's steps, titled « Invité sur {nom} » | “Guest on {name}” |
| Bar (reduced) | what only touches the device (latency and statistics, sound, full screen if there is one, “Picture quality”); « Spectateur » (no right) or « Joueur » (at least one right); « Quitter le partage »; the controller only with controllers; keyboard, mouse and the keys menu only with keyboard and mouse; no profile, no computer settings, no “Computers”, no clipboard | “Spectator” or “Player”; “Leave” |
| Picture quality | on the “Join” screen (below the name) and in the bar: « Qualité de l'image »: « Auto » (suggested), « Comme l'hôte », « 1080p · 60 Hz », « 720p · 30 Hz »; remembered for next time; changing it during the session restarts the guest's picture only (“First images” step), never the host's. “Auto”: resolution and frame rate at most those of the device's screen and of what its decoder keeps up with (and 60 Hz at most in a browser); when that is the host's picture, the guest shares it. The host always comes first: a guest's picture is never above the host's (resolution, frame rate, bitrate, 4:4:4; “Same as host” or more: the host's), and nothing a guest does changes the computer's displays or the host's picture. The bar shows the quality received (“1080p · 60 Hz”) | “Picture quality”: “Auto”, “Same as host”, “1080p · 60 Hz”, “720p · 30 Hz” |
| Picture adjusted by the Station (brief notice) | falling behind on the host's picture, the guest moves by itself to a lighter picture: « Image allégée pour suivre. » (the same notice when the device's decoder cannot keep up and the app falls back to 720p30 by itself); no further separate picture possible (two at most, one on an Intel card; three and two for a share started from the PC; guests with the same quality share theirs): « L'ordinateur ne peut pas faire une image de plus : vous recevez {qualité}. » (the nearest open picture, lighter) | “Picture lightened to keep up.”; “The computer can't make another picture: you get {quality}.” |
| Rights changed | « Vous pouvez jouer à la manette. »; « Les manettes sont reprises. »; « Vous avez le clavier et la souris. »; « Le clavier et la souris sont repris. »; no right left: « Vous regardez de nouveau. » instead of the taken-back notice; both rights changed at once: a single notice, the right given before the right taken back, controllers before keyboard and mouse (a brief notice) | “You can play with a controller.”; “The controllers were taken back.”; “You have the keyboard and mouse.”; “The keyboard and mouse were taken back.”; “You are watching again.” |
| End | « {hôte} a arrêté le partage. »; « {hôte} vous a retiré du partage. »; otherwise a failure's reason; never “Take back control”; a guest's reconnection offers « Quitter le partage » (not « Se déconnecter ») | “{host} stopped sharing.”; “{host} removed you from the share.” |

{host} is the host's name (the host device's name), or the computer's name for a share started from the PC. A guest is not added to the device's computers.

**Web client** (browser): the share's link (`https://<address>:<port>/link/?join=123456`, the host's “Copy link”) opens “Join a share” **without Admin's password**, with the code already filled in (“123 456”) and the suggested name (“Chrome on Mac”, remembered for next time); “Join” has to be clicked (it never connects by itself). “Join with a code” is also on Admin's password screen and on the web client's home screen (next to “Settings”), with “Cancel” to go back; the code is typed as six digits, grouped by three as you type. The browser has a single computer, the one serving the page: the code goes to it alone, with no search on the network. The picture's format is checked once the Station has answered (Kyber's engine only loads with the guest's cookie): the browser asks for the host's if it plays it, otherwise H.264 (a separate picture); it only leaves the share at once, saying so, if it plays neither H.264 nor HEVC. “Picture quality”: at “Join” and in the bar (a “1080p · 60 Hz” chip that opens the four choices); “Auto” from the screen (`screen × devicePixelRatio`), 60 Hz at most, and what the browser's decoder accepts at that size; if the quality could not change: « La qualité de l'image n'a pas pu changer : réessayez. » / “The picture quality could not be changed: try again.”. A decoding error asks for a keyframe instead of cutting; if it comes back, the picture falls back once to 720p30 (« Image allégée pour suivre. »), then the usual failure text. When the share ends, the end card (“Session ended”, the end text) has a single button, “Join with a code”; no “Reconnect”. A guest's bar: latency and statistics, sound, full screen, “Spectator” or “Player”, “Leave”; with “keyboard and mouse”: the mouse mode and the “Keys” menu (on touch: trackpad, direct touch, keyboard); the controllers chip only as a player. No profile, no settings, no clipboard.

**Admin's feed**: « {hôte} partage l'écran {n} » / “{host} is sharing display {n}”; « {prénom} regarde votre écran » / “{name} is watching your screen” (on arrival, or no right left); « {prénom} joue avec vous » / “{name} is playing with you” (a right given); « {prénom} a quitté le partage » / “{name} left the share” (gone, removed, or timed out); « Partage arrêté » / “Sharing stopped”.

## Computer settings

Title: « Réglages de {nom} » / “Settings for {name}”. At the top: « Profil » / “Profile” (Game, Desktop, Max, Custom). Then six sections, in this order (a platform may merge or hide one when it has nothing to put in it, never rename them):

| Section | Français | English | Contents |
|---|---|---|---|
| 1 | Image | Picture | resolution, displays, refresh rate, virtual display, video format, sharp text (4:4:4), bitrate, frame display, keep the last image |
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
| Écrans | Displays | Auto ; Cet écran / This display ; Tous les écrans / All displays ; help: « Auto : tous les écrans de cet appareil en Bureau et Max, un seul en Jeu et en Personnalisé. Chaque écran reçoit un écran de l'ordinateur, placé de la même façon. » / “Auto: all of this device's displays in Desktop and Max, one in Game and Custom. Each display gets a display on the computer, laid out the same way.” Inactive, on « Cet écran », with a custom resolution (help: « Avec une définition sur mesure, la session prend un seul écran. » / “With a custom resolution, the session takes a single display.”), with a session in a window (same help as the custom resolution, the window setting the display's size) and without a virtual display (setting off, or a Station without a usable virtual display: then with no help, the greyed row on « Cet écran » stands on its own and the virtual display has its own row; with a custom resolution, the custom resolution's help stays). Mac, Windows and Linux apps ([multi-ecrans.en.md](multi-ecrans.en.md)) |
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
- what was sent: “2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s”; with several displays, their number first, then the main display and the total bitrate: “3 displays · 3840 × 2160 · 160 Hz · HEVC · 120 Mbit/s”; that number is the displays the computer took (those it could not create are not in it), the same in the session bar, the session statistics and the last session;
- help: “The delay between the computer and this screen. Below a mark, the picture arrives before the next frame at that refresh rate.”
- with no session: “No session on this computer yet.”

Data kept by the app for each computer, at the end of every session of at least 3 s: median, 95th percentile, duration, date, number of displays, resolution and refresh rate of the main display, format, bitrate.

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

- **Drawing**: the KyberNow Link symbol alone (the `now` state of [brand/v2](../brand/v2/README.en.md)), in a single colour: a template image on the Mac (the system tints it), white or black depending on the taskbar theme on Windows. It does not look like the KyberNow Station icon (the coloured tile and its badge): a PC that has both shows two icons. In a session, the symbol lights up (a distinct variant, legible at small size). Tooltip: « KyberNow Link » / “KyberNow Link”, in a session « KyberNow Link · en session sur {nom} » / “KyberNow Link · in session on {name}”, on several computers « KyberNow Link · en session sur {n} ordinateurs » / “KyberNow Link · in session on {n} computers”.
- **Click**: on the Mac, a click opens the menu. On Windows, a right or left click opens the same menu (on a left click, after Windows' double-click delay); a double click opens the window.

### Menu

From top to bottom. During a session, first (with several computers, for the active window's session):

| Item | Français | English |
|---|---|---|
| Header (inactive) | « En session sur {nom} » | “In session on {name}” |
| Measurements (inactive, updated while the menu is open) | « 12,4 ms · 60 i/s · 80 Mbit/s »: the latency as in the session bar, to the tenth, frames per second and bitrate; “—” for what is not measured yet; before the first frame, the connection step | “12.4 ms · 60 fps · 80 Mbit/s” |
| Full screen (checked in full screen; absent with several displays) | « Plein écran » | “Full screen” |
| Mouse | « Souris » ▸ « Auto », « Souris du bureau », « Souris des jeux » (the settings' order), the computer's mode checked; picking a mode applies it at once and keeps it for that computer | “Mouse” ▸ “Auto”, “Desktop mouse”, “Game mouse” |
| Live measurements | « Statistiques » (Mac: the measurements panel; Windows: the session bar stays shown until the pointer passes over it, a second click hides it) | “Statistics” |
| End | « Se déconnecter » | “Disconnect” |

Then your computers, in the home screen's order: « {nom} — En ligne », « {nom} — Hors ligne », « {nom} — Recherche… » (“{name} — Online”, “Offline”, “Searching…”), each preceded by a small coloured dot: green online, red offline, amber while waking and while the Station restarts or updates, grey while searching (state colours from [brand/v2](../brand/v2/tokens/kybernow.css), `--kn-ok`, `--kn-bad`, `--kn-warn`, `--kn-faint`, in their light or dark version to match the menu); the state's text stays, colour alone is not enough for everyone. Those in a session are checked; their submenu starts with « Afficher » / “Show”, « Garder le son » / “Keep sound” (from two open sessions on, checked when on) and « Se déconnecter » / “Disconnect”. Each one opens a submenu:

| Item | Français | English |
|---|---|---|
| Online (and “Searching…”, like the home screen's button) | « Se connecter ({profil}) », the computer's profile; inactive when four computers are already in a session; on a computer in a session: see above | “Connect ({profile})”; “Disconnect” |
| Offline | « Réveiller l'ordinateur » when the app knows its MAC address, otherwise « Réessayer »; while waking: « Réveil de l'ordinateur… », inactive; while the Station restarts or updates: « Se connecter ({profil}) », inactive (it comes back by itself) | “Wake the computer”, “Try again”, “Waking the computer…” |
| Profile | « Profil » ▸ « Jeu », « Bureau », « Max », the computer's one checked; « Personnalisé » checked and inactive when the settings depart from the profile. Picking a profile applies it as on the home screen (in a session, at once, like the bar's menu) | “Profile” ▸ “Game”, “Desktop”, “Max”, “Custom” |
| Settings | « Réglages » (Mac: « Réglages… »): the window opens on the computer's settings | “Settings” (Mac: “Settings…”) |
| Admin (online) | « Ouvrir Admin » | “Open Admin” |
| Restart (online, Station 0.5.0 and later) | « Redémarrer la Station »; meanwhile: « La Station redémarre… », inactive | “Restart the Station”; “The Station is restarting…” |
| Update (a version that installs from a device) | « Mettre à jour la Station ({v}) »; meanwhile: « Mise à jour de la Station… », inactive | “Update the Station ({v})”; “Updating the Station…” |

The menu's groups are separated by a line. During a session, “Settings” and “Add a computer” show the home screen over the session (Mac and Windows: the home screen no longer serves as the session window).

Restarting and updating follow the **Station** section: the same confirmation when a device is in a session (“{device} is connected”, “Restart anyway”, “Cancel”), the same follow-up, the same texts at the end.

At the bottom:

| Item | Français | English |
|---|---|---|
| Window | « Ouvrir KyberNow Link » | “Open KyberNow Link” |
| Adding | « Ajouter un ordinateur » (Mac: « Ajouter un ordinateur… ») | “Add a computer” (Mac: “Add a computer…”) |
| App settings | « Réglages de l'app » (Mac: « Réglages de l'app… ») | “App settings” (Mac: “App settings…”) |
| App update | Mac: « Rechercher des mises à jour… »; Windows: « Version {v} disponible » when a version is waiting (opens the app settings), otherwise « Rechercher des mises à jour » (opens the app settings and starts the check) | “Check for updates…”; “Version {v} available”; “Check for updates” |
| Quit | « Quitter KyberNow Link »; the sessions in progress end first | “Quit KyberNow Link” |

### Window and startup

- Closing the home window does not quit the app: it keeps running in the icon. It is quit with “Quit KyberNow Link” (Mac: also ⌘Q and the Dock). Closing the session window still means disconnecting.
- The first time the window closes, a notification says so, once only: « KyberNow Link est toujours là » / “KyberNow Link is still here”, « Vos ordinateurs vous attendent dans la barre des menus. » / “Your computers are waiting in the menu bar.” (Windows: « … dans la zone de notification, près de l'horloge (sous la flèche ^ si Windows l'a rangée). » / “… in the notification area, next to the clock (under the ^ arrow if Windows tucked it away).”: Windows 11 first puts a new icon under that arrow).
- Mac: the Dock icon is only there while a window is open (home, settings, session); without a window, the app lives in the menu bar only. Opening the app again (Finder, Launchpad, Spotlight) opens the home screen.
- **Ouvrir à l'ouverture de session** / “Open at login”, in the App settings, checked by default (from the first launch of the version that brings it; turning it off in the system, System Settings or Task Manager, is respected). Help: « KyberNow Link démarre avec votre Mac, sans fenêtre : vos ordinateurs vous attendent dans la barre des menus. » / “KyberNow Link starts with your Mac, without a window: your computers wait for you in the menu bar.” (Windows: « … démarre avec Windows, … dans la zone de notification. » / “… starts with Windows, … in the notification area.”). Launched this way, the app starts without a window, in the icon only.
- With the window closed, the app keeps following your computers (less often than with the window open, and at once when the menu opens): the menu and the notifications are up to date.

### Notifications

System notifications, only when the app is not in the foreground (for the end of a session without “Disconnect”, also when other sessions go on, even with the app in front: see “Several computers”):

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
