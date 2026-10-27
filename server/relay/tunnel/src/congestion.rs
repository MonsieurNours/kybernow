//! Contrôle de congestion « permissif » de `kn-media/1` (docs/relais.md) :
//! fenêtre très grande, pas d'espacement (quinn espace d'après la fenêtre),
//! pertes ignorées. Kyber a déjà son contrôle de débit à l'intérieur ; le
//! tunnel ne doit pas en ajouter un second qui freinerait l'image.

use std::any::Any;
use std::sync::Arc;
use std::time::Instant;

use quinn::congestion::{Controller, ControllerFactory};

/// Fenêtre du contrôle permissif : 32 Mio en vol (270 ms à 1 Gbit/s).
pub const PERMISSIVE_WINDOW: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Permissive;

impl Controller for Permissive {
    fn on_congestion_event(&mut self, _now: Instant, _sent: Instant, _persistent: bool, _lost: u64) {}

    fn on_mtu_update(&mut self, _new_mtu: u16) {}

    fn window(&self) -> u64 {
        PERMISSIVE_WINDOW
    }

    fn clone_box(&self) -> Box<dyn Controller> {
        Box::new(self.clone())
    }

    fn initial_window(&self) -> u64 {
        PERMISSIVE_WINDOW
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

/// Fabrique du contrôle permissif.
#[derive(Debug, Clone, Default)]
pub struct PermissiveFactory;

impl ControllerFactory for PermissiveFactory {
    fn build(self: Arc<Self>, _now: Instant, _mtu: u16) -> Box<dyn Controller> {
        Box::new(Permissive)
    }
}

#[cfg(feature = "server")]
pub use switchable::{CcMode, SwitchableFactory};

#[cfg(feature = "server")]
mod switchable {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    use quinn::congestion::CubicConfig;
    use quinn_proto::RttEstimator;

    /// Côté relais, le contrôle se choisit avant de connaître l'ALPN : Cubic
    /// au départ, permissif dès que le relais sait que la connexion est
    /// `kn-media/1` ([`CcMode::set_permissive`]).
    #[derive(Debug, Clone, Default)]
    pub struct CcMode(Arc<AtomicBool>);

    impl CcMode {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn set_permissive(&self) {
            self.0.store(true, Ordering::Relaxed);
        }

        pub fn is_permissive(&self) -> bool {
            self.0.load(Ordering::Relaxed)
        }
    }

    pub struct Switchable {
        cubic: Box<dyn Controller>,
        mode: CcMode,
    }

    impl Controller for Switchable {
        fn on_sent(&mut self, now: Instant, bytes: u64, last_packet_number: u64) {
            self.cubic.on_sent(now, bytes, last_packet_number)
        }

        fn on_ack(&mut self, now: Instant, sent: Instant, bytes: u64, app_limited: bool, rtt: &RttEstimator) {
            self.cubic.on_ack(now, sent, bytes, app_limited, rtt)
        }

        fn on_end_acks(&mut self, now: Instant, in_flight: u64, app_limited: bool, largest: Option<u64>) {
            self.cubic.on_end_acks(now, in_flight, app_limited, largest)
        }

        fn on_congestion_event(&mut self, now: Instant, sent: Instant, persistent: bool, lost: u64) {
            self.cubic.on_congestion_event(now, sent, persistent, lost)
        }

        fn on_mtu_update(&mut self, new_mtu: u16) {
            self.cubic.on_mtu_update(new_mtu)
        }

        fn window(&self) -> u64 {
            if self.mode.is_permissive() { PERMISSIVE_WINDOW } else { self.cubic.window() }
        }

        fn clone_box(&self) -> Box<dyn Controller> {
            Box::new(Switchable { cubic: self.cubic.clone_box(), mode: self.mode.clone() })
        }

        fn initial_window(&self) -> u64 {
            self.cubic.initial_window()
        }

        fn into_any(self: Box<Self>) -> Box<dyn Any> {
            self
        }
    }

    /// Une fabrique par connexion entrante, liée à son [`CcMode`].
    pub struct SwitchableFactory {
        pub mode: CcMode,
    }

    impl ControllerFactory for SwitchableFactory {
        fn build(self: Arc<Self>, now: Instant, mtu: u16) -> Box<dyn Controller> {
            let cubic = Arc::new(CubicConfig::default()).build(now, mtu);
            Box::new(Switchable { cubic, mode: self.mode.clone() })
        }
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn bascule_en_permissif_apres_la_negociation() {
        let mode = CcMode::new();
        let mut cc = Arc::new(SwitchableFactory { mode: mode.clone() }).build(Instant::now(), 1280);
        let cubic = cc.window();
        assert!(cubic < PERMISSIVE_WINDOW);
        cc.on_congestion_event(Instant::now(), Instant::now(), false, 1200);
        assert!(cc.window() <= cubic, "Cubic réagit aux pertes");
        mode.set_permissive();
        assert_eq!(cc.window(), PERMISSIVE_WINDOW);
        let mut p = Arc::new(PermissiveFactory).build(Instant::now(), 1280);
        p.on_congestion_event(Instant::now(), Instant::now(), true, 1_000_000);
        assert_eq!(p.window(), PERMISSIVE_WINDOW, "pertes ignorées");
    }
}
