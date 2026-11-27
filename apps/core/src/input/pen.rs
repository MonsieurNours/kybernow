// Stylet (docs/link-ui.md, « Stylet » ; docs/stylet.md) : ce que l'app lit
// de son stylet (tablette graphique, Apple Pencil, stylet d'Android, Pointer
// Events), mis au format du paquet kynput `Pen`.
//
// Le stylet ne part en stylet que si l'ordinateur l'a annoncé
// (`PenAvailable`, reçu à l'arrivée des entrées) et en souris du bureau ;
// sinon [`crate::input::InputPipeline::pen_on`] rend `false` et l'app le
// traite en souris, comme avant.
//
// « Stylet : Pression / Souris » (décidé le 27/11/2026) : [`PenMode`], retenu
// par ordinateur ; en « Souris », le stylet part en souris absolue même quand
// l'ordinateur le reçoit. La règle de la ligne et de la tuile ([`PenChoice`]),
// ses textes et l'avis du raccourci (Ctrl+Alt+Maj+P, ⌃⌥⌘P) sont ici, pour
// toutes les apps.

use kynput_rs::Pen;
use serde::{Deserialize, Serialize};

use crate::i18n::{self, Lang};

/// « Stylet : Pression / Souris » d'un ordinateur.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PenMode {
    /// Le vrai stylet (pression, inclinaison, Windows Ink) : par défaut.
    #[default]
    Pressure,
    /// Le stylet comme une souris en position absolue : pointe posée = clic
    /// gauche, bouton du stylet = clic droit ; ni pression ni Windows Ink.
    Mouse,
}

impl PenMode {
    /// Les clés gardées par les apps et synchronisées (`pen`).
    pub const KEYS: [&'static str; 2] = ["pressure", "mouse"];

    pub fn key(self) -> &'static str {
        match self {
            Self::Pressure => "pressure",
            Self::Mouse => "mouse",
        }
    }

    /// Une clé gardée ; inconnue (version plus récente) : `None`.
    pub fn parse(key: &str) -> Option<Self> {
        match key {
            "pressure" => Some(Self::Pressure),
            "mouse" => Some(Self::Mouse),
            _ => None,
        }
    }

    /// Le raccourci (Ctrl+Alt+Maj+P, ⌃⌥⌘P) et la tuile basculent.
    pub fn toggled(self) -> Self {
        match self {
            Self::Pressure => Self::Mouse,
            Self::Mouse => Self::Pressure,
        }
    }

    /// « Pression » / « Souris » (choix de la ligne, sous-titre de la tuile).
    pub fn label(self, lang: Lang) -> String {
        i18n::tr(lang, if self == Self::Mouse { "pen.mouse" } else { "pen.pressure" }, &[])
    }

    /// L'avis bref du raccourci : « Stylet : souris » / « Stylet : pression ».
    pub fn notice(self, lang: Lang) -> String {
        i18n::tr(lang, if self == Self::Mouse { "pen.notice_mouse" } else { "pen.notice_pressure" }, &[])
    }
}

/// La ligne « Stylet » du menu « Souris » de la barre (et la tuile des barres
/// en tuiles) : seulement quand un stylet a servi dans la session ou que
/// l'appareil en a un, et que l'ordinateur reçoit le stylet (« stylet
/// disponible »). Les réglages de l'ordinateur la montrent toujours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PenChoice {
    pub mode: PenMode,
    /// Un stylet a servi dans cette session ([`crate::input::InputPipeline::pen_seen`]).
    pub seen: bool,
    /// L'appareil a un stylet (tablette graphique vue, Apple Pencil associé, stylet d'Android).
    pub device_has_pen: bool,
    /// L'ordinateur reçoit le stylet ([`crate::input::InputPipeline::pen_offered`]).
    pub offered: bool,
}

impl PenChoice {
    pub fn shown(&self) -> bool {
        (self.seen || self.device_has_pen) && self.offered
    }

    /// Pour les ponts : `{shown, mode, title, label, choices: [{key, label}],
    /// help}` dans la langue `lang`.
    pub fn ui_json(&self, lang: Lang) -> serde_json::Value {
        let choices: Vec<serde_json::Value> =
            [PenMode::Pressure, PenMode::Mouse].iter().map(|m| serde_json::json!({ "key": m.key(), "label": m.label(lang) })).collect();
        serde_json::json!({
            "shown": self.shown(),
            "mode": self.mode.key(),
            "title": i18n::tr(lang, "pen.title", &[]),
            "label": self.mode.label(lang),
            "choices": choices,
            "help": i18n::tr(lang, "pen.help", &[]),
        })
    }
}

