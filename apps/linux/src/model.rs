//! Ce que l'app garde : les ordinateurs (adresse d'Admin, empreinte, compte),
//! leurs réglages, la dernière session, l'identité de cet appareil et les
//! réglages de l'app. Un fichier JSON dans le dossier de configuration de
//! l'utilisateur (`Store::dir` : `$XDG_CONFIG_HOME/kybernow`, soit
//! `~/.var/app/fr.leslignesclaires.kybernow/config/kybernow` en Flatpak).
//!
//! Les mots de passe sont dans ce fichier, lisible par l'utilisateur seul
//! (droits 0600) : en mode Jeu de SteamOS, aucun trousseau n'est déverrouillé.
//! Le compte est propre à cet appareil (accordé par le code à six chiffres)
//! et se retire dans Admin.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Profils, dans le vocabulaire du cœur (`Profile::parse`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    #[default]
    Jeu,
    Bureau,
    Max,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::Jeu, Profile::Bureau, Profile::Max];

    pub fn id(self) -> &'static str {
        match self {
            Profile::Jeu => "jeu",
            Profile::Bureau => "bureau",
            Profile::Max => "max",
        }
    }
}

/// Réglages d'un ordinateur, comme Réglages › par ordinateur sur les autres apps.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HostSettings {
    pub profile: Profile,
    /// 0 : comme cet écran ; sinon hauteur (2160, 1440, 1080, 800, 720).
    pub resolution: u32,
    /// Écrans pris par la session (multi-écrans, docs/multi-ecrans.md) : `auto`
    /// (tous les écrans de cet appareil en Bureau et Max, un seul en Jeu et
    /// en Personnalisé), `this` (cet écran), `all` (tous les écrans). Gardé
    /// comme le cœur (`DisplaysSetting`) ; ne rend jamais le profil « Personnalisé ».
    pub displays: String,
    /// Images par seconde : 30, 60, 90, 120, 144 (0 : celle de l'écran).
    pub fps: u32,
    /// `auto`, `hevc`, `h264`, `av1`.
    pub codec: String,
    /// 0 : celui du profil ; sinon en Mbit/s.
    pub mbps: u32,
    /// Affichage : `auto` (fréquence variable si l'écran la permet, sinon
    /// synchronisé), `vsync`, `immediate`, `vrr`.
    pub display: String,
    /// `auto`, `stereo`, `5.1`, `7.1`, `computer` (le son reste sur l'ordinateur).
    pub audio: String,
    /// Souris : `auto`, `desktop`, `game`.
    pub mouse: String,
    /// Toutes les manettes vues comme des Xbox 360 par l'ordinateur.
    pub xbox360: bool,
    /// L1 + R1 + Start + Select ouvrent le menu.
    pub combo: bool,
    /// « Ctrl agit comme ⌘ », face à une Station Mac seulement ; `None` : jamais changé, allumé
    /// (`keymap::control_as_command` du cœur).
    pub ctrl_command: Option<bool>,
    pub clipboard: bool,
    pub reconnect: bool,
    /// Transport fiable même en profil Jeu (Wi-Fi chargé).
    pub reliable: bool,
    /// Sur batterie : 60 i/s et débit réduit au plus.
    pub battery_light: bool,
    /// Écran virtuel de la Station (sinon les écrans de l'ordinateur tels quels).
    pub virtual_display: bool,
    /// Couleurs 4:4:4 (texte net) en HEVC : en option, la carte (VA-API) ou le
    /// processeur décode, l'app convertit en RVB.
    pub yuv444: bool,
    /// Le HDR pour cet ordinateur (puce de l'accueil, Réglages › Image, barre et menu de session) : allumé la
    /// première fois (absent d'un fichier d'avant le HDR : allumé aussi). Il ne compte pas dans le profil. Sans
    /// effet sur un écran SDR.
    pub hdr: bool,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            profile: Profile::Jeu,
            resolution: 0,
            displays: "auto".into(),
            fps: 0,
            codec: "auto".into(),
            mbps: 0,
            display: "auto".into(),
            audio: "auto".into(),
            mouse: "auto".into(),
            xbox360: true,
            combo: true,
            ctrl_command: None,
            clipboard: true,
            reconnect: true,
            reliable: false,
            battery_light: true,
            virtual_display: true,
            yuv444: false,
            hdr: true,
        }
    }
}

