# kybernow-ui — the common interface of Linux and LG TVs

[Français](README.md)

The KyberNow Link apps for Linux (`apps/linux`: desktop, Steam Deck, living room, KyberNow Link OS) and for LG TVs (`apps/webos`) draw their interface themselves, and that interface builds without the core or SDL (screenshots, tests on the Mac). What they each wrote, word for word, is here (27/11 audit, LINAPP-42 and WEBOS-26, phase 2c).

| Module | Contents |
|---|---|
| `i18n` | The app's language (`english`, `set_english`, `resolve` of the Auto / Français / English setting, `force_english` for tests), `tr("…", "…")`, `de` (“d'Atelier”, “de Bureau-Salon”), `settings_for`, `decimal`; **`core_text(key, params)`**: a text from the core's catalogue (`apps/core/src/i18n/*.json`, read as is), word for word the same as in the other apps |
| `time` | Local time (`localtime_r`), day and month names, `hour_label` (“6:30 PM”), `weekday_label`, `date_label` (“November 26”), `relative_day` (“today”, “yesterday”) |
| `autotest` | The Auto profile test panel: `StepState`, `Step`, `Choice`, the waiting steps and the relay (core catalogue), “Chosen by the test of {date} ({network}).”, the texts of the “Bitrate” card and of the banner |
| `station_offer` | The Station update offered by the app: what the core computes, kept by the interface (`StationOffer`, `OfferBanner`), the home banner, the notice after an app update, their texts |
| `share` | A guest's role and rights (`ShareRole`, `ShareMove`), the picture quality, the state of a share's relay and its wait (`ShareRelay`, `relay_wait`, `RELAY_LINK_WAIT`), availability, the small texts |
| `lend` | Lending: a friend's computer's state, borrowing and its session, the host's view and its requests, the code and its link, invitations, their texts |
| `from_core` | `core` feature, for the device layer: the core's role, quality and relay in these types |

What stays in each app: what concerns its screen, focus and actions (`App`, `Target`, `Screen`), and drawing (Linux's canvas follows the screen scale and HiDPI, LG's the TV's 1080p output: only a few bricks are alike).

Rules without text come from `kybernow-rules` (`apps/core/rules`); the device layer's functions that only depend on the core are in the core (`share::join_relays`, `relay::session_end_text`, `input::keymap::scancode_of_usage`).

Tests: `cargo test -p kybernow-ui` (and `--features core`).
