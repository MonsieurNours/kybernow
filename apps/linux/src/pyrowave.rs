//! PyroWave dans l'interface (docs/link-ui.md, « PyroWave » ; docs/pyrowave-client-linux.md) : la valeur de
//! « Format vidéo », ses aides et avertissements, le bandeau du début de session, la puce de la barre, les lignes
//! des statistiques. Les textes sont ceux de link-ui, mot pour mot. Sans SDL ni cœur : les captures s'en servent.

use crate::i18n::{decimal, tr};

/// Par où passe une session PyroWave qui risque de saccader : le Wi-Fi, ou le relais (hors de chez soi). Par câble,
/// rien à dire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    Wifi,
    Relay,
}

impl Risk {
    /// La puce ambre de la barre et le titre du bandeau.
    pub fn chip(self) -> &'static str {
        match self {
            Risk::Wifi => tr("PyroWave en Wi-Fi", "PyroWave on Wi-Fi"),
            Risk::Relay => tr("PyroWave hors de chez vous", "PyroWave away from home"),
        }
    }

    /// L'avertissement sous le choix, dans « Format vidéo ».
    pub fn warning(self) -> &'static str {
        match self {
            Risk::Wifi => tr(
                "En Wi-Fi, PyroWave peut saccader : il lui faut environ 300 Mbit/s en 1080p. Gardez « Auto » si vous n'êtes pas sûr.",
                "On Wi-Fi, PyroWave may stutter: it needs about 300 Mbit/s at 1080p. Keep “Auto” if you're not sure.",
            ),
            Risk::Relay => tr(
                "Hors de chez vous, PyroWave passe par le relais et par votre connexion Internet : l'image risque de saccader. Gardez « Auto » si vous n'êtes pas sûr.",
                "Away from home, PyroWave goes through the relay and your internet connection: the picture may stutter. Keep “Auto” if you're not sure.",
            ),
        }
    }

    /// Le risque d'une session : le relais d'abord, puis le Wi-Fi.
    pub fn of(relay: bool, wired: bool) -> Option<Risk> {
        if relay {
            Some(Risk::Relay)
        } else if !wired {
            Some(Risk::Wifi)
        } else {
            None
        }
    }
}

/// Le titre du bandeau quand le réseau sature en cours de session (avis `PyroWaveStrained` du cœur), par câble
/// aussi ; le reste du bandeau est le même que celui du début.
pub fn strained_title() -> &'static str {
    tr("Le réseau sature", "The network is saturated")
}

/// Le texte du bandeau, sous le titre.
pub fn banner_text() -> &'static str {
    tr("L'image peut saccader.", "The picture may stutter.")
}

/// Le bouton du bandeau : relance l'image en HEVC, pour la session seulement.
pub fn switch_to_hevc() -> &'static str {
    tr("Passer en HEVC", "Switch to HEVC")
}

/// Le bandeau disparaît seul au bout de 8 s (ou à la croix).
pub const BANNER_SECONDS: u64 = 8;

/// L'aide de « Format vidéo », PyroWave choisi.
pub fn chosen_help() -> &'static str {
    tr(
        "Presque aucun délai de compression, mais 5 à 10 fois plus de débit que HEVC. Fait pour un câble Ethernet.",
        "Almost no compression delay, but 5 to 10 times the bitrate of HEVC. Made for an Ethernet cable.",
    )
}

/// L'aide de « Format vidéo » en « Auto », quand l'ordinateur propose PyroWave et que cet appareil le décode.
pub fn auto_help(wired: bool) -> &'static str {
    if wired {
        tr("Auto : PyroWave, car cet appareil est relié par câble. Sinon HEVC.", "Auto: PyroWave, because this device is connected by cable. Otherwise HEVC.")
    } else {
        tr("Auto : HEVC. PyroWave est pris seulement quand cet appareil est relié par câble.", "Auto: HEVC. PyroWave is only used when this device is connected by cable.")
    }
}

/// L'aide de « Texte net (4:4:4) » avec PyroWave.
pub fn yuv444_help() -> &'static str {
    tr("Avec PyroWave, presque sans coût : seul le débit monte.", "With PyroWave, almost free: only the bitrate goes up.")
}

/// L'appareil qui décode, pour la raison d'une valeur PyroWave inactive : un Raspberry Pi (sa génération, et
/// KyberNow Link OS ou Raspberry Pi OS : docs/pyrowave-client-pi.md), ou un autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decoder {
    Device,
    Pi { generation: u8, link_os: bool },
}

