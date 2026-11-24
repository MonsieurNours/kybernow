//! Ce que l'app garde : les ordinateurs (adresse d'Admin, empreinte, compte),
//! leurs réglages, la dernière session, l'identité de ce téléviseur et les
//! réglages de l'app. Un fichier JSON dans le dossier de l'app (`Store::dir` :
//! `/media/developer/temp/kybernow` en mode développeur).
//!
//! Les mots de passe sont dans ce fichier, en clair : webOS n'a pas de
//! trousseau accessible aux apps natives. Le compte est propre à cet appareil
//! (accordé par le code à six chiffres) et se retire dans Admin. Le fichier
//! est lisible par l'utilisateur de l'app seul (droits 0600).
//!
//! Comptes KyberNow (docs/comptes.md, lot 3) : chaque compte connecté est
//! gardé entier dans ce même fichier (`Store::accounts`, la session
//! d'appareil comprise), comme les mots de passe des ordinateurs et des
//! prêts. La dernière liste de leurs ordinateurs (`Store::account_computers`,
//! rien de secret) l'accompagne, pour l'accueil dès le lancement.

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

    /// Transport fiable d'office : Bureau, comme `StreamSettings::for_profile`
    /// du cœur (l'interface se compile sans lui, pour les captures).
    pub fn reliable(self) -> bool {
        self == Profile::Bureau
    }
}

/// Réglages d'un ordinateur, comme Réglages › par ordinateur sur les autres apps.
/// Le profil donne le format, le débit et le transport ; en choisir un autre
/// à la main (`codec`, `mbps`, `transport`) rend le profil « Personnalisé ».
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HostSettings {
    pub profile: Profile,
    /// 0 : comme ce téléviseur ; sinon hauteur (2160, 1440, 1080, 720).
    pub resolution: u32,
    /// Images par seconde : 30, 60, 120 (0 : la fréquence de la dalle).
    pub fps: u32,
    /// `auto`, `hevc`, `h264`, `av1`.
    pub codec: String,
    /// 0 : celui du profil ; plafonné dans tous les cas par le réseau
    /// (`NetInfo::cap_mbps` : 80 Mbit/s en Ethernet à 100 Mbit/s, 95 sinon).
    pub mbps: u32,
    /// `auto` (stéréo), `stereo`, `dolby` (5.1 en E-AC-3), `surround` (5.1 en
    /// PCM), `computer` (le son reste sur l'ordinateur).
    pub audio: String,
    /// Toutes les manettes vues comme des Xbox 360 par l'ordinateur.
    pub xbox360: bool,
    /// L1 + R1 + Start + Select ouvrent le menu.
    pub combo: bool,
    /// Bouton Retour de la Magic Remote : ouvre le menu (sinon Échap).
    pub back_opens_menu: bool,
    /// « Ctrl agit comme ⌘ », face à une Station Mac seulement ; `None` : jamais changé, allumé
    /// (`keymap::control_as_command` du cœur).
    pub ctrl_command: Option<bool>,
    /// Sans effet : pas de presse-papiers sur webOS (la session le coupe).
    pub clipboard: bool,
    pub reconnect: bool,
    /// Transport de l'image : `""` celui du profil, `fast` ou `reliable`.
    pub transport: String,
    /// Copie de `transport == "reliable"` : le seul champ que lisent les
    /// versions 0.4.x (« Transport fiable » forcé).
    pub reliable: bool,
    /// Rappeler, dans le menu de session, de passer la télé en Optimiseur de jeu.
    pub game_mode_hint: bool,
    /// Le HDR pour cet ordinateur (puce de l'accueil, Réglages › Image, menu de session) : allumé la première fois
    /// (absent d'un fichier d'avant le HDR : allumé aussi). Il ne compte pas dans le profil. Sans effet sur un
    /// écran SDR.
    pub hdr: bool,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            profile: Profile::Jeu,
            resolution: 0,
            fps: 0,
            codec: "auto".into(),
            mbps: 0,
            audio: "auto".into(),
            xbox360: true,
            combo: true,
            back_opens_menu: true,
            ctrl_command: None,
            clipboard: true,
            reconnect: true,
            transport: String::new(),
            reliable: false,
            game_mode_hint: true,
            hdr: true,
        }
    }
}

