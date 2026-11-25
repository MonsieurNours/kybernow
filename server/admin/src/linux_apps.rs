//! Station Linux : l'app ouverte au début de la session (`launch.rs`).
//!
//! **Catalogue.** Les applications de la session, comme les montre le menu
//! du bureau : les fichiers `.desktop` de `applications/` dans
//! `XDG_DATA_HOME` puis `XDG_DATA_DIRS` (paquets, Flatpak, Snap ; le premier
//! trouvé l'emporte, comme le veut la spécification), sans celles cachées
//! (`NoDisplay`, `Hidden`, `OnlyShowIn`/`NotShowIn` du bureau courant,
//! `TryExec` absent) ni celles du terminal. L'identifiant d'une app est
//! celui de son fichier (`org.mozilla.firefox.desktop`). L'icône : un chemin,
//! ou un nom cherché dans les thèmes d'icônes (`hicolor` d'abord) et
//! `pixmaps`, en PNG ; une icône SVG passe par `rsvg-convert` s'il est là.
//! Suggestions : Steam Big Picture (si Steam est installé, paquet ou
//! Flatpak), Lutris et Heroic.
//!
//! **Lancement.** Admin vit déjà dans la session (service systemd de
//! l'utilisateur) : pas d'agent. Chaque app part dans **son propre service
//! systemd de l'utilisateur** (`systemd-run --user`, unité
//! `kybernow-app-<empreinte>`), hors du groupe d'Admin : redémarrer Admin ne
//! la ferme pas, et la fermer arrête tout ce qu'elle a lancé (`systemctl
//! --user stop`). La ligne `Exec` du fichier est découpée et ses codes (`%U`,
//! `%i`…) remplacés comme le veut la spécification ; une adresse
//! (`steam://open/bigpicture`) passe par `xdg-open`. Une app dont le service
//! tourne encore est dite « déjà ouverte » : Wayland ne laisse pas ramener
//! une fenêtre au premier plan. Session verrouillée : `LockedHint` de
//! `loginctl`, et l'on attend le déverrouillage comme sous Windows.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Une entrée `[Desktop Entry]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: Option<String>,
    pub try_exec: Option<String>,
    pub app: bool,
    pub no_display: bool,
    pub hidden: bool,
    pub terminal: bool,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
}

impl DesktopEntry {
    /// L'app paraît dans le menu de ce bureau (`XDG_CURRENT_DESKTOP`, liste
    /// séparée par des deux-points).
    pub fn visible(&self, desktops: &[String]) -> bool {
        let shown_here = self.only_show_in.is_empty() || self.only_show_in.iter().any(|d| desktops.contains(d));
        let hidden_here = self.not_show_in.iter().any(|d| desktops.contains(d));
        self.app && !self.no_display && !self.hidden && !self.terminal && !self.exec.is_empty() && shown_here && !hidden_here
    }
}

/// Valeur d'une clé : `\s`, `\n`, `\t`, `\r`, `\\` remplacés.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn list(value: &str) -> Vec<String> {
    value.split(';').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// Les langues à essayer pour `Name[…]`, de la plus précise à la moins
/// (`fr_FR.UTF-8` → `fr_FR`, `fr`).
pub fn locales(lang: &str) -> Vec<String> {
    let base = lang.split(['.', '@']).next().unwrap_or("");
    if base.is_empty() || base == "C" || base == "POSIX" {
        return Vec::new();
    }
    let mut out = vec![base.to_string()];
    if let Some((short, _)) = base.split_once('_') {
        out.push(short.to_string());
    }
    out
}

