# KyberNow Link: shared screens and texts

English · [Français](link-ui.md)

Every KyberNow Link app (Mac, Apple TV, iPhone and iPad, Windows, Android and Android TV, LG TVs, Linux, Steam Deck, Raspberry Pi and KyberNow Link OS, the `/link` web client) follows this document: same screens, same order, same words. Each platform keeps what is specific to it (remote, controller, touch, windows, QR code on the TV), but one thing has one name everywhere. The tone is the website's ([ton-du-site.en.md](ton-du-site.en.md)); names and typography come from [i18n.en.md](i18n.en.md).

## Principles

- In French, the reader is addressed as “vous”. Short sentences with a verb; a button is a verb and its object (“Restart the Station”).
- Nothing technical on the home screen: no graphics card, address, decoder or latency. Figures go into **Statistics**, settings into the **computer's settings**.
- Nothing over the picture when it arrives: connecting shows its progress, then the picture alone. Measurements show on demand (session bar or menu).
- Ellipses only for states in progress (“Connecting…”) and, on Mac, for commands that open a window (Apple's convention).
- Honest: say how long it takes and what will be cut off.
- A button fits on one line: never “Se / connecter” on two lines. When space runs short (one more profile, a longer language), the row wraps or shrinks elsewhere, not the button's label.
- « Se connecter » (“Connect”) means connecting to a **computer** (opening the session). For the KyberNow account, say « Connecter un compte » (“Connect an account”) and « Créer un compte » (“Create an account”): never a bare « Se connecter » for the account, which would be mistaken for the computer's.
- In French, « de » elides before a name starting with a vowel or an h: « Réglages d'Atelier », « Réglages d'Hugo-PC », but « Réglages de Bureau-Salon ».

## Home screen

For the selected computer, in this order and nothing else:

| Element | Français | English |
|---|---|---|
| State | « En ligne », « Hors ligne », « Recherche… »; « Hors de chez vous » when the computer can only be reached through the relay ([relais.en.md](relais.en.md)) | “Online”, “Offline”, “Searching…”; “Away from home” |
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
| Living-room apps only (Apple TV, Android TV, LG TVs, Linux living room and Link OS) | « 2 manettes », « 1 manette », « Aucune manette »; « Manettes indisponibles sur cet ordinateur » when the computer creates none (see “Controllers unavailable”) | “2 controllers”, “1 controller”, “No controller”; “Controllers unavailable on this computer” |

The display shown is the one the session will ask for (same computation as the session: resolution, refresh rate, 4096-wide limit in H.264 on Windows); “as they are” when the “Virtual display” setting is off or the Station has none.

Errors under the name, when there are any: « Mot de passe refusé : mettez-le à jour dans les réglages. » / “Password refused: update it in the settings.”; « La Station a été réinstallée : vérifiez son empreinte dans les réglages. » / “The Station was reinstalled: check its fingerprint in the settings.”

The other computers (row or list): the name and the state (“Online”, “Offline”), nothing else, except “· Update” when their Station has one (see “Station updates”). A computer announced on the network and not yet added: « Sur le réseau » / “On your network”. Adding: « Ajouter un ordinateur » / “Add a computer”.

Under the “Connect” and profiles row, one banner at most: first the Station update when there is one (see “Station updates”), otherwise the KyberNow account prompts.

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
| Computer not answering | « La connexion a échoué : l'ordinateur ne répond pas (rien en {secondes} s) : vérifiez qu'il est allumé et que KyberNow Station tourne, puis réessayez » | “The connection failed: the computer is not responding (nothing in {seconds} s): check that it is on and that KyberNow Station is running, then try again” |

A connection never waits forever: each set-up call (Admin, relay) has its own timeout, and a computer that does not answer ends with “The connection failed”, with the reason above. “Cancel” stops the connection right away, at any step, without waiting for the computer's answer; the connection screen closes at once, even if the session no longer answers.

If Admin answers but the picture does not arrive despite the session's own repairs (keyframe, restart, lower bitrate, 4:2:0), or the connection to the streaming engine fails, the app restarts the Station, once, without ending another device's session, waits for it to come back (1 min 30 at most) and tries to connect once more. The connection screen says so: « La Station redémarre… » / “The Station is restarting…”, then resumes its steps. If the picture still does not come: “The connection failed”, and the reason.

**Picture that doesn't start** (web client): the browser announces a format (HEVC) but refuses to open it, or its audio player doesn't start (the engine opens the sound when the picture starts). The page then tries again, once, in H.264 if it asked for HEVC and the browser reads H.264, then once without sound; an engine reason that blames the sound mutes it first. A picture that can't be decoded (the engine asked for a keyframe again, in vain) also goes to H.264, once. The fallbacks hold for the page, without changing the settings; the notice comes with the picture: « Image en H.264 : ce navigateur n'a pas pu démarrer le HEVC. » / “Picture in H.264: this browser couldn’t start HEVC.”, « Image sans le son : ce navigateur n'a pas pu démarrer l'audio. » / “Picture without sound: this browser couldn’t start the audio.”. On the third failure: “Kyber on {name} could not start the picture. Try again in a moment.”, and below, small, « Détails : » / “Details:” followed by the engine's message (`streaming_start_failed: …`), to copy. A share's guest and a loan's friend also send each failure to the Station, which writes it to its event feed ([pret.en.md](pret.en.md)); for the host, these fallbacks come before restarting the Station.

From the first frame, the picture alone. No arrival capsule, no bar that shows then hides, no figure. The window title (Windows) is “{name} · KyberNow Link” (the command line keeps its measurements in the title, as it has no bar).

**Several displays** (Mac, Windows and Linux apps, see [multi-ecrans.en.md](multi-ecrans.en.md)): the connection screen shows on the device's main display; the others show its background (its colour, without the home screen photo), with no text; each display switches to its picture at its first frame. If the connection fails, the message is on the main display and the other windows close. A display the computer could not create keeps that background, with a single line: « L'ordinateur n'a pas pu créer cet écran. » / “The computer could not create this display.” When the device's displays change during the session, or a profile picked during the session changes their number, the connection starts again at its first step on the main display; if the computer then refuses the new displays, the session ends with the reason, like a failed connection. A display created but left without a picture (never announced, or its encoder did not open) shows the same line as a display not created. With a Station that handles only one display, the other displays' windows do not open, with no message. A single session bar, on the display the pointer is on; the window it opens in becomes the active window; its commands apply to the whole session. The “Displays” setting changed in Settings during a session applies to the next session; the profile picked in the bar or the icon's menu applies at once. “Full screen” is not offered (bar, menu, shortcut): the windows stay full screen; to work in a window, choose “This display”. Every window has the same title.

**Several computers** (Mac, Windows and Linux apps in Desktop mode, see [plusieurs-ordinateurs.en.md](plusieurs-ordinateurs.en.md)): during a session, the bar has a « Ordinateurs » / “Computers” menu, presented like the other menus of the app's bar (a computer icon; with its label on the Mac and Windows, the icon alone in a narrow bar; alone on Linux, whose bar shows its menus as icons) and absent when you have no other computer: your other computers, in the home screen's order, with their colour dot and state as in the icon's menu (“{name} — Online”…); a click on an online computer opens its session, on an offline computer wakes it (“Wake the computer”, like the home screen); those already in a session are checked and offer « Afficher » / “Show”. The first session opens as usual; each further session opens in a window on top, on the active window's display, centred, two thirds of that display, with a single display, and its connection shows in that window. Each window bears its computer's name. The session of the foreground window gets the keyboard, the mouse, the controllers, the clipboard and the sound; the others keep their picture, without sound. With at least two sessions open, the bar's sound chip becomes a button, accessible label « Garder le son » / “Keep sound” (a speaker icon, lit when the session stays audible even when not active; off by default, for the current session); with a single session, it stays as before. « Ordinateur suivant » / “Next computer” shortcut: ⌃⌥⌘O on the Mac, Ctrl+Alt+Shift+O on Windows and Linux, to the next computer's window in opening order. “Connect” on a computer already in a session brings its window to the front. Four computers at most: beyond that, “Connect” is inactive, help « Quatre ordinateurs au plus en même temps. » / “Four computers at most at the same time.” The click that brings a session window to the front also goes to its computer, as with a single session. A session that stops without “Disconnect” (another device, Admin, an error) while others go on always gives its notification, even with the app in front and even if it was the active session, except when its message stays shown in its window (a failed connection, with “Close”): no duplicate (Linux, without notifications: the message stays in its window until “Close”). If a further session's connection fails, the message stays in its window with a single button, « Fermer » / “Close”, which closes that window (instead of going back to the home screen: the other sessions go on). Closing a window or “Disconnect” only ends its own session; quitting the app ends them all.

During a session: « Se déconnecter » / “Disconnect” (bar, menu, shortcut), « Statistiques » / “Statistics” for live measurements, « Touches » / “Keys”, « Plein écran » / “Full screen”, « Clavier » / “Keyboard”, « Caméra » / “Camera” (see “Device camera”), « Ordinateurs » / “Computers” and « Garder le son » / “Keep sound” (several computers, below).

**Session bar from the pointer** (Mac and Windows apps, Linux app in Desktop mode, web client): it opens when the pointer stays a quarter of a second against the top edge of the picture (2 points at most), in a centred zone as wide as the bar, and only if the app is in the foreground, the pointer is on the session window and the mouse is in desktop mode. Passing quickly does not open it; nor does a pointer on another display or another app, or a mouse button held down (a window on the computer dragged to the top). With the game mouse, the pointer is captured: the bar comes through the app's other means (the icon's menu, the controller). A short line (220 points), centred at the top of the picture, marks the spot, only when the gesture is possible (app in the foreground, desktop mouse, bar closed); the web client also makes it tappable. The bar closes 0.4 s after the pointer has left it, never while one of its menus is open. With several displays, it opens on the display where the gesture happens.

