//! Statistiques (docs/link-ui.md, « Statistiques ») : sous « Débit », la
//! ligne « Débit visé » (« 42 / 60 Mbit/s · baissé, réseau saturé ») et, sur
//! la courbe du débit, un trait au niveau du débit demandé à l'ordinateur,
//! seconde par seconde. Textes du cœur (`adaptive::TargetBitrate`) ; rien sans
//! `Second::target` (avant l'image, invité sur l'image de l'hôte). Sans rien
//! de Windows : testé sur le Mac.

use kybernow_core::adaptive::TargetBitrate;
use kybernow_core::i18n::Lang;
use serde::Serialize;

/// Ce que l'interface reçoit chaque seconde (`Stats::target`, `api.ts`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TargetLine {
    /// Le débit demandé à l'ordinateur (Mbit/s) : niveau du trait.
    pub mbps: f64,
    /// « 42 / 60 Mbit/s · baissé, réseau saturé » (`TargetBitrate::text`).
    pub text: String,
    /// « Débit visé » / “Target bitrate” (`TargetBitrate::label`).
    pub label: String,
}

impl TargetLine {
    pub fn new(target: &TargetBitrate, lang: Lang) -> Self {
        Self { mbps: target.bitrate as f64 / 1_000_000.0, text: target.text(lang), label: TargetBitrate::label(lang) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::adaptive::TargetReason;

    #[test]
    fn debit_vise() {
        let lowered = TargetBitrate { bitrate: 42_000_000, profile: 60_000_000, reason: TargetReason::Lowered };
        let l = TargetLine::new(&lowered, Lang::Fr);
        assert_eq!(l.mbps, 42.0);
        assert_eq!(l.text, lowered.text(Lang::Fr));
        assert!(l.text.starts_with("42 / 60 Mbit/s"), "{}", l.text);
        assert_eq!(l.label, "Débit visé");
        let auto = TargetLine::new(&TargetBitrate { bitrate: 60_000_000, profile: 60_000_000, reason: TargetReason::Auto }, Lang::En);
        assert_eq!(auto.label, "Target bitrate");
        assert!(auto.text.starts_with("60 Mbit/s"), "{}", auto.text);
        // En JSON pour l'interface : `{mbps, text, label}`.
        let json = serde_json::to_value(&auto).unwrap();
        assert_eq!(json["mbps"], 60.0);
        assert!(json["text"].is_string() && json["label"].is_string());
    }
}
