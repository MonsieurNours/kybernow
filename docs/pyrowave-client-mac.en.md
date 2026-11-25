# PyroWave: the Mac client

English · [Français](pyrowave-client-mac.md)

Client section of [pyrowave.en.md](pyrowave.en.md) (the contract and the Station), to be merged into it. About the Mac app; since KyberKit is shared, the Apple TV and iPhone/iPad apps compile the same decoder.

## PyroWave's Metal port

- Sources: the `metal/` folder of [PyroWave](https://github.com/Themaister/pyrowave) (MIT licence), an “AI assisted port”, barely maintained upstream. Copied as is into `apps/macos/PyroWave/upstream`, at the commit in `apps/macos/PyroWave/COMMIT` (a32b05e, the same as the Station). `apps/macos/scripts/vendor-pyrowave.sh [commit]` refreshes it; changing the commit means reading the `metal/` diff and running the offline test again.
- Compiled into KyberKit (`project.yml` of the Mac, Apple TV and iPhone/iPad apps: `pyrowave_common.mm`, `pyrowave_decoder.mm`, `pyrowave_bitstream.cpp`, ARC, C++17); Swift sees it through the `CPyroWave` module (`PyroWave/module`). No library in `Vendor/` is rewritten: the app may stay open during a build. The port's encoder isn't in the app (only in the test).
- The shaders are MSL source text (`shaders/pyrowave_msl.h`), compiled with the first decoder: the launch probe pays that cost, off the main thread.
- GPU: Apple7 family and later (M1, A14), 32-wide SIMD groups. Neither Intel nor AMD, nor the 2021 Apple TV 4K (A12).

## Decoding (`KyberKit/PyroWaveDecoder.swift`)

- `VideoDecoder` recognises the `pwv1` format (`0x7077_7631`) and hands its packets to `PyroWaveDecoder` instead of VideoToolbox.
- Configuration packet (16 bytes, `KNPW`, recognised by `is_config` or by its signature): 4:2:0 or 4:4:4 chroma, SDR BT.709 or HDR10 BT.2020 PQ, full or limited range, width, height. A change recreates the PyroWave decoder, the buffer pool and the chroma textures. If it's missing, resolution and chroma are read from the stream's sequence header (fallback to SDR, full range: the encoder doesn't write the colour there reliably).
- Media packet: a whole frame (PyroWave packets back to back). `pyrowave_decoder_clear`, then the whole packet into `push_packet`, then decoding is encoded into a Metal command buffer and submitted; Kyber's network thread returns at once. An incomplete frame (under 90 % of the blocks or low bands missing) or an unreadable one is skipped; nothing is asked of the computer, the next frame decodes on its own. At most 3 frames on the GPU at once: beyond that, the frame is skipped rather than queued.
- Output: a two-plane IOSurface `CVPixelBuffer`, the same format as VideoToolbox's. SDR: `420v`/`420f` or `444v`/`444f` (8-bit); HDR10: `x420`/`xf20` or `x444`/`xf44` (10-bit in the top of 16 bits). PyroWave writes Y straight into the IOSurface plane (r8Unorm or r16Unorm); Cb and Cr come out in two 16-bit textures, which a small compute pass interleaves into the CbCr plane. Tags: BT.709 matrix, primaries and transfer, or BT.2020 and PQ (SMPTE ST 2084). The Metal renderer (`VideoRenderer`) has nothing PyroWave-specific: YCbCr → RGB per matrix and range, HDR chain for a PQ picture.
- Values: PyroWave decodes normalised values. In r16Unorm, a value v becomes code v · 65,535, i.e. the P010 convention (10 bits at the top of 16) the renderer reads; chroma midpoint expected at 0.5.
- Timing: `received` when the packet arrives, `decoded` when the command buffer completes (GPU done); the frame goes to the renderer from that thread, like a VideoToolbox frame. The statistics' decode time (`VideoStats.decodeMs`) and a frame's path account for it unchanged.
- Loss: `dataLost()` (gap in the stream) does nothing for PyroWave; never a keyframe request.

## Capabilities

`DecoderCapabilities.pyrowave`: at launch (`DecoderCapabilities.prepare()`, off the main thread), the PyroWave device is created (pipelines compiled), then a 128 × 128 decoder, which decodes a built-in test frame (1,812 bytes, Y ramp from 16 to 235, encoded by the port's Metal encoder) and reads back a few points, within 4. About 55 ms on an M4 Max. The core gets `pyrowave` in the decoder capabilities (see apps/core/README.en.md).

## Offline test (`apps/macos/tests/pyrowave/run.sh`)

ffmpeg's `testsrc2` pattern (120 frames, BT.709, limited range) encoded by the port's Metal encoder (`encode` tool, 1.2 bits per pixel at 60 fps, one frame = packets back to back like a Station media packet), then decoded by `VideoDecoder` (`pwv1`) → `PyroWaveDecoder` (`bench` tool): decode time paced at 60 fps, back-to-back throughput, PSNR of the first frame against the source, truncated frame skipped without breaking the next, launch probe. HDR10: same 8-bit source tagged HDR (10-bit output), to measure that path.

Measured on 25/11/2026, MacBook Pro M4 Max (40-core GPU), macOS 27.2.1; mean decode time paced at 60 fps (median; maximum) and frames per second back to back:

| Resolution | Format | Bitrate | SDR (8-bit) | HDR10 (10-bit) | Back to back |
|---|---|---|---|---|---|
| 2560 × 1440 | 4:2:0 | 265 Mbit/s | 1.28 ms (1.23; 3.6) | 1.43 ms (1.46; 2.0) | 1,000 to 1,700 fps |
| 2560 × 1440 | 4:4:4 | 265 Mbit/s | 1.74 ms (1.76; 3.1) | 1.69 ms (1.64; 3.2) | 690 to 780 fps |
| 3840 × 2160 | 4:2:0 | 596 Mbit/s | 2.04 ms (2.08; 5.0) | 1.88 ms (1.87; 5.7) | 665 to 710 fps |
| 3840 × 2160 | 4:4:4 | 597 Mbit/s | 2.88 ms (2.94; 5.7) | 3.18 ms (3.15; 5.8) | 540 to 550 fps |

First-frame PSNR: 64 dB on Y (1440p 4:2:0), 56 dB on Y and 49 to 51 dB on chroma in 4:4:4 (same bitrate for three times the samples). When paced, the GPU clocks down between frames: a frame takes longer than back to back (1.3 ms against 0.6 to 1 ms). A second pass, with the machine busy elsewhere, gives 1.3 to 4 ms on average and up to 17 ms worst case at 1440p, 1.4 to 2.5 ms at 4K: the shared GPU matters more than the resolution. Not yet measured on an M1 chip or an Apple TV, nor with a stream from the Station.

## The core

`apps/core` knows PyroWave (format, capabilities, Station announcement, “Auto” rule when wired): see [apps/core/README.en.md](../apps/core/README.en.md), “PyroWave”. The Mac app sends `pyrowave` in the decoder capabilities; it doesn't yet give `wired` (Ethernet cable) in the settings, nor the format screen: they'll come with the mock-ups.

## Waiting on the Station

- The kyclient patch (codec identifier, `VideoCodec::PyroWave`): without it, Kyber never sends `pwv1` and the core doesn't ask for PyroWave.
- How the Station announces PyroWave to the app (`GET /api/stream/info`).
