# KyberNow Station installer

English · [Français](README.md)

`KyberNow-Station-Setup.exe`: a single file that installs KyberNow Station: the Kyber 0.29 server, KyberNow Admin (the administration in the browser), the notification area icon, the SudoVDA virtual display driver and the ViGEmBus gamepad driver.

## Architecture

```
KyberNow-Station-Setup.exe = [launcher][tar.zst payload][32-byte “KNPAYLD1” footer]
```

- **Launcher** (`src/bin/setup.rs`): does not use WebView2, administrator rights. It checks that the payload is attached to the file, and that WebView2 is present on the computer (otherwise it says so and opens Microsoft's download page). It drops the interface and `WebView2Loader.dll` into `C:\Program Files\KyberNow\.setup` (deleted afterwards), then launches the interface. Not into `%TEMP%`: the interface would run there as administrator next to files any of the user's programs can drop (a DLL loaded in its place, a UAC bypass). WebView2's data folder (profile, cache: no code), on the other hand, stays in `%TEMP%`: WebView2's sandboxed processes cannot write under Program Files and Edge would refuse to start. With `--uninstall`, it uninstalls (it is also `uninstall.exe` once installed).
- **Interface** (`src/bin/ui.rs` + `ui/`): borderless window (tao + wry/WebView2) that serves the Svelte app through the `kn://` protocol. JSON messages: `window.ipc.postMessage` one way, `window.__knReceive` the other (`src/events.rs`). v2 identity (KyberNow Station signed logo, `ui/src/lib/Logo.svelte`, taken from `brand/v2/tools/geometry.json`; “server” symbol; Archivo and Martian Mono embedded), four steps in the left column, whose seam fills up as you go: **Welcome** (the computer is already being analysed alongside, ports can be changed; nothing changes before “Install”), **Your account**, **Options** (virtual display, gamepads, sound, start with the computer, performance), **Installation** then “Done” (address, Admin's QR code, `/ecran` in a browser, restart for the gamepads). Version shown: the one in `Cargo.toml`.
- **Core** (`src/`): `checks` (diagnosis; graphics card and encoder, see below), `ports` (random draw, reserved ranges, TCP + UDP), `install` (tasks), `uninstall`, `payload` (payload format). The stream and Admin ports are drawn at random in 20000–44999, outside the ranges Windows reserves (IPv4 and IPv6, TCP and UDP) and Steam's (27000–27100), and are only checked to be free at installation time.

## What the installation does

1. Stops and removes a previous version or official Kyber (NSIS uninstaller launched with `_?=` to wait for it).
2. Extracts the payload into `C:\Program Files\KyberNow\` (`kyber\`, `admin\`, `tray\`, `drivers\`, `licenses\`). In `kyber\`, the Kyber 0.29 files are overlaid by the patched overlay (`kyavserver.exe`, txproto and their runtime, `kycontroller.exe`, the input server `kynput.dll` and `kynputserver.exe`, the web client engine in `webclient\`, see [docs/kyber-changes.en.md](../../docs/kyber-changes.en.md)). Right after: `kybernow.json`, `uninstall.exe` and the entry in “Installed apps” (display name “KyberNow Station”, `DISPLAY_NAME`; key `…\Uninstall\KyberNow`, `PRODUCT`, the folders' name, which does not change: existing installations, Admin and `update.ps1` depend on it), so that an installation interrupted further on can be uninstalled normally.
3. Generates Kyber's TLS certificate (`cert-gen.exe`).
4. `kyber-admin init`: port, encoder (see “Choosing the encoder”), single account (Kyber Basic + Admin, recorded as the installation account), random JWT secret instead of Kyber's development key, written to `C:\ProgramData\KyberNow\kyber-jwt.key` (`kyber_config.toml` only carries the path). Just before, and again after: `kyber\kyber_config.toml`, Kyber's certificate and key (`kybertest_cert.pem`, `kybertest_key.pem`) are reserved to SYSTEM and Administrators (`src/acl.rs`, `secure_file`, descriptor `O:BAD:PAI(A;;FA;;;SY)(A;;FA;;;BA)`), also with “Keep my settings”: Program Files is readable by everyone and these files hold the accounts' password hashes. Admin reapplies these permissions at every start.
5. Creates the `kyber` service (display name “KyberNow Station - serveur de streaming” or “KyberNow Station - streaming server” depending on the language, quoted path, automatic restart on failure) and opens its start and stop to interactive users (`sc sdset`, see below).
6. Firewall: stream ports (TCP/UDP) and Admin port (TCP), Admin's Bonjour announcement (UDP 5353, so the apps can find the computer), one rule per program for `kycontroller.exe`, `kyavserver.exe` and `kynputserver.exe`, on the private and domain profiles; a “Public” network is switched to “Private”.
7. SudoVDA: certificate in Root + Trusted Publishers, device node and driver through `nefconc`.
8. ViGEmBus, if it is not already present (`/quiet /norestart`).
8. bis. Remote sound (option): Steam's virtual speakers (`SteamStreamingSpeakers.inf` through `pnputil /add-driver /install`) if Steam is installed. Admin switches Windows output to them during sessions: the sound leaves the computer's speakers and goes out with the right number of channels.
9. `kyber-admin` service (KyberNow Admin, display name set by `kyber-admin service install`, same permissions as `kyber`), High performance power mode (option unticked by default if the computer already uses it; recreated with `powercfg /duplicatescheme` when missing, as on many laptops; if Windows refuses it, the step ends with a warning and the installation goes on), “KyberNow Admin” (`.url`) and “KyberNow Station” (the icon, `.lnk`) shortcuts in the Start menu's `KyberNow` folder; those of versions 0.3.1 and earlier (“Console KyberNow”, “KyberNow console”, “KyberNow”) are removed (`LEGACY_SHORTCUTS`). An update, through `update.ps1` as through Admin's applicator (`server/admin/src/update/apply.rs`), removes these shortcuts the same way and sets the current display names (services, “Installed apps”).
10. Notification area icon: launched at the end of the installation, and at each sign-in if “Start with the computer” is ticked (`KyberNow` value of `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). Without this option, both services switch to manual start.
11. Checked start: the installer waits for the `kyber` service to be running (`sc query`, state 4) and then for the server and Admin to accept a connection on their ports; otherwise a “ATTENTION” line says so in the log.

## Reinstalling over KyberNow Station

The analysis reads `C:\Program Files\KyberNow\kybernow.json`: if KyberNow Station is already installed, the “Your account” step starts with a choice.

- **Keep my settings** (recommended, offered when `kyber\kyber_config.toml` and `C:\ProgramData\KyberNow\kyber-admin.toml` exist): only the programs, the drivers and Kyber's overlay are replaced. `kyber_config.toml` is not extracted from the payload (a copy first goes to `C:\ProgramData\KyberNow\backups\kyber_config.toml.reinstallation-<date>`), Kyber's certificate (`kybertest_cert.pem`, `kybertest_key.pem`) is not regenerated, `kyber-admin init` is not run: Kyber accounts, JWT secret, encoder, Admin password, `devices.json` and `events.json` stay. The ports are the ones in use (`[kycontroller] port` in `kyber_config.toml`, `listen` in `kyber-admin.toml`, otherwise `kybernow.json`), and the firewall is redone with them. The “Your account” screen is skipped.
- **Start from scratch**: like a first installation (new account, new random ports, new certificate, original `kyber_config.toml`), with a warning: every device must be paired again.

An official Kyber installation (without `kybernow.json`) is always removed and replaced.

During the installation, the window does not close: the button is greyed out and Alt+F4 is ignored.

Log: `C:\Program Files\KyberNow\install.log` (and `%TEMP%\KyberNow-install.log`).

## Permissions of `C:\ProgramData\KyberNow`

By default, ProgramData lets any account create folders in it and become their owner. On a computer with several accounts, a standard account could therefore drop a file into `update\` between the check of an update package and its extraction by PowerShell, which runs as SYSTEM. The data folder is reserved to SYSTEM and Administrators (full control), with no inheritance from ProgramData, and nothing in it can be read by other accounts (it holds Admin password hash, the certificate and the set-up code).

- Applied by the installer (“files” step, before `kyber-admin init` writes the settings) and at every Admin start (`server/admin/src/update/acl.rs`, same code as `src/acl.rs`): top-down walk that never follows a link, each folder locked before it is read (no other account can create anything in it during the pass), Administrators as owner everywhere, explicit permissions replaced, junctions, symbolic links and hard links deleted (the link, not its target). Handles are opened in backup mode with the backup, restore and take-ownership privileges: a file another account protected does not stop the pass. Folder descriptor: `O:BAD:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)` (its contents inherit it).
- `update\` (packages, signed manifests, logs, `resultat.json`, `relaunch-tray`), `kyber-jwt.key` and `local-token` are created afterwards by Admin, its applicator or `update.ps1`: they inherit it. The package is extracted elsewhere, under `C:\Program Files\KyberNow\.staging\` (Program Files' permissions).
- A failure is written to `install.log` or `kyber-admin.log` without blocking the installation or Admin.
- The icon, which runs with the user's rights, reads nothing in this folder: Admin's port comes from `C:\Program Files\KyberNow\kybernow.json` (readable by everyone, no secret), and the set-up code is handed over by its elevated part (see “Resetting the Admin password”).

To check on the computer: `icacls C:\ProgramData\KyberNow` must list `NT AUTHORITY\SYSTEM:(OI)(CI)(F)` and `BUILTIN\Administrators:(OI)(CI)(F)`, nothing else; `icacls "C:\Program Files\KyberNow\kyber\kyber_config.toml"` (and both `kybertest_*.pem`) must list SYSTEM and Administrators with `(F)`, without `BUILTIN\Users`, and `type` from a standard account must be refused.

## Uninstalling

“Installed apps” › KyberNow Station › Uninstall runs `uninstall.exe --uninstall` (the launcher alone, without the payload). After confirmation, it asks whether to keep the settings, then:

- stops and deletes the `kyber` and `kyber-admin` services, stops the icon and removes its Run value (account that uninstalls), removes the firewall rules and the Start menu's `KyberNow` folder (current and old shortcuts);
- SudoVDA: device node, driver store package (`pnputil /delete-driver oemNN.inf /uninstall /force`, `oemNN.inf` found by its original name `sudovda.inf` in `pnputil /enum-drivers`), certificate from the Root and Trusted Publishers stores;
- puts back the power mode used before KyberNow Station (and deletes the High performance mode if it had recreated it), and switches back to “Public” the network it had switched to “Private”, if it still has the same name. These previous settings are recorded in `kybernow.json` (`restore`) at the first installation, and kept by later ones and by updates;
- keeps `C:\ProgramData\KyberNow` if asked to (with a copy of `kyber_config.toml` in `backups\kyber_config.toml.desinstallation`, since the devices' accounts are in it), otherwise deletes it;
- shows the final dialog, then, once it is closed, deletes `C:\Program Files\KyberNow` with a deferred command (the uninstaller runs from that folder).

ViGEmBus stays installed: other software may use it.

## Choosing the encoder

The analysis lists Windows' graphics cards (`Win32_VideoController`, connected eGPUs included) and keeps only working NVIDIA (`VEN_10DE`), AMD (`VEN_1002`, `VEN_1022`) and Intel (`VEN_8086`) PCI cards with the manufacturer's driver: Microsoft's basic adapter (`display.inf`, card without a driver), disabled cards, virtual displays (SudoVDA) and remote desktop are left out. It then takes the most capable: NVIDIA → `nvenc`, otherwise AMD → `amf`, otherwise Intel → `qsv`, otherwise the processor → `x264` (warning “No compatible graphics card”). On a hybrid laptop (integrated Intel + NVIDIA), that means NVENC. If Windows does not answer, NVML is the fallback for NVIDIA cards. The choice is written to `[kyavserver] encoder` by `kyber-admin init` and can then be changed in Admin (System › Advanced). The update (`update.ps1`) replaces only the programs, never `kyber_config.toml`: the chosen encoder is kept. So does the installer when reinstalling with “Keep my settings”; “Start from scratch” makes the choice again.

The “Graphics driver” check shows the driver version of that card (for NVIDIA, in the manufacturer's format: 32.0.15.6094 becomes 560.94).

## Notification area icon (`server/tray`)

`kybernow-tray.exe` runs in the user's session, without administrator rights. “server” symbol of the v2 identity (`resources/kybernow.ico`, regenerated by `resources/make-ico.py`: the symbol alone up to 32 px, the app tile above that), with a badge: green when running, Spectre during a session, orange while starting or stopping; greyed out when the Station is stopped. Tooltip: “KyberNow Station”, the state and the session.

- **Left click: the panel** (`src/panel.rs`, `src/panel.html`). A borderless window (Windows 11 rounded corners, shadow) placed against the icon, on the taskbar side, which closes as soon as it loses focus or with Esc. WebView2 view (wry, tao loop), data in `%LOCALAPPDATA%\KyberNow\tray-webview`, created at the first click only and released after 2 minutes without being opened (60 to 100 MB and a WebView2 process per user, otherwise permanently); the empty window stays. With the panel closed, the icon pushes nothing to it: the page asks for the state when it is created (“ready”) and receives it at each opening. At the top, the state: running (computer name, last session), live (device, since when, latency), starting, stopped. Then: “End the session” (`POST /api/local/end`), “Open KyberNow Admin”, “Install KyberNow Station x.y” if a release is published, “Restart the Station” (`kyber` and `kyber-admin`), “Stop / Start the Station” (`kyber` service, Admin keeps running), “Start with the computer”, “Reset the Admin password…”, and “Close the icon”, which offers to stop the Station too.
- **Right click: the Windows menu**, with the same actions. Without WebView2, the menu also opens on left click. `WebView2Loader.dll` is installed next to the icon (`tray\`), by the installer and by the update.
- **Resetting the Admin password.** The icon relaunches itself as administrator (`--reset-console-password`, UAC prompt): it stops Admin, runs `kyber-admin.exe reset-password`, restarts it. Admin then writes a set-up code (`C:\ProgramData\KyberNow\setup-code.txt`, readable by administrators only); the icon's elevated part reads it there and leaves a copy in the account's profile (`%LOCALAPPDATA%\KyberNow\setup-code.txt`), which the ordinary icon reads and then deletes. The panel shows and copies it until Admin says it has a password (`GET /api/session`), and Admin opens on the first visit. If another administrator account is typed into the UAC prompt, the copy goes to that other account's profile: the icon then points to the ProgramData file. Devices and their accounts do not change.

“Open KyberNow Admin”: `https://localhost:<port>`, port read from `C:\Program Files\KyberNow\kybernow.json` (`console_port`, written by the installer; Admin does not change port outside an installation). A second launch (Start menu) opens Admin.

