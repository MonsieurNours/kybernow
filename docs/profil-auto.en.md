# “Auto” profile and adaptive bitrate

English · [Français](profil-auto.md)

Texts and screens: [link-ui.md](link-ui.md), “Profil Auto et débit adaptatif”. Adaptive bitrate logic: [debit-automatique.en.md](debit-automatique.en.md).

## What changes

- **Adaptive bitrate** becomes a switch in the “Bitrate” card (on by default), for every bitrate: profiles as well as a bitrate set by hand. Off, the bitrate never moves. Settings field `adaptive` (absent: on).
- **PyroWave by cable** is only used by **Max** and **Auto**; Game and Desktop stay on HEVC (`profile::pyrowave_format`).
- **Auto**, a fourth profile (`auto`): a test of the conditions, run only on demand (when choosing Auto, or “Run the test again”), which suggests settings; nothing changes before “Apply”. The app keeps the result for that computer.

## The test

The core runs it (`apps/core/src/autotest.rs`) and gives the apps its progress, step by step. Each step is ticked, or skipped with its reason. About 10 seconds.

| Step | Key | What is measured |
|---|---|---|
| 1. Connection | `link` | cable, Wi-Fi or relay (given by the app and by the console's path) |
| 2. Latency at rest | `latency` | 20 round trips to the Station (`GET /api/stream/ping`) over 2 s: lowest, median, jitter (gap between the 9th and 1st decile) |
| 3. Bitrate to this device | `bandwidth` | a 4 s burst sent by the Station (`GET /api/stream/probe`), read as fast as possible: bitrate of each quarter second, median of the second half; meanwhile, round trips (`/api/stream/ping`) give the latency under load |
| 4. Decoding on this device | `decode` | embedded video samples (HEVC, HEVC 4:4:4, AV1, H.264, PyroWave 4:2:0 and 4:4:4), in 4K, decoded as fast as possible by the app's decoder: frames per second of each |
| 5. The computer's encoder | `encoder` | the Station's encoder (`GET /api/stream/encoder`, see “The computer's encoder”) tried at the chosen format, resolution, refresh rate and bitrate; refused: 50 Mbit/s less, down to the first bitrate accepted. Skipped with PyroWave, with an older Station (“Update {name} to try its encoder.”) or when the Station couldn't try it |

**Older Station** (no `/api/stream/probe`, 404 reply): step 3 is skipped, “Update {name} to measure the bitrate.”, and the bitrate follows the connection type (cable 300 Mbit/s, Wi-Fi 80, relay 25, at 4K 60 Hz).

## The choices

The core decides (`autotest::decide`), in this order, and gives the reason for each choice:

1. **Budget**: the measured bitrate, minus a margin (cable 40 %, Wi-Fi and relay 50 %), minus error correction (30 %).
2. **Format**: PyroWave if the computer offers it, the device decodes it fast enough, it is connected by cable (not through the relay) and the budget covers its bitrate at the display's resolution; otherwise HEVC (AV1 if the device doesn't decode HEVC, H.264 last).
3. **Resolution and refresh rate**: the device display's, as long as decoding keeps up with the refresh rate with a 20 % margin; otherwise the rate goes down (120 → 60 Hz), then the resolution (4K → 1440p → 1080p).
4. **Sharp text (4:4:4)**: if the device decodes it fast enough and the budget covers a third more bitrate (with PyroWave: half).
5. **Bitrate**: the budget, at most Max's, at most the bitrate the computer's encoder accepted (step 5; with PyroWave, at most its ceiling on the computer) and at least 10 Mbit/s; kept calibrated on 4K at 60 Hz like the profiles'.
6. **Transport**: Reliable if jitter exceeds 10 ms or latency under load rises by more than 50 ms (a network that holds packets back); otherwise Fast.
7. **Adaptive bitrate**: on.

HDR, frame display, sound and controls don't change: they are the user's choices.

## Auto's settings

`AutoResult` (JSON, kept by the app for that computer): the date, the test's network (`network`: the name the app gives, or `relay`), the measurements, and the chosen settings (`codec`, `yuv444`, `width`, `height`, `refresh`, `mbps`, `reliable`; `AutoResult::settings(hdr, wired)` gives the `StreamSettings` with `profile: "auto"`). In a session, Auto behaves like a profile: the bitrate follows pixels per second, adaptive bitrate acts. `codec: "pyrowave"` means PyroWave by cable, `mbps` staying HEVC's (used on Wi-Fi or through the relay).

**Another network** than the test's: the result stays, and a discreet banner says so at the start of the session, with “Run the test again” (opens the test panel).

No test yet (Auto never applied): choosing Auto opens the test; “Cancel” keeps the previous profile.

## Station side

`GET /api/stream/probe?ms=4000` (Admin, same access as `/api/stream/info`, over the network or the relay): random bytes, sent as fast as the connection takes them, for `ms` milliseconds (at most 6,000), at most 2 Gbit/s; one burst at a time (429 otherwise). `GET /api/stream/ping`: an immediate empty reply. The burst goes through HTTPS (TCP), not Kyber's QUIC: it measures the path, not the encoder.

## The computer's encoder

A hardware encoder doesn't accept just any bitrate: an RTX 3080 Ti's NVENC refused HEVC 4:2:0 at 593 Mbit/s in 4K 160 Hz (“picture pipeline error” at every start) and held 290. Every card has its own limits, unknown in advance: we look for them.

- **Station probe**: `GET /api/stream/encoder?codec=hevc&width=3840&height=2160&fps=160&mbps=593&yuv444=0&hdr=0` (Admin, same access as `/api/stream/probe`, one probe at a time, 429 otherwise) → `{"ok": true}`, `{"ok": false, "error": "…"}` (refused), or `{"ok": null, "error": "…"}` (couldn't try: tool missing, card busy). Admin runs `kybernow-encoder-check` (next to it: `kybernow-encoder-check.exe` on Windows, inside the app on Mac, in the package on Linux) with the encoder set for Kyber (`video.encoder`: NVENC, AMF, QSV on Windows; VideoToolbox on Mac; VA-API, NVENC on Linux): it opens FFmpeg's encoder with Kyber's options and encodes a few frames; x264 is always accepted. `info` announces it (`encoder_check: true`); an older Station answers 404.
- **In the test** (step 5): the core tries the chosen bitrate (per screen, at the chosen resolution and refresh rate), then 50 Mbit/s less at each refusal, 50 Mbit/s at the lowest. The result (`measures.encoder`: format, 4:4:4, resolution, refresh rate, `mbps`, `limited` when a bitrate was refused) bounds the chosen bitrate; bitrate reason: “{name} encodes up to {n} Mbit/s”. Step measure: “up to {n} Mbit/s in {format}”, or “{n} Mbit/s accepted in {format}” when the first try passes.
- **Ceiling per computer**: the app keeps for each computer the highest known bitrate of its encoder (`encoder_max`, Mbit/s per screen, with the computer's settings) and gives it to the session (`StreamSettings::encoder_max`, JSON `encoder_max`), which never asks for more, whatever the profile or a bitrate set by hand. Updated by “Apply” (`AutoResult::settings`: `encoder_max` when the step found a limit; `encoder_ok` when the bitrate tried passed first time, and then a lower kept ceiling is removed) and by the session (below). No known ceiling: none.
- **In a session** (all Stations, older ones included): a picture refused by the computer before any frame (an error `StreamerPipelineEvent`) brings the bitrate down by 50 Mbit/s per screen and restarts the stream, down to 50 Mbit/s; once the picture comes, the session tells the app (`Notice::EncoderCeiling { mbps }`), which keeps it as the computer's ceiling. Below 50: the usual fallback (4:2:0), then three restarts at most and the “no picture” failure. Statistics, target bitrate: “{n} Mbit/s · encoder ceiling”.

## Decoding

The samples live in the core (`apps/core/samples/`, made by `apps/core/scripts/samples.sh`): 3840 × 2160, one key frame then 19 frames (HEVC, AV1, H.264), one intra frame (PyroWave). The core gives them as packets like Kyber's (`autotest::samples_for(caps)`); the app passes them to its decoder, as fast as it takes them, for at most 1 s per sample (looping), and returns the decoded frames per second (`SampleDecoder`). A format the app doesn't decode is skipped without a try.
