# Installing KyberNow Station on Linux

English · [Français](INSTALLATION.md)

KyberNow Station turns your Linux computer into a Station: its screen, its sound and its controllers on your other devices, with the KyberNow Link apps. It works on GNOME, KDE Plasma, Sway and Hyprland, on Wayland. Everything is on the releases page: https://github.com/MonsieurNours/kybernow/releases

## Ubuntu, Debian, Fedora

Take the package for your system and processor (`x86_64` for a PC, `arm64` for an ARM machine), then:

```sh
sudo apt install ./KyberNow-Station-Linux-x86_64.deb     # Ubuntu, Debian
sudo dnf install ./KyberNow-Station-Linux-x86_64.rpm     # Fedora
```

The Station starts the next time you log in. Install it with your own account (`sudo`): that account can then update it from Admin. On Sway or Hyprland without a session launcher (uwsm), see “Startup” in [docs/station-linux.en.md](../../docs/station-linux.en.md).

Your devices' camera (“KyberNow Camera”) goes through PipeWire, seen by browsers and OBS. For Zoom, Discord and every other app to see it, also install v4l2loopback, then restart:

```sh
sudo apt install v4l2loopback-dkms        # Ubuntu, Debian
sudo dnf install akmod-v4l2loopback       # Fedora (RPM Fusion repository)
```

## Bazzite, Fedora Silverblue or Kinoite, SteamOS

On these systems, the system folder can't be changed: the Station installs in your home folder, without a reboot, and stays in place when the system updates. It then updates itself, from Admin, without a password.

1. **On SteamOS**: switch to Desktop Mode (Steam button › Power › Switch to Desktop). The Station works in Desktop Mode, not in Game Mode.
2. **Download** `KyberNow-Station-Linux-x86_64.tar.xz` (or `…-aarch64.tar.xz` for an ARM machine).
3. **Install**: open a terminal (Konsole) in the download folder and type

   ```sh
   tar -xf KyberNow-Station-Linux-x86_64.tar.xz
   ./kybernow-station-*/install.sh
   ```

4. **If your password is asked for**: it's so the keyboard, mouse and controllers of your devices can drive the computer (a system rule, added once). On SteamOS, if you've never set a password, type `passwd` first. On most machines with Steam, it isn't asked for: Steam already did it.
5. **You're set**: installation shows Admin's address and where your first account's password is. Your devices find the Station on the network on their own.

For the Station to be there after a reboot, your session must open on its own. On SteamOS and Bazzite Deck, which boot into Game Mode, choose to boot into Desktop Mode:

```sh
steamos-session-select plasma-persistent
```

### Removing the Station

```sh
~/.local/lib/kybernow-station/current/bin/kybernow-station-home uninstall
```

Your settings, accounts and pairings stay, for a later installation; add `--purge` to remove them too.
