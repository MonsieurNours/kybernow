//! Messages échangés entre l'interface (JavaScript) et l'installateur (Rust).

use serde::{Deserialize, Serialize};

use crate::payload::Arch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Running,
    Ok,
    Warn,
    Error,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    Running,
    Done,
    Skipped,
    /// Faite en partie ou pas faite, sans empêcher la suite (détail dans le journal).
    Warn,
    Failed,
}

/// Ce que l'installateur envoie à l'interface.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Event {
    Check { id: String, status: CheckStatus, title: String, detail: String },
    ChecksDone {
        arch: Arch,
        /// Version de KyberNow Link déjà installée (mise à jour).
        existing: Option<String>,
        /// L'app est ouverte : elle sera fermée au moment d'installer.
        running: bool,
        /// Un raccourci est déjà sur le Bureau : l'option reste cochée.
        desktop: bool,
        dir: String,
    },
    Task { id: String, status: TaskStatus },
    Progress { percent: f32 },
    Log { line: String },
    Done,
    Failed { task: String, message: String },
    /// « Autre relais… » vérifié (`input` : ce qui a été saisi).
    RelayProbed {
        input: String,
        #[serde(flatten)]
        probe: crate::account::RelayProbe,
    },
    /// Connecté : la session d'appareil est remise à l'app.
    AccountSignedIn { relay: String, name: String, email: String },
    /// La connexion n'a pas abouti (`cancelled`, `exchange`, `store`).
    AccountFailed { code: String, message: String },
}

/// Ce que l'interface demande à l'installateur.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    RunChecks,
    Install { desktop: bool },
    /// Ouvre l'app installée, puis ferme l'installateur.
    Launch,
    OpenUrl { url: String },
    RelayProbe { address: String },
    /// La page du relais dans le cadre donné (points CSS) : connexion de cet
    /// appareil, ou création d'un compte (`signup`).
    AccountView { relay: String, signup: bool, x: f64, y: f64, width: f64, height: f64 },
    AccountViewHide,
    Minimize,
    Drag,
    Close,
}

pub type Emit = std::sync::Arc<dyn Fn(Event) + Send + Sync>;
