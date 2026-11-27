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

/// Un jour : « 26 novembre », “November 26” (le test du profil Auto).
pub fn date_label(at: u64) -> String {
    let Some(tm) = local(at as i64) else { return String::new() };
    let m = tm.tm_mon.clamp(0, 11) as usize;
    if crate::i18n::english() {
        let months = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
        format!("{} {}", months[m], tm.tm_mday)
    } else {
        let months = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
        let day = if tm.tm_mday == 1 { "1er".to_string() } else { tm.tm_mday.to_string() };
        format!("{day} {}", months[m])
    }
}

/// « aujourd'hui », « hier », sinon « 12/10 ».
pub fn relative_day(at: u64) -> String {
    relative_day_at(at, now())
}

/// [`relative_day`] à l'instant `now` : la date locale de `at` comparée à celle d'aujourd'hui et d'hier (un jour
/// plus tôt), jamais par un numéro de jour dans l'année (« hier » le 1er janvier : LINAPP-26).
fn relative_day_at(at: u64, now: i64) -> String {
    let (Some(t), Some(n), Some(y)) = (local(at as i64), local(now), local(now - 86_400)) else { return String::new() };
    let same = |a: &libc::tm, b: &libc::tm| a.tm_year == b.tm_year && a.tm_mon == b.tm_mon && a.tm_mday == b.tm_mday;
    if same(&t, &n) {
        tr("aujourd'hui", "today").into()
    } else if same(&t, &y) {
        tr("hier", "yesterday").into()
    } else {
        format!("{:02}/{:02}", t.tm_mday, t.tm_mon + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LINAPP-26 : le 1er janvier, la veille est « hier », pas le 31/12.
    #[test]
    fn hier_au_nouvel_an() {
        crate::i18n::force_english(false);
        // Midi le 1er janvier 2027 (heure locale à peu près : midi ± le fuseau reste le même jour).
        let jan1 = local_noon(2027, 0, 1);
        assert_eq!(relative_day_at((jan1 - 86_400) as u64, jan1), "hier");
        assert_eq!(relative_day_at(jan1 as u64, jan1), "aujourd'hui");
        assert_eq!(relative_day_at((jan1 - 2 * 86_400) as u64, jan1), "30/12");
    }

    fn local_noon(year: i32, mon: i32, day: i32) -> i64 {
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        tm.tm_year = year - 1900;
        tm.tm_mon = mon;
        tm.tm_mday = day;
        tm.tm_hour = 12;
        tm.tm_isdst = -1;
        unsafe { libc::mktime(&mut tm) as i64 }
    }
}
