# KyberNow installer

English · [Français](README.md)

`KyberNow-Setup.exe`: a single file that installs the Kyber 0.29 server, the administration console, the notification area icon, the SudoVDA virtual display driver and the ViGEmBus gamepad driver.

## Architecture

```
KyberNow-Setup.exe = [lanceur][contenu tar.zst][pied 32 o « KNPAYLD1 »]
```

(Launcher, tar.zst payload, 32-byte “KNPAYLD1” footer.)

- **Launcher** (`src/bin/setup.rs`): without WebView2, administrator rights. It drops the interface and `WebView2Loader.dll` into `C:\Program Files\KyberNow\.setup` (deleted afterwards), then launches the interface. Not into `%TEMP%`: the interface would run there as administrator next to files any of the user's programs can drop (a DLL loaded in its place, a UAC bypass). With `--uninstall`, it uninstalls (it is also `uninstall.exe` once installed).
- **Interface** (`src/bin/ui.rs` + `ui/`): borderless window (tao + wry/WebView2) that serves the Svelte app through the `kn://` protocol. JSON messages: `window.ipc.postMessage` one way, `window.__knReceive` the other (`src/events.rs`). v2 identity (“server” symbol, Archivo and Martian Mono embedded), four steps in the left column, whose seam fills up as you go: **Welcome** (the computer is already being analysed alongside, ports can be changed; nothing changes before “Install”), **Your account**, **Options** (virtual display, gamepads, sound, start with the computer, performance), **Installation** then “Done” (address, the console's QR code, `/ecran` in a browser, restart for the gamepads). Version shown: the one in `Cargo.toml`.
- **Core** (`src/`): `checks` (diagnosis; graphics card and encoder, see below), `ports` (random draw, reserved ranges, TCP + UDP), `install` (tasks), `uninstall`, `payload` (payload format).

## What the installation does

1. Stops and removes a previous version or official Kyber (NSIS uninstaller launched with `_?=` to wait for it).
2. Extracts the payload into `C:\Program Files\KyberNow\` (`kyber\`, `admin\`, `tray\`, `drivers\`, `licenses\`). In `kyber\`, the Kyber 0.29 files are overlaid by the patched overlay (`kyavserver.exe`, txproto and their runtime, `kycontroller.exe`, see [docs/kyber-changes.en.md](../../docs/kyber-changes.en.md)). Right after: `kybernow.json`, `uninstall.exe` and the entry in “Installed apps”, so that an installation interrupted further on can be uninstalled normally.
3. Generates Kyber's TLS certificate (`cert-gen.exe`).
4. `kyber-admin init`: port, encoder (see “Choosing the encoder”), single account (Kyber Basic + console), random JWT secret instead of Kyber's development key.
5. Creates the `kyber` service (quoted path, automatic restart on failure) and opens its start and stop to interactive users (`sc sdset`, see below).
6. Firewall: stream ports (TCP/UDP) and console port (TCP), the console's Bonjour announcement (UDP 5353, so the apps can find the computer), on the private and domain profiles; a “Public” network is switched to “Private”.
7. SudoVDA: certificate in Root + Trusted Publishers, device node and driver through `nefconc`.
8. ViGEmBus, if it is not already present (`/quiet /norestart`).
8. bis. Remote sound (option): Steam's virtual speakers (`SteamStreamingSpeakers.inf` through `pnputil /add-driver /install`) if Steam is installed. The console switches Windows output to them during sessions: the sound leaves the computer's speakers and goes out with the right number of channels.
9. `kyber-admin` service (console, same permissions as `kyber`), High performance power mode (option unticked by default if the computer already uses it; recreated with `powercfg /duplicatescheme` when missing, as on many laptops; if Windows refuses it, the step ends with a warning and the installation goes on), shortcuts to the console and the icon in the Start menu.
10. Notification area icon: launched at the end of the installation, and at each sign-in if “Start with Windows” is ticked (`KyberNow` value of `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). Without this option, both services switch to manual start.
11. Checked start: the installer waits for the `kyber` service to be running (`sc query`, state 4) and then for the server and the console to accept a connection on their ports; otherwise a “ATTENTION” line says so in the log.

## Reinstalling over KyberNow

The analysis reads `C:\Program Files\KyberNow\kybernow.json`: if KyberNow is already installed, the “Your account” step starts with a choice.