impl HostSettings {
    /// Les réglages s'écartent du profil : « Personnalisé ».
    pub fn is_custom(&self) -> bool {
        self.codec != "auto" || self.mbps != 0 || !self.transport.is_empty()
    }

    /// Choisit un profil : format, débit et transport reviennent aux siens.
    pub fn apply(&mut self, p: Profile) {
        self.profile = p;
        self.codec = "auto".into();
        self.mbps = 0;
        self.set_transport("");
    }

    /// Transport fiable pour la session : le choix, sinon celui du profil.
    pub fn reliable_transport(&self) -> bool {
        match self.transport.as_str() {
            "reliable" => true,
            "fast" => false,
            _ => self.profile.reliable(),
        }
    }

    pub fn set_transport(&mut self, t: &str) {
        self.transport = t.into();
        self.reliable = t == "reliable";
    }
}

/// La dernière session (au moins 3 s) : latence médiane et 95e centile (ms),
/// durée (s), date (secondes Unix), définition, fréquence, format et débit
/// moyen. Les fichiers de la 0.4.x n'ont que `mode` (« 3840 × 2160 · HEVC »)
/// à la place de la définition et du format : les champs nouveaux sont
/// facultatifs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct LastSession {
    pub median: f64,
    pub p95: f64,
    pub seconds: u64,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub mbps: f64,
    pub at: u64,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    /// Fréquence de l'écran de la session (Hz).
    #[serde(default)]
    pub refresh: u32,
    /// `HEVC`, `AV1`, `H.264`.
    #[serde(default)]
    pub codec: String,
    /// Le chemin de la session : `direct`, `relay` (par le relais) ou `relay_tcp` (par le relais, repli TCP) ;
    /// vide : direct, ou une session d'avant le relais.
    #[serde(default)]
    pub route: String,
    /// L'image était en HDR : « HDR » après le format.
    #[serde(default)]
    pub hdr: bool,
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
    /// Adresse matérielle pour le réveil, écrite à la main (aucun écran ne
    /// la saisit au téléviseur ; lue si un fichier en porte une).
    #[serde(default)]
    pub mac: Option<String>,
    /// Les cartes réseau de l'ordinateur pour le réveil (Wake-on-LAN), telles
    /// qu'Admin les donne (`wake` d'`info`) : gardées au dernier sondage
    /// réussi qui en donne, pour le réveiller une fois éteint.
    #[serde(default)]
    pub wake: Vec<WakeTarget>,
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

impl Host {
    /// L'app sait réveiller cet ordinateur : des cartes données par Admin, ou
    /// une adresse MAC écrite à la main.
    pub fn can_wake(&self) -> bool {
        !self.wake.is_empty() || self.mac.as_deref().is_some_and(|m| !m.trim().is_empty())
    }

    /// L'adresse MAC montrée dans les réglages : celle de la première carte
    /// (Ethernet d'abord, dans l'ordre d'Admin), sinon celle écrite à la main.
    pub fn wake_mac(&self) -> Option<String> {
        self.wake.first().map(|t| t.mac.clone()).or_else(|| self.mac.clone().filter(|m| !m.trim().is_empty()))
    }
}

/// Une carte réseau de l'ordinateur à réveiller, au format JSON de
/// `kybernow_core::wake::WakeTarget` (le même dans `kybernow.json` que dans
/// la réponse d'Admin) ; l'interface se compile sans le cœur, pour les captures.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WakeTarget {
    pub mac: String,
    #[serde(default)]
    pub ipv4: Option<String>,
    #[serde(default)]
    pub broadcast: Option<String>,
    /// `ethernet` ou `wifi` (vide : inconnu).
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub armed: Option<bool>,
    #[serde(default)]
    pub name: String,
}

