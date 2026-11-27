//! L'encodeur de l'ordinateur (docs/profil-auto.md, « L'encodeur de
//! l'ordinateur ») : `GET /api/stream/encoder?codec=hevc&width=3840&height=2160&fps=160&mbps=593&yuv444=0&hdr=0`
//! dit si l'encodeur réglé pour Kyber (`video.encoder`) accepte ce format à
//! ce débit. Le test du profil Auto des apps descend de 50 Mbit/s à chaque
//! refus jusqu'au premier débit accepté.
//!
//! Admin n'a pas FFmpeg : l'outil `kybernow-encoder-check`
//! (server/encoder-check) ouvre l'encodeur de FFmpeg avec les options de
//! Kyber et encode quelques images. Windows : `kybernow-encoder-check.exe`
//! dans le dossier de Kyber (à côté de ses DLL de FFmpeg) ; Mac et Linux :
//! à côté d'Admin. `KYBERNOW_ENCODER_CHECK` en donne un autre (essais).
//! x264 est toujours accepté (pas d'encodeur matériel).
//!
//! Réponse : `{"ok": true}`, `{"ok": false, "error": "…"}` (refusé),
//! `{"ok": null, "error": "…"}` (pas pu essayer). Une sonde à la fois (429).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::{ApiError, ApiResult, stream_user};
use crate::i18n::Msg;

/// Délai le plus long d'un essai (ouverture de l'encodeur, quelques images).
const TOOL_TIMEOUT: Duration = Duration::from_secs(10);

static BUSY: AtomicBool = AtomicBool::new(false);

/// Libère [`BUSY`] à la fin de la sonde, même en erreur.
struct Busy;

impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

#[derive(Deserialize)]
pub struct EncoderQuery {
    codec: String,
    width: u32,
    height: u32,
    fps: u32,
    mbps: u32,
    #[serde(default)]
    yuv444: u8,
    #[serde(default)]
    hdr: u8,
}

/// Ce que l'outil reçoit (validé).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub encoder: String,
    pub codec: &'static str,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub mbps: u32,
    pub yuv444: bool,
    pub hdr: bool,
}

/// La réponse d'un essai.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Refused(String),
    Unknown(String),
}

impl Outcome {
    pub fn json(&self) -> Value {
        match self {
            Self::Accepted => json!({ "ok": true }),
            Self::Refused(e) => json!({ "ok": false, "error": e }),
            Self::Unknown(e) => json!({ "ok": null, "error": e }),
        }
    }
}

impl Probe {
    /// La demande des apps, bornée : formats connus, 64 à 16384 pixels,
    /// 1 à 500 Hz, 1 à 4000 Mbit/s.
    pub fn new(encoder: &str, q: &EncoderQuery) -> Option<Self> {
        let codec = match q.codec.as_str() {
            "hevc" => "hevc",
            "h264" => "h264",
            "av1" => "av1",
            _ => return None,
        };
        let ok = (64..=16384).contains(&q.width) && (64..=16384).contains(&q.height) && (1..=500).contains(&q.fps) && (1..=4000).contains(&q.mbps);
        ok.then(|| Self {
            encoder: encoder.to_string(),
            codec,
            width: q.width,
            height: q.height,
            fps: q.fps,
            mbps: q.mbps,
            yuv444: q.yuv444 == 1,
            hdr: q.hdr == 1,
        })
    }

    /// Les arguments de l'outil.
    pub fn args(&self) -> Vec<String> {
        vec![
            "--encoder".into(),
            self.encoder.clone(),
            "--codec".into(),
            self.codec.into(),
            "--width".into(),
            self.width.to_string(),
            "--height".into(),
            self.height.to_string(),
            "--fps".into(),
            self.fps.to_string(),
            "--mbps".into(),
            self.mbps.to_string(),
            "--yuv444".into(),
            u8::from(self.yuv444).to_string(),
            "--hdr".into(),
            u8::from(self.hdr).to_string(),
        ]
    }
}

/// L'encodeur réglé pour Kyber (`video.encoder`), comme `hdr_unavailable`.
fn kyber_encoder(s: &AppState) -> String {
    s.kyber
        .values()
        .ok()
        .and_then(|v| v.into_iter().find(|f| f.id == "video.encoder"))
        .and_then(|f| f.value.as_str().map(str::to_owned))
        .unwrap_or_else(|| default_encoder().into())
}

fn default_encoder() -> &'static str {
    if cfg!(target_os = "macos") {
        "videotoolbox"
    } else if cfg!(target_os = "linux") {
        "vaapi"
    } else {
        "nvenc"
    }
}

/// L'outil, s'il est là.
pub fn tool(kyber_dir: &Path) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("KYBERNOW_ENCODER_CHECK").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    let p = if cfg!(windows) {
        kyber_dir.join("kybernow-encoder-check.exe")
    } else {
        std::env::current_exe().ok()?.parent()?.join("kybernow-encoder-check")
    };
    p.is_file().then_some(p)
}

/// `encoder_check` de `info` : la Station sait essayer son encodeur.
pub fn available(kyber_dir: &Path) -> bool {
    tool(kyber_dir).is_some()
}