- **Keep my settings** (recommended, offered when `kyber\kyber_config.toml` and `C:\ProgramData\KyberNow\kyber-admin.toml` exist): only the programs, the drivers and Kyber's overlay are replaced. `kyber_config.toml` is not extracted from the payload (a copy first goes to `C:\ProgramData\KyberNow\backups\kyber_config.toml.reinstallation-<date>`), Kyber's certificate (`kybertest_cert.pem`, `kybertest_key.pem`) is not regenerated, `kyber-admin init` is not run: Kyber accounts, JWT secret, encoder, console password, `devices.json` and `events.json` stay. The ports are the ones in use (`[kycontroller] port` in `kyber_config.toml`, `listen` in `kyber-admin.toml`, otherwise `kybernow.json`), and the firewall is redone with them. The “Your account” screen is skipped.
- **Start from scratch**: like a first installation (new account, new random ports, new certificate, original `kyber_config.toml`), with a warning: every device must be paired again.

An official Kyber installation (without `kybernow.json`) is always removed and replaced.

During the installation, the window does not close: the button is greyed out and Alt+F4 is ignored.

Log: `C:\Program Files\KyberNow\install.log` (and `%TEMP%\KyberNow-install.log`).

## Permissions of `C:\ProgramData\KyberNow`

By default, ProgramData lets any account create folders in it and become their owner. On a shared computer, a standard account could therefore drop a file into `update\` between the check of an update package and its extraction by PowerShell, which runs as SYSTEM. The data folder is reserved to SYSTEM and Administrators (full control), with no inheritance from ProgramData, and nothing in it can be read by other accounts (it holds the console password hash, the certificate and the set-up code).

- Applied by the installer (“files” step, before `kyber-admin init` writes the settings) and at every console start (`server/admin/src/update/acl.rs`, same code as `src/acl.rs`): Administrators as owner everywhere, explicit permissions on files removed, links (junctions, symbolic links, hard links) deleted without being followed. Descriptor: `O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)`.
- `update\`, `lancement.ps1` and `relaunch-tray` are created afterwards by the console or by `update.ps1`: they inherit it.
- A failure is written to `install.log` or `kyber-admin.log` without blocking the installation or the console.
- The icon, which runs with the user's rights, no longer reads anything in this folder: the console port comes from `C:\Program Files\KyberNow\kybernow.json` (readable by everyone, no secret), and the set-up code is handed over by its elevated part (see “Resetting the console password”).

To check on the computer: `icacls C:\ProgramData\KyberNow` must list `NT AUTHORITY\SYSTEM:(OI)(CI)(F)` and `BUILTIN\Administrators:(OI)(CI)(F)`, nothing else.

## Uninstalling

“Installed apps” › KyberNow › Uninstall runs `uninstall.exe --uninstall` (the launcher alone, without the payload). After confirmation, it asks whether to keep the settings, then:

- stops and deletes the `kyber` and `kyber-admin` services, stops the icon and removes its Run value (account that uninstalls), removes the firewall rules;
- SudoVDA: device node, driver store package (`pnputil /delete-driver oemNN.inf /uninstall /force`, `oemNN.inf` found by its original name `sudovda.inf` in `pnputil /enum-drivers`), certificate from the Root and Trusted Publishers stores;
- puts back the power mode used before KyberNow (and deletes the High performance mode if it had recreated it), and switches back to “Public” the network it had switched to “Private”, if it still has the same name. These previous settings are recorded in `kybernow.json` (`restore`) at the first installation, and kept by later ones and by updates;
- keeps `C:\ProgramData\KyberNow` if asked to (with a copy of `kyber_config.toml` in `backups\kyber_config.toml.desinstallation`, since the devices' accounts are in it), otherwise deletes it;
- shows the final dialog, then, once it is closed, deletes `C:\Program Files\KyberNow` with a deferred command (the uninstaller runs from that folder).

ViGEmBus stays installed: other software may use it.

## Choosing the encoder

The analysis lists Windows' graphics cards (`Win32_VideoController`, connected eGPUs included) and keeps only working NVIDIA (`VEN_10DE`), AMD (`VEN_1002`, `VEN_1022`) and Intel (`VEN_8086`) PCI cards with the manufacturer's driver: Microsoft's basic adapter (`display.inf`, card without a driver), disabled cards, virtual displays (SudoVDA) and remote desktop are left out. It then takes the most capable: NVIDIA → `nvenc`, otherwise AMD → `amf`, otherwise Intel → `qsv`, otherwise the processor → `x264` (warning “No compatible graphics card”). On a hybrid laptop (integrated Intel + NVIDIA), that means NVENC. If Windows does not answer, NVML is the fallback for NVIDIA cards. The choice is written to `[kyavserver] encoder` by `kyber-admin init` and can then be changed in the console (System › Advanced). The update (`update.ps1`) replaces only the programs, never `kyber_config.toml`: the chosen encoder is kept. So does the installer when reinstalling with “Keep my settings”; “Start from scratch” makes the choice again.

The “Graphics driver” check shows the driver version of that card (for NVIDIA, in the manufacturer's format: 32.0.15.6094 becomes 560.94).

## Notification area icon (`server/tray`)

`kybernow-tray.exe` runs in the user's session, without administrator rights. “server” symbol of the v2 identity (`resources/kybernow.ico`, regenerated by `resources/make-ico.py`: the symbol alone up to 32 px, the app tile above that), with a badge: green when running, Spectre during a session, orange while starting or stopping; greyed out when the server is stopped.

- **Left click: the panel** (`src/panel.rs`, `src/panel.html`). A borderless window (Windows 11 rounded corners, shadow) placed against the icon, on the taskbar side, which closes as soon as it loses focus or with Esc. WebView2 view (wry, tao loop), data in `%LOCALAPPDATA%\KyberNow\tray-webview`. At the top, the state: running (computer name, last session), live (device, since when, latency), starting, stopped. Then: “End the session” (`POST /api/local/end`), “Open the console”, “Install KyberNow x.y” if a release is published, “Restart the server” (`kyber` and `kyber-admin`), “Stop / Start the server” (`kyber` service, the console keeps running), “Start with the computer”, “Reset the console password…”, and “Close the icon”, which offers to stop the server too.
- **Right click: the Windows menu**, with the same actions. Without WebView2, the menu also opens on left click. `WebView2Loader.dll` is installed next to the icon (`tray\`), by the installer and by the update.
- **Resetting the console password.** The icon relaunches itself as administrator (`--reset-console-password`, UAC prompt): it stops the console, runs `kyber-admin.exe reset-password`, restarts it. The console then writes a set-up code (`C:\ProgramData\KyberNow\setup-code.txt`, readable by administrators only); the icon's elevated part reads it there and leaves a copy in the account's profile (`%LOCALAPPDATA%\KyberNow\setup-code.txt`), which the ordinary icon reads and then deletes. The panel shows and copies it until the console says it has a password (`GET /api/session`), and the console opens on the first visit. If another administrator account is typed into the UAC prompt, the copy goes to that other account's profile: the icon then points to the ProgramData file. Devices and their accounts do not change.

“Open the console”: `https://localhost:<port>`, port read from `C:\Program Files\KyberNow\kybernow.json` (`console_port`, written by the installer; the console does not change port outside an installation). A second launch (Start menu) opens the console.

