//! La sortie vers le micro virtuel, propre à chaque système. Chacune tire le
//! son du tampon de gigue à son rythme, rend le micro virtuel par défaut à la
//! demande et remet le précédent en s'arrêtant.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod mac;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::{Player, check};
#[cfg(target_os = "macos")]
pub use mac::{Player, check};
#[cfg(windows)]
pub use windows::{Player, check};

/// Réponse de `--check`.
pub enum Check {
    /// Le micro virtuel est là (son nom).
    Ready(String),
    /// Il manque (VB-CABLE pas installé, pilote absent, PipeWire injoignable).
    Missing(String),
    /// Rien ne peut marcher ici.
    #[allow(dead_code)]
    Unsupported(String),
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub fn check() -> Check {
    Check::Unsupported("système non pris en charge".into())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub struct Player;

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
impl Player {
    pub fn start(_: std::sync::Arc<std::sync::Mutex<crate::jitter::Jitter>>) -> Result<Self, String> {
        Err("système non pris en charge".into())
    }
    pub fn name(&self) -> &str {
        ""
    }
    pub fn make_default(&mut self) {}
    pub fn stop(self) {}
}

/// Tire `out.len()` échantillons sans jamais bloquer le fil du son : le
/// tampon occupé (le décodage y écrit), du silence pour cette fois.
#[allow(dead_code)]
pub(crate) fn pull(jitter: &std::sync::Mutex<crate::jitter::Jitter>, out: &mut [f32]) {
    match jitter.try_lock() {
        Ok(mut j) => j.pull(out),
        Err(_) => out.fill(0.0),
    }
}
