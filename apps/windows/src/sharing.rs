//! KyberNow Share, ce que l'app en tire sans rien de Windows (docs/kybernow-share.md,
//! textes de docs/link-ui.md, « Partage »), pour se tester sur le Mac : les
//! Stations à qui envoyer un code, le prénom de l'invité, le panneau de
//! l'hôte tel que la barre le montre (code, lien, QR code, invités), et les
//! avis de changement de rôle. Les appels à Admin, la session d'invité et les
//! fenêtres sont dans `ui/share.rs`.

use kybernow_core::share::{self, ShareAvailability, ShareGuest, ShareRole, ShareState, ShareTarget};
use serde::Serialize;

/// Une Station vue sur le réseau ou déjà ajoutée à l'app : son adresse, le
/// port d'Admin et son empreinte quand on la connaît.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Known {
    pub host: String,
    pub port: u16,
    pub fingerprint: Option<String>,
}

/// Les Stations à qui envoyer le code d'un partage : celles annoncées sur le
/// réseau d'abord (comme pour l'appairage), puis celles des ordinateurs de
/// l'app (une adresse de VPN, par exemple), sans doublon (même adresse et
/// même port : l'empreinte qu'on connaît est gardée) et jamais ce PC (une
/// session d'invité sur le PC qui partage montrerait son propre écran).
pub fn targets(discovered: impl IntoIterator<Item = Known>, saved: impl IntoIterator<Item = Known>, is_this_pc: impl Fn(&str) -> bool) -> Vec<ShareTarget> {
    let mut out: Vec<ShareTarget> = Vec::new();
    for k in discovered.into_iter().chain(saved) {
        if is_this_pc(&k.host) {
            continue;
        }
        match out.iter_mut().find(|t| t.host.eq_ignore_ascii_case(&k.host) && t.port == k.port) {
            Some(t) => {
                if t.fingerprint.is_none() {
                    t.fingerprint = k.fingerprint;
                }
            }
            None => out.push(ShareTarget { host: k.host, port: k.port, fingerprint: k.fingerprint }),
        }
    }
    out
}

/// Longueur au plus d'un prénom (en caractères).
const NAME_MAX: usize = 40;

/// Un prénom tapé : espaces de bord et doublons retirés, coupé à 40 caractères.
pub fn clean_name(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    words.join(" ").chars().take(NAME_MAX).collect::<String>().trim().to_string()
}

/// Le prénom proposé à « Rejoindre » : celui retenu la fois d'avant, sinon le
/// nom de cet appareil.
pub fn proposed_name(remembered: Option<&str>, device: &str) -> String {
    let remembered = remembered.map(clean_name).unwrap_or_default();
    if remembered.is_empty() { clean_name(device) } else { remembered }
}

/// L'avis bref d'un changement de droits (clé du catalogue de l'app), les deux
/// droits étant indépendants : « Vous pouvez jouer à la manette. » et « Vous
/// avez le clavier et la souris. » (un droit donné), « Les manettes sont
/// reprises. » et « Le clavier et la souris sont repris. » (un droit repris,
/// l'autre gardé), « Vous regardez de nouveau. » quand il ne reste aucun droit
/// (à la place de l'avis de reprise : ce dernier dit tout). Rien au départ
/// (`previous` nul), ni quand les droits ne changent pas.
///
/// Deux changements dans un même signe de vie (l'hôte a touché aux deux boutons
/// entre deux battements) : un seul avis tient dans la barre ; le droit donné
/// passe avant le droit repris, les manettes avant le clavier et la souris.
pub fn role_notice(role: ShareRole, previous: Option<ShareRole>) -> Option<&'static str> {
    let previous = previous?;
    if role == previous {
        return None;
    }
    if !role.plays() {
        return Some("share.notice_watch");
    }
    if role.pads() && !previous.pads() {
        Some("share.notice_play")
    } else if role.kbm() && !previous.kbm() {
        Some("share.notice_kbm")
    } else if previous.pads() && !role.pads() {
        Some("share.notice_pads_back")
    } else if previous.kbm() && !role.kbm() {
        Some("share.notice_kbm_back")
    } else {
        None
    }
}

/// L'étiquette d'un rôle dans la barre de l'invité : « Joueur » dès qu'il a un
/// droit (manettes, ou clavier et souris), « Spectateur » sinon (la clé du
/// catalogue de l'app).
pub fn role_label(role: ShareRole) -> &'static str {
    if role.plays() { "share.role_player" } else { "share.role_spectator" }
}

