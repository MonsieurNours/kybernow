# Installing KyberNow Link on Linux and the Steam Deck

English · [Français](INSTALLATION.md)

KyberNow Link for Linux is a **Flatpak** package, attached to every release published on GitHub: `KyberNow-Link-Linux-x86_64.flatpak` (Steam Deck, PC) and `KyberNow-Link-Linux-aarch64.flatpak` (ARM machines). It installs without administrator rights; the freedesktop runtime comes from Flathub on first install.

## Steam Deck (SteamOS)

1. **Desktop Mode**: Steam button › Power › Switch to Desktop.
2. **Download** `KyberNow-Link-Linux-x86_64.flatpak` from the releases page (https://github.com/MonsieurNours/kybernow/releases), with Firefox or the Deck's browser.
3. **Install**: open Konsole and type

   ```sh
   flatpak install --user ~/Downloads/KyberNow-Link-Linux-x86_64.flatpak
   ```

   Double-clicking the file also opens Discover, which installs it.
4. **Add it to Steam**: open Steam in Desktop Mode › Games › Add a Non-Steam Game › tick **KyberNow Link** › Add Selected Programs.
5. **Back to Gaming Mode** (the "Return to Gaming Mode" desktop icon). KyberNow Link is in the library, Non-Steam tab.
6. **Controller layout**: on KyberNow Link's page › controller › Layout, pick the **"Gamepad with Mouse Trackpad"** template: sticks and buttons go to the computer's gamepad, the right trackpad moves the mouse, trackpad clicks click.

Then, with the controller: A confirms, B goes back, Y opens the computer's settings, X adds a computer, L1 and R1 change profile, Start opens the app settings. In a session, **L1 + R1 + Start + Select** held together (or a touch at the top of the screen) open the menu; "Keyboard" there opens Steam's keyboard.

## SteamOS in the living room (Steam Machine, PC on SteamOS or Bazzite)

As on the Deck; the app picks the living-room interface by itself (larger text, "Ready to play" column) when the screen is larger than 1280 × 800 under gamescope. App settings › Interface forces Desktop, Handheld or Living room.

## Desktop Linux (GNOME, KDE…)

```sh
flatpak install --user KyberNow-Link-Linux-x86_64.flatpak
flatpak run fr.leslignesclaires.kybernow
```

KyberNow Link also shows up in the applications menu. Session shortcuts: **Ctrl+Alt+Shift+D** disconnect, **Ctrl+Alt+Shift+F** full screen, **Ctrl+Alt+Shift+M** game or desktop mouse, **Esc** held for a second releases the mouse.

## Updating

A Flatpak installed from a file does not update itself: the app announces a new version ("Version X available"), installed the same way (`flatpak install --user` of the new file; computers and settings stay).

## Uninstalling

```sh
flatpak uninstall --user fr.leslignesclaires.kybernow
rm -rf ~/.var/app/fr.leslignesclaires.kybernow   # computers, settings, log
```

## Log

`~/.var/app/fr.leslignesclaires.kybernow/config/kybernow/kybernow.log` (the previous one as `.1`). Started from a terminal (`flatpak run fr.leslignesclaires.kybernow`), the app also writes the log to the screen.