impl HostSettings {
    /// Les réglages s'écartent du profil (« Personnalisé ») : format ou débit
    /// choisis, 4:4:4 en Jeu (Bureau et Max le prennent quand il est permis),
    /// transport fiable hors Bureau (Bureau l'est déjà).
    pub fn custom(&self) -> bool {
        self.codec != "auto" || self.mbps != 0 || (self.yuv444 && self.profile == Profile::Jeu) || (self.reliable && self.profile != Profile::Bureau)
    }
}

/// La dernière session d'au moins 3 s (Réglages de l'ordinateur ›
/// Statistiques) : latence médiane et 95e centile (ms, 0 : non mesurée),
/// durée (s), date, définition, fréquence, format et débit moyen.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct LastSession {
    pub median: f64,
    pub p95: f64,
    pub seconds: u64,
    /// « 1920 × 1200 » ; les fichiers de la 0.4.x y mettent aussi le format
    /// (« 1920 × 1200 · HEVC ») et n'ont ni `hz` ni `codec`.
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub mbps: f64,
    pub at: u64,
    /// Fréquence (Hz), 0 : inconnue.
    #[serde(default)]
    pub hz: u32,
    /// « HEVC », « H.264 », « AV1 » ; vide : inconnu.
    #[serde(default)]
    pub codec: String,
    /// Écrans pris par la session (0 ou 1 : un seul) ; avec plusieurs, la
    /// définition et la fréquence sont celles de l'écran principal, le débit
    /// le total.
    #[serde(default)]
    pub screens: u32,
    /// Le chemin de la session : `direct`, `relay` (par le relais) ou `relay_tcp`
    /// (par le relais, repli TCP) ; vide : direct, ou une session d'avant le relais
    /// (le `RouteInfo` du cœur, ramené à son transport).
    #[serde(default)]
    pub route: String,
    /// Un écran au moins était en HDR : « HDR » après le format.
    #[serde(default)]
    pub hdr: bool,
}

impl LastSession {
    /// Ce qui a été transmis : « 2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s » ; avec
    /// plusieurs écrans, leur nombre d'abord : « 3 écrans · 3840 × 2160 · 160 Hz
    /// · HEVC · 120 Mbit/s » ; « HDR » après le format quand un écran au moins
    /// l'était (docs/link-ui.md, « Statistiques »).
    pub fn transmitted(&self) -> String {
        let mut v: Vec<String> = Vec::new();
        if self.screens >= 2 {
            v.push(format!("{} {}", self.screens, crate::i18n::tr("écrans", "displays")));
        }
        if !self.mode.is_empty() {
            v.push(self.mode.clone());
        }
        if self.hz > 0 {
            v.push(format!("{} Hz", self.hz));
        }
        if !self.codec.is_empty() {
            v.push(self.codec.clone());
        }
        if self.hdr {
            v.push("HDR".into());
        }
        if self.mbps > 0.0 {
            v.push(format!("{:.0} Mbit/s", self.mbps));
        }
        v.join(" · ")
    }
}

/// Un ordinateur équipé de KyberNow Station.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Host {
    pub id: String,
    pub name: String,
    /// Adresse et port d'Admin.
    pub address: String,
    pub port: u16,
    /// Empreinte du certificat d'Admin (`sha256:…`), épinglée.
    pub fingerprint: Option<String>,
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub os: Option<String>,
    #[serde(default)]
    pub gpu: Option<String>,
    #[serde(default)]
    pub av1: bool,
    /// Adresse MAC saisie à la main (Réglages › Réseau, réglage avancé) :
    /// le réveil s'en sert quand la Station n'a pas donné ses cartes.
    #[serde(default)]
    pub mac: Option<String>,
    /// Les cartes réseau de l'ordinateur pour le réveil, telles qu'Admin les
    /// donne (`wake` de `GET /api/stream/info`, Station 0.5.0 et suivantes),
    /// gardées d'un `info` réussi à l'autre (une liste vide ne les efface pas).
    #[serde(default)]
    pub wake: Vec<WakeCard>,
    /// Le relais de l'ordinateur (`relay` de `GET /api/stream/info`), gardé d'un `info` réussi à l'autre : de quoi
    /// le joindre hors de chez soi (`Route::Auto`) et le dire « Hors de chez vous » sur l'accueil.
    #[serde(default)]
    pub relay: Option<HostRelay>,
    #[serde(default)]
    pub settings: HostSettings,
    #[serde(default)]
    pub last: Option<LastSession>,
}

