# Pen (graphics tablet, Apple Pencil)

English · [Français](stylet.md)

The rule, for every app, is in [link-ui.en.md](link-ui.en.md), “Pen”. Here: the protocol, what each part does and where testing stands.

The author's choices (25/11/2026), asked for by a Linux user (Fedora, Cinnamon) whose tablet worked without pressure:

- every app and both Stations;
- pressure, tilt, rotation, eraser end, pen button and hover;
- with an older Station, the pen stays a mouse at an absolute position, with no notice;
- no setting to turn it off.

## Protocol (kynput)

Two additions to Kyber's input protocol, which an older app or Station ignores (unknown type: the packet is dropped with a log line, the connection goes on).

**`Pen` packet** (type 11), from the app to the computer, the pen's whole state in each packet (no deltas: a lost packet is repaired by the next one):

| Field | Size | Meaning |
|---|---|---|
| `display_id` | u32 | the computer's display, as in `MousePosition` |
| `x`, `y` | i16, i16 | pixels of that display, as in `MousePosition` |
| `pressure` | u16 | 0 to 65535; 0 while hovering |
| `tilt_x`, `tilt_y` | i8, i8 | degrees, −90 to 90; X to the right, Y toward the user (Windows' and Pointer Events' convention) |
| `rotation` | u16 | degrees, 0 to 359 |
| `flags` | u8 | `CONTACT` 1 (tip or eraser touches), `IN_RANGE` 2 (near the surface; absent: the pen leaves), `ERASER` 4, `BARREL` 8 (pen button), `TILT` 16 and `ROTATION` 32 (given by the device) |

**`ControlEvent::PenAvailable`** (value 3), from the computer to the app: a Station that can inject a pen sends it to every app that connects, along with the pointers it already sends at that moment (`on_client_connected` of the input server). It lives in kynput itself, not in `/api/stream/info`: a share guest and a borrowing friend, who do not read Admin, get the pen too. An app that has not received it sends the pen as a mouse, as before.

On the Station, the pen is the mouse: it goes through with the “keyboard and mouse” right (`InputRights::keyboard_mouse`), not without it. The input server remembers the pen while it is near the surface; “release everything” (`FlushKeys`) and taking back the keyboard and mouse lift it and let it go where it was.

## Kyber patches

