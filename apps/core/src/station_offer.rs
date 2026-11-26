//! Mise à jour des Stations proposée par les apps, sans passer par les
//! réglages (docs/link-ui.md, « Mise à jour des Stations ») : le bandeau de
//! l'accueil, la puce « · Mise à jour » des autres ordinateurs, l'avis après
//! une mise à jour de l'app, et quand demander à la Station de vérifier
//! (`Console::station_check`). Le même calcul dans toutes les apps ; les
//! textes sont dans leurs catalogues.
//!
//! Rien n'est gardé ici : l'app garde, par ordinateur, un [`Memory`] (JSON)
//! et, pour toute l'app, la dernière version lancée (voir [`app_updated`]).
//! Les apps qui passent par une interface C ou JNI appellent [`evaluate`]
//! (JSON en entrée et en sortie).

use serde::{Deserialize, Serialize};

use crate::console::StationInfo;
use crate::update::newer;

/// « Plus tard » : le bandeau revient au bout de 7 jours (ou avec une
/// version plus récente).
pub const SNOOZE_SECS: u64 = 7 * 24 * 3600;
/// Au plus une demande de vérification par jour et par ordinateur.
pub const CHECK_EVERY_SECS: u64 = 24 * 3600;
/// L'avis après une mise à jour de l'app attend au plus ce temps les
/// vérifications des ordinateurs en ligne.
pub const NOTICE_WAIT_SECS: u64 = 10;

/// Gardé par l'app pour chaque ordinateur.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    /// « Plus tard » : la version remise à plus tard, et quand (secondes Unix).
    #[serde(default)]
    pub snoozed_version: Option<String>,
    #[serde(default)]
    pub snoozed_at: u64,
    /// Dernière vérification demandée : la version de l'app d'alors, et quand.
    #[serde(default)]
    pub checked_app_version: Option<String>,
    #[serde(default)]
    pub checked_at: u64,
}

impl Memory {
    /// « Plus tard » sur le bandeau (ou dans l'avis) pour cette version.
    pub fn snooze(&mut self, version: &str, now: u64) {
        self.snoozed_version = Some(version.to_string());
        self.snoozed_at = now;
    }

    /// Une vérification vient d'être demandée à la Station.
    pub fn checked(&mut self, app_version: &str, now: u64) {
        self.checked_app_version = Some(app_version.to_string());
        self.checked_at = now;
    }

    fn snoozed(&self, version: &str, now: u64) -> bool {
        self.snoozed_version.as_deref() == Some(version) && now < self.snoozed_at.saturating_add(SNOOZE_SECS)
    }
}

/// Ce que l'app sait de l'ordinateur à cet instant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Context {
    /// `station` de `GET /api/stream/info` (absent : Station trop ancienne,
    /// ou pas encore lue).
    #[serde(default)]
    pub station: Option<StationInfo>,
    /// Cet appareil peut mettre la Station à jour : ordinateur appairé, ou
    /// ordinateur d'un compte KyberNow avec le droit `admin`. Jamais un
    /// invité, un prêt, ni « Regarder » seul.
    #[serde(default)]
    pub may_update: bool,
    /// Joignable : sur le réseau, ou par le relais (`away`).
    #[serde(default)]
    pub online: bool,
    /// Joignable seulement par le relais (« Hors de chez vous ») : Admin
    /// refuse la mise à jour par là.
    #[serde(default)]
    pub away: bool,
    /// Une session de cet appareil est ouverte (avec cet ordinateur ou un autre).
    #[serde(default)]
    pub in_session: bool,
    /// Secondes Unix.
    #[serde(default)]
    pub now: u64,
}

/// Le bandeau de l'accueil, pour l'ordinateur choisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Banner {
    /// Rien à montrer.
    None,
    /// « KyberNow Station {v} est disponible pour cet ordinateur. Environ une
    /// minute. » ; « Mettre à jour », « Plus tard ».
    Update { version: String },
    /// « Mise à jour de la Station… » (lancée ailleurs, ou depuis cet appareil).
    Installing,
    /// La version exige l'installateur, sur l'ordinateur ; « Plus tard » seul.
    Setup { version: String },
    /// Hors de chez vous : l'app ne peut pas la mettre à jour ; « Plus tard » seul.
    Away { version: String },
}

/// Version disponible que cet appareil peut proposer, sans égard à « Plus tard ».
fn offered(ctx: &Context) -> Option<&StationInfo> {
    let station = ctx.station.as_ref()?;
    let u = &station.update;
    (ctx.may_update && ctx.online && (u.available || u.installing) && !station.restarting).then_some(station)
}