/// Réglages de l'app.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    /// `auto`, `fr`, `en`.
    pub language: String,
    /// Mises à jour : aussi les bêtas (sinon les versions stables seules).
    pub beta: bool,
    /// KyberNow Share : le prénom donné en rejoignant un partage, retenu pour la fois suivante (vide : le nom
    /// de ce téléviseur est proposé). Avec la qualité de l'image ci-dessous, la seule trace d'un partage sur
    /// l'appareil : un invité n'est jamais ajouté aux ordinateurs.
    pub share_name: String,
    /// KyberNow Share : la « Qualité de l'image » choisie en rejoignant un partage ou dans le menu de la session,
    /// retenue pour la fois suivante (`auto`, `host`, `2160p60`, `1080p60`, `720p30` ; vide ou inconnue : « Auto »).
    pub share_quality: String,
    /// KyberNow Share, prêt : l'avertissement de confiance est dit dans le panneau « Prêter l'ordinateur… » jusqu'au
    /// premier code créé, puis plus (« une fois », docs/link-ui.md).
    pub lend_warned: bool,
    /// HDR : les valeurs de l'écran du téléviseur réglées à l'œil (« Réglage du HDR ») ; `None` : automatiques
    /// (webOS ne les donne pas : `Luminance::default()`).
    pub hdr_luminance: Option<Luminance>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { language: "auto".into(), beta: false, share_name: String::new(), share_quality: String::new(), lend_warned: false, hdr_luminance: None }
    }
}

/// Les valeurs HDR d'un écran (docs/hdr.md, « Réglage du HDR »), en nits : luminosité maximale (petite zone), sur
/// tout l'écran, noir et blanc du bureau ; miroir de `HdrLuminance` du cœur.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Luminance {
    pub max: f32,
    pub full_frame: f32,
    pub min: f32,
    pub sdr_white: f32,
}

impl Default for Luminance {
    /// Par défaut : 1 000 nits, 1 000 sur tout l'écran, 0, blanc du bureau à 203 (référence BT.2408).
    fn default() -> Self {
        Self { max: 1000.0, full_frame: 1000.0, min: 0.0, sdr_white: 203.0 }
    }
}

/// Un ordinateur emprunté (KyberNow Share, prêt de l'ordinateur, docs/pret.md) : la Station d'un ami qui a créé un
/// compte d'ami pour ce téléviseur, gardée à part des ordinateurs appairés (`Store::borrowed`). Le mot de passe n'est
/// écrit que si « Retenir le mot de passe » est coché ; sinon l'app le garde le temps de son lancement.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct Borrowed {
    /// `lend:<uuid>` : jamais celui d'un ordinateur appairé.
    pub id: String,
    /// Le prénom de l'hôte et le nom de son ordinateur, tels qu'Admin les donne.
    pub host: String,
    pub computer: String,
    pub address: String,
    pub port: u16,
    pub fingerprint: Option<String>,
    /// Le relais de la Station et son identifiant chez lui (lien d'invitation) : l'ordinateur s'emprunte aussi hors de
    /// chez soi. `None` : ajouté sur le réseau, ou lien sans relais.
    pub relay: Option<String>,
    pub sid: Option<String>,
    pub login: String,
    pub password: String,
    pub remember: bool,
    /// Le HDR pour cet ordinateur emprunté (sa puce, « Emprunter ») : allumé d'office.
    #[serde(default = "yes")]
    pub hdr: bool,
}

fn yes() -> bool {
    true
}

impl Borrowed {
    /// « Ordinateur de Matthieu », “Matthieu's computer”.
    pub fn title(&self) -> String {
        borrowed_title(&self.host)
    }
}

/// « Ordinateur de {hôte} » (élidé : « Ordinateur d'Hugo »), “{host}'s computer”.
pub fn borrowed_title(host: &str) -> String {
    if crate::i18n::english() { format!("{host}'s computer") } else { format!("Ordinateur {}{host}", crate::i18n::de(host)) }
}

// MARK: - Comptes KyberNow (docs/comptes.md ; textes : docs/link-ui.md, « Comptes KyberNow »)

/// Un compte KyberNow connecté sur cet appareil : le JSON d'`account::Account` du cœur, recopié ici pour l'interface
/// seule (mêmes noms, mêmes valeurs ; le pilote passe de l'un à l'autre par serde). `token` est la session d'appareil :
/// gardée dans ce fichier (0600), jamais dans les journaux (`Debug` la masque).
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct KnAccount {
    /// Le relais, sous sa forme normale (`share.kybernow.app`, `relais.exemple.org:9443`, empreinte `#sha256:…` gardée).
    pub relay: String,
    pub token: String,
    pub account: KnUser,
    pub device: KnDevice,
}

impl std::fmt::Debug for KnAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnAccount").field("relay", &self.relay).field("token", &"…").field("account", &self.account).finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct KnUser {
    pub id: String,
    pub name: String,
    pub email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct KnDevice {
    pub id: String,
    pub name: String,
}

