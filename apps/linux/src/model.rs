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
//!
//! Comptes KyberNow (docs/comptes.md, lot 3) : chaque compte connecté est
//! gardé entier dans ce même fichier (`Store::accounts`, la session
//! d'appareil comprise, droits 0600), comme les mots de passe des
//! ordinateurs et des prêts : il n'y a pas de Secret Service en mode Jeu de
//! SteamOS ni sur KyberNow Link OS. La dernière liste de leurs ordinateurs
//! (`Store::account_computers`, rien de secret) l'accompagne, pour l'accueil
//! dès le lancement.

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
    /// Les réglages choisis par le test des conditions (docs/profil-auto.md), gardés pour cet ordinateur
    /// (`HostSettings::auto`).
    Auto,
}

impl Profile {
    pub const ALL: [Profile; 4] = [Profile::Jeu, Profile::Bureau, Profile::Max, Profile::Auto];

    pub fn id(self) -> &'static str {
        match self {
            Profile::Jeu => "jeu",
            Profile::Bureau => "bureau",
            Profile::Max => "max",
            Profile::Auto => "auto",
        }
    }
}

/// Ce que l'interface lit du résultat gardé du test (`AutoResult` du cœur, gardé tel quel en JSON) : la date, le
/// réseau du test et les réglages choisis. Le pilote relit le JSON entier avec le cœur (`device/autotest.rs`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct AutoView {
    /// Date du test (secondes depuis 1970).
    pub at: u64,
    /// Le réseau du test (« Ethernet », le nom du Wi-Fi, `relay` par le relais) ; `None` : inconnu.
    pub network: Option<String>,
    /// `hevc`, `h264`, `av1`, `pyrowave`.
    pub codec: String,
    pub yuv444: bool,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    pub reliable: bool,
}

impl AutoView {
    pub fn of(v: &serde_json::Value) -> Option<Self> {
        serde_json::from_value::<Self>(v.clone()).ok().filter(|a| a.at > 0 && !a.codec.is_empty())
    }

    /// Le test a-t-il été fait sur ce réseau ? (`AutoResult::same_network` du cœur : le relais s'appelle `relay`.)
    pub fn same_network(&self, network: Option<&str>) -> bool {
        self.network.as_deref() == network
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
    /// Mode immersif (docs/link-ui.md) : `fullscreen` (le clavier entier, Alt+Tab et la touche ⊞ compris, va à
    /// l'ordinateur en plein écran), `always` (aussi dans une fenêtre), `never`.
    pub immersive: String,
    /// « Stylet : Pression / Souris » (docs/link-ui.md, « Stylet ») : `pressure` (le vrai stylet, par défaut ; absent
    /// d'un fichier d'avant : pression aussi) ou `mouse` (le stylet en souris absolue), les clés du cœur
    /// (`PenMode::key`). Synchronisé (`pen`).
    pub pen: String,
    pub clipboard: bool,
    /// « Définition de la caméra » (docs/webcam.md) : 1080p, sinon 720p ; hors profil.
    pub camera_1080: bool,
    /// L'ordinateur a déjà proposé la caméra : « Définition de la caméra » apparaît dans ses réglages.
    pub camera_seen: bool,
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
    /// « Débit adaptatif » (docs/profil-auto.md) : allumé par défaut (absent d'un fichier d'avant : allumé), pour un
    /// profil comme pour un débit choisi ; envoyé dans les réglages du flux (`adaptive`). Ne rend jamais le profil
    /// « Personnalisé ».
    pub adaptive: bool,
    /// Le résultat du test du profil Auto pour cet ordinateur (`AutoResult` du cœur, en JSON), gardé à
    /// « Appliquer » ; `None` : jamais appliqué (choisir Auto ouvre le test).
    pub auto: Option<serde_json::Value>,
    /// Le plafond de l'encodeur de cet ordinateur (Mbit/s par écran, docs/profil-auto.md, « L'encodeur de
    /// l'ordinateur ») : le débit le plus haut qu'il encode, trouvé par le test (« Appliquer » d'un nouveau résultat :
    /// `AutoResult::next_encoder_max`) ou par une session qui a dû redescendre (`Notice::EncoderCeiling`). Envoyé dans
    /// les réglages du flux (`encoder_max`) quel que soit le profil ; `None` : aucun connu. Hors du profil, propre à
    /// cet appareil : jamais synchronisé.
    pub encoder_max: Option<u32>,
    /// « Micro » (docs/micro.md) : le micro de cet appareil envoyé à cet ordinateur pendant la session. Coupé la
    /// première fois ; la barre, la tuile du menu et la rubrique Son le changent, la session suivante le reprend.
    /// Propre à cet appareil : jamais synchronisé.
    pub mic: bool,
    /// « Garder la dernière image » (docs/link-ui.md, rubrique Image) : l'accueil montre la dernière image de la
    /// session de cet ordinateur. Propre à cet appareil : jamais synchronisé.
    pub keep_last_image: bool,
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
            immersive: "fullscreen".into(),
            pen: crate::pen::PRESSURE.into(),
            clipboard: true,
            camera_1080: false,
            camera_seen: false,
            reconnect: true,
            reliable: false,
            battery_light: true,
            virtual_display: true,
            yuv444: false,
            hdr: true,
            adaptive: true,
            auto: None,
            encoder_max: None,
            mic: false,
            keep_last_image: true,
        }
    }
}