/// Le bandeau de l'ordinateur choisi (jamais en session, jamais hors ligne).
pub fn banner(ctx: &Context, memory: &Memory) -> Banner {
    if ctx.in_session {
        return Banner::None;
    }
    let Some(station) = offered(ctx) else { return Banner::None };
    let u = &station.update;
    if u.installing {
        return Banner::Installing;
    }
    let Some(version) = u.version.clone().filter(|v| !v.is_empty()) else { return Banner::None };
    if memory.snoozed(&version, ctx.now) {
        return Banner::None;
    }
    if u.requires_setup {
        Banner::Setup { version }
    } else if !u.installable {
        // Paquet refusé : la rubrique Station des réglages le dit.
        Banner::None
    } else if ctx.away {
        Banner::Away { version }
    } else {
        Banner::Update { version }
    }
}

/// La puce « · Mise à jour » d'un ordinateur dans la rangée des autres
/// (« Plus tard » ne l'enlève pas).
pub fn chip(ctx: &Context) -> bool {
    offered(ctx).is_some_and(|s| s.update.available && s.update.version.as_deref().is_some_and(|v| !v.is_empty()))
}

/// L'ordinateur entre dans l'avis après une mise à jour de l'app : une
/// version installable depuis l'app, sur le réseau (pas par le relais), hors
/// session. Rend cette version. « Plus tard » d'avant n'y compte pas : une
/// nouvelle version de l'app est un nouveau moment.
pub fn notice_version(ctx: &Context) -> Option<String> {
    if ctx.in_session || ctx.away {
        return None;
    }
    let s = offered(ctx)?;
    let u = &s.update;
    (u.available && u.installable && !u.requires_setup && !u.installing)
        .then(|| u.version.clone())
        .flatten()
        .filter(|v| !v.is_empty())
}

/// Demander à la Station de vérifier ses mises à jour maintenant
/// (`Console::station_check`) : la version de l'app a changé depuis la
/// dernière demande à cet ordinateur, ou la Station est plus ancienne que
/// l'app ; au plus une fois par jour, jamais en session, et seulement quand
/// elle n'annonce pas déjà une version.
pub fn should_check(ctx: &Context, memory: &Memory, app_version: &str) -> bool {
    if ctx.in_session || !ctx.may_update || !ctx.online {
        return false;
    }
    let Some(station) = ctx.station.as_ref() else { return false };
    if station.update.available || station.update.installing || station.restarting {
        return false;
    }
    let app_changed = memory.checked_app_version.as_deref() != Some(app_version);
    let behind = !station.version.is_empty() && newer(app_version, &station.version);
    let due = app_changed || ctx.now >= memory.checked_at.saturating_add(CHECK_EVERY_SECS);
    (app_changed || behind) && due
}

/// Premier lancement après une mise à jour de l'app (pas le tout premier
/// lancement : `last_launched` vide) : l'avis peut s'afficher.
pub fn app_updated(last_launched: Option<&str>, current: &str) -> bool {
    last_launched.is_some_and(|last| !last.is_empty() && newer(current, last))
}

/// Entrée de [`evaluate`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Input {
    #[serde(flatten)]
    pub context: Context,
    #[serde(default)]
    pub memory: Memory,
    /// Version de l'app.
    #[serde(default)]
    pub app_version: String,
}

/// Sortie de [`evaluate`].
#[derive(Debug, Clone, Serialize)]
pub struct Output {
    pub banner: Banner,
    pub chip: bool,
    pub notice_version: Option<String>,
    pub should_check: bool,
}

/// Tout le calcul pour un ordinateur, en une fois (interfaces C et JNI).
pub fn evaluate(input: &Input) -> Output {
    Output {
        banner: banner(&input.context, &input.memory),
        chip: chip(&input.context),
        notice_version: notice_version(&input.context),
        should_check: should_check(&input.context, &input.memory, &input.app_version),
    }
}

