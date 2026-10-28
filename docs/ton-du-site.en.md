# Writing for the site: KyberNow's tone of voice

English · [Français](ton-du-site.md)

Agreed with the author on 24/10/2026. These rules apply to every visible text on the site (www.kybernow.app): pages, headings, buttons, captions, alt texts, `aria-label`, `<title>` and `meta description` tags, structured data, `llms.txt`. App screens recreated in HTML (class `.sc`, `aria-hidden`) carry the apps' real strings: leave them alone.

## Who we talk to

- **The home page** talks to the general public: someone with a Windows PC who wants to use it elsewhere (play on the living-room TV, pick up their desktop from the sofa or a hotel room), who has never heard of Moonlight and doesn't know what QUIC is. They want to know what it does for them, on which devices, what they need, whether it is free and safe.
- **The Technology page** talks to the curious and to enthusiasts (who know Moonlight, Sunshine, Parsec): figures, the trip of a picture, QUIC, Kyber, the comparison. Same voice, more detail, always explained.
- **The guide** talks to someone installing, step by step, hands on the keyboard: precise, concrete, in the order of the screens, no needless jargon.

French uses **"vous"** everywhere. The visitor is often the subject of the sentence: "You play on the TV" rather than "The game is displayed on the television".

## The voice: simple and warm

- Short, complete sentences with a subject and a verb. One idea per sentence.
- Everyday words: "the TV", "your PC" or "your computer", "the picture", "the sound", "the controller".
- Concrete: devices, rooms of the house, moments ("in the evening, on the sofa", "at a hotel", "on the train"). An example beats an adjective.
- Warm means attentive, not chummy: no slang, no emoji, no strings of exclamation marks, no forced jokes.
- Honest: KyberNow is in beta, some apps are still installed by hand (Apple TV, iPhone, LG TVs), some have not been tried on real hardware. Say it plainly, without apologising and without hiding it.

## Never write again

These turns of phrase read as "generated text" and the author rejects them. Examples found on the former site:

| Pattern | Example to ban | Why |
|---|---|---|
| "X, not Y" antithesis | "Measured, not promised." ; "Not a demo: …" | Empty formula, slogan tone |
| Pivot sentence "What changes is…" | "What changes is how you drive it" | Artificial construction, wrong word |
| Elliptical slogan heading | "Everything, right away." ; "Designed down to the last pixel." ; "Convinced? Three steps." | Says nothing concrete |
| Personification | "The computer goes quiet, the device listens." | Gratuitous style |
| Systematic rule of three | "Free, open, no account." as a heading ; "No subscription, no rented machine: yours." | Advertising hammering |
| Punchline colon | "…: yours." ; "Install, connect, nothing else." | Artificial payoff |
| Instructions for the page itself | "Plays as you scroll · no sound" ; "Scroll: from capture to display." ; "The video starts on its own when…" | The user doesn't care |
| Unverifiable superlative | "No other solution brings all this together" ; "next-gen" | Empty promise |
| Unexplained jargon on the home page | QUIC, TLS 1.3, Opus, 4:4:4, 4:2:0, HEVC, encoder, FEC, A/B, HDMI-CEC, "flash", VRR, Adaptive-Sync, "relative mouse" | The general public switches off |
| Hollow words | "experience", "seamless", "smooth", "solution", "ecosystem", "revolutionary", "powerful", "ultimate", "effortless", "no compromise" | Meaningless |
| Cascading labels | "On your devices", "Ready to use", "Already in your hands" above every heading | Decoration, not information |

Also avoid: starting a sentence with "And" or "Or", ellipses, rhetorical questions as headings, cascades of verbless fragments ("Keyboard, mouse, controller. Everything."), English loanwords in French when a French word exists ("streaming" is allowed in texts for enthusiasts, "jeu à distance" elsewhere).

## What to write instead

