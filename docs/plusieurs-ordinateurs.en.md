# Several computers at once

English · [Français](plusieurs-ordinateurs.md)

A KyberNow Link app can hold sessions on several computers at the same time, one window per computer: keep an eye on the office PC while playing on the living-room one, or work on two machines from the same Mac. This document is the shared contract of the Rust core and of the Mac, Windows and Linux (desktop interface) apps. The other Link apps (iPhone, iPad, Apple TV, Android, LG TVs, web client), the Steam Deck, the living room and KyberNow Link OS stay on one computer at a time. The Station does not change: each computer sees an ordinary session, from one device.

This work has no screens of its own: it reuses the home screen, the connection, the session bar, the icon's menu and the notifications of [link-ui.en.md](link-ui.en.md), which gives the exact texts (“Several computers” section). The three apps behave the same and use the same words; a new text goes into `link-ui.md` first, then into each app's catalogues.

## What the user sees

- **Opening another computer during a session**, in three ways, the same everywhere:
  - **the session bar**: a “Computers” menu lists your other computers, with their state; an online computer opens with a click, an offline computer wakes as from the home screen (the only way on Linux, which has no icon next to the clock);
  - **the icon's menu next to the clock** (Mac, Windows): “Connect” stays active on the other computers;
  - **the home screen** (Mac, Windows), reopened with “Open KyberNow Link”: “Connect” stays active on the other computers.
- **One window per computer**, titled with its name. The first session opens as today (full screen, or every display with multi-display). Each further session opens **in a window**, on top, on the active window's display, centred, two thirds of that display in 16:9; it takes a single display (“This display”), whose size is the window's, like any windowed session. Its bar offers “Full screen”: it then covers its display, the first session keeping its own.
- **The active session** is the foreground window's. Only it gets the keyboard, the mouse, the controllers, the clipboard and the sound. The others keep their picture on screen, to keep an eye on them, but their computer receives nothing, and they are not heard. A window that is fully hidden or minimised lightens its picture, as today.
- **Sound**: “Keep sound” in the bar (the speaker button) keeps a session audible even when it is not active. Off by default, for the current session.
- **Switching computers**: a click on its window, or the **“Next computer”** shortcut, ⌃⌥⌘O on the Mac, Ctrl+Alt+Shift+O on Windows and Linux, which goes to the next computer's window (in opening order), even in full screen, where ⌘Tab and Alt+Tab go to the computer. The icon's menu and the bar's “Computers” menu also have “Show” for each session.
- **The same computer twice** is impossible: “Connect” on a computer already in a session brings its window to the front; the home screen then shows “Show session” instead of “Connect”.
- **Four computers at most** at the same time. Beyond that, “Connect” is inactive, with the reason.
- **End of a session**: “Disconnect” or closing its window only ends its own. The last one closed brings back the home screen, as today. Quitting the app ends them all. A session that stops without “Disconnect” (another device, Admin, an error) while its window was not active gives the usual notification, even if the app is in front.
- **The rest does not change**: the last session, the settings, the pictures, the profile and the mouse stay each computer's own; the statistics and ⌃⌥⌘S (Ctrl+Alt+Shift+S) are the active window's.

## Rust core

The exact API and what each app plugs into it: [apps/core/README.en.md](../apps/core/README.en.md#several-computers-at-once-srcfocusrs-srcgamepads-srcjournalrs) (Rust) and [coeur-apple.en.md](coeur-apple.en.md) (C bridge).

- **Several `Session`s in one process**: already possible (no global state besides the language and the log). Log lines carry the computer's name (`Session::set_name`, given by the registry).
- **Each session's state**: `Session::set_active(bool)` and `Session::set_muted(bool)`, kept from one image start to the next (every image's input chain and sound engine take them). A single session is active and audible: nothing changes for an app that does not use them.
  - Not active: nothing leaves the device (keyboard, mouse, text, controller buttons and axes), keys and buttons released, controllers at rest (sticks centred, triggers released), relative mouse off, clipboard paused (nothing read from the device, nothing written to it), rumble ignored. Active again: the device's clipboard content is sent to the computer again. No cut of kynput's input processing as a safety net: it would also cut the pointer received by the windows behind and the releases; everything that leaves comes from the app or the core's reader and is stopped at the chain's entrance (`InputPipeline::set_active`).
  - Muted: 20 ms fade (`AudioEngine::set_muted`); sound keeps being decoded and drained: it comes back without drift.
- **A session registry** (`kybernow_core::focus`, `focus::global()`), the single source for “which one is active”:
  - `register(&session, name) → Result<id, Full>` (the registered session becomes active), `unregister(id)`, `activate(id)`, `set_keep_sound(id, bool)`, `keep_sound(id)`, `active()`, `count()`, `next()` (“Next computer”), `MAX_SESSIONS`;
  - on every change, it applies to all sessions: inputs active or not, sound muted or not (except “Keep sound”), clipboard, controllers; those losing the hand first;
  - the apps only tell it which window comes to the foreground.
