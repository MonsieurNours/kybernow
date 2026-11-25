# PyroWave: the Raspberry Pi

English · [Français](pyrowave-client-pi.md)

Client section of [pyrowave.en.md](pyrowave.en.md), for the Linux app on the Raspberry Pi (`apps/linux`): the .deb package on Raspberry Pi OS (labwc desktop) and the KyberNow Link OS image (`apps/linux/os`, KMS/DRM without a compositor). Desktop Linux client: [pyrowave-client-linux.en.md](pyrowave-client-linux.en.md); interface rules: [link-ui.en.md](link-ui.en.md), “PyroWave”.

**Status (25/11/2026)**: step 1 (study from the code of PyroWave a32b05e, Mesa and the Raspberry Pi kernel, and the Raspberry Pi OS archive; build) and step 2 (Pi probe, dequantiser patch, fragment path, interface) written. Checked in the “KyberNow Linux” VM (below). **Nothing has run on a board**: the “Steps on a board” will do it.

## Verdict per board

| Board | GPU | PyroWave |
|---|---|---|
| Pi 5, Pi 500, CM5 | V3D 7.1 (BCM2712) | **Likely**, on three conditions: an up-to-date Raspberry Pi OS trixie (Mesa 25.3 or later; 26.2.7 today), the decoder's fragment path, and a one-line patch to PyroWave's shaders (below). With PyroWave a32b05e as is: impossible. On bookworm (Mesa 24.2): impossible. Speed never measured. |
| Pi 4, Pi 400, CM4 | V3D 4.2 (BCM2711) | **Impossible**: the driver only has basic subgroup operations, the decoder needs six. Upstream's dequantiser would have to be rewritten without subgroups. |
| Pi 3, Zero 2 W | VideoCore IV | **Impossible**: no Vulkan driver (v3dv only takes V3D 4.2 and 7.1). |

## What the decoder needs (PyroWave a32b05e)

Read in upstream's code (`third_party/pyrowave`); the encoder needs more, but the Pi only decodes.

- **Vulkan 1.1** at least (`pyrowave_c.cpp:115`, Granite `context.cpp:918`), with subgroup size control and `computeFullSubgroups` (Vulkan 1.3 in practice, Granite `device.cpp:5916`), a size between 4 and 128 (`pyrowave_decoder.cpp:1016`).
- **Subgroup operations** BASIC, VOTE, BALLOT, ARITHMETIC, SHUFFLE and SHUFFLE_RELATIVE, otherwise `pyrowave_decoder_create` refuses (`pyrowave_decoder.cpp:999`).
- **8-bit storage** (`storageBuffer8BitAccess`) **or** texel buffers of at least 16 M elements; outside AMD, Intel and NVIDIA, upstream takes texel buffers anyway (`pyrowave_common.cpp:268`).
- `shaderInt16`: encoder only (`pyrowave_encoder.cpp:1254`). `shaderFloat16`: optional; the default build (`PYROWAVE_FP32_MATH`, precision 1) computes in FP32 and only keeps intermediate storage in 16 bits (`R16_SFLOAT`).
- **Writes to images without a format**: the embedded SPIR-V (`shaders/slangmosh.hpp`, disassembled with `spirv-dis` on 25/11) declares `StorageImageWriteWithoutFormat` in the dequantiser's three variants (`wavelet_dequant.comp`, an `R16_SFLOAT` image) and in the compute IDWT (`idwt.comp`); not in the fragment IDWT (`idwt.frag`).
- **Fragment path** (mobile GPUs weak at compute): the IDWT goes through render passes (`R16_SFLOAT`, `R16G16_SFLOAT`, then the `R8_UNORM` output) instead of compute shaders; the dequantiser stays in compute. Upstream only picks it for Qualcomm, ARM and PanVK (`pyrowave_decoder.cpp:916`); the app chooses it (`fragment_path` in `pyrowave_decoder_create_info`), and the readback to the CPU supports it.

## What Mesa v3dv provides

