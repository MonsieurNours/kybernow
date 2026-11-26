# Measurement bench

English · [Français](banc-de-mesure.md)

Development tools, outside the apps, to put numbers on a KyberNow version and compare it with another: before and after an optimisation, with the same instruments and the same conditions. Everything lives in [tools/banc](../tools/banc):

| Tool | What it measures | Where it runs |
|---|---|---|
| `banc.py ecran` (+ `sonde.html`, `lecteur.swift`) | latency from the Station's screen to the device's screen | probe: a browser on the Station; reader: the client Mac |
| `banc bride` | nothing: it plays a network (bandwidth, delay, jitter, loss) between the device and the Station | any machine, no administrator rights |
| `banc echo` + `banc enregistre` | a real link (Wi-Fi, 4G, relay), recorded as a replayable trace | both ends of the link |
| `banc rejeu-debit`, `banc.py debit` | the apps' automatic bitrate replayed on traces, without a network | anywhere, in seconds |
| `banc.py rapport` | an HTML page comparing measurements | anywhere |

The time of each stage measured by the session itself (`kyclient --metrics`) stays with [tools/latency](../tools/latency); `analyze_metrics.py --json` hands it to the report.

## Building

```bash
cd tools/banc && cargo build --release
```

`banc` is a separate crate (its own workspace) that builds without `third_party`. `banc.py` builds the Swift reader on first use (`swiftc`, Xcode's tools). Python 3.9 or later, nothing to install.

## Screen-to-screen latency

The time between the Station showing a frame and the device showing the same frame. That is what one feels; the session statistics only give part of it (neither the Station's nor the device's screen composition).

**How**:

1. **The probe** (`sonde.html`), opened full screen on the Station in any browser, draws a strip of 16 blocks at the top left: two markers (yellow, blue), an 11-bit Gray-coded frame counter, a parity bit, and the markers reversed. It changes with every screen frame and records the time of each frame (start of rendering, page clock).
2. It sends these times to the bench over HTTP, with the offset between its clock and the bench's: 25 round trips every 5 s, keeping the shortest. Precision is half that round trip (± 0.5 ms on loopback, ± 1 to 2 ms over Wi-Fi), shown with each measurement.
3. **The reader** (`lecteur.swift`), on the client Mac, looks for the strip on its screens (in the KyberNow Link window), then captures that screen with ScreenCaptureKit as fast as it composes, up to 240 frames per second. For each capture it reads the counter and records when the screen showed it.
4. The bench pairs them: a frame's latency is the time it first appears on the Mac minus the time the probe drew it. Thanks to the Gray code, a frame torn between two (wrong parity) still reads as one of the two: it is counted separately. A reading that neither its previous nor its next neighbour follows closely (0 to 16 frames) is a misread and is rejected.

**What a number includes**: the browser's rendering and the Station's screen composition, capture, encoding, the network, decoding, queueing and display in the app, the Mac's screen composition and its capture. **What it leaves out**: the Mac panel's response time (constant for a given screen).

**The floor**: the probe opened on the Mac itself, with no KyberNow in between, measures the instrument's own delay (browser rendering, composition, capture). Only compare measurements with the same floor, and subtract it mentally. A measurement is only as precise as one refresh of the Station's screen.

**Measuring**:

```bash
python3 tools/banc/banc.py ecran --plancher --etiquette floor        # the probe opens right here
python3 tools/banc/banc.py ecran --etiquette main --note "HEVC Game, cable, 4K 60" --duree 30
```

For the second one, the bench prints the probe's address (`http://<Mac address>:8765/sonde`): open it full screen on the Station (F11); as soon as it reports in, bring the KyberNow Link session to the front, full screen or in a window large enough for a block to keep at least 8 pixels (`?bloc=60` on the probe's address for larger blocks). The reader starts 3 s later (`--delai`) and captures for `--duree` seconds. The Mac must allow screen recording for the app that runs the bench (System Settings › Privacy & Security › Screen Recording). The Station's firewall must let its browser reach the Mac on port 8765 (`--port`).

Each measurement goes to `tools/banc/mesures/<date>-<label>/` (outside the repository): `resultat.json` (statistics and latencies), and the raw data (`sonde.json`, `lectures.jsonl`) that `banc.py analyse <folder>` recomputes.

**Result**: median, p95, p99, mean, maximum; Station frames reaching the screen per second; share of torn frames; rejected readings; clock precision.

The reader is Mac only. On another device (Apple TV, TV, phone), filming both screens at 240 frames per second remains the way ([prototype A](archives/prototype-A.en.md), §3.2).

## The shaper: a played network