- `apps/macos/kyber-patches/0005-kynput-stylet.patch` (client side, applied by every app and by the Stations, which reapply the apps' patches after theirs): the `Pen` type (`kynput/src/types/pen.rs`), `PenAvailable`, the C API (`kynput_pen_to_packet`, `kynput_pen_from_packet`, `kynput_Pen` struct) and kynput-rs (`Pen`, `ControlEvent::PenAvailable`).
- `server/kyber-patches/0027-kynput-stylet.patch` (Station): Windows' synthetic pen (`kynput/src/windows/host_pen.rs`), its wiring in `host_keyboard_mouse.rs`, `kynput_host_pen_available()`, and the input server (`pipeline_state.rs`: right, lifted pen, announcement).
- `server/macos/kyber-patches/0020-kynput-stylet-mac.patch` (Mac Station): macOS tablet events (`kynput/src/macos/host_keyboard_mouse.rs`) and `kynput_host_pen_available()` true on the Mac too.

Details in [kyber-changes.en.md](kyber-changes.en.md).

## Windows Station

A Windows synthetic pen (`CreateSyntheticPointerDevice(PT_PEN)`, Windows 10 1809 and later), created at the first `Pen` packet, fed by `InjectSyntheticPointerInput`: position in virtual desktop pixels (the display's plus the pen's), pressure brought to 0-1024 (never 0 in contact: Windows Ink would draw nothing), tilt, rotation, `PEN_FLAG_BARREL`, eraser (`PEN_FLAG_INVERTED` near the surface, `PEN_FLAG_ERASER` in contact). Windows lets go of a synthetic pen that stops moving: the last state is injected again every 50 ms while the pen is near the surface. A desktop switch (UAC, lock screen) is followed as for `SendInput`. Apps that know nothing of pens get the mouse Windows makes from the pen (tip: left click; button held and tip down: right click).

## Mac Station

Patch `server/macos/kyber-patches/0020-kynput-stylet-mac.patch`, after 0002 (which injects the mouse through `CGEvent`) and 0015: the app's pen becomes a tablet for macOS, like one from a Wacom driver. Approach and leaving are proximity events (`kCGEventTabletProximity`, tip or eraser: `NSPointingDeviceType` 1 or 3, with fixed device and tool IDs); switching ends without leaving makes one end leave before the other enters. The rest are mouse events of the tablet subtype (`kCGEventMouseSubtypeTabletPoint`) carrying the pressure (0 to 1, never 0 in contact), the tilt (−1 to 1, 64° at most, the reverse of the Mac app), the rotation (degrees) and the pen's buttons: hovering as `MouseMoved`, tip down as a dragged left click (double click included). The pen button is a right click, as a tablet driver does: pressed while hovering or with the tip down, held until both the button and the tip are released; a tip put down first stays a left click. Software that knows nothing of tablets sees a mouse. `kynput_host_pen_available()` is true on the Mac too: the Mac Station announces the pen (`PenAvailable`). Nothing to repeat at intervals, unlike Windows. It needs the Accessibility permission, as the mouse does.

kynput's log: “Pen near (tip)” or “(eraser)”, “Pen Left button down” / “up”, “Pen left”; each point at `debug` level.

## The apps' core

`InputPipeline::pen_on(screen, x, y, &PenInput)` (`apps/core/src/input/pipeline.rs`, `pen.rs`): returns `false` when the pen must go as a mouse (no `PenAvailable`, or game mouse); the app then sends the position and clicks as before. Contact changes and leaving are sent reliably (retried), moves once. Session behind another one or right taken back while the tip touches: the pen is lifted, then nothing more. The device's mouse resuming (position or click) first lets a pen still near the surface go. `tilt_from_altitude_azimuth` turns altitude and azimuth (Apple Pencil, Android) into X / Y tilt.

## The apps

| App | Reading the pen | Status |
|---|---|---|
| Linux | `src/native/pen.c`: XInput2 under X11 (own connection, libX11 and libXi loaded at run time; “Abs Pressure”, “Abs Tilt X/Y” axes; “eraser” in the device name: the eraser; still hover for 500 ms: gone), `tablet-v2` under Wayland (own queue on SDL's connection). `src/device/pen.rs`: sending, SDL's mouse held back while the pen goes as a pen (X11), pen replayed as a mouse in SDL for the app's interface (Wayland, where the compositor no longer sends the pen as a mouse). `KYBERNOW_NO_PEN=1` leaves the pen to SDL (troubleshooting). | written; tried in the VM with a virtual tablet (uinput) under X11 (XWayland) and Wayland (GNOME): approach, contact, pressure, tilt, leaving; not yet with a Station |
| Mac | `StreamView.swift`: mouse events from the tablet (`subtype == .tabletPoint`), `tabletProximity` (tip or eraser); pressure, tilt (`tilt` × 64°), rotation; pen button: the right click the driver makes of it | written, built (separate copy, with the new libkynput); not tried |
| iPad | `KyberKitIOS/StreamView.swift`: `.pencil` touches (pressure `force / maximumPossibleForce`, tilt from altitude and azimuth, Apple Pencil Pro roll), coalesced touches, hover (`UIHoverGestureRecognizer`, `zOffset > 0`). The Pencil writes where it touches in every mode; with an older Station, a mouse at that spot and a left click | written; Swift built for the simulator; to link with the rebuilt iOS bridge (`build-apple-mobile.sh ios`); not tried |
| Windows | `window.rs`: `WM_POINTER*` of a `PT_PEN` pointer (`GetPointerPenInfo`, history of coalesced movements through `GetPointerPenInfoHistory`); pressure 0-1024, tilt, rotation, eraser (`INVERTED` / `ERASER`), barrel button; position brought back like the mouse; the mouse Windows makes from the pen held back (`GetMessageExtraInfo`) and Windows Ink gestures switched off while the pen goes as a pen; touch and the pen as a mouse to `DefWindowProc`, as before. A Wacom without “Windows Ink” stays a mouse | written, built for x64 and ARM64; not tried |
| Android | `session/Inputs.kt` (`StylusInput`): `MotionEvent` with `TOOL_TYPE_STYLUS` / `TOOL_TYPE_ERASER` on the picture, ahead of the finger (the pen writes where it touches whatever the touch mode); pressure `getPressure`, tilt from `AXIS_TILT` and `AXIS_ORIENTATION` (JNI bridge `Native.inputPen`, `jni/src/exports.rs`), batched positions (`getHistorical*`), `BUTTON_STYLUS_PRIMARY` button, `ACTION_HOVER_*` hover (`dispatchGenericMotionEvent`, on the picture only). Leaving waits 120 ms (Android sends the end of hover right before contact, and the lift right before hover). With an older Station, mouse at that spot and left click | written, built, Robolectric tests (`StylusInputTest`); not tried on a device or with a Station |
| Web | Pointer Events `pointerType == "pen"` (`pressure`, `tiltX`, `tiltY`, `twist`, `buttons` 32 for the eraser) in kynput's web engine | to do |

The Apple bridge (`apps/apple/src/input.rs`) has `kn_input_pen` and `kn_pen_tilt`, for the Mac and the iPad (and the Apple TV, which compiles `InputCore.swift` without using them).

## Tests to do

1. Windows Station (built with 0027) and the Linux app under X11 (Cinnamon, Wacom tablet): “Synthetic pen created” in kynput's log, pressure in Krita or Photoshop, tilt (brush flattening), eraser, pen button (right click), hover (drawing app's cursor), pen leaving then mouse.
2. The same under Wayland (GNOME, KDE), and the app's menus with the pen (bar, settings).
3. Mac with a tablet, iPad with an Apple Pencil (pressure, tilt, hover on an M2 iPad or later), Android tablet or phone with a stylus (Galaxy with S Pen: pressure, hover, button; tilt depending on the device); on the Android emulator, `adb shell input stylus swipe …` should give a pen without pressure or tilt (not tried yet).
4. With an older Station (without 0027): the pen as a mouse as before, with no notice.
5. Share guest with the keyboard and mouse (pen), then without (nothing); mouse taken back by the host while the guest writes (pen lifted).
6. Several displays: the pen on the second display's window.
7. Mac Station (built with 0016, Accessibility permission) and the Mac app with a tablet, or an iPad with an Apple Pencil: “Pen near (tip)” in kynput's log, pressure and tilt in Krita or Photoshop on the Mac, eraser (eraser tool picked on approach), pen button as a right click, hover (the app's cursor), pen leaving then mouse. Already checked without a tablet: event sequence (unit tests) and reading by AppKit.