/// [`evaluate`] en JSON ; une entrée illisible donne « rien à montrer ».
pub fn evaluate_json(input: &str) -> String {
    let input: Input = serde_json::from_str(input).unwrap_or_default();
    serde_json::to_string(&evaluate(&input)).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::StationUpdate;

    const NOW: u64 = 1_800_000_000;

    fn ctx(version: &str, update: StationUpdate) -> Context {
        Context {
            station: Some(StationInfo { version: version.into(), update, restarting: false }),
            may_update: true,
            online: true,
            away: false,
            in_session: false,
            now: NOW,
        }
    }

    fn available(v: &str) -> StationUpdate {
        StationUpdate { available: true, version: Some(v.into()), installable: true, requires_setup: false, installing: false }
    }

    #[test]
    fn banner_offers_an_installable_version() {
        let c = ctx("0.11.2", available("0.11.3"));
        assert_eq!(banner(&c, &Memory::default()), Banner::Update { version: "0.11.3".into() });
        assert!(chip(&c));
        assert_eq!(notice_version(&c).as_deref(), Some("0.11.3"));
    }

    #[test]
    fn nothing_for_whoever_may_not_update_or_offline_or_in_session() {
        let mut c = ctx("0.11.2", available("0.11.3"));
        c.may_update = false;
        assert_eq!(banner(&c, &Memory::default()), Banner::None);
        assert!(!chip(&c));
        let mut c = ctx("0.11.2", available("0.11.3"));
        c.online = false;
        assert_eq!(banner(&c, &Memory::default()), Banner::None);
        let mut c = ctx("0.11.2", available("0.11.3"));
        c.in_session = true;
        assert_eq!(banner(&c, &Memory::default()), Banner::None);
        assert_eq!(notice_version(&c), None);
        assert!(!should_check(&c, &Memory::default(), "0.11.3"));
    }

    #[test]
    fn later_hides_for_seven_days_or_until_a_newer_version() {
        let c = ctx("0.11.2", available("0.11.3"));
        let mut m = Memory::default();
        m.snooze("0.11.3", NOW - 3600);
        assert_eq!(banner(&c, &m), Banner::None);
        assert!(chip(&c), "la puce reste");
        m.snooze("0.11.3", NOW - SNOOZE_SECS);
        assert_eq!(banner(&c, &m), Banner::Update { version: "0.11.3".into() });
        m.snooze("0.11.3", NOW - 3600);
        let newer = ctx("0.11.2", available("0.11.4"));
        assert_eq!(banner(&newer, &m), Banner::Update { version: "0.11.4".into() });
        // L'avis après une mise à jour de l'app ne tient pas compte de « Plus tard ».
        assert_eq!(notice_version(&c).as_deref(), Some("0.11.3"));
    }

    #[test]
    fn setup_away_installing_and_refused() {
        let mut u = available("0.12.0");
        u.requires_setup = true;
        u.installable = false;
        let c = ctx("0.11.2", u);
        assert_eq!(banner(&c, &Memory::default()), Banner::Setup { version: "0.12.0".into() });
        assert_eq!(notice_version(&c), None);

        let mut c = ctx("0.11.2", available("0.11.3"));
        c.away = true;
        assert_eq!(banner(&c, &Memory::default()), Banner::Away { version: "0.11.3".into() });
        assert_eq!(notice_version(&c), None);

        let mut u = available("0.11.3");
        u.installing = true;
        assert_eq!(banner(&ctx("0.11.2", u), &Memory::default()), Banner::Installing);

        let mut u = available("0.11.3");
        u.installable = false;
        assert_eq!(banner(&ctx("0.11.2", u), &Memory::default()), Banner::None);
    }

    #[test]
    fn check_after_an_app_update_or_when_the_station_is_behind() {
        let up_to_date = StationUpdate::default();
        let mut m = Memory::default();
        // Jamais demandé : la version de l'app « a changé ».
        assert!(should_check(&ctx("0.11.3", up_to_date.clone()), &m, "0.11.3"));
        m.checked("0.11.3", NOW - 60);
        // Même version de l'app, Station au même niveau : pas de demande.
        assert!(!should_check(&ctx("0.11.3", up_to_date.clone()), &m, "0.11.3"));
        // Station plus ancienne : une fois par jour.
        assert!(!should_check(&ctx("0.11.2", up_to_date.clone()), &m, "0.11.3"));
        m.checked_at = NOW - CHECK_EVERY_SECS;
        assert!(should_check(&ctx("0.11.2", up_to_date.clone()), &m, "0.11.3"));
        // L'app passe en 0.11.4 : tout de suite.
        m.checked("0.11.3", NOW - 60);
        assert!(should_check(&ctx("0.11.3", up_to_date), &m, "0.11.4"));
        // Une version déjà annoncée : rien à demander.
        assert!(!should_check(&ctx("0.11.2", available("0.11.3")), &Memory::default(), "0.11.4"));
    }

    #[test]
    fn app_updated_not_on_first_launch() {
        assert!(!app_updated(None, "0.11.3"));
        assert!(!app_updated(Some(""), "0.11.3"));
        assert!(!app_updated(Some("0.11.3"), "0.11.3"));
        assert!(app_updated(Some("0.11.2"), "0.11.3"));
    }

    #[test]
    fn evaluate_json_round_trip() {
        let out = evaluate_json(
            r#"{"station":{"version":"0.11.2","update":{"available":true,"version":"0.11.3","installable":true}},
                "may_update":true,"online":true,"now":1800000000,"memory":{},"app_version":"0.11.3"}"#,
        );
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["banner"]["kind"], "update");
        assert_eq!(v["banner"]["version"], "0.11.3");
        assert_eq!(v["chip"], true);
        assert_eq!(v["notice_version"], "0.11.3");
        assert_eq!(v["should_check"], false);
        assert_eq!(serde_json::from_str::<serde_json::Value>(&evaluate_json("nope")).unwrap()["banner"]["kind"], "none");
    }
}
