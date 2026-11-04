//! L'image d'un ami (prêt) ou d'un invité (partage) n'a pas démarré dans son
//! navigateur : le client web le dit dans son signe de vie (`error`), avec ce
//! qu'il avait demandé et ce qu'il essaie ensuite, puis, si un repli marche,
//! comment l'image est partie (`started`). Admin l'écrit au journal et dans le
//! fil d'événements (docs/pret.md, docs/kybernow-share.md, « Client web »).

use crate::i18n::Msg;
use serde::Deserialize;
use std::sync::Mutex;

/// Échec au démarrage de l'image, tel que le client web le rapporte.
#[derive(Deserialize, Clone, Debug)]
pub struct StartError {
    /// Le message du moteur (`streaming_start_failed`, `start_streaming: …`, ou ce qu'il a écrit en console).
    pub reason: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub yuv444: bool,
    #[serde(default)]
    pub audio: bool,
    /// Le navigateur (`navigator.userAgent`).
    #[serde(default)]
    pub browser: String,
    /// Le repli que la page essaie ensuite : `h264`, `no_audio`, ou rien (elle s'arrête là).
    #[serde(default)]
    pub next: Option<String>,
}

/// L'image est partie après un ou plusieurs échecs : le format retenu.
#[derive(Deserialize, Clone, Debug)]
pub struct Started {
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub audio: bool,
}

/// Rapports retenus par ami ou invité : un navigateur qui boucle ne remplit pas le fil.
const MAX_REPORTS: u32 = 6;
const MAX_REASON: usize = 300;
const MAX_BROWSER: usize = 200;

/// Rapports déjà reçus, par identifiant (prêt ou invité), les plus récents à la fin.
static COUNTS: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());

/// Compte un rapport de plus pour `id` : faux au-delà de `MAX_REPORTS`.
fn admit(id: &str) -> bool {
    let mut counts = COUNTS.lock().unwrap();
    let n = match counts.iter_mut().find(|(k, _)| k == id) {
        Some((_, n)) => {
            *n += 1;
            *n
        }
        None => {
            counts.push((id.to_string(), 1));
            if counts.len() > 32 {
                counts.remove(0);
            }
            1
        }
    };
    n <= MAX_REPORTS
}

/// Texte d'une ligne, sans caractères de contrôle, borné.
pub fn clean(text: &str, max: usize) -> String {
    let one: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let one = one.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        return one;
    }
    let mut out: String = one.chars().take(max).collect();
    out.push('…');
    out
}

/// Le format, tel que le fil le dit : « HEVC », « H.264 », « AV1 » (et 4:4:4).
fn format_name(codec: &str, yuv444: bool) -> String {
    let name = match codec.to_ascii_lowercase().as_str() {
        "hevc" | "h265" => "HEVC".to_string(),
        "h264" => "H.264".to_string(),
        "av1" => "AV1".to_string(),
        "" => "?".to_string(),
        other => clean(other, 12),
    };
    if yuv444 { format!("{name} 4:4:4") } else { name }
}

/// Qui : un ami (`lend`) ou un invité (`share`), pour le journal.
#[derive(Clone, Copy)]
pub enum Who {
    Friend,
    Guest,
}

impl Who {
    fn label(self) -> &'static str {
        match self {
            Who::Friend => "prêt",
            Who::Guest => "partage",
        }
    }
}

/// Écrit l'échec au journal et, dans la limite des rapports, au fil. Rend la raison nettoyée
/// (la carte du prêt la montre), ou `None` au-delà de la limite.
pub fn report_error(activity: &crate::activity::Activity, who: Who, id: &str, name: &str, e: &StartError) -> Option<String> {
    if !admit(id) {
        return None;
    }
    let reason = clean(&e.reason, MAX_REASON);
    let reason = if reason.is_empty() { "?".to_string() } else { reason };
    let next = e.next.as_deref().unwrap_or("");
    tracing::warn!(
        "{} : {name} n'a pas pu démarrer l'image ({}, son {}, navigateur « {} ») : {reason}{}",
        who.label(),
        format_name(&e.codec, e.yuv444),
        if e.audio { "demandé" } else { "coupé" },
        clean(&e.browser, MAX_BROWSER),
        match next {
            "h264" => " ; nouvel essai en H.264",
            "no_audio" => " ; nouvel essai sans le son",
            _ => "",
        }
    );
    let next_msg = match next {
        "h264" => Some(Msg::new("events.retryH264")),
        "no_audio" => Some(Msg::new("events.retryNoAudio")),
        _ => None,
    };
    let msg = Msg::new("events.startFailed").with("name", name).with("reason", reason.as_str());
    let msg = match next_msg {
        Some(m) => msg.with_msg("next", m),
        None => msg.with("next", ""),
    };
    activity.event("warn", msg);
    Some(reason)
}

/// L'image est partie après un échec : au journal et au fil.
pub fn report_started(activity: &crate::activity::Activity, who: Who, id: &str, name: &str, st: &Started) {
    if !admit(id) {
        return;
    }
    let format = format_name(&st.codec, false);
    tracing::info!("{} : image de {name} démarrée en {format}{}", who.label(), if st.audio { "" } else { ", sans le son" });
    let msg = Msg::new("events.startedAfter").with("name", name).with("format", format.as_str());
    let msg = if st.audio { msg.with("audio", "") } else { msg.with_msg("audio", Msg::new("events.noAudioSuffix")) };
    activity.event("info", msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_raison_tient_sur_une_ligne_bornee() {
        assert_eq!(clean("a\nb\t c", 20), "a b c");
        assert_eq!(clean(&"x".repeat(310), MAX_REASON).chars().count(), MAX_REASON + 1);
    }

    #[test]
    fn un_navigateur_qui_boucle_ne_remplit_pas_le_fil() {
        let id = "essai-boucle";
        let admitted = (0..10).filter(|_| admit(id)).count();
        assert_eq!(admitted, MAX_REPORTS as usize);
    }

    #[test]
    fn le_format_se_lit() {
        assert_eq!(format_name("hevc", false), "HEVC");
        assert_eq!(format_name("h264", true), "H.264 4:4:4");
    }
}