`banc bride` is a relay: the app connects to it as it would to the Station; it forwards UDP (Kyber's video, sound and input) while playing a trace in each direction, and TCP (Admin) untouched. It runs in user space: no `sudo`, no firewall change, on Mac, Linux or Windows.

Each direction is a link: random loss, then a limited bandwidth behind a bounded queue (`file_ms`, 100 ms by default: a packet that would wait longer is dropped, as in a home router), then a fixed delay and jitter (a random delay for each packet, without reordering them, unless `--desordre`). **Down** (`descente`) goes from the Station to the device, **up** (`montee`) from the device to the Station.

```bash
tools/banc/target/release/banc profils                    # built-in profiles
tools/banc/target/release/banc trace chute-20 --rtt 20    # what a profile plays
tools/banc/target/release/banc bride --vers 192.168.1.100 --udp 8080 --tcp 8080,37462 \
    --profil 4g --rtt 20 --journal bride.jsonl
```

- `--udp`: Kyber's port (8080 by default; the app's log writes it, "console : … port Some(…)"); `--tcp`: the same and Admin's. `40241:8080` listens on 40241 and reaches 8080, for a Station on the same machine.
- Then, in the app, add the computer by the address of the machine running the shaper. The shaper listens on all addresses (`--ecoute` to pick one).
- **Mac or Linux Station**: Admin may give the app a "preferred" address for the video (app log: "image par l'adresse préférée de l'ordinateur"), which bypasses the shaper. In that case, go through `tools/latency/run_client.sh <shaper address>`, which reaches Kyber directly. Windows Station: no preferred address.
- `--boucle` replays the trace endlessly; otherwise its last step stays. `--duree S` stops the shaper. Every second it writes what passed and what was lost (randomly, or by the queue) in each direction, on screen and to `--journal`.
- Precision is around half a millisecond (sending waits in one-millisecond slices, then spins: macOS wakes long waits late).

| Profile | What it plays |
|---|---|
| `propre` | nothing (the `--rtt` delay only) |
| `capacite-20` | down at 20 Mbit/s |
| `chute-20` | down at 100 Mbit/s, 20 Mbit/s from 30 to 60 s, then 100 Mbit/s |
| `capacite-3-1` | down at 3 Mbit/s, 1 Mbit/s from 60 s |
| `pertes-2` | 2 % loss down, 5 % from 60 s |
| `rafales` | everything down lost for 50 ms every 2 s |
| `gigue-15` | down delayed by 0 to 15 ms at random, packet by packet |
| `wifi` | busy Wi-Fi: 4 ms jitter, 0.2 % loss, a 60 ms spike for 300 ms every 7 s |
| `4g` | mobile network: 15 to 40 Mbit/s changing every second (always the same sequence), +25 ms, 8 ms jitter, 0.5 % loss |
| `retour-coupe` | everything up lost for 1 s at 30 s: the device's acknowledgements stop |

Profiles last 2 minutes; `--rtt` adds a base round trip, half in each direction. Reference conditions: `--rtt 10` on the local network, `--rtt 40` for the relay.

### Trace format

JSON Lines: an optional header line, then one step per line, which holds until the next; a missing direction is not shaped.

```json
{"banc":"trace","nom":"salon","source":"banc enregistre …","duree_ms":60000}
{"t_ms":0,"descente":{"debit_mbps":0,"retard_ms":3.1,"gigue_ms":2.4,"pertes_pct":0,"file_ms":100},"montee":{"retard_ms":3.1}}
{"t_ms":200,"descente":{"debit_mbps":38.5,"retard_ms":3.1,"gigue_ms":0,"pertes_pct":0,"file_ms":100}}
```

`debit_mbps` 0: unlimited.

## Recording a real link

To replay at home the living-room Wi-Fi, a 4G network or the relay path: `banc echo` runs at the other end, `banc enregistre` asks it for a steady stream towards the device, like a session's video, and turns it into a trace.

```bash
banc echo                                              # at the other end (UDP 7979)
banc enregistre --echo 192.168.1.100:7979 --debit 40 --duree 120 --nom salon --sortie salon.jsonl
```

- Every `--pas` ms (200 by default): the bandwidth received, the queueing delay (arrival minus send, relative to its minimum: no shared clock needed), its spread (9th minus 1st decile, which becomes the jitter) and lost packets. The base delay is half the shortest round trip of probes sent every 20 ms.
- A step where the queue grows (over 10 ms) or that loses over 1 % is **saturated**: the link carries no more, its received bandwidth becomes the step's capacity, and the queue comes from that (the shaper will rebuild it). Otherwise the link is not limited at that rate: `--debit` must exceed what you want to learn (40 to 80 Mbit/s for Wi-Fi, 20 to 40 for 4G).
- Lost probes count as upstream loss (above 20 % of a step, and not when the downstream saturates: the downstream queue dropped them on the way back).
- Recording loads the link: do not run it on someone's connection without saying so.

**Trial** (26/11/2026, Mac loopback): with the shaper playing `chute-20` with `--rtt 20`, a recording at 50 Mbit/s finds "unlimited, +10 ms" until 29.5 s, "20 Mbit/s, +10 ms" from 30 to 59.5 s, then unlimited, with the probes' round trip at 20 ms (120 ms with a full queue).

## Replaying the automatic bitrate without a network

