# Stylet (tablette graphique, Apple Pencil)

[English](stylet.en.md) · Français

La règle, pour toutes les apps, est dans [link-ui.md](link-ui.md), « Stylet ». Ici : le protocole, ce que fait chaque partie et où en sont les essais.

Choix de l'auteur (25/11/2026), à la demande d'un utilisateur sous Linux (Fedora, Cinnamon) dont la tablette marchait sans la pression :

- toutes les apps et les deux Stations ;
- pression, inclinaison, rotation, bout gomme, bouton du stylet et survol ;
- face à une Station d'avant, le stylet reste une souris en position absolue, sans avis ;
- pas de réglage pour le couper.

## Protocole (kynput)

Deux ajouts au protocole d'entrées de Kyber, qu'une app ou une Station d'avant ignore (type inconnu : le paquet est jeté avec une ligne au journal, la connexion continue).

**Paquet `Pen`** (type 11), de l'app vers l'ordinateur, état complet du stylet à chaque paquet (rien en différence : un paquet perdu est réparé par le suivant) :

| Champ | Taille | Sens |
|---|---|---|
| `display_id` | u32 | l'écran de l'ordinateur, comme `MousePosition` |
| `x`, `y` | i16, i16 | pixels de cet écran, comme `MousePosition` |
| `pressure` | u16 | 0 à 65535 ; 0 en survol |
| `tilt_x`, `tilt_y` | i8, i8 | degrés, −90 à 90 ; X vers la droite, Y vers soi (convention de Windows et des Pointer Events) |
| `rotation` | u16 | degrés, 0 à 359 |
| `flags` | u8 | `CONTACT` 1 (la pointe ou la gomme touche), `IN_RANGE` 2 (près de la surface ; absent : le stylet part), `ERASER` 4, `BARREL` 8 (bouton du stylet), `TILT` 16 et `ROTATION` 32 (donnés par l'appareil) |

**`ControlEvent::PenAvailable`** (valeur 3), de l'ordinateur vers l'app : une Station qui sait injecter un stylet l'envoie à chaque app qui se connecte, avec les pointeurs qu'elle envoie déjà à ce moment (`on_client_connected` du serveur d'entrées). C'est dans kynput même, pas dans `/api/stream/info` : l'invité d'un partage et l'ami qui emprunte, qui ne lisent pas Admin, ont le stylet aussi. Une app qui ne l'a pas reçu envoie le stylet en souris, comme avant.

Côté Station, le stylet est la souris : il passe avec le droit « clavier et souris » (`InputRights::keyboard_mouse`), pas sans. Le serveur d'entrées retient le stylet tant qu'il est près de la surface ; « tout relâcher » (`FlushKeys`) et la reprise du clavier et de la souris le lèvent et le laissent partir à l'endroit où il était.

## Correctifs de Kyber

- `apps/macos/kyber-patches/0005-kynput-stylet.patch` (côté client, appliqué par toutes les apps et par les Stations, qui reposent les correctifs des apps après les leurs) : le type `Pen` (`kynput/src/types/pen.rs`), `PenAvailable`, l'API C (`kynput_pen_to_packet`, `kynput_pen_from_packet`, structure `kynput_Pen`) et kynput-rs (`Pen`, `ControlEvent::PenAvailable`).
- `server/kyber-patches/0027-kynput-stylet.patch` (Station) : le stylet synthétique de Windows (`kynput/src/windows/host_pen.rs`), son branchement dans `host_keyboard_mouse.rs`, `kynput_host_pen_available()`, et le serveur d'entrées (`pipeline_state.rs` : droit, stylet levé, annonce).
- `server/macos/kyber-patches/0020-kynput-stylet-mac.patch` (Station Mac) : les événements tablette de macOS (`kynput/src/macos/host_keyboard_mouse.rs`) et `kynput_host_pen_available()` vrai sur Mac aussi.
- `server/linux/kyber-patches/0006-kynput-stylet-linux.patch` (Station Linux) : la tablette uinput (`kynput/src/linux/host_pen.rs`, traduction dans `pen_tablet.rs`) et `kynput_host_pen_available()` vrai sous Linux aussi.

Détail dans [kyber-changes.md](kyber-changes.md).

## Station Windows

Un stylet synthétique de Windows (`CreateSyntheticPointerDevice(PT_PEN)`, Windows 10 1809 et après), créé au premier paquet `Pen`, nourri par `InjectSyntheticPointerInput` : position en pixels du bureau virtuel (celle de l'écran plus celle du stylet), pression ramenée à 0-1024 (jamais 0 en contact : Windows Ink n'y dessinerait rien), inclinaison, rotation, `PEN_FLAG_BARREL`, gomme (`PEN_FLAG_INVERTED` près de la surface, `PEN_FLAG_ERASER` en contact). Windows lâche un stylet synthétique qui ne bouge plus : le dernier état est redonné toutes les 50 ms tant que le stylet est près de la surface. Un changement de bureau (UAC, écran de verrouillage) est suivi comme pour `SendInput`. Les logiciels qui ne connaissent pas les stylets reçoivent la souris que Windows fait du stylet (pointe : clic gauche ; bouton tenu et pointe posée : clic droit).

## Station Mac

Correctif `server/macos/kyber-patches/0020-kynput-stylet-mac.patch`, après 0002 (qui injecte la souris par `CGEvent`) et 0015 : le stylet de l'app devient une tablette pour macOS, comme celle d'un pilote Wacom. L'approche et le départ sont des événements de proximité (`kCGEventTabletProximity`, pointe ou gomme : `NSPointingDeviceType` 1 ou 3, avec un identifiant d'appareil et d'outil fixes) ; un changement de bout sans départ fait sortir l'un avant d'entrer l'autre. Le reste, ce sont des événements de souris de sous-type tablette (`kCGEventMouseSubtypeTabletPoint`) qui portent la pression (0 à 1, jamais 0 en contact), l'inclinaison (−1 à 1, 64° au plus, l'inverse de l'app Mac), la rotation (degrés) et les boutons du stylet : survol en `MouseMoved`, pointe posée en clic gauche glissé (double clic compris). Le bouton du stylet est un clic droit, comme le fait le pilote d'une tablette : appuyé en survol ou pointe posée, tenu jusqu'à ce que le bouton et la pointe soient relâchés ; la pointe posée d'abord reste un clic gauche. Un logiciel qui ne connaît pas les tablettes y voit une souris. `kynput_host_pen_available()` est vrai sur Mac aussi : la Station Mac annonce le stylet (`PenAvailable`). Rien à redonner à intervalles, contrairement à Windows. Il faut l'autorisation Accessibilité, comme pour la souris.

Journal de kynput : « Pen near (tip) » ou « (eraser) », « Pen Left button down » / « up », « Pen left » ; chaque point en `debug`.

## Station Linux

Correctif `server/linux/kyber-patches/0006-kynput-stylet-linux.patch`, après 0027 : le stylet de l'app devient une tablette graphique uinput, « KyberNow stylet », créée au premier paquet `Pen`, que libinput donne au compositeur comme une tablette branchée (Wayland : `tablet-v2` ; X11 : XInput2), sous GNOME comme sous KDE. Ses axes X et Y couvrent la boîte des écrans annoncés, comme le pointeur absolu (correctif Linux 0002) : le compositeur étend une tablette sans écran à elle sur tout le bureau. L'approche entre l'outil pointe (`BTN_TOOL_PEN`) ou gomme (`BTN_TOOL_RUBBER`), toujours sans pression d'abord (libinput prendrait une pression à l'approche pour l'usure de la pointe) ; un changement de bout sans départ fait sortir l'un avant d'entrer l'autre. En contact : `BTN_TOUCH` et la pression sur 0-4095, jamais sous 6 % (libinput décide du contact par la pression, avec un seuil de 5 % sur une tablette qu'il ne connaît pas). Inclinaison en centièmes de degré (`ABS_TILT_X/Y`, résolution en unités par radian), mêmes sens que le protocole. Bouton du stylet : `BTN_STYLUS`. La rotation ne passe pas : libinput ne la lit que sur certains stylets Wacom. Rien à redonner à intervalles. `kynput_host_pen_available()` est vrai sous Linux : la Station Linux annonce le stylet.

Journal de kynput : « 'KyberNow stylet' plugged », « Pen near (tip) » ou « (eraser) », « Pen left » ; chaque point en `debug`.

## Le cœur des apps

`InputPipeline::pen_on(écran, x, y, &PenInput)` (`apps/core/src/input/pipeline.rs`, `pen.rs`) : rend `false` quand le stylet doit partir en souris (pas de `PenAvailable`, ou souris des jeux) ; l'app envoie alors la position et les clics comme avant. Les changements de contact et le départ partent sûrement (réessayés), les mouvements une fois. Session derrière une autre ou droit repris pendant que la pointe touche : le stylet est levé, puis plus rien. La souris de l'appareil qui reprend (position ou clic) fait d'abord partir un stylet resté près de la surface. `tilt_from_altitude_azimuth` convertit l'altitude et l'azimut (Apple Pencil, Android) en inclinaison X / Y.

## Les apps

| App | Lecture du stylet | État |
|---|---|---|
| Linux | `src/native/pen.c` : XInput2 sous X11 (connexion à part, libX11 et libXi chargées à l'exécution ; axes « Abs Pressure », « Abs Tilt X/Y » ; « eraser » dans le nom de l'appareil : la gomme ; survol immobile 500 ms : parti), `tablet-v2` sous Wayland (file à part sur la connexion de SDL). `src/device/pen.rs` : envoi, souris de SDL retenue tant que le stylet part en stylet (X11), stylet rejoué en souris dans SDL pour l'interface (Wayland, où le compositeur n'envoie plus le stylet en souris). `KYBERNOW_NO_PEN=1` laisse le stylet à SDL (dépannage). | écrit ; essayé dans la VM avec une tablette virtuelle (uinput) sous X11 (XWayland) et sous Wayland (GNOME) : approche, contact, pression, inclinaison, départ ; pas encore avec une Station |
| Mac | `StreamView.swift` : événements de souris venus de la tablette (`subtype == .tabletPoint`), `tabletProximity` (pointe ou gomme) ; pression, inclinaison (`tilt` × 64°), rotation ; bouton du stylet : le clic droit qu'en fait le pilote | écrit, compilé (copie à part, avec la nouvelle libkynput) ; pas essayé |
| iPad | `KyberKitIOS/StreamView.swift` : touchers `.pencil` (pression `force / maximumPossibleForce`, inclinaison d'après l'altitude et l'azimut, rotation de l'Apple Pencil Pro), touchers groupés (`coalescedTouches`), survol (`UIHoverGestureRecognizer`, `zOffset > 0`). Le Pencil écrit là où il touche dans tous les modes ; face à une Station d'avant, souris à cet endroit et clic gauche | écrit ; Swift compilé pour le simulateur ; à lier avec le pont iOS refait (`build-apple-mobile.sh ios`) ; pas essayé |
| Windows | `window.rs` : `WM_POINTER*` d'un pointeur `PT_PEN` (`GetPointerPenInfo`, historique des mouvements regroupés par `GetPointerPenInfoHistory`) ; pression 0-1024, inclinaison, rotation, gomme (`INVERTED` / `ERASER`), bouton ; position ramenée comme la souris ; souris tirée du stylet retenue (`GetMessageExtraInfo`) et gestes de Windows Ink coupés pendant que le stylet part en stylet ; le toucher et le stylet en souris à `DefWindowProc`, comme avant. Une Wacom sans « Windows Ink » reste une souris | écrit, compilé pour x64 et ARM64 ; pas essayé |
| Android | `session/Inputs.kt` (`StylusInput`) : `MotionEvent` d'outil `TOOL_TYPE_STYLUS` / `TOOL_TYPE_ERASER` sur l'image, avant le doigt (le stylet écrit là où il touche quel que soit le mode du toucher) ; pression `getPressure`, inclinaison d'après `AXIS_TILT` et `AXIS_ORIENTATION` (pont JNI `Native.inputPen`, `jni/src/exports.rs`), positions groupées (`getHistorical*`), bouton `BUTTON_STYLUS_PRIMARY`, survol `ACTION_HOVER_*` (`dispatchGenericMotionEvent`, sur l'image seulement). Le départ attend 120 ms (Android envoie la fin du survol juste avant le contact, et le lever juste avant le survol). Face à une Station d'avant, souris à cet endroit et clic gauche | écrit, compilé, tests Robolectric (`StylusInputTest`) ; pas essayé sur un appareil ni avec une Station |
| Web | Pointer Events `pointerType == "pen"` (`pressure`, `tiltX`, `tiltY`, `twist`, `buttons` 32 pour la gomme) dans le moteur web de kynput | à faire |

