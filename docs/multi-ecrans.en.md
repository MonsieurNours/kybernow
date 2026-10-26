# Multiple displays

English · [Français](multi-ecrans.md)

A KyberNow session can take all of the device's displays: the computer creates one virtual display per display of the device, laid out the same way, and each of the device's displays shows its own in a full-screen window. This document is the shared contract between the Station, the Rust core and the Mac, Windows and Linux apps. The other Link apps (iPhone, iPad, Apple TV, Android, LG TVs, web client) stay on one display and have nothing to change.

Multiple displays have no screens of their own: they reuse the home screen, connection, session bar, settings and statistics of [link-ui.en.md](link-ui.en.md), which gives the exact texts (“Several displays” in the connection section, “Displays” row in the settings). The three apps behave the same way and use the same words; a new text goes into `link-ui.md` first, then into each app's catalogue, word for word.

## What the user sees

- **Per-computer setting** “Displays” (Picture section, under Resolution): Auto, This display, All displays. **Auto** (the default) takes all displays in the Desktop and Max profiles, one in Game. Changing this setting does not turn the profile into “Custom”. Inactive, on “This display”, with a custom resolution or without a virtual display.
- **Home screen**: the display line says what the session will take (“3 displays, sized for this device's screens”), with the same calculation as the session.
- **Connection**: the connection screen on the main display, the same background without text on the others, each display switches to its picture at its first frame.
- **Displays taken**: all of the device's active displays, four at most; mirrored displays count as one. “This display” is the display the app's window is on, like a one-display session.
- **The device's main display** (Mac: the one with the menu bar; Windows: the primary display; Linux: SDL display 0) becomes Windows' primary display: the taskbar and full-screen games go there.
- **Resolution**: each virtual display has the device display's resolution in pixels, like “Match this display”; “4K at most” applies to each display; in H.264, 4096 pixels wide at most per display.
- **Refresh rate**: the main display keeps its own; the others run at 60 Hz (less if their display does not go that high). A still display costs almost nothing: Kyber only sends a frame when the display changes.
- **Windows**: one full-screen window per display of the device. With the desktop mouse, the pointer moves from one display to the next as on the device. With the game mouse, it stays captured in the active window.
- **Session bar**: a single one, on the display the pointer is on. Its commands (end session, mouse, Keys, Statistics) apply to the whole session, from any window. “Full screen” is not offered: the windows stay full screen (“This display” to work in a window). Ending the session closes every window.
- **The device's displays change during the session** (plugged in, unplugged, layout or mirroring changed): the session is set up again with the new displays, without going back to the home screen; the connection starts again at its first step on the main display.
- **A display the computer could not create**: its window keeps the connection background with a single line, “The computer could not create this display.”; the other displays work.
- **A Station that handles only one display** (earlier version): the session runs on the main display, with no blocking message.

## Display layout

The core works out where each virtual display goes from the device's layout (one function shared by the three apps, with tests). Input: for each of the device's displays, its frame in the device's logical space (x, y, width, height, y pointing down; the Mac app converts from Cocoa's coordinates), its resolution in pixels (after “4K at most”), its refresh rate and whether it is the main one. Output: each display's position in pixels on the Windows desktop.

- The main display is at (0, 0).
- The others are placed step by step, starting from the main one: each sticks, on the same side (right, left, above, below), to an already placed neighbour it shares an edge with on the device; its offset along that edge is the device's, converted with the neighbour's pixels-per-logical-unit scale.
- Two placed displays share at least one edge segment (Windows requires it) and never overlap: a display that would overlap slides along the edge until it no longer does.
- A display that touches no neighbour on the device (rare) sticks to the nearest one, on the side of the largest gap.

Example (the author's test setup): XG32UQ 4K at 160 Hz in the middle (main), Studio Display 5K on the right, the MacBook Pro's display on the left, lower down.

## Exchange with Admin

`POST /api/stream/prepare` accepts an optional `screens` field. The `width`, `height` and `refresh` fields stay and describe the main display: a Station that ignores `screens` runs a one-display session.

```json
{
  "width": 3840, "height": 2160, "refresh": 160,
  "screens": [
    { "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
    { "width": 5120, "height": 2880, "refresh": 60, "x": 3840, "y": 0 },
    { "width": 3456, "height": 2234, "refresh": 60, "x": -3456, "y": 400 }
  ],
  "audio_channels": 2, "virtual_display": true, "device": { "…": "…" }, "profile": "Bureau"
}
```

- From 1 to 4 displays; exactly one `primary`, at (0, 0); each mode valid as today (7680 × 4320 at most); rectangles that do not overlap, all connected by shared edges. Otherwise: `400`, with the displays left untouched.
- `screens` only has an effect with the virtual display (`virtual_display` true and the trigger on); without a virtual display, the session stays on the computer's displays, as today.
- The `400` error has the API's usual shape (`{"error": "some displays overlap"}`, in the request's language). A `screens` with a single entry makes a one-display session (the answer then has a one-entry `screens`). The same `screens` asked again during the session (Kyber restarted) keep it, as the same mode does today; other displays redo it.