/// Le relais d'un ordinateur, tel que la Station le donne (`RelayInfo` du cœur) : le relais (`hôte:port`, ou
/// `hôte:port#sha256:…` pour un relais épinglé), l'identifiant de la Station chez lui, et « Accès hors de chez
/// vous » (actif dans Admin ?). Sans lui, l'ordinateur ne se joint que sur le réseau.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct HostRelay {
    pub server: String,
    pub id: String,
    pub remote_access: bool,
}

/// Une carte réseau de l'ordinateur à réveiller, au JSON de `WakeTarget` du
/// cœur (`apps/core/src/wake.rs`) : l'app la garde telle qu'Admin la donne.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct WakeCard {
    /// `AA:BB:CC:DD:EE:FF`.
    pub mac: String,
    pub ipv4: Option<String>,
    pub broadcast: Option<String>,
    /// `ethernet`, `wifi`, ou vide.
    pub kind: String,
    pub armed: Option<bool>,
    pub name: String,
}

impl Host {
    /// L'app sait réveiller cet ordinateur : des cartes apprises d'Admin, ou une MAC saisie.
    pub fn can_wake(&self) -> bool {
        !self.wake.is_empty() || self.mac.is_some()
    }
}

/// Réglages de l'app.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    /// `auto`, `fr`, `en`.
    pub language: String,
    /// Mises à jour : aussi les bêtas (sinon les versions stables seules).
    pub beta: bool,
    /// Interface : `auto` (console sous gamescope, bureau ailleurs),
    /// `desktop`, `handheld`, `tv`.
    pub interface: String,
    /// Garder la dernière image de chaque session pour l'accueil.
    pub keep_last_image: bool,
    /// KyberNow Link OS : le premier démarrage (manette, langue, réseau, usage,
    /// ordinateur) est fait.
    pub os_setup_done: bool,
    /// KyberNow Link OS : HDMI-CEC, tout ou rien : l'annonce du nom
    /// « KyberNow Link » au téléviseur au lancement (qui donne aussi à
    /// l'appareil son adresse CEC, sans laquelle la télécommande ne lui parle
    /// pas), la veille et le réveil du téléviseur avec l'appareil.
    pub os_cec: bool,
    /// KyberNow Link OS : télécharger les versions dès qu'elles sortent ;
    /// l'installation attend « Redémarrer sur la X ».
    pub os_auto_update: bool,
    /// KyberNow Link OS : mode de la sortie HDMI au démarrage : `auto`,
    /// `2160p60`, `1080p120`, `1080p60`.
    pub os_hdmi: String,
    /// KyberNow Share : le prénom donné en rejoignant un partage, retenu pour
    /// la fois suivante (vide : le nom de cet appareil est proposé). Avec la qualité de l'image ci-dessous, la
    /// seule trace d'un partage sur l'appareil : un invité n'est jamais ajouté aux ordinateurs.
    pub share_name: String,
    /// KyberNow Share : la « Qualité de l'image » choisie en rejoignant un partage ou dans la barre, retenue
    /// pour la fois suivante (`auto`, `host`, `2160p60`, `1080p60`, `720p30` ; vide ou inconnue : « Auto »).
    pub share_quality: String,
    /// KyberNow Share, prêt : l'avertissement de confiance (« Votre ami joue dans votre session Windows… ») est dit
    /// dans le panneau « Prêter l'ordinateur… » jusqu'au premier code créé, puis plus (« une fois », docs/link-ui.md).
    pub lend_warned: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { language: "auto".into(), beta: false, interface: "auto".into(), keep_last_image: true, os_setup_done: false, os_cec: true, os_auto_update: true, os_hdmi: "auto".into(), share_name: String::new(), share_quality: String::new(), lend_warned: false }
    }
}

