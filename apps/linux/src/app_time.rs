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

/// « aujourd'hui, 9:12 », « hier, 21:40 », sinon « 12/10 ».
pub fn relative_day(at: u64) -> String {
    let (Some(t), Some(n)) = (local(at as i64), local(now())) else { return String::new() };
    let day = |tm: &libc::tm| tm.tm_year * 400 + tm.tm_yday;
    let hm = format!("{}:{:02}", t.tm_hour, t.tm_min);
    match day(&n) - day(&t) {
        0 => format!("{}, {hm}", tr("aujourd'hui", "today")),
        1 => format!("{}, {hm}", tr("hier", "yesterday")),
        _ => format!("{:02}/{:02}", t.tm_mday, t.tm_mon + 1),
    }
}
