# PyroWave: the Raspberry Pi

English · [Français](pyrowave-client-pi.md)

Client section of [pyrowave.en.md](pyrowave.en.md), for the Linux app on the Raspberry Pi (`apps/linux`): the .deb package on Raspberry Pi OS (labwc desktop) and the KyberNow Link OS image (`apps/linux/os`, KMS/DRM without a compositor). Desktop Linux client: [pyrowave-client-linux.en.md](pyrowave-client-linux.en.md); interface rules: [link-ui.en.md](link-ui.en.md), “PyroWave”.

**Status (25/11/2026)**: study made from PyroWave's code (a32b05e), Mesa's and the Raspberry Pi kernel's code, and the Raspberry Pi OS archive. The library builds for the Pi and ships in the .deb and the image, but the app still refuses it on the Pi (`system.rs`, `pyrowave_probe_start`): nothing changes for the user until step 2 (below) is written. **Nothing has run on a board.**

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

- `apps/linux/scripts/build.sh`: step 4 (PyroWave a32b05e, Vulkan path, `libpyrowave-shared.so.1`) also runs with `--rpi`. The headers in the prefix make `build.rs` compile `pyrowave.c` instead of `pyrowave_none.c`; the app still refuses the Pi until step 2 is written.
- `apps/linux/scripts/deb.sh`: the library goes to `/usr/lib/libpyrowave-shared.so.1` (stripped, 1.1 MB), its dependencies are read from it (`libstdc++6`, `libgcc-s1`); the library opens Vulkan itself (volk), so `libvulkan1` and `mesa-vulkan-drivers` are **recommended** (installed by default by `apt install ./…deb`). New `--prefix DIR` option (the dependency prefix, passed to `build.sh`); `--no-build` also takes the library from the prefix.
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

## Step 2 (after the Linux client is merged)

1. **PyroWave patch** (the dequantiser's `r16f`), applied by `build.sh` to its own clone (`$PREFIX/build/pyrowave`, never `third_party`), shaders regenerated with upstream's tool (`slangmosh.sh`); for all Linux builds (no effect outside the Pi); to be offered upstream.
2. **A Pi-specific probe**, in a separate process (`kybernow --pyrowave-probe`, like `pyrowave-device-validation` for the Windows Station: a crashing driver does not take the app down): model (`system::pi()`), Pi 5 / 500 / CM5 only; Vulkan 1.3, the v3dv driver (`VK_DRIVER_ID_MESA_V3DV`), subgroup operations, then the test frame decoded through the fragment path. Result kept per Mesa and app version (no probe at every launch).
3. **Fragment path forced** on v3dv (`pyrowave.c`).
4. **Interface**: PyroWave offered in Settings › Picture and the “This Raspberry Pi” panel when the probe succeeds, with its label; otherwise the inactive value and “This Raspberry Pi can't decode PyroWave.” (« Ce Raspberry Pi ne sait pas décoder PyroWave. »): link-ui's `{this device}` wording, made specific.
5. **Display** following the proposal above.
6. **Steps** for the author on a board (Pi 5 and Pi 4), written with step 2.

## Checking a board (before step 2)

On a Pi 5 under an up-to-date Raspberry Pi OS trixie (`sudo apt update && sudo apt full-upgrade`), or under KyberNow Link OS (SSH):

```sh
sudo apt install vulkan-tools          # already there in Link OS
vulkaninfo --summary                   # expected: “V3D 7.1…”, apiVersion 1.3, driverName v3dv, Mesa 25.3 or later
vulkaninfo | grep -A3 subgroupSupportedOperations   # expected on the Pi 5: ARITHMETIC in the list; on the Pi 4: BASIC alone
vulkaninfo | grep shaderStorageImageWriteWithoutFormat   # expected: false (hence the patch)
```

## Sources

- PyroWave a32b05e: `pyrowave.h`, `pyrowave_c.cpp`, `pyrowave_decoder.cpp`, `pyrowave_common.cpp`, `shaders/wavelet_dequant.comp`, `shaders/slangmosh.hpp`; Granite `vulkan/context.cpp`, `vulkan/device.cpp`.
- Mesa v3dv: https://gitlab.freedesktop.org/mesa/mesa/-/raw/mesa-26.2.7/src/broadcom/vulkan/v3dv_device.c (properties and features: subgroups, 8/16-bit, `shaderStorageImageWriteWithoutFormat`, memory types, compute limits; V3D before 4.2 refused), `…/mesa-26.2.7/src/broadcom/vulkan/v3dv_formats.c` (format features: no `STORAGE_WRITE_WITHOUT_FORMAT`, storage in `OPTIMAL` only), `…/mesa-26.2.7/src/broadcom/compiler/v3d_nir_lower_image_load_store.c` (stores: format required), `…/mesa-25.0.7/src/broadcom/vulkan/v3dv_device.c` (before ARITHMETIC), `…/main/src/broadcom/common/v3d_device_info.c` (V3D 4.2 and 7.1 only).
- Mesa release notes: https://docs.mesa3d.org/relnotes/24.3.0.html (Vulkan 1.3 on v3dv, Pi 4 and Pi 5), https://docs.mesa3d.org/relnotes/25.3.0.html (arithmetic subgroup operations), https://docs.mesa3d.org/relnotes/26.2.0.html (CLUSTERED, float16/int8, 32 KB shared).
- Conformance: https://www.khronos.org/conformance/adopters/conformant-products/vulkan (Raspberry Pi 4 Model B and Raspberry Pi 5, Vulkan 1.3).
- Raspberry Pi OS: http://archive.raspberrypi.com/debian/dists/trixie/main/binary-arm64/Packages.gz and `…/bookworm/…` (`mesa-vulkan-drivers` versions), https://github.com/RPi-Distro/pi-gen (`arm64` branch, stage 0 to 2 package lists).
- Raspberry Pi kernel: https://github.com/raspberrypi/linux/blob/rpi-6.12.y/drivers/gpu/drm/vc4/vc4_plane.c (plane YUV formats, SAND, `COLOR_ENCODING`, `COLOR_RANGE`), `…/drivers/gpu/drm/v3d/v3d_bo.c` (write-combined).
- Not read: vulkan.gpuinfo.org reports (list loaded by JavaScript); everything above comes from code, not from a board.
