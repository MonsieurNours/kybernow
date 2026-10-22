# Measurements to take on the hardware

English · [Français](mesures.md)

Four measurements asked for by the audit of 20 October 2026 ([audit-2026-10-20.md](audit-2026-10-20.md), in French only, section 16, row “Vérifications acceptées”, accepted checks). Each one is done on the author's hardware and settles a decision: without its result, the matching work item stays on hold. None has a result yet.

| # | Measurement | Where | Settles |
|---|---|---|---|
| 1 | Display latency of the Mac app | Mac, in a session | The “one frame in flight” guard; full screen presented directly or composited (§3) |
| 2 | Windows permissions | Computer | Permissions of `ProgramData` (§7, work item 2); integrity level of the icon (§7) |
| 3 | Saturated Wi-Fi | Mac on Wi-Fi, computer, a third device | Default send policy: deep or short queue (patches 0007 and 0010; §3, §8) |
| 4 | SudoVDA virtual display modes | Computer, in a session | Whether the “follow / put back” policy acts on the virtual display (§15) |

Reference hardware: a Mac M4 Max; a Windows computer with an NVIDIA card, at 192.168.1.100 on the local network, with a 4K 160 Hz display.

Each measurement has its own table to fill in. Measurements 1 and 3 have the columns condition, p50, p95, losses and remark. Measurements 2 and 4 do not measure durations: their tables have the columns that fit.

## Before you start