/// Lit le groupe `[Desktop Entry]` d'un fichier `.desktop`.
pub fn parse_desktop(text: &str, locales: &[String]) -> Option<DesktopEntry> {
    let mut in_entry = false;
    let mut seen_entry = false;
    let mut keys: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            seen_entry |= in_entry;
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            keys.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    if !seen_entry {
        return None;
    }
    let get = |k: &str| keys.get(k).map(|v| unescape(v));
    let yes = |k: &str| get(k).is_some_and(|v| v == "true");
    let name = locales.iter().find_map(|l| get(&format!("Name[{l}]"))).or_else(|| get("Name"))?;
    Some(DesktopEntry {
        name,
        exec: get("Exec").unwrap_or_default(),
        icon: get("Icon").filter(|i| !i.is_empty()),
        try_exec: get("TryExec").filter(|t| !t.is_empty()),
        app: get("Type").as_deref() == Some("Application"),
        no_display: yes("NoDisplay"),
        hidden: yes("Hidden"),
        terminal: yes("Terminal"),
        only_show_in: get("OnlyShowIn").map(|v| list(&v)).unwrap_or_default(),
        not_show_in: get("NotShowIn").map(|v| list(&v)).unwrap_or_default(),
    })
}

/// Découpe la ligne `Exec` en arguments et remplace ses codes : rien pour
/// les fichiers et adresses (`%f %F %u %U`), l'icône (`%i`), le nom
/// (`%c`), le fichier (`%k`). `None` : ligne mal formée.
pub fn exec_argv(exec: &str, name: &str, icon: Option<&str>, file: &Path) -> Option<Vec<String>> {
    // 1. Arguments, entre guillemets ou non (\" \` \$ \\ dans les guillemets).
    let mut words: Vec<(String, bool)> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_word = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                has_word = true;
            }
            '\\' if quoted => match chars.next() {
                Some(n @ ('"' | '`' | '$' | '\\')) => current.push(n),
                Some(n) => {
                    current.push('\\');
                    current.push(n);
                }
                None => return None,
            },
            ' ' | '\t' if !quoted => {
                if has_word {
                    words.push((std::mem::take(&mut current), false));
                    has_word = false;
                }
            }
            _ => {
                current.push(c);
                has_word = true;
            }
        }
    }
    if quoted {
        return None;
    }
    if has_word {
        words.push((current, false));
    }
    // 2. Codes.
    let mut argv = Vec::new();
    for (word, _) in words {
        match word.as_str() {
            "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%n" | "%N" | "%v" | "%m" => continue,
            "%i" => {
                if let Some(icon) = icon {
                    argv.push("--icon".to_string());
                    argv.push(icon.to_string());
                }
                continue;
            }
            _ => {}
        }
        let mut out = String::new();
        let mut it = word.chars();
        while let Some(c) = it.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            match it.next() {
                Some('%') => out.push('%'),
                Some('c') => out.push_str(name),
                Some('k') => out.push_str(&file.display().to_string()),
                Some('f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm' | 'i') | None => {}
                Some(other) => {
                    out.push('%');
                    out.push(other);
                }
            }
        }
        argv.push(out);
    }
    (!argv.is_empty()).then_some(argv)
}