impl KnAccount {
    /// Ce qui distingue ce compte des autres (`Account::key` du cœur) : le relais et l'identifiant du compte.
    pub fn key(&self) -> String {
        format!("{}|{}", self.relay, self.account.id)
    }

    /// {compte} des textes : le nom, sinon l'e-mail.
    pub fn label(&self) -> String {
        let n = self.account.name.trim();
        if n.is_empty() { self.account.email.trim().to_string() } else { n.to_string() }
    }

    /// Le relais à montrer : `share.kybernow.app`, `relais.exemple.org:9443` (sans l'empreinte).
    pub fn relay_label(&self) -> String {
        relay_label(&self.relay)
    }

    /// « Gérer le compte » : la page du compte sur son relais (`account::console_url` du cœur).
    pub fn console(&self) -> String {
        format!("https://{}/compte", self.relay_label())
    }
}

/// Un relais tel qu'on le montre : sa forme normale sans l'empreinte épinglée (`RelayAddr::web_authority`).
pub fn relay_label(relay: &str) -> String {
    let r = relay.split('#').next().unwrap_or_default().trim();
    let r = r.strip_suffix(":443").unwrap_or(r);
    if r.is_empty() { DEFAULT_RELAY.to_string() } else { r.to_string() }
}

/// Le relais public, proposé d'abord (« Ajouter un compte »).
pub const DEFAULT_RELAY: &str = "share.kybernow.app";

/// Un ordinateur d'un compte (`account::AccountComputer` du cœur, même JSON). La dernière liste est gardée (rien de
/// secret) : l'accueil la montre dès le lancement, et l'empreinte d'Admin, que le relais ne donne que pour une Station
/// connectée, y reste d'une liste à l'autre (`remember_fingerprints`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct KnComputer {
    /// Le compte qui le donne (`KnAccount::key`).
    pub account: String,
    pub relay: String,
    /// L'identifiant de la Station chez le relais.
    pub station: String,
    pub name: String,
    pub rights: Vec<String>,
    pub owner: KnOwner,
    pub mine: bool,
    pub online: bool,
    pub admin_fp: Option<String>,
    pub attached_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct KnOwner {
    pub kind: String,
    pub id: String,
    pub name: String,
}

/// Un ordinateur de l'accueil venu d'un compte (et non appairé) : son identifiant chez l'app (`Host::id`), le compte
/// qui le donne et sa Station chez le relais. L'ordinateur lui-même est dans `Store::hosts`, sans compte ni mot de
/// passe (`user` et `password` vides), avec l'empreinte d'Admin du compte, son relais (`Host::relay`, pour la présence
/// « Hors de chez vous ») et, quand la découverte le voit sur le réseau, son adresse ; ses réglages et sa dernière
/// session restent comme ceux d'un ordinateur appairé.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct AccountHost {
    pub id: String,
    pub account: String,
    pub relay: String,
    pub station: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Store {
    /// Identifiant de ce téléviseur auprès d'Admin, tiré une fois.
    pub device_id: String,
    pub hosts: Vec<Host>,
    /// KyberNow Share : les ordinateurs empruntés (comptes d'amis), à part des ordinateurs appairés.
    pub borrowed: Vec<Borrowed>,
    /// Dernier ordinateur choisi sur l'accueil (appairé, emprunté : `lend:…`, ou console : `ps:<host_id>`).
    pub selected: Option<String>,
    /// Consoles PlayStation : comptes PSN, consoles associées et leurs réglages (docs/playstation.md).
    pub playstation: PsStore,
    /// Comptes KyberNow : les comptes connectés (entiers, session d'appareil comprise), la dernière liste de leurs
    /// ordinateurs, et ceux de l'accueil qui en viennent (`AccountHost`).
    pub accounts: Vec<KnAccount>,
    pub account_computers: Vec<KnComputer>,
    pub account_hosts: Vec<AccountHost>,
    pub app: AppSettings,
    #[serde(skip)]
    path: Option<PathBuf>,
}