/// Pourquoi la valeur PyroWave est inactive : l'ordinateur `name` ne le propose pas, ou cet appareil ne le décode
/// pas (dans cet ordre). `None` : rien ne l'empêche. Sur le Raspberry Pi, « {cet appareil} » de link-ui devient
/// « Ce Raspberry Pi », avec ce qu'il lui faut : un Pi 5 ; sur un Pi 5 sous Raspberry Pi OS, le système à jour.
pub fn unavailable(name: &str, offered: bool, decodes: bool, who: Decoder) -> Option<String> {
    if !offered {
        Some(if crate::i18n::english() { format!("{name} doesn't offer PyroWave.") } else { format!("{name} ne propose pas PyroWave.") })
    } else if !decodes {
        Some(
            match who {
                Decoder::Device => tr("Cet appareil ne sait pas décoder PyroWave.", "This device can't decode PyroWave."),
                Decoder::Pi { generation, .. } if generation < 5 => tr(
                    "Ce Raspberry Pi ne sait pas décoder PyroWave : il faut un Raspberry Pi 5.",
                    "This Raspberry Pi can't decode PyroWave: it takes a Raspberry Pi 5.",
                ),
                Decoder::Pi { link_os: false, .. } => tr(
                    "Ce Raspberry Pi ne sait pas décoder PyroWave : il lui faut Raspberry Pi OS « Trixie », à jour.",
                    "This Raspberry Pi can't decode PyroWave: it needs an up-to-date Raspberry Pi OS “Trixie”.",
                ),
                Decoder::Pi { .. } => tr("Ce Raspberry Pi ne sait pas décoder PyroWave.", "This Raspberry Pi can't decode PyroWave."),
            }
            .into(),
        )
    } else {
        None
    }
}

/// « Définition ramenée à 1920 × 1080 pour tenir dans 300 Mbit/s. » (bandeau et statistiques).
pub fn lowered(width: u32, height: u32, mbps: u32) -> String {
    if crate::i18n::english() {
        format!("Resolution lowered to {width} × {height} to fit in {mbps} Mbit/s.")
    } else {
        format!("Définition ramenée à {width} × {height} pour tenir dans {mbps} Mbit/s.")
    }
}

/// Statistiques : le libellé de la ligne « Images perdues ».
pub fn lost_label() -> &'static str {
    tr("Images perdues", "Lost frames")
}

/// Statistiques : la part des images perdues, « 0,8 % » (`lost` sur `lost + shown`).
pub fn lost_value(lost: u32, shown: u32) -> String {
    let total = lost + shown;
    let pct = if total == 0 { 0.0 } else { f64::from(lost) * 100.0 / f64::from(total) };
    format!("{} %", decimal(pct, 1))
}

/// Statistiques : le libellé de la ligne « Définition ramenée ».
pub fn lowered_label() -> &'static str {
    tr("Définition ramenée", "Resolution lowered")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::force_english;

    #[test]
    fn textes_de_link_ui() {
        force_english(false);
        assert_eq!(Risk::Wifi.chip(), "PyroWave en Wi-Fi");
        assert_eq!(Risk::Relay.chip(), "PyroWave hors de chez vous");
        let d = Decoder::Device;
        let pi = |generation, link_os| Decoder::Pi { generation, link_os };
        assert_eq!(unavailable("Atelier", false, true, d).unwrap(), "Atelier ne propose pas PyroWave.");
        assert_eq!(unavailable("Atelier", true, false, d).unwrap(), "Cet appareil ne sait pas décoder PyroWave.");
        assert_eq!(unavailable("Atelier", true, true, d), None);
        assert_eq!(unavailable("Atelier", false, false, pi(4, true)).unwrap(), "Atelier ne propose pas PyroWave.");
        assert_eq!(unavailable("Atelier", true, false, pi(4, true)).unwrap(), "Ce Raspberry Pi ne sait pas décoder PyroWave : il faut un Raspberry Pi 5.");
        assert_eq!(unavailable("Atelier", true, false, pi(3, false)).unwrap(), "Ce Raspberry Pi ne sait pas décoder PyroWave : il faut un Raspberry Pi 5.");
        assert_eq!(unavailable("Atelier", true, false, pi(5, false)).unwrap(), "Ce Raspberry Pi ne sait pas décoder PyroWave : il lui faut Raspberry Pi OS « Trixie », à jour.");
        assert_eq!(unavailable("Atelier", true, false, pi(5, true)).unwrap(), "Ce Raspberry Pi ne sait pas décoder PyroWave.");
        assert_eq!(unavailable("Atelier", true, true, pi(5, true)), None);
        assert_eq!(lowered(1920, 1080, 300), "Définition ramenée à 1920 × 1080 pour tenir dans 300 Mbit/s.");
        assert_eq!(lost_value(8, 992), "0,8 %");
        assert_eq!(lost_value(0, 0), "0,0 %");
        assert_eq!(strained_title(), "Le réseau sature");
        force_english(true);
        assert_eq!(strained_title(), "The network is saturated");
        assert_eq!(Risk::Relay.chip(), "PyroWave away from home");
        assert_eq!(unavailable("Atelier", false, false, Decoder::Device).unwrap(), "Atelier doesn't offer PyroWave.");
        assert_eq!(unavailable("Atelier", true, false, Decoder::Pi { generation: 4, link_os: true }).unwrap(), "This Raspberry Pi can't decode PyroWave: it takes a Raspberry Pi 5.");
        assert_eq!(unavailable("Atelier", true, false, Decoder::Pi { generation: 5, link_os: true }).unwrap(), "This Raspberry Pi can't decode PyroWave.");
        assert_eq!(switch_to_hevc(), "Switch to HEVC");
        assert_eq!(lost_value(8, 992), "0.8 %");
    }

    #[test]
    fn risque_d_une_session() {
        assert_eq!(Risk::of(true, true), Some(Risk::Relay));
        assert_eq!(Risk::of(false, false), Some(Risk::Wifi));
        assert_eq!(Risk::of(false, true), None);
    }
}