/// Les dossiers de données, dans l'ordre de préférence (`XDG_DATA_HOME`,
/// puis `XDG_DATA_DIRS`).
pub fn data_dirs(home: Option<&str>, data_home: Option<&str>, data_dirs: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    match (data_home.filter(|d| !d.is_empty()), home) {
        (Some(d), _) => out.push(PathBuf::from(d)),
        (None, Some(h)) => out.push(Path::new(h).join(".local/share")),
        _ => {}
    }
    let dirs = data_dirs.filter(|d| !d.is_empty()).unwrap_or("/usr/local/share:/usr/share");
    for d in dirs.split(':').filter(|d| !d.is_empty()) {
        let p = PathBuf::from(d);
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Les fichiers `.desktop` (identifiant, chemin), le premier trouvé pour un
/// identifiant l'emportant. L'identifiant d'un fichier d'un sous-dossier
/// prend des tirets (`kde4/okular.desktop` → `kde4-okular.desktop`).
pub fn desktop_files(dirs: &[PathBuf]) -> Vec<(String, PathBuf)> {
    fn walk(base: &Path, dir: &Path, depth: u32, out: &mut Vec<(String, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            if path.is_dir() {
                if depth < 3 {
                    walk(base, &path, depth + 1, out);
                }
            } else if path.extension().is_some_and(|x| x == "desktop")
                && let Ok(rel) = path.strip_prefix(base)
            {
                out.push((rel.to_string_lossy().replace('/', "-"), path));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for d in dirs {
        let base = d.join("applications");
        let mut found = Vec::new();
        walk(&base, &base, 0, &mut found);
        for (id, path) in found {
            if seen.insert(id.clone()) {
                out.push((id, path));
            }
        }
    }
    out
}

/// Tailles d'icônes, de la préférée à la moins bonne.
const ICON_SIZES: [&str; 9] = ["128x128", "256x256", "96x96", "64x64", "512x512", "48x48", "scalable", "32x32", "symbolic"];

/// Le fichier d'une icône : un chemin tel quel, sinon un nom cherché dans
/// les thèmes (`hicolor` d'abord, puis les autres) et `pixmaps`, PNG avant SVG.
pub fn find_icon(icon: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    if icon.starts_with('/') {
        let p = PathBuf::from(icon);
        return p.is_file().then_some(p);
    }
    if icon.contains('/') || icon.contains("..") {
        return None;
    }
    for ext in ["png", "svg"] {
        for d in dirs {
            let icons = d.join("icons");
            let mut themes = vec![icons.join("hicolor")];
            if let Ok(entries) = std::fs::read_dir(&icons) {
                let mut others: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir() && !p.ends_with("hicolor")).collect();
                others.sort();
                themes.extend(others);
            }
            for theme in &themes {
                for size in ICON_SIZES {
                    let p = theme.join(size).join("apps").join(format!("{icon}.{ext}"));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
            let p = d.join("pixmaps").join(format!("{icon}.{ext}"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Le programme de `argv` pour reconnaître l'app (`None` pour un lanceur
/// générique : `flatpak run …`, `env …`, `sh -c …`).
pub fn program(argv: &[String]) -> Option<String> {
    let first = Path::new(argv.first()?).file_name()?.to_string_lossy().into_owned();
    (!["flatpak", "env", "sh", "bash", "snap", "gtk-launch", "gio", "kioclient", "kioclient5", "xdg-open"].contains(&first.as_str())).then_some(first)
}

/// Nom de l'unité systemd d'une app (`kybernow-app-<16 hex>.service`).
pub fn unit_name(target: &str) -> String {
    let icon = crate::launch::icon_name(target);
    format!("kybernow-app-{}.service", &icon[..16])
}

/// `LockedHint=yes` de `loginctl show-session`.
pub fn parse_locked(text: &str) -> bool {
    text.lines().any(|l| matches!(l.trim(), "yes" | "LockedHint=yes"))
}

/// Les apps qui méritent une suggestion, d'après les identifiants trouvés.
pub fn suggestion_ids(ids: &[&str]) -> (Option<String>, Vec<(&'static str, &'static str, String)>) {
    let has = |id: &str| ids.iter().any(|i| *i == id);
    // Steam en paquet, ou en Flatpak.
    let steam = ["steam.desktop", "com.valvesoftware.Steam.desktop"].into_iter().find(|i| has(i)).map(str::to_string);
    let mut others = Vec::new();
    for (key, name, candidates) in [
        ("lutris", "Lutris", ["net.lutris.Lutris.desktop", "lutris.desktop"]),
        ("heroic", "Heroic Games Launcher", ["com.heroicgameslauncher.hgl.desktop", "heroic.desktop"]),
    ] {
        if let Some(id) = candidates.into_iter().find(|i| has(i)) {
            others.push((key, name, id.to_string()));
        }
    }
    (steam, others)
}

#[cfg(target_os = "linux")]
pub use station::*;

#[cfg(target_os = "linux")]
mod station {
    use std::process::Command;

    use anyhow::{Context, Result, bail};
    use serde_json::{Value, json};

    use super::*;
    use crate::launch::{AgentCatalogue, AppEntry, Launch, LaunchKind, Root, Suggestion, icon_name};

    fn env(name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }

    fn dirs() -> Vec<PathBuf> {
        data_dirs(env("HOME").as_deref(), env("XDG_DATA_HOME").as_deref(), env("XDG_DATA_DIRS").as_deref())
    }

    fn desktops() -> Vec<String> {
        env("XDG_CURRENT_DESKTOP").map(|d| d.split(':').map(str::to_string).collect()).unwrap_or_default()
    }

    fn user_locales() -> Vec<String> {
        locales(&env("LC_ALL").or_else(|| env("LC_MESSAGES")).or_else(|| env("LANG")).unwrap_or_default())
    }

    fn in_path(program: &str) -> bool {
        if program.starts_with('/') {
            return Path::new(program).is_file();
        }
        env("PATH").unwrap_or_else(|| "/usr/local/bin:/usr/bin:/bin".into()).split(':').any(|d| Path::new(d).join(program).is_file())
    }

    /// Les apps visibles : (identifiant, chemin, entrée).
    fn visible_apps() -> Vec<(String, PathBuf, DesktopEntry)> {
        let (desktops, locales) = (desktops(), user_locales());
        desktop_files(&dirs())
            .into_iter()
            .filter_map(|(id, path)| {
                let entry = parse_desktop(&std::fs::read_to_string(&path).ok()?, &locales)?;
                if !entry.visible(&desktops) || entry.try_exec.as_deref().is_some_and(|t| !in_path(t)) {
                    return None;
                }
                Some((id, path, entry))
            })
            .collect()
    }

    /// Copie (ou convertit) l'icône sous le nom qu'Admin garde.
    fn write_icon(icon: &str, dirs: &[PathBuf], out_dir: &Path, id: &str) -> Option<String> {
        let file = find_icon(icon, dirs)?;
        let name = icon_name(id);
        let dest = out_dir.join(&name);
        if file.extension().is_some_and(|x| x == "svg") {
            let ok = Command::new("rsvg-convert").args(["-w", "128", "-h", "128", "-o"]).arg(&dest).arg(&file).status().is_ok_and(|s| s.success());
            ok.then_some(name)
        } else {
            std::fs::copy(&file, &dest).ok().map(|_| name)
        }
    }

    fn user_dir(kind: &str, fallback: &str) -> String {
        Command::new("xdg-user-dir")
            .arg(kind)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|d| !d.is_empty() && Path::new(d).is_dir())
            .unwrap_or_else(|| format!("{}/{fallback}", env("HOME").unwrap_or_default()))
    }

    pub fn read_catalogue() -> Result<AgentCatalogue> {
        let out_dir = std::env::temp_dir().join(format!("kybernow-apps-linux-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out_dir);
        std::fs::create_dir_all(&out_dir)?;
        let dirs = dirs();
        let found = visible_apps();
        if found.is_empty() {
            bail!("aucune application trouvée dans {}", dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", "));
        }
        let mut apps: Vec<AppEntry> = found
            .iter()
            .map(|(id, path, e)| AppEntry {
                id: id.clone(),
                name: e.name.clone(),
                icon: e.icon.as_deref().and_then(|i| write_icon(i, &dirs, &out_dir, id)),
                exe: exec_argv(&e.exec, &e.name, e.icon.as_deref(), path).as_deref().and_then(program),
            })
            .collect();
        apps.sort_by_key(|a| a.name.to_lowercase());

        let ids: Vec<&str> = found.iter().map(|(id, _, _)| id.as_str()).collect();
        let (steam, others) = suggestion_ids(&ids);
        let icon_of = |id: &str, key: &str| found.iter().find(|(i, _, _)| i == id).and_then(|(_, _, e)| e.icon.as_deref()).and_then(|i| write_icon(i, &dirs, &out_dir, key));
        let mut suggestions = Vec::new();
        if let Some(id) = &steam {
            suggestions.push(Suggestion {
                id: "steam-bigpicture".into(),
                name: "Steam Big Picture".into(),
                kind: LaunchKind::Uri,
                target: "steam://open/bigpicture".into(),
                close: Some("steam://close/bigpicture".into()),
                exe: Some("steam".into()),
                icon: icon_of(id, "steam-bigpicture"),
            });
        }
        for (key, name, id) in others {
            suggestions.push(Suggestion { id: key.into(), name: name.into(), kind: LaunchKind::App, target: id.clone(), close: None, exe: None, icon: icon_of(&id, key) });
        }
        let home = env("HOME").unwrap_or_default();
        let roots = vec![
            Root { id: "userPrograms".into(), path: format!("{home}/Applications") },
            Root { id: "desktop".into(), path: user_dir("DESKTOP", "Desktop") },
            Root { id: "programs".into(), path: "/opt".into() },
        ];
        Ok(AgentCatalogue { user: env("USER"), apps, suggestions, roots, icons_dir: out_dir.display().to_string() })
    }

    fn systemctl(args: &[&str]) -> Result<std::process::Output> {
        Command::new("systemctl").arg("--user").args(args).output().context("systemctl --user")
    }

    fn unit_active(unit: &str) -> bool {
        systemctl(&["is-active", "--quiet", unit]).is_ok_and(|o| o.status.success())
    }

    fn main_pid(unit: &str) -> Option<u32> {
        let out = systemctl(&["show", "--property=MainPID", "--value", unit]).ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok().filter(|p| *p > 0)
    }

    /// Lance `argv` dans son propre service de l'utilisateur.
    fn run_in_unit(unit: &str, argv: &[String]) -> Result<()> {
        let mut cmd = Command::new("systemd-run");
        cmd.args(["--user", "--collect", "--quiet", "--property=TimeoutStopSec=10"]).arg(format!("--unit={unit}"));
        // Le bureau a donné ces variables au gestionnaire de l'utilisateur ;
        // passées quand même, au cas où.
        for var in ["DISPLAY", "WAYLAND_DISPLAY", "XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE", "DBUS_SESSION_BUS_ADDRESS"] {
            if let Some(v) = env(var) {
                cmd.arg(format!("--setenv={var}={v}"));
            }
        }
        let out = cmd.arg("--").args(argv).output().context("systemd-run introuvable")?;
        if !out.status.success() {
            bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }

    /// L'app d'un fichier `.desktop`, en arguments.
    fn argv_of_desktop(path: &Path) -> Result<Vec<String>> {
        let text = std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;
        let entry = parse_desktop(&text, &user_locales()).context("fichier .desktop illisible")?;
        exec_argv(&entry.exec, &entry.name, entry.icon.as_deref(), path).context("ligne Exec illisible")
    }

    fn open_uri(unit_base: &str, uri: &str) -> Result<()> {
        // Une adresse peut resservir (Big Picture rouvert) : une unité à chaque fois.
        let unit = format!("{}-{:08x}.service", unit_base.trim_end_matches(".service"), rand::random::<u32>());
        run_in_unit(&unit, &["xdg-open".to_string(), uri.to_string()])
    }

    pub fn launch(launch: &Launch) -> Result<Value> {
        let unit = unit_name(&launch.target);
        if launch.kind == LaunchKind::Uri {
            open_uri(&unit, &launch.target)?;
            return Ok(json!({ "front": false, "pid": null }));
        }
        if unit_active(&unit) {
            return Ok(json!({ "front": true, "pid": main_pid(&unit) }));
        }
        let argv = match launch.kind {
            LaunchKind::App => {
                let (_, path, _) = visible_apps().into_iter().find(|(id, _, _)| *id == launch.target).with_context(|| format!("app introuvable : {}", launch.target))?;
                argv_of_desktop(&path)?
            }
            _ if launch.target.ends_with(".desktop") => argv_of_desktop(Path::new(&launch.target))?,
            _ => vec![launch.target.clone()],
        };
        run_in_unit(&unit, &argv)?;
        Ok(json!({ "front": false, "pid": main_pid(&unit) }))
    }

    pub fn close(launch: &Launch, pid: Option<u32>) -> Result<Value> {
        let unit = unit_name(&launch.target);
        if let Some(uri) = &launch.close {
            open_uri(&unit, uri)?;
            return Ok(json!({ "closed": 1 }));
        }
        if unit_active(&unit) {
            let out = systemctl(&["stop", &unit])?;
            if !out.status.success() {
                bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
            }
            return Ok(json!({ "closed": 1 }));
        }
        // L'app a passé la main à une autre instance (déjà ouverte avant) :
        // son programme, s'il est connu.
        if let Some(pid) = pid.filter(|p| Path::new(&format!("/proc/{p}")).exists()) {
            let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
            return Ok(json!({ "closed": 1 }));
        }
        if let Some(exe) = &launch.exe {
            let uid = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find_map(|l| l.strip_prefix("Uid:")).and_then(|u| u.split_whitespace().next().map(str::to_string)));
            let mut cmd = Command::new("pkill");
            cmd.args(["-TERM", "-x"]);
            if let Some(uid) = uid {
                cmd.args(["-u", &uid]);
            }
            let closed = cmd.arg(exe.chars().take(15).collect::<String>()).status().is_ok_and(|s| s.success());
            return Ok(json!({ "closed": u32::from(closed) }));
        }
        Ok(json!({ "closed": 0 }))
    }

    /// La session graphique de l'utilisateur est-elle verrouillée ?
    pub fn session_locked() -> bool {
        let session = env("XDG_SESSION_ID").or_else(|| {
            let uid = std::fs::read_to_string("/proc/self/status").ok()?.lines().find_map(|l| l.strip_prefix("Uid:"))?.split_whitespace().next()?.to_string();
            let out = Command::new("loginctl").args(["show-user", &uid, "--property=Display", "--value"]).output().ok()?;
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
        });
        let Some(session) = session else { return false };
        Command::new("loginctl")
            .args(["show-session", &session, "--property=LockedHint"])
            .output()
            .is_ok_and(|o| parse_locked(&String::from_utf8_lossy(&o.stdout)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = "[Desktop Entry]\nVersion=1.0\nName=Firefox Web Browser\nName[fr]=Navigateur Web Firefox\nExec=firefox %u\nIcon=firefox\nTerminal=false\nType=Application\nCategories=Network;WebBrowser;\n\n[Desktop Action new-window]\nName=Open a New Window\nExec=firefox -new-window\n";

    #[test]
    fn reads_desktop_entries() {
        let e = parse_desktop(FIREFOX, &locales("fr_FR.UTF-8")).unwrap();
        assert_eq!((e.name.as_str(), e.exec.as_str(), e.icon.as_deref()), ("Navigateur Web Firefox", "firefox %u", Some("firefox")));
        assert!(e.visible(&["GNOME".into()]));
        assert_eq!(parse_desktop(FIREFOX, &locales("C")).unwrap().name, "Firefox Web Browser");
        assert_eq!(locales("fr_FR.UTF-8"), ["fr_FR", "fr"]);
        // Cachée, réservée à KDE, terminal, pas une app.
        let only_kde = parse_desktop("[Desktop Entry]\nType=Application\nName=Dolphin\nExec=dolphin\nOnlyShowIn=KDE;\n", &[]).unwrap();
        assert!(!only_kde.visible(&["GNOME".into()]) && only_kde.visible(&["KDE".into()]));
        assert!(!parse_desktop("[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n", &[]).unwrap().visible(&[]));
        assert!(!parse_desktop("[Desktop Entry]\nType=Application\nName=htop\nExec=htop\nTerminal=true\n", &[]).unwrap().visible(&[]));
        assert!(!parse_desktop("[Desktop Entry]\nType=Link\nName=Doc\nURL=https://x\n", &[]).unwrap().visible(&[]));
        assert!(parse_desktop("[Autre]\nName=X\n", &[]).is_none());
    }

    #[test]
    fn splits_the_exec_line() {
        let f = Path::new("/usr/share/applications/x.desktop");
        assert_eq!(exec_argv("firefox %u", "Firefox", None, f).unwrap(), ["firefox"]);
        assert_eq!(
            exec_argv("/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=steam com.valvesoftware.Steam @@u %U @@", "Steam", None, f).unwrap(),
            ["/usr/bin/flatpak", "run", "--branch=stable", "--arch=x86_64", "--command=steam", "com.valvesoftware.Steam", "@@u", "@@"]
        );
        assert_eq!(
            exec_argv(r#""/opt/My Game/run.sh" --title "%c" %i --rate=100%%"#, "Mon jeu", Some("mygame"), f).unwrap(),
            ["/opt/My Game/run.sh", "--title", "Mon jeu", "--icon", "mygame", "--rate=100%"]
        );
        assert_eq!(exec_argv(r#"sh -c "echo \"\$HOME\"""#, "x", None, f).unwrap(), ["sh", "-c", "echo \"$HOME\""]);
        assert_eq!(exec_argv("app %k", "x", None, f).unwrap(), ["app", "/usr/share/applications/x.desktop"]);
        assert!(exec_argv(r#"broken "quote"#, "x", None, f).is_none());
        assert!(exec_argv("%U", "x", None, f).is_none());
        // Échappement propre aux fichiers .desktop : \s devient une espace.
        let e = parse_desktop("[Desktop Entry]\nType=Application\nName=A\nExec=\"/opt/a\\sb/run\"\n", &[]).unwrap();
        assert_eq!(exec_argv(&e.exec, "A", None, f).unwrap(), ["/opt/a b/run"]);
    }

    #[test]
    fn finds_desktop_files_and_icons() {
        let root = std::env::temp_dir().join(format!("kn-apps-{}", rand::random::<u64>()));
        let home = root.join("home/.local/share");
        let usr = root.join("usr/share");
        let write = |p: PathBuf, text: &[u8]| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        write(usr.join("applications/firefox.desktop"), FIREFOX.as_bytes());
        write(usr.join("applications/kde4/okular.desktop"), b"[Desktop Entry]\nType=Application\nName=Okular\nExec=okular\n");
        // La copie de l'utilisateur l'emporte.
        write(home.join("applications/firefox.desktop"), b"[Desktop Entry]\nType=Application\nName=Firefox perso\nExec=firefox\n");
        write(usr.join("icons/hicolor/48x48/apps/firefox.png"), b"png48");
        write(usr.join("icons/hicolor/128x128/apps/firefox.png"), b"png128");
        write(usr.join("icons/hicolor/scalable/apps/steam.svg"), b"<svg/>");
        write(usr.join("icons/Adwaita/64x64/apps/okular.png"), b"png");
        write(usr.join("pixmaps/lutris.png"), b"png");

        let dirs = data_dirs(None, Some(home.to_str().unwrap()), Some(&format!("{}:{}", usr.display(), usr.display())));
        assert_eq!(dirs, [home.clone(), usr.clone()]);
        let files = desktop_files(&dirs);
        assert_eq!(files.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["firefox.desktop", "kde4-okular.desktop"]);
        assert!(files[0].1.starts_with(&home));

        assert_eq!(find_icon("firefox", &dirs), Some(usr.join("icons/hicolor/128x128/apps/firefox.png")));
        assert_eq!(find_icon("okular", &dirs), Some(usr.join("icons/Adwaita/64x64/apps/okular.png")));
        assert_eq!(find_icon("steam", &dirs), Some(usr.join("icons/hicolor/scalable/apps/steam.svg")));
        assert_eq!(find_icon("lutris", &dirs), Some(usr.join("pixmaps/lutris.png")));
        assert_eq!(find_icon("../../etc/passwd", &dirs), None);
        assert_eq!(find_icon("absent", &dirs), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn suggestions_units_and_lock() {
        let (steam, others) = suggestion_ids(&["firefox.desktop", "com.valvesoftware.Steam.desktop", "net.lutris.Lutris.desktop"]);
        assert_eq!(steam.as_deref(), Some("com.valvesoftware.Steam.desktop"));
        assert_eq!(others, [("lutris", "Lutris", "net.lutris.Lutris.desktop".to_string())]);
        assert_eq!(suggestion_ids(&["steam.desktop"]).0.as_deref(), Some("steam.desktop"));

        let unit = unit_name("firefox.desktop");
        assert!(unit.starts_with("kybernow-app-") && unit.ends_with(".service") && unit.len() == "kybernow-app-.service".len() + 16);
        assert_eq!(unit, unit_name("firefox.desktop"));

        assert_eq!(program(&["/usr/bin/firefox".into()]).as_deref(), Some("firefox"));
        assert_eq!(program(&["/usr/bin/flatpak".into(), "run".into()]), None);

        assert!(parse_locked("LockedHint=yes\n") && parse_locked("yes\n"));
        assert!(!parse_locked("LockedHint=no\n") && !parse_locked(""));
    }
}