- **Service permissions.** The installer applies the `SERVICE_SDDL` SDDL (`src/install.rs`) to both services: Windows' default, plus `RP` (start) and `WP` (stop) for `IU` (interactive users). `DT` (pause) is useless and no longer granted (0.2.6). Same string in `update.ps1`. Changing the configuration (`DC`) stays reserved for administrators: granting `DC` would allow changing the executable of a service that runs as SYSTEM.
- **Start with the computer.** The box follows the start type of the `kyber` service. Toggling it changes that of both services (automatic or manual) and the user's Run value. Changing the start type requires elevation: the icon then relaunches itself as administrator (`--start-with-computer on|off`), with a one-off UAC prompt.
- **Current session.** The console answers `GET /api/local/state` and `POST /api/local/end` without authentication, but only from the computer itself (`127.0.0.1`): state of the `kyber` service, a device's session (virtual display or sound opened by the app), Kyber identifier, connected device and its latency, last session, the computer's name and address.
- **Uninstall.** The process is stopped and the Run value removed (for the user who uninstalls).

Building it alone: `cd server/tray && cargo build --release --target x86_64-pc-windows-gnu` (with the linker from `build.sh`).

## Building (from the Mac)

`build.sh` produces `dist/KyberNow-Setup.exe`; `package-update.sh` then produces the update package for a computer that is already installed (`dist/kybernow-update.zip`, with `update.ps1` and `version.txt`, plus `dist/SHA256SUMS`). Publishing: [docs/releases.en.md](../../docs/releases.en.md). `update.ps1` can be tested without Windows with `pwsh -NoProfile -File tests/update-bench.ps1`.

```bash
server/installer/build.sh      # → server/installer/dist/KyberNow-Setup.exe
```

Prerequisites: `brew install mingw-w64 zstd`, `rustup target add x86_64-pc-windows-gnu`, node, and the payloads extracted into `third_party/payload/` (see the script's header).

## Developing the interface

```bash
npm --prefix server/installer/ui run dev     # navigateur : backend simulé (ui/src/lib/backend.ts)
```

(In a browser, with a simulated backend.) Outside Windows, `kybernow-ui` also works with a simulated installation run.
