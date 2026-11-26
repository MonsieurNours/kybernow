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

Kyber 0.29 has no upstream sound at all: only a “microphone” type exists in the Station's capture (txproto), unused. Everything below is ours (patches 0080).

## The sound

Opus, mono, 48 kHz, 10 ms frames (480 samples), voice mode (`OPUS_APPLICATION_VOIP`), 32 kbit/s, Opus's in-band forward error correction (FEC, 10 % expected loss), no DTX. Encoded by the core for every app (`apps/core/src/micro.rs`): the app only captures and hands its samples, at any rate (the core brings them to 48 kHz).

The transport is the one of the computer's sound, the other way round: one more Kymux audio endpoint on the session's connection, with the session's audio protocol (`UnreliableFec`). The downstream sound is untouched; if the downstream sound's redundancy changes (transport work), the upstream sound will take the same.

## Protocol (patches 0080)

**Request** (`POST /kymux/start_session`, `kyctl-common`): `uplink: { microphone: <id> }`, the id of the audio endpoint the device registered just before on the Kymux connection. Absent: nothing offered (as before). An older Station ignores the field.

**Answer**: `uplink: { microphone: true }` when the computer takes it, otherwise `microphone: false` and `microphone_refused`:

| Reason | Meaning | What the app says |
|---|---|---|
| `not_allowed` | KyberNow Share guest without the right | “The host isn't letting guests speak.” |
| `no_device` | no virtual microphone on the computer (VB-CABLE not installed, KyberNow.driver missing, PipeWire unreachable) | “The computer has no virtual microphone: see Admin › Picture and sound.” |
| `unsupported` | the Station can't take it (helper missing, WebSocket transport) | “This computer can't receive the microphone.” |
| `old_station` (said by kyclient: answer without `uplink`) | Station from before the microphone | “Update the Station to send your microphone.” |

Packets: a codec packet first (`opus`, 480 samples per packet), then one packet per frame with the capture time in microseconds. An empty packet means “the device muted its microphone”: the computer plays silence at once, without waiting for its buffer to run dry. Nothing waits: a packet the connection can't take is dropped (10 queued at most, 100 ms).

**Right** (kycontroller): the host always; a share guest or the borrowing friend only with `kn_mic: true` in their Kyber token. Admin sets it for a loan, and for guests when the host lets them speak. During a session, `POST /kybernow/role` accepts `mic` (with the same revision as the role): with the right taken back, packets are dropped and the virtual microphone plays silence; given back, they go through again.

**kyclient** (`uplink.rs`): `Uplink`, kept by the core for the whole session (it outlives the picture's restarts) and given to each `StreamingSessionConfig` (`uplink`). `set_offer(true)`: offered at the next start of the picture; `set_codec`, `send(packet, time)` (`false` while the computer doesn't take it), `state()`: `Idle`, `Offered`, `Open`, `Refused(reason)`.

**kycontroller** (`uplink.rs`): checks the right, asks the Station's helper whether the virtual microphone is there (`kybernow-micro --check`, 3 s at most), connects to the device's endpoint, starts the helper for the session and passes it every packet on its standard input, one record per packet (2-byte length, then `C` codec, samples and channels; `M` time and packet; `S` silence). The input closes with the session; the helper is stopped a second later.

## The Station's helper: `kybernow-micro`

`server/micro` (Rust), installed next to kycontroller (`KYBERNOW_MICRO` for another one). One per session sending its microphone; several (the host and their guests) mix in the virtual microphone, the system does the mixing.

- **Decoding**: libopus; 20 ms jitter buffer aimed for, 60 ms at most (beyond, the oldest is dropped); a lost frame is rebuilt from the next packet's error correction when it is there, otherwise concealed by Opus (PLC); silence when nothing has come for 60 ms.
- **Default microphone**: at the first packet, if the computer's default microphone isn't the virtual one already, it notes it and makes the virtual microphone the default; when leaving, it puts the previous one back if it hasn't changed meanwhile. A second helper (a guest) finds the virtual microphone already default and touches nothing.
- **Windows**: the sound is played with WASAPI (shared, 48 kHz) into VB-CABLE's “CABLE Input”; the computer's apps hear it in “CABLE Output”. `--check`: the “CABLE Input” output exists and is active.
- **Mac**: played with Core Audio into the hidden “KyberNow Micro (entrée)” input of the KyberNow.driver; apps hear it in **“Micro KyberNow”**, an input-only device (the driver keeps the ring between both, like its output and loopback for the session's sound).
- **Linux**: if needed, the helper creates with `pactl` a hidden null sink (`kybernow_micro_entree`) and a source taking its monitor, **“Micro KyberNow”** (`kybernow_micro`, `module-remap-source`), then plays into it with `pacat` (PipeWire, through pipewire-pulse). `--check`: `pactl info` answers.

## VB-CABLE (Windows)

VB-Audio Software's driver, signed and proven; we can't write a sound driver of our own without Microsoft's signature (pending, see the “Authenticode” memory). VB-CABLE is **donationware**: VB-Audio allows shipping it as is with another app, silent install included, provided the user can tell it is a VB-Audio product and support it. Hence, wherever we show it (installer, Admin › Picture and sound, this page): “VB-CABLE, by VB-Audio Software, is donationware: vb-cable.com.” The pack is downloaded by `server/build-windows.sh` (`VBCABLE_Driver_Pack`, checksum verified) and the Station's installer installs it when missing (`VBCABLE_Setup_x64.exe -i -h`). Never rename its devices.

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

Written on 26/11/2026, branch `claude/micro-5db32e`:

- patches 0080 (kycontroller, kyclient), built and tested (`cargo test -p kycontroller uplink`, `cargo test -p kyclient --features kymux uplink`);
- see the following sections as commits come; what isn't written is in “Still to do”.

### Still to do

- Try it end to end: no session has sent a microphone yet.
