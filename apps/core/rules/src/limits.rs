//! Valeurs que toutes les apps partagent (TRANS-40 à 42) : débits, maxima,
//! délais. Le cœur les reprend dans ses modules (`share::MAX_GUESTS`,
//! `focus::MAX_SESSIONS`…) ; les ponts les donnent aux interfaces.

// MARK: - Débit

/// L'appareil de l'app, pour ce qui dépend de lui.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    /// Mac, PC sous Windows, Linux (Steam Deck, Raspberry Pi compris).
    Computer,
    /// iPhone, iPad.
    AppleMobile,
    /// Apple TV.
    AppleTv,
    /// Téléphone, tablette ou téléviseur Android.
    Android,
    /// Téléviseur LG : sa prise Ethernet s'arrête à 100 Mbit/s, son
    /// décodeur à 95.
    LgTv,
}

/// Débit demandé à l'ordinateur quand l'image n'est plus regardée (fenêtre
/// cachée ou réduite, app en arrière-plan), en bit/s (TRANS-40, décision
/// de l'auteur du 27/11) : 20 Mbit/s sur Mac et PC, 8 sur iPhone, iPad et
/// Apple TV, 2 sur Android et sur les téléviseurs LG.
pub fn background_bitrate(device: Device) -> u32 {
    match device {
        Device::Computer => 20_000_000,
        Device::AppleMobile | Device::AppleTv => 8_000_000,
        Device::Android | Device::LgTv => 2_000_000,
    }
}

/// PyroWave : débit de base d'un profil, calé sur la 4K à 60 Hz (Mbit/s),
/// en 4:2:0 et en 4:4:4.
pub const PYROWAVE_MBPS: u32 = 800;
pub const PYROWAVE_MBPS_444: u32 = 1200;

/// Débit réglé à la main (« Personnalisé ») : le plus bas, le pas du
/// curseur, le plus haut en HEVC, AV1 et H.264 (aucune carte n'encode
/// au-delà), puis en PyroWave (Mbit/s).
pub const MANUAL_MBPS_MIN: u32 = 5;
pub const MANUAL_MBPS_STEP: u32 = 5;
pub const MANUAL_MBPS_MAX: u32 = 500;
pub const MANUAL_MBPS_MAX_PYROWAVE: u32 = 2500;
/// Le plus haut sur un téléviseur LG, quel que soit le format.
pub const LG_TV_MBPS_MAX: u32 = 95;

/// Les bornes du débit réglé à la main sur cet appareil, dans ce format
/// (TRANS-41), en Mbit/s. Le client web, qui ne lie pas le cœur, garde
/// celles de son navigateur.
pub fn manual_mbps(device: Device, pyrowave: bool) -> (u32, u32) {
    let max = match (device, pyrowave) {
        (Device::LgTv, _) => LG_TV_MBPS_MAX,
        (_, true) => MANUAL_MBPS_MAX_PYROWAVE,
        (_, false) => MANUAL_MBPS_MAX,
    };
    (MANUAL_MBPS_MIN, max)
}

/// Un débit réglé à la main (reçu d'un autre appareil, ancien réglage)
/// ramené dans les bornes de cet appareil et de ce format.
pub fn clamp_manual_mbps(mbps: u32, device: Device, pyrowave: bool) -> u32 {
    let (min, max) = manual_mbps(device, pyrowave);
    mbps.clamp(min, max)
}

// MARK: - Partage, sessions

/// Invités d'un partage KyberNow Share, au plus.
pub const MAX_GUESTS: usize = 4;
/// Sessions ouvertes en même temps sur un appareil (plusieurs ordinateurs).
pub const MAX_SESSIONS: usize = 4;
/// Ce que l'app attend au plus le lien du relais d'un partage ou d'un code
/// de prêt en préparation, à partir du moment où elle le voit (secondes).
pub const RELAY_LINK_WAIT_SECS: u64 = 10;

// MARK: - Mise à jour des Stations

/// L'avis après une mise à jour de l'app attend au plus ce temps les
/// vérifications des ordinateurs en ligne (secondes).
pub const STATION_NOTICE_WAIT_SECS: u64 = 10;
/// Dernière version lancée à supposer quand l'app n'en a pas gardé mais
/// connaît déjà un ordinateur : une version d'avant l'avis.
pub const STATION_NOTICE_BEFORE: &str = "0.11.2";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bornes_du_debit_a_la_main() {
        assert_eq!(manual_mbps(Device::Computer, false), (5, 500));
        assert_eq!(manual_mbps(Device::Android, true), (5, 2500));
        assert_eq!(manual_mbps(Device::LgTv, true), (5, 95));
        assert_eq!(clamp_manual_mbps(4294, Device::AppleMobile, false), 500);
        assert_eq!(clamp_manual_mbps(4294, Device::Computer, true), 2500);
        assert_eq!(clamp_manual_mbps(1, Device::Computer, true), 5);
        assert_eq!(clamp_manual_mbps(120, Device::LgTv, false), 95);
        assert_eq!(MANUAL_MBPS_MAX % MANUAL_MBPS_STEP, 0);
        assert_eq!(MANUAL_MBPS_MAX_PYROWAVE % MANUAL_MBPS_STEP, 0);
    }

    #[test]
    fn debit_en_arriere_plan_par_appareil() {
        assert_eq!(background_bitrate(Device::Computer), 20_000_000);
        assert_eq!(background_bitrate(Device::AppleMobile), 8_000_000);
        assert_eq!(background_bitrate(Device::AppleTv), 8_000_000);
        assert_eq!(background_bitrate(Device::Android), 2_000_000);
        assert_eq!(background_bitrate(Device::LgTv), 2_000_000);
    }
}
