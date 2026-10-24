# Prototype A — Kyber 0.29 end to end (Windows computer → Mac)

English · [Français](prototype-A.md)

> **Archive.** Snapshot of 19 October 2026: it describes the project as it was then, not as it is now. The current state is in the living docs; planned work is in [ROADMAP.en.md](../../ROADMAP.en.md).

Goal: a Kyber server running on the NVIDIA computer, the command-line macOS client connecting to it, and a **reference latency measurement** before writing a single line of our own code.

## 1. On the Windows computer

### 1.1 Preparing the machine
- Windows 10 or 11, **up-to-date NVIDIA driver** (GeForce Experience / NVIDIA App or nvidia.com).
- **Ethernet cable** for the computer (and if possible for the Mac) during the measurements; Wi-Fi will come later, for comparison.
- A **connected display** (or an HDMI/DisplayPort dummy plug): Kyber's virtual display driver is not published yet, and without a display there is nothing to capture.
- Note the computer's IP address: `ipconfig` in a terminal → “IPv4 Address” (e.g. `192.168.1.42`). Ideally, reserve this IP for it in the router.

### 1.2 Installing Kyber 0.29.0
1. Download the official installer:
   https://gitlab.com/api/v4/projects/78083474/packages/generic/kyber-installer/0.29.0/kyber-installer-win64-0.29.0.exe
2. Run it as administrator. If it is not signed, SmartScreen shows a warning: “More info” → “Run anyway”.
3. In the wizard:
   - **Encoder**: `NVENC`
   - **TLS**: `Generate` (self-signed certificate, enough for the LAN)
   - **Authentication**: `Basic`, with a username and password of your choice (keep them for the client)
   - **Startup**: `Windows service`
4. Install the **ViGEmBus** gamepad driver (needed for gamepads): https://github.com/nefarius/ViGEmBus/releases (latest version, `.exe`).

### 1.3 Performance settings
Kyber's docs measure 2.5 ms of NVENC encoding at 1080p with the right settings, against 8 ms when the GPU saves power.
- **NVIDIA Control Panel** → Manage 3D settings → Power management mode → **Prefer maximum performance**.
- **Windows power plan**: in an administrator PowerShell:
  ```powershell
  powercfg /setactive SCHEME_MIN
  ```
  (“High performance”; switch back afterwards with `powercfg /setactive SCHEME_BALANCED` if needed).
- Resolution and refresh rate of the computer's display: start at **1920×1080 at 60 Hz** for the reference.

### 1.4 Checking
In a PowerShell:
```powershell
sc query kyber                                   # STATE : RUNNING
Get-NetFirewallRule -DisplayName *kyber* | Format-Table DisplayName, Enabled, Direction, Action
```
If no firewall rule appears, add them (administrator PowerShell):
```powershell
New-NetFirewallRule -DisplayName "Kyber TCP 8080" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow -Profile Private
New-NetFirewallRule -DisplayName "Kyber UDP 8080" -Direction Inbound -Protocol UDP -LocalPort 8080 -Action Allow -Profile Private
```
Also check that the Windows network is set to the **Private** profile (Settings → Network → connection properties).

Useful files:
- Config: `C:\Program Files\Kyber\kyber_config.toml` (restart the service after a change: `net stop kyber` then `net start kyber`)
- Logs: `C:\Program Files\Kyber\log\`

### 1.5 First test without a native client
From **Chrome** on the Mac (not Safari: black screen): `https://<IP-du-PC>:8080/webclient/` → accept the certificate warning → sign in with the Basic username. If the computer's image appears, the server is fine.

✅ Validated on 2026-10-19: computer `192.168.1.100`, image received in Chrome.

## 2. On the Mac (native client)

The client is built from source (`third_party/kyber-desktop`, tag `0.29.0`, arm64): `third_party/kyber-desktop/Kyber-Client.app` (see `docs/build-notes.md` ([English](../build-notes.en.md)) for the Xcode 27 workaround).

```bash
tools/latency/run_client.sh 192.168.1.100 matthieu            # options kyclient en plus si besoin
```
(Extra kyclient options can be added if needed.) The launcher asks for the password without showing it, connects with `--tls-tofu` (trusts the computer's certificate on first connection, like SSH) and `--metrics true`, then analyses the session when it closes. Each session is stored in `tools/latency/runs/<date>/` (`metrics.json`, `client.log`, `report.txt`). Shortcut: `Ctrl+Alt+F` for full screen.

## 3. Measuring latency

### 3.1 Pipeline latency (automatic)
The client timestamps each frame at each step (capture → encoding → sending → reception → decoding → display) and aligns the computer's clock.
1. On the computer, open `tools/latency/timer.html` in a browser in full screen (F11): it produces an image that changes constantly, otherwise the capture sends only a few frames.
2. Launch the client with `--metrics true`, let it run for **60 s**, close it.
3. Analyse:
   ```bash
   python3 tools/latency/analyze_metrics.py metrics.json
   ```
   You get p50 / p95 / p99 per step and in total.

### 3.2 Latency “from the computer's screen to the Mac's screen” (camera)
The automatic measurement includes neither the rendering delay before capture, nor the display by the Mac's screen.
1. The full-screen timer on the computer, the Kyber client in full screen on the Mac, both screens side by side.
2. Photograph both screens together (or film in slow motion at 240 fps) about ten times.
3. Latency = number shown on the computer − number shown on the Mac. Keep the median.

### 3.3 Reference test matrix
| # | Network | Codec | Bitrate | Notes |
|---|---|---|---|---|
| 1 | Ethernet | h264 | default | reference |
| 2 | Ethernet | hevc | default | `--video-codec hevc` |
| 3 | Ethernet | h264 | 50 Mb/s | `--bitrate` (unit given by the help: bytes) |
| 4 | Wi-Fi | h264 | default | comparison |
| 5 | Ethernet | h264 | default | fast-moving game + gamepad |

Record the results in `docs/prototype-A-results.md` ([English](prototype-A-results.en.md)).
