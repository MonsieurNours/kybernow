# KyberNow: playing your PlayStation

English · [Français](playstation.md)

Link apps also show a PS5 or PS4 at home, like Sony's Remote Play: the console's picture and sound, driven by the device's controller. The console appears on the home screen next to the computers.

This document is the contract for this work. The exact wording will go in [link-ui.en.md](link-ui.en.md), section “PlayStation consoles”, when the screens are written. Status: **framed on 2026-11-03; blocks 1 and 2 (discovery, wake, account ID, pairing) and block 3 (session request, control channel, Takion transport, stream start: H.265 picture and sound received and decrypted) written in the core and tried on the PS5 Pro on 2026-11-03; pictures assembled (error correction), in SDR and HDR10; block 4 in the Mac app (home screen, pairing with the Sony sign-in, play shown by VideoToolbox and Metal, HDR10 on EDR) tried on 2026-11-03; controller (block 5): buttons, sticks, triggers, rumble (from the HD haptics) and adaptive triggers tried in the Mac app on 2026-11-03; touchpad and gyroscope tried; picture recovery after a loss and play resume after a drop; still to do: true HD haptics, then keyboard and mouse, settings and the session bar** ([apps/core/README.en.md](../apps/core/README.en.md#playstation-consoles-srcplaystation)).

## Decisions

| Topic | Choice |
|---|---|
| Consoles | PS5 (Pro included) and PS4, from the start |
| Protocol | Written in Rust in the core (`apps/core`), after [chiaki-ng](https://github.com/streetpea/chiaki-ng) (AGPL-3.0, like KyberNow), which serves as reference and as a control during trials. No link to chiaki-ng's C library |
| First app | Mac. The others follow, on the same core |
| Away from home | Through the KyberNow relay or a VPN (Tailscale, WireGuard). No use of Sony's servers (PSN “remote play”) |
| PSN account | The account ID, which the console asks for when pairing, is read by signing in to the Sony account in an app window, **once only**, then kept by the app. The account is used for nothing else (no connection through Sony's servers). As a fallback, if Sony's page changes: typing the ID by hand |
| DualSense | Adaptive triggers and HD haptics in the first version |
| Brand | “PS5” and “PS4” written out to name the console; no Sony logo. A project not affiliated with Sony, to be stated in the docs and on the website |
| Keyboard and mouse | Turned into a controller in the app (Remote Play only carries a controller): keyboard on the buttons and left stick, mouse on the right stick (adjustable sensitivity and curve) and clicks on R2/L2; a careful default layout, changeable key by key in the settings, following the Mac's keyboard layout (ZQSD on AZERTY); mouse captured during play, released by the usual shortcut; a connected controller excludes keyboard and mouse; text typed directly when the console opens its keyboard. For menus and action games. After the controller |
| Settings | “Settings for {name}” gear on the console, laid out like a computer's. On the home screen, no profiles: “Auto” (the best the console and screen allow) or “Custom”. Resolution: Auto, 1080p, 720p, 540p, 360p (the PS5 Pro, system 14.22, refuses 1440p and 4K on 2026-11-03: “AvCap failed to initialize video”, whatever the bitrate); 30 or 60 fps; bitrate Auto (follows the resolution) or chosen up to 100 Mbit/s; format to choose (Auto, H.264, H.265); full screen or window kept per console, full screen the first time; keyboard and mouse layout per console; DualSense intensities with the full DualSense |
| Trials | A PS5 Pro on the same network as the Mac. No PS4: its part is written after chiaki-ng and remains to be tried |

## What the user sees

- **Finding the console**: PS5 and PS4 consoles that are on or in rest mode appear on the home screen, with their name and state. One can also be added by its address.
- **Pairing it, once**: the app first opens Sony's sign-in page to read the PSN account ID (once for all consoles of that account), then asks for the 8-digit code shown on the console (Settings › System › Remote Play › Link Device on PS5), valid for 5 minutes. The pairing is kept by the app.
- **Waking it**: a console in rest mode wakes when chosen, like a sleeping computer.
- **Playing**: full-screen picture, sound, controller. Same session bar, same statistics and same way to quit as for a computer.
- **The controller**: a DualSense plugged in over USB or Bluetooth goes through as is, adaptive triggers, HD haptics, touchpad, gyroscope and light bar included. Any other controller is seen as a PlayStation controller: buttons, sticks, simple rumble.

## What the console does

- **Picture**: H.264 or H.265 (HEVC), chosen by the console. 1080p at 60 fps on PS5 according to chiaki-ng; the PS5 Pro's maximum resolution is to be noted at the first trial. HDR possible in HEVC on PS5. PS4: H.264 only, 720p or 1080p (PS4 Pro).
- **Sound**: Opus stereo (already decoded by the core, `audio/opus.rs`).
- **Network**: discovery by UDP broadcast (port 9302 on PS5, 987 on PS4), then a session on ports 9295 to 9297. IPv4 only.
- **One session at a time** per console.

## Architecture

The core keeps Kyber for computers and adds a second kind of source, the console. Blocks to write, in order, each compared with chiaki-ng on the same console:

1. **Discovery and wake** (`discovery` in chiaki-ng).
2. **Pairing**: signing in to the Sony account to read the ID (app web window, as chiaki-ng does), then the code (`regist`, `rpcrypt`: key tables specific to PS4 and PS5).
3. **Session**: encrypted HTTP control, Takion transport over UDP, bitrate test (`ctrl`, `session`, `takion`, `streamconnection`, `senkusha`). Protobuf messages (`takion.proto`, read with prost).
4. **Picture and sound**: reassembly, Reed-Solomon error correction, decryption (`videoreceiver`, `frameprocessor`, `fec`, `gkcrypt`, `audioreceiver`).
5. **Input and feedback**: controller state, touchpad, gyroscope (`feedback`, `controller`), and the other way round rumble, triggers and light bar.

Connecting through Sony's servers (`holepunch`, `rudp`, `stun` in chiaki-ng, the largest piece) is not carried over.

**Mac app**: the console's picture does not go through VLC (`kyvlcplayer` is tied to Kyber sessions). Decoding with VideoToolbox and display in the same window as Kyber sessions; the exact display layer is chosen in block 4. The DualSense goes through GameController (adaptive triggers); HD haptics arrive from the console as an audio stream, to be played on the controller's haptic channels over USB, and turned into simple rumble over Bluetooth (to be checked on macOS).

**Away from home**: a Station running on the console's network forwards the session's packets inside the relay tunnel, without decoding the picture. Over a VPN, the app reaches the console directly.

## Open points

- **PS5 Pro resolution** in Remote Play: to be noted at the first trial.
- **HD haptics on Mac**: is the DualSense haptic stream over USB reachable through Core Audio?
- **Relay**: forwarding a console session needs a Station on the same network; the “console alone, no PC” case only works over a VPN.
