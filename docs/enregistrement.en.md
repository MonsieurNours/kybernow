# Session recording

English · [Français](enregistrement.md)

The device in session records what it sees and hears into an MP4 file, on the computer. The apps' texts and interface are in [link-ui.en.md](link-ui.en.md), “Session recording”.

## The choices

Settled with the author on 26/11/2026:

| Question | Choice |
|---|---|
| Where | On the Station, from the picture and sound already encoded for the session: **no re-encoding**, no extra load on the graphics card. Every app benefits, browser and TVs included. |
| Who starts it | The host's device in session, and Admin. A share guest sees it without being able to stop it. During a lend, no recording: a lend that starts stops it. |
| Telling people | A red “Recording · 02:14” pill for everyone in the session, and a brief notice when it starts (or when joining a recorded session). No consent prompt: people are told. |
| Contents | The picture of the session's main display and the computer's sound. KyberNow carries no microphone: there is none. |
| Where the file goes | The computer user's Videos › KyberNow (Movies › KyberNow on a Mac, `XDG_VIDEOS_DIR` › KyberNow on Linux), changeable in Admin. A folder that is (or goes through) a link or a junction is refused: Kyber writes there with the system's rights. |
| How long | 30 days by default (7, 30, 90 days or forever, in Admin); stops under 2 GB free. |
| Always record | No: by hand only. |
| Allowed | Yes by default (“Allow session recording” in Admin). |

## How it works

```
app (host)  ──POST /api/stream/recording──▶  Admin  ──POST /kybernow/recording──▶  Kyber (kycontroller)
     ▲                                          │                                          │
     └──── heartbeat: “recording” ◀─────────────┘           host's picture + sound ──▶ MP4 ──┘
```

1. **The app** asks (`POST /api/stream/recording` `{"record": true, "device_id"}`), like the host's other calls. Admin checks that it is the device in session, that recording is allowed, that no lend is running and that 2 GB are left.
2. **Admin** gives Kyber the folder and the file name (`KyberNow 2026-11-26 14-32-05`, in the computer's time) through its reserved route (`POST /kybernow/recording`, Admin's token, from the computer only).
3. **Kyber** (patch 0070, controller shared by the three Stations) reads the host's picture of the main display and the shared sound **as a share guest would**: it never holds them back (a slow disk loses frames and resumes at the next keyframe; the session does not suffer). A thread of its own writes a fragmented MP4: H.264 (`avc1`) or HEVC (`hvc1`, 8 or 10 bits, HDR included) and Opus, one fragment a second, written as it goes. A file cut short (power loss) plays up to its last second; a closed file carries its duration and an index to seek in it.
4. **Everyone** sees the state in their heartbeat: the host (`POST /api/stream/heartbeat`) and the guests (`POST /api/share/beat`), `recording` field. The time is counted by the Station (`elapsed`): every device shows the same.

Kyber asks nothing of the encoder but a keyframe at the start, after a loss, and every 10 seconds (to seek in the file). With intra refresh (the usual setting), these keyframes are the only ones: a little heavier than other frames, one every 10 s.

**End**: the host or Admin stops it; the host's session ends (3 s without its picture); the disk falls under 2 GB (Admin checks every 2 s) or writing fails; a lend starts; the profile switches to PyroWave or AV1; Admin turns recording off. A picture that changes size or format (a profile chosen in session) goes on in a new file, `… (2).mp4`.

**Not recorded**: PyroWave and AV1 (refused when starting, with the reason; stops if the session switches to them); the cursor, which the apps draw themselves (it is not in the picture sent); the other displays of a multi-display session.

## Exchanges

### The app and Admin

