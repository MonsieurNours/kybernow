# PyroWave: the web client

English · [Français](pyrowave-client-web.md)

Client section of [pyrowave.en.md](pyrowave.en.md) (the contract and the Station), to be merged into it. Covers the `/link` web client served by Admin (`server/web/src/link`). Interface rules: [link-ui.en.md](link-ui.en.md), "PyroWave".

There is neither Vulkan nor Metal in a browser, and WebCodecs doesn't know PyroWave: the decoder is written in **WebGPU** (WGSL), in the page, from the upstream shaders (`wavelet_dequant.comp`, `idwt.comp`, through the Metal port vendored in `apps/macos/PyroWave/upstream/shaders`, commit a32b05e) and `pyrowave_bitstream.cpp`.

## Decoder (`server/web/src/link/pyrowave/`)

- `bitstream.ts`: the `KNPW` configuration packet, version 1 (16 bytes, the one from a 0.11.7 Station) or 2 (48 bytes, HDR10 metadata read and written to the page's log; a later version is read through its first 16 bytes, like the core), the stream's sequence header (fallback: resolution and chroma, SDR full range, like the Mac), the layout of 32 × 32 blocks and reading a frame (received blocks put end to end, block → position table). A frame is ready like upstream's `decode_is_ready(true)`: complete, or more than 90% of the blocks with the two lowest bands whole; otherwise skipped.
- `shaders.ts`: dequantization (one group of 128 threads per 32 × 32 block, one thread per 4 × 2 sub-block: bit planes, scale, dead zone, signs), inverse CDF 9/7 transform by lifting over 5 levels (transposed reads with mirroring through `textureGather`, like upstream), then YCbCr → RGB into the canvas: BT.709 SDR, or BT.2020 PQ brought down to SDR by the curve of every app and the Station (`rules::hdr::sdr_curve`: knee at 0.75 of the 203-nit SDR white, 4 times the white brought to 1, on the largest component) (the web client's canvas is SDR; the web client never asks for HDR), full or limited range, 4:2:0 chroma interpolated. Float32 values throughout (`r32float`).
- **Subgroups**: the prefix sum of non-zero coefficients (sign positions) goes through `subgroupInclusiveAdd` when the browser has the `subgroups` feature, otherwise through workgroup memory. The subgroup variant assumes a subgroup's threads are consecutive: the probe checks it by decoding the test image and takes the other one if the image is wrong. Measured: no notable difference between the two (table below), dequantization isn't what costs.
- `decoder.ts`: device and pipelines (shared), one decoder per configuration (textures of the 5 levels, Y, Cb, Cr planes, bind groups prepared once), stream upload through `writeBuffer`, one compute pass (≈ 45 to 60 dispatches) then one render pass. At most 3 frames on the GPU: beyond that, the frame is skipped.
- `live.ts`: the session decoder. Kyber's web engine (patch 0025-kyctlweb) hands it the `pwv1` packets through `globalThis.__kybernowPyroWave.create(output)`: `push(data, pts, is_config)` returns true when a frame will come out. The frame is drawn into an offscreen WebGPU canvas, taken right away (`transferToImageBitmap`), then handed back to the engine as a `VideoFrame` once the GPU is done: the engine draws it into the session canvas like a WebCodecs frame (same pacing, same latency measurements). A lost or incomplete frame is skipped, never a keyframe request. Decoding time (arrival → frame ready) and lost frames (skipped, or a gap in the pts) for the Statistics.

## Web engine (patches 0025-kyctlweb and 0025-kyberweb)

See [kyber-changes.en.md](kyber-changes.en.md). The web engine starts from a stock kyctl (`build.sh`): 0025-kyctlweb brings in the `PyroWave` variant and `fallback_codec` from 0025-kyclient, and kywebplayer hands the `pwv1` packets to the page instead of WebCodecs. 0025-kyberweb adds the `pyrowave` codec and `fallback_codec` to the JS configuration. The page recognizes an engine that knows PyroWave by its `fallback_codec` setting (`engineKnowsPyroWave()`).

## Capabilities and rules

- **Probe at load** (`main.ts` → `pyrowave/state.svelte.ts`): WebGPU present, an adapter (`high-performance`), shaders compiled, and a real decode of the Mac client's test image (128 × 128, Y ramp from 16 to 235, Cb and Cr at 128, 1,812 bytes), read back within 4. 0.3 to 1.2 s depending on the browser, in the background.
- **Offered** in "Video format" only if the Station announces it (`pyrowave.available` in `GET /api/stream/info`; field missing: value hidden), the probe passed and Kyber's engine knows PyroWave. Otherwise an inactive value with its reason: "{name} doesn't offer PyroWave." or "This browser can't decode PyroWave.". A PyroWave choice that is no longer offered is kept, "Auto" applies.
- **Auto never picks PyroWave**: a browser can't tell whether it is connected by cable. Picked by hand, it is always warned: the "unknown network" text (proposed in link-ui), or the relay one.
- **Through the relay** (WebSocket): allowed, with the relay warning, no bitrate cap at the relay (common rule). What really holds at PyroWave's bitrate over WebSocket (relay, and Safari or Firefox on the local network, which always use WebSocket) isn't measured: to try.
- **Fallback**: the request carries `fallback_codec` (Auto's format, HEVC or H.264); a Station that no longer offers it uses that format. PyroWave that won't open in the page (decoder refused, or PyroWave frames arrive and none decodes): the picture restarts at once in Auto's format (`fallback_codec`), for this session only, with link-ui's brief notice “Picture in {format}: PyroWave couldn't start on this device.” (`fallback.ts`, `pyrowaveFallback`); the bar and Statistics show the format received.
- **Bitrate and resolution**: the core's rule ([pyrowave.en.md](pyrowave.en.md), "Lowered resolution"), same computation (`pyrowave/rules.ts`, tested with the core's cases). Ceiling: the Station's `pyrowave.max_mbps` and the bitrate set by hand (slider up to 1,000 Mbit/s with PyroWave). The lowered resolution is that of the virtual display asked for in `prepare`; it is said at the start of the session and in the Statistics. 4:4:4 with the Desktop and Max profiles: PyroWave does it on its own, whatever WebCodecs decodes.
- **For the host only**: a sharing guest follows the host's picture (host in PyroWave: a separate picture in a format it reads); a friend who borrows the computer keeps Auto's format.

## Validation

Tools in `server/web/tests/pyrowave/`:

1. `prepare.sh <folder>`: ffmpeg's `testsrc2` pattern (30 frames, BT.709, limited range) encoded by the Metal port's encoder (`encode` tool from `apps/macos/tests/pyrowave`, 1.2 bits per pixel at 60 fps), at 1440p and 4K, 4:2:0 and 4:4:4; the first frame decoded by the Metal port's decoder (`refdump`, the reference, as float32).
2. `deno run --allow-read --unstable-webgpu tests/pyrowave/offline.ts <folder>`: headless test (Deno's WebGPU).
3. `deno run -A tests/pyrowave/serve.ts <folder>` then `http://127.0.0.1:8765/bench.html` in a browser: probe, frame 0 against the reference and the source, truncated frame, paced at 60 fps (GPU time through `timestamp-query`, and up to frame ready), back to back, with and without subgroups, the client's full path (canvas → `VideoFrame`, colors compared with the source), and the bridge as the engine calls it. Results are written to `<folder>/results.txt`.

**Accuracy** (all browsers, identical): at most 0.24 to 0.31 code (8-bit) away from the Metal reference, PSNR against the source identical to the Mac's (64.3 dB in Y at 1440p 4:2:0, 55.8 dB in 4:4:4). Full path colors: 4.4 codes mean difference from the source (361 points, pattern edges included). Truncated frame skipped, the next one decodes; bridge: 28 frames accepted for 28 out, truncated frame skipped, 2 missing frames out of 30 counted (6.7%).

**Measurements** of 25/11/2026, MacBook Pro M4 Max, macOS 27.2; median paced at 60 fps (frame ready: from submission to the GPU finishing; GPU: GPU time), and frames per second back to back:

| Browser | Resolution | Format | Frame ready | GPU | Back to back |
|---|---|---|---|---|---|
| Chrome 158 (headless), subgroups | 2560 × 1440 | 4:2:0 | 4.5 ms | 3.1 ms | 1,010 fps |
| | 2560 × 1440 | 4:4:4 | 3.5 ms | 2.2 ms | 658 fps |
| | 3840 × 2160 | 4:2:0 | 4.3 ms | 2.5 ms | 538 fps |
| | 3840 × 2160 | 4:4:4 | 6.8 ms | 4.5 ms | 401 fps |
| Chrome 158, no subgroups | 2560 × 1440 | 4:2:0 | 3.3 ms | 2.0 ms | 887 fps |
| | 3840 × 2160 | 4:2:0 | 5.5 ms | 3.9 ms | 662 fps |
| | 3840 × 2160 | 4:4:4 | 6.5 ms | 4.2 ms | 403 fps |
| Safari 27.2 (no subgroups) | 2560 × 1440 | 4:2:0 | 4 to 6 ms | 2.3 to 4.6 ms | 675 fps |
| | 2560 × 1440 | 4:4:4 | 5 ms | 2.5 to 3.0 ms | 490 fps |
| | 3840 × 2160 | 4:2:0 | 5 to 6 ms | 2.9 to 3.3 ms | 475 fps |
| | 3840 × 2160 | 4:4:4 | 7 ms | 4.3 to 4.6 ms | 300 fps |

Reading the stream on the CPU: 0.4 to 0.9 ms. Full path (canvas → `VideoFrame`, 1440p 4:2:0): 2.7 to 4.9 ms median in Chrome, 8 ms in Safari. Probe: 0.4 to 1.2 s in Chrome, 0.9 s in Safari, 0.3 s in the Claude app's built-in browser (Chromium, subgroups). Back to back, throughput is about the Metal decoder's (665 to 710 fps at 4K 4:2:0); paced, the GPU clocks down between frames: a frame takes 2 to 3 times as long as with the Metal decoder (1.3 to 2.9 ms). Safari rounds `performance.now()` to the millisecond.

## Where PyroWave will be unavailable

Wherever the probe doesn't pass: no WebGPU (browsers that don't have it or have it turned off; Firefox doesn't have it everywhere), no adapter (GPU refused by the browser, remote desktop), or a wrong test image. And with a Kyber engine without patch 0025 (older Station), or a Station that doesn't offer it (Mac Station, no Vulkan 1.3). Tried here: Chrome, Safari and the built-in browser on the Mac; neither Firefox, Edge, Windows, Linux, Android, iPhone nor iPad.

## To try

- A PyroWave session with a Windows Station that offers it, in Chrome (WebTransport) then Safari (WebSocket on the local network), at 1440p and 4K, 4:2:0 and 4:4:4: does the bitrate hold over WebSocket? The banner, the chip through the relay, "Switch to HEVC", the lowered resolution with a Station ceiling.
- The fallback when the Station no longer offers PyroWave (`fallback_codec`) and PyroWave that won't open in the page (fallback to Auto's format, with its notice).
- WebGPU on other machines (Windows with an NVIDIA, AMD, Intel GPU; Firefox; iPad).
