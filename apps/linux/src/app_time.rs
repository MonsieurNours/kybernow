//! Heure locale pour l'interface (en-tête des interfaces console, dernière
//! session) : `localtime_r` de la bibliothèque C, qui suit le fuseau du système.

use crate::i18n::tr;

fn local(secs: i64) -> Option<libc::tm> {
    let t = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::localtime_r(&t, &mut tm) };
    (!r.is_null()).then_some(tm)
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// « 19:42 ».
pub fn local_hhmm() -> String {
    match local(now()) {
        Some(tm) => format!("{}:{:02}", tm.tm_hour, tm.tm_min),
        None => String::new(),
    }
}

/// Une heure du prêt (docs/link-ui.md, « Prêt de l'ordinateur ») : « 18 h », « 18 h 30 » ; “6 PM”, “6:30 PM”.
pub fn hour_label(at: u64) -> String {
    let Some(tm) = local(at as i64) else { return String::new() };
    let (h, m) = (tm.tm_hour, tm.tm_min);
    if crate::i18n::english() {
        let (h12, ampm) = (if h % 12 == 0 { 12 } else { h % 12 }, if h < 12 { "AM" } else { "PM" });
        if m == 0 { format!("{h12} {ampm}") } else { format!("{h12}:{m:02} {ampm}") }
    } else if m == 0 {
        format!("{h} h")
    } else {
        format!("{h} h {m:02}")
    }
}

/// Le jour de la semaine d'un créneau : « mercredi », “Wednesday”.
pub fn weekday_label(at: u64) -> String {
    let Some(tm) = local(at as i64) else { return String::new() };
    let i = tm.tm_wday.clamp(0, 6) as usize;
    if crate::i18n::english() {
        ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"][i].into()
    } else {
        ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"][i].into()
    }
}

/// « aujourd'hui », « hier », sinon « 12/10 ».
pub fn relative_day(at: u64) -> String {
    let (Some(t), Some(n)) = (local(at as i64), local(now())) else { return String::new() };
    let day = |tm: &libc::tm| tm.tm_year * 400 + tm.tm_yday;
    match day(&n) - day(&t) {
        0 => tr("aujourd'hui", "today").into(),
        1 => tr("hier", "yesterday").into(),
        _ => format!("{:02}/{:02}", t.tm_mday, t.tm_mon + 1),
    }
}
