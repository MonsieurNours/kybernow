# Recovery after a lost picture

English · [Français](reprise-apres-perte.md)

When a picture is lost on the way (packets lost beyond error correction),
the app's decoder no longer has the reference of the following pictures.
Until now, the app asked for a keyframe (`POST /force_idr`): a full
picture, five to ten times heavier than another one, arriving when the
network has the least room, and a drop in quality while the bitrate
recovers. Now the Station repairs the loss **without a keyframe** when its
encoder can: the next picture relies only on pictures the app did receive.

## What happens

1. **The Station numbers every picture** (HEVC and H.264): a small “user
   data unregistered” SEI (UUID `KyberNow:reprise`, about thirty bytes) in
   front of the first slice, with a number that grows by one with each
   picture. Kyber does not tell the app that a picture was lost (its
   receiver only counts the missing packets): the apps' core
   (`apps/core/src/media.rs`) sees the gap in the numbers and hands it to the
   decoder as a loss (`Packet::Hole`).
2. **The app's decoder drops what follows** (it relies on the lost picture)
   and the core asks for a repair: `POST /recover {"last_pts": N}`, N being
   the pts of the last picture received before the loss.
3. **The Station repairs at the next picture**, with the best means its
   encoder has:
   - **invalidation** (NVENC): the pictures newer than N are removed from
     the references, the next picture relies on N or an older one, still in
     the decoder's memory (4 pictures at least);
   - **long-term reference** (NVENC, VideoToolbox): the Station keeps
     long-term reference pictures (NVENC: two, one marked every 250 ms;
     VideoToolbox: those it picks), and the next picture relies on the
     newest one not past N;
   - otherwise, **a keyframe**, as before.
4. **The repairing picture says so**: its SEI also carries the bound, the
   pts of the newest picture it depends on. A decoder that received up to
   there resumes on it without being rebuilt; another one (another app on a
   shared picture, or one that lost more) waits, and asks again.
5. Nothing within 300 ms: the core asks for a keyframe, then restarts the
   stream at 1.5 s, as before.

**With the feedback channel** ([canal-de-retour.en.md](canal-de-retour.en.md),
patches 0050 to 0052), the Station doesn't wait for the app: the `Lost`
message names the lost pictures, and kycontroller (patch 0061) asks at once
for a repair up to the picture before the first lost one (the encoder takes
the newest it sent up to there). The app's request (step 2) stays the safety
net, for an app or a Station without the feedback channel, and a session
with several displays.

Several requests before the next picture (several apps on a shared
picture): the oldest one wins, it repairs everyone. The same loss reported
twice (feedback channel, then the app): the second request, which the repair
already sent meets, is ignored for 200 ms.

Limit: a loss is only seen when the next picture arrives (skipped number, or
`Lost`, which leaves with the next packet). On a still screen, the last lost
picture waits for the Station's next picture.

## Encoders

| Encoder | Invalidation | Long-term reference | Otherwise |
|---|---|---|---|
| NVENC (Windows, Linux Station) | yes, if the card can (`NV_ENC_CAPS_SUPPORT_REF_PIC_INVALIDATION`) | 2, if the card keeps any | keyframe |
| VideoToolbox (Mac Station) | no | yes (`EnableLTR`, macOS 12 and later) | keyframe |
| AMF, QSV, VA-API, x264 | no | no | keyframe |
| libavconv engine | — | — | keyframe |

Only **HEVC** is repaired without a keyframe: in H.264, not every decoder
accepts the `frame_num` gap of the dropped pictures. H.264 is numbered all
the same (losses seen at once, keyframe asked for without waiting for the
decoder to complain). AV1 and PyroWave don't change.

Setting: `[kyavserver] loss_repair = false` turns off repairs without a
keyframe (pictures stay numbered); true by default.

## Apps

The core (`media.rs`) reads the numbers and the bound for every app, and
`ResumeGate` tells each decoder whether it can resume: it must have
received, without losing anything before, the picture of the bound. The
decoder is never rebuilt for a repair, and a dropped picture is never shown.

| App | Decoder |
|---|---|
| Mac, iPhone, iPad, Apple TV | VideoToolbox (`VideoDecoder.swift`, KyberKit) |
| Windows | D3D11VA through FFmpeg (`apps/windows/src/video.rs`) |
| Android | MediaCodec (`apps/android/jni/src/video.rs`) |
| Linux, Raspberry Pi | VA-API or V4L2 through FFmpeg (`apps/linux/src/device/video.rs`) |
| LG TVs | NDL (`apps/webos/src/device/media.rs`) |

