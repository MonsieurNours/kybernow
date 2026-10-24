# Installing KyberNow Link on Linux, the Steam Deck and the Raspberry Pi

English · [Français](INSTALLATION.md)

KyberNow Link for Linux is a **Flatpak** package, attached to every release published on GitHub: `KyberNow-Link-Linux-x86_64.flatpak` (Steam Deck, PC) and `KyberNow-Link-Linux-aarch64.flatpak` (ARM machines). It installs without administrator rights; the freedesktop runtime comes from Flathub on first install. For the **Raspberry Pi**, it is a **.deb** package (`KyberNow-Link-RaspberryPi-arm64.deb`), see below.

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

## Raspberry Pi (Raspberry Pi OS)

For a Raspberry Pi 3, 4 or 5 (Pi 400, 500 and Compute Modules included) on **64-bit Raspberry Pi OS** (Debian 13 "trixie", desktop, or Lite with a desktop), the package is a `.deb` that uses the chip's decoders: HEVC on Pi 4 and 5 (up to 4K at 60 Hz), H.264 on Pi 3 and 4.

1. **Download** `KyberNow-Link-RaspberryPi-arm64.deb` from the releases page (https://github.com/MonsieurNours/kybernow/releases).
2. **Install** (a terminal, or a double-click in the file manager):

   ```sh
   sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb
   ```

   apt also installs what is missing (SDL2, libopus, libdrm…). KyberNow Link shows up in the menu, Games category; on the command line: `kybernow`.
3. **On a TV, with a controller**: App settings › Interface › Living room gives the ten-foot interface (A confirms, B goes back, Y settings, X add; session menu by L1 + R1 + Start + Select). `kybernow --ui tv` does the same.

What the Pi allows is stated in Settings › Picture (greyed choices, "Pi decoder" and "CPU" tags): 4K at 60 Hz at most on the HDMI output of Pi 4 and 5, 1080p at 120 Hz when the TV announces it, no variable refresh; on the Pi 5, H.264 goes through the CPU (1080p), HEVC through the chip; 4:4:4 colour through the CPU, 1080p at most.

**apt repository** (once published): updates will then come with `sudo apt upgrade`. The repository is declared once:

```sh
curl -fsSL https://www.kybernow.app/apt/kybernow.gpg | sudo tee /usr/share/keyrings/kybernow.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/kybernow.gpg] https://www.kybernow.app/apt stable main" | sudo tee /etc/apt/sources.list.d/kybernow.list
sudo apt update && sudo apt install kybernow-link
```

Uninstalling: `sudo apt remove kybernow-link`; computers, settings and the log are in `~/.config/kybernow/`.

**Not yet tried on a board** (23/10/2026): the package is built and installed in an ARM64 virtual machine; the Pi's HEVC decoder, the HDMI output and Bluetooth controllers remain to be verified on a real Raspberry Pi. The aarch64 Flatpak also works on the Pi, but without the chip's decoder (HEVC on the CPU).

## KyberNow Link OS (the image to flash)

A Raspberry Pi that does nothing else: the card boots into KyberNow Link, on a TV with a controller or at a desk, with no Linux desktop. Pi 3, 4 and 5 (Pi 400, 500, Compute Module); in-place (A/B) updates need a Pi 4 or 5.

1. **Flash** `KyberNow-Link-OS.img.xz` (releases page) onto a microSD card of at least 8 GB (or an SSD) with **Raspberry Pi Imager**: Choose OS › Use custom, and the file; or paste the KyberNow repository into Imager › Settings › Custom repository: `https://www.kybernow.app/updates/rpi-imager.json`, and "KyberNow Link OS" shows up in the list. No OS customisation to do in Imager: everything is set at first boot.
2. **Plug in**: HDMI to the TV (the HDMI 0 socket, next to the power, on Pi 4 and 5), Ethernet if you can, power. The mark shows, then lights up with sound.
3. **First boot, with a controller**: put it in pairing mode (Xbox: pairing button near the USB port; DualSense: Create + PS; Switch Pro: SYNC), it is accepted without a code; a USB controller, the TV remote (HDMI-CEC) or a keyboard work too. Then the language, the network (Ethernet recognised, otherwise Wi-Fi and its password), the use (Living room or Desktop), and the computer (the six-digit code, accepted in Admin › Link devices).
4. Afterwards: ☰ on the home screen opens the **Device settings** (network, controllers, screen, sound, update, language, SSH); **Start held three seconds** switches Living room ⇄ Desktop; Guide (or PS) puts the device to standby, a key wakes it.

**Updating the OS**: Device settings › Update. The new version downloads (as soon as it is out, when "Automatic" is on), then "Restart on X" tries it in the card's other slot; if it does not boot up to the home screen, the Pi goes back to the previous version by itself. Computers and settings live on a separate partition and stay.

**Not yet tried on a board** (2026-10-24): the image builds and boots under QEMU emulation (first boot included), but no Raspberry Pi has booted it yet. See [os/README.en.md](os/README.en.md).

## Updating

A Flatpak installed from a file does not update itself: the app announces a new version ("Version X available"), installed the same way (`flatpak install --user` of the new file; computers and settings stay). The same goes for the Raspberry Pi `.deb` (`sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb`), as long as the apt repository is not declared.

## Uninstalling

```sh
flatpak uninstall --user fr.leslignesclaires.kybernow
rm -rf ~/.var/app/fr.leslignesclaires.kybernow   # computers, settings, log
```

## Log

`~/.var/app/fr.leslignesclaires.kybernow/config/kybernow/kybernow.log` (the previous one as `.1`). Started from a terminal (`flatpak run fr.leslignesclaires.kybernow`), the app also writes the log to the screen.
