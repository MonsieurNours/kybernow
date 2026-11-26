# The device's microphone, sent to the computer

English · [Français](micro.md)

Talking in a game, a call or a video meeting running on the computer, with the microphone of the device connected to it. The wording and the interface, for every app, are in [link-ui.en.md](link-ui.en.md), “Microphone”. Here: the protocol, what each part does and where testing stands.

The author's choices (26/11/2026):

- off for the first session; « Micro » in the session bar (or menu) turns it on; the choice is kept for each computer;
- the computer receives the sound in a virtual microphone: **VB-CABLE** on Windows (installed with the Station), the **« Micro KyberNow »** device of the KyberNow.driver on the Mac, a **PipeWire source** on Linux; while it is sent, it becomes the computer's default microphone;
- lending the PC: the friend talks like the owner; KyberNow Share guests: only if the host lets them speak (off by default);
- echo and noise reduction: the device system's own, on by default, can be turned off;
- every app with a microphone: Mac, iPhone and iPad, Android, Windows, Linux (desktop, Steam Deck, Raspberry Pi with a USB microphone), web client; neither the Apple TV (its microphone is Siri's) nor LG TVs;
- a « Micro » setting in the App settings (the system's or a chosen microphone) and an indicator in the session bar while it is sent.

Kyber 0.29 has no upstream sound at all: only a “microphone” type exists in the Station's capture (txproto), unused.

## The sound

Opus, mono, 48 kHz, 10 ms frames (480 samples), voice mode (`OPUS_APPLICATION_VOIP`), 32 kbit/s, Opus's in-band forward error correction (FEC, 10 % expected loss), no DTX. Encoded by the core for every app (`apps/core/src/micro.rs`): the app only captures and hands its samples, at any rate (the core brings them to 48 kHz).

The transport is the camera's and microphone's **common upstream channel** (patches 0085, written with the webcam session: [webcam.en.md](webcam.en.md), “The upstream stream”): one kyproto data endpoint per kind, a reliable QUIC stream of its own, apart from the picture, sound and inputs. The downstream sound is untouched. A reliable stream waits for what gets lost: with 10 ms frames, a loss on Wi-Fi delays by one round trip; the Station's buffer (60 ms at most, the rest dropped) absorbs it. To be measured; if it is too much, an unreliable path for the microphone alone (the patch first written here did it over Kymux's audio protocol, `UnreliableFec`).

## Protocol (patch 0085, kind `microphone`)

- **Start of the picture**: the core asks for `uplink: ["camera", "microphone"]` (the microphone as soon as the app has one, even off, and for a guest too); the Station returns a data endpoint for the microphone when `kybernow-micro` sits next to kycontroller. No stream: “Update the Station to send your microphone.” for the host, “The host isn't letting guests speak.” for a guest or the borrowing friend.
- **Turning on**: `START` + `{"codec": "opus", "rate": 48000, "channels": 1, "frame": 480, "bitrate": 32000, "device": "…"}`; kycontroller starts `kybernow-micro --uplink` and answers `{"state": "started"}`. Then one `MEDIA` per frame: flags (0), capture time in µs, Opus packet. **Turning off**: `PAUSE` (the helper plays silence at once, stays ready). Picture restarted: `START` goes again on the new stream.
- **Answers** (`STATUS`, JSON): `started` → « Micro » lit; `failed` with `reason` `no_device` (written by the helper when the virtual microphone is missing) → “The computer has no virtual microphone: see Admin › Picture and sound.”; any other refusal → “This computer can't receive the microphone.”
- **Right and count**: 0085 refuses guests and takes one device at a time per kind. For the microphone, the author's choices ask for the borrowing friend and the guests the host lets speak (`kn_mic` in their token, already set by Admin; `POST /kybernow/role` with `mic` during a session), and several microphones at once (the system mixes them). Proposed to the main session on 26/11: a patch of ours applied after 0085 (`0081-kycontroller-micro-droits`), not written yet.

## The Station's helper: `kybernow-micro`

`server/micro` (Rust), installed next to kycontroller (`KYBERNOW_MICRO` for another one). One per session sending its microphone; several (the host and their guests) mix in the virtual microphone, the system does the mixing.

- **Decoding**: libopus; 20 ms jitter buffer aimed for, 60 ms at most (beyond, the oldest is dropped); a lost frame is rebuilt from the next packet's error correction when it is there, otherwise concealed by Opus (PLC); silence when nothing has come for 60 ms.
- **Default microphone**: at the first packet, if the computer's default microphone isn't the virtual one already, it notes it and makes the virtual microphone the default; when leaving, it puts the previous one back if it hasn't changed meanwhile. A second helper (a guest) finds the virtual microphone already default and touches nothing.
- **Windows**: the sound is played with WASAPI (shared, 48 kHz) into VB-CABLE's “CABLE Input”; the computer's apps hear it in “CABLE Output”. `--check`: the “CABLE Input” output exists and is active.
- **Mac**: played with Core Audio into the hidden “KyberNow Micro (entrée)” input of the KyberNow.driver; apps hear it in **“Micro KyberNow”**, an input-only device (the driver keeps the ring between both, like its output and loopback for the session's sound).
- **Linux**: if needed, the helper creates with `pactl` a hidden null sink (`kybernow_micro_entree`) and a source taking its monitor, **“Micro KyberNow”** (`kybernow_micro`, `module-remap-source`), then plays into it with `pacat` (PipeWire, through pipewire-pulse). `--check`: `pactl info` answers.

## VB-CABLE (Windows)

VB-Audio Software's driver, signed and proven; we can't write a sound driver of our own without Microsoft's signature (pending, see the “Authenticode” memory). VB-CABLE is **donationware**: VB-Audio allows shipping it as is with another app, silent install included, provided the user can tell it is a VB-Audio product and support it. Hence, wherever we show it (installer, Admin › Picture and sound, this page): “VB-CABLE, by VB-Audio Software, is donationware: vb-cable.com.” VB-Audio's pack (`VBCABLE_Driver_Pack*.zip`, extracted) goes into `third_party/payload/vbcable/` before building the installer, which installs it when missing (`VBCABLE_Setup_x64.exe -i -h`, after `kybernow-micro --check`); without it, the step is skipped. Never rename its devices.

## The apps

Each app captures its microphone its own way and hands its samples to the core (`Session::push_microphone`, mono floats, any rate). The core encodes, sends, and tells the app the state (`Platform::on_microphone`): `Off`, `On`, `Refused(reason)`, `Waiting` (offered, no answer yet).

| App | Capture | Echo and noise |
|---|---|---|
| Mac | AVAudioEngine (input node) | Apple's voice processing (`setVoiceProcessingEnabled`) |
| iPhone, iPad | AVAudioEngine, `playAndRecord` session in `voiceChat` mode while the microphone is on | Apple's voice processing |
| Android | `AudioRecord`, `VOICE_COMMUNICATION` source | `AcousticEchoCanceler` and `NoiseSuppressor` when the device has them |
| Windows | WASAPI, “Communications” category | the system's processing when the microphone's driver has some |
| Linux, Deck, Pi | SDL2 (capture), “communication” role | PipeWire's echo module if loaded; otherwise none |
| Web client | `getUserMedia`, Opus through WebCodecs | the browser's `echoCancellation`, `noiseSuppression` |

App settings: « Micro » (« Celui du système » or one of the device's microphones) and « Réduire l'écho et le bruit » (on). Computer settings › Sound: « Micro », the same switch as the session's, kept for this computer.

## Where things stand

Written on 26/11/2026, branch `claude/micro-5db32e`; **nothing has been tried end to end yet** (no microphone has reached a computer).

| Part | State |
|---|---|
| Transport | common upstream channel 0085 (webcam session), merged into the branch; “microphone” asked at every start |
| Right and several microphones (friend, allowed guests, no `busy`) | written and tested on 0085 (`micro-droits-sur-0085.diff`, shared notes folder), to be put into 0085 by the main session |
| `kybernow-micro` helper | built for Mac, Windows, Linux; 7 tests (packets, losses, buffer); `--check` checked on this Mac (3: older driver) |
| Mac driver “Micro KyberNow” | built; not installed (administrator) |
| VB-CABLE | installer step and Admin card written; the pack goes into `third_party/payload/vbcable/` |
| Admin | `kn_mic`, “Let guests speak”, “Virtual microphone” card; 368 tests |
| Core | `micro.rs`; 6 tests |
| Apps | Android (`22acbeea`), Windows (`eff90dba`), Linux (`1cd9692b`) built; Mac, iPhone, iPad: see below; web client: the page is ready (`d9c3dc4d`) but hidden, its engine (wasm, its own copy of Kyber) doesn't have the upstream channel yet |

### Still to do

- Put the microphone's right into 0085.
- Web client: the upstream channel in the engine (web copy of kyclient and kyber-web), then `engineSendsMicrophone()`.
- Tests: microphone received in “CABLE Output” (Windows, the helper runs as SYSTEM: WASAPI and the default microphone from session 0 are to be checked), “Micro KyberNow” (Mac, once the driver is installed), PipeWire (VM); echo with and without voice processing; Wi-Fi loss on the reliable stream; guests and lending.
- `scripts/release.sh`: the Windows overlay's fingerprint (`kyber-overlay.stamp`) doesn't count `server/micro`; `server/micro/Cargo.toml`'s version isn't aligned.
