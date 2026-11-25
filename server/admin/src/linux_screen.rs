//! Station Linux : l'écran de l'ordinateur vu par Admin.
//!
//! **Aperçu de l'accueil** (`preview.rs`). GNOME et KDE ne laissent aucun
//! programme lire l'écran sans le dialogue du partage d'écran… sauf par le
//! compositeur lui-même : c'est ce que fait l'agent `kybernow-display`, qui
//! tient hors session un flux PipeWire par écran de l'ordinateur (Mutter
//! `RecordMonitor`, KWin `stream_output`) et les écrit dans le fichier des
//! écrans de Kyber (`kyber-screens.toml`). L'aperçu lit **une image** du flux
//! de l'écran principal (celui placé en 0, 0) par GStreamer, la réduit et
//! l'enregistre en JPEG (`gst-launch-1.0 pipewiresrc … num-buffers=1 !
//! videoscale ! jpegenc`) : aucun dialogue, rien de plus à installer que ce
//! que Kyber demande déjà (plugins GStreamer PipeWire, `jpegenc` des plugins
//! « good »). Jamais pendant une session (`preview.rs`).
//!
//! **Plein écran** (`foreground.rs`, souris Auto). Wayland ne dit à personne
//! quelle fenêtre a la main. Mais les jeux de Steam (Proton, et la plupart
//! des jeux natifs) sont des fenêtres X11, servies par Xwayland, dont GNOME
//! et KDE restent le gestionnaire de fenêtres : la fenêtre X11 active
//! (`_NET_ACTIVE_WINDOW`), son état plein écran (`_NET_WM_STATE_FULLSCREEN`)
//! et son programme (`_NET_WM_PID`, `WM_CLASS`) se lisent par `xprop`. Un
//! jeu en fenêtre Wayland native n'est pas vu (la souris reste celle du
//! bureau) ; sous X11, tout est vu.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

// --- Aperçu -----------------------------------------------------------------

/// Le fichier des écrans que tient l'agent (le même chemin que lui).
pub fn screens_file() -> PathBuf {
    if let Some(p) = std::env::var_os("KYBERNOW_SCREENS_FILE").filter(|p| !p.is_empty()) {
        return PathBuf::from(p);
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    data.join("kybernow/station/kyber-screens.toml")
}

/// Un écran du fichier des écrans.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub id: u32,
    /// Élément GStreamer (`pipewiresrc target-object=… …`).
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub x: i64,
    pub y: i64,
}

/// Les écrans du fichier (`[[video_sources]]`).
pub fn parse_sources(text: &str) -> Vec<Source> {
    let Ok(v) = text.parse::<toml::Table>() else { return Vec::new() };
    let Some(list) = v.get("video_sources").and_then(|s| s.as_array()) else { return Vec::new() };
    list.iter()
        .filter_map(|s| {
            let int = |k: &str| s.get(k).and_then(toml::Value::as_integer);
            Some(Source {
                id: u32::try_from(int("id")?).ok()?,
                source: s.get("source")?.as_str()?.to_string(),
                width: u32::try_from(int("width")?).ok().filter(|w| *w > 0)?,
                height: u32::try_from(int("height")?).ok().filter(|h| *h > 0)?,
                x: int("x").unwrap_or(0),
                y: int("y").unwrap_or(0),
            })
        })
        .collect()
}

/// L'écran principal : celui placé en (0, 0), sinon le premier.
pub fn primary(sources: &[Source]) -> Option<&Source> {
    sources.iter().find(|s| s.x == 0 && s.y == 0).or_else(|| sources.first())
}

/// Arguments de `gst-launch-1.0` : une image de `source`, réduite à `width`
/// pixels de large (jamais agrandie), en JPEG dans `out`. `None` si la
/// source n'est pas un `pipewiresrc` aux propriétés simples (le fichier vient
/// de l'agent : rien d'autre ne doit entrer dans le pipeline).
pub fn pipeline(source: &Source, width: u32, quality: u8, out: &Path) -> Option<Vec<String>> {
    let mut words = source.source.split_whitespace();
    if words.next() != Some("pipewiresrc") {
        return None;
    }
    let props: Vec<&str> = words.collect();
    let safe = |w: &&str| {
        w.split_once('=').is_some_and(|(k, v)| {
            !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') && !v.is_empty() && v.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
        })
    };
    if !props.iter().all(safe) {
        return None;
    }
    let w = width.min(source.width).max(1);
    let h = ((u64::from(source.height) * u64::from(w) + u64::from(source.width) / 2) / u64::from(source.width)).max(1);
    let mut args = vec!["-q".to_string(), "pipewiresrc".to_string()];
    args.extend(props.iter().filter(|p| !p.starts_with("num-buffers=")).map(|p| p.to_string()));
    args.push("num-buffers=1".into());
    for part in ["!", "videoconvert", "!", "videoscale", "!"] {
        args.push(part.into());
    }
    args.push(format!("video/x-raw,width={w},height={h},pixel-aspect-ratio=1/1"));
    args.extend(["!".to_string(), "jpegenc".to_string(), format!("quality={quality}"), "!".to_string(), "filesink".to_string()]);
    args.push(format!("location={}", out.display()));
    Some(args)
}