impl Store {
    /// Le dossier de l'app, choisi une fois : le premier où l'on peut
    /// écrire parmi `$HOME/conf`, `/media/developer/temp/kybernow` et
    /// `/tmp/kybernow`. En mode développeur, `HOME` vaut `/media/developer`,
    /// qui n'est pas ouvert à l'utilisateur de l'app ; `temp` l'est, survit au
    /// redémarrage et se lit en ssh (le `/tmp` de l'app est vidé au redémarrage
    /// et invisible en ssh). Ce qui était dans `/tmp/kybernow` y est recopié.
    pub fn dir() -> PathBuf {
        static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        DIR.get_or_init(|| {
            let tmp = PathBuf::from("/tmp/kybernow");
            let mut candidates = Vec::new();
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(PathBuf::from(home).join("conf"));
            }
            candidates.push(PathBuf::from("/media/developer/temp/kybernow"));
            candidates.push(tmp.clone());
            let d = candidates.into_iter().find(|d| writable(d)).unwrap_or_else(|| tmp.clone());
            if d != tmp && !d.join("kybernow.json").exists() && tmp.join("kybernow.json").exists() {
                let _ = std::fs::copy(tmp.join("kybernow.json"), d.join("kybernow.json"));
            }
            d
        })
        .clone()
    }

    pub fn load() -> Self {
        let path = Self::dir().join("kybernow.json");
        let mut s: Store = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if s.device_id.is_empty() {
            s.device_id = new_id();
        }
        // Fichier de la 0.4.x : « Transport fiable » forcé devient le choix
        // « Fiable », sauf en Bureau, où il l'est déjà.
        for h in &mut s.hosts {
            if h.settings.reliable && h.settings.transport.is_empty() {
                let t = if h.settings.profile.reliable() { "" } else { "reliable" };
                h.settings.set_transport(t);
            }
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
        // Lisible par l'utilisateur de l'app seul : le fichier porte les mots de passe et les sessions des comptes.
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let written = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp).and_then(|mut f| {
            // Un fichier temporaire laissé par une version d'avant garde ses droits : ils sont resserrés.
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            f.write_all(&bytes)
        });
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
        let i = self.hosts.iter().position(|h| h.address == host.address && h.port == host.port);
        self.put_at(i, host)
    }

    /// Invitations (docs/invitations.md) : ajoute l'ordinateur d'une invitation. Le même ordinateur déjà là (même
    /// empreinte d'Admin, sinon même adresse) prend le nouvel accès (adresse, compte, relais) au lieu d'être ajouté
    /// deux fois.
    pub fn put_invited(&mut self, host: Host) -> String {
        let same_fp = host.fingerprint.as_deref().filter(|f| !f.is_empty()).and_then(|fp| self.hosts.iter().position(|h| h.fingerprint.as_deref() == Some(fp)));
        let i = same_fp.or_else(|| self.hosts.iter().position(|h| h.address == host.address && h.port == host.port));
        self.put_at(i, host)
    }

    /// Met à jour l'ordinateur `i` avec `host`, ou ajoute `host`.
    fn put_at(&mut self, i: Option<usize>, host: Host) -> String {
        if let Some(h) = i.and_then(|i| self.hosts.get_mut(i)) {
            let id = h.id.clone();
            let settings = h.settings.clone();
            let last = h.last.clone();
            let wake = if host.wake.is_empty() { h.wake.clone() } else { host.wake.clone() };
            let relay = host.relay.clone().or_else(|| h.relay.clone());
            *h = Host { id: id.clone(), settings, last, wake, relay, ..host };
            // Un ordinateur d'un compte appairé ici (par code, par une invitation) devient un ordinateur appairé.
            self.account_hosts.retain(|a| a.id != id);
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
            *old = Borrowed { id: id.clone(), hdr: old.hdr, ..b };
            return id;
        }
        let id = b.id.clone();
        self.borrowed.push(b);
        id
    }

    /// L'ordinateur `id` vient d'un compte (pas appairé sur cet appareil).
    pub fn account_host(&self, id: &str) -> Option<&AccountHost> {
        self.account_hosts.iter().find(|a| a.id == id)
    }

    pub fn account(&self, key: &str) -> Option<&KnAccount> {
        self.accounts.iter().find(|a| a.key() == key)
    }

    /// Garde un compte connecté : celui de même clé (même relais, même compte) est remplacé (nouvelle session
    /// d'appareil), sans doublon.
    pub fn put_account(&mut self, a: KnAccount) {
        match self.accounts.iter_mut().find(|o| o.key() == a.key()) {
            Some(o) => *o = a,
            None => self.accounts.push(a),
        }
    }

    /// Oublie un compte (« Se déconnecter », session d'appareil fermée) : ses ordinateurs quittent l'accueil, ceux
    /// qui sont appairés restent. Rend les identifiants des ordinateurs retirés.
    pub fn forget_account(&mut self, key: &str) -> Vec<String> {
        self.accounts.retain(|a| a.key() != key);
        self.account_computers.retain(|c| c.account != key);
        let gone: Vec<String> = self.account_hosts.iter().filter(|a| a.account == key).map(|a| a.id.clone()).collect();
        for id in &gone {
            self.remove(id);
        }
        gone
    }

    /// Les ordinateurs des comptes à l'accueil : `added` (de `account::merge` du cœur : ceux des comptes qui ne sont
    /// pas déjà appairés ici, un seul exemplaire d'un compte à l'autre). Un ordinateur déjà là (même relais, même
    /// Station) garde son identifiant, ses réglages et sa dernière session ; son nom et son empreinte suivent la liste ;
    /// ceux qui n'y sont plus quittent l'accueil. Rend les identifiants retirés.
    pub fn sync_account_hosts(&mut self, added: &[KnComputer]) -> Vec<String> {
        let mut keep = Vec::new();
        for c in added {
            let relay = Some(HostRelay { server: c.relay.clone(), id: c.station.clone(), remote_access: true });
            let id = match self.account_hosts.iter_mut().find(|a| a.relay == c.relay && a.station == c.station) {
                Some(a) => {
                    a.account = c.account.clone();
                    a.id.clone()
                }
                None => {
                    let id = new_id();
                    self.account_hosts.push(AccountHost { id: id.clone(), account: c.account.clone(), relay: c.relay.clone(), station: c.station.clone() });
                    id
                }
            };
            match self.host_mut(&id) {
                Some(h) => {
                    if !c.name.trim().is_empty() {
                        h.name = c.name.clone();
                    }
                    if c.admin_fp.is_some() {
                        h.fingerprint = c.admin_fp.clone();
                    }
                    h.relay = relay;
                }
                None => self.hosts.push(Host {
                    id: id.clone(),
                    name: c.name.clone(),
                    address: String::new(),
                    port: 0,
                    fingerprint: c.admin_fp.clone(),
                    user: String::new(),
                    password: String::new(),
                    os: None,
                    gpu: None,
                    av1: false,
                    mac: None,
                    wake: Vec::new(),
                    relay,
                    settings: HostSettings::default(),
                    last: None,
                }),
            }
            keep.push(id);
        }
        let gone: Vec<String> = self.account_hosts.iter().filter(|a| !keep.contains(&a.id)).map(|a| a.id.clone()).collect();
        for id in &gone {
            self.remove(id);
        }
        gone
    }

    /// La découverte : un ordinateur d'un compte annoncé sur le réseau avec la même empreinte d'Admin y prend son
    /// adresse (la session passe alors par le réseau, puis par le relais s'il ne répond pas) ; absent, il la perd (par
    /// le relais seulement). Rend vrai si quelque chose a changé.
    pub fn locate_account_hosts(&mut self, found: &[(String, u16, Option<String>)]) -> bool {
        let norm = |s: &str| s.trim().to_ascii_lowercase();
        let ids: Vec<String> = self.account_hosts.iter().map(|a| a.id.clone()).collect();
        let mut changed = false;
        for id in ids {
            let Some(h) = self.host_mut(&id) else { continue };
            let seen = h.fingerprint.as_deref().filter(|f| !f.trim().is_empty()).and_then(|fp| found.iter().find(|(_, _, f)| f.as_deref().is_some_and(|f| norm(f) == norm(fp))));
            let (address, port) = seen.map(|(a, p, _)| (a.clone(), *p)).unwrap_or_default();
            if h.address != address || h.port != port {
                h.address = address;
                h.port = port;
                changed = true;
            }
        }
        changed
    }

    pub fn remove(&mut self, id: &str) {
        self.account_hosts.retain(|a| a.id != id);
        self.hosts.retain(|h| h.id != id);
        self.borrowed.retain(|b| b.id != id);
        self.playstation.consoles.retain(|c| c.key() != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
    }

    /// Aperçu d'Admin gardé pour l'accueil (un fichier par ordinateur).
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

// MARK: - Consoles PlayStation (docs/playstation.md, textes : docs/link-ui.md, « Consoles PlayStation »)

/// Les consoles PlayStation de ce téléviseur : les comptes PSN et les consoles associées, gardés dans
/// `kybernow.json` comme les comptes des ordinateurs (webOS ne donne pas de trousseau à une app en mode
/// développeur ; le dossier de l'app n'est lisible qu'en ssh sur le téléviseur).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PsStore {
    pub accounts: Vec<PsAccount>,
    pub consoles: Vec<PsConsole>,
}

