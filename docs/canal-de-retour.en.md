# Feedback channel and a transport fitted to the network

English · [Français](canal-de-retour.md)

This document is the contract of the app → Station feedback channel (Kyber patches 0050 to 0052) and of what uses it: the repair of video datagrams (RaptorQ), the wait for out-of-order packets and the sound's redundancy. Status: **written and built (kyproto for Mac, Windows and the web engine; kycontroller for Windows and Mac; kyclient and the core for Mac), checked by the core's and kyproto's tests, including an end-to-end one over an in-memory connection losing one datagram in 20; never tried on a real network** (see "To try").

## Why

Until now the Station knew nothing of what the app received. So it sent the same everywhere:

- **30 % RaptorQ repair** on every frame (and at least 2 datagrams), whether the network lost 0 % or 10 % of its packets. On a clean cable that is 30 % of the bitrate for nothing; with PyroWave (several hundred Mbit/s), tens of Mbit/s.
- On the app's side, a **50 ms wait** each time a frame is missing while the next one is there, before moving on. A frame lost beyond repair froze the picture 50 ms longer.
- **Sound in RaptorQ**: each 10 ms Opus packet fits in one datagram, to which 2 repair datagrams were added. Three times as many packets to protect a few hundred bytes.

And the app's automatic bitrate (`apps/core/src/adaptive.rs`) judged losses from QUIC's counters, which count what each side loses *while sending* (the app's losses towards the Station mix with the picture's), over a packet count estimated from the bytes received.

## Negotiation (older Stations, older apps)

Nothing changes until both sides agree:

1. The app adds a `feedback` field to its start request (`POST /start_session`, `StartRequest`): `{"v": 1, "audio_redundancy": true}`. An older Station ignores it (serde lets unknown fields through) and sends as before.
2. A Station that understands it numbers its datagrams to that app from then on (see "Numbering"), opens the feedback endpoint **before** the picture and sound streams, and answers in `StartSessionResponse`: `"feedback": {"v": 1, "endpoint": <id>, "audio_redundancy": true|false}`. An older app asks for nothing and gets nothing new.
3. The app joins `endpoint`. If the answer has no `feedback` (older Station), the app keeps its 50 ms and reports nothing.

The web client (the `kyber-web` engine) never asks for the channel: it keeps the former transport.

The relay (`server/relay/tunnel`) sees none of it: everything travels inside Kyber's encrypted QUIC connection, which it carries as is (UDP, or TCP as a fallback). No relay change.

## Datagram numbering (Station → app)