/// Un ordinateur emprunté (KyberNow Share, prêt de l'ordinateur, docs/pret.md) : la Station d'un ami qui a créé un
/// compte d'ami pour cet appareil. Gardé à part des ordinateurs appairés (`Store::borrowed`) : ni réglages, ni
/// dernière session, ni Station à redémarrer ; la carte « Ordinateur de {hôte} » de l'accueil et « Emprunter ».
/// Le mot de passe est dans ce fichier (droits 0600, comme ceux des ordinateurs) seulement si « Retenir le mot de
/// passe » est coché ; sinon l'app le garde le temps de son lancement (`App::lend_passwords`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct Borrowed {
    /// `lend:<uuid>` : jamais celui d'un ordinateur appairé (`Store::selected` désigne l'un ou l'autre).
    pub id: String,
    /// Le prénom de l'hôte (« Ordinateur de Matthieu ») et le nom de son ordinateur, tels qu'Admin les donne.
    pub host: String,
    pub computer: String,
    /// Adresse et port d'Admin, empreinte épinglée.
    pub address: String,
    pub port: u16,
    pub fingerprint: Option<String>,
    /// Le relais de la Station et son identifiant chez lui (lien d'invitation) : l'ordinateur s'emprunte aussi hors de
    /// chez soi. `None` : ajouté sur le réseau, ou lien sans relais.
    pub relay: Option<String>,
    pub sid: Option<String>,
    /// Le compte d'ami (« Identifiant »), et son mot de passe s'il est retenu.
    pub login: String,
    pub password: String,
    pub remember: bool,
    /// La puce « HDR » de cet ordinateur emprunté éteinte (allumée la première fois, comme celle d'un hôte).
    pub hdr_off: bool,
}

impl Borrowed {
    /// « Ordinateur de Matthieu », “Matthieu's computer” : le nom de la carte et de la session.
    pub fn title(&self) -> String {
        borrowed_title(&self.host)
    }
}

/// « Ordinateur de {hôte} » (élidé : « Ordinateur d'Hugo »), “{host}'s computer”.
pub fn borrowed_title(host: &str) -> String {
    if crate::i18n::english() { format!("{host}'s computer") } else { format!("Ordinateur {}", crate::i18n::de(host)) }
}

// MARK: - Consoles PlayStation (docs/playstation.md)

/// Un compte PlayStation Network : son identifiant (base64, lu par la connexion Sony, reçu d'un autre appareil
/// ou saisi à la main) et son nom (le pseudo PSN, ou celui donné). Dans ce fichier (droits 0600), comme les mots
/// de passe des ordinateurs : en mode Jeu de SteamOS et sur KyberNow Link OS, aucun trousseau n'est déverrouillé.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct PsAccount {
    /// Un UUID, le même que celui de l'appareil qui l'a envoyé quand il est libre.
    pub id: String,
    /// `None` : un compte gardé sans nom (« Compte PlayStation »).
    pub name: Option<String>,
    /// L'identifiant du compte, en base64 (8 octets).
    pub psn_id: String,
}

impl PsAccount {
    /// Le nom affiché : le pseudo, sinon « Compte PlayStation ».
    pub fn label(&self) -> String {
        self.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| crate::i18n::tr("Compte PlayStation", "PlayStation account").into())
    }
}

/// Une association : le compte et ce que la console a donné (le JSON `Registered` du cœur, gardé tel quel).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PsPairing {
    pub account: String,
    pub registered: serde_json::Value,
}

/// Les réglages d'une console : le JSON de `PlaySettings` du cœur (mêmes noms, mêmes valeurs), recopié ici pour
/// l'interface seule (captures, sans le cœur). L'agrandissement (`upscale`, `sharpness`) est propre aux appareils
/// Apple : gardé pour ne rien perdre, jamais montré.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PsSettings {
    /// `auto`, `p1080`, `p720`, `p540`, `p360`.
    pub resolution: String,
    /// 60 ou 30.
    pub fps: u32,
    /// kbit/s ; `None` : Auto (suit la définition).
    pub bitrate_kbps: Option<u32>,
    /// `auto`, `h264`, `h265`.
    pub codec: String,
    pub hdr: bool,
    pub fullscreen: bool,
    pub upscale: bool,
    pub sharpness: u8,
    /// Intensités de la DualSense, de 0 à 100 %.
    pub rumble: u8,
    pub triggers: u8,
}

