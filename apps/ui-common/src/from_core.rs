//! Ce que le cœur dit, dans les types de l'interface (fonctionnalité `core`,
//! prise par la couche appareil de Linux et de LG ; l'interface seule n'en a
//! pas besoin).

use kybernow_core::share;

use crate::share::{ShareQuality, ShareRelay, ShareRole};

/// Le rôle de l'interface d'après celui du cœur.
pub fn role_of(r: share::ShareRole) -> ShareRole {
    match r {
        share::ShareRole::Spectator => ShareRole::Spectator,
        share::ShareRole::Player => ShareRole::Player,
        share::ShareRole::Kbm => ShareRole::Kbm,
        share::ShareRole::PlayerKbm => ShareRole::PlayerKbm,
    }
}

/// Le rôle du cœur d'après celui de l'interface (les deux boutons de droits de l'hôte).
pub fn core_role(r: ShareRole) -> share::ShareRole {
    match r {
        ShareRole::Spectator => share::ShareRole::Spectator,
        ShareRole::Player => share::ShareRole::Player,
        ShareRole::Kbm => share::ShareRole::Kbm,
        ShareRole::PlayerKbm => share::ShareRole::PlayerKbm,
    }
}

/// La « Qualité de l'image » du cœur d'après celle de l'interface (le choix de « Rejoindre » et de la barre).
pub fn core_quality(q: ShareQuality) -> share::ShareQuality {
    match q {
        ShareQuality::Auto => share::ShareQuality::Auto,
        ShareQuality::Host => share::ShareQuality::Host,
        ShareQuality::P2160_60 => share::ShareQuality::P2160_60,
        ShareQuality::P1080_60 => share::ShareQuality::P1080_60,
        ShareQuality::P720_30 => share::ShareQuality::P720_30,
    }
}

/// Le relais d'un partage ou d'un code de prêt (`relay.state` d'Admin) tel que l'interface le montre.
pub fn relay_of(r: Option<share::ShareRelay>) -> Option<ShareRelay> {
    r.map(|r| match r.state {
        share::ShareRelayState::Joined => ShareRelay::Joined,
        share::ShareRelayState::Connecting => ShareRelay::Connecting,
        share::ShareRelayState::Unreachable => ShareRelay::Unreachable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        for r in [ShareRole::Spectator, ShareRole::Player, ShareRole::Kbm, ShareRole::PlayerKbm] {
            assert_eq!(role_of(core_role(r)), r);
        }
    }
}