- **Controllers: a single reader per app** (`kybernow_core::gamepads`), in the core, taken from the Linux app's (`apps/linux/src/device/pads.rs`, SDL). SDL opens only once per process: a session can no longer read the controllers by itself.
  - Controllers are plugged into every session (the Station keeps its virtual controller, a game does not lose it); buttons and axes only go to the active session; on deactivation, the rest state is sent once.
  - Rumble only goes back to the controller from the active session; a session going behind stops the rumble it had started.
  - Mac and Windows: libkynput is built there without its SDL reader; the core reads the controllers on its own SDL thread, which starts with the first input chain and closes 5 s after the last one (SDL then gives the controllers back: player light, HIDAPI mode; nothing stays open outside a session). `gamepads::start_sdl()` keeps it for the whole life of the app, only to read controllers outside a session. These apps keep `gamepads_from_app: false` (the Mac's controller hook sees these packets as before). Linux feeds the reader from its SDL loop (`gamepads::plug`, `unplug`, `button`, `axis`). An app with a single session behaves as before.
- **Clipboard on Windows**: the client's kynput.dll is built without kynput's (its hidden window only exists once per process); the app gives the core the Windows clipboard, one per session.
- **Displays**: a session opened while another one runs takes “This display” in a window, whatever the “Displays” setting (two sessions never take the same displays): `screens::screens_for_extra_session` and `screens::extra_session_window` (two thirds of the display, 16:9, centred). Only the first one can take every display.
- **C bridge**: `kn_focus_register`, `kn_focus_unregister`, `kn_focus_activate`, `kn_focus_set_keep_sound`, `kn_focus_keep_sound`, `kn_focus_active`, `kn_focus_next`, `kn_focus_count`, `kn_focus_max`; `kn_session_set_active`, `kn_session_is_active`, `kn_session_set_muted`, `kn_session_is_muted`, `kn_session_attach_audio`; `kn_screens_for_extra_session`, `kn_screens_extra_session_window`; `kn_gamepads_start_sdl` (usually unneeded: the reader follows the input chains); for a single chain or engine, `kn_input_set_active`, `kn_input_is_active` and `kn_audio_set_muted`. `KnStatus.error` also carries the reason for a failure as early as `KN_STATUS_ENDING`. Nothing on the JNI side (Android stays on one computer).
- **Limit**: four sessions per app.

## Apps

- **Mac**: a session manager replaces the single session; each session has its own window, and the home screen no longer serves as a session window: it stays available during sessions (“Open KyberNow Link”, the Dock), and the icon menu's “Settings” and “Add a computer” become active there, as on Windows. The further sessions' windows must be able to sit on top of a full-screen session. The Session menu's commands and the shortcuts go to the active session. The code shared with the Apple TV and the iPhone (`KyberKit`) keeps their behaviour.
- **Windows**: sessions in a registry (window → session) instead of the global state; the keyboard hook and Raw Input follow the active window; a single bar window, serving the session of the window where it opens; connection backgrounds only for the first session; `--connect`, the jump list and the notifications open one more session instead of being refused.
- **Linux** (desktop interface): a list of sessions instead of the single session; events carry their session's id; each window belongs to a session; the core's registry says which one is active and applies input, clipboard and sound (no more global `MUTED`; "Mute" in the session menu stays the user's own choice, apart); controllers go through the core's reader, which the SDL loop feeds; a further session takes its display, window and size from `screens_for_extra_session` and `extra_session_window`, and its stream follows its window to another display. The Steam Deck, the living room and Link OS keep one session.

## Outside this work

- Several devices on the same computer (spectator, 2nd player): that is KyberNow Share, on the roadmap.
- Two sessions on the same computer from the same app: impossible (the Station holds only one device at a time).

## End-to-end test

Two Stations (for instance PC-DE-MATTHIEU and PC-ATELIER), on the Mac, then on Windows and Linux:

1. A session on the first in full screen (then with every display); from the bar, “Computers” › the second: its window opens on top, with its name, and becomes active.
2. Type, click, plug in a controller: only the active window's computer reacts; ⌃⌥⌘O (Ctrl+Alt+Shift+O) goes to the other one, and the controller follows without the game losing it.
3. Sound: only the active session is heard; “Keep sound” on the other one keeps it audible.
4. Copy a text on one, switch to the other, paste: the text only goes through the device, never from one computer to the other in the background.
5. “Connect” on a computer already in a session: its window comes to the front. A fifth computer: refused with the reason.
6. Close the second one's window: the first session goes on. Quit the app: both end, both computers' displays come back.