`banc rejeu-debit` runs a trace through a simple link model (a queue in front of the bandwidth, loss, delay), second by second, and hands each second to the apps' real computation, [apps/core/src/adaptive.rs](../apps/core/src/adaptive.rs), taken as is. Two minutes of trace replay in a few milliseconds: enough to tune thresholds before trying on the shaper and for real.

- The model advances in 10 ms steps; a second's round trip is taken at an instant that changes from one second to the next (Kyber gives one every few seconds); a picture hole is counted when a step loses more than 10 % of its packets (beyond error correction).
- It models neither the encoder (the requested bitrate is the bitrate sent) nor the upstream (`retour-coupe` can only be judged on the shaper).
- Result per trace: mean bandwidth received; **utilisation** (share of the useful bandwidth available, capacity or the profile's bitrate, actually received); seconds with the queue over 15 ms; seconds with holes; lost packets; bitrate changes; round trip p50 and p95.

```bash
python3 tools/banc/banc.py debit main .                 # main against the working tree, all profiles
python3 tools/banc/banc.py debit main claude/opti-pertes-aa4ebe --trace pertes-2 --trace salon.jsonl
```

`banc.py debit` takes `apps/core/src/adaptive.rs` from each git version ("." : the working tree), builds it with the current branch's bench (the replay's rules stay the same), and writes the report. Profile bitrate: `--besoin` (60 Mbit/s); base round trip: `--rtt` (10 ms).

**When adaptive.rs changes**: the replay takes the file as is and replaces what it borrows from the core with small modules (`latency::Second` reduced to `rtt_ms`, `lost_packets`, `bytes`, `holes`; `i18n`). If the computation starts reading another field of `Second`, add that field to `tools/banc/src/main.rs` (and its value to `simu::Mesure`) in the same commit; a version that no longer builds with the bench stops it with a message.

## The report

```bash
python3 tools/banc/banc.py rapport tools/banc/references/2026-11-26-main/plancher.json mesures/…-before mesures/…-after -o report.html
python3 tools/latency/analyze_metrics.py metrics.json --json stages-after.json --etiquette after
```

A self-contained page (no dependency, light and dark), with the first measurement as the reference: for screen-to-screen, a table (median, p95, p99, mean, maximum, frames per second, torn frames) with each version's difference, green when it improves, and the latency distribution; for the session's stages (`analyze_metrics.py --json`), median and p95 of each stage; for the automatic bitrate, a table per trace and the curve of each version's requested bitrate over the link's capacity.

## Reference measurement: main on 26/11/2026

In [tools/banc/references/2026-11-26-main](../tools/banc/references/2026-11-26-main): `plancher.json` and `debit.json` (main 36c25c24, KyberNow 0.11.4). Screen-to-screen against a Station is for the author to run (commands above): it needs the probe open on the Station and the session in front.

**Floor** (author's Mac, 5K screen at 60 Hz, default browser): median 29.2 ms, p95 30.5, p99 30.9; 59.8 frames per second; none torn; precision ± 0.5 ms.

**Automatic bitrate** (60 Mbit/s profile, `--rtt 10`):

| Trace | Received (Mbit/s) | Utilisation | Queue > 15 ms | Holes | Lost | Changes | Round trip p95 |
|---|---|---|---|---|---|---|---|
| propre | 60.0 | 100 % | 0 s | 0 s | 0 | 0 | 10 ms |
| capacite-20 | 17.6 | 88 % | 21 s | 3 s | 7,458 | 32 | 110 ms |
| chute-20 | 30.6 | 61 % | 9 s | 3 s | 7,410 | 25 | 37 ms |
| capacite-3-1 | 2.0 | 100 % | 120 s | 120 s | 52,961 | 7 | 110 ms |
| pertes-2 | 24.9 | 41 % | 0 s | 0 s | 7,963 | 7 | 10 ms |
| rafales | 58.5 | 97 % | 0 s | 60 s | 18,750 | 0 | 10 ms |
| gigue-15 | 60.0 | 100 % | 0 s | 0 s | 0 | 0 | 25 ms |
| wifi | 59.9 | 100 % | 0 s | 0 s | 1,575 | 0 | 22 ms |
| 4g | 16.8 | 68 % | 18 s | 10 s | 8,067 | 33 | 235 ms |
| retour-coupe | 60.0 | 100 % | 0 s | 0 s | 0 | 0 | 10 ms |

What the replay shows about main, to be confirmed on the shaper:

- **Loss without saturation** (`pertes-2`): 2 % random loss lowers the bitrate one step (42 Mbit/s), 5 % down to the floor (5 Mbit/s), although lowering changes nothing; only 41 % of the useful bandwidth.
- **Floor above the link** (`capacite-3-1`): the 5 Mbit/s floor exceeds a 3 then 1 Mbit/s link; the queue stays full the whole session.
- **Fixed capacity** (`capacite-20`, `4g`): the bitrate saw-tooths around the capacity (32 and 33 changes in 2 minutes), with seconds of full queue at each rise.
- **Bursts**: no bitrate change, rightly, but 50 ms lost at once exceeds error correction at every burst (60 seconds with holes).