impl Default for PsSettings {
    fn default() -> Self {
        Self { resolution: "auto".into(), fps: 60, bitrate_kbps: None, codec: "auto".into(), hdr: false, fullscreen: true, upscale: true, sharpness: 40, rumble: 100, triggers: 100 }
    }
}

#[allow(dead_code)]
impl PsSettings {
    /// « Personnalisé » : définition, images par seconde, débit ou format s'écartent d'Auto (`PlaySettings::is_custom`).
    pub fn custom(&self) -> bool {
        self.resolution != "auto" || self.fps != 60 || self.bitrate_kbps.is_some() || self.codec != "auto"
    }

    /// La hauteur de la définition (Auto : 1080).
    pub fn height(&self) -> u32 {
        match self.resolution.as_str() {
            "p720" => 720,
            "p540" => 540,
            "p360" => 360,
            _ => 1080,
        }
    }

    /// Le débit effectif, kbit/s (Auto : préréglages de chiaki-ng, `Resolution::auto_bitrate`).
    pub fn bitrate(&self) -> u32 {
        self.bitrate_kbps.map(|b| b.clamp(2_000, 100_000)).unwrap_or(match self.height() {
            720 => 10_000,
            540 => 6_000,
            360 => 2_000,
            _ => 15_000,
        })
    }
}

/// Une console PlayStation : trouvée sur le réseau (ou par son adresse, ou reçue d'un autre appareil), avec ses
/// associations (une par compte), le compte choisi pour jouer, ses réglages et sa disposition du clavier et de la
/// souris. Gardée tant qu'un compte au moins y est associé.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PsConsole {
    /// `host-id` de la découverte (= `mac` de l'association), 12 chiffres hexadécimaux, en majuscules.
    pub host_id: String,
    pub name: String,
    /// `PS5` ou `PS4`.
    pub model: String,
    /// La cible de l'association (`ps5_v1`, `ps4_v10`…), pour réassocier.
    pub target: Option<String>,
    /// Adresses retenues (« Par adresse », un VPN) : la console y est cherchée en direct, en plus du réseau.
    pub addresses: Vec<String>,
    /// Les ordinateurs (`Host::id`) dont la Station voit la console : hors de chez soi, la partie passe par l'un d'eux.
    pub stations: Vec<String>,
    pub pairings: Vec<PsPairing>,
    /// Le compte choisi pour jouer (la puce de l'accueil), retenu par console.
    pub account: Option<String>,
    pub settings: PsSettings,
    /// La disposition du clavier et de la souris (JSON de `keymouse::Layout`) ; `None` : celle par défaut.
    pub layout: Option<serde_json::Value>,
    /// La puce « HDR » de l'accueil éteinte pour cette console (allumée la première fois ; une PS5 sur un écran HDR).
    pub hdr_off: bool,
}

impl PsConsole {
    /// L'association du compte choisi (ou de la première).
    pub fn pairing(&self) -> Option<&PsPairing> {
        self.account.as_ref().and_then(|a| self.pairings.iter().find(|p| &p.account == a)).or_else(|| self.pairings.first())
    }