- **Headings** say what the visitor gets or does: "Play on the living-room TV", "What you need to get started", "Install KyberNow in three steps". A normal sentence, not a slogan. The heading's bold part (`<strong>`) highlights what matters; it doesn't create a payoff.
- **Figures** stay (the 9.4 ms latency, 4K at 120 Hz, 7.1 sound…), but each is said plainly the first time. Example: "9.4 milliseconds between your PC and your screen: less than a hundredth of a second, measured on a local network". Don't repeat the same figure in every section.
- **Jargon**: on the home page, say the effect, not the technique ("the picture and your keystrokes are encrypted" rather than "TLS 1.3"; "if the Wi-Fi drops, the app reconnects on its own" rather than "FEC"). The technical name may follow in brackets on the Technology page, never on its own.
- **App names**: keep the three names, KyberNow Station (on the PC you share), KyberNow Admin (the settings, in the browser) and KyberNow Link (on your devices), plus KyberNow Link OS (the Raspberry Pi image). Introduce them clearly once, then "the Station", "Admin", "the Link app" are fine.
- **Buttons**: a verb and an object. "Download", "Watch the video", "Install on Apple TV", "Read the guide".
- **Alt texts and `aria-label`**: describe what is shown, in one useful sentence ("A TV shows the KyberNow Link home screen; someone holds a controller"). No instructions.

## What KyberNow brings (the messages to get across)

In order of importance for the general public, always with facts checked in the code and docs (PROJECT.md, ROADMAP.md, the apps' READMEs): nothing "tested" that hasn't been, no sample figure presented as a measurement.

1. Your PC on all your screens: the TV, the Mac, the tablet, the phone, the Steam Deck, a browser. For playing as well as working.
2. As responsive as a plugged-in screen: very low latency thanks to the Kyber engine, a picture at each screen's resolution and refresh rate.
3. It holds up: if the network wobbles, the picture is repaired and the app reconnects on its own (ten reconnection attempts, a second apart, on every app).
4. The controller handles a lot, with nothing to add: the app chosen per device opens at the start of the session (Steam Big Picture, Playnite…), the PC sees the controller as an Xbox pad, the TV and Steam Deck apps are fully driven by controller (which can also act as a mouse), the right-sized screen is created on its own. These are well-known remote play irritations, sorted without add-ons. Don't write "everything by controller in every app": the Windows app and the browser have no controller navigation, Mac and iPhone only the session menu.
5. Free, open source, no account and no subscription, everything is encrypted. At home, nothing goes through a server; away from home, KyberNow's relay passes the connection without being able to read the picture or the keystrokes in the apps (the browser is the exception), and what it does see (addresses, bitrates, share codes) is said plainly.
6. In beta: already used every day, and we say so openly.

## Typographic conventions (FR)

- Narrow no-break space before ; : ! ? (`&#8239;` in HTML), French guillemets « » with narrow no-break spaces (`«&#8239;` and `&#8239;»`), no-break space (`&nbsp;`) between a number and its unit (9,4&nbsp;ms, 4K à 120&nbsp;Hz, 77,3&nbsp;Mo).
- Decimal comma in French (9,4 ms), point in English (9.4 ms).
- Capital letter only at the start of headings and for proper names (no Title Case in French).
- No em dash for a pause in a sentence: a comma, a colon, or a new sentence.

## English

English is not a word-for-word translation: same voice, same facts, written the way an English-speaking writer would (British English, already used on the site: "colour", "favourite"…). Same bans (no "not X, but Y", no elliptical slogans, no "seamless", "next-gen", "effortless"). The home page title and description keep "Sunshine and Moonlight alternative" for search.

## Images

The home page photos show real-life moments with real app captures on the screens: people aged 30 to 50, women and men, seen from behind or three-quarter, in warm evening light, often together. They are generated and assembled by `scripts/site-scenes/` (see its README). App screens recreated in HTML (`.sc`) remain for showing the interface up close.