Le pont Apple (`apps/apple/src/input.rs`) a `kn_input_pen` et `kn_pen_tilt`, pour le Mac et l'iPad (et l'Apple TV, qui compile `InputCore.swift` sans s'en servir).

## Essais à faire

1. Station Windows (compilée avec 0027) et app Linux sous X11 (Cinnamon, tablette Wacom) : ligne « Synthetic pen created » au journal de kynput, pression dans Krita ou Photoshop, inclinaison (pinceau qui s'aplatit), gomme, bouton du stylet (clic droit), survol (curseur du logiciel de dessin), départ du stylet puis souris.
2. Même chose sous Wayland (GNOME, KDE), et les menus de l'app au stylet (barre, réglages).
3. Mac avec une tablette, iPad avec un Apple Pencil (pression, inclinaison, survol d'un iPad M2 ou après), tablette ou téléphone Android à stylet (Galaxy à S Pen : pression, survol, bouton ; inclinaison selon l'appareil) ; sur l'émulateur Android, `adb shell input stylus swipe …` doit donner un stylet sans pression ni inclinaison (pas encore essayé).
4. Face à une Station d'avant (sans 0027) : le stylet en souris comme avant, sans avis.
5. Invité d'un partage avec le clavier et la souris (stylet), puis sans (rien) ; souris reprise par l'hôte pendant que l'invité écrit (stylet levé).
6. Plusieurs écrans : le stylet sur la fenêtre du second écran.
7. Station Mac (compilée avec 0016, autorisation Accessibilité) et app Mac avec une tablette, ou iPad avec un Apple Pencil : « Pen near (tip) » au journal de kynput, pression et inclinaison dans Krita ou Photoshop sur le Mac, gomme (outil gomme choisi à l'approche), bouton du stylet en clic droit, survol (curseur du logiciel), départ du stylet puis souris. Déjà vérifié sans tablette : suite des événements (essais unitaires) et lecture par AppKit.
8. Station Linux (compilée avec le correctif Linux 0006), sous GNOME puis KDE Wayland, puis sous X11 : « 'KyberNow stylet' plugged » et « Pen near (tip) » au journal de kynput, `libinput list-devices` montre une tablette, pression et inclinaison dans Krita, gomme, bouton du stylet, survol, départ du stylet puis souris (le pointeur absolu reprend). Déjà vérifié sans tablette : traduction (essais unitaires sur le Mac).