Response: on top of the current fields (`applied` stays the main display's mode), `screens` gives, **in the order of the request**, what is in place: the applied mode and position, or `null` for a display the Station could not create. The main display is always created, otherwise preparation fails as today. Mode and position are read back on the computer afterwards: usually those of the request, but Windows may adjust them (a mode refused, a layout it corrects); the answer is what counts to find the displays in Kyber's list. `primary` is only present on the main display.

```json
"screens": [
  { "width": 3840, "height": 2160, "refresh": 160, "x": 0, "y": 0, "primary": true },
  { "width": 5120, "height": 2880, "refresh": 60, "x": 3840, "y": 0 },
  null
]
```

Heartbeat, end of session and takeover do not change: one session, one device.

## Station

- **Virtual displays**: one SudoVDA display per entry in `screens`. The main one, created first wherever it sits in the request, keeps the current identifier (`KyberNow-Display`), the next ones a derived identifier (`KyberNow-Disp-02` to `-04`, in the order of the request), with serial numbers `KYBERNOW-0001` to `KYBERNOW-0004`: Windows finds each display's settings (scale) again from one session to the next. A secondary display that the driver refuses or that Windows does not activate (8 s) is removed: `null` in the answer, and a warning in the event feed.
- **Layout**: only the virtual displays are active, at the requested positions, the main one at (0, 0); the physical displays switch off and come back at the end, as today. The driver's heartbeat and the safety nets cover every display.
- **Mode changed during the session** (`follow` / `restore`): applies to each display. A physical display turned back on during the session is turned off again by putting each virtual display back at its mode and position from the start of the session.
- **Admin**: the Overview shows the number of displays and their modes; the event feed says how many displays the session took.
- **Kyber patch**: the bitrate of a multi-display session is shared out by each display's pixels per second (width × height × refresh rate) instead of in equal parts; equal parts if a mode is unknown.

## Rust core

- `PrepareRequest` carries `screens` and `Prepared` the applied displays; with no `screens` in the response, the session falls back to one display.
- **Kyber's displays**: each applied display is found in Kyber's list by its position and size; the core waits until they have all been announced (same delay as today), then streams those that are there. One video entry per display in the Kyber session, the main display first.
- **Streams**: each video stream carries the index of the device display it goes to; the app receives each stream's packets separately.
- **Inputs**: an absolute mouse position comes with the display index (that display's pixels) and goes out with the matching Kyber display identifier. The pointer announced by the computer arrives with its display: the app draws it only in that display's window. Keyboard, relative mouse and controllers do not change.
- **Bitrate**: the profile's bitrate is worked out on the sum of the displays' pixels per second, with the same ceilings as today.
- **Recovery after a loss**: one keyframe per stream, never before that stream's first data (a request too early leaves kyavserver silent); restarting the stream restarts every display.
- **Metrics**: latency and frames per second of the main display; Statistics also gives the number of displays.
- **App API**: the one-display session keeps exactly its API (apps that do not handle multiple displays do not change). Multiple displays are requested by the app, which gives the core the list of its displays; the C bridge (`apps/apple`) and the JNI bridge (`apps/android/jni`) still build.

## Apps

- **Mac**: one full-screen window per display (`NSScreen.screens`, mirrors grouped), pointer captured in the active window with the game mouse, menu bar and Dock hidden.
- **Windows**: one borderless full-screen window per display (`EnumDisplayMonitors`), a single Direct3D 11 device and one swap chain per window; the keyboard hook stays single.
- **Linux**: one SDL window per display (`SDL_GetNumVideoDisplays`, `SDL_GetDisplayBounds`), in Desktop mode; Living-room mode and the Steam Deck stay on one display.

## Outside multiple displays

- Streaming several of the computer's physical displays without a virtual display.
- Spreading one of the computer's displays across several of the device's displays.
- The web client (Kyber sends only one video stream over WebSocket), the external display of iPhone and iPad, Android and TVs.

## End-to-end test

On the author's Mac (XG32UQ 4K at 160 Hz as main, Studio Display 5K at 60 Hz, MacBook Pro at 120 Hz, mirroring off) to the test computer (RTX 3080 Ti): Desktop profile, Displays on Auto → three virtual displays laid out like the Mac's, the pointer moves from one to the next, a window dragged from one display to another follows; a full-screen game opens on the XG32UQ at 160 Hz; Game profile → a single display; end of session → the computer's physical displays are back; unplugging the Studio Display during the session → the session is set up again on two displays.