| Item | Pi 5 (V3D 7.1) | Pi 4 (V3D 4.2) |
|---|---|---|
| Mesa in Raspberry Pi OS (arm64) | trixie: **26.2.7-1~bpo13+0~rpt1** (Raspberry Pi archive, above Debian 13's 25.0.7); bookworm: 24.2.8-1~bpo12+rpt5 | same |
| Vulkan | 1.3 since Mesa 24.3; 1.3 conformant (Khronos, 24/07/2024, CTS 1.3.8.3) | 1.3 since Mesa 24.3; 1.3 conformant |
| Subgroup size | 16 (min = max), compute and fragment | 16 |
| Size control, `computeFullSubgroups` | yes (compute) | yes (compute) |
| Subgroup operations | BASIC, VOTE, BALLOT, SHUFFLE, SHUFFLE_RELATIVE, QUAD; **ARITHMETIC since Mesa 25.3**; CLUSTERED since 26.2 | **BASIC only** |
| 8-bit and 16-bit storage | yes | yes |
| `maxTexelBufferElements` | 2^28 | 2^28 |
| `shaderInt16`, `shaderFloat16` | since Mesa 26.2 (the latter emulated) | no |
| Image writes without a format | **no**: `shaderStorageImageWriteWithoutFormat` false, no format has `STORAGE_WRITE_WITHOUT_FORMAT`, and the compiler requires a format on stores | **no** |
| Storage images | `OPTIMAL` tiling only (never `LINEAR`) | same |
| Memory | a single type, `DEVICE_LOCAL \| HOST_VISIBLE \| HOST_COHERENT`, never `HOST_CACHED`; the kernel maps v3d buffers write-combined | same |
| Compute | 256 invocations per workgroup; 32 KB shared since Mesa 26.2 (16 KB before) | same |
| dmabuf, DRM modifiers, `VK_KHR_display`, `VK_EXT_acquire_drm_display` | yes | yes |

Consequences:

- **Pi 4**: refused when the decoder is created (subgroups), cleanly, without crashing.
- **Pi 5**: everything is there except writes without a format. The compute path uses them twice (dequantiser, IDWT), the fragment path once (the dequantiser). Hence the patch: `layout(set = 0, binding = 0, r16f) writeonly uniform image2DArray uDequantImg;` in `wavelet_dequant.comp` (`r32f` at precision 2), shaders recompiled. Tried on 25/11 on a copy: the three variants compiled by glslang no longer have `StorageImageWriteWithoutFormat` (an `R16f` image) and pass `spirv-val`. That format is the image's own on every GPU: the patch changes nothing elsewhere and can be offered upstream.
- **Without the patch**, on the Pi 5, the pipeline is invalid for Vulkan; Mesa built without assertions would write garbage, or crash. The probe must therefore check the feature before creating the decoder, and run outside the app's process (below).
- **CPU readback** (what `pyrowave.c` does on Linux): GPU memory is never cached on the CPU side, so reading a frame back is slow (not measured). Hence the display proposal without readback.
- **Bitrate**: 1 Gbit/s Ethernet on the Pi 4 and Pi 5 (the core's lowered resolution keeps the bitrate within the port's speed); it is the decoding speed, unknown, that will set the resolution offered to the Pi (1080p targeted, like the Pi's 4:4:4).
- **HDR**: not relevant (the Pi is SDR).

## Build (step 1, done)

- `apps/linux/scripts/build.sh`: step 4 (PyroWave a32b05e, Vulkan path, `libpyrowave-shared.so.1`) also runs with `--rpi`. The headers in the prefix make `build.rs` compile `pyrowave.c` instead of `pyrowave_none.c`. The dequantiser is patched there (step 2).
- `apps/linux/scripts/deb.sh`: the library goes to `/usr/lib/libpyrowave-shared.so.1` (stripped, 1.1 MB), its dependencies are read from it (`libstdc++6`, `libgcc-s1`); the library opens Vulkan itself (volk), so `libvulkan1` and `mesa-vulkan-drivers` are **recommended** (installed by default by `apt install ./…deb`). Options `--prefix DIR` (the dependency prefix, passed to `build.sh`) and `--kyber-pyrowave` (passed to `build.sh`: kyclient patch 0025 and the core's `pyrowave` feature, without which the app decodes PyroWave but never requests it); `--no-build` also takes the library from the prefix.
- **KyberNow Link OS**: Raspberry Pi OS Lite has neither `libvulkan1` nor `mesa-vulkan-drivers` (pi-gen's package lists, stages 0 to 2) and our stage installs the .deb without recommends: `stage-kybernow/00-packages` adds `libvulkan1`, `mesa-vulkan-drivers` and `vulkan-tools` (`vulkaninfo`, for diagnostics). The `kybernow` account is already in the `render` group (`/dev/dri/renderD128`).
- **Checked on 25/11 in the “KyberNow Linux” VM** (private prefix, not `third_party`): PyroWave built in the Pi prefix (aarch64, `NEEDED`: libstdc++, libm, libgcc_s, libc); a .deb assembled (`--no-build`, a Pi binary from an earlier build) with the library, `libstdc++6 (>= 12)` in `Depends`, `libvulkan1, mesa-vulkan-drivers` in `Recommends`. **Not checked**: the app itself (the `claude/pyrowave` branch does not build for Linux today: `build.sh` applies patch 0025, which adds `fallback_codec` to kyclient, without the core's `pyrowave` feature, and the app does not build with it either; work in progress elsewhere), the image (the packages exist in trixie).

## Display in Link OS (proposal, not written)

Today, on the Pi, the V4L2 decoder's HEVC frame is copied by the CPU (SAND into linear NV12, `av_hwframe_transfer_data`) and handed to SDL (a GLES texture on KMS/DRM); Linux's PyroWave reads its planes back the same way. On the Pi, that readback reads uncached memory: to avoid.

**Recommended: a DRM plane through dmabuf.**

1. **Buffers**: 3 or 4 linear buffers per resolution, allocated through GBM (or as dumb buffers) in `DRM_FORMAT_YUV420` (three planes in one buffer), exported as dmabuf, imported into Vulkan as a **VkBuffer** (`VK_EXT_external_memory_dma_buf`): v3dv does not accept a linear image for storage or sampling, a buffer it does.
2. **Decode** on our own `VkDevice` (`pyrowave_create_device` with our handles, dmabuf extensions enabled), fragment path, into PyroWave `R8_UNORM` images (`pyrowave_image_create`, optimal tiling) through `pyrowave_decoder_decode_gpu_buffer`, then in the same command buffer `vkCmdCopyImageToBuffer` of each plane into the dmabuf (a GPU copy, never a CPU one). Completion signalled by a fence (or an exported `sync_file`).
3. **Show**: SDL kmsdrm gives its DRM descriptor (`SDL_GetWindowWMInfo`, `info.kmsdrm.drm_fd`, the same DRM master); the app puts the buffer on an **overlay plane** (`drmModeSetPlane` or an atomic commit) with `COLOR_ENCODING` = BT.709 and `COLOR_RANGE` = full (PyroWave's SDR contract). The Pi 4 and Pi 5 HVS takes linear YUV420 and NV12 on all its planes, scales and converts to RGB at scanout: no render pass. The video plane goes under SDL's primary plane (`zpos`), which the session interface (bar, statistics, banners) draws transparent where the picture must show (ARGB8888, already accepted by vc4). A buffer goes back to the pool only after the next flip.
4. **The same path for HEVC**: the HVS also takes NV12 in SAND128: the V4L2 decoder's `DRM_PRIME` frames can go on that plane without today's copy (the “zero-copy SAND → DRM plane” path left open since the Pi's milestone 1).
5. **On the desktop** (.deb under labwc): planes belong to the compositor. Same Vulkan side, then the dmabuf imported into SDL's GLES (`EGL_EXT_image_dma_buf_import`, YUV420), or a `linux-dmabuf` Wayland subsurface that wlroots can scan out directly.

**Ruled out: `VK_KHR_display`** (Vulkan owning the screen, `VK_EXT_acquire_drm_display`). SDL kmsdrm and Vulkan cannot both be master: the whole Link interface (home, settings, session) would have to go through a Vulkan swapchain, for Link OS only, and a YUV → RGB GPU pass would still be needed. Much more work, for less than the plane.

**Proposed order**: first today's readback (nothing more to write for display), to measure on a board the decode and the readback at 1080p60; the DRM plane next, all the sooner as the readback costs (above 2 ms, it alone justifies the plane).

## Step 2 (written)

1. **PyroWave patch** (`apps/linux/pyrowave/v3dv-dequant.py`, run by `build.sh` on its clone, `$PREFIX/build/pyrowave`, never `third_party`): `layout(r16f)` on `uDequantImg` in `wavelet_dequant.comp`, and the same change in the already compiled SPIR-V of `shaders/slangmosh.hpp`, in place and at equal size (image format `Unknown` → `R16f`, capability `StorageImageWriteWithoutFormat` → `StorageImageExtendedFormats`, the one glslang emits for the patched source): 6 words across the three variants, nothing else moves, `spirv-val` accepts them. The script checks the SPIR-V's shape and refuses if it does not recognise it (another commit). For all Linux builds: no effect outside the Pi. `PYROWAVE_PATCHES` (`build.sh`) rebuilds a library compiled without it (`$PREFIX/lib/.pyrowave-v3dv-dequant-1`). Regenerating the shaders with upstream's tool (`slangmosh.sh`) would have meant building Granite's glslang and SPIRV-Tools.
2. **Fragment path forced on v3dv** (`pyrowave.c`, `fragment_path`): the driver is recognised through `VkPhysicalDeviceDriverProperties` (`VK_DRIVER_ID_MESA_V3DV`); HDR10 refused on v3dv (the fragment path has no 10-bit output). `KYBERNOW_PYROWAVE_FRAGMENT=1` (or `0`) forces it elsewhere, for tests.
3. **Probe** (`device/system.rs`, `pyrowave_probe_start`):
   - Pi 4, 400, CM4, Pi 3, Zero 2 W: unavailable without trying anything (log: “PyroWave : indisponible sur Raspberry Pi 4 Model B (pas les opérations de sous-groupe de Vulkan (V3D 4.2) ; Raspberry Pi 5 seulement)”);
   - Pi 5, 500, CM5: first **in a separate process**, `kybernow --pyrowave-probe` (30 s at most; it prints “ok” or “no”, the GPU and the reason), so that a crashing driver does not take the app down. Its result is kept in `pyrowave-probe` (the app's folder: `/var/lib/kybernow/config/kybernow` under Link OS, `~/.config/kybernow` under Raspberry Pi OS) as long as the app version and the Vulkan driver (files in `/usr/share/vulkan/icd.d` and `libvulkan_broadcom.so`, size and date) do not change. When it succeeds, the probe is run again inside the app (the device and its pipelines are ready before the first session);
   - everywhere, `pyrowave.c` also checks subgroup operations and gives the reason (“opérations de sous-groupe manquantes (0x… au lieu de 0x7f…)”); the GPU in the log names the driver (“V3D 7.1… (Vulkan 1.3, Mesa 26.2.7…)”).
4. **Interface** ([link-ui.en.md](link-ui.en.md), “PyroWave”): Settings › Picture › Video format offers PyroWave when the computer offers it; its label is “graphics card” (on the Pi too: V3D, not the Pi's decoder); the Pi's “This device” card adds “PyroWave by the graphics card”. Inactive value: “This Raspberry Pi can't decode PyroWave: it takes a Raspberry Pi 5.” (Pi 4, 3, Zero 2 W), “…: it needs an up-to-date Raspberry Pi OS “Trixie”.” (Pi 5 on Raspberry Pi OS), “This Raspberry Pi can't decode PyroWave.” (Pi 5 on Link OS). Screenshots: `--preview`, `rpi-46-pyrowave-carte`, `rpi-46b-pyrowave-image`, `rpi-46c-pyrowave-inactif-pi5`, `rpi-46d-pyrowave-inactif-pi4`.
5. **Display**: CPU readback (the desktop Linux path, nothing more to write), as the proposal's order says: it gets measured on a board (the kit below); the DRM plane comes next if it costs more than 2 ms at 1080p.
6. **Requested from Kyber**: only with `--kyber-pyrowave` (`deb.sh`, `build.sh`), like the other apps while patch 0025 is not on everywhere.

### Checked in the VM (25/11/2026)

“KyberNow Linux” VM (Ubuntu 24.04 ARM64, lavapipe: no v3dv), private Pi prefix (`build.sh --rpi --kyber-pyrowave --prefix …`):

- the Pi app builds; 153 `kybernow-linux` tests pass; the .deb assembles (patched `/usr/lib/libpyrowave-shared.so.1`, `Recommends: … libvulkan1, mesa-vulkan-drivers`);
- `kybernow --pyrowave-probe`: “no”, “rendu logiciel seulement (llvmpipe …)”, code 1; with `KYBERNOW_PYROWAVE_CPU=1`: “ok”, code 0;
- the **fragment path with the patched dequantiser** decodes correctly (`KYBERNOW_PYROWAVE_FRAGMENT=1`, streams from the Mac's Metal encoder): 1920 × 1080 4:2:0, PSNR Y 57.1 dB (57.4 through the compute path); 1280 × 720 4:2:0, 51.8 dB; 4:4:4, middle pixel right; truncated frame skipped, the next one decoded. Timings are the CPU's (lavapipe), unrelated to the Pi.
- Not checkable without a board: v3dv itself (that the patched SPIR-V compiles and runs there), speed, readback from uncached memory, the KyberNow Link OS image.

## Steps on a board

A Raspberry Pi 5 (or 500, CM5) on **Ethernet**, and a Pi 4 if you have one (to see the refusal). A Station that offers PyroWave (patch 0025 and `[kyavserver] pyrowave`).

**A. Prepare, on the Mac and in the VM**

1. The 1080p test streams, on the Mac: `apps/linux/tests/pyrowave/samples-mac.sh ~/pw-samples-1080 1920x1080 60`.
2. In the VM: the package with PyroWave requested from Kyber, `apps/linux/scripts/deb.sh --kyber-pyrowave` (→ `apps/linux/dist/KyberNow-Link-RaspberryPi-arm64.deb`).
3. In the VM: the measuring kit, `apps/linux/tests/pyrowave/pi-kit.sh STREAMS_FOLDER ~/pw-kit` (copy the Mac's streams into the VM first; `--444` adds 4:4:4, 370 MB more).
4. For Link OS: `apps/linux/os/build-image.sh` with that .deb (empty pi-gen's cache first if it holds a previous image).

**B. Raspberry Pi OS (Trixie, 64-bit), on the Pi 5**

1. `sudo apt update && sudo apt full-upgrade`, then reboot.
2. `sudo apt install ./KyberNow-Link-RaspberryPi-arm64.deb vulkan-tools` (the package pulls in `libvulkan1` and `mesa-vulkan-drivers`).
3. `vulkaninfo --summary`: expected “V3D 7.1…”, the v3dv driver (`driverName`), `apiVersion = 1.3…`, Mesa 25.3 or later. Note the `driverInfo` line.
4. `kybernow --pyrowave-probe; echo $?`: expected “ok”, the GPU, then `0`. Anything else: copy the full output (it answers “does the patched SPIR-V pass on v3dv?”).
5. The kit: `scp -r ~/pw-kit <user>@<pi>:` from the VM, then on the Pi `~/pw-kit/mesure.sh`. Note for each stream: PSNR, “décodage, relecture comprise, cadencé à 60 i/s : moyenne … médiane … maximum … ; à la chaîne : … i/s”. The readback alone is not split out: the whole average decides (under 6 ms at 1080p60: good; above, the DRM plane).
6. Launch KyberNow Link from the menu, App settings: the “This device” card says “PyroWave by the graphics card”. The log (`~/.config/kybernow/kybernow.log`): “PyroWave : la sonde à part a réussi” then “PyroWave : prêt sur V3D 7.1…”.
7. Computer settings › Picture › Video format › PyroWave (label “graphics card”), then a session over Ethernet: the picture, then Statistics (Decoding, Lost frames). A session over Wi-Fi: the “PyroWave on Wi-Fi” banner, “Switch to HEVC”.
8. Relaunch the app: the log says “(gardée)” (no new separate probe).

**C. KyberNow Link OS, on the Pi 5**

1. Flash the image from A.4, boot, go through first boot.
2. SSH (key dropped on `BOOTA`, see the OS README): `vulkaninfo --summary`, `kybernow --pyrowave-probe; echo $?` (as B.3 and B.4), `grep -i pyrowave /var/lib/kybernow/config/kybernow/kybernow.log`.
3. With the controller: Computer settings › Picture › Video format › PyroWave, a session over Ethernet, the statistics.

**D. On a Pi 4 (Raspberry Pi OS or Link OS)**

1. The app does not run `kybernow --pyrowave-probe`: the log says “PyroWave : indisponible sur Raspberry Pi 4 Model B (pas les opérations de sous-groupe de Vulkan (V3D 4.2) ; Raspberry Pi 5 seulement)”.
2. Video format: PyroWave greyed out, “This Raspberry Pi can't decode PyroWave: it takes a Raspberry Pi 5.”. Run by hand, `kybernow --pyrowave-probe` must say “no” and “opérations de sous-groupe manquantes”, without crashing.

To report: the outputs of B.3, B.4, B.5 and C.2, and what the session shows (picture right or not, stutter).

## Sources

- PyroWave a32b05e: `pyrowave.h`, `pyrowave_c.cpp`, `pyrowave_decoder.cpp`, `pyrowave_common.cpp`, `shaders/wavelet_dequant.comp`, `shaders/slangmosh.hpp`; Granite `vulkan/context.cpp`, `vulkan/device.cpp`.
- Mesa v3dv: https://gitlab.freedesktop.org/mesa/mesa/-/raw/mesa-26.2.7/src/broadcom/vulkan/v3dv_device.c (properties and features: subgroups, 8/16-bit, `shaderStorageImageWriteWithoutFormat`, memory types, compute limits; V3D before 4.2 refused), `…/mesa-26.2.7/src/broadcom/vulkan/v3dv_formats.c` (format features: no `STORAGE_WRITE_WITHOUT_FORMAT`, storage in `OPTIMAL` only), `…/mesa-26.2.7/src/broadcom/compiler/v3d_nir_lower_image_load_store.c` (stores: format required), `…/mesa-25.0.7/src/broadcom/vulkan/v3dv_device.c` (before ARITHMETIC), `…/main/src/broadcom/common/v3d_device_info.c` (V3D 4.2 and 7.1 only).
- Mesa release notes: https://docs.mesa3d.org/relnotes/24.3.0.html (Vulkan 1.3 on v3dv, Pi 4 and Pi 5), https://docs.mesa3d.org/relnotes/25.3.0.html (arithmetic subgroup operations), https://docs.mesa3d.org/relnotes/26.2.0.html (CLUSTERED, float16/int8, 32 KB shared).
- Conformance: https://www.khronos.org/conformance/adopters/conformant-products/vulkan (Raspberry Pi 4 Model B and Raspberry Pi 5, Vulkan 1.3).
- Raspberry Pi OS: http://archive.raspberrypi.com/debian/dists/trixie/main/binary-arm64/Packages.gz and `…/bookworm/…` (`mesa-vulkan-drivers` versions), https://github.com/RPi-Distro/pi-gen (`arm64` branch, stage 0 to 2 package lists).
- Raspberry Pi kernel: https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/gpu/drm/vc4/vc4_plane.c (plane YUV formats, SAND, `COLOR_ENCODING`, `COLOR_RANGE`), `…/drivers/gpu/drm/v3d/v3d_bo.c` (write-combined).
- Not read: vulkan.gpuinfo.org reports (list loaded by JavaScript); everything above comes from code, not from a board.