/// Un essai (bloquant, [`TOOL_TIMEOUT`] au plus).
pub fn check(kyber_dir: &Path, probe: &Probe) -> Outcome {
    if probe.encoder == "x264" {
        return Outcome::Accepted;
    }
    let Some(tool) = tool(kyber_dir) else {
        return Outcome::Unknown("kybernow-encoder-check introuvable".into());
    };
    let mut cmd = Command::new(&tool);
    cmd.args(probe.args());
    if cfg!(windows) {
        // Les DLL de FFmpeg sont dans le dossier de Kyber.
        cmd.current_dir(kyber_dir);
    }
    match crate::proc::run(cmd, TOOL_TIMEOUT) {
        Ok(out) => outcome(out.code(), out.stderr.trim()),
        Err(crate::proc::Failure::Spawn(e)) => Outcome::Unknown(format!("{} ne se lance pas ({e})", tool.display())),
        // Ce que l'outil a dit avant d'être arrêté (pilote bloqué) aide à comprendre.
        Err(crate::proc::Failure::Timeout { stderr }) => Outcome::Unknown(match stderr.lines().last().map(str::trim) {
            Some(last) if !last.is_empty() => format!("délai dépassé ({})", last.chars().take(200).collect::<String>()),
            _ => "délai dépassé".into(),
        }),
    }
}

/// Code de sortie de l'outil : 0 accepté, 1 refusé, autre (2 : encodeur
/// absent, arguments) : pas pu essayer.
pub fn outcome(code: Option<i32>, said: &str) -> Outcome {
    let said = said.lines().last().unwrap_or("").chars().take(300).collect::<String>();
    match code {
        Some(0) => Outcome::Accepted,
        Some(1) => Outcome::Refused(said),
        Some(c) => Outcome::Unknown(if said.is_empty() { format!("code {c}") } else { said }),
        None => Outcome::Unknown("arrêté".into()),
    }
}

/// `GET /api/stream/encoder` (droit Session, comme `/api/stream/probe`).
pub async fn stream_encoder(State(s): State<AppState>, headers: HeaderMap, Query(q): Query<EncoderQuery>) -> ApiResult<Json<Value>> {
    stream_user(&s, &headers)?;
    let encoder = kyber_encoder(&s);
    let Some(probe) = Probe::new(&encoder, &q) else {
        return Err(ApiError::msg(StatusCode::BAD_REQUEST, Msg::new("api.encoderQuery")));
    };
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err(ApiError::msg(StatusCode::TOO_MANY_REQUESTS, Msg::new("api.probeBusy")));
    }
    let busy = Busy;
    let dir = s.kyber_dir.clone();
    let p = probe.clone();
    let out = tokio::task::spawn_blocking(move || check(&dir, &p)).await.unwrap_or_else(|e| Outcome::Unknown(e.to_string()));
    drop(busy);
    match &out {
        Outcome::Accepted => tracing::info!("encodeur : {} {} {}×{} {} Hz à {} Mbit/s accepté", probe.encoder, probe.codec, probe.width, probe.height, probe.fps, probe.mbps),
        Outcome::Refused(e) => tracing::info!("encodeur : {} {} {}×{} {} Hz à {} Mbit/s refusé ({e})", probe.encoder, probe.codec, probe.width, probe.height, probe.fps, probe.mbps),
        Outcome::Unknown(e) => tracing::warn!("encodeur : pas pu essayer {} ({e})", probe.encoder),
    }
    Ok(Json(out.json()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(codec: &str, mbps: u32) -> EncoderQuery {
        EncoderQuery { codec: codec.into(), width: 3840, height: 2160, fps: 160, mbps, yuv444: 0, hdr: 0 }
    }

    #[test]
    fn demande_bornee_et_arguments() {
        let p = Probe::new("nvenc", &q("hevc", 593)).unwrap();
        assert_eq!(
            p.args().join(" "),
            "--encoder nvenc --codec hevc --width 3840 --height 2160 --fps 160 --mbps 593 --yuv444 0 --hdr 0"
        );
        assert!(Probe::new("nvenc", &q("pyrowave", 593)).is_none(), "PyroWave : pas cet outil");
        assert!(Probe::new("nvenc", &q("hevc", 0)).is_none());
        assert!(Probe::new("nvenc", &q("hevc", 9000)).is_none());
    }

    #[test]
    fn codes_de_l_outil() {
        assert_eq!(outcome(Some(0), ""), Outcome::Accepted);
        assert_eq!(outcome(Some(1), "ouverture\nInitializeEncoder failed: invalid param (8)"), Outcome::Refused("InitializeEncoder failed: invalid param (8)".into()));
        assert_eq!(outcome(Some(2), ""), Outcome::Unknown("code 2".into()));
        assert_eq!(outcome(None, ""), Outcome::Unknown("arrêté".into()));
        assert_eq!(Outcome::Refused("x".into()).json(), json!({ "ok": false, "error": "x" }));
        assert_eq!(Outcome::Unknown("x".into()).json(), json!({ "ok": null, "error": "x" }));
    }

    #[test]
    fn x264_toujours_accepte() {
        let p = Probe::new("x264", &q("h264", 900)).unwrap();
        assert_eq!(check(Path::new("/nulle-part"), &p), Outcome::Accepted);
    }
}