/// Le clavier et la souris de l'invité partent vers l'ordinateur : avec ce
/// droit-là seulement, que les manettes soient données ou non.
pub fn has_keyboard_mouse(role: ShareRole) -> bool {
    role.kbm()
}

/// Ce que « Partager » peut faire, tel que la barre le montre : `available`,
/// `unsupported` (Station trop ancienne), `disabled` (partage coupé sur
/// l'ordinateur) ; `unknown` : l'ordinateur n'a pas encore dit ce qu'il sait
/// (« Partager » reste actif, Admin répondra à l'ouverture).
pub fn availability_code(a: Option<ShareAvailability>) -> &'static str {
    match a {
        None => "unknown",
        Some(ShareAvailability::Unsupported) => "unsupported",
        Some(ShareAvailability::Disabled) => "disabled",
        Some(ShareAvailability::Available) => "available",
    }
}

/// Un QR code pour l'interface : `size` modules de côté et le tracé SVG de ses
/// modules sombres (une rangée de modules voisins par segment), sans marge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Qr {
    pub size: i32,
    pub path: String,
}

pub fn qr(text: &str) -> Option<Qr> {
    use qrcodegen::{QrCode, QrCodeEcc};
    let code = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let size = code.size();
    let mut path = String::new();
    for y in 0..size {
        let mut x = 0;
        while x < size {
            if !code.get_module(x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < size && code.get_module(x, y) {
                x += 1;
            }
            path.push_str(&format!("M{start} {y}h{}v1h-{}z", x - start, x - start));
        }
    }
    Some(Qr { size, path })
}

/// Un invité, tel que le panneau de l'hôte le montre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuestView {
    pub id: String,
    pub name: String,
    /// `mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`, `web`.
    pub app: String,
    /// `spectator`, `player` (les manettes), `kbm` (le clavier et la souris) ou
    /// `player_kbm` (les deux).
    pub role: &'static str,
    /// Arrivée, en secondes Unix de la Station (docs/link-ui.md, « Durée ») : la
    /// liste dit « depuis 3 min ».
    pub since: Option<i64>,
}

impl From<&ShareGuest> for GuestView {
    fn from(g: &ShareGuest) -> Self {
        GuestView { id: g.id.clone(), name: g.name.clone(), app: g.app.clone(), role: g.role.as_str(), since: g.since }
    }
}

/// Le panneau de l'hôte, envoyé à la barre (événement `share-state`, et dans
/// `bar-shown`) : ce que « Partager » peut faire, le partage ouvert et ses invités.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShareView {
    /// Voir [`availability_code`].
    pub availability: &'static str,
    /// Le texte à montrer quand « Partager » est inactif.
    pub text: Option<String>,
    pub open: bool,
    /// « 123 456 ».
    pub code: String,
    /// Le lien des apps (`kybernow://join?…`) : celui du QR code.
    pub link: String,
    /// L'adresse du client web : celle que « Copier le lien » copie.
    pub web: String,
    /// L'écran partagé (« Écran 1 »).
    pub display: Option<String>,
    pub guests: Vec<GuestView>,
    /// Quatre invités : « Quatre invités au plus. »
    pub full: bool,
    pub qr: Option<Qr>,
}

