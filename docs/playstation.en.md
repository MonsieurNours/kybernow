# KyberNow: playing your PlayStation

English · [Français](playstation.md)

Link apps also show a PS5 or PS4 at home, like Sony's Remote Play: the console's picture and sound, driven by the device's controller. The console appears on the home screen next to the computers.

This document is the contract for this work. The exact wording will go in [link-ui.en.md](link-ui.en.md), section “PlayStation consoles”, when the screens are written. Status: **framed on 2026-11-03; blocks 1 and 2 (discovery, wake, account ID, pairing) and block 3 (session request, control channel, Takion transport, stream start: H.265 picture and sound received and decrypted) written in the core and tried on the PS5 Pro on 2026-11-03; pictures assembled (error correction), in SDR and HDR10; block 4 in the Mac app (home screen, pairing with the Sony sign-in, play shown by VideoToolbox and Metal, HDR10 on EDR) tried on 2026-11-03; controller (block 5): buttons, sticks, triggers, rumble (from the HD haptics) and adaptive triggers tried in the Mac app on 2026-11-03; touchpad and gyroscope tried; picture recovery after a loss without waiting for a key frame (pictures sorted by their references, 40 ms instead of a one-second freeze) and play resume after a drop; keyboard and mouse turned into a controller, tried; console settings (picture, bitrate, format, full screen, keyboard and mouse layout) tried; game bar and MetalFX upscaling tried; still to do: true HD haptics** ([apps/core/README.en.md](../apps/core/README.en.md#playstation-consoles-srcplaystation)).

## Decisions

| Topic | Choice |
|---|---|
| Consoles | PS5 (Pro included) and PS4, from the start |
| Protocol | Written in Rust in the core (`apps/core`), after [chiaki-ng](https://github.com/streetpea/chiaki-ng) (AGPL-3.0, like KyberNow), which serves as reference and as a control during trials. No link to chiaki-ng's C library |
| First app | Mac. The others follow, on the same core |
| Away from home | Through the KyberNow relay or a VPN (Tailscale, WireGuard). No use of Sony's servers (PSN “remote play”) |
| PSN accounts | **Several accounts** per device, each with its consoles; a console can be paired with several accounts (one profile per person), one card per console on the home screen and the account chosen before “Play” (kept per console). The ID and the online ID (`online_id`, in the same Sony answer as `user_id`) are read by signing in to the Sony account in an app window, **once per account**, then kept by the app (the device's secure storage). The account is used for nothing else (no connection through Sony's servers). As a fallback: ID and name typed by hand. Accounts, found consoles and pairing in the “PlayStation” tab of “Add a computer or a console”; removing an account removes its pairings (decided on 2026-11-03) |
| Console by address | The add window's “By address” tab also looks for a console at the address typed (direct discovery, ports 9302 and 987): through a VPN or from another network. The address is kept and searched directly afterwards |
| DualSense | Adaptive triggers and HD haptics in the first version |
| Brand | “PS5” and “PS4” written out to name the console; no Sony logo. A project not affiliated with Sony, to be stated in the docs and on the website |
| Keyboard and mouse | Turned into a controller in the app (Remote Play only carries a controller): keyboard on the buttons and left stick, mouse on the right stick (adjustable sensitivity and curve) and clicks on R2/L2; a careful default layout, changeable key by key in the settings, following the Mac's keyboard layout (ZQSD on AZERTY); mouse captured during play, released by the usual shortcut; a connected controller excludes keyboard and mouse; text typed directly when the console opens its keyboard. For menus and action games. After the controller |
| Settings | “Settings for {name}” gear on the console, laid out like a computer's. On the home screen, no profiles: “Auto” (the best the console and screen allow) or “Custom”. Resolution: Auto, 1080p, 720p, 540p, 360p (the PS5 Pro, system 14.22, refuses 1440p and 4K on 2026-11-03: “AvCap failed to initialize video”, whatever the bitrate); 30 or 60 fps; bitrate Auto (follows the resolution) or chosen up to 100 Mbit/s; format to choose (Auto, H.264, H.265); full screen or window kept per console, full screen the first time; the picture upscaled to the screen's pixels (MetalFX “spatial”, on by default, sharpness from 0 to 100 %, turned off by itself above 2 ms per frame; see “Upscaling”); keyboard and mouse layout per console; DualSense intensities with the full DualSense |
| Trials | A PS5 Pro on the same network as the Mac. No PS4: its part is written after chiaki-ng and remains to be tried |

## What the user sees

- **Finding the console**: PS5 and PS4 consoles that are on or in rest mode appear on the home screen, with their name and state. One can also be added by its address.
- **Pairing it, once per account**: the app first opens Sony's sign-in page to read the PSN account ID and online ID (once for all consoles of that account; or choose an account already kept), then asks for the 8-digit code shown on the console (Settings › System › Remote Play › Link Device on PS5), valid for 5 minutes. The pairing is kept by the app.
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

**Away from home**: a Station running on the console's network (Windows or Mac: the same Admin) forwards the session's packets inside the relay tunnel, without decoding the picture. Over a VPN, the app reaches the console directly (“By address”). Details: [relais.en.md](relais.en.md#playstation-consoles).

- **Which Station**: the one that sees the console. Each Admin looks for the consoles on its network and lists them (`GET /api/consoles`); the app keeps by itself, for each console, the Stations that see it (by the console's ID), with no setting. Several: the first that answers.
- **When**: the console doesn't answer directly (neither on the network nor at its kept address) and a Station that sees it can be reached through the relay, with “Access away from home” on. Waking, state and play then go through the Station.
- **Packets**: through the relay, the app asks the console for packets of at most 1,200 bytes (Takion MTU given in BIG), what a tunnel datagram carries; on the network, 1,454 as before.
- **Texts**: those of a computer through the relay: “Away from home” state on the home screen, “Via the relay” chip in the bar, “Path” line in the statistics.

**HD haptics**: a DualSense (or DualSense Edge) plugged in over USB is also a four-channel sound card; channels 3 and 4 drive its motors. The console's HD haptics stream (PCM, two channels) is played there as is, resampled to 48 kHz, and no longer goes through simple rumble. Over Bluetooth, or without the controller's sound card: simple rumble taken from the same stream, as before.

**DualSense intensities**: per console, in the settings (“Controls”): “Rumble” and “Adaptive triggers”, from 0% (off) to 100% (default), in steps of 10; at once during a game. Rumble (HD and simple) is multiplied by its intensity; triggers: the strength of each effect (resistance, trigger vibration) is multiplied by its own, 0% leaves the trigger free.

**The console's keyboard**: when the console opens its on-screen keyboard (search, message; `KEYBOARD_OPEN`), the device's keyboard types into its field: a banner says so, keys give text (in the keyboard's layout) instead of holding the controller; the whole text is sent on each change (`KEYBOARD_TEXT_CHANGE_REQ`, increasing counter); Return accepts (`KEYBOARD_CLOSE_REQ`, accepted), Escape cancels (rejected); the console closes its keyboard (`KEYBOARD_CLOSE_REMOTE`): the banner goes away, the keyboard holds the controller again. Text sent back by the console (`KEYBOARD_TEXT_CHANGE_RES`) replaces the app's. With a controller plugged in too: the same, the controller stays with the console. Return and Escape also remove the banner at once, without waiting for the console. The keyboard is enabled at the start of the session (`KEYBOARD_ENABLE` then `KEYBOARD_ENABLE_TOGGLE`, as chiaki-ng has it written but switched off); not yet tried with the console.

## Upscaling

The console sends 1080p at most (the PS5 Pro refuses 1440p and 4K). The app upscales the picture to the real pixels of the screen, or of the window (4K, 5K, 6K…), in three passes on the graphics chip: YCbCr → RGB at the received size, MetalFX “spatial” (AMD's FSR 1, frame by frame) to the shown size, then sharpening (FidelityFX RCAS, MIT licence) while copying into the screen's buffer. HDR included: in PQ, MetalFX works in perceptual mode; in linear light, in HDR mode. Consoles only: a computer already sends at the screen's size (`apps/macos/KyberKit/Upscaler.swift`).

Measured on 2026-11-03 on an M4 Max: MetalFX, 0.4 ms to 4K, 0.7 ms to 5K and 1 ms to 6K, in SDR as in HDR. The cost is measured during the game, over 120 frames: above a 2 ms median, upscaling turns off for that game, and the log says so. VideoToolbox super-resolution was ruled out: its “low latency” version refuses more than 1280 pixels per side, and the other only does ×4 (8K) and takes 413 ms per frame at 1080p.

## Open points

- **Resolution**: PS5 Remote Play goes no further than 1080p (1440p and 4K refused by the PS5 Pro, “AvCap failed to initialize video”; the author, 2026-11-03: the PS5 can't send them). Dropped; the app's upscaling makes up for it.
- **Relay without a Station**: the “console alone, no computer on at home” case only works over a VPN.
