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
    Minimize,
    Drag,
    Close,
}

pub type Emit = std::sync::Arc<dyn Fn(Event) + Send + Sync>;
