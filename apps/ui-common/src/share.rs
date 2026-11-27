//! Ce que l'interface montre du partage KyberNow Share (docs/link-ui.md,
//! « Partage ») : le rôle d'un invité et ses droits, ce que l'hôte voit
//! d'eux, la disponibilité du partage et ses petits textes.

use crate::i18n::{english, tr};

/// Le rôle d'un invité, que la Station impose : miroir de `kybernow_core::share::ShareRole` (l'interface seule,
/// celle des captures et des tests sur Mac, n'a pas le cœur). Deux droits indépendants, les manettes et le clavier
/// et la souris, que l'hôte donne ou reprend chacun sans toucher à l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShareRole {
    /// Aucun droit : voit et entend, n'envoie rien (le rôle d'arrivée).
    #[default]
    Spectator,
    /// Les manettes : elles deviennent la manette suivante de Windows.
    Player,
    /// Le clavier et la souris, sans les manettes.
    Kbm,
    /// Les manettes, et le clavier et la souris.
    PlayerKbm,
}

impl ShareRole {
    /// Le rôle de ces deux droits.
    pub fn of(pads: bool, kbm: bool) -> Self {
        match (pads, kbm) {
            (false, false) => ShareRole::Spectator,
            (true, false) => ShareRole::Player,
            (false, true) => ShareRole::Kbm,
            (true, true) => ShareRole::PlayerKbm,
        }
    }

    /// Les manettes de l'invité partent vers l'ordinateur.
    pub fn pads(self) -> bool {
        matches!(self, ShareRole::Player | ShareRole::PlayerKbm)
    }

    /// Le clavier et la souris de l'invité partent vers l'ordinateur.
    pub fn kbm(self) -> bool {
        matches!(self, ShareRole::Kbm | ShareRole::PlayerKbm)
    }

    /// Un droit au moins : l'invité a une chaîne d'entrées, et la barre le dit « Joueur ».
    pub fn plays(self) -> bool {
        self.pads() || self.kbm()
    }

    /// Le rôle quand l'hôte donne (`true`) ou reprend (`false`) les manettes, le clavier et la souris tels quels.
    pub fn with_pads(self, on: bool) -> Self {
        Self::of(on, self.kbm())
    }

    /// Le rôle quand l'hôte donne ou reprend le clavier et la souris, les manettes telles quelles.
    pub fn with_kbm(self, on: bool) -> Self {
        Self::of(self.pads(), on)
    }

    /// « Spectateur », « Joueur » : la barre réduite de l'invité.
    pub fn label(self) -> &'static str {
        if self.plays() { tr("Joueur", "Player") } else { tr("Spectateur", "Spectator") }
    }

    /// « Spectateur », « Joueur », « Clavier et souris », « Joueur, clavier et souris » : la liste de l'hôte.
    pub fn long_label(self) -> &'static str {
        match self {
            ShareRole::Spectator => tr("Spectateur", "Spectator"),
            ShareRole::Player => tr("Joueur", "Player"),
            ShareRole::Kbm => tr("Clavier et souris", "Keyboard and mouse"),
            ShareRole::PlayerKbm => tr("Joueur, clavier et souris", "Player, keyboard and mouse"),
        }
    }

    /// L'avis bref d'un rôle changé par l'hôte (docs/link-ui.md). Les deux droits changés d'un coup : un seul
    /// avis, le droit donné avant le droit repris, les manettes avant le clavier et la souris. Plus aucun droit :
    /// « Vous regardez de nouveau. » à la place de l'avis de reprise. Rien au départ (`previous` nul) ni sans
    /// changement.
    pub fn notice(self, previous: Option<ShareRole>) -> Option<&'static str> {
        let before = previous?;
        if self.pads() && !before.pads() {
            Some(tr("Vous pouvez jouer à la manette.", "You can play with a controller."))
        } else if self.kbm() && !before.kbm() {
            Some(tr("Vous avez le clavier et la souris.", "You have the keyboard and mouse."))
        } else if !self.plays() && before.plays() {
            Some(tr("Vous regardez de nouveau.", "You are watching again."))
        } else if !self.pads() && before.pads() {
            Some(tr("Les manettes sont reprises.", "The controllers were taken back."))
        } else if !self.kbm() && before.kbm() {
            Some(tr("Le clavier et la souris sont repris.", "The keyboard and mouse were taken back."))
        } else {
            None
        }
    }
}

impl ShareQuality {
    /// Dans l'ordre du choix.
    pub const ALL: [ShareQuality; 5] = [Self::Auto, Self::Host, Self::P2160_60, Self::P1080_60, Self::P720_30];

