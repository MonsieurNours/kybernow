# Installing KyberNow Link on an LG TV (developer mode)

English · [Français](INSTALLATION.md)

KyberNow Link for webOS is not in the LG Content Store: it installs in **developer mode**, which LG opens to everyone with a free account, without changing anything on the TV (no root). Allow a quarter of an hour the first time, then a minute for each update.

You need:

- an LG TV running **webOS 5 or later** (2020 and later; a 2022 OLED G2 is fine);
- a computer (Windows, Mac or Linux) on the same network as the TV;
- the app's package: `fr.leslignesclaires.kybernow_<version>_arm.ipk` (built by `apps/webos/scripts/build.sh`, see [README.en.md](README.en.md));
- a Windows computer with KyberNow Station, to connect to afterwards.

## 1. The LG developer account

1. On the computer, open [webostv.developer.lge.com](https://webostv.developer.lge.com) and create an account ("Sign In" › "Create Account"). It is free.
2. Confirm the e-mail address.

## 2. Developer mode on the TV

1. On the TV, open the **LG Content Store** (Apps), search for **Developer Mode** and install the app.
2. Open **Developer Mode** and sign in with the account from step 1.
3. Turn on **Dev Mode Status**. The TV restarts.
4. Open **Developer Mode** again. Turn on **Key Server**. Write down the TV's **IP address** and the **passphrase** (six characters) the app shows.

Developer mode lasts a limited time, shown in the app ("Remaining time"): **when it reaches zero, the TV leaves developer mode and the apps installed this way disappear**. The app's **Extend Session** button resets it to the maximum; remember it from time to time (webOS Dev Manager, below, can extend it too).

## 3. Installing the app

Two ways. The first one is the simplest.

### With webOS Dev Manager (windows, no command line)

1. On the computer, install [webOS Dev Manager](https://github.com/webosbrew/dev-manager-desktop/releases) (webosbrew, open source).
2. Add the TV: **Add device** › **Developer mode**, with the IP address and passphrase from step 2.
3. **Apps** tab › **Install** (or drop the file onto the window): pick `fr.leslignesclaires.kybernow_<version>_arm.ipk`.
4. **KyberNow Link** shows up in the TV's app list.

### On the command line (LG's tools)

```sh
# Once: LG's webOS tools (Node.js 18 or later)
npm install -g @webos-tools/cli

# Once: the TV (IP address from step 2, port 9922, user prisoner)
ares-setup-device --add tv -i "host=192.168.1.50" -i "port=9922" -i "username=prisoner"
ares-novacom --device tv --getkey          # asks for the passphrase from step 2

# For each release
ares-install --device tv fr.leslignesclaires.kybernow_0.4.4_arm.ipk
ares-launch --device tv fr.leslignesclaires.kybernow
```

Replace `192.168.1.50` with the TV's address. `ares-install --device tv --list` shows what is installed, `ares-install --device tv --remove fr.leslignesclaires.kybernow` removes the app.

## 4. First launch

1. Open **KyberNow Link** from the TV's home (app bar).
2. Computers with KyberNow Station on the same network show up at the bottom ("On the network"). Pick one: a **six-digit code** shows.
3. On the computer, open **KyberNow Admin › Link devices** (or the icon by the clock) and **accept the code**. The TV gets an account of its own, which you can remove in Admin.
4. The computer shows large on the home screen: **Connect**.

The computer does not show up? Choose **Add** › **With address and port** and type Admin's address and port (shown in Admin › Link devices, under the QR code), then compare the fingerprint shown with Admin's.

## 5. Recommended TV settings

- **Picture mode: Game Optimizer.** During the session, Magic Remote Settings button › Picture › Mode › Game Optimizer. No app can pick it for you; without it, the TV adds its picture processing (and latency).
- **Controllers**: pair them with the TV over Bluetooth, in the TV's settings (devices or gamepads section; the path depends on the webOS version). DualSense, DualShock 4, Xbox and Switch Pro work. In the app, **Settings › Controllers and Magic Remote › Test controllers** shows the buttons and tries the rumble.
- **5.1 sound with a receiver**: in the app, Computer settings › Sound › **5.1 Dolby**; on the TV, Sound › Digital Sound Output › **Pass-Through**. Without it the receiver gets stereo, from the TV's apps too. "5.1 PCM" only works with the few receivers that announce multichannel PCM.
- **Network**: LG TVs' Ethernet port stops at 100 Mbit/s; the app caps the bitrate at 80 Mbit/s there. Wi-Fi (5 GHz, close to the access point) often goes further: the app goes up to 95 Mbit/s there.

## Updating

Install the new `.ipk` as in step 3: it replaces the old one. The added computers are kept (in `/media/developer/temp/kybernow`, outside the app's folder).

## Troubleshooting

- **The app does not install**: developer mode may have expired (open Developer Mode again, turn it on, extend), or the TV's IP address changed (`ares-setup-device` or Dev Manager).
- **Black screen in session, but sound**: note it with the log (below); the TV's decoder refused the stream. Try Settings › Picture › Codec: H.264, or a lower resolution.
- **The log**: the app writes `kybernow.log` in `/media/developer/temp/kybernow/` on the TV (with `kybernow.json`, the added computers). To read it from the computer:

  ```sh
  ares-novacom --device tv --run "cat /media/developer/temp/kybernow/kybernow.log"
  ```

  (Dev Manager also has a terminal and a file browser.)