/// Capture de l'écran principal en JPEG dans `out` (10 s au plus).
#[cfg(target_os = "linux")]
pub fn capture(out: &Path, width: u32, quality: u8) -> anyhow::Result<()> {
    use anyhow::{Context, bail};
    let file = screens_file();
    let text = std::fs::read_to_string(&file).with_context(|| format!("écrans de la Station inconnus ({})", file.display()))?;
    let sources = parse_sources(&text);
    let Some(screen) = primary(&sources) else { bail!("aucun écran à capturer") };
    let Some(args) = pipeline(screen, width, quality, out) else { bail!("source d'écran inattendue : {}", screen.source) };
    let mut child = std::process::Command::new("gst-launch-1.0")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("gst-launch-1.0 introuvable")?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() && out.metadata().is_ok_and(|m| m.len() > 0) {
                return Ok(());
            }
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                use std::io::Read;
                let _ = e.read_to_string(&mut err);
            }
            bail!("capture de l'écran : {}", err.lines().last().unwrap_or("échec").trim());
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("capture de l'écran : pas d'image en 10 s (écran éteint ?)");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

// --- Plein écran --------------------------------------------------------------

/// Plein écran mais pas un jeu (programme ou classe de fenêtre, en minuscules).
const NOT_GAMES: &[&str] = &[
    "firefox", "firefox-esr", "chromium", "chromium-browser", "chrome", "google-chrome", "brave", "brave-browser", "vivaldi",
    "vivaldi-bin", "opera", "microsoft-edge", "epiphany", "vlc", "mpv", "totem", "celluloid", "haruna", "dragon", "smplayer",
    "kodi", "kodi.bin", "plex", "spotify", "soffice.bin", "libreoffice", "zoom", "teams", "teams-for-linux", "obs", "eog",
    "loupe", "gwenview", "okular", "evince", "papers", "nautilus", "dolphin", "kybernow-link",
];

/// La fenêtre de `xprop -root _NET_ACTIVE_WINDOW` (`None` : aucune fenêtre
/// X11 n'a la main, ou une fenêtre Wayland l'a).
pub fn parse_active_window(text: &str) -> Option<String> {
    let id = text.split('#').nth(1)?.split([',', ' ', '\n']).find(|w| !w.is_empty())?.trim().to_string();
    let n = u64::from_str_radix(id.trim_start_matches("0x"), 16).ok()?;
    (n != 0).then_some(id)
}

/// (plein écran, pid, classe) de `xprop -id … _NET_WM_STATE _NET_WM_PID WM_CLASS`.
pub fn parse_window(text: &str) -> (bool, Option<u32>, Option<String>) {
    let mut fullscreen = false;
    let mut pid = None;
    let mut class = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        if key.starts_with("_NET_WM_STATE(") {
            fullscreen = value.split(',').any(|a| a.trim() == "_NET_WM_STATE_FULLSCREEN");
        } else if key.starts_with("_NET_WM_PID(") {
            pid = value.trim().parse().ok();
        } else if key.starts_with("WM_CLASS(") {
            // « "instance", "Classe" » : la classe (la seconde).
            class = value.split(',').next_back().map(|c| c.trim().trim_matches('"').to_string()).filter(|c| !c.is_empty());
        }
    }
    (fullscreen, pid, class)
}

/// L'état écrit par l'agent de veille : `exe` est le programme de la fenêtre.
pub fn classify(fullscreen: bool, exe: Option<&str>, class: Option<&str>) -> Value {
    let exe = exe.map(str::to_lowercase);
    let class = class.map(str::to_lowercase);
    let steam_game = class.as_deref().is_some_and(|c| c.starts_with("steam_app_"));
    let listed = |n: &Option<String>| n.as_deref().is_some_and(|n| NOT_GAMES.contains(&n));
    let game = fullscreen && (steam_game || !(listed(&exe) || listed(&class)));
    json!({ "fullscreen": fullscreen, "game": game, "exe": exe.or(class), "d3d": false })
}