    /// Le nom retenu dans `kybernow.json` : celui de l'échange avec Admin (`auto`, `host`, `2160p60`, `1080p60`, `720p30`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Host => "host",
            Self::P2160_60 => "2160p60",
            Self::P1080_60 => "1080p60",
            Self::P720_30 => "720p30",
        }
    }

    /// Un nom inconnu (fichier plus récent ou écrit à la main) se lit « Auto ».
    pub fn parse(s: &str) -> Self {
        Self::ALL.into_iter().find(|q| q.as_str() == s.trim()).unwrap_or_default()
    }

    /// « Auto », « Comme l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz » (docs/link-ui.md, « Qualité de l'image »).
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => tr("Auto", "Auto"),
            Self::Host => tr("Comme l'hôte", "Same as host"),
            Self::P2160_60 => "4K · 60 Hz",
            Self::P1080_60 => "1080p · 60 Hz",
            Self::P720_30 => "720p · 30 Hz",
        }
    }
}

/// Ce que « Partager » peut faire avec un ordinateur (`Info::share`) : miroir de
/// `kybernow_core::share::ShareAvailability`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareAvail {
    /// Une Station antérieure à la 0.8.0.
    Unsupported,
    /// Le partage est coupé sur l'ordinateur (`multi_client` de Kyber).
    Disabled,
    Available,
}

impl ShareAvail {
    /// Le texte de « Partager » inactif ; rien quand le partage est disponible.
    pub fn text(self) -> Option<&'static str> {
        match self {
            ShareAvail::Unsupported => Some(tr("Le partage demande KyberNow Station 0.8.0 ou plus récente.", "Sharing needs KyberNow Station 0.8.0 or later.")),
            ShareAvail::Disabled => Some(tr("Le partage est coupé sur cet ordinateur (Admin › Appareils Link).", "Sharing is turned off on this computer (Admin › Link devices).")),
            ShareAvail::Available => None,
        }
    }
}