/// Un état du stylet, tel que l'app le lit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PenInput {
    /// Près de la surface (survol ou contact). Faux : le stylet s'en va.
    pub in_range: bool,
    /// La pointe (ou la gomme) touche.
    pub contact: bool,
    /// Pression, de 0 à 1.
    pub pressure: f32,
    /// Inclinaison en degrés (-90 à 90 ; X vers la droite, Y vers soi), si
    /// l'appareil la donne.
    pub tilt: Option<(f32, f32)>,
    /// Rotation en degrés (0 à 360), si l'appareil la donne.
    pub rotation: Option<f32>,
    /// Le bout gomme est celui qui sert.
    pub eraser: bool,
    /// Le bouton du stylet est tenu.
    pub barrel: bool,
}

impl PenInput {
    /// Le paquet kynput, à (`x`, `y`) de l'écran `display_id` de l'ordinateur.
    pub fn to_kynput(&self, display_id: u32, x: i16, y: i16) -> Pen {
        let mut flags = 0;
        if self.in_range {
            flags |= Pen::IN_RANGE;
            if self.contact {
                flags |= Pen::CONTACT;
            }
        }
        if self.eraser {
            flags |= Pen::ERASER;
        }
        if self.barrel {
            flags |= Pen::BARREL;
        }
        let (tilt_x, tilt_y) = match self.tilt {
            Some((tx, ty)) => {
                flags |= Pen::TILT;
                (degrees_i8(tx), degrees_i8(ty))
            }
            None => (0, 0),
        };
        let rotation = match self.rotation {
            Some(r) if r.is_finite() => {
                flags |= Pen::ROTATION;
                (r.rem_euclid(360.0).round() as u16) % 360
            }
            _ => 0,
        };
        let pressure = if self.in_range && self.contact { Pen::pressure_from_fraction(self.pressure) } else { 0 };
        Pen { display_id, x, y, pressure, tilt_x, tilt_y, rotation, flags }
    }
}

fn degrees_i8(v: f32) -> i8 {
    if v.is_finite() { v.clamp(-90.0, 90.0).round() as i8 } else { 0 }
}

/// Inclinaison X / Y (degrés) d'après l'altitude (angle avec la surface :
/// π/2 debout) et l'azimut (direction vers laquelle penche le stylet, 0 vers
/// la droite, sens horaire à l'écran), en radians : Apple Pencil, Android.
/// Formule des Pointer Events (W3C).
pub fn tilt_from_altitude_azimuth(altitude: f32, azimuth: f32) -> (f32, f32) {
    use std::f32::consts::FRAC_PI_2;
    let altitude = altitude.clamp(0.0, FRAC_PI_2);
    if altitude >= FRAC_PI_2 - 1e-4 {
        return (0.0, 0.0);
    }
    if altitude <= 1e-4 {
        // À plat : penché tout entier dans la direction de l'azimut.
        let (s, c) = azimuth.sin_cos();
        let pick = |v: f32| if v > 1e-4 { 90.0 } else if v < -1e-4 { -90.0 } else { 0.0 };
        return (pick(c), pick(s));
    }
    let t = altitude.tan();
    let tx = (azimuth.cos() / t).atan().to_degrees();
    let ty = (azimuth.sin() / t).atan().to_degrees();
    (tx, ty)
}

/// Le choix d'une session, partagé entre [`crate::session::Session`] et son
/// fil : donné par l'app (départ, barre, raccourci, réglage), appliqué à la
/// chaîne d'entrées en cours et à chaque nouvelle.
#[derive(Default)]
pub(crate) struct PenSwitch {
    mode: std::sync::Mutex<PenMode>,
    input: std::sync::Mutex<std::sync::Weak<super::InputPipeline>>,
}

impl PenSwitch {
    pub(crate) fn set(&self, mode: PenMode) {
        *self.mode.lock().unwrap() = mode;
        if let Some(input) = self.input.lock().unwrap().upgrade() {
            input.set_pen_mode(mode);
        }
    }

    pub(crate) fn mode(&self) -> PenMode {
        *self.mode.lock().unwrap()
    }

    /// Une nouvelle chaîne d'entrées : elle prend le choix en cours.
    pub(crate) fn attach(&self, input: &std::sync::Arc<super::InputPipeline>) {
        input.set_pen_mode(self.mode());
        *self.input.lock().unwrap() = std::sync::Arc::downgrade(input);
    }

