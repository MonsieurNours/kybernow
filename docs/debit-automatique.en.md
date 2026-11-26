# Automatic bitrate

English · [Français](debit-automatique.md)

With the “Adaptive bitrate” switch of the settings (on by default, [profil-auto.en.md](profil-auto.en.md)), the app lowers the picture's bitrate when the network is saturated, then raises it again once the network is clean: for a profile (Game, Desktop, Max, Auto) as for a bitrate set by hand. Off, the bitrate never moves. The logic lives in the core (`apps/core/src/adaptive.rs`), so it is the same in every app that uses it; the browser's web client doesn't have it yet.

## What the app looks at

Every second, the session reads what Kyber and the core already measure (`latency::Second`):

- **the network round trip** (that of Kyber's QUIC connection), compared with its lowest over the last 10 seconds: a rise of more than 15 ms, or of more than half the idle value, means a queue is filling up somewhere (router, Wi-Fi, relay);
- **lost packets**, relative to packets received (estimated from bytes, 1,200 per packet): 2 % or more, with at least 3 packets lost;
- **holes in the picture**, losses the error correction couldn't repair.

A single loaded second changes nothing: an isolated latency spike, common on Wi-Fi, has no effect. It takes two in a row. Heavy losses (10 % or more) lower the bitrate at once.

## What it does

The app keeps a **share of the profile's bitrate**, between 1 (the profile's bitrate) and the 5 Mbit/s floor (the Station encoder's, Admin › Minimum bitrate):

- loaded network: the share is multiplied by 0.7;
- 4 clean seconds in a row: it rises by 15 %, and only by 5 % near the bitrate that loaded the network (for one minute);
- after each change, 2 seconds without a decision (the network and the encoder settle), except for heavy losses.

It is a share, not a bitrate: a change of resolution or of displays keeps the same margin. The session starts at the profile's bitrate (on the home network it gets through right away), keeps the share when the picture is restarted (same network), and relearns the idle round trip at each restart (the path may have changed: home network, relay). A new profile resets it to 1.

Automatic bitrate pauses when the app already caps the bitrate (hidden window, app in the background) or shows nothing: what gets through then says nothing about the network. It only applies to the host's picture: a KyberNow Share guest's own picture is adjusted by the Station (patch 0020), and a borrower doesn't set the bitrate.

It comes on top of the existing fallback (“no picture”: capped bitrate, then 4:2:0): the fallback remains the last resort when nothing arrives at all.

Each change is written to the app's log: “débit automatique : 60 → 42 Mb/s (profil 60 Mb/s, aller-retour 41 ms, 0 paquets perdus)”.

## PyroWave: paused, and “Switch to HEVC” when the network is saturated

With PyroWave ([pyrowave.en.md](pyrowave.en.md)), the automatic bitrate is **paused**. PyroWave encodes each frame on its own, with a fixed budget: 30 % less bitrate spoils every frame at once, where HEVC spreads the drop. And PyroWave's bitrate is already fitted to the link by the lowered resolution (the Station's, the link's and the app's ceiling), which lowers the resolution rather than sharpness.

The network is still watched with the same signs. When it is saturated for **5 seconds in a row** (`PYROWAVE_STRAIN`), the core sends the `PyroWaveStrained` notice: the apps show the banner “The network is saturated”, “The picture may stutter.”, with “Switch to HEVC” (same shape as the start banner, [link-ui.en.md](link-ui.en.md)). Once per session, for the host only, and not when the app already caps the bitrate (hidden window). In HEVC, the automatic bitrate takes over again.

## On the Station: the bitrate changes live

The app sends the new bitrate through Kyber's `/set_bitrate`, which hands it to the encoder. The patch `server/kyber-patches/0026-kymedia-ffmpeg-debit-en-direct.patch` (FFmpeg 8.1, Windows, Mac and Linux Stations) makes a bitrate change free:

- **NVENC**: FFmpeg reset the encoder and forced a keyframe at every bitrate change. The patch changes the bitrate without a reset or a keyframe (dynamic change, which NVENC accepts when the card reports it). The encoder's buffer (VBV), set by default to 2 seconds of the initial bitrate, follows the new bitrate: otherwise a lowered bitrate would still let through the bursts of the old one.
- **VideoToolbox**: FFmpeg only read the bitrate when the session was created; a change while encoding was ignored. The patch hands it to the session (`AverageBitRate`) before the next frame.
- **VA-API** (Linux Station, AMD and Intel cards): FFmpeg only sent the rate control parameters with IDR frames (every 10 s on the Linux Station). The patch scales them to the new bitrate (HRD buffer included) and sends them with the next frame, without a keyframe.

Without this patch (older Station), automatic bitrate still works, but each change costs a keyframe on NVENC and has no effect on the Mac Station (on the Linux Station with VA-API, it waits for the next IDR frame).

## In the statistics: the target bitrate

Every second, the core gives the apps the bitrate it asks of the computer and what sets it (`Second::target`, `adaptive::TargetBitrate`); the statistics show it under “Bitrate” (“Target bitrate” line and a line on the graph, [link-ui.en.md](link-ui.en.md)). When several things apply, the one that sets the value is told, in the order the session applies them: the app's cap (“20 Mbit/s · window hidden”), then the fallback (“15 Mbit/s · lightened stream”), then automatic bitrate (“42 / 60 Mbit/s · lowered, network saturated”); otherwise the kind of setting: “320 Mbit/s · PyroWave, Auto paused”, “80 Mbit/s · set by hand”, “60 Mbit/s · Auto”. A cap above the bitrate sets nothing. API and bridges: [apps/core/README.en.md](../apps/core/README.en.md#target-bitrate-in-the-statistics-srcadaptivers-srcsessionrs-srclatencyrs).

## Not done

- Nothing warns of a link that is too weak, apart from the target bitrate in the statistics (below; not in the LG TV app).
- **AMD cards** (AMF encoder): the bitrate changes live since 0.11.1 (FFmpeg patch 0023 of 0026); **Intel** (Quick Sync): each change resets the encoder (a keyframe), which FFmpeg does on its own.
- No lower resolution or frame rate: only the bitrate moves.
- Error correction no longer moves on its own: with a Station that has the feedback channel, it follows the measured losses and stays within the 30 % share the profile leaves it; beyond, the bitrate asked for drops accordingly (`TargetInputs::repair_pm`, reason "lowered"), PyroWave included. With an older Station it keeps its fixed 30 % share ([canal-de-retour.en.md](canal-de-retour.en.md)).
- With the feedback channel, losses are the ones the app counts on the Station's datagrams (`Second::feedback`), no longer an estimate from QUIC's counters.
- The browser's web client keeps its fixed bitrate, and doesn't show the “The network is saturated” banner with PyroWave.
- Values set by reasoning and the core's tests, not yet on a really saturated network (loaded Wi-Fi, 4G, relay).

## Tests

`cargo test -p kybernow-core adaptive` (the logic: clean home network, growing queue, isolated Wi-Fi spike, heavy losses, still desktop, floor, climbing back, careful climb, no picture, new path, holes) and `… debit_automatique` (the session: profile only, paused under an app cap, bitrate set by hand untouched, target bitrate and its reason), `… debit_vise` (reasons, precedence, texts). The FFmpeg patch is built for the Mac (VideoToolbox) and for Windows (NVENC, cross-compiled), not yet tried on a Station.