/// Un compte PlayStation Network : son identifiant (base64, `PsnId` du cœur) et son nom (le pseudo reçu d'un
/// autre appareil, ou le nom saisi à la main ; `None` : « Compte PlayStation »).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct PsAccount {
    /// Un UUID propre à ce téléviseur.
    pub id: String,
    pub name: Option<String>,
    pub psn_id: String,
}

impl PsAccount {
    /// Le nom montré : le pseudo, sinon « Compte PlayStation ».
    pub fn label(&self) -> String {
        match self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None => crate::i18n::tr("Compte PlayStation", "PlayStation account").to_string(),
        }
    }
}

/// Une console associée (au moins un compte) ou retenue (adresse saisie, reçue d'un autre appareil).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PsConsole {
    /// Le `host_id` de la découverte, en majuscules (le `mac` de l'association).
    pub host_id: String,
    pub name: String,
    /// « PS5 » ou « PS4 ».
    pub model: String,
    /// Pour associer de nouveau (`TargetName` du cœur, ex. `ps5_v1`) ; vide : lu à la découverte.
    pub target: String,
    /// Adresses retenues (la dernière trouvée d'abord) : cherchées en direct en plus du réseau local.
    pub addresses: Vec<String>,
    /// Les Stations (identifiants `Host::id` de ce téléviseur) qui voient la console (`GET /api/consoles`) : de quoi
    /// la joindre hors de chez soi par le relais.
    pub stations: Vec<String>,
    /// Les associations, par compte (`PsAccount::id`) : le `Registered` du cœur, tel quel (JSON stable).
    pub pairings: std::collections::BTreeMap<String, serde_json::Value>,
    /// Le compte choisi à l'accueil (la puce « {pseudo} »), parmi ceux associés.
    pub account: Option<String>,
    /// La puce « HDR » de l'accueil (PS5, écran HDR) : allumée la première fois.
    #[serde(default = "yes")]
    pub hdr: bool,
    pub settings: PsSettings,
}