/// Un invité du partage de l'hôte (`ShareGuest` du cœur).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ShareGuest {
    pub id: String,
    pub name: String,
    pub role: ShareRole,
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`, `web`.
    pub app: String,
    /// Son arrivée (secondes depuis 1970), telle que la Station la donne.
    pub since: Option<i64>,
    /// La qualité de l'image qu'il reçoit (« 720p · 30 Hz »), seulement quand c'est une image à part, pas celle
    /// de l'hôte, et que Kyber en sait la taille : sinon `None` (une image encore inconnue compte comme celle de
    /// l'hôte). L'hôte la voit sans la changer.
    pub quality: Option<String>,
    /// Venu par le relais, une demande pas encore acceptée (« Demandes ») : hors des quatre places.
    pub pending: bool,
    /// Venu par le relais (`via` `relay`) : « hors de chez vous » après son app.
    pub via_relay: bool,
}

/// L'app d'un invité (champ `app`) : Mac, Windows, Linux, « iPhone ou iPad », Apple TV, Android,
/// « Téléviseur LG », « Navigateur » ; une app inconnue (Station plus récente) se dit telle quelle.
pub fn app_label(app: &str) -> String {
    match app {
        "mac" => "Mac".into(),
        "windows" => "Windows".into(),
        "linux" => "Linux".into(),
        "ios" => tr("iPhone ou iPad", "iPhone or iPad").into(),
        "tvos" => "Apple TV".into(),
        "android" => "Android".into(),
        "webos" => tr("Téléviseur LG", "LG TV").into(),
        "web" => tr("Navigateur", "Browser").into(),
        other => other.into(),
    }
}

/// « moins d'une minute », « 3 min », « 1 h 12 » (docs/link-ui.md) : la durée d'un invité.
pub fn duration_label(seconds: u64) -> String {
    let m = seconds / 60;
    if m < 1 {
        tr("moins d'une minute", "under a minute").into()
    } else if m < 60 {
        format!("{m} min")
    } else {
        format!("{} h {:02}", m / 60, m % 60)
    }
}

/// « 2 invités », « 1 invité », « 0 invité » ; “2 guests”, “1 guest” : la pastille de la barre.
pub fn guests_label(n: usize) -> String {
    if crate::i18n::english() {
        format!("{n} {}", if n == 1 { "guest" } else { "guests" })
    } else {
        format!("{n} {}", if n >= 2 { "invités" } else { "invité" })
    }
}

/// « Invité sur Atelier », “Guest on Atelier” : le titre d'une session d'invité.
pub fn guest_title(computer: &str) -> String {
    format!("{} {computer}", tr("Invité sur", "Guest on"))
}

/// Un des deux boutons de la ligne d'un invité (hors « Retirer ») : un droit, donné ou repris sans toucher à
/// l'autre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareMove {
    /// « Donner » ou « Reprendre les manettes » : `SharePads(i)`.
    Pads,
    /// « Donner » ou « Reprendre le clavier et la souris » : `ShareKbm(i)`.
    Kbm,
}

impl ShareMove {
    /// Les deux boutons, dans l'ordre de la ligne : les manettes, puis le clavier et la souris.
    pub const ALL: [ShareMove; 2] = [ShareMove::Pads, ShareMove::Kbm];

    /// Ce que dit le bouton pour un invité de ce rôle : donner le droit qu'il n'a pas, reprendre celui qu'il a.
    pub fn label(self, role: ShareRole) -> &'static str {
        match (self, self.held(role)) {
            (ShareMove::Pads, false) => tr("Donner les manettes", "Give controllers"),
            (ShareMove::Pads, true) => tr("Reprendre les manettes", "Take back controllers"),
            (ShareMove::Kbm, false) => tr("Donner le clavier et la souris", "Give keyboard and mouse"),
            (ShareMove::Kbm, true) => tr("Reprendre le clavier et la souris", "Take back keyboard and mouse"),
        }
    }

    /// L'invité a ce droit.
    pub fn held(self, role: ShareRole) -> bool {
        match self {
            ShareMove::Pads => role.pads(),
            ShareMove::Kbm => role.kbm(),
        }
    }

    /// Le rôle que demande le bouton : ce droit basculé, l'autre tel quel.
    pub fn toggled(self, role: ShareRole) -> ShareRole {
        match self {
            ShareMove::Pads => role.with_pads(!role.pads()),
            ShareMove::Kbm => role.with_kbm(!role.kbm()),
        }
    }
}

/// « Arrêter de regarder » / “Stop watching”, à la place de « Quitter le partage » (`watch_word(lang, "stop")`).
pub fn watch_stop_word() -> &'static str {
    tr("Arrêter de regarder", "Stop watching")
}

/// « Regarder {nom} » / “Watching {name}”, à la place de « Invité sur {nom} » (`share::watch_title`).
pub fn watch_title(computer: &str) -> String {
    if english() { format!("Watching {computer}") } else { format!("Regarder {computer}") }
}

/// La « Qualité de l'image » qu'un invité demande : miroir de `kybernow_core::share::ShareQuality` (l'interface
/// seule, celle des captures et des tests sur Mac, n'a pas le cœur). L'image reçue n'est jamais au-dessus de
/// celle de l'hôte ; la Station décide et le dit (`ShareGuest::quality`, `GuestUi::image`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShareQuality {
    /// D'après l'écran de l'appareil (la sortie du téléviseur) et ce que son décodeur suit : proposé.
    #[default]
    Auto,
    /// « Comme l'hôte ».
    Host,
    /// « 4K · 60 Hz ».
    P2160_60,
    /// « 1080p · 60 Hz ».
    P1080_60,
    /// « 720p · 30 Hz ».
    P720_30,
}

/// « 1 demande », « 2 demandes » ; “1 request”, “2 requests” : la pastille des demandes en attente.
pub fn requests_label(n: usize) -> String {
    if crate::i18n::english() {
        format!("{n} {}", if n == 1 { "request" } else { "requests" })
    } else {
        format!("{n} {}", if n >= 2 { "demandes" } else { "demande" })
    }
}

/// « Regarder » / “Watch” : la carte (droit « Regarder » seul) et le menu de l'ordinateur. Miroir de
/// `share::watch_word(lang, "action")` du cœur (l'interface seule ne l'a pas).
pub fn watch_word() -> &'static str {
    tr("Regarder", "Watch")
}

/// « Écran » / “Screen” : le menu des écrans (`watch_word(lang, "screen")`).
pub fn watch_screen_word() -> &'static str {
    tr("Écran", "Screen")
}

/// L'état du relais d'un partage ouvert (`relay.state` d'Admin).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareRelay {
    /// Le code est réservé chez le relais.
    Joined,
    /// Connexion au relais, ou code pas encore réservé : « Préparation du lien hors de chez vous… », `link` et `web`
    /// sont encore ceux du réseau (le lien et le QR code attendent, `RELAY_LINK_WAIT_SECS` au plus).
    Connecting,
    /// Le relais ne répond pas : « Pas encore joignable hors de chez vous. »
    Unreachable,
}

impl ShareGuest {
    /// « Léa · Spectateur », « Léa · Joueur », « Léa · Joueur, clavier et souris ».
    pub fn line(&self) -> String {
        format!("{} · {}", self.name, self.role.long_label())
    }

    /// « {prénom} demande à rejoindre », dessous « {app} · hors de chez vous » : la ligne d'une demande.
    pub fn request_line(&self) -> String {
        format!("{} {}", self.name, tr("demande à rejoindre", "wants to join"))
    }

    /// « {prénom} demande à rejoindre. » : l'avis bref d'une demande qui arrive pendant la session.
    pub fn request_notice(&self) -> String {
        format!("{}.", self.request_line())
    }

    /// « Android · hors de chez vous » : sous la ligne d'une demande.
    pub fn request_meta(&self) -> String {
        format!("{} · {}", app_label(&self.app), tr("hors de chez vous", "away from home"))
    }

    /// « iPhone ou iPad · 720p · 30 Hz · depuis 4 min » : son app, sa qualité d'image si elle n'est pas celle de
    /// l'hôte, et depuis quand il est là (`now` : secondes depuis 1970). `since` est en secondes Unix, d'après
    /// l'horloge de la Station : la durée n'est jamais négative (horloges décalées). Sans arrivée lisible, sans
    /// la durée.
    pub fn meta(&self, now: i64) -> String {
        let mut parts = vec![app_label(&self.app)];
        if self.via_relay {
            parts.push(tr("hors de chez vous", "away from home").into());
        }
        parts.extend(self.quality.clone());
        if let Some(t) = self.since.filter(|t| *t > 0) {
            parts.push(format!("{} {}", tr("depuis", "for"), duration_label(now.saturating_sub(t).max(0) as u64)));
        }
        parts.join(" · ")
    }
}

/// Ce que l'app attend au plus le lien du relais d'un partage ou d'un code de prêt en préparation
/// (`rules::limits::RELAY_LINK_WAIT_SECS`).
pub const RELAY_LINK_WAIT: std::time::Duration = std::time::Duration::from_secs(kybernow_rules::limits::RELAY_LINK_WAIT_SECS);

/// L'attente du lien du relais, commune au partage (`ShareView`) et au code de prêt (`LendCodeUi`) : « Préparation du
/// lien hors de chez vous… », « Copier le lien » et le QR code retenus `RELAY_LINK_WAIT` au plus, puis le lien du
/// réseau et « Pas encore joignable hors de chez vous. » (docs/link-ui.md, « Hors de chez vous » et « Prêt »).
pub mod relay_wait {
    use super::{tr, ShareRelay, RELAY_LINK_WAIT};
    use std::time::{Duration, Instant};

    /// Le lien du relais se prépare et l'app l'attend encore (`since` : quand elle l'a vu en préparation).
    pub fn preparing(relay: Option<ShareRelay>, since: Option<Instant>) -> bool {
        relay == Some(ShareRelay::Connecting) && since.is_none_or(|t| t.elapsed() < RELAY_LINK_WAIT)
    }

    /// L'attente court encore, ou vient de finir : l'interface se redessine jusqu'à un peu après `RELAY_LINK_WAIT`.
    pub fn redraw(relay: Option<ShareRelay>, since: Option<Instant>) -> bool {
        relay == Some(ShareRelay::Connecting) && since.is_none_or(|t| t.elapsed() < RELAY_LINK_WAIT + Duration::from_millis(250))
    }

    /// « Préparation du lien hors de chez vous… », pendant l'attente.
    pub fn wait_text(relay: Option<ShareRelay>, since: Option<Instant>) -> Option<&'static str> {
        preparing(relay, since).then(|| tr("Préparation du lien hors de chez vous…", "Getting the away-from-home link ready…"))
    }

    /// « Pas encore joignable hors de chez vous. » : le relais est réglé mais injoignable, ou son lien s'est fait
    /// attendre plus de `RELAY_LINK_WAIT`.
    pub fn down_text(relay: Option<ShareRelay>, since: Option<Instant>) -> Option<&'static str> {
        let down = match relay {
            Some(ShareRelay::Unreachable) => true,
            Some(ShareRelay::Connecting) => !preparing(relay, since),
            Some(ShareRelay::Joined) | None => false,
        };
        down.then(|| tr("Pas encore joignable hors de chez vous.", "Not reachable away from home yet."))
    }

    /// Le moment où l'app a vu ce code en préparation pour la première fois : celui de la vue d'avant (`kept`, même
    /// code), sinon le sien, sinon maintenant s'il se prépare.
    pub fn keep_since(relay: Option<ShareRelay>, since: Option<Instant>, kept: Option<Instant>) -> Option<Instant> {
        kept.or(since).or_else(|| (relay == Some(ShareRelay::Connecting)).then(Instant::now))
    }
}