#[cfg(target_os = "linux")]
pub fn probe() -> Value {
    let xprop = |args: &[&str]| {
        let mut cmd = std::process::Command::new("xprop");
        cmd.args(args).env("LC_ALL", "C");
        if std::env::var_os("DISPLAY").is_none_or(|d| d.is_empty()) {
            cmd.env("DISPLAY", ":0");
        }
        cmd.output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let Some(window) = xprop(&["-root", "_NET_ACTIVE_WINDOW"]).as_deref().and_then(parse_active_window) else {
        return classify(false, None, None);
    };
    let (fullscreen, pid, class) = xprop(&["-id", &window, "_NET_WM_STATE", "_NET_WM_PID", "WM_CLASS"]).as_deref().map(parse_window).unwrap_or((false, None, None));
    let exe = pid.and_then(|p| std::fs::read_to_string(format!("/proc/{p}/comm")).ok()).map(|c| c.trim().to_string());
    classify(fullscreen, exe.as_deref(), class.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREENS: &str = "# Écrans de KyberNow Station (kybernow-display) : ne pas modifier.\n\n[[video_sources]]\nid = 2\nname = \"DP-2\"\nsource = \"pipewiresrc target-object=88 do-timestamp=true keepalive-time=1000 resend-last=true\"\nwidth = 1920\nheight = 1080\nframerate = \"0/1\"\nformat = \"BGRx\"\nx = 2560\ny = 0\n\n[[video_sources]]\nid = 1\nname = \"DP-1\"\nsource = \"pipewiresrc target-object=87 do-timestamp=true keepalive-time=1000 resend-last=true\"\nwidth = 2560\nheight = 1440\nframerate = \"0/1\"\nformat = \"BGRx\"\nx = 0\ny = 0\n";

    #[test]
    fn one_frame_of_the_main_screen() {
        let sources = parse_sources(SCREENS);
        assert_eq!(sources.len(), 2);
        let main = primary(&sources).unwrap();
        assert_eq!((main.id, main.width, main.height), (1, 2560, 1440));
        let args = pipeline(main, 960, 70, Path::new("/tmp/apercu.jpg")).unwrap();
        assert_eq!(
            args.join(" "),
            "-q pipewiresrc target-object=87 do-timestamp=true keepalive-time=1000 resend-last=true num-buffers=1 ! videoconvert ! videoscale ! video/x-raw,width=960,height=540,pixel-aspect-ratio=1/1 ! jpegenc quality=70 ! filesink location=/tmp/apercu.jpg"
        );
        // Jamais agrandi.
        let small = Source { id: 1, source: "pipewiresrc target-object=3".into(), width: 800, height: 600, x: 0, y: 0 };
        assert!(pipeline(&small, 1600, 70, Path::new("/tmp/a.jpg")).unwrap().iter().any(|a| a.contains("width=800,height=600")));
        // Rien d'autre qu'un pipewiresrc aux propriétés simples.
        let evil = Source { source: "pipewiresrc target-object=3 ! filesink location=/etc/x".into(), ..small.clone() };
        assert!(pipeline(&evil, 960, 70, Path::new("/tmp/a.jpg")).is_none());
        let other = Source { source: "ximagesrc".into(), ..small };
        assert!(pipeline(&other, 960, 70, Path::new("/tmp/a.jpg")).is_none());
        assert!(parse_sources("pas du toml [").is_empty());
        assert!(primary(&[]).is_none());
    }

    #[test]
    fn full_screen_x11_window() {
        assert_eq!(parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x2a00007\n").as_deref(), Some("0x2a00007"));
        assert_eq!(parse_active_window("_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0\n"), None);
        assert_eq!(parse_active_window("_NET_ACTIVE_WINDOW:  not found.\n"), None);
        let props = "_NET_WM_STATE(ATOM) = _NET_WM_STATE_FULLSCREEN, _NET_WM_STATE_FOCUSED\n_NET_WM_PID(CARDINAL) = 4242\nWM_CLASS(STRING) = \"steam_app_1086940\", \"steam_app_1086940\"\n";
        let (fs, pid, class) = parse_window(props);
        assert_eq!((fs, pid, class.as_deref()), (true, Some(4242), Some("steam_app_1086940")));
        let v = classify(fs, Some("bg3.exe"), class.as_deref());
        assert_eq!((v["fullscreen"].as_bool(), v["game"].as_bool(), v["exe"].as_str()), (Some(true), Some(true), Some("bg3.exe")));
        // Vidéo en plein écran dans Firefox : pas un jeu.
        let (fs, _, class) = parse_window("_NET_WM_STATE(ATOM) = _NET_WM_STATE_FULLSCREEN\nWM_CLASS(STRING) = \"Navigator\", \"firefox\"\n");
        assert_eq!(classify(fs, None, class.as_deref())["game"], false);
        // Fenêtre ordinaire.
        let (fs, _, _) = parse_window("_NET_WM_STATE(ATOM) = _NET_WM_STATE_MAXIMIZED_VERT, _NET_WM_STATE_MAXIMIZED_HORZ\n");
        assert_eq!(classify(fs, Some("game"), None)["fullscreen"], false);
        assert_eq!(classify(false, None, None)["exe"], Value::Null);
    }
}