impl PsConsole {
    /// La clé de la console dans `Store::selected`.
    pub fn key(&self) -> String {
        format!("ps:{}", self.host_id)
    }

    pub fn ps5(&self) -> bool {
        !self.model.eq_ignore_ascii_case("PS4")
    }

    /// Les comptes associés, dans l'ordre des comptes du téléviseur.
    pub fn paired_accounts<'a>(&self, accounts: &'a [PsAccount]) -> Vec<&'a PsAccount> {
        accounts.iter().filter(|a| self.pairings.contains_key(&a.id)).collect()
    }

    /// Le compte qui ouvre la partie : celui choisi, sinon le premier associé.
    pub fn play_account<'a>(&self, accounts: &'a [PsAccount]) -> Option<&'a PsAccount> {
        let paired = self.paired_accounts(accounts);
        self.account.as_ref().and_then(|id| paired.iter().find(|a| &a.id == id).copied()).or_else(|| paired.first().copied())
    }
}

/// Réglages d'une console : le JSON de `PlaySettings` du cœur (`apps/core/src/playstation/settings.rs`), recopié
/// pour l'interface seule (sans le cœur) ; mêmes noms, mêmes valeurs par défaut. Le plein écran et
/// l'agrandissement (Apple) n'ont pas de sens sur le téléviseur : gardés tels quels pour les autres appareils.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PsSettings {
    /// `auto`, `p1080`, `p720`, `p540`, `p360`.
    #[serde(default = "auto")]
    pub resolution: String,
    #[serde(default = "sixty")]
    pub fps: u32,
    /// kbit/s ; absent : Auto (suit la définition).
    #[serde(default)]
    pub bitrate_kbps: Option<u32>,
    /// `auto`, `h264`, `h265`.
    #[serde(default = "auto")]
    pub codec: String,
    #[serde(default)]
    pub hdr: bool,
    #[serde(default = "yes")]
    pub fullscreen: bool,
    #[serde(default = "yes")]
    pub upscale: bool,
    #[serde(default = "sharpness")]
    pub sharpness: u8,
    #[serde(default = "hundred")]
    pub rumble: u8,
    #[serde(default = "hundred")]
    pub triggers: u8,
}