- **Admin certificate pinned** (`src/http.rs`). Any interactive account can stop Admin's service (that is intended) and serve something else on its port: the icon therefore checks Admin's certificate. `kyber-admin init`, then Admin at every start, write the SHA-256 fingerprint of its certificate to `kybernow.json` (`console_fingerprint`, “sha256:aa:bb:…”, the same format as the apps' pairing; the folder is only writable by administrators, and the installer keeps the field when it rewrites the file). The icon reads the fingerprint again at every round (2 s) and rebuilds its client if it changes; its three calls (`GET /api/session`, `GET /api/local/state`, `POST /api/local/end|pair`) go through a minimal HTTPS client on rustls (one request per connection, `Connection: close`) whose verifier only accepts the certificate with that fingerprint. Without `console_fingerprint` (an installation from before 0.2.6 whose Admin has not restarted yet), the icon does not talk to Admin and shows “Admin is not responding”.

- **Service permissions.** The installer applies the `SERVICE_SDDL` SDDL (`src/install.rs`) to both services: Windows' default, plus `RP` (start) and `WP` (stop) for `IU` (interactive users). `DT` (pause) is useless and not granted. Same string in `update.ps1`, which applies it again on a Station already installed. Changing the configuration (`DC`) stays reserved for administrators: granting `DC` would allow changing the executable of a service that runs as SYSTEM.
- **Start with the computer.** The box follows the start type of the `kyber` service. Toggling it changes that of both services (automatic or manual) and the user's Run value. Changing the start type requires elevation: the icon then relaunches itself as administrator (`--start-with-computer on|off`), with a one-off UAC prompt.
- **Current session.** Admin answers `GET /api/local/state` and `POST /api/local/end` without an account, but only from the computer itself (`127.0.0.1`) and with the **local token**: drawn by Admin at its start and at every Windows logon, written to `C:\ProgramData\KyberNow\local-token` (reserved to SYSTEM and Administrators), copied by `kyber-admin display-agent local-token` into the profile of the session's account (`%LOCALAPPDATA%\KyberNow\local-token`, 64 hexadecimal characters, readable by that account and administrators), read by the icon at every call and sent in the `x-kyber-local` header (see [server/README.en.md](../README.en.md), “Local routes”). Without it, `403`: the icon shows “Admin is not responding” until the next copy (a few seconds after a logon or an Admin start). The state gives the `kyber` service, a device's session (virtual display or sound opened by the app), the Kyber identifier, the connected device and its latency, the last session, the computer's name and address, and the access requests without their code: the panel offers “Check in Admin” (which opens Link devices, where the code is compared and accepted) and “Refuse”.
- **Uninstall.** The process is stopped and the Run value removed (for the user who uninstalls).

Building it alone: `cd server/tray && cargo build --release --target x86_64-pc-windows-gnu` (with the linker from `build.sh`).

## Languages

French and English, following the rule in [docs/i18n.en.md](../../docs/i18n.en.md): if Windows' first display language (`GetUserPreferredUILanguages`, otherwise `GetUserDefaultUILanguage`) starts with `fr`, French; any other language, English. Neither the installer nor the tray icon has a language setting.

- **Installer.** The Rust texts (launcher and uninstall dialogs, analysis checks, port messages, errors shown on screen, name and description of the `kyber` service, Start menu shortcuts) live in `i18n/fr.json` and `i18n/en.json`, embedded (`src/i18n.rs`, `tr(lang, "key", &[("name", &value)])`). The interface gets the language from its host before it starts (`window.__knLang`) and takes its texts from `ui/src/lib/i18n/fr.json` and `en.json` (`t('screen.key', { name })`); in a browser, `?lang=en` or the browser's language. The log (`install.log`, and the “details” lines of the Installation screen that come from it) stays in French.
- **Tray icon.** Same catalogs in `server/tray/i18n/`: `tray` for the menu, tooltip and dialogs, `panel` for the panel (set in the page before it loads, `window.__knI18n`). The tray icon sends `Accept-Language: fr|en` to Admin, which answers in that language (errors, event names).
- **Checks.** A key missing in English falls back to French. `cargo test` (installer and tray icon) and `npm run check` (interface) check that both languages have the same keys and the same `{variables}`, and, for the tray icon, that every `t('…')` in `panel.html` exists. `KYBERNOW_LANG=en` (or `fr`) forces the installer's and the tray icon's language, for testing.

## Building (from the Mac)

`build.sh` produces `dist/KyberNow-Station-Setup.exe`; `package-update.sh` then produces the update package for a computer that is already installed (`dist/kybernow-update.zip`, with `update.ps1`, `version.txt` and `requirements.json`, which `update.ps1` does not read; plus `dist/update.ps1` and `dist/SHA256SUMS`). Publishing: [docs/releases.en.md](../../docs/releases.en.md); `scripts/release.sh` also publishes the installer under its old names, `KyberNow-Server-Setup.exe` and `KyberNow-Setup.exe`, identical, for Admin 0.3.1 and earlier and existing links (see “Old installer names”). `update.ps1` can be tested without Windows with `pwsh -NoProfile -File tests/update-bench.ps1`.

```bash
server/installer/build.sh      # → server/installer/dist/KyberNow-Station-Setup.exe
```

Prerequisites: `brew install mingw-w64 zstd`, `rustup target add x86_64-pc-windows-gnu`, node, and the payloads extracted into `third_party/payload/` (see the script's header).

## Developing the interface

```bash
npm --prefix server/installer/ui run dev     # browser: simulated backend (ui/src/lib/backend.ts)
```

In English: `http://localhost:5173/?lang=en` (`&existant` simulates a reinstallation).

Outside Windows, `kybernow-ui` also works with a simulated installation run.