`POST /api/stream/recording` `{"record": true|false, "device_id": "…"}` (the device's account, like sharing): returns the state, or a refusal `409 {"error", "code"}`:

| `code` | When | Text (link-ui) |
|---|---|---|
| `disabled` | turned off in Admin | “Recording is turned off on this computer (Admin › Recordings).” |
| `not_in_session` | the caller is not the device in session | “Only the device in a session can record.” |
| `no_session` | nobody in session | “No session to record.” |
| `lent` | computer lent | “{name} is using the computer.” |
| `codec` | PyroWave or AV1 | “PyroWave and AV1 can't be recorded: switch to HEVC or H.264.” |
| `disk_full` | under 2 GB | “Less than 2 GB free on the computer: can't record.” |
| `folder` | folder can't be made | “The recordings folder can't be reached (Admin › Recordings).” |
| `unavailable` | Kyber too old (404) | “Kyber doesn't record on this computer yet: update KyberNow Station.” |
| `failed` | other | “Recording could not start.” |

The state (host's and guest's heartbeat, answer above):

```json
{"allowed": true, "active": true, "elapsed": 134, "started_at": 1764166325000,
 "last_end": {"reason": "stopped", "at": 1764166000000}}
```

`last_end.reason`: `stopped`, `ended` (session over), `write`, `disk_full`, `codec`, `lent`. A Station that does not record sends no `recording`: the apps show nothing.

### Admin

| Route | |
|---|---|
| `GET /api/recording` | settings (`allowed`, `folder`, `custom_folder`, `default_folder`, `keep_days`), state, device that started it (`by`), device in session (`session`), free space (`free`), files (`name`, `size`, `modified`, `duration_ms` read in the file, `in_progress`) |
| `PUT /api/recording/settings` | `{allowed?, folder? ("": default), keep_days? (7, 30, 90, 0 = forever)}` |
| `POST /api/recording` | `{record}`: Admin starts or stops recording the current session |
| `GET /api/recording/files/{name}` | the file, in pieces (`Range`, for the page's player); `?download=1` to save it |
| `DELETE /api/recording/files/{name}` | deletes (not the one being written) |

Admin settings, `[recording]`: `allowed` (true by default), `folder` (absent: default), `keep_days` (30). Every hour, and whenever a setting changes, Admin deletes files older than the chosen duration: only those whose name starts with “KyberNow” (never another video in the folder), never the one being written. Admin restarted during a recording picks it up again (it reads Kyber back).

Admin's feed says “{device} is recording the session”, “Recording saved ({duration})”, “Recording stopped: disk full”.

### Admin and Kyber (patch 0070)

`POST /kybernow/recording` `{"record": true, "dir": "…", "name": "…"}` or `{"record": false}`; `GET /kybernow/recording`: `{"active", "started_at_ms", "display", "files", "bytes", "last_end": {"reason", "at_ms"}}`. Refusals `409 {"code"}`: `no_session`, `codec`, `folder`. Like the other `/kybernow` routes, from the computer only and with Admin's token.

## By system

| | Windows | Mac | Linux |
|---|---|---|---|
| Default folder | `C:\Users\{account}\Videos\KyberNow` (the open session's profile: Admin runs as a service) | `~/Movies/KyberNow` | `XDG_VIDEOS_DIR/KyberNow` (`~/.config/user-dirs.dirs`), else `~/Videos/KyberNow` |
| Free space | `GetDiskFreeSpaceExW` | `statvfs` | `statvfs` |
| Written by | Kyber (`kycontroller`), in the user's folder | Kyber, with the user's account | Kyber, with the user's account |

A Videos folder moved on Windows (OneDrive, another disk) is not followed: choose it in Admin.

## Tests

- **Done**: the MP4 writer on real streams (x264 1080p, VideoToolbox HEVC 8-bit 1080p and 10-bit 1440p, Opus): read and sought by ffprobe/ffmpeg and by AVFoundation (duration, tracks, decoded frame), unit tests (parameter sets, Opus durations, fragments, duration and index at closing); `cargo test` of kycontroller, Admin and the core.
- **To try on a Station** (Windows, Mac, Linux): start from an app, from Admin; pill for a guest; file played in QuickTime, Windows' player, VLC, Admin's page; profile change in session (`… (2).mp4`); switching to PyroWave (stops); nearly full disk; a lend that starts; Admin restarted during a recording; default folder on Windows (service under SYSTEM); HDR; 5.1.
