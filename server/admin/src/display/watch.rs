//! Veille de l'écran virtuel pendant une session.
//!
//! Le service vit dans la session 0, qui ne voit pas les écrans : un agent
//! (`kyber-admin display-agent watch`) tourne dans la session de l'écran le
//! temps de la session et écrit chaque seconde dans `display-watch/state.json`
//! le mode courant de l'écran virtuel (`EnumDisplaySettingsW`) et les écrans
//! actifs (`QueryDisplayConfig`). Il s'arrête de lui-même quand
//! `display-watch/run` disparaît ou n'est plus rafraîchi (la console le touche
//! à chaque lecture), et au bout de 24 h quoi qu'il arrive.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};

use super::{Created, Mode, Observed, agent, topology};

/// Sans nouvelles de la console depuis ce délai, l'agent s'arrête.
const STALE: Duration = Duration::from_secs(15);
/// Un état plus ancien n'est plus cru.
const FRESH: Duration = Duration::from_secs(3);
/// Délai avant de relancer un agent muet.
const RELAUNCH: Duration = Duration::from_secs(10);

fn age(path: &Path) -> Option<Duration> {
    path.metadata().and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok())
}

pub struct Watch {
    dir: PathBuf,
    /// Écran surveillé et dernier lancement de l'agent.
    target: Mutex<Option<(Created, Instant)>>,
}

impl Watch {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("display-watch"), target: Mutex::new(None) }
    }

    fn launch(&self, data_dir: &Path, c: Created) {
        let _ = std::fs::create_dir_all(&self.dir);
        let _ = std::fs::write(self.dir.join("run"), b"");
        let args = [
            "watch".to_string(),
            "--dir".into(), self.dir.display().to_string(),
            "--luid-low".into(), c.luid_low.to_string(),
            "--luid-high".into(), c.luid_high.to_string(),
            "--target".into(), c.target.to_string(),
        ];
        if let Err(e) = agent::launch(data_dir, &args) {
            tracing::warn!("veille de l'écran virtuel : {e:#}");
        }
    }

    /// Début de session : l'agent part sur l'écran créé.
    pub fn start(&self, data_dir: &Path, c: Created) {
        let _ = std::fs::remove_file(self.dir.join("state.json"));
        self.launch(data_dir, c);
        *self.target.lock().unwrap() = Some((c, Instant::now()));
    }

    /// Fin de session : l'agent s'arrête à son prochain tour.
    pub fn stop(&self) {
        *self.target.lock().unwrap() = None;
        let _ = std::fs::remove_file(self.dir.join("run"));
        let _ = std::fs::remove_file(self.dir.join("state.json"));
    }

    /// Dernier état écrit par l'agent. L'agent est prié de continuer, et
    /// relancé s'il se tait (session Windows changée, agent tué).
    pub fn observe(&self, data_dir: &Path) -> Option<Observed> {
        let mut target = self.target.lock().unwrap();
        let (created, launched) = (*target)?;
        let _ = std::fs::write(self.dir.join("run"), b"");
        let state = self.dir.join("state.json");
        if !age(&state).is_some_and(|a| a < FRESH) {
            if launched.elapsed() > RELAUNCH {
                tracing::info!("veille de l'écran virtuel muette : relance");
                self.launch(data_dir, created);
                *target = Some((created, Instant::now()));
            }
            return None;
        }
        let v: Value = serde_json::from_slice(&std::fs::read(&state).ok()?).ok()?;
        if v.get("error").is_some() {
            return None;
        }
        let n = |k: &str| v[k].as_u64().unwrap_or(0) as u32;
        Some(Observed {
            mode: Mode { width: n("width"), height: n("height"), refresh: n("refresh") },
            gdi_name: v["gdi_name"].as_str().unwrap_or("").to_string(),
            displays: n("displays"),
            others: n("others"),
        })
    }
}

/// Agent : boucle de veille, dans la session de l'écran.
pub fn run_agent(dir: &Path, luid_low: u32, luid_high: i32, target: u32) -> anyhow::Result<()> {
    let run = dir.join("run");
    let state = dir.join("state.json");
    let tmp = dir.join("state.tmp");
    let started = Instant::now();
    loop {
        let alive = age(&run).is_some_and(|a| a < STALE);
        if !alive || started.elapsed() > Duration::from_secs(86_400) {
            let _ = std::fs::remove_file(&state);
            return Ok(());
        }
        let value = match topology::observe(luid_low, luid_high, target) {
            Ok(seen) => serde_json::to_value(seen)?,
            Err(e) => json!({ "error": format!("{e:#}") }),
        };
        std::fs::write(&tmp, serde_json::to_vec(&value)?)?;
        std::fs::rename(&tmp, &state)?;
        std::thread::sleep(Duration::from_secs(1));
    }
}