    pub fn ps4(&self) -> bool {
        self.model.eq_ignore_ascii_case("PS4")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    /// Identifiant de cet appareil auprès d'Admin, tiré une fois.
    pub device_id: String,
    pub hosts: Vec<Host>,
    /// KyberNow Share : les ordinateurs empruntés (comptes d'amis), à part des ordinateurs appairés.
    pub borrowed: Vec<Borrowed>,
    /// Dernier ordinateur choisi sur l'accueil (un ordinateur appairé, ou emprunté : `lend:…`, ou une console :
    /// `ps:<host_id>`).
    pub selected: Option<String>,
    /// Consoles PlayStation : les comptes PSN, et les consoles associées.
    pub ps_accounts: Vec<PsAccount>,
    pub ps_consoles: Vec<PsConsole>,
    pub app: AppSettings,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Store {
    /// Le dossier de l'app : `$XDG_CONFIG_HOME/kybernow` (Flatpak :
    /// `~/.var/app/fr.leslignesclaires.kybernow/config/kybernow`), sinon
    /// `~/.config/kybernow`, sinon `/tmp/kybernow`.
    pub fn dir() -> PathBuf {
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let mut candidates = Vec::new();
            if let Some(x) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
                candidates.push(PathBuf::from(x).join("kybernow"));
            }
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(PathBuf::from(home).join(".config/kybernow"));
            }
            candidates.push(PathBuf::from("/tmp/kybernow"));
            candidates.into_iter().find(|d| writable(d)).unwrap_or_else(|| PathBuf::from("/tmp/kybernow"))
        })
        .clone()
    }

    pub fn load() -> Self {
        let path = Self::dir().join("kybernow.json");
        let mut s: Store = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if s.device_id.is_empty() {
            s.device_id = new_id();
        }
        s.path = Some(path);
        s.save();
        s
    }

    /// Écrit le fichier (par un fichier temporaire renommé : jamais à moitié écrit).
    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let Ok(bytes) = serde_json::to_vec_pretty(self) else { return };
        let tmp = path.with_extension("json.tmp");
        // Lisible par l'utilisateur seul : le fichier porte les mots de passe.
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let written = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).and_then(|mut f| f.write_all(&bytes));
        if written.is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }

    pub fn host(&self, id: &str) -> Option<&Host> {
        self.hosts.iter().find(|h| h.id == id)
    }

    pub fn host_mut(&mut self, id: &str) -> Option<&mut Host> {
        self.hosts.iter_mut().find(|h| h.id == id)
    }

    /// Ajoute un ordinateur, ou met à jour celui de même adresse.
    pub fn put(&mut self, host: Host) -> String {
        if let Some(h) = self.hosts.iter_mut().find(|h| h.address == host.address && h.port == host.port) {
            let id = h.id.clone();
            let settings = h.settings.clone();
            let last = h.last.clone();
            // Ajouté à nouveau : ses réglages, sa dernière session et de quoi le réveiller restent.
            let wake = if host.wake.is_empty() { h.wake.clone() } else { host.wake.clone() };
            let mac = host.mac.clone().or_else(|| h.mac.clone());
            let relay = host.relay.clone().or_else(|| h.relay.clone());
            *h = Host { id: id.clone(), settings, last, wake, mac, relay, ..host };
            return id;
        }
        let id = host.id.clone();
        self.hosts.push(host);
        id
    }

    /// Un ordinateur emprunté.
    pub fn borrowed(&self, id: &str) -> Option<&Borrowed> {
        self.borrowed.iter().find(|b| b.id == id)
    }

    /// Ajoute un ordinateur emprunté, ou met à jour celui du même compte sur la même Station.
    pub fn put_borrowed(&mut self, b: Borrowed) -> String {
        if let Some(old) = self.borrowed.iter_mut().find(|o| o.address == b.address && o.port == b.port && o.login == b.login) {
            let id = old.id.clone();
            // Le choix « HDR » de cet ordinateur reste.
            *old = Borrowed { id: id.clone(), hdr_off: old.hdr_off, ..b };
            return id;
        }
        let id = b.id.clone();
        self.borrowed.push(b);
        id
    }

    pub fn console(&self, host_id: &str) -> Option<&PsConsole> {
        self.ps_consoles.iter().find(|c| c.host_id.eq_ignore_ascii_case(host_id))
    }

    pub fn console_mut(&mut self, host_id: &str) -> Option<&mut PsConsole> {
        self.ps_consoles.iter_mut().find(|c| c.host_id.eq_ignore_ascii_case(host_id))
    }

    pub fn ps_account(&self, id: &str) -> Option<&PsAccount> {
        self.ps_accounts.iter().find(|a| a.id == id)
    }

    /// Les consoles associées avec ce compte.
    pub fn consoles_of(&self, account: &str) -> usize {
        self.ps_consoles.iter().filter(|c| c.pairings.iter().any(|p| p.account == account)).count()
    }

    /// Garde un compte : un compte de même identifiant prend le nouveau nom (s'il en a un), sans doublon. Rend son
    /// identifiant et vrai s'il était déjà là.
    pub fn put_ps_account(&mut self, psn_id: &str, name: Option<String>, id: Option<String>) -> (String, bool) {
        if let Some(a) = self.ps_accounts.iter_mut().find(|a| a.psn_id == psn_id) {
            if name.as_ref().is_some_and(|n| !n.trim().is_empty()) {
                a.name = name;
            }
            return (a.id.clone(), true);
        }
        let id = id.filter(|i| !i.is_empty() && self.ps_account(i).is_none()).unwrap_or_else(new_id);
        self.ps_accounts.push(PsAccount { id: id.clone(), name, psn_id: psn_id.into() });
        (id, false)
    }

    /// Retire un compte et ses associations ; une console sans plus d'association part.
    pub fn remove_ps_account(&mut self, id: &str) {
        self.ps_accounts.retain(|a| a.id != id);
        for c in &mut self.ps_consoles {
            c.pairings.retain(|p| p.account != id);
            if c.account.as_deref() == Some(id) {
                c.account = c.pairings.first().map(|p| p.account.clone());
            }
        }
        self.drop_unpaired();
    }

    /// Une association de plus (ou remplacée, pour ce compte), la console créée si besoin ; elle devient celle
    /// du compte choisi quand la console n'en avait pas.
    pub fn put_pairing(&mut self, console: PsConsole, account: &str, registered: serde_json::Value) {
        let host_id = console.host_id.to_ascii_uppercase();
        if self.console(&host_id).is_none() {
            self.ps_consoles.push(PsConsole { host_id: host_id.clone(), pairings: Vec::new(), ..console.clone() });
        }
        let c = self.console_mut(&host_id).expect("console");
        if !console.name.is_empty() {
            c.name = console.name;
        }
        if !console.model.is_empty() {
            c.model = console.model;
        }
        if console.target.is_some() {
            c.target = console.target;
        }
        for a in console.addresses {
            if !c.addresses.contains(&a) {
                c.addresses.push(a);
            }
        }
        c.pairings.retain(|p| p.account != account);
        c.pairings.push(PsPairing { account: account.into(), registered });
        if c.account.as_ref().is_none_or(|a| !c.pairings.iter().any(|p| &p.account == a)) {
            c.account = Some(account.into());
        }
    }

    /// La console ne reconnaît plus ce compte : son association est effacée (la console part si c'était la
    /// dernière).
    pub fn forget_pairing(&mut self, host_id: &str, account: &str) {
        if let Some(c) = self.console_mut(host_id) {
            c.pairings.retain(|p| p.account != account);
            if c.account.as_deref() == Some(account) {
                c.account = c.pairings.first().map(|p| p.account.clone());
            }
        }
        self.drop_unpaired();
    }

    fn drop_unpaired(&mut self) {
        let gone: Vec<String> = self.ps_consoles.iter().filter(|c| c.pairings.is_empty()).map(|c| format!("ps:{}", c.host_id)).collect();
        self.ps_consoles.retain(|c| !c.pairings.is_empty());
        if self.selected.as_ref().is_some_and(|s| gone.contains(s)) {
            self.selected = None;
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.hosts.retain(|h| h.id != id);
        self.borrowed.retain(|b| b.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
    }

    /// Dossier des dernières images de session (une par ordinateur).
    pub fn image_path(id: &str) -> PathBuf {
        let d = Self::dir().join("images");
        let _ = std::fs::create_dir_all(&d);
        d.join(format!("{id}.jpg"))
    }
}

/// On peut créer le dossier et y écrire.
fn writable(d: &std::path::Path) -> bool {
    let probe = d.join(".probe");
    let ok = std::fs::create_dir_all(d).is_ok() && std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}

/// Un identifiant aléatoire au format UUID v4.
pub fn new_id() -> String {
    let mut b = [0u8; 16];
    if getrandom::getrandom(&mut b).is_err() {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        b.copy_from_slice(&t.to_le_bytes());
    }
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

/// Un code à six chiffres pour l'ajout par code (accepté dans Admin).
pub fn pair_code() -> String {
    let mut b = [0u8; 4];
    let _ = getrandom::getrandom(&mut b);
    format!("{:06}", u32::from_le_bytes(b) % 1_000_000)
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
