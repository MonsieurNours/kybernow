# Prototype A — results

English · [Français](prototype-A-results.md)

> **Archive.** Snapshot of 19 October 2026: it describes the project as it was then, not as it is now. The current state is in the living docs; planned work is in [ROADMAP.en.md](../../ROADMAP.en.md).

## Run 1 — reference (2026-10-19 16:41, `tools/latency/runs/20261019-164146`)

**Configuration**: Kyber 0.29.0 server (official installer) on a Windows NVIDIA computer `192.168.1.100`, NVENC, h264, default bitrate. `kyclient` 0.29.0 client built locally, MacBook Pro M4 Max. Network: QUIC RTT 0.66 ms (LAN). Computer's display: **4K at 160 Hz**.

**Conclusion: prototype A validated.** The server and the client work end to end, with a latency in line with Kyber's claims. This run is the reference for comparing our own clients (KyberKit, macOS and tvOS apps). No other measurements planned until then.

| step | p50 | p95 | p99 | max (ms) |
|---|---|---|---|---|
| encoding (acquired → encoded) | 5.19 | 9.84 | 36.37 | 307.92 |
| sending (encoded → sent) | 0.12 | 0.44 | 0.94 | 1.42 |
| network (sent → received) | 0.66 | 1.30 | 2.01 | 12.84 |
| decoding (received → decoded) | 2.50 | 4.75 | 14.51 | 62.31 |
| display (decoded → displayed) | 0.62 | 0.85 | 1.48 | 4.30 |
| **TOTAL (acquired → displayed)** | **9.36** | **17.39** | **53.77** | **316.44** |

3,263 frames analysed, 59 fps displayed, no frame skipped. Clock alignment: spread 0.22 ms.

### Reading
- **The median (9.4 ms) matches the 8–10 ms announced by Kyber.** The network is negligible on a LAN.
- **Encoding dominates**: 5.2 ms median, in 4K. This is consistent with the 2.5 ms quoted by Kyber at 1080p, for four times fewer pixels.
- **Encoding spikes follow the moments when the screen stayed still**: after a gap of 200 ms or more without a new frame, encoding rises to p95 = 107 ms, against 9 ms in a continuous stream. This is the signature of a GPU lowering its clocks when idle and then having to ramp up.
- **Decoding spikes on the Mac come in bursts** (6 to 8 frames in a row at the same moment): large frames (scene change, keyframe) rather than a regular problem.
- The median capture interval is 6.3 ms, which matches the display at 160 Hz.
- The session ended with “closed by peer” on the server: to check whether this was the window being closed on purpose.