fn auto() -> String {
    "auto".into()
}
fn sixty() -> u32 {
    60
}
fn sharpness() -> u8 {
    40
}
fn hundred() -> u8 {
    100
}

impl Default for PsSettings {
    fn default() -> Self {
        Self { resolution: auto(), fps: 60, bitrate_kbps: None, codec: auto(), hdr: false, fullscreen: true, upscale: true, sharpness: 40, rumble: 100, triggers: 100 }
    }
}

impl PsSettings {
    /// « Personnalisé » à l'accueil : définition, images par seconde, débit ou format s'écartent d'Auto (règle du cœur,
    /// `PlaySettings::is_custom`).
    pub fn is_custom(&self) -> bool {
        self.resolution != "auto" || self.fps != 60 || self.bitrate_kbps.is_some() || self.codec != "auto"
    }
}

impl PsStore {
    pub fn console(&self, host_id: &str) -> Option<&PsConsole> {
        self.consoles.iter().find(|c| c.host_id.eq_ignore_ascii_case(host_id))
    }

    pub fn console_mut(&mut self, host_id: &str) -> Option<&mut PsConsole> {
        self.consoles.iter_mut().find(|c| c.host_id.eq_ignore_ascii_case(host_id))
    }

    pub fn account(&self, id: &str) -> Option<&PsAccount> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// Ajoute un compte, ou met à jour celui de même identifiant PSN (son nom, s'il en a un) : rend son `id` et s'il
    /// était déjà là (« Ce compte est déjà là. »).
    pub fn put_account(&mut self, psn_id: &str, name: Option<String>) -> (String, bool) {
        let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        if let Some(a) = self.accounts.iter_mut().find(|a| a.psn_id == psn_id) {
            if name.is_some() {
                a.name = name;
            }
            return (a.id.clone(), true);
        }
        let id = new_id();
        self.accounts.push(PsAccount { id: id.clone(), name, psn_id: psn_id.to_string() });
        (id, false)
    }

    /// Retire un compte et ses associations ; une console sans plus aucune association est retirée aussi (sauf une
    /// console retenue par son adresse, qu'on peut associer de nouveau). Rend le nombre de consoles touchées.
    pub fn remove_account(&mut self, id: &str) -> usize {
        self.accounts.retain(|a| a.id != id);
        let mut n = 0;
        for c in &mut self.consoles {
            if c.pairings.remove(id).is_some() {
                n += 1;
            }
            if c.account.as_deref() == Some(id) {
                c.account = None;
            }
        }
        self.consoles.retain(|c| !c.pairings.is_empty() || !c.addresses.is_empty());
        n
    }

    /// Les consoles associées avec ce compte.
    pub fn consoles_of(&self, account: &str) -> usize {
        self.consoles.iter().filter(|c| c.pairings.contains_key(account)).count()
    }

    /// Ajoute une console ou met à jour celle de même `host_id` (nom, modèle, cible, adresse en tête).
    pub fn put_console(&mut self, host_id: &str, name: &str, model: &str, target: &str, address: Option<&str>) -> &mut PsConsole {
        let host_id = host_id.to_ascii_uppercase();
        let i = match self.consoles.iter().position(|c| c.host_id.eq_ignore_ascii_case(&host_id)) {
            Some(i) => i,
            None => {
                self.consoles.push(PsConsole { host_id: host_id.clone(), hdr: true, ..Default::default() });
                self.consoles.len() - 1
            }
        };
        let c = &mut self.consoles[i];
        if !name.is_empty() {
            c.name = name.to_string();
        }
        if !model.is_empty() {
            c.model = model.to_ascii_uppercase();
        }
        if !target.is_empty() {
            c.target = target.to_string();
        }
        if let Some(a) = address.filter(|a| !a.is_empty() && *a != "127.0.0.1") {
            c.addresses.retain(|x| x != a);
            c.addresses.insert(0, a.to_string());
            c.addresses.truncate(4);
        }
        c
    }
}