The web client is not concerned: it keeps the keyframe (a lost picture shows
there as a WebCodecs error).

A Station without the patch answers 404 to `/recover`: the app then asks for
a keyframe, as before, and no longer asks that Station for `/recover`. Its
pictures are not numbered: nothing changes for it.

## How it's done

| Part | Files |
|---|---|
| FFmpeg | `server/kyber-patches/0060-kymedia-ffmpeg-reprise.patch`: `0060-avcodec-reprise-sans-image-cle.patch` put in the wrap after 0017 (`nvenc.c`, `nvenc.h`, `nvenc_h264.c`, `nvenc_hevc.c`, `videotoolboxenc.c`). Options `ref_recovery` (when opening), `ref_recover` (for the next picture), `ref_method` and `ref_bound` (read by the caller). |
| txproto | `0060-txproto-reprise.patch`: `kn_recovery.h` (new), `recover` command (`last_pts`), `tx_encoder_recover`, number and bound in the SEI. |
| kyavservice | `0060-kyavservice-reprise.patch`: `Command::VideoRecover { last_pts }`, `ref_recovery` given to NVENC and VideoToolbox in HEVC, `[kyavserver] loss_repair`; libavconv engine: keyframe. |
| kycontroller | `0060-kycontroller-reprise.patch`: `POST /recover`, same scope as `/force_idr`; a share guest repairs on the pictures it gets; requests merged per shared stream. `0061-kycontroller-reprise-canal-de-retour.patch` (after 0052, which it needs): `FeedbackEvent::Lost` repaired at once, one-picture-stream sessions only. |
| kyclient (apps) | `apps/macos/kyber-patches/0060-kyclient-reprise.patch`: `Client::recover(last_pts)`, falls back to `/force_idr` (404). |
| Core | `apps/core/src/media.rs` (`image_mark`, `StreamLoss`, `ResumeGate`), `session.rs` (`repair`). |

## To try on hardware

None of this has run against a real Station yet (written, compiled, core
tests only):

1. **NVENC** (Windows Station, NVIDIA card, HEVC): cut the Wi-Fi for a split
   second, or lose packets (`clumsy` on Windows), then read kyavserver's
   log. FFmpeg's lines are at VERBOSE level, only written with
   `txproto_log_level = "debug"` (txproto engine) or `ffmpeg_log_level =
   "debug"` (libavconv engine) in the `[kyavserver]` section of
   `kyber_config.toml`. When the stream starts: “Repairs without keyframe:
   DPB …, invalidation …, … long-term references” (otherwise “No
   invalidation nor long-term reference: losses are repaired by IDR”); at
   each loss: “Repair up to {pts} at {pts}: invalidation” (or “long-term
   reference”, or “IDR”). Check
   that the picture comes back sharp, with no corrupt picture, and that the
   bitrate no longer spikes. To check as well: does the card accept
   invalidation with progressive refresh (the default setting)?
2. **VideoToolbox** (Mac Station): same test, same log level. VideoToolbox
   writes a single line, when the stream starts: “Repairs without keyframe:
   long-term references”; if it refuses `EnableLTR` in HEVC, “No long-term
   references (…): losses are repaired by keyframes” (WARNING level, always
   written), and everything goes through the keyframe. Nothing is written
   at each repair: the app's keyframe counter (point 3) shows it.
3. **Each app**: “image perdue après le pts … : reprise demandée” in the
   log (written by the core, in every app), then the
   keyframe counter no longer growing at each loss (Statistics). Above all
   the hardware decoders we don't know from the inside: MediaCodec (by
   manufacturer), NDL (LG), V4L2 (Raspberry Pi).
4. **Shared picture** (KyberNow Share): two apps on the same picture, one
   losing packets; the other one must see nothing.
5. **Older Station**: an up-to-date app facing a Station without the patch
   keeps the keyframe (404 once, then `/force_idr`).
6. **Feedback channel**: with 0050 to 0052, the repair leaves on `Lost`
   (kycontroller log: “picture lost from …, repaired”, debug level), before
   the app's request, which must be ignored. Still screen: does the Station
   send a picture again, so that a loss on the last one is seen?
