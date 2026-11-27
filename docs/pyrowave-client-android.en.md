# PyroWave: the Android client

English · [Français](pyrowave-client-android.md)

Client section of [pyrowave.en.md](pyrowave.en.md) (the contract and the Station), to be merged into it. About the Android app (phone, tablet, TV); rules and texts: [link-ui.en.md](link-ui.en.md), “PyroWave”.

## Build (`apps/android/scripts/build-native.sh`)

- **PyroWave**: the upstream Vulkan path, at the commit pinned in `server/kyber-patches/pyrowave.sh` (a32b05e, the same as the Station and the Mac), sources in `third_party/pyrowave` with the partial Granite of `checkout_granite.sh` (the Station's `pyrowave_fetch`; nothing in that folder is modified). The CMake project `apps/android/jni/pyrowave` adds Granite cut down to what the codec needs (no renderer, no shader compiler, no windowing, `null` platform), the `pyrowave` library, the C API (`pyrowave_c.cpp`) and the app's bridge (`kn_pyrowave.cpp`), with the NDK's CMake toolchain (`ANDROID_STL=c++_static`) and the NDK's `glslc` for the two display shaders.
- **Output**: `libkybernow_pyrowave.so` (1.0 MB for arm64-v8a, 1.1 MB for x86_64, stripped), in `jniLibs/<abi>/` next to `libkybernow.so`, which links to it through the `pyrowave` Cargo feature of `kybernow-android`. Only the `kn_pw_*` functions are exported; it depends only on Bionic, `liblog` and `libandroid`: Vulkan is loaded on demand by volk, so the app starts on a device without Vulkan.
- **ABIs**: arm64-v8a and x86_64. armeabi-v7a stays without PyroWave (its probe answers “unavailable”). 32-bit Android devices (Chromecast with Google TV, entry-level TVs) have no Vulkan 1.3 anyway.
- **Patches reapplied**: `build-native.sh` resets its patches' files to the original and reapplies them all (`scripts/kyber-client-patches.sh`, docs/build-notes.en.md); a patch covered by another build no longer stops it.
- **Separate rootfs**: `KYBERNOW_ANDROID_ROOTFS=<folder>` builds libkynput (and takes libopus, `libvlc.pc`) in `<folder>/rootfs-<target>` instead of `third_party/kyber-desktop/rootfs-<target>`, to build against the current kynput sources without touching the shared rootfs.
- **What Kyber requests**: the kyclient patch 0025 (`VideoCodec::PyroWave`, `fallback_codec`) and the core's `pyrowave` feature go together; `third_party/kyber-desktop` is shared by every app. `build-native.sh` applies them by default since 0.11.1 (the `kyber-pyrowave` feature of `kybernow-android`; `--kyber-pyrowave` is still accepted). `--sans-pyrowave` leaves them out: the app decodes PyroWave and reports `pyrowave` to the core, but the core never asks Kyber for it. `--no-pyrowave` also removes the decoder.

## The launch probe

When the app starts, on a background thread (`PyroWave.probe`, `Native.pyrowaveProbe`, `kn_pw_probe`):

1. A Vulkan loader (`volkInitialize`) at **1.3** or later: Android 11's loader stops at 1.1 (the Shield will go no further).
2. A GPU at Vulkan **1.3** with what `pyrowave.h` asks of the decoder: BASIC, VOTE, BALLOT, ARITHMETIC, SHUFFLE and SHUFFLE_RELATIVE subgroup operations in compute, `subgroupSizeControl` and `computeFullSubgroups`; and for display `synchronization2`, `dynamicRendering`, a graphics and compute queue, `VK_KHR_android_surface` and `VK_KHR_swapchain`.
3. The app's device (created by the app, lent to PyroWave with `pyrowave_create_device`), kept for sessions; the fragment path when `pyrowave_decoder_device_prefers_fragment_path` says so (Qualcomm, Mali), compute otherwise.
4. A 128 × 128 decoder, the Mac probe's test frame (1,812 bytes, Y ramp from 16 to 235, chroma at 128) decoded and read back (`decode_cpu_buffer_synchronous`), within 4.

The result (`PyroWave.decodes`) goes to the core in the capabilities (`pyrowave` in `Hdr.capabilities`, and for a KyberNow Share guest). The log names the GPU and the path, or the reason: `adb logcat -s KyberNow | grep PyroWave`.

## Decoding and display (`jni/src/video.rs`, `jni/src/pyrowave.rs`, `jni/pyrowave/kn_pyrowave.cpp`)

- The decoder thread recognises `pwv1` (`0x7077_7631`) and goes alongside MediaCodec, on the same `SurfaceView`: switching format closes one before opening the other (a surface has a single producer).
- **`KNPW` configuration packet** (read by the core, `pyrowave::Config`: 16-byte version 1, 48-byte version 2 that the core asks the Station for, `is_config` or recognised by its signature): chroma, colour (SDR BT.709 or HDR10 BT.2020 PQ), range, resolution, and in version 2 the content's HDR10 metadata (`Config::hdr10`). Without it: the session's size, 4:2:0, full-range SDR. A change recreates the decoder, the three planes and, if HDR changes, the swapchain.
- **One frame**: `pyrowave_decoder_clear`, the whole packet into `push_packet`; incomplete (under 90 % of blocks, or low bands missing) or unreadable: skipped, nothing is asked of the computer. If several frames are waiting, only the newest is decoded.
- **A single command buffer** per frame: PyroWave's decode (`pyrowave_device_set_command_buffer`) into three R8 (SDR) or R16 (HDR, if the GPU takes them; R8 otherwise) planes, in `COLOR_ATTACHMENT_OPTIMAL` (fragment) or `GENERAL` (compute), then a render pass (dynamic rendering) that samples the planes (4:2:0 chroma upscaled by the sampler) and converts YCbCr → RGB into the swapchain image: BT.709 in SDR, BT.2020 in HDR, full or limited range (P010 at 10 bits) from the configuration packet. Submitted, presented, and the function returns with the GPU done: the decode time for the statistics, and PyroWave's internal buffers free for the next frame.
- **Swapchain** on the session's `ANativeWindow`: MAILBOX when the surface offers it (the newest frame at the next refresh, like MediaCodec), FIFO otherwise; upright image (`IDENTITY`, the compositor rotates when needed); rebuilt when the surface changes size. The display's refresh rate and mode stay the ones the app picked (`preferredDisplayModeId`, `Surface.setFrameRate`, set on the window and the surface, not by the decoder).
- **HDR10**: an `A2B10G10R10` swapchain in `HDR10_ST2084` when the surface accepts it (`VK_EXT_swapchain_colorspace`), the display's metadata through `VK_EXT_hdr_metadata`; otherwise the PQ image is brought down to SDR in the shader (white at 203 nits, BT.2020 → BT.709, soft knee, gamma 2.2).
- **Statistics**: `frame_displayed(received, decoded with the GPU done, shown ≈ decoded + one period)`, so “Decoding” and the path of a frame are unchanged; a skipped frame is never shown and counts in “Lost frames”. Decoder name: “PyroWave (Vulkan, {GPU}, fragment|compute[, HDR10])”. A gap in the stream, a rebuilt surface: nothing to request (no keyframes).

## Capabilities and the “Auto” rule

- `pyrowave`: the probe (above). `wired`: the active network is Ethernet (`ConnectivityManager`, `NetworkCapabilities.TRANSPORT_ETHERNET`), sent in the session settings; the core picks PyroWave on “Auto” only by cable, not through the relay, when the Station offers it (`profile::pyrowave_format`).
- The Station's announcement (`pyrowave.available` of `GET /api/stream/info`) is kept with the computer (`Host.pyrowave`, null for a Station from before PyroWave).
- **Link bitrate** (`PyroWave.linkMbps`, `Native.sessionSetLinkMbps`): given to the core when the session starts and whenever the network changes (`registerDefaultNetworkCallback`), for the lowered resolution (`pyrowave::ceiling`). On Wi-Fi, the negotiated receive rate (`WifiInfo.getRxLinkSpeedMbps`, else `getLinkSpeed`); on Ethernet, 0 (unknown): Android only gives a fixed estimate (`getLinkDownstreamBandwidthKbps`), not the port's speed. The Station's ceiling (Admin's “Maximum bitrate”) and a bitrate set by hand count too.

## Interface (link-ui texts, `apps/android/scripts/strings.py`)

- **Settings › Picture › Video format**: “PyroWave” after AV1, only when the Station announces it; inactive with its reason in the help (“{name} doesn't offer PyroWave.”, “This device can't decode PyroWave.”). Help texts: PyroWave chosen, Auto by cable, Auto without cable. Chosen on Wi-Fi or away from home (computer reached through the relay): the amber warning under the choice. “Sharp text (4:4:4)”: “With PyroWave, almost free: only the bitrate goes up.” when PyroWave is chosen or taken by “Auto”. On TV, the same rows with focus.
- **Session**: chosen by hand, on Wi-Fi or through the relay, the start banner (“PyroWave on Wi-Fi” or “PyroWave away from home”, “The picture may stutter.”, “Switch to HEVC”, the close button; gone after 8 s). “Switch to HEVC” restarts the picture in HEVC for this session only (`Native.sessionSetSettings`). The amber chip in the bar (a tile in the TV menu), which opens the statistics.
- **Lowered resolution** (the core's rule and calculation, `apps/core/src/pyrowave.rs`): the core's notice (`onNotice` 9, `SessionListener.NOTICE_RESOLUTION_LOWERED`, the core's text) gives the banner “Resolution lowered to {w} × {h} to fit in {n} Mbit/s.” (8 s), folded into the PyroWave start banner when that one is showing; the notice without a resolution (back to the device's) clears it. Statistics show a line from `Second::lowered` (values 18 to 21 of `onSecond`).
- **The network is saturated** ([debit-automatique.en.md](debit-automatique.en.md)): the core's notice (`onNotice` 10, `SessionListener.NOTICE_PYROWAVE_STRAINED`) shows the same banner, titled “The network is saturated”, with “The picture may stutter.”, “Switch to HEVC” and the close button, gone after 8 s. Once per session, host only, on cable too; it replaces the start banner if that one is still showing.
- **Statistics**: “Lost frames” (as a % of the second's frames) and the lowered resolution.
- Tests: `PyroWaveTest` (announcement, choice, computer kept), `PyroWaveUiTest` (Compose: help texts, warnings, inactive value, banner, “Switch to HEVC”, chip, “Lost frames”, the core's lowered resolution in Statistics, its notice and its return, folded into the start banner, “The network is saturated” banner on phone and TV).

## Where PyroWave will be unavailable

- **Nvidia Shield TV** (Tegra X1, Android 11): unavailable, Android 11's Vulkan loader stops at 1.1. Inferred, not checked on the device: the probe will say so in the log.
- Any device on Android 12 or earlier, without Vulkan 1.3, or 32-bit (armeabi-v7a).
- **Emulator**: no test possible. With `-gpu host` (MoltenVK) or `-gpu lavapipe`, gfxstream brings the emulator down as soon as the decoder is created (“Failed to unbox VkSampler”, inside gfxstream, on Granite's samplers); with `-gpu swiftshader_indirect`, SwiftShader (subgroups of 4) refuses the decoder and the probe cleanly answers “unavailable”. The emulator on an Apple-silicon Mac runs arm64: no x86_64 image.
- **Redmi A5** (Unisoc T7250, Mali-G57, Android 15): probably available, on the fragment path; to be tried.

## Offline test on a device (`apps/android/tests/pyrowave/run.sh`)

Without a remote computer, on the connected device: the probe, then a test pattern (ffmpeg `testsrc2`, 120 frames, BT.709 limited range, 1.2 bits per pixel at 60 fps) encoded **on the device** by the upstream encoder (one frame = its packets back to back, like a Station media packet), then decoded and presented through the app's real path onto an `AImageReader` surface that is read back: first-frame RGB PSNR against the source, truncated frame skipped, mean time paced at 60 fps; in 4:2:0 and 4:4:4, then labelled HDR10 (R16 planes, PQ brought down to SDR). The upstream encoder needs more than the decoder: if it refuses the GPU, the test stops after the probe. The `kn_pyrowave_test` tool is built by `build-native.sh` in `target/pyrowave-android/<abi>/`.

### Steps for the author

On the Mac, in the branch's worktree (or after merging):

1. `apps/android/scripts/build-native.sh` (all three ABIs; PyroWave for arm64 and x86_64).
2. Redmi A5 connected over USB (USB debugging): `apps/android/tests/pyrowave/run.sh` (1080p) then `apps/android/tests/pyrowave/run.sh 1280x720`. Expected: “Sonde : PyroWave disponible (Mali-G57…, fragment)”, “Encodé : 120 images …”, “PyroWave (Vulkan, …, fragment), 1920x1080 4:2:0”, “Première image : PSNR … dB” (at least 30 dB), “Image tronquée au tiers : sautée”, the mean time, then “OK”, for 4:2:0, 4:4:4 and HDR10. Note the whole output.
3. The app: `cd apps/android && ./gradlew assembleDebug && adb install -r app/build/outputs/apk/debug/app-debug.apk`, open it, then `adb logcat -d -s KyberNow | grep PyroWave`: the probe line (“PyroWave : disponible (…)” or the reason).
4. Shield connected over the network (`adb connect <address>:5555`): `ANDROID_SERIAL=<address>:5555 apps/android/tests/pyrowave/run.sh`; expected: “Sonde : PyroWave indisponible (Vulkan loader 1.1, 1.3 needed)” (or another reason), nothing more. Then the installed app: the same log line, and no “PyroWave” in Settings › Picture › Video format… unless the Station announces it: it is then inactive, “This device can't decode PyroWave.”.
5. A real session, once the Station does PyroWave and the core is built with PyroWave (the default): Redmi on Wi-Fi, PyroWave chosen in Video format: banner, amber chip, “Switch to HEVC”; Statistics: “Decoding” and “Lost frames”; the decoder name.

## Test status

| What | Status |
|---|---|
| `libkybernow_pyrowave.so` arm64-v8a and x86_64 (NDK 30, API 30) | built (25/11/2026), linked to `libkybernow.so`, APK built with PyroWave |
| Probe on the emulator (SwiftShader) | answers “unavailable (… no decoder)” without crashing, app launched (25/11/2026) |
| Decoding, display, HDR10 on a real GPU | **to try** (Redmi A5: steps above) |
| Shield: probe unavailable | to confirm |
| Interface | `PyroWaveUiTest` Compose tests pass; not seen on a device |
| Full session to the Station | to try (patch 0025, applied by default) |