impl HostSettings {
    /// Les réglages s'écartent du profil (« Personnalisé ») : format ou débit
    /// choisis, 4:4:4 en Jeu (Bureau et Max le prennent quand il est permis),
    /// transport fiable hors Bureau (Bureau l'est déjà). Auto : le 4:4:4 et le
    /// transport sont ceux du test ; « Débit adaptatif » n'y compte jamais.
    pub fn custom(&self) -> bool {
        if self.profile == Profile::Auto {
            let a = self.auto_view();
            return self.codec != "auto" || self.mbps != 0 || a.is_some_and(|a| self.yuv444 != a.yuv444 || self.reliable != a.reliable);
        }
        self.codec != "auto" || self.mbps != 0 || (self.yuv444 && self.profile == Profile::Jeu) || (self.reliable && self.profile != Profile::Bureau)
    }

    /// Le résultat gardé du test, tel que l'interface le lit.
    pub fn auto_view(&self) -> Option<AutoView> {
        self.auto.as_ref().and_then(AutoView::of)
    }

    /// Choisit un profil : ce qui s'en écartait (format, débit, 4:4:4 en Jeu, transport fiable hors Bureau) revient
    /// au profil. Choisi ici, ou reçu d'un autre appareil (réglages synchronisés, `crate::synced`). Auto prend le
    /// 4:4:4 et le transport du test gardé (la définition et la fréquence, propres à l'écran : `App::apply_auto`).
    pub fn pick_profile(&mut self, p: Profile) {
        self.profile = p;
        self.codec = "auto".into();
        self.mbps = 0;
        if p == Profile::Auto {
            if let Some(a) = self.auto_view() {
                self.yuv444 = a.yuv444;
                self.reliable = a.reliable;
            }
            return;
        }
        if p == Profile::Jeu {
            self.yuv444 = false;
        }
        if p != Profile::Bureau {
            self.reliable = false;
        }
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
    /// La Station propose PyroWave (`pyrowave` de `GET /api/stream/info`, docs/pyrowave.md) ; absent : non.
    #[serde(default)]
    pub pyrowave: bool,
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
    /// « Garder la dernière image » d'avant la 0.11.8, pour toute l'app : lu une fois et repris par chaque ordinateur
    /// (`HostSettings::keep_last_image`, `Store::adopt_legacy_keep_image`), jamais réécrit.
    #[serde(rename = "keep_last_image", skip_serializing)]
    pub legacy_keep_last_image: Option<bool>,
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
    /// KyberNow Link OS : le pays (code ISO, vide : pas encore choisi) et son fuseau (docs/link-ui.md, « KyberNow
    /// Link OS : langue, pays et fuseau ») ; le Wi-Fi, l'heure et le clavier le suivent (`kybernow-os-ctl locale`).
    pub os_country: String,
    pub os_timezone: String,
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
    /// Comptes KyberNow : « Synchroniser les réglages des ordinateurs » (Réglages de l'app › Comptes), allumé par
    /// défaut, gardé sur cet appareil seulement (docs/comptes.md, « Réglages synchronisés »).
    pub sync_settings: bool,
    /// La version de l'app au dernier lancement (vide : jamais lancée, ou avant la 0.11.3) : au premier lancement
    /// d'une nouvelle version, l'avis « … a aussi une mise à jour » (docs/link-ui.md, « Mise à jour des Stations »).
    pub last_launched: String,
    /// Accueil et compte KyberNow (docs/link-ui.md, « Accueil et compte KyberNow ») : l'accueil vu, la carte du compte
    /// fermée, le bandeau après un appairage et le moment « Hors de chez vous ? » déjà montrés (`crate::welcome`).
    pub welcome: crate::welcome::WelcomeState,
    /// Le micro choisi (Réglages de l'app › « Micro ») : son nom chez SDL ; vide : celui du système. Gardé quand il
    /// est débranché (celui du système le remplace jusqu'à ce qu'on le rebranche).
    pub mic_device: String,
    /// « Réduire l'écho et le bruit » (Réglages de l'app) : allumé par défaut.
    pub mic_echo: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { language: "auto".into(), beta: false, interface: "auto".into(), legacy_keep_last_image: None, os_setup_done: false, os_cec: true, os_auto_update: true, os_hdmi: "auto".into(), os_country: String::new(), os_timezone: String::new(), share_name: String::new(), share_quality: String::new(), lend_warned: false, sync_settings: true, last_launched: String::new(), welcome: Default::default(), mic_device: String::new(), mic_echo: true }
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
    /// « Micro » pour cet ordinateur emprunté : coupé la première fois, retenu comme celui d'un hôte.
    pub mic: bool,
    /// « Stylet : Souris » pour cet ordinateur emprunté (« Pression » la première fois), retenu comme celui d'un hôte.
    pub pen_mouse: bool,
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

impl PsSettings {
    /// « Personnalisé » : définition, images par seconde, débit ou format s'écartent d'Auto (`PlaySettings::is_custom`).
    pub fn custom(&self) -> bool {
        self.resolution != "auto" || self.fps != 60 || self.bitrate_kbps.is_some() || self.codec != "auto"
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

/// Un relais tel qu'on le montre (`kybernow_rules::relay::label` : sans l'empreinte épinglée ni le port 443) ; aucun :
/// le relais public.
pub fn relay_label(relay: &str) -> String {
    if relay.split('#').next().unwrap_or_default().trim().is_empty() { DEFAULT_RELAY.to_string() } else { kybernow_rules::relay::label(relay) }
}

/// Le relais public, proposé d'abord (« Ajouter un compte »).
pub use kybernow_rules::relay::DEFAULT_RELAY;

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

impl KnComputer {
    /// Droit « Regarder » (`view`) : miroir d'`AccountComputer::can_watch` du cœur.
    pub fn can_watch(&self) -> bool {
        self.rights.iter().any(|r| r == "view")
    }

    /// « Regarder » sans « Ouvrir le bureau » (`view` sans `session`) : la carte propose « Regarder » à la place de
    /// « Se connecter » (miroir d'`AccountComputer::watch_only`).
    pub fn watch_only(&self) -> bool {
        self.can_watch() && !self.rights.iter().any(|r| r == "session")
    }
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
    /// Comptes KyberNow : les comptes connectés (entiers, session d'appareil comprise), la dernière liste de leurs
    /// ordinateurs, et ceux de l'accueil qui en viennent (`AccountHost`).
    pub accounts: Vec<KnAccount>,
    pub account_computers: Vec<KnComputer>,
    pub account_hosts: Vec<AccountHost>,
    /// Réglages synchronisés (`crate::synced`) : la mémoire de chaque compte, le JSON d'`account::AccountSync` du cœur
    /// (ce que le relais garde pour ses ordinateurs, ce qui est déjà appliqué ici, ce qu'il n'a pas encore reçu).
    pub account_sync: Vec<crate::synced::AccountMemory>,
    /// La mémoire d'avant la 0.11.8, lue une fois et reprise dans `account_sync` (`adopt_legacy_sync`), jamais réécrite.
    #[serde(rename = "synced", skip_serializing)]
    pub legacy_synced: Vec<crate::synced::LegacySynced>,
    #[serde(rename = "sync_pending", skip_serializing)]
    pub legacy_sync_pending: Vec<crate::synced::LegacyPending>,
    /// Mise à jour des Stations (docs/link-ui.md) : pour chaque ordinateur (`Host::id`), « Plus tard » et la dernière
    /// vérification demandée. Gardé sur cet appareil seulement (jamais dans les réglages synchronisés).
    pub station_offers: std::collections::HashMap<String, StationMemory>,
    pub app: AppSettings,
    #[serde(skip)]
    path: Option<PathBuf>,
    /// Le fichier trouvé illisible au lancement, mis de côté (`kybernow.json.illisible-<date>`) : l'app le dit.
    #[serde(skip)]
    pub set_aside: Option<PathBuf>,
}

/// Ce que l'app garde d'un ordinateur pour la mise à jour de sa Station : le même JSON que `station_offer::Memory` du
/// cœur, que la couche appareil relit pour le calcul (l'interface seule n'a pas le cœur).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct StationMemory {
    /// « Plus tard » : la version remise à plus tard, et quand (secondes Unix).
    pub snoozed_version: Option<String>,
    pub snoozed_at: u64,
    /// Dernière vérification demandée à la Station : la version de l'app d'alors, et quand.
    pub checked_app_version: Option<String>,
    pub checked_at: u64,
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
        Self::load_from(Self::dir().join("kybernow.json"), now_secs())
    }

    /// Lit `path`. Un fichier illisible (JSON abîmé, coupure pendant une écriture d'une version d'avant, disque
    /// plein) n'est jamais réécrit vide : il est mis de côté sous `kybernow.json.illisible-<date>` et l'app le dit
    /// (`set_aside`) ; s'il ne peut pas être déplacé, rien n'est écrit pendant cette session (audit, comme WEBOS-01).
    pub fn load_from(path: PathBuf, now: u64) -> Self {
        let mut aside = None;
        let mut writable = true;
        let mut s: Store = match std::fs::read(&path) {
            Ok(b) => match serde_json::from_slice(&b) {
                Ok(s) => s,
                Err(e) => {
                    let to = path.with_file_name(format!("{}.illisible-{}", path.file_name().and_then(|n| n.to_str()).unwrap_or("kybernow.json"), utc_stamp(now)));
                    match std::fs::rename(&path, &to) {
                        Ok(()) => {
                            log::error!("{} illisible ({e}) : mis de côté sous {}", path.display(), to.display());
                            aside = Some(to);
                        }
                        Err(r) => {
                            log::error!("{} illisible ({e}) et impossible à mettre de côté ({r}) : rien ne sera écrit", path.display());
                            writable = false;
                        }
                    }
                    Store::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Store::default(),
            Err(e) => {
                // Présent mais pas lisible (droits) : le réécrire effacerait ce qu'il contient.
                log::error!("{} : {e} ; rien ne sera écrit", path.display());
                writable = false;
                Store::default()
            }
        };
        if s.device_id.is_empty() {
            s.device_id = new_id();
        }
        s.adopt_legacy_sync();
        s.adopt_legacy_keep_image();
        s.set_aside = aside;
        if writable {
            s.path = Some(path);
            s.save();
        }
        s
    }

    /// « Garder la dernière image » coupé pour toute l'app avant la 0.11.8 : coupé pour chaque ordinateur.
    pub fn adopt_legacy_keep_image(&mut self) {
        if self.app.legacy_keep_last_image.take() == Some(false) {
            for h in &mut self.hosts {
                h.settings.keep_last_image = false;
            }
        }
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
        let i = self.hosts.iter().position(|h| h.address == host.address && h.port == host.port);
        self.put_at(i, host)
    }

    /// Invitations (docs/invitations.md) : ajoute l'ordinateur d'une invitation. Le même ordinateur déjà là (même
    /// empreinte d'Admin, sinon même adresse) prend le nouvel accès (adresse, compte, relais) au lieu d'être ajouté
    /// deux fois.
    pub fn put_invited(&mut self, host: Host) -> String {
        let same_fp = self.hosts.iter().position(|h| kybernow_rules::fingerprint::same_opt(h.fingerprint.as_deref(), host.fingerprint.as_deref()));
        let i = same_fp.or_else(|| self.hosts.iter().position(|h| h.address == host.address && h.port == host.port));
        self.put_at(i, host)
    }

    /// Met à jour l'ordinateur `i` avec `host`, ou ajoute `host`.
    fn put_at(&mut self, i: Option<usize>, host: Host) -> String {
        if let Some(h) = i.and_then(|i| self.hosts.get_mut(i)) {
            let id = h.id.clone();
            let settings = h.settings.clone();
            let last = h.last.clone();
            // Ajouté à nouveau : ses réglages, sa dernière session et de quoi le réveiller restent.
            let wake = if host.wake.is_empty() { h.wake.clone() } else { host.wake.clone() };
            let mac = host.mac.clone().or_else(|| h.mac.clone());
            let relay = host.relay.clone().or_else(|| h.relay.clone());
            *h = Host { id: id.clone(), settings, last, wake, mac, relay, ..host };
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
            *old = Borrowed { id: id.clone(), hdr_off: old.hdr_off, mic: old.mic, pen_mouse: old.pen_mouse, ..b };
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
        self.station_offers.remove(id);
        self.account_hosts.retain(|a| a.id != id);
        self.hosts.retain(|h| h.id != id);
        self.borrowed.retain(|b| b.id != id);
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
    }

    /// L'ordinateur `id` vient d'un compte (pas appairé sur cet appareil).
    pub fn account_host(&self, id: &str) -> Option<&AccountHost> {
        self.account_hosts.iter().find(|a| a.id == id)
    }

    /// L'ordinateur d'un compte derrière `id` (un ordinateur de l'accueil venu d'un compte) : ses droits.
    pub fn account_computer(&self, id: &str) -> Option<&KnComputer> {
        let link = self.account_host(id)?;
        self.account_computers.iter().find(|c| c.relay == link.relay && c.station == link.station)
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
        self.sync_forget_account(key);
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
                    pyrowave: false,
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
        let ids: Vec<String> = self.account_hosts.iter().map(|a| a.id.clone()).collect();
        let mut changed = false;
        for id in ids {
            let Some(h) = self.host_mut(&id) else { continue };
            let seen = found.iter().find(|(_, _, f)| kybernow_rules::fingerprint::same_opt(h.fingerprint.as_deref(), f.as_deref()));
            let (address, port) = seen.map(|(a, p, _)| (a.clone(), *p)).unwrap_or_default();
            if h.address != address || h.port != port {
                h.address = address;
                h.port = port;
                changed = true;
            }
        }
        changed
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

/// « 20261127-091500 » (UTC), pour nommer un fichier mis de côté.
fn utc_stamp(secs: u64) -> String {
    kybernow_rules::format::Utc::from_unix(secs as i64).stamp()
}

#[cfg(test)]
mod store_file_tests {
    use super::*;

    /// « Garder la dernière image » coupé pour toute l'app avant la 0.11.8 : coupé pour chaque ordinateur, et le champ
    /// d'avant n'est plus écrit (TRANS-39 : le réglage est celui de l'ordinateur).
    #[test]
    fn garder_la_derniere_image_par_ordinateur() {
        let d = dir("garder-image");
        let path = d.join("kybernow.json");
        let host = r#"{"id":"a","name":"Atelier","address":"192.168.1.20","port":9443,"fingerprint":null,"user":"u","password":"","os":null,"gpu":null,"av1":false,"pyrowave":false,"mac":null,"wake":[],"relay":null,"settings":{},"last":null}"#;
        std::fs::write(&path, format!(r#"{{"hosts":[{host}],"app":{{"keep_last_image":false}}}}"#)).unwrap();
        let s = Store::load_from(path.clone(), 0);
        assert!(!s.hosts[0].settings.keep_last_image);
        let saved = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert!(v["app"].get("keep_last_image").is_none());
        assert_eq!(v["hosts"][0]["settings"]["keep_last_image"], false);
        // Sans réglage d'avant : gardée.
        assert!(HostSettings::default().keep_last_image);
        let _ = std::fs::remove_dir_all(&d);
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kybernow-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn date_utc() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(1_795_770_000), "20261127-090000");
        assert_eq!(utc_stamp(951_782_400), "20000229-000000");
    }

    /// Un kybernow.json illisible n'est jamais réécrit vide : il est mis de côté, et l'app le sait.
    #[test]
    fn unreadable_file_is_set_aside() {
        let d = dir("illisible");
        let p = d.join("kybernow.json");
        std::fs::write(&p, b"{\"hosts\": [ {\"id\": \"a\", tronqu").unwrap();
        let s = Store::load_from(p.clone(), 0);
        let aside = s.set_aside.clone().expect("mis de côté");
        assert_eq!(aside, d.join("kybernow.json.illisible-19700101-000000"));
        assert_eq!(std::fs::read(&aside).unwrap(), b"{\"hosts\": [ {\"id\": \"a\", tronqu");
        // Un nouveau fichier, vide mais lisible, à sa place.
        let again = Store::load_from(p.clone(), 1);
        assert!(again.set_aside.is_none() && again.device_id == s.device_id);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_file_starts_fresh() {
        let d = dir("absent");
        let p = d.join("kybernow.json");
        let s = Store::load_from(p.clone(), 0);
        assert!(s.set_aside.is_none() && !s.device_id.is_empty() && p.exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