Once the channel is negotiated, **every datagram** the Station sends on that connection (picture, sound, the channel's beats, all endpoints together) carries a connection number:

```text
[endpoint id | 0x8000: u16][endpoint's content…][number: u32, big-endian]
```

- The endpoint id's top bit says "numbered". kyproto's endpoint ids step by 2 from 0 or 1: they never reach 0x8000.
- The number is **at the end**: the app's router strips it without copying the datagram (`Bytes::slice`), and the endpoints see no difference (they ignore the id they read).
- It costs 4 bytes per datagram: an endpoint's useful size (`max_datagram_size`) drops by 4 while numbering is on.
- The app always understands a numbered datagram, whether it asked for the channel or not: a Station only numbers for an app that asked.
- The app does not number what it sends.

## The `feedback` endpoint

An ordinary kyproto endpoint (registered by the Station, joined by the app), **datagrams only**: one message per datagram, MessagePack **with field names** (`rmp_serde::to_vec_named`). An unknown field is ignored, a missing field takes its default, a message of an unknown kind is ignored: later versions add fields without breaking earlier ones. A lost message loses nothing important: counters are cumulative from the start.

### `Report` — app → Station, every 100 ms (even empty)

| Field | Type | Meaning |
|---|---|---|
| `n` | u32 | report number |
| `hi` | u32 | highest datagram number received |
| `received`, `lost`, `late` | u64 | cumulative since the start: received; judged lost (missing 50 ms after a later one arrived); arrived after being judged lost |
| `expected_i`, `lost_i` | u32 | over this interval: datagrams whose fate was settled, and among them the lost ones (before any repair) |
| `reorder_us` | u32 | delay of datagrams that arrived out of order: 99th percentile over 2 s (0 if none) |
| `wait_us` | u32 | missing-frame wait in place in the app |
| `streams` | list | one entry per picture endpoint: `ep` (endpoint id), `delivered` (last picture packet number handed to the decoder), `frames` (packets handed over, cumulative), `lost` (packets given up, cumulative) |
| `losses` | list | the last 8 frame losses (`FrameLoss`, below), repeated to cover a lost message |

### `Lost` — app → Station, at once

A `FrameLoss` sent as soon as the app gives up a picture packet (repair not enough, wait over): `id` (u32, increasing, to deduplicate), `ep` (endpoint id), `first` (number of the first lost packet, in the endpoint's `kypacket_seq` numbering), `count`. It is repeated in the following reports. **This is the input of the "opti-pertes" work** (repair without a keyframe).

### `Beat` — Station → app, every 100 ms

| Field | Type | Meaning |
|---|---|---|
| `n` | u32 | beat number |
| `report` | u32 | last report read |
| `repair_pm` | u16 | repair share aimed at, in ‰ of the source symbols (300 = the former 30 %) |
| `repair_ratio_pm` | u16 | repair actually sent over the last second, in ‰ of the source bytes |
| `loss_p95_pm` | u16 | raw loss, 95th percentile over 5 s, in ‰ |
| `audio_redundancy` | bool | the sound carries the previous packet |
| `rtt_us` | u32 | QUIC round trip seen from the Station |
| `quic_lost` | u64 | QUIC packets lost by the Station (cumulative) |

## What the Station does with it

### Picture repair

For each reported interval (at least 20 settled datagrams), the share lost; over the last 5 seconds, its **95th percentile** `p95`. The loss probability protected against: `p = min(35 %, 0.5 % + 1.25 × p95)` (never under 0.5 %, even on a perfect cable).

For a frame of `K` source symbols, the Station sends the **smallest number `R` of repair symbols** such that the probability of losing more than `R` of the `K + R` datagrams (binomial law of parameter `p`) stays under **1 in 1,000**. `R` is at least 1, and at most half of `K` (at least 3 for the very smallest frames). Exact up to 128 symbols (table rebuilt when `p` changes), normal approximation beyond.

Orders of magnitude, clean network (`p` = 0.5 %): 1 repair symbol for a one-symbol frame, 2 for 10, 3 for 50, 8 for 300 (2.7 %). Before: 2, 3, 16 and 90. Beyond 128 symbols, the normal approximation takes 2 more symbols for the binomial law's heavier tail.

Safe fallback: before the first second of reports, or after a second without one, the Station goes back to the former 30 % and at least 2.

**Within the video bitrate's budget.** A profile's bitrate is already computed to leave 30 % for repair ([profil-auto.en.md](profil-auto.en.md): "less error correction (30 %)"). While repair stays under those 30 %, the encoder keeps the target bitrate and the network carries less than before. When it goes over 30 % (a network losing a lot), the app lowers the bitrate it asks for to `target × 1.3 / (1 + r)` (`r` = the beat's `repair_ratio_pm`): picture and repair together never exceed what the session sent before. Reason shown in Statistics: "lowered" (automatic bitrate).

### Sound: the previous packet in the same datagram

With `audio_redundancy`, the sound goes in plain datagrams (`Unreliable`) and **each datagram also carries the previous Opus packet**:

```text
[endpoint id: u16][kypacket_seq: u32][end | redundancy (bit 30) | no.: u32][packet length: u16][packet][previous packet]
```

- A lost datagram costs no sound: the next one, 10 ms later, brings it back. Two in a row must be lost for a 10 ms gap.
- As many datagrams as without protection (one per 10 ms), instead of three with RaptorQ. The sound's bitrate doubles (the previous packet travels twice), about 256 kbit/s more in HD sound: little next to the picture.
- If the two packets do not fit in one datagram (7.1 "studio" sound), the previous packet leaves in a datagram of its own, in the plain format: a receiver ignores a packet it already has.
- Without `audio_redundancy` (older Station or app), the sound keeps `UnreliableFec` and its 30 %.

## What the app does with it

### Waiting for out-of-order packets

The wait for a missing frame is `min(50 ms, max(8 ms, 2 × reorder_us + 4 ms))`, recomputed at each report, 50 ms until the app has seen 500 numbered datagrams. It follows the **measured disorder** of the datagrams (thanks to their numbers), not the frames' jitter: waiting only helps a datagram overtaken by a later one arrive; a lost datagram never arrives, and a queue's jitter delays the later ones too, without disorder. On a cable or an ordinary Wi-Fi (which delivers in order), the wait drops to 8 ms: a lost frame freezes 42 ms less.

Sound keeps its 10 ms (the backup packet comes with the next one, 10 ms later).

### Automatic bitrate

Every second the session gets the channel's summary (kyclient's `Metric::Feedback`): datagrams expected and lost (raw, before repair), disorder, wait, repair share, frame losses. The automatic bitrate then judges losses on these exact numbers, instead of estimating them from QUIC's counters and the bytes received.

## For the other works

- **opti-pertes** (repair without a keyframe): on the Station's side, `kyproto::FeedbackHost::subscribe()` returns the events `FeedbackEvent::Lost { endpoint, first, count, frames }`, where `frames` gives, for each lost packet still known, its `pts` and whether it was a keyframe (the Station keeps the last 512 packets sent per endpoint), and `FeedbackEvent::Report` with `delivered` per endpoint (acknowledgement of the frames handed to the decoder). kycontroller keeps the session's `FeedbackHost` in its services (`services.feedback`).
- Adding a field: in `Report`, `Beat` or `FrameLoss`, with `#[serde(default)]`, without changing `v`. A new message kind: ignored by earlier versions. `v` only changes for an incompatible change.

## To try on a real network

- Cable: repair share around 3 to 20 % depending on frame size (the tiny frames of a still desktop take the most, in symbols), no frame lost; wait at 8 ms.
- Loaded Wi-Fi (microwave oven, a second device downloading): the share follows losses, lost frames stay rare; check that burst losses (Wi-Fi dropping out 20 to 50 ms) cost no more than before.
- Relay over UDP and over TCP (over TCP, no loss: share at its minimum).
- Sound: drop 1 datagram in 20 (`pfctl`/`dnctl` or Network Link Conditioner): no audible gap.
- Older Station + new app, new Station + older app: normal session, 30 % and 50 ms as before.