- Note the date, the version of the Mac app (KyberNow Link › About KyberNow) and that of the Station (Admin's **Updates** page, or `C:\Program Files\KyberNow\kybernow.json`).
- One session at a time. Close the Apple TV app and the web client: a second device that connects takes over and cuts the first.
- For the latency measurements, prepare a scene on the computer that moves all the time and that you can replay identically: a game demo on a loop, or an animation test in full screen. No still screen: Kyber sends nothing when the image does not change (§15).
- Keep a timer at hand (phone).
- This file stays blank. The filled-in tables go into the audit (see “Where to write the results”).

---

## 1. Display latency of the Mac app

**Goal.** Measure what the “one frame in flight” guard of the renderer (`VideoRenderer`) costs, and find out whether the full screen is presented directly or composited by the WindowServer. According to §3 of the audit, lifting the guard gains 4 to 8 ms at 120 Hz and 8 to 16 ms at 60 Hz; direct presentation gains about 8 ms at 120 Hz.

**What the app writes.**
- `KN_PRESENT_STATS=1`: every second, one line in the unified log (subsystem `fr.leslignesclaires.kybernow`, category `video`) with the p50 and p95 of “presentedTime − commit” (the time between handing the frame to Metal and its appearance on screen), the number of frames discarded by Core Animation and the number of frames rendered.
- `KN_INFLIGHT_GUARD=1`: puts the guard back, for comparison. Without this variable, the guard is lifted.

### Prepare

1. Check that your repository is up to date: the following command must print lines from `VideoRenderer.swift`.

```bash
cd ~/Sites/kyber-apps
grep -rn KN_PRESENT_STATS apps/macos/KyberNow apps/macos/KyberKit
```

2. Quit KyberNow Link (⌘Q), then rebuild the development app. `pgrep` must print nothing.

```bash
pgrep KyberNow
cd ~/Sites/kyber-apps/apps/macos
xcodebuild -project KyberNow.xcodeproj -scheme KyberNow -configuration Debug -derivedDataPath build/dev build
```

3. In the computer's settings (Mac app): **Game** profile, **vertical sync on** (without it, the guard plays no part).
4. On the computer, start the scene (see “Before you start”).

### Launch the app with the variables

Open two Terminal windows.

Window 1, the log:

```bash
log stream --predicate 'subsystem == "fr.leslignesclaires.kybernow" && category == "video"' --level info
```

Window 2, the app. Run one of the two commands, never both:

```bash
cd ~/Sites/kyber-apps
APP="$PWD/apps/macos/build/dev/Build/Products/Debug/KyberNow.app"

# Without the guard (default behavior)
KN_PRESENT_STATS=1 "$APP/Contents/MacOS/KyberNow"

# With the guard (for comparison)
KN_PRESENT_STATS=1 KN_INFLIGHT_GUARD=1 "$APP/Contents/MacOS/KyberNow"
```

The app stays attached to window 2. ⌘Q or ⌃C quits it. Always launch the binary as above: `open` does not pass the variables on.

From the first frame of the session, one line per second appears in window 1: p50, p95, discarded, rendered (the exact wording is the app's). Nothing after 5 s: the app does not have the variables (step 1 of “Prepare”).

### Protocol

Do it at 120 Hz first, then at 60 Hz.

1. Set the Mac's display: System Settings › Displays › Refresh Rate. On a ProMotion display, “ProMotion” follows the stream up to 120 Hz and “60 Hertz” fixes 60 Hz. On an external display, pick 120 Hz, then 60 Hz.
2. Launch the app **with the guard** (the second command above). Open the session on the computer in the Game profile, then go full screen (⌃⌥⌘F). Bar and panel closed, mouse still and away from the top of the screen.
3. Start the scene on the computer. Let it run 10 s, then count **60 s** on the timer without touching anything.
4. Disconnect (⌃⌥⌘D), quit the app (⌘Q), then stop the log (⌃C in window 1).
5. Keep the lines of the measurement, to read them without hurry:

```bash
log show --last 2m --info --style compact --predicate 'subsystem == "fr.leslignesclaires.kybernow" && category == "video"' > ~/Desktop/display-120hz-with-guard.log
```

6. Relaunch the app **without the guard** (the first command above) and redo steps 2 to 5. Change the file name.
7. Set the display to 60 Hz and redo steps 2 to 6.

If two identical measurements differ by more than 1 ms at the p50, redo the pair.

### What to write down

In the file of each measurement, take the 60 lines of the minute and note:
- **p50**: the middle value of the 60 p50s;
- **p95**: the middle value of the 60 p95s (note the highest in the remark);
- **losses**: the total of discarded frames and the total of rendered frames over 60 s;
- in the remark: the median of “Dernière session” (last session) on the Mac app's home screen, which must drop by the same amount as the p50.

| Condition | p50 (ms) | p95 (ms) | Losses (discarded / rendered) | Remark |
|---|---|---|---|---|
| 120 Hz, with guard | | | | |
| 120 Hz, without guard | | | | |
| Difference at 120 Hz (with − without) | | | | |
| 60 Hz, with guard | | | | |
| 60 Hz, without guard | | | | |
| Difference at 60 Hz (with − without) | | | | |

### Interpret

- **A p50 drop of 4 to 8 ms at 120 Hz (8 to 16 ms at 60 Hz) confirms the audit.** Keep the guard lifted; that is the default behavior.
- A drop of 1 to 4 ms is a real gain, smaller than expected: keep the guard lifted if the p95 does not rise.
- A drop of less than 1 ms, or a rise: the audit's hypothesis is wrong. Do not lift the guard for latency.
- The **discarded** and **rendered** frames go up without the guard. That is not a loss: the stream arrives at 160 frames per second on a 120 Hz display, and Core Animation shows the most recent one (the panel calls them “Remplacées”, replaced). A problem would be a p95 that rises by more than the p50 drops, or visible stutters.

### The Metal HUD

It says whether the session's Metal layer is presented directly on screen or composited by the WindowServer. According to §3, it is composited, because a SwiftUI view sits on top of the layer for the whole session.

1. Quit the app. Relaunch it with the HUD (and the stats, to get the p50 of the same minute):

```bash
MTL_HUD_ENABLED=1 KN_PRESENT_STATS=1 "$APP/Contents/MacOS/KyberNow"
```

2. Open the session, go full screen (⌃⌥⌘F), start the scene. The HUD appears over the image (device, frames per second, frame time, GPU time…).
3. Look in the HUD for the line that says how the layer is presented. Its wording changes with the macOS version: “Direct” or “Composited”, sometimes a mention of compositing. What matters: **is the layer presented directly, without going through the compositor?** If you find no such line, take a screenshot (⇧⌘3) and say so in the remark.
4. **Bar closed**: mouse away from the top of the screen, panel closed. Wait 10 s, note the presentation line, take a screenshot (⇧⌘3) and note the p50 and p95 of the last 10 log lines.
5. **Bar open**: bring the mouse to the top of the screen until the bar appears, leave it 10 s, note and take a screenshot. Same log reading.
6. Optional, **measurements panel open** (⌃⌥⌘S), bar closed: same reading.

The HUD costs a little GPU time: compare measurements with the HUD to measurements with the HUD, never to those of the previous table.

| Condition | HUD presentation line | p50 (ms) | p95 (ms) | Remark |
|---|---|---|---|---|
| Bar closed | | | | |
| Bar open | | | | |
| Measurements panel open (optional) | | | | |

**Interpret.**
- “Composited” with the bar closed: the composition of the full screen is confirmed. The work item in §3 (Metal layer = the content view's layer, bar and panels in a child window ordered only while visible) is justified. Its real gain, about one refresh, will be measured after the work item with the same procedure.
- “Direct” with the bar closed and “Composited” with the bar open: composition only costs while the bar or a panel is visible; the rest of the time the full screen is already direct. The work item brings nothing more; the p50 difference between the two lines tells what the open bar costs.
- “Direct” everywhere: composition is not the cause. Drop this work item.

---

## 2. Windows permissions

**Goal.** Check on the computer that the folder `C:\ProgramData\KyberNow` is only accessible to administrators (§7, §17 of the audit), and that the icon relaunched by an Admin update does not run with an elevated token (§7, the point marked “to confirm”).

### The data folder

Open PowerShell **as administrator** (right-click › Run as administrator) on the computer (or from a session opened on the Mac). Never open this folder with File Explorer: faced with a denied access, it offers “Continue” and then adds your account to the permissions, which spoils the measurement.

```powershell
icacls C:\ProgramData\KyberNow
icacls C:\ProgramData\KyberNow /T | Select-Object -First 40
```

**Expected**, on the folder itself (the names follow Windows' language):

```text
C:\ProgramData\KyberNow NT AUTHORITY\SYSTEM:(OI)(CI)(F)
                        BUILTIN\Administrators:(OI)(CI)(F)
```

With `/T`, the subfolders and files give the same two lines, marked inherited: `(I)(OI)(CI)(F)` for a folder, `(I)(F)` for a file. That is correct.

The measurement fails if a line names `Users` (`Utilisateurs` on a French Windows), `Everyone` (`Tout le monde`), `Authenticated Users` (`Utilisateurs authentifiés`) or `CREATOR OWNER` (`CREATEUR PROPRIETAIRE`), or if a right is not `(F)`. Then note the path concerned, run `Restart-Service kyber-admin` (Admin sets the permissions again at each start) and start over.

Three more checks:

```powershell
(Get-Acl C:\ProgramData\KyberNow).Owner
(Get-Acl C:\ProgramData\KyberNow).AreAccessRulesProtected
Select-String -Path C:\ProgramData\KyberNow\kyber-admin.log -Pattern 'droits de' | Select-Object -Last 3
```

Expected: the owner is `BUILTIN\Administrators` (`BUILTIN\Administrateurs` on a French Windows); `True`; at least one line “droits de C:\ProgramData\KyberNow : SYSTEM et Administrateurs seulement” (Admin logs in French), dated from Admin's last start.

### From a standard account

1. Sign in to Windows with a standard account (Settings › Accounts › Other users, if you have none; switch user without signing the other one out).
2. Open PowerShell (not as administrator) and run:

```powershell
dir C:\ProgramData\KyberNow
New-Item -ItemType Directory C:\ProgramData\KyberNow\update\essai
```

**Expected**: both commands are denied (“Access to the path … is denied”, “Accès refusé” on a French Windows). No file name is displayed.
3. In the same session, click the KyberNow Station icon, then the entry that opens Admin: it opens.

Without a standard account, the same `dir` in a **non-elevated** PowerShell window of your administrator account must also be denied (the Administrators group is deny-only there). The proof is weaker: it does not cover another account.

### The icon after an update

An icon relaunched by Admin with an administrator's full token runs at high integrity. The test only makes sense on an **administrator** account: on a standard account, the token is already limited.

1. Before the update, measure the icon started at sign-in (reference) with one of the tools below: it must be at medium integrity.
2. Start an update from Admin (page **Updates › Install**). If there is no newer version, serve a package from the Mac (see [releases.en.md](releases.en.md), “To try a package served from the Mac”). Wait about a minute: the icon reappears.
3. Check that you are looking at the icon relaunched by the update, not the old one:

```powershell
Get-Process kybernow-tray | Select-Object Id,Path,StartTime
```

`StartTime` must be that of the end of the update. Note the `Id`.
4. Click the icon to open its panel (it starts `msedgewebview2.exe` processes, children of the icon).
5. Read the integrity level of `kybernow-tray.exe` (and of its `msedgewebview2.exe` children) with one of these tools:
   - **Process Explorer** (Sysinternals), run as administrator. View › Select Columns, add the “Integrity Level” column. Or double-click the process, Security tab: the group “Mandatory Label\… Mandatory Level”. This is the most reliable method.
   - **Task Manager**, run as administrator. Details tab, right-click a header › Select columns › “Elevated” (“Élevé” or “Niveau d'élévation” on a French Windows, depending on the version). The column says “Yes” or “No” for each process.

**Expected: medium integrity (Medium), “Elevated” column at “No”.** High integrity or “Yes” confirms the problem of §7: Admin relaunches the icon with the full token (`launch_as_user`, `server/admin/src/display/agent.rs`, through `WTSQueryUserToken`). The fix is to give it the linked limited token (`TokenLinkedToken`).

| Condition | Expected | Obtained | Remark |
|---|---|---|---|
| `icacls` on the folder | SYSTEM and Administrators, `(OI)(CI)(F)`, nothing else | | |
| `icacls /T`, first 40 lines | the same two names, inheritance `(I)`; no Users line | | |
| Owner, protected inheritance | Administrators; `True` | | |
| “droits de” log line | one line per Admin start | | |
| Standard account, `dir` | denied | | |
| Standard account, creating a folder | denied | | |
| Standard account, icon → Admin | opens | | |
| Icon at sign-in (reference) | medium integrity | | |
| Icon after an Admin update | medium integrity, “Elevated”: No | | |
| `msedgewebview2.exe` children of the panel | medium integrity | | |

---

## 3. Saturated Wi-Fi

**Goal.** Find out whether the deep queue (8 MiB, the default send policy) causes bufferbloat when the Wi-Fi saturates, and what the short queue (3 MiB) gives. The setting: [kyber-changes.en.md](kyber-changes.en.md#measuring-the-send-policy-0010). The audit (§3, §8) estimates that 0.7 to 2.2 s of stale frames are sent before being dropped: 8 MiB is 67 Mbit, i.e. 0.67 s at 100 Mbit/s and 2.2 s at 30 Mbit/s.

### Hardware and settings

- The Mac on Wi-Fi, on the same access point as the computer. The computer preferably on Ethernet: the Wi-Fi is then the only bottleneck. Unplug the Mac's Ethernet and turn off any VPN.
- A third device on Wi-Fi on the same access point (laptop, phone) with `iperf3`.
- Note the Mac's Wi-Fi: Option-click on the Wi-Fi icon gives the channel, the band, the link rate and the signal.
- The Mac app in the **Game** profile, 4K resolution. Open the session, start the scene, go full screen, then open the **detailed measurements** panel (⌃⌥⌘S).

### Prepare the saturation

On the computer, in PowerShell:

```powershell
iperf3 -s
```

Allow `iperf3` through the firewall (private network) at the first prompt. Remove that rule when you are done.

On the third device, **with no session open**, first check that the link saturates:

```bash
iperf3 -c 192.168.1.100 -u -b 300M -t 10 -R
```

`-R` reverses the direction: the computer sends, like for the image. Read the summary. If the received rate is clearly under 300 Mbit/s or if datagrams are lost, the link is saturated. Otherwise raise `-b` (500M, 800M) until it is, and note the value you keep. Without iperf3, a big download started from the third device will do, less reproducibly.

### Protocol

One session, three phases of 60 s, chained without closing the session.

1. **Before (60 s).** Session open for at least 30 s, scene running, panel open, no traffic elsewhere. Every 10 s, take the readings in the table below. At the start of the phase, note the line “Session : N images, M paquets” in the Losses area. Take it again at the end of the phase.
2. **During (60 s).** Run on the third device:

```bash
iperf3 -c 192.168.1.100 -u -b 300M -t 60 -R
```

Same readings every 10 s, and the session counter at the start and the end.
3. **After (60 s).** As soon as iperf3 ends. Same readings. Also note how long the latency takes to come back to within 2 ms of the value from before.

At each 10 s reading, note: the **latency** (the big number at the top of the panel, median of the last second), the **Network** line of the cascade (“Réseau”) and the **round trip** (Details area, “aller-retour”). At 30 s and 60 s of each phase, also note the four boxes of the “Pertes · 30 s” (Losses · 30 s) area: “Images perdues” (lost frames), “Paquets perdus” (lost packets), “Images clés” (keyframes requested again) and “Remplacées” (replaced). These boxes cover a sliding 30 s: the two readings follow each other without overlapping. A phase's keyframes are their sum.

| Phase | 10 s | 20 s | 30 s | 40 s | 50 s | 60 s |
|---|---|---|---|---|---|---|
| Latency, before (ms) | | | | | | |
| Latency, during (ms) | | | | | | |
| Latency, after (ms) | | | | | | |
| Round trip, before (ms) | | | | | | |
| Round trip, during (ms) | | | | | | |
| Round trip, after (ms) | | | | | | |

For the final table:
- **p50**: the middle value of the phase's 6 latency readings;
- **p95**: the highest of the 6 readings (six readings do not allow better); the p95 of the whole session is on the app's home screen, under “Dernière session” (last session), once the session is closed;
- **losses**: the difference of the “Session” counter between the start and the end of the phase (frames and packets), then the keyframes.

### The two series

Do the complete series twice, with the same Mac app and the same Station version:
1. **deep queue** (`datagram_send_queue = "deep"`, the default);
2. **short queue** (`datagram_send_queue = "short"`).

The policy is chosen in Admin (System › Advanced › Image send queue) or in `kyber_config.toml`. After each change, restart Kyber (Admin offers to) and check the `Datagram send queue: …` line in the controller log (see [kyber-changes.en.md](kyber-changes.en.md#measuring-the-send-policy-0010)).

| Condition | p50 (ms) | p95 (ms) | Losses (frames / packets / keyframes) | Remark |
|---|---|---|---|---|
| Deep queue, before | | | | |
| Deep queue, during | | | | |
| Deep queue, after | | | | |
| Short queue, before | | | | |
| Short queue, during | | | | |
| Short queue, after | | | | |

In the remark: the `-b` rate you kept and the rate iperf3 received, the Mac's Wi-Fi, the stream's bitrate (“Débit” curve), the time to return to normal after the saturation, and any event (fallback banner, frozen image, session cut).

### Interpret

- **If the median latency “during” exceeds the one “before” by several hundred milliseconds, the deep queue causes bufferbloat.** That is what the audit predicts. A latency that stays high for a long time “after” is the same signature: the queue empties at the link's pace.
- With the short queue, the latency “during” must stay close to the one “before”. The price is more losses and more keyframes requested again: compare them. As a rule of thumb, the short queue is the right one if the latency “during” stays within 100 ms of the one “before”, with no image freeze longer than a second.
- No difference between “during” and “before” with the deep queue: the link was not saturated for the stream. Raise `-b` or move the Mac closer to the Wi-Fi limit.

---

## 4. SudoVDA virtual display modes

**Goal.** Find out which modes the virtual display exposes: one, or a list. This decides whether a game can change the virtual display's resolution, hence whether the “the captured display's mode changes” scenario of §15 exists on the virtual display.

Do not change any mode during the readings: the session may stop, depending on the capture engine (§15).

### In Windows Settings

1. Open a session from the Mac, with the virtual display in place on the computer. You can do everything from the computer's window on the Mac.
2. On the computer: Settings › System › Display. Select the virtual display at the top of the page (if the physical displays are off, it is alone; its monitor is probably named “KyberNow”).
3. Scroll to **Display resolution** and open the list. Note the modes offered: one, or several (take a screenshot with ⇧⌘4 on the Mac). Close the list with Esc, choosing nothing.
4. Open **Advanced display** for this display. Open **Choose a refresh rate** and note the rates. Same rule: choose nothing.
5. Optional: Advanced display › Display adapter properties › **List All Modes** gives the driver's complete list.

### With a PowerShell script

Run it in a PowerShell window of the signed-in user's session (not in a service or a remote connection: the list would be empty there). It does not need to be administrator. Paste the whole block.

```powershell
# Lists the modes of each display with EnumDisplaySettingsEx.
$source = @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class KnDisplay
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODE
    {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public ushort dmSpecVersion;
        public ushort dmDriverVersion;
        public ushort dmSize;
        public ushort dmDriverExtra;
        public uint dmFields;
        public int dmPositionX;
        public int dmPositionY;
        public uint dmDisplayOrientation;
        public uint dmDisplayFixedOutput;
        public short dmColor;
        public short dmDuplex;
        public short dmYResolution;
        public short dmTTOption;
        public short dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public ushort dmLogPixels;
        public uint dmBitsPerPel;
        public uint dmPelsWidth;
        public uint dmPelsHeight;
        public uint dmDisplayFlags;
        public uint dmDisplayFrequency;
        public uint dmICMMethod;
        public uint dmICMIntent;
        public uint dmMediaType;
        public uint dmDitherType;
        public uint dmReserved1;
        public uint dmReserved2;
        public uint dmPanningWidth;
        public uint dmPanningHeight;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DISPLAY_DEVICE
    {
        public uint cb;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceString;
        public uint StateFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceID;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceKey;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplayDevices(string lpDevice, uint iDevNum, ref DISPLAY_DEVICE lpDisplayDevice, uint dwFlags);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplaySettingsEx(string lpszDeviceName, int iModeNum, ref DEVMODE lpDevMode, uint dwFlags);

    static DEVMODE NewMode()
    {
        DEVMODE dm = new DEVMODE();
        dm.dmSize = (ushort)Marshal.SizeOf(typeof(DEVMODE));
        return dm;
    }

    static string Describe(DEVMODE dm)
    {
        return dm.dmPelsWidth.ToString() + " x " + dm.dmPelsHeight.ToString()
            + " @ " + dm.dmDisplayFrequency.ToString() + " Hz";
    }

    public static string[] Report(uint flags)
    {
        List<string> lines = new List<string>();
        for (uint i = 0; ; i++)
        {
            DISPLAY_DEVICE adapter = new DISPLAY_DEVICE();
            adapter.cb = (uint)Marshal.SizeOf(typeof(DISPLAY_DEVICE));
            if (!EnumDisplayDevices(null, i, ref adapter, 0)) break;

            string monitorName = "(none)";
            DISPLAY_DEVICE monitor = new DISPLAY_DEVICE();
            monitor.cb = (uint)Marshal.SizeOf(typeof(DISPLAY_DEVICE));
            if (EnumDisplayDevices(adapter.DeviceName, 0, ref monitor, 0)) monitorName = monitor.DeviceString;

            bool attached = (adapter.StateFlags & 0x1) != 0;
            bool primary = (adapter.StateFlags & 0x4) != 0;
            lines.Add("");
            lines.Add(adapter.DeviceName + " | " + adapter.DeviceString + " | monitor: " + monitorName
                + (attached ? " | active" : " | inactive") + (primary ? " | primary" : ""));

            DEVMODE current = NewMode();
            if (EnumDisplaySettingsEx(adapter.DeviceName, -1, ref current, 0))
                lines.Add("  current mode: " + Describe(current));

            List<string> modes = new List<string>();
            for (int n = 0; ; n++)
            {
                DEVMODE dm = NewMode();
                if (!EnumDisplaySettingsEx(adapter.DeviceName, n, ref dm, flags)) break;
                string mode = Describe(dm);
                if (!modes.Contains(mode)) modes.Add(mode);
            }
            lines.Add("  " + modes.Count.ToString() + " mode(s):");
            foreach (string line in modes) lines.Add("    " + line);
        }
        return lines.ToArray();
    }
}
'@

if (-not ('KnDisplay' -as [type])) { Add-Type -TypeDefinition $source }

'=== Modes offered by the driver ==='
[KnDisplay]::Report([uint32]0)

'=== Raw modes (EDS_RAWMODE), even those the monitor does not declare ==='
[KnDisplay]::Report([uint32]2)
```

For each `\\.\DISPLAYn`, it prints: the adapter, the monitor, whether it is active, its current mode, then the list of modes as `width x height @ frequency Hz` (without duplicates, all color depths merged). The virtual display is the one whose monitor is named “KyberNow” or whose adapter is SudoVDA's; failing that, the one whose current mode is the session's. An inactive display may show “0 mode(s)”.

| Source | Modes read | Count | Remark |
|---|---|---|---|
| Settings › Display resolution | | | |
| Settings › Advanced display › refresh rates | | | |
| Script, modes offered by the driver | | | |
| Script, raw modes | | | |
| “List All Modes” (optional) | | | |

### Interpret

- **A single mode** (the one Admin asked for when it created the display): a game cannot change the virtual display's resolution. The “mode changes” scenario of §15 then only exists on physical displays, and Admin's “follow / put back” policy has nothing to do on the virtual display.
- **A list**: a game can change the virtual display's resolution. The scenario of §15 is real: the image goes on at the new size (txproto engine) or the session is cut (libavconv engine), and Admin's “follow / put back” policy acts on it (let the game choose the mode, or put the device's back). Note whether the list contains your Mac display's resolution and 4K, and several rates for the same size.
- Raw modes more numerous than the offered modes: the virtual monitor hides some. Note them: a game that enumerates modes the way the script does will see the driver's list.

---

## Where to write the results

In [audit-2026-10-20.md](audit-2026-10-20.md), **section 16** (“Décisions prises le 20 octobre 2026”, decisions made on 20 October 2026), row “Vérifications acceptées”, whose end says “Procédures à préparer” (procedures to prepare): those procedures are this file. Under the section 16 table, add one subheading per measurement with the date, the versions, the filled-in table and the decision taken. Then make the row point to those subheadings.

| Measurement | Who reads the result in the audit | Decision to write |
|---|---|---|
| 1. Display latency | §3, rows “Affichage” (display) and “Composition du plein écran” (full-screen composition) | Guard lifted or put back; direct Metal layer work item started or dropped |
| 2. Windows permissions | §7 (permissions, icon at high integrity); §17, list “Mise à jour et installation” (update and installation) | Folder permissions validated or to redo; `TokenLinkedToken` fix to do or not |
| 3. Saturated Wi-Fi | §3, row “Envoi (serveur)” (sending, server); §8, patch 0007 | “Short queue” policy as default or not, and its value |
| 4. SudoVDA modes | §15, “Questions propres à ce sujet” (questions specific to this topic) and “Choix de politique” (policy choice) | One or several modes; what the “follow / put back” policy does on the virtual display |