impl ShareView {
    /// `state` : le partage tel qu'Admin le dit (`None` : pas de partage) ;
    /// `availability` : ce que l'ordinateur annonce (`None` : pas encore su) ;
    /// `lang` : la langue du texte d'un « Partager » inactif.
    pub fn new(state: Option<&ShareState>, availability: Option<ShareAvailability>, lang: kybernow_core::i18n::Lang) -> Self {
        let open = state.is_some_and(|s| s.open);
        let text = availability.and_then(|a| a.text(lang));
        let availability = availability_code(availability);
        match state.filter(|s| s.open) {
            Some(s) => ShareView {
                availability,
                text,
                open,
                code: share::format_code(&s.code),
                link: s.link.clone(),
                web: s.web.clone(),
                display: s.display.as_ref().map(|d| d.label.clone()).filter(|l| !l.is_empty()),
                guests: s.guests.iter().map(GuestView::from).collect(),
                full: s.guests.len() >= share::MAX_GUESTS,
                qr: qr(&s.link),
            },
            None => ShareView {
                availability,
                text,
                open: false,
                code: String::new(),
                link: String::new(),
                web: String::new(),
                display: None,
                guests: Vec::new(),
                full: false,
                qr: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::i18n::Lang;
    use kybernow_core::share::ShareScreen;

    fn known(host: &str, port: u16, fp: Option<&str>) -> Known {
        Known { host: host.into(), port, fingerprint: fp.map(str::to_string) }
    }

    #[test]
    fn stations_a_essayer() {
        let seen = vec![known("192.168.1.100", 9443, None), known("192.168.1.77", 31544, Some("sha256:aa"))];
        let saved = vec![
            // La même Station : l'empreinte connue est gardée, pas de doublon.
            known("192.168.1.100", 9443, Some("sha256:bb")),
            // Une adresse que le réseau n'annonce pas (VPN).
            known("100.64.0.9", 9443, Some("sha256:cc")),
            // Ce PC : jamais.
            known("127.0.0.1", 9443, None),
        ];
        let t = targets(seen, saved, |h| h == "127.0.0.1");
        assert_eq!(
            t,
            vec![
                ShareTarget { host: "192.168.1.100".into(), port: 9443, fingerprint: Some("sha256:bb".into()) },
                ShareTarget { host: "192.168.1.77".into(), port: 31544, fingerprint: Some("sha256:aa".into()) },
                ShareTarget { host: "100.64.0.9".into(), port: 9443, fingerprint: Some("sha256:cc".into()) },
            ]
        );
        // Une empreinte annoncée par le réseau n'est pas remplacée.
        let t = targets(vec![known("a", 1, Some("sha256:1"))], vec![known("A", 1, Some("sha256:2"))], |_| false);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].fingerprint.as_deref(), Some("sha256:1"));
        // Même adresse, autre port : une autre Station.
        assert_eq!(targets(vec![known("a", 1, None), known("a", 2, None)], vec![], |_| false).len(), 2);
        assert!(targets(vec![], vec![], |_| false).is_empty());
    }

    #[test]
    fn prenom() {
        assert_eq!(clean_name("  Léa   Martin "), "Léa Martin");
        assert_eq!(clean_name("   "), "");
        assert_eq!(clean_name(&"é".repeat(60)).chars().count(), NAME_MAX);
        // Le prénom retenu passe avant le nom de l'appareil.
        assert_eq!(proposed_name(Some("Léa"), "PORTABLE-BUREAU"), "Léa");
        assert_eq!(proposed_name(None, "PORTABLE-BUREAU"), "PORTABLE-BUREAU");
        assert_eq!(proposed_name(Some("  "), "PORTABLE-BUREAU"), "PORTABLE-BUREAU");
    }

    #[test]
    fn avis_de_role() {
        use ShareRole::*;
        // Rien au départ, ni sans changement.
        assert_eq!(role_notice(Spectator, None), None);
        assert_eq!(role_notice(PlayerKbm, None), None);
        for r in [Spectator, Player, Kbm, PlayerKbm] {
            assert_eq!(role_notice(r, Some(r)), None);
        }
        // Un droit donné (rôle d'arrivée, rôle d'avant).
        assert_eq!(role_notice(Player, Some(Spectator)), Some("share.notice_play"));
        assert_eq!(role_notice(PlayerKbm, Some(Kbm)), Some("share.notice_play"));
        assert_eq!(role_notice(Kbm, Some(Spectator)), Some("share.notice_kbm"));
        assert_eq!(role_notice(PlayerKbm, Some(Player)), Some("share.notice_kbm"));
        // Un droit repris, l'autre gardé.
        assert_eq!(role_notice(Kbm, Some(PlayerKbm)), Some("share.notice_pads_back"));
        assert_eq!(role_notice(Player, Some(PlayerKbm)), Some("share.notice_kbm_back"));
        // Plus aucun droit : « Vous regardez de nouveau. » à la place de l'avis de reprise.
        assert_eq!(role_notice(Spectator, Some(Player)), Some("share.notice_watch"));
        assert_eq!(role_notice(Spectator, Some(Kbm)), Some("share.notice_watch"));
        assert_eq!(role_notice(Spectator, Some(PlayerKbm)), Some("share.notice_watch"));
        // Deux changements d'un coup : le droit donné passe avant le droit repris, les manettes avant le clavier.
        assert_eq!(role_notice(PlayerKbm, Some(Spectator)), Some("share.notice_play"));
        assert_eq!(role_notice(Kbm, Some(Player)), Some("share.notice_kbm"));
        assert_eq!(role_notice(Player, Some(Kbm)), Some("share.notice_play"));
    }

    #[test]
    fn etiquettes() {
        use ShareRole::*;
        // « Joueur » dès qu'il a un droit, « Spectateur » sans aucun.
        assert_eq!(role_label(Spectator), "share.role_spectator");
        for r in [Player, Kbm, PlayerKbm] {
            assert_eq!(role_label(r), "share.role_player");
        }
        // Le clavier et la souris partent avec leur droit, que les manettes soient données ou non.
        assert!(has_keyboard_mouse(PlayerKbm));
        assert!(has_keyboard_mouse(Kbm));
        assert!(!has_keyboard_mouse(Player));
        assert!(!has_keyboard_mouse(Spectator));
    }

    fn guest(id: &str, name: &str, role: ShareRole) -> ShareGuest {
        ShareGuest { id: id.into(), name: name.into(), app: "ios".into(), role, since: Some(1), ..ShareGuest::default() }
    }

    #[test]
    fn panneau_de_l_hote() {
        let state = ShareState {
            open: true,
            code: "123456".into(),
            link: "kybernow://join?host=192.168.1.100&port=9443&fp=sha256:aa&code=123456".into(),
            web: "https://192.168.1.100:9443/link/?join=123456".into(),
            display: Some(ShareScreen { id: 1, label: "Écran 1".into() }),
            guests: vec![guest("g1", "Léa", ShareRole::Spectator), guest("g2", "Noé", ShareRole::PlayerKbm), guest("g3", "Zoé", ShareRole::Kbm)],
        };
        let v = ShareView::new(Some(&state), Some(ShareAvailability::Available), Lang::Fr);
        assert_eq!((v.availability, v.open, v.code.as_str(), v.full), ("available", true, "123 456", false));
        assert_eq!(v.display.as_deref(), Some("Écran 1"));
        assert_eq!(v.text, None);
        // « Copier le lien » copie l'adresse du client web ; le QR code porte le lien des apps.
        assert_eq!(v.web, "https://192.168.1.100:9443/link/?join=123456");
        assert!(v.link.starts_with("kybernow://join?"));
        assert_eq!(v.guests[0].since, Some(1));
        assert_eq!(v.guests[0].app, "ios");
        assert_eq!(v.guests.len(), 3);
        assert_eq!((v.guests[1].name.as_str(), v.guests[1].role), ("Noé", "player_kbm"));
        assert_eq!((v.guests[2].name.as_str(), v.guests[2].role), ("Zoé", "kbm"));
        assert!(v.qr.as_ref().is_some_and(|q| q.size >= 21 && q.path.starts_with('M')));
        // Quatre invités : la limite.
        let mut full = state.clone();
        full.guests = (0..4).map(|i| guest(&format!("g{i}"), "X", ShareRole::Spectator)).collect();
        assert!(ShareView::new(Some(&full), None, Lang::Fr).full);
        // Pas de partage : un panneau vide, « Partager » reste ce que l'ordinateur annonce.
        let none = ShareView::new(None, Some(ShareAvailability::Available), Lang::Fr);
        assert!(!none.open && none.code.is_empty() && none.guests.is_empty() && none.qr.is_none());
        // Un partage dit fermé n'est pas ouvert.
        let closed = ShareState { open: false, ..state };
        assert!(!ShareView::new(Some(&closed), None, Lang::Fr).open);
    }

    #[test]
    fn partager_inactif() {
        let old = ShareView::new(None, Some(ShareAvailability::Unsupported), Lang::Fr);
        assert_eq!(old.availability, "unsupported");
        assert_eq!(old.text.as_deref(), Some("Le partage demande KyberNow Station 0.8.0 ou plus récente."));
        let off = ShareView::new(None, Some(ShareAvailability::Disabled), Lang::En);
        assert_eq!(off.availability, "disabled");
        assert_eq!(off.text.as_deref(), Some("Sharing is turned off on this computer (Admin › Link devices)."));
        // Pas encore su : actif, sans texte.
        let unknown = ShareView::new(None, None, Lang::Fr);
        assert_eq!((unknown.availability, unknown.text), ("unknown", None));
    }

    #[test]
    fn qr_code() {
        let q = qr("kybernow://join?host=192.168.1.100&port=9443&code=123456").unwrap();
        // Version 4 ou 5 pour ce texte : 33 ou 37 modules ; le tracé reste dans le carré.
        assert!((21..=45).contains(&q.size), "{}", q.size);
        assert!(q.path.contains('h') && q.path.ends_with('z'));
        // Les trois repères des coins : le module (0, 0) est sombre, le tracé commence en haut à gauche.
        assert!(q.path.starts_with("M0 0h7"));
        // Chaque segment tient dans le carré.
        for seg in q.path.split('z').filter(|s| !s.is_empty()) {
            let (m, rest) = seg[1..].split_once('h').unwrap();
            let (x, y) = m.split_once(' ').unwrap();
            let len: i32 = rest.split_once('v').unwrap().0.parse().unwrap();
            assert!(x.parse::<i32>().unwrap() + len <= q.size && y.parse::<i32>().unwrap() < q.size);
        }
    }
}