    /// La ligne « Stylet » de cette session ([`PenChoice`]).
    pub(crate) fn choice(&self, device_has_pen: bool) -> PenChoice {
        let input = self.input.lock().unwrap().upgrade();
        PenChoice {
            mode: self.mode(),
            seen: input.as_ref().is_some_and(|i| i.pen_seen()),
            device_has_pen,
            offered: input.as_ref().is_some_and(|i| i.pen_offered()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// « Stylet : Pression / Souris » : pression par défaut, bascule, clés
    /// gardées, ligne montrée seulement avec un stylet et un ordinateur qui le
    /// reçoit, textes de la ligne et de l'avis du raccourci.
    #[test]
    fn stylet_pression_ou_souris() {
        assert_eq!(PenMode::default(), PenMode::Pressure);
        assert_eq!(PenMode::Pressure.toggled(), PenMode::Mouse);
        assert_eq!(PenMode::Mouse.toggled(), PenMode::Pressure);
        for m in [PenMode::Pressure, PenMode::Mouse] {
            assert_eq!(PenMode::parse(m.key()), Some(m));
            assert!(PenMode::KEYS.contains(&m.key()));
            assert_eq!(serde_json::to_value(m).unwrap(), serde_json::json!(m.key()));
        }
        assert_eq!(PenMode::parse("gomme"), None);
        let c = |seen, device_has_pen, offered| PenChoice { mode: PenMode::Pressure, seen, device_has_pen, offered };
        assert!(!c(false, false, true).shown(), "aucun stylet : pas de ligne");
        assert!(!c(true, true, false).shown(), "ordinateur qui ne reçoit pas le stylet : pas de ligne");
        assert!(c(true, false, true).shown());
        assert!(c(false, true, true).shown());
        assert_eq!(PenMode::Mouse.notice(Lang::Fr), "Stylet\u{202f}: souris");
        assert_eq!(PenMode::Pressure.notice(Lang::En), "Pen: pressure");
        let ui = PenChoice { mode: PenMode::Mouse, ..c(true, false, true) }.ui_json(Lang::Fr);
        assert_eq!((ui["shown"].as_bool(), ui["mode"].as_str(), ui["label"].as_str()), (Some(true), Some("mouse"), Some("Souris")));
        assert_eq!(ui["title"], "Stylet");
        assert_eq!(ui["choices"][0]["label"], "Pression");
    }
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    #[test]
    fn hover_has_no_pressure_and_contact_does() {
        let hover = PenInput { in_range: true, pressure: 0.7, ..Default::default() };
        let p = hover.to_kynput(2, 10, 20);
        assert_eq!((p.display_id, p.x, p.y, p.pressure), (2, 10, 20, 0));
        assert!(p.has(Pen::IN_RANGE) && !p.has(Pen::CONTACT));

        let down = PenInput { in_range: true, contact: true, pressure: 1.0, ..Default::default() };
        let p = down.to_kynput(2, 10, 20);
        assert_eq!(p.pressure, Pen::MAX_PRESSURE);
        assert!(p.has(Pen::CONTACT));
    }

    #[test]
    fn out_of_range_is_never_in_contact() {
        let gone = PenInput { contact: true, pressure: 1.0, ..Default::default() };
        let p = gone.to_kynput(0, 0, 0);
        assert_eq!((p.flags, p.pressure), (0, 0));
    }

    #[test]
    fn tilt_rotation_eraser_and_barrel() {
        let pen = PenInput {
            in_range: true,
            contact: true,
            pressure: 0.5,
            tilt: Some((-120.0, 30.4)),
            rotation: Some(-90.0),
            eraser: true,
            barrel: true,
        };
        let p = pen.to_kynput(0, 0, 0);
        assert_eq!((p.tilt_x, p.tilt_y, p.rotation), (-90, 30, 270));
        for f in [Pen::TILT, Pen::ROTATION, Pen::ERASER, Pen::BARREL] {
            assert!(p.has(f));
        }
        let plain = PenInput { in_range: true, ..Default::default() }.to_kynput(0, 0, 0);
        assert!(!plain.has(Pen::TILT) && !plain.has(Pen::ROTATION));
    }

    #[test]
    fn altitude_azimuth() {
        assert_eq!(tilt_from_altitude_azimuth(FRAC_PI_2, 1.0), (0.0, 0.0));
        let (tx, ty) = tilt_from_altitude_azimuth(FRAC_PI_4, 0.0);
        assert!((tx - 45.0).abs() < 0.01 && ty.abs() < 0.01, "{tx} {ty}");
        let (tx, ty) = tilt_from_altitude_azimuth(FRAC_PI_4, PI / 2.0);
        assert!(tx.abs() < 0.01 && (ty - 45.0).abs() < 0.01, "{tx} {ty}");
        assert_eq!(tilt_from_altitude_azimuth(0.0, PI), (-90.0, 0.0));
    }
}