**Session bar** (Mac and Windows apps: same bar, same order, same words, same icons; model: the Mac app's `SessionChrome`). A dark capsule centred at the top of the picture; its menus and panels open below it. Menus are chips: icon, label, ▾ when they open a menu. From left to right, for the host:

| Item | French | English | Icon |
|---|---|---|---|
| Computer | the KyberNow Link symbol, lit, and the computer's name | | |
| Other computers (absent without another computer) | « Ordinateurs » ▾ (see “Several computers”) | “Computers” | computer tower |
| Sharing | « Partager », its pills inside the button: « 2 invités », « 1 demande » (see “Sharing”) | “Share”, “2 guests”, “1 request” | share (three linked dots) |
| Lending | « Prêter l'ordinateur… », its « 1 demande » pill inside the button (see “Lending the computer”) | “Lend the computer…” | key |
| Recording (a Station that records) | « Enregistrer »; while recording: red « Enregistrement · 02:14 » pill (see “Session recording”) | “Record”; “Recording · 02:14” | filled circle |
| Relay (only via the relay) | « Par le relais » | “Via the relay” | globe |
| *divider* | | | |
| Measurements (a button: it opens “Statistics”) | « 12,4 ms » lit and the curve of the last seconds, then the format: « 2560 × 1440 · 120 Hz · HEVC » (resolution and refresh rate of the session's display, the format; several displays: « 3 écrans · » first; Mac: « 4:4:4 » or « 4:2:0 » after the format); before the first latency, « 60 i/s » | “12.4 ms”; “2560 × 1440 · 120 Hz · HEVC”; “60 fps” | |
| *divider* | | | |
| Profile | the profile's name ▾: « Jeu », « Bureau », « Max », the computer's one ticked; « Personnalisé » when the settings differ; tooltip « Profil : Jeu, Bureau ou Max » | “Game”, “Desktop”, “Max”, “Custom”; “Profile: Game, Desktop or Max” | controller (Game), text (Desktop), lightning (Max), sliders (Custom) |
| HDR (HDR screen only, see “HDR”) | « HDR », on or off | | sparkle |
| Sound | « Stéréo », « 5.1 », « 7.1 » (the session's channels), « Son coupé » when sound is off in the settings; from two sessions, the pill becomes the « Garder le son » button | “Stereo”, “5.1”, “7.1”, “Sound off”; “Keep sound” | speaker (struck through: sound off) |
| Microphone (only when the computer offers it; rules to be detailed by the microphone work) | « Micro » ▾, drawn like « Caméra »; menu: « Allumer le micro » or « Éteindre le micro », then the device's microphones | “Microphone”; “Turn on microphone”, “Turn off microphone” | microphone (struck through: off) |
| Camera (only when the computer offers it and the device has a camera, see “Device camera”) | « Caméra » ▾, dimmed when it isn't sending, lit red with a dot while sending; menu: « Allumer la caméra » or « Éteindre la caméra », then the device's cameras, the chosen one ticked | “Camera”; “Turn on camera”, “Turn off camera” | camera (struck through: off) |
| Mouse | the current mode ▾: « Souris du bureau » or « Souris des jeux » (in Auto too); menu: « Auto », « Souris du bureau », « Souris des jeux », the computer's setting ticked; picking one applies it at once and keeps it for this computer, like the icon's menu; tooltip « Souris : Auto passe en souris des jeux quand un jeu est en plein écran sur l'ordinateur ({raccourci} bascule) » | “Desktop mouse”, “Game mouse”, “Auto”; “Mouse: Auto switches to the game mouse when a game is full screen on the computer ({shortcut} switches)” | pointer; crosshair in game mouse |
| Keys (Windows and Linux only) | « Touches » ▾: « Ctrl + Alt + Suppr », « Touche ⊞ seule », « Alt + Tab », « Impr. écran »; with a Mac Station, its equivalents (see “With a Mac Station”) | “Keys” | keyboard |
| Clipboard (a pill, no action) | « Presse-papiers », dimmed when not shared; tooltip « Presse-papiers partagé avec l'ordinateur (texte) » or « Presse-papiers non partagé (Réglages) » | “Clipboard”; “Clipboard shared with the computer (text)”, “Clipboard not shared (Settings)” | clipboard |
| Computer's display (Mac only: without a virtual display, with several displays on the computer) | « Écran 1 » ▾: « Écran 1 · 2560 × 1440 »…; then, when the Mac's display changes, « Adapter à 2560 × 1440 », and « Remettre 2560 × 1440 » when the computer changed resolution | | display |
| *divider* | | | |
| Full screen (absent with several displays) | icon only; tooltip « Plein écran ({raccourci}) » | “Full screen ({shortcut})” | four corners |
| End | « Se déconnecter », in red; tooltip « Se déconnecter ({raccourci}) » | “Disconnect ({shortcut})” | power |

Shortcuts: ⌃⌥⌘ + letter on the Mac, Ctrl+Alt+Shift + letter on Windows and Linux: C camera (turns it on or off), D disconnect, F full screen, M mouse, O next computer, R record, S statistics.

**Narrow bar** (an extra session, in a window two thirds of the screen): when the bar does not fit in the window's width, its chips lose their text (icon only; accessible label and tooltip kept), except the profile and “HDR”; pills shrink to the number (“2”); the measurements' format disappears. Never a clipped button.

**Guest of a share** (reduced bar, see “Sharing”): the symbol and « Invité sur {nom} » / “Guest on {name}”; their role as a chip (« Spectateur », eye icon; « Joueur », controller icon; keyboard and mouse only: keyboard icon); « Par le relais »; « Enregistrement » while the host records; the measurements | sound; « Qualité de l'image » (display icon, the received quality ▾); with the keyboard and mouse: the mouse and « Touches » (Windows, Linux) | full screen; « Quitter le partage » (power, in red).

**Friend borrowing** (reduced bar, see “Lending the computer”): the symbol, « Prêté par {hôte} » and the time left (« Encore 1 h 20 », amber during the notice) | « Par le relais »; the measurements | « HDR »; sound; the microphone; the camera; the mouse; « Touches » (Windows, Linux) | full screen; « Se déconnecter » (power, low-key: one can come back); « Rendre l'ordinateur » (key, in red, confirmation « Rendre l'ordinateur à {hôte} ? »).

**Statistics** (a click on the measurements, the S shortcut, or « Statistiques » in the icon's menu; a second click or the cross closes them): title « Statistiques », the latency in large type and « de la capture à votre écran » (“from capture to your screen”), the shortcut, a « Fermer les statistiques » cross (“Close statistics”); then the path of a frame (see “Statistics”); « Latence » and « Débit » over the last 30 seconds (“Latency”, “Bitrate”), with the « 1 image » mark (“1 frame”); under « Débit », the « Débit visé » line (“Target bitrate”) and, on its graph, a line for the target: “42 / 60 Mbit/s · lowered, network saturated” (« 42 / 60 Mbit/s · baissé, réseau saturé ») when the automatic bitrate lowered it below the profile's, “60 Mbit/s · Auto”, “80 Mbit/s · set by hand” (« réglé à la main »), “320 Mbit/s · PyroWave, Auto paused” (« PyroWave, Auto en pause »), “20 Mbit/s · window hidden” (« fenêtre cachée ») when the app caps it, “15 Mbit/s · lightened stream” (« flux allégé ») after a fallback (docs/debit-automatique.en.md); « Pertes · 30 s » (“Losses · 30 s”): « Images perdues », « Paquets perdus », « Images clés » (“Lost frames”, “Lost packets”, “Keyframes”), on the Mac also « Remplacées », and « Session : 12 images, 40 paquets » (“Session: 12 frames, 40 packets”); finally « Chemin » (« Direct », « Par le relais »), « Réseau » (« aller-retour 2,1 ms » / “round trip 2.1 ms”), « Image » (resolution, the format as the device decodes it, frames per second: « 2560 × 1440 · HEVC · matériel · 120 i/s »), « Mesures » (« 98 % des images » / “98% of frames”) and « Son » (« tampon 20 ms · sortie 12 ms » / “buffer 20 ms · output 12 ms”). Before the first frame: « Les statistiques s'affichent dès que l'image arrive. » / “Statistics appear as soon as the picture arrives.” On the Mac, the panel is at the bottom right of the picture and stays open with the bar closed, letting clicks and moves through; on Windows, the bar being a window of its own, it hangs below the measurements and the bar stays open as long as it is (a click on the picture closes them).

**With a Mac Station** (KyberNow Station for Mac, see `docs/station-macos.en.md`): the app knows it from the system the Station gives (`os` in `GET /api/stream/info`, “macOS 26.1”: it starts with “macOS”), kept for each computer; the rule lives in the core (`keymap::station_is_mac`). A PC keyboard reaches the Mac with the ⊞ key as ⌘ (the Mac's kynput translates the Windows key set). Two things change:

| Item | French | English |
|---|---|---|
| The bar's “Keys” menu (Windows, Linux) | « ⌘ ⇥ » (changer d'app), « ⌘ Espace » (Spotlight), « ⌃ ⌘ Q » (verrouiller), « ⇧ ⌘ 5 » (capture), « ⌥ ⌘ Échap » (forcer à quitter); these keys go as they are, without the swap below | “⌘ ⇥” (switch app), “⌘ Space” (Spotlight), “⌃ ⌘ Q” (lock), “⇧ ⌘ 5” (screenshot), “⌥ ⌘ Esc” (force quit) |
| Setting, Controls section, only with a Mac Station | « Ctrl agit comme ⌘ », on by default, kept for each computer; help « Copier, coller et les autres raccourcis du Mac avec Ctrl, comme sur un PC. La touche ⊞ devient ⌃. » | “Ctrl acts as ⌘”; “Copy, paste and the Mac's other shortcuts with Ctrl, as on a PC. The ⊞ key becomes ⌃.” |
| “Keyboard” help in the settings (Windows) | « Face à un Mac, la touche ⊞ est ⌘, et Ctrl agit comme ⌘ si l'option est allumée (⊞ devient alors ⌃). ⌃⌘Q et ⌥⌘Échap passent par le menu Touches de la barre de session. » | “With a Mac, the ⊞ key is ⌘, and Ctrl acts as ⌘ when the option is on (⊞ then becomes ⌃). ⌃⌘Q and ⌥⌘Esc go through the Keys menu of the session bar.” |

“Ctrl acts as ⌘” swaps the two keys, left and right: Ctrl goes as ⌘ and ⊞ as ⌃ (as macOS does when you swap a PC keyboard's keys). The core does the swap (`InputPipeline::set_control_as_command`, `keymap::control_as_command`, on by default with a Mac), on typed keys only; a key held while the setting changes is released as it went. A change in session applies at once. The app's shortcuts (⌃⌥⌘ / Ctrl+Alt+Shift + letter) do not change. With a PC, none of this appears. A share guest or a friend borrowing, whose app does not know the computer's system, keeps a PC's menu and no swap. In each app:

- **Wherever the app offers a PC's key combinations** (the bar's “Keys” menu, the living room's key tiles or menu, keys above the on-screen keyboard): with a Mac, the Mac's ones above, instead. Above the on-screen keyboard (phones, tablets), the sticky keys are labelled « ⌃ », « ⌥ », « ⌘ » instead of « Ctrl », « Alt », « ⊞ » (same keys sent), and « ⌥ ⌘ Échap » replaces « Ctrl+Alt+Suppr ».
- **Wherever a PC keyboard can type** (Windows, Linux, Android, LG TVs, web client): “Ctrl acts as ⌘” in the computer's settings, with a Mac only. The web client, without the core, does the same swap itself (same release rule).
- **Controller as mouse** (living-room apps, phones and tablets that have it: A click, L1 / R1 to go from one window to another, Start the ⊞ key): with a Mac, L1 / R1 hold ⌘ instead of Alt (⇧ ⌘ ⇥ / ⌘ ⇥, ⌘ released a moment after the last press, like Alt), and Start sends “⌘ Space” (Spotlight) instead of ⊞ alone.
- **Apple apps** (Mac, iPhone and iPad, Apple TV): ⌘ goes as ⌘ with a Mac (the ⊞ key for Kyber, which the Mac receives as ⌘); the setting “⌘ key sent as Ctrl” (Mac) / “⌘ sent as Ctrl” (iPhone, iPad) is not shown with a Mac Station and does not apply there; what is kept for PCs does not change. No “Ctrl acts as ⌘”: an Apple keyboard has its ⌘ key.

**With a Linux Station** (KyberNow Station for Linux, see `docs/station-linux.en.md`): nothing specific in the apps, a computer running Linux presents itself as a PC. Its system (`os` in `GET /api/stream/info`) is the name the distribution gives (“Ubuntu 24.04.5 LTS”, “Fedora Linux 42 (Workstation Edition)”), shown as is in the Station section. A PC keyboard reaches it as is (the ⊞ key is the Super key of GNOME and KDE); the “Keys” menu keeps a PC's (“Ctrl + Alt + Del” opens GNOME's log-out window). Virtual displays on GNOME, KDE Plasma, Sway and Hyprland, with no new text; on X11 and other desktops, the computer's displays as they are (rules for a Station without virtual displays, already written). What a Linux Station cannot do yet, it does not announce, and the apps follow the rules already written, with no new text: no HDR (`hdr` false: SDR picture, as with a display without HDR; GNOME and KDE don't give an HDR virtual display yet, see `docs/station-linux.en.md`, “HDR”). PyroWave and computer lending come through the same fields, with no new text: `pyrowave.available` true when the Station's graphics card is suitable (Vulkan 1.3, checked by Admin), otherwise `reason: "vulkan"` and Admin's Windows texts; `lend.available` true when the Station has its console mode (the `kybernow-tray --console` agent), otherwise “Lending isn't possible on this computer.”. In Admin, a Linux Station's lending texts speak of “your session”, never of Windows (“Your session”, “your session must stay open”), as on Mac. Controllers are there (`gamepads` absent: available): a PlayStation controller arrives as a DualSense, the others as an Xbox controller, as on Windows and the Mac. So is the pen (“pen available” message): a graphics tablet for the computer, same rules as “Pen” below, without rotation. The clipboard follows the same rules as on the other Stations, on X11 as on Wayland. So are updates (`station.update`): “Update the Station” as with a PC, and a release that needs the installer says so the same way (its .deb or .rpm package installs on the computer). The day one of these features comes to Linux, the Station announces it and the apps get it unchanged.

**Immersive mode** / « Mode immersif » (Mac, Windows and Linux apps): the whole keyboard goes to the computer, system shortcuts included: ⌘ ⇥, ⌘ Space, ⌘ Q, ⌃ ← → and Mission Control on Mac; Alt+Tab, the ⊞ key, Ctrl+Esc and Alt+Esc on Windows and Linux. Only the app's shortcuts (⌃⌥⌘ + letter on Mac, Ctrl+Alt+Shift + letter on Windows and Linux) and what the system never lets go (Ctrl+Alt+Del, ⌥⌘Esc) stay on the device. Computer setting, Controls section, kept for each computer:

| Element | Français | English |
|---|---|---|
| Setting | « Mode immersif » : « En plein écran » (default), « Toujours », « Jamais » | “Immersive mode”: “In full screen”, “Always”, “Never” |
| Help (Windows, Linux) | « Alt+Tab, la touche ⊞ et les autres raccourcis du système vont à l'ordinateur. Ctrl+Alt+Maj + lettre reste à l'app. » (Linux, whose font lacks the ⊞ sign, writes “the Windows key”, as its “Keys” menu does) | “Alt+Tab, the ⊞ key and the system's other shortcuts go to the computer. Ctrl+Alt+Shift + letter stays with the app.” |
| Help (Mac) | « ⌘ ⇥, ⌘ Espace et les autres raccourcis du Mac vont à l'ordinateur, et ⌘ y devient la touche ⊞. ⌃⌥⌘ + lettre reste à l'app. »; with a Mac Station: « ⌘ ⇥, ⌘ Espace et les autres raccourcis vont à l'ordinateur. ⌃⌥⌘ + lettre reste à l'app. » | “⌘ ⇥, ⌘ Space and the Mac's other shortcuts go to the computer, and ⌘ becomes the ⊞ key there. ⌃⌥⌘ + letter stays with the app.”; with a Mac Station: “⌘ ⇥, ⌘ Space and the other shortcuts go to the computer. ⌃⌥⌘ + letter stays with the app.” |
| Missing permission (Mac), under the setting | « Autorisez KyberNow Link dans Réglages Système › Confidentialité et sécurité › Accessibilité. » and the button « Ouvrir les réglages » | “Allow KyberNow Link in System Settings › Privacy & Security › Accessibility.”; “Open Settings” |

- **When**: the session's window is in front and its picture has the keyboard (not a menu, panel or field of the bar). “In full screen”: its window is full screen (with several displays, always: the windows are); “Always”: in a window too; “Never”: the system's shortcuts stay on the device, even in full screen. A change applies at once, in session too. You leave it by leaving full screen (shortcut F) or by moving to another window with the mouse.
- **No immersive mode** for a guest without the keyboard and mouse (Alt+Tab goes back to the system when the host takes them back), nor for a game on a PlayStation console (the keyboard stands in for a controller there).
- **Mac**: the Mac only lets an app allowed in Accessibility take its shortcuts. The first time the mode should turn on, the app asks for it (system window); while the permission is missing, nothing changes (as with “Never”) and the setting says so. In immersive mode, with a PC, ⌘ goes as the ⊞ key and Ctrl stays Ctrl: “⌘ key sent as Ctrl” does not apply there, and comes back when the mode turns off (held keys are released on the way). With a Mac Station, ⌘ already goes as ⌘.
- **Windows, Linux**: “In full screen” is the rule from before this setting (the keyboard was only taken in full screen).

**Pen** / « Stylet » (every app where a pen exists: graphics tablet on Mac, Windows and Linux; Apple Pencil on iPad; Android stylus; web client, through Pointer Events; details in [stylet.en.md](stylet.en.md)): the computer gets a real pen, not a mouse. It has pressure, tilt, rotation (when the device gives it), the eraser end, the pen button and hover (the pen just above the surface, not touching it). Drawing apps use them for the stroke's width and opacity; apps that know nothing of pens see a left click, as with a pen plugged into the computer.

- **Always**: no setting, nothing in the bar, no new text. The pen follows the desktop mouse; with the game mouse (captured pointer), it stays a mouse as before.
- **Older Station** (which does not know the pen): the pen stays a mouse at an absolute position, as before, with no notice. The Station says it can take a pen within kynput itself (a “pen available” message when the app arrives), so a share guest and a borrowing friend get it too, without reading `/api/stream/info`.
- **Rights**: the pen is the mouse; a guest has it with “the keyboard and mouse”, a spectator does not.
- **Several displays**: the pen goes to the display of the window it writes in, like the mouse. The pointer bar also opens when the pen hovers against the top edge.
- **Buttons**: whatever the computer does with them (on a PC, pen button held and tip down: right click; on a Mac, whatever the drawing app sets). The eraser end erases in apps that know it.
- **Touch** (iPad, Android): the pen always writes where it touches, whatever the touch mode (trackpad or direct touch); the finger keeps its mode. The Apple Pencil's double tap or squeeze (switch tool) stays with the iPad.

**Controllers unavailable**: a Station may not be able to create controllers (a Mac Station until Apple grants its entitlement, see `docs/station-macos.en.md`). It says so in `gamepads` of `GET /api/stream/info`: `{ "available": false, "reason": "entitlement" }`; absent (older Stations): available. The core reads it (`Info::gamepads_available`), the app keeps it for each computer. Controllers still drive the app (bar, menus, Start + Select + L1 + R1 combination); the computer does not see them.

| Element | Français | English |
|---|---|---|
| Home, living-room apps | « Manettes indisponibles sur cet ordinateur » instead of « 2 manettes », « 1 manette », « Aucune manette » | “Controllers unavailable on this computer” |
| Brief notice, all apps | once per session, when the picture arrives if a controller is connected, otherwise when the first one connects: « Manettes indisponibles sur cet ordinateur. » (controller icon) | “Controllers unavailable on this computer.” |

A share guest or a borrowing friend does not see this notice (their app does not read the host's `/api/stream/info`).

**Brief notices** (« Léa demande à rejoindre. », « Vous pouvez jouer à la manette. », « Image allégée pour suivre. »…): a small capsule centred at the top of the picture, without the bar, with its icon, for a few seconds; it does not take the keyboard from the session. A request's capsule can be picked: the bar opens with the « Partager » or « Prêter l'ordinateur… » panel. With the bar open, no capsule: the pills and chips already show it.

With touch (iPhone, iPad, Android outside the TV):

- **Keyboard**: it opens from the bar; a touch on the picture folds it away (that touch does not click on the computer).
- **On-screen controller** / « Manette à l'écran »: a bar button, remembered for each computer. Two fixed layouts, for both thumbs in landscape: “Full” / « Complète », a whole Xbox controller (two sticks which click: L3, R3; the D-pad; A, B, X, Y; LB, RB, LT and RT, progressive; View, Menu and Guide); “Simple” / « Simple », the left stick, the D-pad, A, B, X, Y and Menu. A small button on the controller switches between them (accessible label “Simple controller” / “Full controller”, « Manette simple » / « Manette complète »), the choice remembered for each computer. The computer sees it as an Xbox 360 controller, in the first free player slot. While it is shown, the rest of the screen does not act as a mouse. Settings in Controls: “Layout” (Full, Simple), “Size” (Small, Medium, Large), “Opacity” and “Haptic feedback” (Off, Light, Medium, Strong; Medium by default). Every press can be felt and seen: the button lights up and sinks slightly; a crisp click when pressing a button, the D-pad or a stick; for the triggers, a light click at the start of the travel and a firm click at the end; for the sticks, a tick when the stick leaves the centre, a very light tick at each change of direction (eight directions) and a tick at the edge, never a continuous vibration. While the controller is shown, the game's rumble vibrates the device. Without a haptic engine (iPad), only the visual feedback remains.

## HDR

Full contract: [hdr.en.md](hdr.en.md). HDR is only offered on a device whose screen is HDR; elsewhere, no trace of it (no chip, no setting, no bar item).

| Element | Français | English |
|---|---|---|
| Home chip, next to the profile | « HDR », on or off; accessible label « HDR, allumé » / « HDR, éteint »; help: « Couleurs plus riches et lumières plus vives, sur cet écran HDR. » | “HDR”; “HDR, on” / “HDR, off”; “Richer colours and brighter highlights, on this HDR screen.” |
| Inactive chip (reason, the first that applies) | « L'ordinateur ne propose pas le HDR. »; « Le HDR demande Windows 11 24H2 sur l'ordinateur. »; « La carte graphique de l'ordinateur ne fait pas le HDR. »; « Le HDR demande le format HEVC ou AV1. »; « Cet appareil ne décode pas le HDR. »; « L'écran de l'ordinateur n'est pas en HDR. » | “The computer doesn't offer HDR.”; “HDR needs Windows 11 24H2 on the computer.”; “The computer's graphics card doesn't do HDR.”; “HDR needs the HEVC or AV1 format.”; “This device can't decode HDR.”; “The computer's display isn't in HDR.” |
| Home “Display” line, HDR on | « Écran de 3840 × 2160 à 120 Hz, à la taille de cet écran, en HDR »; several displays: « 2 écrans, à la taille de ceux de cet appareil, en HDR » (as soon as one display is) | “3840 × 2160 display at 120 Hz, sized for this screen, in HDR”; “2 displays, sized for this device's screens, in HDR” |
| Bar and icon menu, during a session | « HDR », checkbox, next to the profile; changing it restarts the picture (“First frames” step) | “HDR”, checkbox |
| Windows couldn't switch to HDR | « L'ordinateur n'a pas pu passer en HDR : image en SDR. » (once per session, like the other session notices) | “The computer couldn't switch to HDR: picture in SDR.” |
| Admin, Picture and sound page | « HDR » : « Si l'appareil le demande » (default) or « Jamais »; help: « Couleurs plus riches et lumières plus vives, pour les appareils à écran HDR : l'écran virtuel passe en HDR pendant leur session, puis revient comme avant. Sans écran virtuel, l'image garde le HDR de l'écran de l'ordinateur. »; below the help, when the computer can't, the chip's reason (« Le HDR demande Windows 11 24H2 sur l'ordinateur. », « La carte graphique de l'ordinateur ne fait pas le HDR. », « L'écran de l'ordinateur n'est pas en HDR. »); pendant une session HDR, sous l'aide, la vérification de chaque écran (« HDR : les jeux voient 1 000 nits. », « Windows n'a pas pris le profil : les jeux voient 1 000 nits au lieu de 1 600. », « HDR : les jeux voient 1 000 nits, mais le blanc du bureau est à 80 nits au lieu de 240. », « L'écran virtuel n'est pas en HDR. », « Windows ne donne pas les luminosités de l'écran virtuel. », précédée de « Écran 2 · » à plusieurs écrans); then « Bureau en HDR » : « Gamma 2.2 (conseillé) » or « sRGB de Windows », help: « En HDR, Windows affiche les fenêtres et le texte avec la courbe sRGB, qui éclaircit les noirs. Le gamma 2.2 les garde comme sur un écran SDR. Les luminosités de l'écran de l'appareil sont données à Windows dans les deux cas. » (inactive on « Jamais ») | “HDR”: “When the device asks” or “Never”; “Richer colours and brighter highlights, for devices with an HDR screen: the virtual display switches to HDR during their session, then goes back to how it was. Without a virtual display, the picture keeps the HDR of the computer's display.”; “The computer's display isn't in HDR.”; during an HDR session, below the help, each display's check (“HDR: games see 1,000 nits.”, “Windows didn't take the profile: games see 1,000 nits instead of 1,600.”, “HDR: games see 1,000 nits, but the desktop white is at 80 nits instead of 240.”, “The virtual display isn't in HDR.”, “Windows doesn't give the virtual display's brightness.”, preceded by “Display 2 · ” with several displays); “Desktop in HDR”: “Gamma 2.2 (recommended)” or “Windows sRGB”; “In HDR, Windows shows windows and text with the sRGB curve, which lightens blacks. Gamma 2.2 keeps them as on an SDR screen. The brightness of the device's screen is given to Windows either way.” |
| HDR calibration: access | App settings: « Réglage du HDR » / “HDR calibration”, with the state below: « Automatique » ou « Réglé à l'œil » ; Settings › Picture, under « HDR »: link « Régler le HDR de cet écran… » | “HDR calibration”; “Automatic”, “Calibrated by eye”; “Calibrate HDR for this screen…” |
| HDR calibration: introduction | « Quatre étapes pour que les jeux connaissent les vraies lumières de cet écran. » | “Four steps so games know this screen's real brightness.” |
| Step 1 | « Luminosité maximale » ; « Montez jusqu'à ce que le symbole du centre disparaisse à peine. » | “Maximum brightness”; “Raise it until the symbol in the middle just disappears.” |
| Step 2 | « Luminosité sur tout l'écran »; same sentence | “Full-screen brightness”; same sentence |
| Step 3 | « Noir » ; « Baissez jusqu'à ce que le symbole disparaisse à peine. » | “Black”; “Lower it until the symbol just disappears.” |
| Step 4 | « Blanc du bureau » ; « Choisissez la luminosité des fenêtres et du texte de l'ordinateur. » | “Desktop white”; “Pick how bright the computer's windows and text look.” |
| Value shown | « 1 000 nits » (steps 1, 2, 4), « 0,05 nit » (step 3) | “1,000 nits”, “0.05 nits” |
| Buttons | « Suivant », « Retour », « Terminer », « Annuler » ; « Revenir au réglage automatique » | “Next”, “Back”, “Done”, “Cancel”; “Back to automatic” |
| After « Terminer » during a session | « Le réglage vaudra à la prochaine session. » | “The calibration will apply from the next session.” |

HDR is **on** the first time a device with an HDR screen sees a computer that offers it; after that the app keeps the choice for that computer. It does not count in the profile: changing it does not turn the profile into “Custom”. With HDR on: “Sharp text (4:4:4)” inactive (help below), the Desktop profile sends 4:2:0, the “Auto” format picks HEVC or AV1. With several displays, each display of the session is HDR if the device screen that shows it is; a device screen that loses or regains HDR during a session switches its picture to SDR or HDR, without a message. Share guests: no chip; a guest with an HDR screen gets HDR when the host is in HDR, otherwise a separate SDR picture. The friend the computer is lent to has the chip (“Borrow” screen) and the bar item, like a host. The browser has no HDR. The “HDR calibration” screen ([hdr.en.md](hdr.en.md), “HDR calibration”) only exists on an HDR screen; its patterns are drawn in HDR by the app, without a computer; touch, arrows, scroll wheel or controller (left, right) change the value; values kept for that screen of the device.

## Sharing (KyberNow Share)

Contract: [kybernow-share.en.md](kybernow-share.en.md), and [relais.en.md](relais.en.md) away from home. Every Link app and the web client, the same texts everywhere.

**The host, in their session**: « Partager » / “Share” in the bar (presented like the other menus of the app's bar, a share icon). It opens a panel:

| Item | French | English |
|---|---|---|
| Title | « Partager {nom} » (the computer) | “Share {name}” |
| Code | « 123 456 » (six digits, in groups of three) | |
| Code, QR code | label « Code »; QR code of the apps' link (`link`, `kybernow://join…`), accessible label « QR code du partage » | “Code”; “QR code for the share” |
| Link | « Copier le lien »: copies the web client's address (`web`); then « Lien copié. », or « Copie impossible : sélectionnez le lien à la main. » with the address to select; no copy without a useful clipboard (Apple TV, TVs, living room) | “Copy link”; “Link copied.”; “Could not copy: select the link by hand.” |
| Help | « Sur le même réseau : « Rejoindre avec un code » dans KyberNow Link, ou ce lien dans un navigateur. » | “On the same network: “Join with a code” in KyberNow Link, or this link in a browser.” |
| Help, relay reached (`relay.state` `joined`) | « Sur le même réseau ou ailleurs : « Rejoindre avec un code » dans KyberNow Link, ou ce lien dans un navigateur. Hors de chez vous, vous acceptez chaque invité. »; “Copy link” then copies the relay's link | “On the same network or elsewhere: “Join with a code” in KyberNow Link, or this link in a browser. Away from home, you accept each guest.” |
| Relay link being prepared (`relay.state` `connecting`: connecting to the relay or code not reserved yet), under the same-network help | « Préparation du lien hors de chez vous… » with a waiting indicator; “Copy link” inactive until the relay link (10 s at most, then it copies the network link); the QR code waits too | “Getting the away-from-home link ready…” |
| Relay unreachable (`relay.state` `unreachable`), under the same-network help | « Pas encore joignable hors de chez vous. » | “Not reachable away from home yet.” |
| Requests (guests who came through the relay, `pending`), above the guests, only when there are some | title « Demandes »; « {prénom} demande à rejoindre », below « {app} · hors de chez vous »; buttons « Accepter » and « Refuser »; « Accepter » inactive when the four places are taken; a request arriving during the session: brief notice « {prénom} demande à rejoindre. » (choosing it opens the panel); subtitle of the “Share” tile in tile menus: « 1 demande · 2 invités » (requests first) | “Requests”; “{name} wants to join”, below “{app} · away from home”; “Accept” and “Decline”; notice “{name} wants to join.”; tile “1 request · 2 guests” |
| List | title « Invités »; none: « Personne pour l'instant. » | “Guests”; “Nobody yet.” |
| A guest | « {prénom} · Spectateur » (no right), « {prénom} · Joueur » (controllers), « {prénom} · Clavier et souris », « {prénom} · Joueur, clavier et souris »; below, their app, their picture quality when it is not the host's (“1080p · 60 Hz”) and « depuis {durée} » | “{name} · Spectator”, “{name} · Player”, “{name} · Keyboard and mouse”, “{name} · Player, keyboard and mouse”; “for {duration}” |
| A guest who came through the relay (`via` `relay`) | « hors de chez vous » after their app: « iPhone ou iPad · hors de chez vous · depuis 3 min » | “away from home”: “iPhone or iPad · away from home · for 3 min” |
| A guest's app (`app`) | Mac, Windows, Linux, « iPhone ou iPad », Apple TV, Android, « Téléviseur LG », « Navigateur » | Mac, Windows, Linux, “iPhone or iPad”, Apple TV, Android, “LG TV”, “Browser” |
| Duration (`since`, the Station's Unix seconds) | « moins d'une minute », « 3 min », « 1 h 12 »; never negative (clocks out of step) | “under a minute”, “3 min”, “1 h 12” |
| Actions on a guest | two independent rights, each given or taken back without touching the other: « Donner les manettes » / « Reprendre les manettes »; « Donner le clavier et la souris » / « Reprendre le clavier et la souris »; then « Retirer » | “Give controllers”; “Take back controllers”; “Give keyboard and mouse”; “Take back keyboard and mouse”; “Remove” |
| New code | « Nouveau code » (guests already there stay) | “New code” |
| End | « Arrêter le partage » (everyone leaves) | “Stop sharing” |
| Limit | « Quatre invités au plus. », only when the four places are taken | “Four guests at most.” |
| Station too old | « Le partage demande KyberNow Station 0.8.0 ou plus récente. » | “Sharing needs KyberNow Station 0.8.0 or later.” |
| Sharing turned off on the computer | « Le partage est coupé sur cet ordinateur (Admin › Appareils Link). » | “Sharing is turned off on this computer (Admin › Link devices).” |

Sharing impossible (Station too old, sharing turned off): “Share” dimmed; with a mouse, inactive with the reason as a tooltip; on touch and with a remote, it stays selectable and opens the panel reduced to the reason.

An open share shows in the bar (a « 2 invités » / “2 guests” chip, from one guest on, never “0 guests”; it reopens the panel), in its always-visible part. Requests have their own chip, next to the guests': « 1 demande », « 2 demandes » (“1 request”, “2 requests”), only when there are some; it also reopens the panel. A request does not count among the guests until it is accepted. In tile menus (Apple TV, TVs, living room, Deck, Android TV), the “Share” tile's subtitle is the number of guests as soon as there is one, otherwise the code (“123 456”) when the share is open, otherwise « Code et QR code » / “Code and QR code”. Closing the session stops the share.

**The PC itself** (KyberNow Station: the icon's menu next to the clock, and Admin's home page): « Partager un écran… » / “Share a screen…”, then the choice of display (« Écran 1 — 3840 × 2160 », the main display first); the same panel (code, link, guests, actions), titled « Partage de l'écran 1 » / “Sharing display 1”. The PC stays usable as usual; no virtual display, no display switched off.

**Station refusals** (Admin, the icon, and the host routes' `error`): « {nom} est en session : le partage se lance depuis cet appareil. » / “{name} is in a session: share from that device.”; « Une connexion est en cours sur cet ordinateur : partagez un écran une fois qu'elle est finie. » / “A connection is in progress on this computer: share a screen once it has ended.”; « Cet écran n'est plus branché. » / “This display is no longer connected.”; « Cet invité n'est plus dans le partage. » / “This guest is no longer in the share.”; « Seul l'appareil en session peut partager cet ordinateur. » / “Only the device in a session can share this computer.”; « Aucun partage en cours. » / “No share in progress.”; restart or update during a share: « Un partage est en cours avec {prénoms} : le redémarrage l'arrêtera » / “A share is in progress with {names}: restarting will stop it” (« l'installation l'arrêtera » / “installing will stop it”).

**Station panel** (Admin: home and Link devices; the icon's panel), besides the texts above:

| Item | French | English |
|---|---|---|
| Card help (Admin) | « Montrez un écran de cet ordinateur à quelqu'un sur votre réseau : il le voit, entend le son, et peut jouer si vous le laissez. Vous continuez d'utiliser l'ordinateur normalement. » | “Show one of this computer's screens to someone on your network: they see it, hear the sound, and can play if you let them. You keep using the computer as usual.” |
| Choice of display | « Choisissez l'écran à partager. »; « Recherche des écrans… »; « Annuler » | “Choose the screen to share.”; “Looking for screens…”; “Cancel” |
| A device's share | « Ouvert depuis la session de {nom}. » | “Opened from {name}'s session.” |
| Share opened for watching (an account's “Watch” right, [comptes.en.md](comptes.en.md)) | « Ouvert pour {prénom}, qui regarde avec son compte. » instead of the code, link, QR code and their help; no “New code”, no requests; the guest list and “Stop sharing” as usual; it closes by itself when its last guest leaves. “Share” (host) or “Share a screen…” (PC) on this share makes it an ordinary one: the code comes back | “Opened for {name}, who is watching with their account.” |
| List | « Participants » when the host is in it; « {nom} · Hôte » | “Participants”; “{name} · Host” |
| Stopped | « Partage arrêté. » | “Sharing stopped.” |
| Admin's header, top of the panel | « Partage en cours · {n} invités »; « Partage en cours » | “Sharing · {n} guests”; “Sharing” |
| Icon, status line | « Partage de l'écran {n} · {n} invités »; with requests: « … · 1 demande » (also at the top of its panel) | “Sharing display {n} · {n} guests”; “… · 1 request” |
| Icon, request | a new request for a share started from the PC opens the panel on the share, once per request (not for a device's share, whose screen is the session's) | — |
| Icon, confirmations | « Un partage est en cours avec {prénoms} : arrêter la Station le coupera. Arrêter la Station ? »; « … : le redémarrage l'arrêtera. Redémarrer la Station ? »; when quitting: « Un partage est en cours avec {prénoms}. La Station peut rester en marche : le partage continue, et vous pourrez rouvrir l'icône depuis le menu Démarrer. » | “A share is in progress with {names}: stopping the Station will end it. Stop the Station?”; “… restarting will stop it. Restart the Station?”; “A share is in progress with {names}. The Station can keep running: the share goes on, and you can reopen the tray icon from the Start menu.” |
| Icon, failure | « Admin n'a pas pu changer le partage ({erreur}). » | “Admin could not change the share ({error}).” |

**The guest**:

| Item | French | English |
|---|---|---|
| Home screen, next to “Add a computer” | « Rejoindre avec un code » | “Join with a code” |
| Screen | title « Rejoindre avec un code »; help « Le code d'un partage, d'un prêt ou d'une invitation. »; « Votre prénom » (suggested: the device's name, remembered); button « Rejoindre » | “Join with a code”; “The code of a share, a loan or an invitation.”; “Your name”; “Join” |
| With a camera (iPhone, iPad, Android phones and tablets) | at the top of the screen, the choice « Code » / « QR code »; a share's QR code read in “Add a computer › QR code” also leads here | “Code” / “QR code” |
| Link | `kybernow://join…` (and the QR code) opens this screen, code and computer filled in; nothing is sent before “Join”; unreadable link: « Ce lien de partage est illisible. » | “This share link cannot be read.” |
| Incomplete code | « Le code a six chiffres. » | “The code has six digits.” |
| Camera | denied: « KyberNow Link n'a pas accès à la caméra. Autorisez-la dans les Réglages, ou tapez le code. »; missing: « Pas de caméra disponible sur cet appareil. Tapez le code. » | “KyberNow Link can't access the camera. Allow it in Settings, or type the code.”; “No camera available on this device. Type the code.” |
| Searching | « Recherche du partage… » | “Looking for the share…” |
| Unknown code | « Aucun partage avec ce code sur le réseau. » | “No share with this code on your network.” |
| Full | « Ce partage est complet (quatre invités au plus). » | “This share is full (four guests at most).” |
| Too many tries (10 a minute) | « Trop d'essais de code : réessayez dans une minute. » | “Too many code attempts: try again in a minute.” |
| Unreadable format (only if the device plays no possible format; otherwise it gets a picture it plays) | « Cet appareil ne lit pas l'image de cet ordinateur ({format}). » | “This device cannot play this computer's picture ({format}).” |
| Sharing unavailable (the Station cannot sign Kyber's token, momentarily) | « Le partage n'est pas disponible pour le moment : réessayez dans un instant. » | “Sharing is not available right now: try again in a moment.” |
| Connecting | the connection's steps, titled « Invité sur {nom} » | “Guest on {name}” |
| Bar (reduced) | what only touches the device (latency and statistics, sound, full screen if there is one, “Picture quality”); « Spectateur » (no right) or « Joueur » (at least one right); « Quitter le partage »; the controller only with controllers; keyboard, mouse and the keys menu only with keyboard and mouse; no profile, no computer settings, no “Computers”, no clipboard | “Spectator” or “Player”; “Leave” |
| Picture quality | on the “Join” screen (below the name) and in the bar: « Qualité de l'image »: « Auto » (suggested), « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz »; remembered for next time; changing it during the session restarts the guest's picture only (“First images” step), never the host's. “Auto”: resolution and frame rate at most those of the device's screen and of what its decoder keeps up with (and 60 Hz at most in a browser); when that is the host's picture, the guest shares it. The host always comes first: a guest's picture is never above the host's (resolution, frame rate, bitrate, 4:4:4; “Same as host” or more: the host's), and nothing a guest does changes the computer's displays or the host's picture. In the bar, in place of the host's profile: a chip (a tile opening a panel in tile menus) showing the quality received (“1080p · 60 Hz”), or the choice while the Station does not know it yet (“Auto”); the five choices, the quality received next to the checked one. In the host's panel, below the guest: “{app} · {quality} · for {duration}”, only for a separate picture whose size is known | “Picture quality”: “Auto”, “Same as host”, “4K · 60 Hz”, “1080p · 60 Hz”, “720p · 30 Hz” |
| Picture adjusted by the Station (brief notice) | falling behind on the host's picture, the guest moves by itself to a lighter picture: « Image allégée pour suivre. » (the same notice when the device's decoder cannot keep up and the app falls back to 720p30 by itself); no further separate picture possible (two at most, one on an Intel card; three and two for a share started from the PC; guests with the same quality share theirs): « L'ordinateur ne peut pas faire une image de plus : vous recevez {qualité}. » (the nearest open picture, lighter) | “Picture lightened to keep up.”; “The computer can't make another picture: you get {quality}.” |
| Rights changed | « Vous pouvez jouer à la manette. »; « Les manettes sont reprises. »; « Vous avez le clavier et la souris. »; « Le clavier et la souris sont repris. »; no right left: « Vous regardez de nouveau. » instead of the taken-back notice; both rights changed at once: a single notice, the right given before the right taken back, controllers before keyboard and mouse (a brief notice) | “You can play with a controller.”; “The controllers were taken back.”; “You have the keyboard and mouse.”; “The keyboard and mouse were taken back.”; “You are watching again.” |
| End | « {hôte} a arrêté le partage. »; « {hôte} vous a retiré du partage. »; otherwise a failure's reason; never “Take back control”; a guest's reconnection offers « Quitter le partage » (not « Se déconnecter ») | “{host} stopped sharing.”; “{host} removed you from the share.” |
| Waiting, through the relay | « En attente de l'accord de l'hôte… », button « Annuler » (withdraws the request); the host's name and the computer's name are only known after approval: the screen stays titled « Rejoindre un partage » | “Waiting for the host to accept…”, “Cancel” |
| Declined | « L'hôte a refusé. » | “The host declined.” |
| No answer (120 s) | « L'hôte n'a pas répondu. » | “The host did not answer.” |
| Relay silent | « Le relais ne répond pas. » | “The relay is not answering.” |
| Too many requests (three waiting for the host) | « Trop de demandes en attente : réessayez dans un instant. » | “Too many requests waiting: try again in a moment.” |

{host} is the host's name (the host device's name), or the computer's name for a share started from the PC. A guest is not added to the device's computers.

**Web client** (browser): the share's link (`https://<address>:<port>/link/?join=123456`, the host's “Copy link”) opens “Join a share” **without Admin's password**, with the code already filled in (“123 456”) and the suggested name (“Chrome on Mac”, remembered for next time); “Join” has to be clicked (it never connects by itself). “Join with a code” is also on Admin's password screen and on the web client's home screen (next to “Settings”), with “Cancel” to go back; the code is typed as six digits, grouped by three as you type. The browser has a single computer, the one serving the page: the code goes to it alone, with no search on the network. The picture's format is checked once the Station has answered (Kyber's engine only loads with the guest's cookie): the browser asks for the host's if it plays it, otherwise H.264 (a separate picture); it only leaves the share at once, saying so, if it plays neither H.264 nor HEVC. “Picture quality”: at “Join” and in the bar (a “1080p · 60 Hz” chip that opens the five choices); “Auto” from the screen (`screen × devicePixelRatio`), 60 Hz at most, and what the browser's decoder accepts at that size (otherwise 3840 × 2160, then 1920 × 1080, then 1280 × 720); a change made too soon (`429`) is sent again after `Retry-After`, without a notice; if the quality could not change: « La qualité de l'image n'a pas pu changer : réessayez. » / “The picture quality could not be changed: try again.”. A decoding error asks for a keyframe instead of cutting; if it comes back, the picture falls back once to 720p30 (« Image allégée pour suivre. »), then the usual failure text. When the share ends, the end card (“Session ended”, the end text) has a single button, “Join with a code”; no “Reconnect”. A guest's bar: latency and statistics, sound, full screen, “Spectator” or “Player”, “Leave”; with “keyboard and mouse”: the mouse mode and the “Keys” menu (on touch: trackpad, direct touch, keyboard); the controllers chip only as a player. No profile, no settings, no clipboard.

**Admin's feed**: « {hôte} partage l'écran {n} » / “{host} is sharing display {n}”; « {prénom} regarde votre écran » / “{name} is watching your screen” (on arrival, or no right left); « {prénom} joue avec vous » / “{name} is playing with you” (a right given); « {prénom} a quitté le partage » / “{name} left the share” (gone, removed, or timed out); « Partage arrêté » / “Sharing stopped”; through the relay: « {prénom} demande à rejoindre (hors de chez vous) » / “{name} wants to join (away from home)”, « {prénom} : demande refusée » / “{name}: request declined”.

## Lending the computer (KyberNow Share)

Contract: [pret.en.md](pret.en.md). Every Link app, the web client, Admin and the icon next to the clock, the same texts everywhere. The launcher and the PC's screen carry the KyberNow Share logo.

**The host**: « Prêter l'ordinateur… » / “Lend the computer…” (session bar of the Link apps, the icon's menu, Admin's home and “Lending” page). It opens a panel:

| Item | French | English |
|---|---|---|
| Title | « Prêter {nom} » (the computer) | “Lend {name}” |
| Duration | « Durée » : « 1 heure », « 2 heures », « 4 heures », « Jusqu'à ce que je reprenne » (default: 2 hours) | “Duration”: “1 hour”, “2 hours”, “4 hours”, “Until I take it back” |
| Steam account | « Compte Steam » : « Son compte Steam » (default), « Votre compte Steam » | “Steam account”: “Their Steam account”, “Your Steam account” |
| Button | « Créer un code » | “Create a code” |
| Code, link, QR code | as for sharing (« 123 456 », « Copier le lien », QR code, relay link being prepared); below « Pour une séance, valable 24 h. » | as for sharing; “For one session, valid for 24 h.” |
| Help | « Votre ami entre ce code avec « Rejoindre avec un code » dans KyberNow Link, ou ouvre ce lien dans un navigateur. » | “Your friend enters this code with “Join with a code” in KyberNow Link, or opens this link in a browser.” |
| Code created from the session | « Votre session se fermera quand votre ami arrivera. » | “Your session will close when your friend arrives.” |
| Cancel | « Annuler le code » | “Cancel code” |
| Warning (once, and in Admin) | « Votre ami joue dans votre session Windows. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance. » | “Your friend plays in your Windows session. They see neither your desktop nor your windows, but a game can read your files: lend to friends you trust.” |
| Lending off in Admin | « Le prêt est coupé dans Admin (page « Prêt »). » | “Lending is off in Admin (“Lending” page).” |
| Station too old | « Le prêt demande KyberNow Station 0.9.0 ou plus récente. » | “Lending needs KyberNow Station 0.9.0 or later.” |

**Requests** (computer in use): notice « {prénom} veut emprunter l'ordinateur. » / “{name} wants to borrow the computer.” with « Accepter » / “Accept” and « Refuser » / “Decline” (the icon's notification, session bar, Admin); below, « {app} · hors de chez vous » when the friend comes through the relay. Accepting from one's session: « Votre session va se fermer. » / “Your session will close.” Time slot: « Créneau de {prénom} : l'ordinateur lui est prêté dans 2 min. » / “{name}'s time slot: the computer is lent to them in 2 min.”

**During a loan, on the host's side**:

| Item | French | English |
|---|---|---|
| Computer state (Link apps' home, icon, Admin) | « Prêté à {prénom} »; with an end: « Prêté à {prénom} jusqu'à 18 h » | “Lent to {name}”; “Lent to {name} until 6 PM” |
| Main button | « Reprendre l'ordinateur » | “Take back the computer” |
| Notice running | « Reprise dans 1:42 » and « Reprendre tout de suite » | “Taking back in 1:42”, “Take back now” |
| PC's screen (cover) | KyberNow Share logo; « Ordinateur prêté à {prénom} » ; « Jusqu'à 18 h » when there is an end; « Pour reprendre l'ordinateur, appuyez sur Entrée. » | “Computer lent to {name}”; “Until 6 PM”; “To take back the computer, press Enter.” |
| PC's screen, after Enter | « Reprendre l'ordinateur ? {prénom} a 2 minutes pour sauvegarder. » ; « Entrée : reprendre · Échap : annuler » | “Take back the computer? {name} has 2 minutes to save.”; “Enter: take back · Esc: cancel” |
| PC's screen, notice | « Reprise dans 1:42 · Entrée : tout de suite » | “Taking back in 1:42 · Enter: right now” |

**The friend**:

| Item | French | English |
|---|---|---|
| Card on the home screen | « Ordinateur de {hôte} » ; état « Disponible », « Occupé », « Prêté à quelqu'un d'autre », « Verrouillé », « Prêt coupé », « Hors ligne »; with a time slot: « Votre créneau : mercredi, 14 h – 18 h » or « Votre créneau, jusqu'à 18 h » | “{host}'s computer”; “Available”, “In use”, “Lent to someone else”, “Locked”, “Lending off”, “Offline”; “Your time slot: Wednesday, 2 PM – 6 PM”, “Your time slot, until 6 PM” |
| Main button | « Emprunter »; in use: « Demander à {hôte} » | “Borrow”; “Ask {host}” |
| Add | « Ajouter un ordinateur » › « Avec un compte d'ami » : « Identifiant », « Mot de passe », « Retenir le mot de passe »; invitation link: « {hôte} vous prête son ordinateur. » and « Ajouter » | “Add a computer” › “With a friend account”: “Username”, “Password”, “Remember password”; “{host} is lending you their computer.”, “Add” |
| Waiting for approval | « En attente de {hôte}… » and « Annuler » | “Waiting for {host}…”, “Cancel” |
| Preparing | the connection steps, with « Préparation de l'ordinateur » as the first step | “Preparing the computer” |
| Bar | « Prêté par {hôte} » ; « Encore 1 h 20 » or « Jusqu'à ce que {hôte} reprenne » ; « Rendre l'ordinateur » (confirmation: « Rendre l'ordinateur à {hôte} ? »); latency and statistics, sound, full screen, mouse and keys, controllers; no Station profile, no clipboard | “Lent by {host}”; “1 h 20 left”, “Until {host} takes it back”; “Give back the computer” (“Give the computer back to {host}?”) |
| Launcher | full screen like a console: the selected app's art in the background (cross-fade, slow zoom, veil, particles); KyberNow Share logo; one tile per app ; « Encore 1 h 20 » ; « Rendre l'ordinateur » | as in the bar |
| Launcher, last played (large 16:9 first tile, focused first) | « Dernière partie » ; the game's name and image ; « Reprendre {jeu} » | “Last played”; “Continue {game}” |
| Launcher, above the row (changes with focus) | « Dernière partie » or « App permise » ; the name, large ; one line: « Reprenez là où vous vous étiez arrêté. » ; Steam: « En Big Picture, avec votre compte Steam. » or « En Big Picture, avec le compte Steam de {hôte}. » ; another app: « Fermez-la pour revenir au lanceur. » | “Last played” or “Allowed app”; “Pick up where you left off.”; “In Big Picture, with your Steam account.” or “In Big Picture, with {host}'s Steam account.”; “Close it to come back to the launcher.” |
| Launcher, controller hints | « A Ouvrir », « B Retour », « ✥ Se déplacer » | “A Open”, “B Back”, “✥ Move” |
| Launcher, time | ring of the time left (amber during the notice, with the “1:42” countdown); the clock « 16:40 » (also on the PC's screen) | “4:40 PM” |
| Launcher, arrival | the two halves of the logo come together; « Préparation de l'ordinateur de {hôte} » (bouncing dots) until the friend's picture arrives, then « Bonne partie, {prénom} » | “Getting {host}'s computer ready”; “Have a good game, {name}” |
| Launcher, opening an app | the app's art, large; « Ouverture de {app}… » (loading bar), then « {app} est ouvert. Fermez-le pour revenir au lanceur. » until the launcher comes back | “Opening {app}…”; “{app} is open. Close it to come back to the launcher.” |
| Launcher, “Give back the computer” | « Rendre l'ordinateur à {hôte} ? » ; « Vos jeux se ferment. Vous pourrez de nouveau l'emprunter plus tard. » ; « Annuler », « Rendre l'ordinateur » | “Give the computer back to {host}?”; “Your games will close. You can borrow it again later.”; “Cancel”, “Give back the computer” |
| Launcher, end | the end (« Vous avez rendu l'ordinateur. », « {hôte} a repris l'ordinateur. », « Le prêt est terminé. », « Votre créneau est terminé. », « Le prêt s'est arrêté. »), then « À bientôt, {prénom} » | “You gave back the computer.”, “{host} took back the computer.”, “The loan is over.”, “Your time slot is over.”, “The loan has stopped.”; “See you soon, {name}” |
| Notice | « {hôte} reprend l'ordinateur dans 2 min : sauvegardez votre partie. » ; « Fin du prêt dans 2 min : sauvegardez votre partie. » ; « Votre créneau se termine dans 2 min : sauvegardez votre partie. » | “{host} is taking back the computer in 2 min: save your game.”; “The loan ends in 2 min: save your game.”; “Your time slot ends in 2 min: save your game.” |
| End | « {hôte} a repris l'ordinateur. » ; « Le prêt est terminé. » ; « Votre créneau est terminé. »; end card with « Fermer » | “{host} took back the computer.”; “The loan is over.”; “Your time slot is over.”; “Close” |
| Refusals | « {hôte} a refusé. » ; « {hôte} n'a pas répondu. » ; « Identifiant ou mot de passe refusé. » ; « Ce code ne sert plus. » ; « L'ordinateur est déjà prêté. » ; « L'ordinateur est verrouillé. » ; « Le prêt est coupé sur cet ordinateur. » | “{host} declined.”; “{host} didn't answer.”; “Username or password refused.”; “This code is no longer valid.”; “The computer is already lent.”; “The computer is locked.”; “Lending is off on this computer.” |
| Coming back | window closed or network lost: « Vous pouvez revenir pendant 15 min. » on the end card, with « Revenir » | “You can come back for 15 min.”, “Come back” |

**Admin, « Prêt » / “Lending” page**: « Prêter l'ordinateur » / “Lend the computer” (switch); « Apps permises » / “Allowed apps” (« Steam », « Ajouter une app » / “Add an app”); « Comptes d'amis » / “Friend accounts” (« Ajouter un ami » / “Add a friend”, « Prénom » / “First name”, « Identifiant » / “Username”, « Mot de passe » / “Password”, « Compte Steam proposé » / “Suggested Steam account”, « Créneaux » / “Time slots”, « Ajouter un créneau » / “Add a time slot”, « Lien d'invitation » / “Invitation link”, « Supprimer » / “Delete”); « Prêt en cours » / “Current loan”.

**Admin, “Lending” page, texts of the state card, the tabs and “Lend now”**:

| Item | French | English |
|---|---|---|
| Header | « Confiez l'ordinateur à un ami quand vous ne vous en servez pas. » | “Hand the computer to a friend while you're not using it.” |
| Lending off | « Prêtez votre PC à un ami, en toute simplicité » ; « Un code pour une séance » ; « Vous donnez un code, votre ami joue le temps choisi depuis son appareil. » ; « Des comptes pour vos amis » ; « Ils empruntent avec leur identifiant quand le PC est libre, sinon ils vous demandent. » ; « Des créneaux » ; « Le mercredi après-midi pour Léa : elle emprunte sans vous demander. » ; « Votre ami joue dans votre session Windows, en mode console : il ne voit que les apps que vous choisissez. Un jeu peut tout de même lire vos fichiers : prêtez à des amis de confiance. » ; « Activer le prêt » | “Lend your PC to a friend, simply”; “A code for one session”; “You give a code, your friend plays for the time you chose, from their own device.”; “Accounts for your friends”; “They borrow with their username when the PC is free, otherwise they ask you.”; “Time slots”; “Wednesday afternoon for Léa: she borrows without asking you.”; “Your friend plays in your Windows session, in console mode: they only see the apps you choose. A game can still read your files: lend to friends you trust.”; “Turn lending on” |
| State card, free | « Ordinateur libre » ; « Personne n'emprunte l'ordinateur » ; « Prêtez-le avec un code pour une séance, ou laissez vos amis emprunter avec leur compte. » ; « Prochain créneau : {name}, {day} à {time} » ; « Créneau de {name} en cours, jusqu'à {time} » ; « Prêter maintenant » | “Computer free”; “Nobody is borrowing the computer”; “Lend it with a code for one session, or let your friends borrow with their account.”; “Next time slot: {name}, {day} at {time}”; “{name}'s time slot is on, until {time}”; “Lend now” |
| State card, code | « Code en attente » ; « Votre ami peut entrer ce code » ; « Valable encore {time} » | “Code waiting”; “Your friend can enter this code”; “Valid for {time} more” |
| State card, request (“{name} wants to borrow the computer.”, “Decline”, “Accept” already exist) | « Demande d'emprunt » ; « L'ordinateur est utilisé : la session en cours se fermera si vous acceptez. » ; « et 1 autre demande : {names} » ; « et {n} autres demandes : {names} » | “Borrow request”; “The computer is in use: the current session will close if you accept.”; “and 1 more request: {names}”; “and {n} more requests: {names}” |
| State card, time slot, preparing, ending | « Créneau » ; « Préparation » ; « Préparation de l'ordinateur pour {name} » ; « Fin du prêt » ; « L'ordinateur revient » | “Time slot”; “Preparing”; “Getting the computer ready for {name}”; “Loan ending”; “The computer is coming back” |
| State card, lent | « Prêté » ; « Depuis {time} » ; « Connecté » ; « Code » ; « Compte d'ami » ; « Créneau » | “Lent”; “Since {time}”; “Connected”; “Code”; “Friend account”; “Time slot” |
| State card, notice (“Taking back in 1:42”, “Take back now” already exist) | « Reprise en cours » ; « Fin du prêt » ; « Fin du prêt dans {time} » ; « {name} a été prévenu : il sauvegarde sa partie. » | “Taking back”; “Loan ending”; “The loan ends in {time}”; “{name} has been warned to save their game.” |
| Tabs | « Prêter » ; « Amis » ; « Réglages » | “Lend”; “Friends”; “Settings” |
| “Lend” tab | « Cette semaine » ; « Les créneaux de vos amis. Pendant le sien, un ami emprunte sans vous demander. » ; « Gérer les créneaux » ; « Aucun créneau pour l'instant. » ; « Maintenant » ; « Derniers prêts » ; « Aucun prêt pour l'instant. » ; « Rendu par {name} » ; « Repris par vous » ; « Fin de la durée » ; « Fin du créneau » ; « Arrêté » | “This week”; “Your friends' time slots. During theirs, a friend borrows without asking you.”; “Manage time slots”; “No time slots yet.”; “Now”; “Recent loans”; “No loans yet.”; “Given back by {name}”; “Taken back by you”; “Time was up”; “End of the time slot”; “Stopped” |
| “Friends” tab, a friend's panel | « Dernier prêt » ; « Nouvel ami » ; « Générer » ; « Votre ami le tape une fois dans son app, qui peut le retenir. » ; « Vous pouvez le changer à chaque demande. » ; « Créer le compte » | “Last loan”; “New friend”; “Generate”; “Your friend types it once in their app, which can remember it.”; “You can change it for each request.”; “Create account” |
| “Settings” tab | « Réglages » ; « Votre prénom » ; « Vos amis lisent « {name} vous prête son ordinateur. » » ; « Prénom enregistré. » ; « Steam en Big Picture » ; « Une tuile Steam dans le lanceur, avec le compte choisi à chaque prêt. » ; « Votre session Windows » ; « Elle doit rester ouverte : tant que le prêt est activé, la Station empêche la veille et le verrouillage automatique. Vous la retrouvez ouverte à la fin de chaque prêt. » ; « Couper le prêt » ; « Plus aucun code ni compte d'ami ne fonctionne. Vos réglages sont gardés. » | “Settings”; “Your first name”; “Your friends read “{name} is lending you their computer.””; “First name saved.”; “Steam in Big Picture”; “A Steam tile in the launcher, with the account chosen for each loan.”; “Your Windows session”; “It must stay open: while lending is on, the Station keeps the computer from sleeping and locking on its own. You find it open at the end of each loan.”; “Turn lending off”; “No code or friend account works any more. Your settings are kept.” |
| “Lend now” (“Duration”, “Steam account”, “Create a code” already exist) | « Prêter maintenant » ; « Il se connecte avec le sien ; vos comptes sont mis de côté. » ; « Il joue avec vos jeux et vos sauvegardes. » ; « Code pour une séance » ; « valable 24 h » ; « Terminé » | “Lend now”; “They sign in with theirs; your accounts are set aside.”; “They play with your games and saves.”; “Code for one session”; “valid 24 h”; “Done” |

**Station texts** (Admin, icon, launcher, web client; for the apps to reuse when they need them):

| Item | French | English |
|---|---|---|
| Time left (launcher, bar) | « Encore {m} min » ; « Encore {h} h » ; « Encore moins d'une minute » | “{m} min left”; “{h} h left”; “Less than a minute left” |
| Times | « 18 h », « 18 h 30 » | “6 PM”, “6:30 PM” |
| Launcher | « Ouverture de {app}… » ; sans app : « Aucune app pour l'instant. » ; confirmation de « Rendre l'ordinateur » : « Annuler » | “Opening {app}…”; “No apps yet.”; “Cancel” |
| Icon menu, during a notice | « Prêté à {prénom} · Reprise dans 1:42 » | “Lent to {name} · Taking back in 1:42” |
| Icon panel | « Quand votre ami arrive, l'écran de ce PC est couvert et vos fenêtres sont réduites jusqu'à la fin du prêt. » ; QR code : « QR code du prêt » ; erreur : « Admin n'a pas pu changer le prêt ({error}). » | “When your friend arrives, this PC's screen is covered and your windows are minimized until the loan ends.”; “QR code for the loan”; “Admin could not change the loan ({error}).” |
| Admin home (card) | « Confiez l'ordinateur à un ami quand vous ne vous en servez pas : un code pour une séance, avec son lien et son QR code. » | “Hand the computer to a friend while you're not using it: a code for one session, with its link and QR code.” |
| Admin, navigation | « Prêt » ; « {n} demande(s) d'emprunt » ; « Demande d'emprunt » | “Lending”; “{n} borrow request(s)”; “Borrow request” |
| “Lending” page, introduction | « Prêtez l'ordinateur à un ami quand vous ne vous en servez pas : il joue depuis son appareil, avec les apps que vous choisissez. À la fin, tout revient comme avant, et votre session Windows reste ouverte. » ; sous l'interrupteur : « Tant que le prêt est activé, la session Windows doit rester ouverte : la Station empêche la mise en veille et le verrouillage automatique. » | “Lend the computer to a friend while you're not using it: they play from their own device, with the apps you choose. Afterwards, everything goes back to how it was, and your Windows session stays open.”; “While lending is on, the Windows session must stay open: the Station keeps the computer from sleeping and locking on its own.” |
| Allowed apps | « Ce que votre ami peut ouvrir depuis le lanceur. Le reste est bloqué. » ; Steam : « En Big Picture, avec le compte Steam choisi à chaque prêt. », « Steam n'est pas installé sur cet ordinateur. » ; « Retirer {app} » ; « Aucune autre app pour l'instant. » ; « Votre ami pourra l'ouvrir depuis le lanceur. Les apps et leurs icônes viennent du menu Démarrer de l'ordinateur. » ; « {app} est permise. », « {app} n'est plus permise. » | “What your friend can open from the launcher. Everything else is blocked.”; “In Big Picture, with the Steam account chosen for each loan.”, “Steam isn't installed on this computer.”; “Remove {app}”; “No other app yet.”; “Your friend can open it from the launcher. Apps and their icons come from the computer's Start menu.”; “{app} is allowed.”, “{app} is no longer allowed.” |
| Friend accounts | « Un compte par ami : il emprunte avec son identifiant et son mot de passe, sans code. » ; « 3 à 32 caractères : lettres minuscules, chiffres, point, tiret, trait bas. » ; « Laisser vide pour le garder » ; « 6 caractères au moins. » ; « Modifier », « Enregistrer », « Annuler » ; « Supprimer le compte de {prénom} ? Il ne pourra plus emprunter l'ordinateur. » ; « Compte de {prénom} enregistré. », « Compte de {prénom} supprimé. », « Aucun compte d'ami pour l'instant. » ; « dernier emprunt {il y a} », « jamais emprunté » | “One account per friend: they borrow with their username and password, no code needed.”; “3 to 32 characters: lowercase letters, digits, dot, hyphen, underscore.”; “Leave empty to keep it”; “At least 6 characters.”; “Edit”, “Save”, “Cancel”; “Delete {name}'s account? They will no longer be able to borrow the computer.”; “{name}'s account saved.”, “{name}'s account deleted.”, “No friend accounts yet.”; “last borrowed {ago}”, “never borrowed” |
| Time slots | « Pendant son créneau, votre ami emprunte sans vous demander, et passe avant vous. » ; « Jours », « Début », « Fin » ; « Retirer le créneau » ; « Chaque créneau a au moins un jour, et finit après son début le même jour. » ; « {jours}, {début} – {fin} » ; « Sans créneau : il demande quand l'ordinateur est utilisé » | “During their time slot, your friend borrows without asking you, and goes before you.”; “Days”, “Start”, “End”; “Remove time slot”; “Each time slot has at least one day, and ends after it starts on the same day.”; “{days}, {start} – {end}”; “No time slot: they ask when the computer is in use” |
| Invitation | « Votre ami scanne ce QR code avec KyberNow Link, ou ouvre ce lien dans un navigateur. Il emprunte avec l'identifiant {identifiant} et son mot de passe. » ; « QR code de l'invitation de {prénom} » | “Your friend scans this QR code with KyberNow Link, or opens this link in a browser. They borrow with the username {login} and their password.”; “QR code of {name}'s invitation” |
| Current loan, friend disconnected | « déconnecté, peut revenir pendant 15 min » | “disconnected, can come back for 15 min” |
| Friend, web client | titre avant de connaître l'hôte : « Emprunter l'ordinateur » ; fin : « Vous avez rendu l'ordinateur. », « Le prêt s'est arrêté. » ; « Trop d'essais : réessayez dans une minute. » | “Borrow the computer”; “You gave back the computer.”, “The loan has stopped.”; “Too many attempts: try again in a minute.” |

**App texts** (added when the apps were merged):

| Item | French | English |
|---|---|---|
| Friend's bar | « Se déconnecter » closes the session without giving the computer back (they can come back for 15 min); « Rendre l'ordinateur » gives it back | “Disconnect”; “Give back the computer” |
| A friend's computer, removing | « Retirer cet ordinateur »; confirmation « Retirer l'ordinateur de {hôte} de cet appareil ? » (elision: « d'Hugo ») | “Remove this computer”; “Remove {host}'s computer from this device?” |
| Invitation link | with the first name (`name`): « {hôte} vous prête son ordinateur. »; without (older link): title « Avec un compte d'ami »; unreadable link: « Ce lien d'invitation est illisible. » | “{host} is lending you their computer.”; “With a friend account”; “This invitation link can't be read.” |
| Password not remembered | on the card, at « Emprunter »: « Entrez votre mot de passe pour emprunter. » | “Enter your password to borrow.” |
| Lending impossible (`lend.available` false) | « Le prêt n'est pas possible sur cet ordinateur. » | “Lending isn't possible on this computer.” |
| End card | eyebrow « KyberNow Share » | |
| Request during a time slot | the time-slot text, without « Refuser » | |
| Lending code and relay | as for sharing: « Préparation du lien hors de chez vous… », « Copier le lien » and the QR code held 10 s at most, then « Pas encore joignable hors de chez vous. » (`code.relay`) | as for sharing |

**Admin's feed**: « Ordinateur prêté à {prénom} » / “Computer lent to {name}” ; « {prénom} a rendu l'ordinateur » / “{name} gave back the computer” ; « Vous avez repris l'ordinateur » / “You took back the computer” ; « {prénom} veut emprunter l'ordinateur » / “{name} wants to borrow the computer” ; « {prénom} : demande refusée » / “{name}: request declined”.

## Session recording

Contract: [enregistrement.en.md](enregistrement.en.md). Every Link app and the web client, same texts everywhere; the texts come from the core (`recording`), the web client has the same ones in its catalogue. The Station writes the file (the computer's picture and sound, never re-encoded); the app only asks for it and shows it. A Station that does not record (before this work) says nothing (`recording` missing from the heartbeat): no button, no pill.

**Who**: the host's device in session, alone, and Admin. A share guest sees it without being able to stop it; the friend borrowing sees nothing (no recording during a lend: a lend that starts stops it).

| Element | Français | English |
|---|---|---|
| Session bar (host), after « Prêter l'ordinateur… » | « Enregistrer » button (filled circle icon); tooltip « Enregistrer la session sur l'ordinateur ({raccourci}) » | “Record”; “Record the session on the computer ({shortcut})” |
| While recording, same place, in the always-visible part of the bar | red pill (`--kn-bad`) « Enregistrement · 02:14 », the circle gently beating (one pulse a second, still when the system asks for less motion); a click stops it; tooltip « Arrêter l'enregistrement ({raccourci}) »; narrow bar: « 02:14 » alone | “Recording · 02:14”; “Stop recording ({shortcut})” |
| Shortcut | ⌃⌥⌘R on the Mac, Ctrl+Alt+Shift+R on Windows and Linux: starts or stops | |
| Tile menus (Apple TV, TVs, living room, Deck, Android TV) | « Enregistrer » tile, subtitle « Sur l'ordinateur »; while recording: « Arrêter l'enregistrement », subtitle « 02:14 », in red | “Record”, “On the computer”; “Stop recording”, “02:14” |
| Icon menu (Mac, Windows), in session, after « Statistiques » | « Enregistrer la session »; while recording: « Arrêter l'enregistrement (02:14) » | “Record the session”; “Stop recording (02:14)” |
| iPhone, iPad, Android (session menu) | like the bar: « Enregistrer »; while recording, the red pill at the top of the picture, always visible, and « Arrêter l'enregistrement » in the menu | |
| Brief notice, started (host) | « Enregistrement lancé sur l'ordinateur. » | “Recording started on the computer.” |
| Brief notice, stopped (host) | « Enregistrement terminé : retrouvez-le dans Admin › Enregistrements. » | “Recording saved: find it in Admin › Recordings.” |
| Share guest, pill | red « Enregistrement », the same circle, no action; tooltip « {nom} enregistre la session. »; inactive tile in tile menus | “Recording”; “{name} is recording the session.” |
| Guest, brief notice (when it starts, or when joining a recorded session) | « {nom} enregistre la session. »; when it stops: « L'enregistrement est arrêté. » | “{name} is recording the session.”; “Recording stopped.” |
| Recording turned off in Admin | dimmed button; with a mouse, inactive with the reason as tooltip; by touch and remote, selectable, it gives the reason as a notice: « L'enregistrement est coupé sur cet ordinateur (Admin › Enregistrements). » | “Recording is turned off on this computer (Admin › Recordings).” |
| Refused when starting (brief notice) | PyroWave or AV1: « PyroWave et AV1 ne s'enregistrent pas : passez en HEVC ou H.264. »; disk: « Moins de 2 Go libres sur l'ordinateur : impossible d'enregistrer. »; folder: « Le dossier des enregistrements est inaccessible (Admin › Enregistrements). »; during a lend: « {prénom} utilise l'ordinateur. »; other: « L'enregistrement n'a pas pu commencer. » | “PyroWave and AV1 can't be recorded: switch to HEVC or H.264.”; “Less than 2 GB free on the computer: can't record.”; “The recordings folder can't be reached (Admin › Recordings).”; “{name} is using the computer.”; “Recording could not start.” |
| Stopped by itself (brief notice, host) | disk full: « L'enregistrement s'est arrêté : le disque de l'ordinateur est plein. »; profile switched to PyroWave or AV1: « L'enregistrement s'est arrêté : PyroWave et AV1 ne s'enregistrent pas. »; a lend starts: nothing (the session closes) | “Recording stopped: the computer's disk is full.”; “Recording stopped: PyroWave and AV1 can't be recorded.” |

The time (« 02:14 », then « 1:02:14 » past an hour) runs from the Station's `started_at`, not the device's: every device shows the same. The pill updates every second, bar closed included. Closing the session stops the recording (the Station closes it when the host leaves). Several displays: only the main display is recorded; nothing changes in the texts. Windows: bar closed, the pill can't be seen (the bar is a window of its own there); the icon menu gives the time. Web client: its bar hides entirely, a small red pill with no action stays in the top-left corner. Android TV: the red pill also stays at the top of the picture, beside the tile.

**Admin** (« Enregistrements » / “Recordings” page):

| Element | Français | English |
|---|---|---|
| Switch | « Permettre l'enregistrement des sessions », on by default; help « L'appareil en session peut enregistrer l'image et le son dans un fichier sur cet ordinateur. Tout le monde dans la session le voit. » | “Allow session recording”; “The device in a session can record the picture and sound into a file on this computer. Everyone in the session sees it.” |
| Folder | « Dossier » and the path; « Modifier » opens a field (« Enregistrer », « Annuler », « Dossier par défaut »); below « {n} Go libres » | “Folder”; “Change”; “Save”, “Cancel”, “Default folder”; “{n} GB free” |
| Duration | « Garder »: « 7 jours », « 30 jours » (default), « 90 jours », « Toujours »; help « Les enregistrements plus anciens sont effacés. » | “Keep”: “7 days”, “30 days”, “90 days”, “Forever”; “Older recordings are deleted.” |
| During a session | « Enregistrer la session de {appareil} »; while recording: the pill and « Arrêter l'enregistrement » | “Record {device}'s session”; “Stop recording” |
| List | « Fichiers » title; none: « Aucun enregistrement. »; each file: date and time (« 26 nov. 2026 · 14:32 »), duration and size (« 12 min · 1,4 Go »); « En cours » for the one being written; « Lire » (player in the page), « Télécharger », « Supprimer », confirmed by « Supprimer cet enregistrement ? Il ne pourra pas être récupéré. » | “Files”; “No recordings.”; “In progress”; “Play”, “Download”, “Delete”; “Delete this recording? It can't be recovered.” |
| Admin formats | duration « 40 s », « 12 min », « 1 h 05 »; size « 820 Mo », « 1,4 Go »; player: « Fermer »; phone tab: « Vidéos » (« Enregistrements » doesn't fit) | “40 s”, “12 min”, “1 h 05”; “820 MB”, “1.4 GB”; “Close”; phone tab “Videos” |
| Admin feed | « {appareil} enregistre la session », « Enregistrement terminé ({durée}) », « Enregistrement arrêté : disque plein » | “{device} is recording the session”, “Recording saved ({duration})”, “Recording stopped: disk full” |

## Invitations

Contract: [invitations.en.md](invitations.en.md). All Link apps and Admin, same texts everywhere; the web client has a single sentence.

| Item | French | English |
|---|---|---|
| Invitation code recognized (“Join with a code”, link, QR code) | écran ou feuille titré « Ajouter {ordinateur} » (sans nom : « Ajouter l'ordinateur ») ; texte « Cette invitation ajoute l'ordinateur à vos appareils. Entrez le mot de passe donné avec le code. » | sheet titled “Add {computer}” (no name: “Add the computer”); text “This invitation adds the computer to your devices. Enter the password given with the code.” |
| Field | « Mot de passe », exemple grisé `xxxx-xxxx-xxxx`, **masqué** (des points) avec un bouton « Afficher » / « Masquer » à droite du champ ; ni correction ni majuscule automatiques ; le clavier de la télécommande suffit (tirets et espaces facultatifs) | “Password”, placeholder `xxxx-xxxx-xxxx`, **masked** (dots) with a “Show” / “Hide” button at the field's right; no autocorrect or autocapitalization; the remote's keyboard is enough (dashes and spaces optional) |
| Buttons | « Ajouter l'ordinateur » (inactif tant que le champ est vide) ; « Annuler » | “Add the computer”; “Cancel” |
| During the call | « Ajout de {ordinateur}… » avec un indicateur d'attente | “Adding {computer}…” |
| Success | l'ordinateur apparaît sur l'accueil, sélectionné ; avis bref « {ordinateur} est ajouté. » ; un ordinateur déjà sur l'accueil (même empreinte d'Admin) prend le nouvel accès au lieu d'être ajouté deux fois | the computer appears on the home screen, selected; brief notice “{computer} has been added.”; a computer already on the home screen (same Admin fingerprint) takes the new access instead of being added twice |
| Refusals, under the field | « Mot de passe refusé. » ; « Ce code ne sert plus. » ; « Cette invitation a déjà ajouté tous ses appareils. » ; « Trop d'essais : réessayez dans {n} min. » ; accès coupé : « {ordinateur} n'est pas joignable hors de chez vous (accès coupé dans Admin). » ; le reste : le texte de l'erreur du cœur | “Password refused.”; “This code is no longer valid.”; “This invitation has already added all its devices.”; “Too many attempts: try again in {n} min.”; access off: “{computer} cannot be reached away from home (access turned off in Admin).”; otherwise: the core's error text |
| Web client | « Ce code sert à ajouter l'ordinateur dans l'app KyberNow Link. » | “This code adds the computer in the KyberNow Link app.” |
| Admin, section (Link devices) | « Invitations », aide « Une invitation ajoute un appareil de loin, sans que vous ayez à l'accepter : un code et un mot de passe, valables quelques jours. » ; aucune : « Aucune invitation. » | “Invitations”, help “An invitation adds a device from afar, without you having to accept it: a code and a password, valid for a few days.”; none: “No invitations.” |
| Admin, create | « Inviter un appareil… » ; « Nom » (exemple « App Review ») ; « Durée » : « 1 jour », « 7 jours », « 30 jours » ; sous la durée « L'invitation et les appareils qu'elle ajoute tombent à la fin de cette durée. » ; « Annuler », « Créer l'invitation » ; deux déjà : bouton inactif, « Deux invitations au plus. » | “Invite a device…”; “Name” (e.g. “App Review”); “Duration”: “1 day”, “7 days”, “30 days”; under the duration “The invitation and the devices it adds expire at the end of this period.”; “Cancel”, “Create the invitation”; two already: “Two invitations at most.” |
| Admin, invitation created | « Code » (« 123 456 ») ; « Mot de passe » avec « Copier » (puis « Copié. ») ; « Notez-le maintenant : il ne sera plus montré. » ; « Valable jusqu'au {date} » ; QR code du lien des apps, libellé « QR code de l'invitation » ; aide « Dans KyberNow Link : « Rejoindre avec un code », puis ce mot de passe. » ; accès activé : « Accès hors de chez vous est activé : l'appareil ajouté pourra se connecter de loin. » ; sans relais : « Sans relais, l'invitation ne marche que sur votre réseau. » | “Code”; “Password” with “Copy” (then “Copied.”); “Write it down now: it won't be shown again.”; “Valid until {date}”; QR code of the apps' link, “QR code for the invitation”; help “In KyberNow Link: “Join with a code”, then this password.”; “Access away from home is on: the added device can connect from afar.”; “Without a relay, the invitation only works on your network.”; “Done” button closing the window |
| Admin, list | « {nom} » ; dessous « Code 123 456 · jusqu'au {date} · {n} / 5 appareils » et les noms des appareils ; « Nouveau mot de passe » ; « Supprimer », confirmé par « Supprimer l'invitation « {nom} » ? Les appareils qu'elle a ajoutés perdent leur accès. » | “{name}”; below “Code 123 456 · until {date} · {n} / 5 devices” and the devices' names; “New password”; “Delete”, confirmed by “Delete the invitation “{name}”? The devices it added lose their access.” |
| Admin, device added by an invitation | « Invitation « {nom} » » sous son nom | “Invitation “{name}”” under its name |

## KyberNow accounts

Contract: [comptes.en.md](comptes.en.md) (batch 3); mock-up: “Link apps” row of https://claude.ai/artifact/RSTsS6BYAt9SN9GEMEgiuc. Every Link app; errors and badges come from the core (`account`), other texts from each app's catalogs. The app neither creates nor deletes accounts (App Store, guideline 5.1.1(v)): it points to the account's page.

| Element | English | Français |
|---|---|---|
| Home with no computer, title | “Find your computers” | « Retrouvez vos ordinateurs » |
| Home with no computer, text | “With a KyberNow account, your computers and the ones opened to you show up here, wherever you are.” | « Avec un compte KyberNow, vos ordinateurs et ceux qu'on vous a ouverts arrivent ici, où que vous soyez. » |
| Main button | “Connect an account” | « Connecter un compte » |
| Help under the button, system window (iPhone, iPad, Mac, Windows, desktop Linux, Android) | “The relay's page opens: password, passkey or two-factor authentication happen there, never in the app. No account yet: “Create an account” on that page.” | « La page du relais s'ouvre : mot de passe, passkey ou double authentification s'y font, jamais dans l'app. Pas encore de compte : « Créer un compte » sur cette page. » |
| Help under the button, code (Apple TV, Android TV, LG TVs, living room, Deck) | “A code appears: you approve it from your phone.” | « Un code s'affiche : vous l'approuvez depuis votre téléphone. » |
| Separator, then the usual buttons | “or without an account”; “Add a computer”, “Join with a code” | « ou sans compte » ; « Ajouter un ordinateur », « Rejoindre avec un code » |
| System window, meanwhile | “Signing in…” | « Connexion au compte… » |
| Success | brief notice “Signed in to {account}.” ({account}: the name, otherwise the e-mail); the account's computers arrive on the home screen | « Connecté à {compte}. » |
| “Cancel” on the relay's page, window closed | nothing: the home screen stays as it is | rien |
| Code, title | “Connect a KyberNow account” | « Connecter un compte KyberNow » |
| Code, steps | “Scan the QR code with your phone,”, “or open {address} and enter the code,”, “then tap “Connect”.” ({address}: `share.kybernow.app/tv`) | « Scannez le QR code avec votre téléphone, », « ou ouvrez {adresse} et entrez le code, », « puis touchez « Connecter ». » |
| Code, the code | “KQ7M-4XPA”, monospaced, large; QR code of the page with the code, labelled “QR code to sign in” | « QR code de connexion » |
| Code, waiting | spinner, “Waiting for your phone… The code changes in {n} min.”; expired: a new code appears by itself | « En attente de votre téléphone… Le code change dans {n} min. » |
| Code, relay | “Relay: {relay}”; “Another relay…”; “Back” | « Relais : {relais} » ; « Un autre relais… » ; « Retour » |
| Code declined on the phone | “Sign-in declined on the phone.”, and “New code” | « Connexion refusée sur le téléphone. », et « Nouveau code » |
| Another relay | title “Another relay”; field “Relay” (e.g. `relais.exemple.org:9443`), help “Your account's relay address, if it isn't share.kybernow.app.”; “Continue”, “Cancel”; refusals: “Unreadable relay address.”, “This relay has no KyberNow accounts.”, “The relay is not answering.” | « Un autre relais » ; « Relais », aide « L'adresse du relais de votre compte, si ce n'est pas share.kybernow.app. » ; « Continuer », « Annuler » ; « Adresse du relais illisible. », « Ce relais n'a pas de comptes KyberNow. », « Le relais ne répond pas. » |
| Origin badge (computer cards and list, only when an account is signed in) | “Account”; with accounts on several relays: “Account · {relay}”; a family's or team's computer: its name (“Les Lignes Claires”); opened by someone else: “Opened by {name}”; paired on this device: “Paired” | « Compte » ; « Compte · {relais} » ; le nom de la famille ou de l'équipe ; « Ouvert par {prénom} » ; « Appairé » |
| One computer once | a computer both paired and on an account (same Admin fingerprint, or same Station at the relay) shows only once: the paired one, with the account's badge; a computer on two accounts, once | un ordinateur une seule fois |
| Account computer, state | like the others: “Online”, “Offline”, “Away from home”; “Connect” opens the session without pairing, on the network if it is there, otherwise through the relay | comme les autres |
| Account computer, refusal | “This computer is no longer on your account.” (it leaves the home screen); otherwise: Admin's text (accounts turned off, device blocked, missing right) | « Cet ordinateur n'est plus sur votre compte. » |
| “Watch” right only (`view`, no `session`) | the card offers “Watch” instead of “Connect”; with both rights, “Watch” is in the computer's menu; contract: [comptes.en.md](comptes.en.md), “Watch, access links and computer groups” |
| Watch, connecting | the connection steps, titled “Watching {name}” |
| Watch, bar | the share guest's bar (“Spectator”, or “Player” if the host gives them a right); “Stop watching” instead of “Leave the share”; with several screens: “Screen” and the list (“Screen 1 — 3840 × 2160”, the main screen first), changing it restarts the picture |
| Watch, end and refusals | “{host} removed you.”; “{name} is lent right now.”; “Sharing is turned off on this computer (Admin › Link devices).”; “This Station can't be watched yet: update it.”; otherwise: Admin's text |
| Device session closed (from the account's page, new password) | “This account was signed out of this device: sign in again.”; its computers leave the home screen | « Ce compte a été déconnecté de cet appareil : reconnectez-vous. » |
| App settings, section | “Accounts”; help “Create an account, change its password or delete it: on the account's page.” | « Comptes » ; aide « Créer un compte, changer son mot de passe ou le supprimer : sur la page du compte. » |
| One account | name, e-mail below, relay (“share.kybernow.app”); “Manage account” opens `https://{relay}/compte`; “Sign out” | « Gérer le compte » ; « Se déconnecter » |
| “Manage account” on a TV | the QR code of `https://{relay}/compte`, below “Scan this code with your phone to open the account's page.” | « Scannez ce code avec votre téléphone pour ouvrir la page du compte. » |
| Settings of an account computer, Station section | “KyberNow account” and the relay instead of the Admin account; no “Remove” (it leaves with the account) | « Compte KyberNow » ; pas de « Retirer » |
| Sign out, confirmation | “Sign out of {account}? Its computers leave this device; the ones you paired stay.”; “Sign out”, “Cancel” | « Se déconnecter de {compte} ? Ses ordinateurs quittent cet appareil ; ceux que vous avez appairés restent. » |
| Add | “Add an account”: the relay first (“share.kybernow.app” offered, “Another relay…”), then sign-in | « Ajouter un compte » |
| No account | “No account.” and “Connect an account” | « Aucun compte. » et « Connecter un compte » |
| Station too old | “This Station doesn't give tokens to KyberNow accounts yet: update it.” | « Cette Station ne donne pas encore de jeton aux comptes KyberNow : mettez-la à jour. » |
| Other relay refusals | “Sign-in didn't go through: try again.” (return refused); “This code is no longer valid: a new one is shown.”; “Too many attempts: try again in {n} min.”; “Unexpected answer from the relay ({error}).” | « La connexion n'a pas abouti : réessayez. » ; « Ce code ne sert plus : un nouveau s'affiche. » ; « Trop d'essais : réessayez dans {n} min. » ; « Réponse inattendue du relais ({erreur}). » |

## Away from home (relay)

Contract: [relais.en.md](relais.en.md). Through the relay, a session looks like a session on the network; only these texts change.

| Item | French | English |
|---|---|---|
| Session bar, through the relay | chip « Par le relais », in the always-visible part of the bar (a tile in tile menus) | “Via the relay” |
| Web client, Settings › Network and Statistics | « WebSocket par le relais : hors de chez vous, l'image passe par le relais de KyberNow (un peu plus de latence). »; « Image par le relais, en WebSocket : un peu plus de latence qu'en WebTransport » | “WebSocket through the relay: away from home, the picture goes through the KyberNow relay (a little more latency).”; “Picture through the relay, over WebSocket: a little more latency than WebTransport” |
| Statistics, “Path” line | « Chemin » : « Direct », « Par le relais », « Par le relais (TCP) » (fallback when UDP does not get through) | “Path”: “Direct”, “Via the relay”, “Via the relay (TCP)” |
| Home screen, reachable only through the relay | state « Hors de chez vous » (instead of « En ligne ») | “Away from home” |
| Refusal, access off | « {nom} n'est pas joignable hors de chez vous (accès coupé dans Admin). » | “{name} cannot be reached away from home (access turned off in Admin).” |
| Refusal, Station absent from the relay | « {nom} n'est pas joignable : l'ordinateur est éteint ou en veille. » | “{name} cannot be reached: the computer is off or asleep.” |
| When to say these refusals | when the computer is opened away from home (instead of the connection), with its name; the home screen only says the state | |
| Through the relay, no heavy action | no Station restart or update from the app away from home (Admin refuses them through the relay); instead: « Hors de chez vous, l'app ne peut ni redémarrer ni mettre à jour la Station. » | “Away from home, the app can neither restart nor update the Station.” |
| Waiting guest, share stopped | « Ce partage est arrêté. » | “This share has stopped.” |

**Admin** (Link devices section):

| Item | French | English |
|---|---|---|
| Section | « Hors de chez vous » | “Away from home” |
| Field | « Relais » (default `share.kybernow.app`; without a port, it is 443; a relay installed elsewhere can carry its port, `relais.exemple.org:9443`), help « Le relais fait passer le partage et vos appareils par internet, hors de chez vous. Laissez vide pour ne pas en utiliser. », button « Enregistrer » (then « Enregistré. ») | “Relay”, help “The relay carries sharing and your devices over the internet, away from home. Leave it empty to use none.”, “Save” (then “Saved.”) |
| State | « Relais joint »; « Relais injoignable ({raison}) », or « Relais injoignable » without a reason; only when an address is set; also in the icon's menu, when a share or the access needs the relay | “Relay reached”; “Relay unreachable ({reason})”, “Relay unreachable” |
| Switch | « Accès hors de chez vous », help « Vos appareils appairés ouvrent cet ordinateur par le relais quand ils ne sont pas sur votre réseau. » | “Access away from home”, help “Your paired devices open this computer through the relay when they are not on your network.” |
| Feed | « {appareil} est en session hors de chez vous »; « Accès hors de chez vous activé », « Accès hors de chez vous coupé » | “{device} is in a session away from home”; “Access away from home turned on”, “Access away from home turned off” |
| Blocking | one address (ten wrong tries): in the feed, « Accès hors de chez vous bloqué 10 min pour {adresse} après trop d'essais. »; all access (safeguard, a hundred wrong tries): « Accès hors de chez vous bloqué 10 min après trop d'essais. », also in the section | “Access away from home blocked for 10 min for {address} after too many attempts.”; “Access away from home blocked for 10 min after too many attempts.” |

## Computer settings

Title: « Réglages de {nom} » / “Settings for {name}”. At the top: « Profil » / “Profile” (Game, Desktop, Max, Custom). Then six sections, in this order (a platform may merge or hide one when it has nothing to put in it, never rename them):

| Section | Français | English | Contents |
|---|---|---|---|
| 1 | Image | Picture | resolution, displays, refresh rate, virtual display, video format, sharp text (4:4:4), HDR, bitrate, frame display, keep the last image |
| 2 | Son | Sound | channels, sound on the computer, and what the platform offers (spatial audio, 5.1 Dolby…) |
| 3 | Commandes | Controls | controller seen by the computer, mouse, keyboard, touch, remote, rumble, shared clipboard |
| 4 | Réseau | Network | picture transport, reconnect automatically, wake on LAN |
| 5 | Statistiques | Statistics | last session, this computer |
| 6 | Station | Station | Admin, restart, update, account, remove this computer |

What applies to the whole app (language, iCloud, the app's updates, controller test, what the device can decode, HDR calibration) goes into the **App settings**, not here.

### Setting labels

| Français | English | Values (FR / EN) |
|---|---|---|
| Définition | Resolution | Comme cet écran / Match this display; Au plus 4K / 4K at most; Sur mesure / Custom |
| Écrans | Displays | Auto ; Cet écran / This display ; Tous les écrans / All displays ; help: « Auto : tous les écrans de cet appareil en Bureau et Max, un seul en Jeu et en Personnalisé. Chaque écran reçoit un écran de l'ordinateur, placé de la même façon. » / “Auto: all of this device's displays in Desktop and Max, one in Game and Custom. Each display gets a display on the computer, laid out the same way.” Inactive, on « Cet écran », with a custom resolution (help: « Avec une définition sur mesure, la session prend un seul écran. » / “With a custom resolution, the session takes a single display.”), with a session in a window (same help as the custom resolution, the window setting the display's size) and without a virtual display (setting off, or a Station without a usable virtual display: then with no help, the greyed row on « Cet écran » stands on its own and the virtual display has its own row; with a custom resolution, the custom resolution's help stays). Mac, Windows and Linux apps ([multi-ecrans.en.md](multi-ecrans.en.md)) |
| Fréquence | Refresh rate | “120 Hz” |
| Écran virtuel | Virtual display | help: « L'ordinateur crée un écran à la taille de cet appareil pendant la session ; les siens s'éteignent, puis reviennent à la fin. » / “The computer creates a display sized for this device during the session; its own displays switch off, then come back at the end.” |
| Format vidéo | Video format | Auto; HEVC; H.264; AV1; PyroWave (only if the computer offers it; see “PyroWave”) |
| Texte net (4:4:4) | Sharp text (4:4:4) | help: « Couleurs complètes : le texte reste net, au prix d'un débit plus élevé. » / “Full colour: text stays sharp, at the cost of a higher bitrate.” Inactive with HDR on, help: « Le HDR n'existe pas en 4:4:4. » / “HDR doesn't exist in 4:4:4.” |
| HDR | HDR | only on an HDR screen; help: « Couleurs plus riches et lumières plus vives, sur cet écran HDR. Le texte net (4:4:4) n'existe pas en HDR. » / “Richer colours and brighter highlights, on this HDR screen. Sharp text (4:4:4) doesn't exist in HDR.”; inactive with the reasons of the “HDR” section |
| Débit | Bitrate | Auto; “80 Mbit/s”; Auto: the profile’s bitrate, lowered when the network is saturated then raised again, paused with PyroWave ([debit-automatique.en.md](debit-automatique.en.md)); a value stays fixed |
| Affichage des images | Frame display | Synchronisé / Synced; Immédiat / Immediate |
| Garder la dernière image | Keep the last image | |
| Son | Sound | Auto; Stéréo / Stereo; 5.1; 7.1; Sur l'ordinateur / On the computer |
| Manette vue par l'ordinateur | Controller seen by the computer | Xbox 360; Fidèle / As is |
| Souris | Mouse | Auto; Souris du bureau / Desktop mouse; Souris des jeux / Game mouse |
| Mode immersif | Immersive mode | En plein écran / In full screen; Toujours / Always; Jamais / Never (see “Immersive mode”) |
| Ctrl agit comme ⌘ | Ctrl acts as ⌘ | only with a Mac Station, on by default (see “With a Mac Station”) |
| Presse-papiers partagé | Shared clipboard | |
| Définition de la caméra | Camera resolution | 720p (default); 1080p; help: « 30 images par seconde. En 1080p, la caméra demande environ 4 Mbit/s vers l'ordinateur. » / “30 frames per second. In 1080p, the camera needs about 4 Mbit/s towards the computer.” Controls section, only when the computer offers the camera (see “Device camera”) |
| Vibrations | Rumble | |
| Transport de l'image | Picture transport | Rapide / Fast; Fiable / Reliable; help: « Rapide convient au réseau local et au jeu. Fiable renvoie ce qui se perd, au prix d'un peu de délai. » / “Fast suits your home network and gaming. Reliable resends what gets lost, at the cost of a little delay.” |
| Reconnexion automatique | Reconnect automatically | |
| Réveil par le réseau | Wake on LAN | « Prêt » / “Ready” with the MAC address learnt from the Station, or « Pas encore d'adresse : connectez-vous une fois à cet ordinateur. » / “No address yet: connect to this computer once.”; help: « Allume l'ordinateur à distance quand il est en veille. Il faut aussi l'autoriser sur l'ordinateur (Admin › Système). » / “Turns the computer on remotely when it is asleep. It must also be allowed on the computer (Admin › System).” |

## Welcome and KyberNow account

Mock-ups: https://claude.ai/artifact/K6tgZaZC8dqeBxGnTBenru (in the app's real style: `brand/v2/logo` logo, extended Archivo in capitals for titles, Martian Mono for labels). Every Link app; the browser's web client is not concerned. Shared logic: `apps/core/src/welcome.rs`.

**Labels.** « Se connecter » (“Connect”) means connecting to a computer; for the account, « Connecter un compte » / “Connect an account” and « Créer un compte » / “Create an account” (see “Principles”).

### Welcome (first launch)

Three screens, shown on the first launch, and **once** after the update that brings them to people with no account (`welcome::should_show`). Replayable: app settings, « Revoir l'accueil » / “Show the welcome again”. Each screen: the app's logo top left, « Passer » / “Skip” top right (screen 3: « Plus tard » / “Later”), the label « BIENVENUE · {n} / 3 » / “WELCOME · {n} / 3”, a big title, a text, the buttons, three progress bars at the bottom.

| Screen | English | Français |
|---|---|---|
| 1, title | “Your computer, on all your devices” | « Votre ordinateur, sur tous vos appareils » |
| 1, text | “Play, work and get back to your desktop from another screen, as if you were sitting in front of it.” | « Jouez, travaillez et retrouvez votre bureau depuis un autre écran, comme si vous étiez devant. » |
| 1, button | “Continue” | « Continuer » |
| 2, title | “With an account, everything follows you” | « Avec un compte, tout vous suit » |
| 2, text | “A KyberNow account, and your computers wait for you everywhere.” | « Un compte KyberNow, et vos ordinateurs vous attendent partout. » |
| 2, benefit 1 | “AWAY FROM HOME”; “Reach your computer from anywhere, with nothing to set up on your router.” | « HORS DE CHEZ VOUS » ; « Rejoignez votre ordinateur de n'importe où, sans rien régler sur votre box. » |
| 2, benefit 2 | “ALL YOUR DEVICES”; “Your computers show up by themselves wherever you sign in.” | « TOUS VOS APPAREILS » ; « Vos ordinateurs apparaissent d'eux-mêmes partout où vous vous connectez. » |
| 2, benefit 3 | “SHARE AND LEND”; “Invite a friend to play, lend your PC, open it to your family or your team.” | « PARTAGER ET PRÊTER » ; « Invitez un ami à jouer, prêtez votre PC, ouvrez-le à la famille ou à votre équipe. » |
| 2, benefit 4 | “YOUR SETTINGS”; “Profile, format and bitrate follow you from one device to another.” | « VOS RÉGLAGES » ; « Profil, format et débit vous suivent d'un appareil à l'autre. » |
| 2, buttons | “Connect an account” (primary); “Create an account”; “Continue without an account” (link) | « Connecter un compte » ; « Créer un compte » ; « Continuer sans compte » |
| 2, help | “{relay}'s page opens: your password never goes through the app.” | « La page de {relais} s'ouvre : votre mot de passe ne passe jamais par l'app. » |
| 3, title | “Add your computer” | « Ajoutez votre ordinateur » |
| 3, text | “Install KyberNow Station on the computer you want to reach: on the same network, it shows up here by itself. A friend gave you a code? Enter it.” | « Installez KyberNow Station sur l'ordinateur à rejoindre : sur le même réseau, il apparaît ici tout seul. Un ami vous a donné un code ? Entrez-le. » |
| 3, row | “COMPUTERS IN REACH”: the computers seen on the network (card “ON THE NETWORK”, “Add”); card “A CODE”, “A friend opens their computer to you: enter the code they gave you.”, “Join with a code”; card “NOT INSTALLED YET”, “Download KyberNow Station on the computer you want to reach.”, link `www.kybernow.app` (opens `https://www.kybernow.app/#telecharger`) | « ORDINATEURS À PORTÉE » ; « SUR LE RÉSEAU », « Ajouter » ; « UN CODE », « Un ami vous ouvre son ordinateur : entrez le code qu'il vous a donné. », « Rejoindre avec un code » ; « PAS ENCORE INSTALLÉ », « Téléchargez KyberNow Station sur l'ordinateur à rejoindre. » |

- “Connect an account”: the usual sign-in (system window, or a code on a TV); once it succeeds, screen 3 is skipped (the account's computers arrive); if cancelled, you stay on screen 2.
- “Create an account”: opens `https://{relay}/compte/inscription?lang={fr|en}` in the device's **browser** (not in the sign-in window: the app creates no account, App Store guideline 5.1.1(v)), then the notice “Create your account in the browser, then come back here: “Connect an account”.” / « Créez votre compte dans le navigateur, puis revenez ici : « Connecter un compte ». » On a TV: the code screen says “No account yet? Create it on this page.” / « Pas encore de compte ? Créez-le sur cette page. »
- TVs (Apple TV, Android TV, LG, Deck, living room): the same screens, with the remote; screen 2 shows on the right the QR code and the code of the code sign-in (texts of “KyberNow accounts”), card title “CONNECT AN ACCOUNT” / « CONNECTER UN COMPTE », and “Continue without an account” as a button.

### With no account, afterwards

- **Account button**: at the top of the home screen, next to the app settings, “Connect an account” with a person icon (phone: icon only, label for accessibility). With an account: it disappears (the account is in the app settings).
- **Account card** in the “YOUR OTHER COMPUTERS” row (last, with the “ON THE NETWORK” glow): « COMPTE KYBERNOW » (“KYBERNOW ACCOUNT”), “Find your computers everywhere” / « Retrouvez vos ordinateurs partout », “On all your devices, even away from home.” / « Sur tous vos appareils, même hors de chez vous. », “Connect an account”, “Create an account”, cross “Close” / « Fermer ». Closed, it comes back 30 days later; once back, its cross says “Don't show again” / « Ne plus afficher » and closes it for good (`welcome::card_visible`). With no computer, the empty home screen of “KyberNow accounts” stays as it is.
- **Useful moment, away from home**: “Connect” on a paired computer that isn't on this network (no route through the relay): a window “Away from home?” / « Hors de chez vous ? », “{name} isn't on this network. To reach it from where you are, connect a KyberNow account: your computer follows you everywhere, with nothing to set up on your router.” / « {nom} n'est pas sur ce réseau. Pour le rejoindre d'où vous êtes, connectez un compte KyberNow : votre ordinateur vous suit partout, sans rien régler sur votre box. » ; “Not now” / « Pas maintenant », “Create an account”, “Connect an account”. At most once per computer and per day.
- **After pairing** (with no account): under the added computer's buttons, a banner “Find it away from home and on your other devices too: connect a KyberNow account.” / « Retrouvez-le aussi hors de chez vous et sur vos autres appareils : connectez un compte KyberNow. » ; “Connect an account”, “Later” / « Plus tard ». Once per computer. It is the banner under the “Connect” row: as long as the Station update banner is there (section “Station updates”), it waits, without being counted as shown (`take_paired_hint` only when it really shows).
- **With the Stations update**: the “Connect an account” button and the account card are not banners and stay displayed; only the banner after pairing gives way.

## Auto profile and adaptive bitrate

Technical contract: [profil-auto.en.md](profil-auto.en.md). Every app except the browser's web client.

- **Profiles**: « Jeu » (“Game”), « Bureau » (“Desktop”), « Max », « Auto », and « Personnalisé » (“Custom”) when the settings depart from them, wherever profiles are listed (home screen, profile menu, icon menu, top of the settings). Auto's icon: wand (sparkles). Sentence: “The best settings for this network and this device.” / « Les meilleurs réglages pour ce réseau et cet appareil. »
- **Choosing Auto** with no kept result, or **“Run the test again”** / « Refaire le test »: opens the test panel. With a kept result: Auto applies right away (its settings), with no test.
- **PyroWave over a cable**: Max and Auto only. Help for “Video format”, Auto, over a cable, in Game and Desktop: “Auto: HEVC. PyroWave is used by Max and Auto, when this device is connected by cable.” / « Auto : HEVC. PyroWave est pris par Max et Auto, quand cet appareil est relié par câble. »

### Test panel

Title “Auto profile” / « Profil Auto ». Subtitle “KyberNow measures your network and this device, then suggests the best settings.” / « KyberNow mesure votre réseau et cet appareil, puis propose les meilleurs réglages. »

During the test, five steps, each with its state (waiting, in progress with a spinner, ticked with its measurement, skipped with its reason):

| Step | English | Français | Measurement once ticked |
|---|---|---|---|
| link | Connection | Liaison | “Cable”, “Wi-Fi”, “Through the relay” (« Câble » / « Wi-Fi » / « Par le relais ») |
| latency | Latency at rest | Latence au repos | “2.1 ms · jitter 0.4 ms” (« 2,1 ms · gigue 0,4 ms ») |
| bandwidth | Bitrate to this device | Débit vers cet appareil | “940 Mbit/s · latency under load +3 ms” (« 940 Mbit/s · latence en charge +3 ms ») ; skipped: “Update {name} to measure the bitrate.” (« Mettez à jour {nom} pour mesurer le débit. ») |
| decode | Decoding on this device | Décodage de cet appareil | “HEVC 4K: 240 fps · PyroWave 4K: 180 fps” (« HEVC 4K : 240 i/s · PyroWave 4K : 180 i/s »; the best of each family) |
| encoder | The computer's encoder | Encodeur de l'ordinateur | “up to 440 Mbit/s in HEVC” (« jusqu'à 440 Mbit/s en HEVC », a bitrate refused); “290 Mbit/s accepted in HEVC” (« 290 Mbit/s acceptés en HEVC », first try); skipped: “PyroWave chosen.” (« PyroWave choisi. »), “Update {name} to try its encoder.” (« Mettez à jour {nom} pour essayer son encodeur. »), “{name} couldn't try its encoder.” (« {nom} n'a pas pu essayer son encodeur. ») |

Button “Cancel” / « Annuler » (stops the test, keeps the previous profile).

**Summary**: title “Suggested settings” / « Réglages proposés », one line per choice, value then reason:

| Setting | English example | Exemple FR |
|---|---|---|
| Video format | “PyroWave” — “connected by cable, and the network can carry it” | « PyroWave » — « relié par câble, et le réseau le porte » |
| | “HEVC” — “PyroWave needs a cable” / “the network can't carry PyroWave” / “{name} doesn't offer PyroWave” / “this device doesn't decode PyroWave fast enough” | « HEVC » — « PyroWave demande un câble » / « le réseau ne porte pas PyroWave » / « {nom} ne propose pas PyroWave » / « cet appareil ne décode pas PyroWave assez vite » |
| Resolution | “3840 × 2160 at 120 Hz” — “matches this display” / “decoding limited to {n} fps in 4K” | « 3840 × 2160 à 120 Hz » — « comme cet écran » / « décodage limité à {n} i/s en 4K » |
| Sharp text (4:4:4) | “On” — “the network and decoding allow it”; “Off” — “bitrate too tight” / “this device can't decode it” | « Oui » — « le réseau et le décodage le permettent » ; « Non » — « débit trop juste » / « cet appareil ne le décode pas » |
| Bitrate | “420 Mbit/s” — “{measured} Mbit/s measured, with a margin”; “based on the connection (Station to update)”; bounded by the encoder: “{name} encodes up to {n} Mbit/s” | « 420 Mbit/s » — « {mesuré} Mbit/s mesurés, avec une marge » ; « selon la liaison (Station à mettre à jour) » ; « {nom} encode jusqu'à {n} Mbit/s » |
| Picture transport | “Fast” — “steady network”; “Reliable” — “unsteady network: what gets lost is resent” | « Rapide » — « réseau stable » ; « Fiable » — « réseau irrégulier : ce qui se perd est renvoyé » |
| Adaptive bitrate | “On” — “follows the network during the session” | « Oui » — « suit le réseau pendant la session » |

Buttons “Apply” / « Appliquer » (Auto becomes this computer's profile with these settings) and “Cancel” / « Annuler ». Link “Run the test again” / « Refaire le test ». Failure (Station unreachable): “The test couldn't reach {name}.” / « Le test n'a pas pu joindre {nom}. » with “Try again” / « Réessayer ».

### “Bitrate” card

- Switch **“Adaptive bitrate”** / « Débit adaptatif », on by default, help: “Lowers the bitrate when the network is saturated, then raises it again. Off, the bitrate never moves.” / « Baisse le débit quand le réseau sature, puis le remonte. Coupé, le débit ne bouge jamais. » With PyroWave, extra help: “With PyroWave it pauses: the resolution goes down instead.” / « En PyroWave, il est en pause : la définition baisse à la place. »
- Auto profile: under the slider, “Chosen by the test of {date} ({network}).” / « Choisi par le test du {date} ({réseau}). » and “Run the test again” / « Refaire le test ».

### During a session

- **Different network from the test** (Auto, network or relay different from the test): a discreet banner at the start, 8 s or the cross, title “Different network from the test” / « Autre réseau que celui du test », “Adaptive bitrate will handle it.” / « Le débit adaptatif s'en charge. », button “Run the test again” / « Refaire le test » (opens the panel after the session, or right away on the apps that can do it during a session). Every app: the panel opens when the session ends (the test measures the free network); “Run the test again”, or Auto chosen with no test during the session, says so in a notice (capsule): “The test will run when the session ends.” / « Le test se fera à la fin de la session. » ; on Apple TV, “Run the test again” is a menu tile (Back), titled “Different network from the test”, like “Switch to HEVC” / « Passer en HEVC ».
- **Statistics**, target bitrate: “60 Mbit/s · fixed” / « 60 Mbit/s · fixe » when adaptive bitrate is off (instead of “set by hand”); “440 Mbit/s · encoder ceiling” / « 440 Mbit/s · plafond de l'encodeur » when the computer's ceiling sets it.
- **The computer's encoder** (every app, no on-screen text): a picture refused at the start lowers the bitrate by 50 Mbit/s and restarts, the status stays “Reconnecting…” (« Reconnexion… »); the bitrate that goes through is kept for this computer (encoder ceiling, [profil-auto.en.md](profil-auto.en.md), “The computer's encoder”) and applies to all its profiles. “Run the test again” measures it again.

## PyroWave

Technical contract: [pyrowave.en.md](pyrowave.en.md). PyroWave is a value of “Video format”, under its own name, in every app that decodes it; mock-ups: https://claude.ai/artifact/9mTTKtn4zLc2zaERtpcDf1.

- **Auto** picks it only when the device is connected by cable (Ethernet), on the same network as the computer, and the computer offers it; otherwise HEVC as today. On Wi-Fi and through the relay, it must be chosen by hand.
- **Apple apps** (Mac, Apple TV, iPhone, iPad): “Auto” is a profile there (Game, Desktop, Max); “Video format” shows “Auto” and “PyroWave” only when the computer knows PyroWave, “Auto” checked when a profile is active; picking it from “Custom” brings back Game ([pyrowave-client-apple.en.md](pyrowave-client-apple.en.md)).
- **Bitrate too high** for the computer's ceiling or the measured bitrate: PyroWave keeps the resolution down to 0.5 bit per pixel (≈ 250 Mbit/s at 4K60); below that, the app lowers the resolution (then the refresh rate) and says so ([pyrowave.en.md](pyrowave.en.md), “Lowered resolution”).
- **Through the relay**: allowed, warning only, no bitrate cap at the relay.
- A lost frame is skipped; Statistics add “Lost frames”.
- **Web client** ([pyrowave-client-web.en.md](pyrowave-client-web.en.md)): a browser can't tell whether it is connected by cable, so **Auto never picks PyroWave**; Auto's help says so when the computer offers it. Picked by hand, it is always warned: the “unknown network” text (below, instead of the Wi-Fi one, which would claim a Wi-Fi the browser can't see), or the relay one. Start banner with “Switch to {format}” (Auto's format: HEVC, or H.264 if this browser doesn't decode HEVC). Amber chip only through the relay: on an unknown network, nothing says there is a problem. Inactive value (“PyroWave” greyed out, its reason in the help and on hover); a PyroWave choice that is no longer offered is kept, “Auto” checked applies. For the host only (a guest, a friend: Auto's format). “PyroWave decoding” joins the Statistics, with “Lost frames”.

| Where | English |
|---|---|
| “Video format” help, PyroWave chosen | “Almost no compression delay, but 5 to 10 times the bitrate of HEVC. Made for an Ethernet cable.” |
| “Video format” help, Auto, by cable | “Auto: PyroWave, because this device is connected by cable. Otherwise HEVC.” |
| “Video format” help, Auto, no cable (computer offers it) | “Auto: HEVC. PyroWave is only used when this device is connected by cable.” |
| Warning under the choice, Wi-Fi | “On Wi-Fi, PyroWave may stutter: it needs about 300 Mbit/s at 1080p. Keep “Auto” if you're not sure.” |
| Warning under the choice, relay | “Away from home, PyroWave goes through the relay and your internet connection: the picture may stutter. Keep “Auto” if you're not sure.” |
| Banner at session start (Wi-Fi or relay, PyroWave chosen; goes after 8 s or on the cross) | title “PyroWave on Wi-Fi” / “PyroWave away from home”; “The picture may stutter.”; button “Switch to HEVC” (restarts the picture in HEVC, for this session only) |
| Banner when the network is saturated (PyroWave: 5 s of losses or queueing in a row, core notice `PyroWaveStrained`; once per session, host only; same shape as the start one, 8 s or the cross) | title “The network is saturated”; “The picture may stutter.”; button “Switch to HEVC” (the same as above) |
| Resolution lowered (banner, and Statistics) | “Resolution lowered to {w} × {h} to fit in {n} Mbit/s.” |
| Session bar, PyroWave on Wi-Fi or via the relay | amber chip “PyroWave on Wi-Fi” / “PyroWave away from home”; the format says “PyroWave” |
| Statistics, extra line | “Lost frames” “0.8%” |
| Sharp text (4:4:4) with PyroWave | help: “With PyroWave, almost free: only the bitrate goes up.” |
| Inactive value | “{name} doesn't offer PyroWave.”; “{this device} can't decode PyroWave.” |
| Inactive value, Raspberry Pi ([pyrowave-client-pi.en.md](pyrowave-client-pi.en.md)) | Pi 4, 3, Zero 2 W: “This Raspberry Pi can't decode PyroWave: it takes a Raspberry Pi 5.”; Pi 5 on Raspberry Pi OS: “This Raspberry Pi can't decode PyroWave: it needs an up-to-date Raspberry Pi OS “Trixie”.”; Pi 5 on KyberNow Link OS: “This Raspberry Pi can't decode PyroWave.” |
| Label of the value, and the Pi's “This device” card | “graphics card” (on the Pi too: PyroWave goes through V3D, not the Pi's decoder); line “PyroWave by the graphics card” |
| Web client, Auto help (computer offers it), after the format's | “Pick PyroWave yourself: a browser can't tell whether this device is connected by cable.” |
| Web client, warning under the choice, unknown network (**proposed**, instead of the Wi-Fi one) | “This browser can't tell whether you're connected by cable. Without an Ethernet cable, PyroWave may stutter: it needs about 300 Mbit/s at 1080p. Keep “Auto” if you're not sure.” |
| Web client, start banner, unknown network (**proposed**) | title “PyroWave, unknown network”; “Without an Ethernet cable, the picture may stutter.”; button “Switch to {format}” |
| Web client, after “Switch to {format}” | “Picture restarted in {format} for this session.” |
| Web client, session Statistics, extra line | “PyroWave decoding” “4.1 ms” |
| Web client, “This browser” (Settings' Statistics section) | “PyroWave decoded by the graphics card (WebGPU)”; “PyroWave: not in this browser” |
| Admin, “PyroWave” card (Picture and sound) | title “PyroWave”, subtitle “An almost delay-free picture, for devices connected by cable.”; chip “Ready · {GPU}” or “Unavailable”; switch “Offer PyroWave”, help “Your devices can pick it in “Video format”. On “Auto”, they use it by themselves when connected to your network by cable.” |
| Admin, “Maximum bitrate” | Auto; 300; 500; 800 Mbit/s; Auto help: “Auto: based on this computer's network port, here {n} Mbit/s. Above that, your devices lower the resolution rather than sharpness.” |
| Admin, card warning | “PyroWave compresses little: it sends 5 to 10 times more data than HEVC. On Wi-Fi or away from home, the picture may stutter; your devices warn you first.” |
| Admin, unavailable (first that applies) | “The graphics driver doesn't provide Vulkan 1.3. Update it, then restart the Station.”; “This graphics card can't run PyroWave.”; “PyroWave starts with the Station: restart it to try it.”; “PyroWave couldn't start: {error}.” |
| Admin, Mac Station (Apple chip, Metal encoder) | chip “Ready · {chip}” (“Ready · Apple M4 Max”); unavailable (`reason: "metal"`), instead of the first two: “This Mac's chip can't run PyroWave.” (a chip older than the Apple7 family: no Apple silicon Mac, so in practice never), then the last two as on Windows; “Maximum bitrate” on Auto: the Mac's port (Ethernet, or the Wi-Fi rate) |
| Admin, Linux Station (Vulkan encoder) | as on Windows: chip “Ready · {GPU}”, unavailable (`reason: "vulkan"`) with the same texts; “Maximum bitrate” on Auto: the Station's port when Admin knows it |
| Admin, current sessions | the “PyroWave” format and bitrate; “Ethernet” or “Wi-Fi” chip; “resolution lowered” when it is |

## Statistics

**In session: path of a frame**, in every app (the session's “Statistics” panel, modelled on the Mac app's `SessionStats`). Six stages, medians of the last second, from the core (`Second.chain`, `apps/core/src/latency.rs`):

| Stage | Place | Help |
|---|---|---|
| “Capture” | On the computer | “From capturing the frame to the start of its encoding” (includes the HDR conversion) |
| “Encoding” | On the computer | “Frame compression by the computer's graphics card” |
| “Sending” | On the computer | “Packetising and sending over the network” |
| “Network” | Network | “From leaving the computer to the full frame received here” |
| “Decoding” | On this device | “Frame decompression by {this device}” |
| “Display” | On this device | “Rendering and waiting for the display's next refresh” |

- Places: “On the computer”, “Network”, “On {this device}”, where {this device} is the name the app uses elsewhere: “this Mac”, “this iPhone”, “this iPad”, “this Apple TV”, “this TV”, “this PC”, “this device” (Linux, Android). French texts in the French version of this page.
- A stage with no measurement (Station too old, first second): “—”. Without the computer's measurements, the device keeps its own decoding and display times.
- Desktop and handheld devices: a waterfall (each stage starts where the previous one ends, on a shared scale, a dotted line at one frame's duration), values on the right, help on hover or long press.
- TVs and living room (Apple TV, LG, Linux living-room mode): the six values in one row readable from a distance, grouped by place (“On the computer”, “Network”, “On this TV”), with each place's total; no help.
- The “Picture” row says “HDR” when the picture is (“3840 × 2160 · HEVC · HDR · 120 fps”): turning HDR on and off from the bar shows which stage changes.

**Last session**, modelled on the Mac app's card (`FrameRuler`):

- large, the median: “8.9 ms” and “median”;
- a ruler in milliseconds, from 0 to 20 ms (more if the measurement goes beyond), with the marks “160 Hz”, “120 Hz” and “60 Hz” at the duration of one frame at those rates (6.25; 8.33; 16.67 ms), and a band from the median to the 95th percentile;
- “95% of frames under 14.2 ms”;
- the duration and when: “42 min · yesterday”;
- what was sent: “2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s”; with several displays, their number first, then the main display and the total bitrate: “3 displays · 3840 × 2160 · 160 Hz · HEVC · 120 Mbit/s”; “HDR” after the format when at least one display was (“3840 × 2160 · 120 Hz · HEVC · HDR · 80 Mbit/s”); that number is the displays the computer took (those it could not create are not in it), the same in the session bar, the session statistics and the last session;
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

## Station updates

The apps offer the Station's update where people see it, without waiting for them to open the Settings: a banner on the home screen and, on the first launch after the app updates, a notice that gathers every Station to update. The “Station” section of the settings stays the same. Every Link app except the web client (`/link`, which pairs no computer).

**Who sees it**: the same as “Update the Station” in the settings, i.e. a computer paired on this device, or a computer from a KyberNow account with the “Administer” right (`admin`). Never a share guest, a loan, or “Watch” alone. The decision is the core's (`station_offer`), the same everywhere.

**When**: what the Station announces (`GET /api/stream/info`, `station.update`), read as today. The Station checks on its own every 6 hours; the app asks it to check right away (`POST /api/stream/station/check`) when the app's version changed since it last asked this computer, or when the Station is older than the app, at most once a day per computer, never during a session. A Station that doesn't know this route (before 0.11.3) isn't asked: its announcement is trusted.

| Element | Français | English |
|---|---|---|
| Home banner, chosen computer | under the “Connect” and profiles row, full width; up-arrow icon; « KyberNow Station {v} est disponible pour cet ordinateur. Environ une minute. » ; « Mettre à jour » (primary), « Plus tard » | “KyberNow Station {v} is available for this computer. About a minute.”; “Update”, “Later” |
| Banner, while updating | « Mise à jour de la Station… », spinner, no button; then the brief notice « KyberNow Station {v} est en place. » and the banner goes away | “Updating the Station…”; then “KyberNow Station {v} is installed.” |
| Banner, a device is in a session | the settings' confirmation (« {appareil} est connecté », « La mise à jour coupera sa session. », « Mettre à jour quand même », « Annuler ») | “{device} is connected”, “The update will end its session.”, “Update anyway”, “Cancel” |
| Banner, failure | the settings' text in the banner, « Réessayer » and « Plus tard » | the settings' text, “Try again”, “Later” |
| Banner, version that needs the installer | « KyberNow Station {v} est disponible. Elle s'installe sur l'ordinateur, avec l'installateur. » ; only « Plus tard » | “KyberNow Station {v} is available. It installs on the computer, with the installer.”; only “Later” |
| Banner, away from home | « KyberNow Station {v} est disponible. Hors de chez vous, l'app ne peut pas mettre à jour la Station : faites-le une fois rentré. » ; only « Plus tard » | “KyberNow Station {v} is available. Away from home, the app can't update the Station: do it once you're back.”; only “Later” |
| Other computers (row or list) | the state followed by « · Mise à jour » | “Online · Update” |
| “Later” | hides this computer's banner for this version; it comes back after 7 days, or right away if a newer version comes out; the “· Update” chip stays | |
| Notice after an app update, one Station | a dialog (sheet on Mac, iPhone and iPad; full screen on TVs); « {nom} a aussi une mise à jour » ; « KyberNow Link est passée en {app}. Mettez aussi la Station à jour : environ une minute, une session en cours sera coupée. » ; « Mettre à jour la Station », « Plus tard » | “{name} has an update too”; “KyberNow Link is now {app}. Update the Station too: about a minute, a session in progress will be cut off.”; “Update the Station”, “Later” |
| Notice, several Stations | « Vos Stations aussi ont une mise à jour » ; « KyberNow Link est passée en {app}. Mettez aussi vos ordinateurs à jour : environ une minute chacun, une session en cours sera coupée. » ; one row per computer: name, « Version {v} », « Mettre à jour » ; at the bottom « Tout mettre à jour », « Plus tard » | “Your Stations have an update too”; “KyberNow Link is now {app}. Update your computers too: about a minute each, a session in progress will be cut off.”; “Version {v}”, “Update”; “Update all”, “Later” |
| Notice, a row during and after | « Mise à jour… » ; « À jour » ; a refusal: its short text under the name (with « Mettre à jour quand même ») ; once all are up to date, « Plus tard » becomes « Fermer » | “Updating…”; “Up to date”; “Close” |

Rules:

- One banner at a time under the “Connect” row: the Station update comes before the KyberNow account prompts (home screen mockups), which wait until it's done or put off.
- The banner shows only for the chosen computer, online or away from home; never during a session (nor in the session bar). Offline: nothing.
- The notice after an app update: once per app version, not on the very first launch (the app keeps the launched version; with no kept version but at least one known computer, it comes from 0.11.2 or earlier: that's an update, `station_offer::BEFORE_NOTICE`); the app first asks the computers online on the network to check (10 seconds at most), then shows the notice only if at least one Station it can install is left to update. Computers away from home or needing the installer aren't in it (their banner is enough). If a session opens by itself at launch, the notice waits for the return to the home screen. “Later” in the notice counts as “Later” for every Station in the list.
- “Update all” updates the Stations one after another; a refusal (session in progress) leaves its row with “Update anyway” and moves on.
- Updating then follows the Station as the settings do (`GET /api/stream/info` until it's back in the new version).
- “Later” is kept on the device, per computer: the version and the date.
- TVs and living room: the banner takes focus after “Connect” (down arrow); the notice puts focus on the primary button.

Admin contract: `POST /api/stream/station/check` (same account as `GET /api/stream/info`) asks the update feed for the latest version right away, at most once every 10 minutes (otherwise the kept result), and answers `{"ok": true, "station": …}` (the same object as `info`'s `station`). Core: `Console::station_check`, and the `station_offer` module (banner, notice, when to ask for a check).

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
| Live measurements | « Statistiques » (the session bar's statistics panel; Mac: at the bottom right of the picture; Windows: the session bar shows with it and stays until the pointer passes over it; a second click hides them) | “Statistics” |
| Recording (a Station that records) | « Enregistrer la session »; while recording: « Arrêter l'enregistrement (02:14) » | “Record the session”; “Stop recording (02:14)” |
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
| Quit | « Quitter KyberNow Link »; the sessions in progress end first, a few seconds at most (5 s on Windows): then the app closes anyway | “Quit KyberNow Link” |

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

## PlayStation consoles

Contract: [playstation.en.md](playstation.en.md). Consoles appear on the home screen with the computers, in the same row, with their own texts; the session keeps the bar, statistics and shortcuts of a computer session.

For the chosen console, in this order:

| Item | French | English |
|---|---|---|
| État | « Allumée », « En veille », « Hors ligne », « Recherche… » | “On”, “In rest mode”, “Offline”, “Searching…” |
| Nom | le nom de la console | |
| Modèle | « PS5 », « PS4 » | |
| Jeu en cours | le nom du jeu, quand la console en annonce un | |
| Bouton principal, allumée | « Jouer » | “Play” |
| Bouton principal, en veille | « Réveiller et jouer » ; pendant le réveil : « Réveil de la console… » | “Wake and play”; while waking: “Waking the console…” |
| Bouton principal, pas encore associée | « Associer » | “Pair” |
| HDR | la puce « HDR » de la section HDR, mêmes règles | |

In the row: name and state. A console found on the network and not yet paired: “On your network”.

**Accounts and adding a console.** Several PlayStation Network accounts on one device, each with its consoles; one console can be paired with several accounts (one profile per person on the console). Everything is managed in the add window: in the apps that have consoles, the home screen button, the menu and the window title become « Ajouter un ordinateur ou une console » / “Add a computer or a console”, and a “PlayStation” tab comes after “With a friend account”.

| Item | French | English |
|---|---|---|
| Tab | « PlayStation » | “PlayStation” |
| Accounts, title | « Comptes PlayStation Network » | “PlayStation Network accounts” |
| One row per account | le pseudo PSN ; « {n} console(s) » ou « Aucune console » ; « Renommer », « Retirer » | the PSN online ID (read at the Sony sign-in; typed by hand: the given name); below “{n} console(s)” or “No console”; on the right “Rename” and “Remove” |
| No account | « Aucun compte : ajoutez-en un pour associer une console. » | “No account: add one to pair a console.” |
| Add | « Ajouter un compte » | “Add an account”: the Account step below |
| Already there | « Ce compte est déjà là. » | same ID as a kept account: its online ID is updated, no duplicate; notice “This account is already here.” |
| Remove | « Retirer le compte {nom} et ses {n} consoles ? », « Ses associations sont effacées de cet appareil. Une console associée aussi avec un autre compte reste, avec cet autre compte. », « Retirer », « Annuler » | confirmation “Remove the account {name} and its {n} consoles?” (one console: “… and its console?”; none: “Remove the account {name}?”), text “Its pairings are erased from this device. A console also paired with another account stays, with that other account.”, buttons “Remove” and “Cancel” |
| Consoles, title | « Consoles sur ce réseau » ; « Aucune console trouvée. Allumez-la ou mettez-la en veille, ou saisissez son adresse dans « Par adresse ». » | “Consoles on this network”; empty: “No console found. Turn it on or put it in rest mode, or enter its address in “By address”.” |
| One row per found console | « Associer » ; « Associée : {pseudos} », « Associer avec un autre compte » | name, model, state; not paired: “Pair”; paired: “Paired: {online IDs}” and “Pair with another account” (only if an account not yet paired with it remains) |

Found consoles also appear in “On this network”, with the computers (row “{name} · PS5”, same “Pair” button), and on the home screen (“On your network”).

**By address**: the same field as for a computer, help « L'adresse d'un ordinateur, ou d'une console PlayStation (par un VPN ou depuis un autre réseau). » / “The address of a computer, or of a PlayStation console (through a VPN or from another network).” The app looks for both a Station and a console at that address; a console that answers (on or in rest mode) opens its pairing. Its address is kept: the console is then searched directly at that address, as well as on the local network. The console is looked for first (1.5 s; a Station and a console don't answer at the same address), then the Station as before. Nothing answers: the tab's usual text; an address alone, without an Admin port, that doesn't answer: “Console unreachable.”

**Pairing** (from the tab, “On this network”, “By address” or the home screen card):

| Step | French | English |
|---|---|---|
| Account choice (two or more accounts not yet paired with this console) | « Avec quel compte ? », « Ajouter un compte » | “Which account?”, one row per account, and “Add an account”; a single possible account: step skipped; no account: the Account step |
| Account | « Connectez-vous au compte PlayStation Network à associer. KyberNow lit seulement l'identifiant et le pseudo du compte, une seule fois. » Bouton : « Se connecter à PlayStation Network » ; lien : « Saisir l'identifiant à la main » | “Sign in to the PlayStation Network account to pair. KyberNow only reads the account's ID and online ID, once.” Button: “Sign in to PlayStation Network”; link: “Enter the ID by hand” |
| By hand | Champs : « Identifiant du compte », « Nom » (« Pour le reconnaître dans la liste. ») ; « Continuer » | Fields: “Account ID” and “Name” (“To recognise it in the list.”); button: “Continue” |
| ID not read | « Identifiant du compte non lu : réessayez, ou saisissez-le à la main. » | “Couldn't read the account ID: try again, or enter it by hand.” |
| Code | « Compte : {pseudo} » ; « Sur la console : Réglages › Système › Lecture à distance › Associer un appareil. Saisissez le code affiché. » Champ : « Code à 8 chiffres » ; bouton : « Associer » | above it, “Account: {online ID}”; “On the console: Settings › System › Remote Play › Link Device. Enter the code shown.” Field: “8-digit code”; button: “Pair” |
| No answer | « La console ne répond pas : gardez l'écran « Associer un appareil » ouvert. » | “The console isn't answering: keep the “Link Device” screen open.” |
| Code refused | « Code refusé : il change toutes les 5 minutes. » | “Code refused: it changes every 5 minutes.” |
| Account refused | « La console refuse ce compte. » | “The console refuses this account.” |
| Unreachable | « Console injoignable. » | “Console unreachable.” |

An account read during a pairing is kept, even if the pairing then fails.

**Several devices** ([playstation.en.md](playstation.en.md#several-devices)):

| Item | French | English |
|---|---|---|
| PlayStation tab, below the accounts (device with accounts) | « Envoyer à un appareil » ; « Aucun appareil en attente. Sur la télé ou l'autre appareil : Ajouter une console › Recevoir d'un autre appareil. » | “Send to a device”: the list of network devices waiting for a sending (“{name} · Apple TV”, “{name} · LG”…); empty: “No device waiting. On the TV or the other device: Add a console › Receive from another device.” |
| Sending, code | « Saisissez le code affiché sur {nom}. », « Code à 6 chiffres », « Envoyer », « Comptes et consoles envoyés à {nom}. », « Code refusé : vérifiez-le sur {nom}. », « {nom} ne répond plus. » | “Enter the code shown on {name}.” Field: “6-digit code”; button: “Send”; done: “Accounts and consoles sent to {name}.”; wrong code: “Code refused: check it on {name}.”; unreachable: “{name} isn't answering any more.” |
| PlayStation tab (and a TV's first screen without an account) | « Recevoir d'un autre appareil », « Sur votre Mac, votre PC ou votre téléphone : Ajouter un ordinateur ou une console › PlayStation › Envoyer à un appareil, puis saisissez ce code. », « {n} compte(s) et {n} console(s) reçus. », « Code expiré. », « Nouveau code » | “Receive from another device”: a large code (“482 913”), below “On your Mac, PC or phone: Add a computer or a console › PlayStation › Send to a device, then enter this code.”; received: “{n} account(s) and {n} console(s) received.”; after 5 min: “Code expired.” and “New code” |
| Apple, iCloud on | « Ordinateurs et consoles » | nothing to do: accounts and consoles arrive by themselves, like computers; the iCloud card of the app's Settings says “Computers and consoles” |
| TV (no browser) | « Recevoir d'un autre appareil », « Saisir l'identifiant à la main » | the Account step offers “Receive from another device” and “Enter the ID by hand” (no Sony sign-in) |

**On the home screen**, one card per console, whatever the number of accounts. Paired with several accounts: a “{online ID}” chip next to the main button opens the account choice (the profile the console opens the game with); kept per console; paired with only one: no chip. The settings stay the console's, for all its accounts. The console no longer recognises an account: that pairing is erased, and “Pair” comes back if it was the last one.

**Linux** (desktop, Steam Deck, living room, Raspberry Pi and KyberNow Link OS; [apps/linux/README.en.md](../apps/linux/README.en.md#playstation-consoles)): the texts above, and what is specific to the Linux app (no web view; HDR only on a Wayland desktop with colour management, not on the Steam Deck, in the living room, or on the Raspberry Pi and KyberNow Link OS, and without an “HDR calibration” screen; the connection drawn in steps like a computer's):

| Element | Français | English |
|---|---|---|
| Adding, button (under the two other ways) | « PlayStation » | “PlayStation” |
| Account, desktop: Sony's page is open in the browser | « Se connecter à PlayStation Network » ; « La page de connexion de Sony s'est ouverte dans votre navigateur. Une fois connecté, Sony affiche une page presque vide : copiez son adresse (elle commence par https://remoteplay.dl.playstation.net) et collez-la ici. » ; « Adresse de la page de Sony » ; « Continuer » ; « Rouvrir la page de Sony », « Saisir l'identifiant à la main » | title “Sign in to PlayStation Network”; “Sony's sign-in page opened in your browser. Once signed in, Sony shows an almost empty page: copy its address (it starts with https://remoteplay.dl.playstation.net) and paste it here.”; field “Sony page address”; “Continue”; links “Open Sony's page again”, “Enter the ID by hand” |
| Account, no browser (Deck, living room, OS) | « Recevez le compte d'un Mac, d'un PC ou d'un téléphone où il est déjà, ou saisissez son identifiant. » | “Receive the account from a Mac, PC or phone that already has it, or enter its ID.” |
| Tab, under “Receive from another device” | « Les comptes et les consoles déjà associées d'un Mac, d'un PC ou d'un téléphone arrivent ici, sans refaire le code de la console. » | “Accounts and consoles already paired on a Mac, PC or phone come here, without the console's code again.” |
| By address, a bare address | « Recherche… », « Console injoignable. » | “Searching…”, then “Console unreachable.” if nothing answers |
| Home, offline | « Réessayer » | “Try again” (one more search, as on Mac) |
| Connection (a computer's capsule) | « Réveil de la console », « Connexion à la console », « Premières images » ; « Déconnexion… » | steps “Waking the console” (in rest mode), “Connecting to the console”, “First frames”; “Disconnecting…” at the end |
| Settings, section subtitles | « Ce que la console envoie. Les réglages valent pour la partie suivante. » ; « Le clavier, la souris et la DualSense changent tout de suite pendant une partie. » | “Picture”: “What the console sends. Settings apply to the next game.”; “Controls”: “The keyboard, mouse and DualSense change at once during a game.” |
| Game menu (controller) | « Reprendre », « Son », « Statistiques », « Manettes », « Réglages », « Veille » (« Mettre la console en veille »), « Par le relais », « Se déconnecter » (« Fin de la partie ») ; « Pendant ce menu, la console reçoit une manette au repos » ; « ouvre ce menu » | tiles “Resume”, “Sound”, “Statistics”, “Controllers”, “Settings” (“Settings for {name}”), “Rest mode” (“Put the console in rest mode”), “Through the relay”, “Disconnect” (“Ends the game”); “While this menu is open, the console gets an idle controller”; legend “Options Create L1 R1 opens this menu” |

Accounts kept from an earlier version (a single account, without an online ID): « Compte PlayStation » / “PlayStation account”, to rename, or updated by signing in again.

**Console settings** (gear next to the main button, accessible label “Settings for {name}”, laid out like a computer's settings):

| Item | French | English |
|---|---|---|
| À l'accueil, à côté de la puce HDR | puce « Auto » ou « Personnalisé » (définition, images par seconde, débit ou format changés) ; un clic ouvre les réglages | “Auto” or “Custom” |
| Section | « Image » | “Picture” |
| Définition | « Définition » : « Auto », « 1080p », « 720p », « 540p », « 360p » ; aide d'Auto : « La plus haute que la console accepte : 1080p. » | “Resolution”: “Auto”…; “The highest the console accepts: 1080p.” |
| Images par seconde | « Images par seconde » : « 60 i/s », « 30 i/s » | “Frame rate”: “60 fps”, “30 fps” |
| Débit | « Débit » : « Auto » (aide : « Suit la définition. ») ou « {n} Mbit/s », de 2 à 100 | “Bitrate”: “Auto” (“Follows the resolution.”) or “{n} Mbit/s” |
| Format | « Format » : « Auto », « H.264 », « H.265 » ; aide d'Auto : « H.265 si cet appareil le lit. » ; aide de H.264 : « Sans HDR. » ; PS4 : réglage inactif, aide « La PS4 n'envoie que du H.264. » | “Format”: “Auto”, “H.264”, “H.265”; “H.265 if this device can play it.”; “No HDR.”; “The PS4 only sends H.264.” |
| Plein écran | « Ouvrir la partie en plein écran » (coché la première fois ; retenu aussi quand on quitte ou rejoint le plein écran pendant la partie) | “Open the game in full screen” |
| Upscaling | « Agrandir l'image à la taille de l'écran », on by default, with the help « La console envoie du 1080p au plus : l'image est agrandie aux pixels de votre écran par la puce graphique du Mac. Coupé de lui-même si ce Mac est trop lent. »; « Netteté », 0 to 100 % in steps of 5 (40 % by default), greyed out when upscaling is off; both apply at once to a game in progress | “Upscale the picture to the screen size”, “The console sends 1080p at most: the Mac's graphics chip upscales the picture to your screen's pixels. Turns itself off if this Mac is too slow.”; “Sharpness” |
| Section | « Commandes » | “Controls” |
| Aide de la section | « Le clavier et la souris tiennent lieu de manette. Une manette branchée les remplace. » | “The keyboard and mouse stand in for a controller. A connected controller replaces them.” |
| Liste | une ligne par action de la manette, dans l'ordre du cœur (`Action::ALL`) : « Stick gauche vers le haut / le bas / la gauche / la droite », « Croix », « Rond », « Carré », « Triangle », « L1 », « R1 », « L2 », « R2 », « L3 », « R3 », « Croix directionnelle vers le haut / le bas / la gauche / la droite », « Options », « Create », « Bouton PS », « Pavé tactile », « Stick droit vers le haut / le bas / la gauche / la droite » ; à droite, ses touches (la lettre du clavier du Mac, ou « Clic gauche », « Clic droit », « Clic du milieu », « Bouton 4 », « Bouton 5 ») et « + » pour en ajouter une ; « Aucune » s'il n'y en a pas | “Left stick up / down / left / right”, “Cross”, “Circle”, “Square”, “Triangle”, …, “D-pad up / down / left / right”, “Options”, “Create”, “PS button”, “Touchpad”, “Right stick up / down / left / right”; “Left click”, “Right click”, “Middle click”, “Button 4”, “Button 5”; “None” |
| Changer une touche | un clic sur une touche : « Appuyez sur une touche ou cliquez… » (Échap annule, Effacement retire la touche) ; une touche déjà prise ailleurs y est retirée | “Press a key or click…” |
| Souris | « La souris tient le stick droit » (case) ; « Sensibilité » (de 0,2 à 5, 1 par défaut) ; « Réactivité au départ » (courbe : « Douce », « Normale », « Vive » : 1,2 / 1 / 0,8 ; Vive par défaut) ; « Inclinaison minimale » (0 à 50 %, 20 % par défaut, aide : « Pour passer la zone morte des jeux. ») ; « Inverser l'axe vertical » | “The mouse holds the right stick”; “Sensitivity”; “Starting response”: “Soft”, “Normal”, “Brisk”; “Minimum tilt” (“To get past games' dead zone.”); “Invert vertical axis” |
| Retour | « Disposition par défaut » | “Default layout” |
| Controller | « Vibrations » et « Gâchettes adaptatives » ; « Pour la DualSense. Les vibrations HD passent par la manette branchée en USB. » | “Rumble” and “Adaptive triggers”, from 0% (off) to 100% in steps of 10 (100% by default); help: “For the DualSense. HD haptics go through the controller plugged in over USB.”; at once during a game |

Settings apply to the next game; keyboard and mouse changes apply at once during a game.

**The console's keyboard** (the console opens its on-screen keyboard): banner at the bottom of the picture, like the mouse hints: « Tapez votre texte · Entrée pour valider · Échap pour annuler » / “Type your text · Return to accept · Escape to cancel”; the keyboard types into the console's field (text in the keyboard's layout, Backspace; arrows not sent), then the banner goes away when the console closes its keyboard. ⌃⌥⌘ shortcuts stay with the app.

**Complementary texts** (written first for the Windows app; the other apps take them when they have the same element):

| Element | Français | English |
|---|---|---|
| Subtitle of the PlayStation tab (add window whose ways to add are listed with a subtitle) | « Comptes PSN et consoles » | “PSN accounts and consoles” |
| Sony sign-in in its own window (Windows: Sony's page is not inside the sheet) | « Connectez-vous dans la fenêtre de PlayStation Network… » | “Sign in in the PlayStation Network window…” |
| Removing a console (bottom of its settings) | « Retirer cette console », then « Retirer {nom} de cet appareil ? Vous pourrez l'associer de nouveau. » | “Remove this console”, “Remove {name} from this device? You can pair it again later.” |
| Accessible labels | account chip: « Compte de la partie : {pseudo} »; an action's “+”: « Ajouter une touche : {action} »; “On your network” card: « Associer {nom} » | “Account for the game: {online ID}”; “Add a key: {action}”; “Pair {name}” |
| Statistics panel | through the relay, a « Chemin : {chemin} » line; before the picture: « Les statistiques s'affichent dès que l'image arrive. » | “Path: {path}”; “Statistics appear as soon as the picture arrives.” |
| Bar driven by the controller | banner under the bar: « La manette pilote KyberNow Link · La console ne la reçoit plus », « naviguer », « ✕ choisir », « ○ fermer », « L1 R1 OPTIONS CREATE revenir à la console » | “The controller drives KyberNow Link · The console no longer receives it”, “navigate”, “select”, “close”, “back to the console” |
| Shortcuts (Windows, Linux) | Ctrl+Alt+Shift+M for the mouse, Ctrl+Alt+Shift+D to disconnect, Ctrl+Alt+Shift+F for full screen; the layout's key names are the system's | |

**Away from home**, a console reached through a Station: “Away from home” state on the home screen, “Via the relay” chip in the bar, “Path” line in the statistics, like a computer ([relais.en.md](relais.en.md#playstation-consoles)). Nothing to set. The main button is “Play” (the exact state is only read by reaching the Station): “Connecting to the console…” while it is reached, then “Waking the console…” if it was in rest mode, then the game. Relay refusals (access turned off, Station off or asleep): the texts of a computer through the relay, with the Station's name; the Station doesn't see the console, or it doesn't wake: “The console isn't answering.”

**Game bar**: the same as a computer session's (same component, same layout), with what makes sense for a console. It opens like the pointer bar (pointer a quarter second against the top edge of the picture, only when the mouse is not captured; same short line at the top of the picture) or with the controller combination Options + Create + L1 + R1 held 250 ms (the same buttons as Start + Select + L1 + R1 in a computer session). Opened by the controller, the controller drives it (D-pad or stick, Cross to select, Circle to go back; Circle or the combination close it) and the console gets an idle controller; keyboard and mouse don't reach the console either while it is open.

| Item | French | English |
|---|---|---|
| Nom | le nom de la console | |
| Statistiques | « Statistiques » : panneau des mesures, une fois par seconde : « 1920 × 1080 · 60 i/s · H.265 · HDR · 15 Mbit/s demandés », « Reçu : 59 i/s · 12,4 Mbit/s », « Images perdues : {n} · réparées : {n} » (depuis le début de la partie) | “Statistics”: “… · 15 Mbit/s requested”, “Received: 59 fps · 12.4 Mbit/s”, “Pictures lost: {n} · repaired: {n}” |
| Son | « Couper le son » / « Rétablir le son » | “Mute” / “Unmute” |
| Plein écran | « Plein écran » / « Quitter le plein écran » (retenu pour la console) | “Full screen” / “Exit full screen” |
| Souris | « Prendre la souris » / « Rendre la souris » (⌃⌥⌘M), absent avec une manette branchée | “Capture the mouse” / “Release the mouse” |
| Réglages | « Réglages de {nom} » | “Settings for {name}” |
| Veille | « Mettre la console en veille » (dans la barre, plus dans la barre de titre) | “Put the console in rest mode” |
| Fin | « Se déconnecter » (⌃⌥⌘D) | “Disconnect” |

No profile, displays, clipboard, sharing or “Computers”.

**Session**:

| Moment | French | English |
|---|---|---|
| Connecting | « Connexion », « Premières images », « Annuler » ; « La connexion a échoué », « Fermer » | a computer's connection screen, the same component (section “Connection”): the passage (the home screen opening in two along the seam), the capsule with the steps “Connecting” then “First frames” and “Cancel”; the picture appears 0.9 s after the stream starts; failure before the picture: “The connection failed” and the reason, “Close”; end during a game: the reason alone, “Close”. Play resumed after a drop: the steps over the last picture. « Connexion à la console… » / “Connecting to the console…” stays the main button's text while connecting through the relay |
| Contenu protégé | « La console ne peut pas diffuser ce qu'elle affiche. » (sur l'image, tant que dure le blocage) | “The console can't stream what it's showing.” |
| Menu de la session | « Mettre la console en veille » (barre de la partie) | “Put the console in rest mode” (game bar) |
| Clavier et souris | Tant que la souris n'est pas capturée et qu'aucune manette n'est branchée : « Cliquez dans l'image pour jouer au clavier et à la souris » ; un instant après la capture : « ⌃⌥⌘M pour rendre la souris » (le raccourci de la souris des jeux ; Windows et Linux : Ctrl+Alt+Maj+M) | “Click the picture to play with keyboard and mouse”; “⌃⌥⌘M to release the mouse” |
| Fin, déjà jouée ailleurs | « La console est déjà jouée à distance par un autre appareil. » | “The console is already being played remotely by another device.” |
| Fin, plus associée | « La console ne reconnaît plus cet appareil : associez-la de nouveau. » | “The console no longer recognizes this device: pair it again.” |
| Fin, injoignable | « La console ne répond pas. » | “The console isn't answering.” |
| Fin, par la console | « La console a terminé la partie. » | “The console ended the session.” |

**iPhone and iPad** (same contract; what the device can't do is absent):

| Element | Français | English |
|---|---|---|
| Game | always full screen (no “Open the game in full screen” setting, no “Full screen” in the bar); landscape on iPhone | |
| Bar | opened by the tab, by swiping down from the top edge or with Options + Create + L1 + R1; in order: « Statistiques », « Couper le son » / « Rétablir le son », « Manette à l'écran », « Réglages » (accessibility label « Réglages de {nom} »), « Mettre la console en veille », then, fixed: « Par le relais » when relevant, hide the bar, « Se déconnecter »; no “Capture the mouse” | “Statistics”, “Mute” / “Unmute”, “On-screen controller”, “Settings”, “Put the console in rest mode”, “Disconnect” |
| Statistics | the game bar's three lines, as a sheet on iPhone, a panel on iPad; « Chemin : … » through the relay | |
| On-screen controller | the computer sessions' one (same layouts, same gestures), with PlayStation signs: ✕ ○ □ △, L1, R1, L2, R2; remembered per console; shown, it counts as a connected controller | |
| Keyboard and mouse (iPad) | turned into a controller as on Mac, no capture to ask for (the iPadOS pointer is hidden over the picture); ⌃⌥⌘D disconnects, ⌃⌥⌘S opens the statistics; the layout is set on iPad only | |
| Console keyboard | a field at the bottom of the picture, above the keyboard: with a hardware keyboard, the banner « Tapez votre texte · Entrée pour valider · Échap pour annuler »; without, « Tapez votre texte » and the on-screen keyboard (its “OK” key accepts); « Annuler » rejects | “Type your text” |
| Upscaling, help | « La console envoie du 1080p au plus : l'image est agrandie aux pixels de votre écran par la puce graphique de l'appareil. Coupé de lui-même si cet appareil est trop lent. » | “The console sends 1080p at most: the device's graphics chip upscales the picture to your screen's pixels. Turns itself off if this device is too slow.” |
| Controller, help | « Pour la DualSense. » (no HD haptics: iOS doesn't expose the controller's sound card; simple rumble) | “For the DualSense.” |
| iCloud (App settings) | « Ordinateurs et consoles : vos ordinateurs et leurs accès, vos comptes PlayStation et leurs consoles suivent votre Mac et vos autres appareils, sans rien taper. Mots de passe et associations dans le Trousseau iCloud, chiffré de bout en bout. » | “Computers and consoles: your computers and their access, your PlayStation accounts and their consoles follow your Mac and your other devices, without typing anything. Passwords and pairings in iCloud Keychain, end-to-end encrypted.” |

**Apple TV** (and any TV with no keyboard or mouse): the game bar is a computer session's tile menu, opened with Back or Options + Create + L1 + R1; hint when the picture arrives “Back or Options + Create + L1 + R1: menu”; at the bottom of the menu “Back or Options + Create + L1 + R1: close the menu”. Tiles: “Resume”, “Controllers”, “Mute” / “Unmute”, “Settings for {name}”, “Put the console in rest mode”, “Disconnect” (below: “The console stays on”). The statistics at the top of the menu, with an upscaling line (“Upscaled to 3840 × 2160 · 0.9 ms”, or “Upscaling off: Apple TV too slow”). No full screen, no keyboard and mouse. Console settings in three sections: “Picture” (“Picture · the console sends 1080p at most”), “Controls” (summary “Rumble 100 % · triggers 100 %”; help: “For the DualSense. On Apple TV, HD haptics become simple rumble.”) and “Console” (pairings, “Pair with another account”, “Remove this console” with “Remove {name} from this device?”). Upscaling help: “The console sends 1080p at most: the Apple TV's graphics chip upscales the picture to your TV's pixels, in SDR. Turns itself off if this Apple TV is too slow.” (tvOS 27.1 and later). On the home screen, in place of the profiles: the “Auto” or “Custom” chip; the add card: “Add a computer or a console”, “iCloud, network, code, address or PlayStation”; the tab: “PlayStation”, “Accounts, consoles, sending between devices”.

## Device camera

The device's camera goes to the computer, which sees it as an ordinary webcam, “KyberNow Camera” (same name in every language, like a device name): Teams, Zoom, Discord, OBS and browsers offer it in their list. Technical detail in [webcam.en.md](webcam.en.md).

**Where**: the Mac, iPhone and iPad, Android (phones and tablets; not Android TV), Windows, Linux (desktop and Steam Deck) apps, and Apple TV (an iPhone or iPad nearby, through Continuity Camera, tvOS 17 and later). Not on LG TVs, the Raspberry Pi or the web client. **Who**: the host in a session and the friend the computer is lent to; never the guests of a share. **When**: the « Caméra » entry only exists when the computer offers the camera for the current picture (`uplink` in Kyber's answer when the picture starts, see [webcam.en.md](webcam.en.md)) and the device has one (at least one camera, or on Apple TV, always: it offers to pick one). Otherwise it is absent, without a message.

**On demand**: the camera is off at the start of every session, even if it was on in the previous one. It is turned on from the bar (« Caméra » chip), the icon's menu, the tile menu (iPhone, iPad, Android, Apple TV: « Caméra » / “Camera” tile), or the C shortcut (⌃⌥⌘C on the Mac, Ctrl+Alt+Shift+C on Windows and Linux). It turns itself off when the session ends, when another session comes to the front (several computers: the camera stays with the session it was turned on in, which keeps it in the background) and when the right is taken back (computer given back).

| Where | Français | English |
|---|---|---|
| Bar, icon menu, tile, off | « Allumer la caméra » | “Turn on camera” |
| On | « Éteindre la caméra » | “Turn off camera” |
| List (menu and tile, under the action; one line per camera, the chosen one ticked) | on iPhone, iPad and Android: « Caméra avant », « Caméra arrière »; otherwise the system's name (« Caméra FaceTime HD », « iPhone de Marie », « Logitech C920 ») | “Front camera”, “Back camera”; otherwise the system's name |
| Apple TV, no iPhone chosen | « Choisir un iPhone ou un iPad… » (opens tvOS's picker) | “Choose an iPhone or iPad…” |
| Turning on | « Caméra… » in the chip | “Camera…” |
| Indicator, while sending | red dot and « Caméra » lit in the bar; with the bar closed and in apps with a tile menu, a small « ● Caméra » capsule at the top left of the picture, nothing else | “● Camera” |
| Paused (app in the background on iPhone, iPad, Android) | nothing on the device; the computer keeps the last picture; sending resumes on return, without a gesture | |
| Statistics, « Caméra » line | « 1280 × 720 · 30 i/s · H.264 · 1,4 Mbit/s »; « En pause » | “1280 × 720 · 30 fps · H.264 · 1.4 Mbit/s”; “Paused” |

**Choosing the camera**: by default the front camera (phone, tablet) or the built-in one (laptop); otherwise the first in the system's list. The one chosen in the list is kept for the device (not per computer); switching in a session does not turn it off: the computer keeps the last picture during the switch. A camera unplugged while sending: the next in the list takes over, brief notice « {caméra} n'est plus branchée. » / “{camera} is no longer connected.”; none left: the camera turns off, same notice.

**Resolution**: the computer setting « Définition de la caméra » (Controls section): 720p by default, or 1080p, always 30 frames per second. A camera that can't do the requested resolution sends the nearest one below; the computer always shows it at its own resolution (720p or 1080p), scaled or letterboxed. Changing it in a session applies at once (the camera restarts with a keyframe). Bitrate: 1.5 Mbit/s in 720p, 4 Mbit/s in 1080p, half via the relay; lowered with the picture when the network saturates, never to the point of delaying the mouse (sending drops frames rather than waiting).

**Device permission**: the first time, the system asks for camera access (Mac, iPhone, iPad, Android, Apple TV; explanation texts below). Denied: notice « L'accès à la caméra est refusé. Autorisez-le dans les Réglages de l'appareil. » / “Camera access is denied. Allow it in the device's Settings.” with « Ouvrir les Réglages » / “Open Settings” where the system allows it (Mac: System Settings › Privacy › Camera; Windows: Settings › Privacy › Camera). Explanation given to the system (Info.plist, Android): « KyberNow Link envoie votre caméra à votre ordinateur quand vous l'allumez en session. » / “KyberNow Link sends your camera to your computer when you turn it on in a session.”

**Refusals and failures** (brief notice, the camera stays off):

| Case | Français | English |
|---|---|---|
| Another device is already sending its own | « {appareil} envoie déjà sa caméra à {nom}. »; without its name: « Un autre appareil envoie déjà sa caméra à {nom}. » | “{device} is already sending its camera to {name}.”; without its name: “Another device is already sending its camera to {name}.” |
| Camera taken by another app on the device | « La caméra est utilisée par une autre app. » | “The camera is being used by another app.” |
| The computer could not open it (virtual camera missing or broken) | « {nom} n'a pas pu ouvrir sa caméra virtuelle. » | “{name} could not open its virtual camera.” |
| Device encoder unavailable | « Cet appareil ne peut pas envoyer sa caméra. » | “This device can't send its camera.” |

**On the computer**: with no device sending, “KyberNow Camera” shows a plain dark grey picture (software keeps it in its list); while paused, the last picture received. Mac Station, the first time: notification « La caméra de vos appareils sur ce Mac », « Autorisez l'extension caméra de KyberNow Station dans Réglages Système › Général › Ouverture et extensions. » / “Your devices' camera on this Mac”, “Allow KyberNow Station's camera extension in System Settings › General › Login Items & Extensions.” Linux Station without v4l2loopback: the camera goes through PipeWire (browsers, OBS); installing it (package v4l2loopback-dkms, then restart) gives it to every app ([webcam.en.md](webcam.en.md), “Linux Station”).

**The microphone** follows the same rules: sibling « Micro » entry, just before « Caméra » (bar, icon menu, « Micro » tile in tile menus), same indicator (« ● Micro »; both on: a single « ● Caméra · Micro » capsule), same refusals with “its microphone” instead of “its camera”. Its shortcut and other texts are to be set by the microphone work.

## Where the texts live

Each app keeps its catalogues ([i18n.en.md](i18n.en.md), “Files”); the texts above are copied into them verbatim. A platform-specific text follows the same rules.
