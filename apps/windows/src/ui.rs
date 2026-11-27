//! L'app (Tauri, interface Svelte dans `ui/`) : accueil, ajout d'un
//! ordinateur, réglages, passage vers la session et barre de session.
//!
//! Fenêtres : `main` (accueil, sans cadre de Windows : la barre de titre est
//! dessinée par l'interface), `bar` (barre de session, transparente, montrée
//! quand le pointeur touche le haut de l'écran de la session) et `fond-N` (le
//! fond de la connexion sur les écrans de la session autres que l'écran
//! principal, `fonds.rs`).
//! La session elle-même est faite de fenêtres Win32 (une par écran) sur son
//! propre fil (`host.rs`).
//!
//! L'app vit aussi dans l'icône près de l'horloge (`tray.rs`) : fermer
//! l'accueil le cache, « Quitter KyberNow Link » la quitte. Ouverte à
//! l'ouverture de session (`--background`), elle démarre sans fenêtre.
//!
//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : une
//! table des sessions (quatre au plus), chacune repérée par son ordinateur
//! dans chaque événement et chaque commande. La première s'ouvre comme
//! d'habitude (passage de l'accueil, plein écran, fonds des autres écrans) ;
//! chaque suivante s'ouvre dans une fenêtre, sur l'écran de la fenêtre active,
//! et sa connexion se montre dans une fenêtre à sa place (`connexion-N`,
//! `connexion.html`), sans passer par l'accueil.

use crate::sync::Locked;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow};

use kybernow_core::console::{self, Console, Info, PairStatus, StationError};
use kybernow_core::relay::RouteInfo;
use kybernow_core::profile::{Codec, Profile};
use kybernow_core::screens::{self, DisplaysSetting};
use kybernow_core::session::{SessionParams, SessionRole, Status};

use crate::discovery::{self, Discovery, Found};
use crate::displays::{self, Layout, Spec};
use crate::fonds::{self, Fonds};
use crate::host::{HostEvents, HostOptions, Role, ScreenSet, SessionHost, Stats, Summary};
use kybernow_core::plan::{Choices, plan};
use crate::store::{self, Computer, LastSession, Settings, Store};
use crate::window::Command;
use crate::pc::{self, ThisPc};
use crate::i18n::t;
use crate::follow::{self, Kind};
use crate::menu::{self, Cmd};
use crate::{audio, focus, i18n, instance, login, relais, shell, snapshot, system, tray, window};
use kybernow_core::share::{ShareEnd, ShareImage, ShareQuality, ShareRole, ShareState};
use kybernow_core::update::{self, Available, Channel};

// Micro de ce PC vers l'ordinateur : puce de la barre, choix par ordinateur,
// Réglages de l'app (docs/micro.md).
mod micro;
// KyberNow Share : « Rejoindre avec un code » (session d'invité) et « Partager »
// (panneau de l'hôte dans la barre).
mod share;
// Profil Auto : le test des conditions, ses réglages appliqués, le bandeau
// « Autre réseau que celui du test » (docs/profil-auto.md).
mod autotest;
// Prêt de l'ordinateur (KyberNow Share) : l'ami (ordinateurs empruntés, session
// d'emprunt) et l'hôte (« Prêter l'ordinateur… », « Reprendre l'ordinateur »).
mod lend;
// Invitations : « Rejoindre avec un code » qui tombe sur un code d'invitation,
// puis l'ordinateur ajouté avec le mot de passe (docs/invitations.md).
mod invite;
// Comptes KyberNow : connexion par la page du relais, comptes gardés, leurs
// ordinateurs à l'accueil, ouverts avec le jeton d'accès (docs/comptes.md).
mod account;
// Réglages synchronisés par le compte : lus avec les ordinateurs des comptes et
// avant une session, envoyés quand la personne les change (docs/comptes.md).
mod synced;
// Consoles PlayStation : recherche, comptes PSN, association, partie et sa
// barre, réglages, envoi entre appareils (docs/playstation.md).
mod playstation;
// Accueil au premier lancement et incitation au compte KyberNow : état gardé
// dans `app.json`, carte du compte, bandeaux (docs/link-ui.md).
mod welcome;
// Mise à jour des Stations proposée à l'accueil (bandeau, puce « · Mise à
// jour »), vérification demandée à la Station, avis après une mise à jour de
// l'app (docs/link-ui.md « Mise à jour des Stations »).
mod stations;
// Enregistrement de la session : le bouton et la pastille de la barre, le menu
// de l'icône, Ctrl+Alt+Maj+R (docs/link-ui.md « Enregistrement de la session »).
mod recording;
// La caméra de ce PC vers l'ordinateur : la puce de la barre, Ctrl+Alt+Maj+C,
// les réponses de l'ordinateur (docs/webcam.md).
mod camera;

#[derive(Debug, Clone, Default)]
struct Live {
    online: Option<bool>,
    info: Option<Info>,
    preview: Option<Image>,
    preview_at: Option<Instant>,
    /// Dernière image de la dernière session (passe avant l'aperçu).
    last: Option<Image>,
    last_loaded: bool,
    /// L'ordinateur ne répond pas sur le réseau mais le relais le tient : « Hors
    /// de chez vous » (`online` est alors vrai : on peut s'y connecter).
    away: bool,
    /// Le prêt de l'ordinateur, quand il y est activé (`GET /api/lend`) :
    /// « Prêté à {prénom} » sur l'accueil.
    lend: Option<kybernow_core::lend::LendView>,
    /// L'erreur sous son nom à l'accueil (docs/link-ui.md, TRANS-39).
    problem: Option<Problem>,
}

/// Ce que l'accueil dit sous le nom d'un ordinateur qui répond mais refuse
/// (docs/link-ui.md, « Accueil » ; comme sur Mac) : « Mot de passe refusé :
/// mettez-le à jour dans les réglages. », « La Station a été réinstallée :
/// vérifiez son empreinte dans les réglages. ».
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Problem {
    /// Admin refuse le compte (401, 403).
    PasswordRefused,
    /// Le certificat d'Admin n'est plus celui retenu (Station réinstallée).
    Reinstalled,
}

/// Une image de l'accueil (dernière image d'une session ou aperçu de
/// l'écran, JPEG de 100 à 300 Ko) et son numéro, unique à chaque nouvelle
/// image. L'interface ne reçoit que son adresse (`image_url`), servie par le
/// protocole `knimage` de l'app : l'événement `computers`, envoyé souvent
/// (suivi de réveil : toutes les 2 s), ne transporte plus les images en
/// base64, et une image n'est relue par la page que quand elle change.
#[derive(Debug, Clone)]
struct Image {
    version: u64,
    jpeg: std::sync::Arc<Vec<u8>>,
}

impl Image {
    fn new(jpeg: Vec<u8>) -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self { version: SEQ.fetch_add(1, Ordering::Relaxed), jpeg: std::sync::Arc::new(jpeg) }
    }
}

/// Protocole des images de l'accueil : `http://knimage.localhost/<ordinateur>?v=<numéro>`
/// (WebView2 sert un protocole de l'app sous `http://<nom>.localhost`).
const IMAGE_SCHEME: &str = "knimage";

fn image_url(id: &str, image: &Image) -> String {
    let id: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).collect();
    format!("http://{IMAGE_SCHEME}.localhost/{id}?v={}", image.version)
}

/// Le JPEG d'une adresse `image_url` : la dernière image, sinon l'aperçu.
fn serve_image(app: &AppHandle, path: &str) -> tauri::http::Response<Vec<u8>> {
    let id = path.trim_start_matches('/');
    let jpeg = app.state::<UiState>().live.locked().get(id).and_then(|l| l.last.clone().or_else(|| l.preview.clone())).map(|i| i.jpeg);
    let builder = tauri::http::Response::builder();
    match jpeg {
        Some(j) => builder
            .header("Content-Type", "image/jpeg")
            // L'adresse change avec l'image (`?v=`) : celle-ci ne change jamais.
            .header("Cache-Control", "max-age=31536000, immutable")
            .body(j.as_ref().clone()),
        None => builder.status(404).body(Vec::new()),
    }
    .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
pub struct ComputerView {
    #[serde(flatten)]
    computer: Computer,
    /// L'empreinte d'Admin telle que link-ui la montre (« 7C41 E09B AABB CCDD »,
    /// `console::short_fingerprint`, TRANS-34) ; vide sans empreinte.
    fingerprint_short: String,
    /// L'erreur sous le nom (`Problem`), quand il y en a.
    problem: Option<Problem>,
    /// Le relais d'un ordinateur d'un compte, tel qu'on le montre
    /// (`account::relay_label`, sans empreinte) ; vide sinon.
    relay_label: String,
    online: Option<bool>,
    /// Joignable seulement par le relais (état « Hors de chez vous », docs/relais.md).
    away: bool,
    /// Par le relais : `away`, ou une session de cet ordinateur y passe. Admin refuse alors le
    /// redémarrage et la mise à jour de la Station : les réglages leur substituent la raison.
    via_relay: bool,
    info: Option<Info>,
    preview: Option<String>,
    /// `last` : dernière image de votre session ; `preview` : aperçu de son écran.
    preview_source: Option<&'static str>,
    /// Réveil par le réseau envoyé : l'app attend l'ordinateur (2 min au plus).
    waking: bool,
    /// `restart` ou `update` : la Station redémarre ou s'installe, l'app
    /// attend son retour (`station-outcome` à la fin).
    station_pending: Option<&'static str>,
    /// Prêté à un ami : « Prêté à {prénom} », « Reprendre l'ordinateur ».
    lend: Option<crate::lending::HomeLend>,
    /// La pastille d'origine (comptes KyberNow, seulement avec un compte
    /// connecté) : « Compte », « Compte · {relais} », l'organisation, « Ouvert
    /// par {prénom} », « Appairé ».
    origin: Option<String>,
    /// La mise à jour de sa Station proposée à l'accueil : le bandeau (ordinateur
    /// choisi) et la puce « · Mise à jour » (calcul du cœur, `stations::offer`).
    offer: stations::Offer,
}

/// Un ordinateur à ajouter : adresse et port de sa console, empreinte
/// attendue (lien, annonce du réseau) ou vue au premier contact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub host: String,
    pub port: u16,
    pub name: Option<String>,
    pub fingerprint: Option<String>,
    /// L'empreinte à comparer à celle qu'Admin affiche (« 7C41 E09B AABB
    /// CCDD », `console::short_fingerprint`) ; rendue par l'interface sans
    /// être relue.
    #[serde(default, skip_deserializing)]
    pub fingerprint_short: String,
}

impl Candidate {
    fn new(host: String, port: u16, name: Option<String>, fingerprint: Option<String>) -> Self {
        let fingerprint_short = fingerprint.as_deref().map(console::short_fingerprint).unwrap_or_default();
        Self { host, port, name, fingerprint, fingerprint_short }
    }
}

struct Pairing {
    candidate: Candidate,
    id: String,
}

/// Une session en cours (une par ordinateur, ou une session d'invité d'un
/// partage : `id` est alors `guest-N`, jamais un ordinateur de l'app).
struct Running {
    id: String,
    /// Le nom de l'ordinateur (journal) ; « Invité sur {nom} » pour un invité.
    name: String,
    host: SessionHost,
    bar: Arc<Mutex<BarLabel>>,
    sent: Arc<Mutex<Sent>>,
    /// Dernières mesures de la barre (menu de l'icône).
    stats: Arc<Mutex<Option<Stats>>>,
    /// Session d'invité (KyberNow Share) : l'ordinateur qui partage et le rôle.
    guest: Option<Arc<share::Guest>>,
    /// Session d'emprunt (prêt de l'ordinateur, `lend-N`) : l'hôte, la fin prévue,
    /// le préavis.
    borrow: Option<Arc<lend::Borrow>>,
    /// Session d'hôte : le partage ouvert et ses invités, tels qu'Admin les dit
    /// (signe de vie de la session, ou réponse à une action de la barre).
    share: Arc<Mutex<Option<ShareState>>>,
    /// Session d'hôte : le prêt de l'ordinateur (code, demandes), tel qu'Admin le dit.
    lend: Arc<Mutex<Option<kybernow_core::lend::LendView>>>,
    /// Par où passe la session (`on_route` du cœur) : la puce « Par le relais » de
    /// la barre, et le chemin que prennent les appels à Admin (par le relais si la
    /// session y est). `None` tant que le cœur ne l'a pas dit.
    route: Arc<Mutex<Option<RouteInfo>>>,
    /// Ses nouvelles (celles que reçoit `SessionHost`) : l'app s'en sert pour
    /// la tenir pour finie quand elle ne finit pas après une fin demandée
    /// (`watch_end`).
    events: Arc<UiEvents>,
}

/// La connexion d'une session ouverte pendant qu'une autre tourne (plusieurs
/// ordinateurs) : une fenêtre à la place de celle de la session
/// (`connexion.html`, le même passage que l'accueil), jusqu'à la première
/// image ; un échec y reste, avec sa raison, jusqu'à « Fermer ».
#[derive(Default)]
struct Connexion {
    /// La session qu'elle montre (`SessionHost::id`) : « Annuler » et sa
    /// fermeture ne visent qu'elle, jamais une autre session du même
    /// ordinateur ni une autre session ; rien quand elle est finie.
    session: u64,
    /// Le nom de l'ordinateur (journal).
    name: String,
    /// Sa fenêtre Tauri quand elle est ouverte.
    label: Mutex<Option<String>>,
    /// Ce qu'elle montre (lu par la page à son ouverture, puis les événements).
    view: Mutex<ConnexionView>,
    /// Dernier geste qui la ferme (« Fermer », « Annuler », sa croix) : elle
    /// reste le temps d'un double clic, pour que le second clic ne tombe pas
    /// sur la fenêtre d'une session restée dessous (`close_connexion`).
    closed_at: Mutex<Option<Instant>>,
}

#[derive(Clone, Serialize, Default)]
struct ConnexionView {
    id: String,
    phase: &'static str,
    restarting: bool,
    /// La session s'est terminée avant sa première image : la raison.
    error: Option<String>,
    /// Session d'invité (KyberNow Share) : son titre (« Invité sur {nom} »,
    /// que la page montre à la place du nom d'un ordinateur de l'app) et la
    /// teinte de l'ordinateur qui partage ; la raison d'un échec se lit alors
    /// telle quelle, sans « La connexion a échoué : ».
    title: Option<String>,
    fingerprint: Option<String>,
    guest: bool,
    /// Session d'emprunt (prêt de l'ordinateur) : « Préparation de
    /// l'ordinateur » en première étape, comme le passage de l'accueil.
    borrow: bool,
}

/// Numéro des fenêtres de connexion (`connexion-N`).
static CONNEXION_SEQ: AtomicU32 = AtomicU32::new(0);

/// Ce que la session transmet (rubrique Statistiques à sa fin) : réglages du
/// flux, suivis quand le profil change depuis la barre, et mode de l'image,
/// à la taille que l'ordinateur envoie vraiment.
struct Sent {
    settings: kybernow_core::profile::StreamSettings,
    /// Définition et fréquence de l'écran principal.
    mode: kybernow_core::console::Mode,
    /// Écrans de la session, et leurs pixels par seconde ensemble (débit total).
    screens: u32,
    pixel_rate: f64,
    /// L'écran virtuel est demandé (sans lui, une session à un écran).
    virtual_display: bool,
}

impl Sent {
    /// Débit demandé à l'ordinateur, en Mbit/s : celui du profil pour
    /// l'écran, ou pour la somme des écrans.
    fn mbps(&self) -> u32 {
        let bits = if self.screens > 1 && self.pixel_rate > 0.0 {
            self.settings.bitrate_for_pixel_rate(self.pixel_rate)
        } else {
            self.settings.bitrate(self.mode.width, self.mode.height, self.mode.refresh)
        };
        bits / 1_000_000
    }
}

/// L'accueil et la connexion de la session sur les écrans (multi-écrans,
/// docs/link-ui.md « Plusieurs écrans »). La connexion se lit sur l'écran
/// principal de ce PC : l'accueil y passe, et reprend sa place à la fin.
#[derive(Default)]
struct Spot {
    /// L'écran principal de la session : son nom et son cadre.
    primary: Option<fonds::Target>,
    /// Les autres écrans pris par la session (le fond de la connexion y est).
    others: Vec<fonds::Target>,
    /// Où était l'accueil avant de passer sur l'écran principal.
    before: Option<tauri::PhysicalPosition<i32>>,
    /// Une image a été montrée : la session est établie, la connexion qui
    /// reprend ensuite (écrans de ce PC ou profil changés) est une reprise.
    shown: bool,
    /// Les écrans qui ont montré leur image depuis le début de la connexion
    /// (ou de sa reprise) : leur fond est parti, il ne revient pas.
    visible: Vec<String>,
    /// Reprise de la connexion en session : l'accueil couvre l'écran principal.
    replanning: Option<Replan>,
}

/// L'accueil avant qu'il couvre l'écran principal pour une reprise de la connexion.
struct Replan {
    visible: bool,
    position: Option<tauri::PhysicalPosition<i32>>,
    size: Option<tauri::PhysicalSize<u32>>,
}

#[derive(Default)]
pub struct UiState {
    store: Mutex<Option<Store>>,
    live: Mutex<HashMap<String, Live>>,
    discovery: OnceLock<Arc<Discovery>>,
    this_pc: Mutex<Option<ThisPc>>,
    pairing: Mutex<Option<Pairing>>,
    /// Les sessions en cours, dans l'ordre d'ouverture (plusieurs ordinateurs,
    /// quatre au plus).
    sessions: Mutex<Vec<Running>>,
    /// Les ordinateurs dont la connexion se lance (`connect`, jusqu'à
    /// l'inscription de leur session) : ils comptent dans les quatre.
    launching: Mutex<Vec<String>>,
    /// Fermeture demandée pendant des sessions : quitter à la fin de la dernière
    /// (`QUIT_WITHIN` au plus).
    quitting: std::sync::atomic::AtomicBool,
    /// « Annuler » pendant le lancement d'une connexion (`connect` pas encore
    /// revenu, session pas encore inscrite) : elle s'arrête dès son inscription.
    cancelled_launch: Mutex<Vec<String>>,
    /// L'ordinateur de la session que sert la barre (une seule barre).
    bar_session: Mutex<Option<String>>,
    /// Les fenêtres de connexion ouvertes (libellé → connexion), y compris
    /// celles qui montrent un échec après la fin de leur session.
    connexions: Mutex<HashMap<String, Arc<Connexion>>>,
    /// Demande du lancement (lien, barre des tâches…), lue par l'interface.
    action: Mutex<Option<Action>>,
    prefs: Mutex<Prefs>,
    update: Mutex<Option<Available>>,
    /// Réveils en cours (paquet envoyé à cet instant), suivis toutes les 2 s.
    waking: Mutex<HashMap<String, Instant>>,
    /// Redémarrages et mises à jour de la Station en attente de son retour.
    following: Mutex<HashMap<String, (follow::Pending, Instant)>>,
    /// Relevé demandé tout de suite (ouverture du menu de l'icône).
    poll_asked: Mutex<bool>,
    poll_signal: Condvar,
    /// Fond de la connexion sur les écrans autres que l'écran principal.
    fonds: OnceLock<Arc<Fonds>>,
    /// Un partage rejoint (`share_join`) dont la session s'ouvre
    /// (`share_connect`) : un seul à la fois.
    pending_guest: Mutex<Option<share::Pending>>,
    /// Une demande par le relais qui attend l'accord de l'hôte (« En attente de
    /// l'accord de l'hôte… ») : une seule à la fois.
    join_wait: Mutex<Option<share::Wait>>,
    /// La présence des ordinateurs au relais, redemandée toutes les 30 s au plus.
    presence: Mutex<relais::Presence>,
    /// Prêt de l'ordinateur : ordinateurs empruntés, demandes, prêt accordé.
    lend: lend::LendState,
    /// Invitations : le code trouvé par « Rejoindre », en attente de son mot de passe.
    invite: invite::InviteState,
    /// Consoles PlayStation : comptes, consoles, parties en cours.
    ps: playstation::PsState,
    /// Les ordinateurs dont la Station vérifie ses mises à jour en ce moment
    /// (`stations::check`).
    checking: Mutex<std::collections::HashSet<String>>,
    /// L'avis après une mise à jour de l'app, en attente de l'interface.
    station_notice: Mutex<Option<stations::StationNotice>>,
}

/// Ce que l'ouverture demande à l'app.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Action {
    Connect { id: String },
    /// « Regarder » (droit « Regarder » d'un compte KyberNow, menu de l'icône).
    Watch { id: String },
    Add,
    /// « Rejoindre avec un code » (menu de l'icône, `kybernow://join?…` : le lien).
    Join { link: Option<String> },
    /// Le lien d'invitation d'un compte d'ami (`kybernow://borrow?…`) : « Ordinateur de {hôte} ».
    Borrow { link: String },
    Link { link: String },
    Updated,
    Open,
    /// L'accueil sur cet ordinateur (notification).
    Show { id: String },
    /// Ses réglages, sur une rubrique (`station`) ou la première.
    Settings { id: String, rubric: Option<String> },
    /// Les réglages de l'app ; `check` : en cherchant une mise à jour.
    App { check: bool },
    /// Les réglages d'une console PlayStation (« Réglages de {nom} » de la barre de la partie).
    #[serde(rename = "ps_settings")]
    PsSettings { id: String },
    /// Le panneau du test du profil Auto (Auto choisi sans test dans le menu de
    /// l'icône, ou demandé pendant la session de cet ordinateur).
    Autotest { id: String },
}

fn parse_action(args: &[String]) -> Option<Action> {
    let first = args.first()?;
    // Le retour de la page de connexion d'un compte (`kybernow://compte?…`) :
    // `account::callback`, rien à demander à l'interface.
    if kybernow_core::account::is_callback(first) {
        return None;
    }
    // Le lien (ou le QR code) d'un partage : « Rejoindre avec un code ».
    if crate::link::is_join(first) {
        return Some(Action::Join { link: Some(first.clone()) });
    }
    // Le lien d'invitation d'un compte d'ami (prêt de l'ordinateur), reconnu
    // par le cœur ; illisible, la feuille le dit (« Ce lien d'invitation est
    // illisible. »).
    if kybernow_core::lend::looks_like_invite(first) {
        return Some(Action::Borrow { link: first.clone() });
    }
    if first.starts_with("kybernow://") {
        // Actions des notifications (link.rs) : kybernow://connect?id=…,
        // kybernow://open (?id=… : sur cet ordinateur), kybernow://station?id=…
        // (sa rubrique Station) ; les autres liens viennent d'Admin.
        return Some(match crate::link::action(first) {
            Some(("connect", Some(id))) => Action::Connect { id },
            Some(("open", Some(id))) => Action::Show { id },
            Some(("station", Some(id))) => Action::Settings { id, rubric: Some("station".into()) },
            Some(_) => Action::Open,
            None => Action::Link { link: first.clone() },
        });
    }
    match first.as_str() {
        "--connect" => args.get(1).map(|id| Action::Connect { id: id.clone() }),
        "--add" => Some(Action::Add),
        "--updated" => Some(Action::Updated),
        _ => None,
    }
}

/// Réglages de l'app (`%APPDATA%\KyberNow\app.json`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Prefs {
    pub channel: Channel,
    /// Langue de l'app : `auto` (celle de Windows, aussi si absent), `fr`, `en`.
    pub language: String,
    /// Sur batterie : `alleger` (30 Hz au plus) ou `garder` ; absent, repris
    /// des réglages des ordinateurs (voir `battery_choice`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery: Option<String>,
    /// « Ouvrir à l'ouverture de session » : absent avant la version qui
    /// l'apporte (elle le coche alors une fois, voir `login.rs`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_at_login: Option<bool>,
    /// La notification « KyberNow Link est toujours là » a été montrée.
    pub close_hint: bool,
    /// Le prénom donné à « Rejoindre avec un code » (KyberNow Share), proposé
    /// la fois suivante ; absent : le nom de ce PC.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_name: Option<String>,
    /// La « Qualité de l'image » choisie à « Rejoindre » ou dans la barre d'un
    /// invité (`auto`, `host`, `2160p60`, `1080p60`, `720p30`), proposée la fois suivante ;
    /// absent : « Auto ».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_quality: Option<ShareQuality>,
    /// « Synchroniser les réglages des ordinateurs » (comptes KyberNow) ;
    /// absent : allumé.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_settings: Option<bool>,
    /// L'accueil au premier lancement et l'incitation au compte KyberNow
    /// (`kybernow_core::welcome`, docs/link-ui.md « Accueil et compte
    /// KyberNow ») ; absent des fichiers d'avant : l'accueil se montre une fois.
    pub welcome: kybernow_core::welcome::WelcomeState,
    /// La dernière version de l'app lancée : l'avis « … a aussi une mise à
    /// jour » au premier lancement d'une nouvelle version (`stations::launched`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_launched: Option<String>,
    /// « Micro » des Réglages de l'app (docs/micro.md) : l'identifiant de
    /// Windows du micro choisi ; absent : celui du système.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub microphone: Option<String>,
    /// « Réduire l'écho et le bruit » ; absent : allumé.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_processing: Option<bool>,
}

/// Sur batterie, pour toutes les sessions de ce PC : `alleger` ou `garder`.
/// Sans choix dans `app.json`, celui que gardaient les réglages des
/// ordinateurs (`garder` si l'un l'avait).
fn battery_choice(state: &UiState) -> &'static str {
    let chosen = state.prefs.locked().battery.clone();
    let keep = match chosen.as_deref() {
        Some(b) => b == "garder",
        None => state.with_store(|s| s.computers.iter().any(|c| c.settings.battery == "garder")),
    };
    if keep { "garder" } else { "alleger" }
}

/// `auto`, `fr` ou `en`.
fn language_choice(p: &Prefs) -> &'static str {
    match p.language.as_str() {
        "fr" => "fr",
        "en" => "en",
        _ => "auto",
    }
}

fn prefs_path() -> std::path::PathBuf {
    crate::files::app_file("app.json")
}

/// `app.json` ; illisible, il est mis de côté et noté au journal
/// (`files::load_json`) avant de repartir des réglages par défaut.
pub fn load_prefs() -> Prefs {
    crate::files::load_json(&prefs_path()).unwrap_or_default()
}

/// Modifie les réglages de l'app et écrit `app.json` sans lâcher le verrou :
/// deux commandes presque simultanées ne peuvent plus écrire dans le désordre
/// (le fichier est minuscule).
pub(crate) fn update_prefs<T>(state: &UiState, f: impl FnOnce(&mut Prefs) -> T) -> T {
    let mut p = state.prefs.locked();
    let out = f(&mut p);
    write_prefs(&p);
    out
}

/// Écrit `app.json` (atomique) ; l'appelant tient le verrou des réglages ou
/// est seul (lancement).
pub(crate) fn write_prefs(prefs: &Prefs) {
    crate::files::save_json(&prefs_path(), prefs);
}

/// Installateur publié : un seul pour x64 et ARM64, il pose la version du
/// processeur (une app x64 émulée sur ARM passe ainsi à la version ARM64).
/// `KyberNow-Link-Setup.exe` d'abord, puis `KyberNow-Client-Setup.exe`, seul
/// nom que listent les manifestes des versions 0.3.1 et avant (publié en copie
/// pour l'app de ces versions).
const INSTALLER: &[&str] = &["KyberNow-Link-Setup.exe", "KyberNow-Client-Setup.exe"];

impl UiState {
    /// `f` sur les ordinateurs ; ce qu'elle a enregistré (`Store::save`) est
    /// écrit après avoir lâché le verrou : `views()`, le menu de l'icône et le
    /// relevé n'attendent plus le disque.
    fn with_store<T>(&self, f: impl FnOnce(&mut Store) -> T) -> T {
        let (out, pending) = {
            let mut s = self.store.locked();
            let s = s.get_or_insert_with(Store::load);
            let out = f(s);
            (out, s.take_pending())
        };
        for p in pending {
            crate::files::write_versioned(p);
        }
        out
    }

    fn views(&self) -> Vec<ComputerView> {
        let (computers, from_accounts) = self.with_store(|s| (s.computers.clone(), s.account_computers.clone()));
        // Les pastilles d'origine (comptes KyberNow), hors du verrou des ordinateurs.
        let mut origins = account::origins(&computers, &from_accounts);
        let waking: Vec<String> = self.waking.locked().keys().cloned().collect();
        let pending: HashMap<String, Kind> = self.following.locked().iter().map(|(id, (p, _))| (id.clone(), p.kind)).collect();
        // Les ordinateurs dont une session passe par le relais (lu avant `live` : les deux verrous ne se tiennent jamais ensemble).
        let relayed = self.sessions_via_relay();
        // Une session ouverte : ni bandeau ni avis de la mise à jour de la Station.
        let in_session = !self.session_ids().is_empty();
        let mut live = self.live.locked();
        computers
            .into_iter()
            .map(|c| {
                let l = live.entry(c.id.clone()).or_default();
                // Dernière image gardée sur le disque : lue une fois.
                if !l.last_loaded {
                    l.last_loaded = true;
                    if c.settings.keep_last_image {
                        l.last = snapshot::load(&c.id).map(Image::new);
                    }
                }
                let l = l.clone();
                let offer = stations::offer(&c, &l, in_session);
                let (preview, preview_source) = match (&l.last, &l.preview) {
                    (Some(last), _) => (Some(image_url(&c.id, last)), Some("last")),
                    (None, Some(p)) => (Some(image_url(&c.id, p)), Some("preview")),
                    (None, None) => (None, None),
                };
                let waking = waking.contains(&c.id);
                let station_pending = pending.get(&c.id).map(|k| k.code());
                let via_relay = l.away || relayed.contains(&c.id);
                let lend = l.lend.as_ref().and_then(crate::lending::HomeLend::of);
                let origin = origins.remove(&c.id);
                let fingerprint_short = c.fingerprint.as_deref().map(console::short_fingerprint).unwrap_or_default();
                let relay_label = c.account.as_ref().map(|a| kybernow_core::account::relay_label(&a.relay)).unwrap_or_default();
                ComputerView { computer: c, fingerprint_short, problem: l.problem, relay_label, online: l.online, away: l.away, via_relay, info: l.info, preview, preview_source, waking, station_pending, lend, origin, offer }
            })
            .collect()
    }

    /// Les ordinateurs dont une session passe par le relais (puce « Par le relais »).
    fn sessions_via_relay(&self) -> Vec<String> {
        self.sessions
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.route.locked().as_ref().is_some_and(RouteInfo::via_relay))
            .map(|r| r.id.clone())
            .collect()
    }

    /// Les ordinateurs en session, dans l'ordre d'ouverture.
    fn session_ids(&self) -> Vec<String> {
        self.sessions.locked().iter().map(|r| r.id.clone()).collect()
    }

    fn in_session(&self, id: &str) -> bool {
        self.sessions.locked().iter().any(|r| r.id == id)
    }

    /// `f` sur la session de cet ordinateur, s'il est en session.
    fn with_session<T>(&self, id: &str, f: impl FnOnce(&Running) -> T) -> Option<T> {
        self.sessions.locked().iter().find(|r| r.id == id).map(f)
    }

    /// `f` sur cette session-là (`SessionHost::id`), si elle tourne encore.
    fn with_instance<T>(&self, session: u64, f: impl FnOnce(&Running) -> T) -> Option<T> {
        self.sessions.locked().iter().find(|r| r.host.id() == session).map(f)
    }

    /// La session active (celle de la dernière fenêtre de session au premier
    /// plan), sinon la première.
    fn active_id(&self) -> Option<String> {
        let sessions = self.sessions.locked();
        sessions.iter().find(|r| r.host.is_active()).or_else(|| sessions.first()).map(|r| r.id.clone())
    }

    /// Quatre ordinateurs sont en session ou s'y connectent (ou le registre
    /// du cœur en tient quatre).
    fn full(&self) -> bool {
        let sessions = self.sessions.locked();
        sessions.len() + self.launching.locked().len() >= focus::MAX_SESSIONS || focus::count() >= focus::MAX_SESSIONS
    }
}

/// Une session qui ne finit pas après une fin demandée (« Annuler »,
/// « Se déconnecter ») : au bout de ce délai, l'app ferme ses fenêtres et la
/// tient pour finie (accueil, fenêtre de connexion, menu de l'icône), quel que
/// soit l'état du cœur, qui finit derrière.
const END_WITHIN: Duration = Duration::from_secs(5);

/// « Quitter » : les sessions ont ce délai pour finir (Admin de chaque
/// ordinateur prévenue) ; ensuite l'app se ferme quand même.
const QUIT_WITHIN: Duration = Duration::from_secs(5);

/// Un ordre de fin à la session de cet ordinateur (barre, menu de l'icône,
/// passage de l'accueil), et pourquoi (journal) : à elle seule, jamais à la
/// session active ni à une autre. Une connexion qui se lance encore (pas
/// encore inscrite) s'arrête dès son inscription ; rien s'il n'est plus en
/// session.
fn end_session(app: &AppHandle, id: &str, why: &str) -> bool {
    let state = app.state::<UiState>();
    let sent = {
        // Tenu pendant le contrôle : `connect` regarde les annulations sous ce verrou.
        let sessions = state.sessions.locked();
        match sessions.iter().find(|r| r.id == id) {
            Some(r) => {
                log::info!("[{}] {why} : ordre de fin à sa session (n°{})", r.name, r.host.id());
                r.host.command(Command::Disconnect);
                Some(r.host.id())
            }
            None => {
                if state.launching.locked().iter().any(|x| x == id) {
                    log::info!("{why} pour {id} : la connexion se lance, elle s'arrêtera dès son inscription");
                    state.cancelled_launch.locked().push(id.to_string());
                    return true;
                }
                None
            }
        }
    };
    match sent {
        Some(session) => {
            watch_end(app, session);
            true
        }
        None => {
            log::info!("{why} pour {id} : plus en session, rien à arrêter");
            false
        }
    }
}

/// Un ordre de fin à cette session-là (`SessionHost::id` : sa fenêtre de
/// connexion, Quitter pendant son lancement), et pourquoi : rien si elle est
/// finie, même si son ordinateur a une autre session depuis.
fn end_instance(app: &AppHandle, session: u64, why: &str) -> bool {
    let state = app.state::<UiState>();
    let sent = state
        .with_instance(session, |r| {
            log::info!("[{}] {why} : ordre de fin à sa session (n°{session})", r.name);
            r.host.command(Command::Disconnect);
        })
        .is_some();
    if sent {
        watch_end(app, session);
    } else {
        log::info!("{why} : la session n°{session} est finie, rien à arrêter");
    }
    sent
}

/// Après une fin demandée : si la session n'a pas fini dans `END_WITHIN`, ses
/// fenêtres ferment et l'app la tient pour finie, comme une fin demandée
/// (`UiEvents::ended`) ; sa vraie fin, plus tard, n'y change plus rien.
fn watch_end(app: &AppHandle, session: u64) {
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("kybernow-end-watch".into()).spawn(move || {
        std::thread::sleep(END_WITHIN);
        let state = app.state::<UiState>();
        let Some((name, events)) = state.with_instance(session, |r| {
            r.host.close_windows();
            (r.name.clone(), r.events.clone())
        }) else {
            return;
        };
        log::warn!(
            "[{name}] session n°{session} pas finie {} s après la demande de fin : fenêtres fermées, tenue pour finie (le cœur finit derrière)",
            END_WITHIN.as_secs()
        );
        events.ended(&Summary { requested: true, ..Default::default() });
    });
    if let Err(e) = spawned {
        log::error!("surveillance de la fin : fil refusé ({e})");
    }
}

/// Une connexion annulée pendant son lancement (`end_session`) : vrai une
/// fois, à son inscription (sous le verrou des sessions).
fn take_cancelled_launch(state: &UiState, id: &str) -> bool {
    let mut cancelled = state.cancelled_launch.locked();
    match cancelled.iter().position(|x| x == id) {
        Some(i) => {
            cancelled.remove(i);
            true
        }
        None => false,
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    computers: Vec<ComputerView>,
    discovered: Vec<Found>,
    this_pc: Option<ThisPc>,
    /// Les ordinateurs en session, dans l'ordre d'ouverture.
    sessions: Vec<String>,
    version: String,
}

fn emit_computers(app: &AppHandle) {
    let state = app.state::<UiState>();
    let views = state.views();
    shell::update_jump_list(views.iter().map(|v| (v.computer.id.clone(), v.computer.name.clone())).collect());
    let _ = app.emit_to("main", "computers", views);
    // Un ordinateur ajouté ou retiré : la Station trouvée sur le réseau part ou revient.
    emit_discovered(app);
    emit_bar_computers(app);
    tray::refresh();
}

/// Les Stations trouvées sur le réseau qui ne sont pas déjà à l'accueil :
/// même adresse et même port qu'un ordinateur, ou la même empreinte d'Admin
/// qu'un ordinateur d'un compte (la règle du cœur, `console::same_fingerprint`,
/// TRANS-34 : préfixe, deux-points, espaces et casse ignorés).
fn fresh_found(state: &UiState) -> Vec<Found> {
    let list = state.discovery.get().map(|d| d.list()).unwrap_or_default();
    state.with_store(|s| {
        list.into_iter()
            .filter(|f| {
                !s.computers.iter().any(|c| {
                    (c.host == f.host && c.port == f.port) || (c.account.is_some() && console::same_fingerprint_opt(c.fingerprint.as_deref(), f.fingerprint.as_deref()))
                })
            })
            .collect()
    })
}

/// La liste des Stations trouvées pour l'accueil (`fresh_found`).
fn emit_discovered(app: &AppHandle) {
    let _ = app.emit_to("main", "discovered", fresh_found(&app.state::<UiState>()));
}

/// Les sessions ont changé (une de plus, une de moins) : l'accueil (« Afficher
/// la session », quatre au plus), la barre (menu « Ordinateurs ») et l'icône.
fn emit_sessions(app: &AppHandle) {
    let mut ids = app.state::<UiState>().session_ids();
    // Les parties sur console aussi (« Afficher la session », la barre).
    ids.extend(playstation::session_ids(app));
    let _ = app.emit("sessions", &ids);
    // Le bandeau de la mise à jour de la Station se cache en session, et revient après.
    let _ = app.emit_to("main", "computers", app.state::<UiState>().views());
    emit_bar_computers(app);
    tray::refresh();
}

/// Un ordinateur du menu « Ordinateurs » de la barre (plusieurs ordinateurs) :
/// son état et son point de couleur comme dans le menu de l'icône, et ce que
/// fait un clic (`menu::Pick`).
#[derive(Clone, Serialize)]
struct BarComputer {
    id: String,
    name: String,
    /// `online`, `away`, `offline`, `searching`.
    state: &'static str,
    /// `ok`, `bad`, `warn`, `faint`.
    dot: &'static str,
    /// `show`, `connect`, `full`, `wake`, `waking`, `retry`, `busy`.
    pick: &'static str,
    /// Droit « Regarder » seul (ordinateur d'un compte) : « Regarder » (mot du
    /// cœur) ; un clic regarde au lieu de se connecter.
    watch: Option<String>,
}

fn bar_computer_list(app: &AppHandle) -> Vec<BarComputer> {
    let s = tray_state(app);
    s.computers
        .iter()
        .map(|c| {
            let (seen, dot) = menu::status(c);
            BarComputer {
                id: c.id.clone(),
                name: c.name.clone(),
                state: seen.code(),
                dot: match dot {
                    menu::Dot::Ok => "ok",
                    menu::Dot::Bad => "bad",
                    menu::Dot::Warn => "warn",
                    menu::Dot::Faint => "faint",
                },
                pick: menu::pick(&s, c).code(),
                watch: c.watch.as_ref().filter(|w| w.only).map(|w| w.word.clone()),
            }
        })
        .collect()
}

/// La liste du menu « Ordinateurs » à la barre (qui écarte l'ordinateur de sa
/// session), seulement quand elle est affichée.
fn emit_bar_computers(app: &AppHandle) {
    if app.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false) {
        let _ = app.emit_to("bar", "bar-computers", bar_computer_list(app));
    }
}

/// Le menu « Ordinateurs » de la barre s'ouvre.
#[tauri::command(async)]
fn bar_computers(app: AppHandle) -> Vec<BarComputer> {
    bar_computer_list(&app)
}

/// Une autre ouverture (raccourci, lien, barre des tâches, notification) :
/// l'accueil revient devant et reçoit la demande.
fn handle_args(app: &AppHandle, args: Vec<String>) {
    match args.first().map(String::as_str) {
        // Ouverture de session alors que l'app tourne déjà : rien à montrer.
        Some(login::BACKGROUND) => return,
        // Installateur ou désinstalleur : l'app se ferme (une session d'abord).
        Some("--quit") => return quit(app),
        // Bouton « Mettre à jour » d'une notification : comme le menu de l'icône.
        Some(link) if crate::link::action(link).is_some_and(|(a, _)| a == "station-update") => {
            if let Some((_, Some(id))) = crate::link::action(link) {
                tray_command(app, Cmd::Update(id));
            }
            return;
        }
        // Le retour de la page de connexion d'un compte KyberNow (avant les liens
        // de partage) : le code s'échange, l'accueil revient devant.
        Some(link) if kybernow_core::account::is_callback(link) => {
            account::callback(app, link);
            return show_action(app, Action::Open);
        }
        // Une notification de demande à rejoindre ou à emprunter : le panneau de « Partager »
        // ou de « Prêter l'ordinateur… » de cette session.
        Some(link) if crate::link::action(link).is_some_and(|(a, _)| a == "share" || a == "lend") => {
            if let Some((panel, Some(id))) = crate::link::action(link)
                && share::open_panel(app, &id, if panel == "lend" { "lend" } else { "share" })
            {
                return;
            }
        }
        _ => {}
    }
    show_action(app, parse_action(&args).unwrap_or(Action::Open));
}

/// L'accueil revient devant et reçoit `action` (gardée aussi pour
/// l'interface, si elle n'écoute pas encore). Une connexion pendant une
/// session (`--connect`, liste de la barre des tâches, notification, menu de
/// l'icône) ouvre une session de plus sans passer par l'accueil : sa connexion
/// se montre dans sa propre fenêtre (plusieurs ordinateurs) ; un ordinateur
/// déjà en session montre sa fenêtre.
fn show_action(app: &AppHandle, action: Action) {
    if let Action::Connect { id } = &action {
        let state = app.state::<UiState>();
        let busy = !state.sessions.locked().is_empty() || !state.launching.locked().is_empty();
        if busy {
            let (a, id) = (app.clone(), id.clone());
            std::thread::spawn(move || {
                if let Err(e) = connect(a, id) {
                    tray::alert(&e);
                }
            });
            return;
        }
    }
    if !matches!(action, Action::Open) {
        *app.state::<UiState>().action.locked() = Some(action);
    }
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = a.get_webview_window("main") {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
        // L'interface la reprend par take_action (une seule fois).
        let _ = a.emit_to("main", "action-ready", ());
    });
}

#[tauri::command(async)]
fn take_action(state: tauri::State<'_, UiState>) -> Option<Action> {
    state.action.locked().take()
}

#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    channel: Channel,
    /// Choix de la langue (`auto`, `fr`, `en`) et langue appliquée (`fr`, `en`).
    language: &'static str,
    lang: &'static str,
    update: Option<Available>,
    /// Sur batterie : `alleger` ou `garder`.
    battery: &'static str,
    /// « Ouvrir à l'ouverture de session », tel que Windows le fera.
    open_at_login: bool,
    /// « Synchroniser les réglages des ordinateurs » et son aide (textes du cœur).
    sync_settings: bool,
    sync_settings_title: String,
    sync_settings_help: String,
}

#[tauri::command(async)]
fn app_info(state: tauri::State<'_, UiState>) -> AppInfo {
    let prefs = state.prefs.locked().clone();
    let (sync_title, sync_help) = kybernow_core::account::sync_settings_texts(i18n::lang());
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        channel: prefs.channel,
        language: language_choice(&prefs),
        lang: i18n::lang().code(),
        update: state.update.locked().clone(),
        battery: battery_choice(&state),
        open_at_login: login::enabled(),
        sync_settings: prefs.sync_settings.unwrap_or(true),
        sync_settings_title: sync_title.clone(),
        sync_settings_help: sync_help.clone(),
    }
}

/// « Ouvrir à l'ouverture de session » dans les réglages de l'app. Rend
/// l'état tel que Windows le fera.
#[tauri::command(async)]
fn set_open_at_login(app: AppHandle, on: bool) -> bool {
    let state = app.state::<UiState>();
    update_prefs(&state, |p| p.open_at_login = Some(on));
    login::set(on)
}

/// « Sur batterie » dans les réglages de l'app : `alleger` ou `garder`.
#[tauri::command(async)]
fn set_battery(app: AppHandle, battery: String) -> &'static str {
    let state = app.state::<UiState>();
    update_prefs(&state, |p| p.battery = Some(if battery == "garder" { "garder" } else { "alleger" }.into()));
    battery_choice(&state)
}

/// Cherche une version plus récente sur le canal choisi.
fn check_now(app: &AppHandle) -> Result<Option<Available>, String> {
    let state = app.state::<UiState>();
    let channel = state.prefs.locked().channel;
    let found = update::check(env!("CARGO_PKG_VERSION"), channel, INSTALLER)?;
    let fresh = found.is_some() && state.update.locked().as_ref().map(|u| &u.version) != found.as_ref().map(|u| &u.version);
    *state.update.locked() = found.clone();
    let _ = app.emit_to("main", "update", &found);
    if fresh && !shell::app_in_front() {
        if let Some(u) = &found {
            shell::notify(&t("notify.update_title", &[("version", &u.version)]), &t("notify.update_body", &[]), None);
        }
    }
    Ok(found)
}

#[tauri::command(async)]
fn check_update(app: AppHandle) -> Result<Option<Available>, String> {
    check_now(&app)
}

#[tauri::command(async)]
fn set_channel(app: AppHandle, channel: Channel) -> Result<Option<Available>, String> {
    let state = app.state::<UiState>();
    update_prefs(&state, |p| p.channel = channel);
    check_now(&app)
}

/// Langue choisie dans les réglages de l'app (`auto`, `fr`, `en`) : appliquée
/// tout de suite aux deux fenêtres, aux messages du côté Rust, aux notes de
/// version et à la liste de la barre des tâches. Rend la langue appliquée.
#[tauri::command(async)]
fn set_language(app: AppHandle, language: String) -> &'static str {
    let state = app.state::<UiState>();
    let prefs = update_prefs(&state, |p| {
        p.language = language;
        p.language = language_choice(p).to_string();
        p.clone()
    });
    let lang = i18n::apply(&prefs.language);
    log::info!("langue : {} ({})", prefs.language, lang.code());
    let update = {
        let mut u = state.update.locked();
        if let Some(u) = u.as_mut() {
            u.localize(lang);
        }
        u.clone()
    };
    let _ = app.emit("lang", lang.code());
    let _ = app.emit_to("main", "update", &update);
    emit_computers(&app);
    lang.code()
}

/// Télécharge l'installateur vérifié, le lance en silence et quitte : il
/// attend la fermeture de l'app, remplace ses fichiers et la relance.
#[tauri::command(async)]
fn install_update(app: AppHandle) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let state = app.state::<UiState>();
    if !can_quit(&state) {
        return Err(t("errors.session_open_install_later", &[]));
    }
    let u = state.update.locked().clone().ok_or_else(|| t("errors.no_update", &[]))?;
    let dir = std::env::temp_dir().join("KyberNow");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("KyberNow-{}-{}", u.version, u.file));
    // Installateurs des mises à jour précédentes.
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        if e.path() != path && e.file_name().to_string_lossy().ends_with(".exe") {
            let _ = std::fs::remove_file(e.path());
        }
    }
    let size = u.size.max(1);
    let a = app.clone();
    let last = std::sync::atomic::AtomicU64::new(u64::MAX);
    update::download(&u, &path, move |got| {
        let pct = got * 100 / size;
        if last.swap(pct, std::sync::atomic::Ordering::Relaxed) != pct {
            let _ = a.emit_to("main", "update-progress", pct);
        }
    })?;
    // Une session ou une partie a pu démarrer pendant le téléchargement :
    // l'installateur fermerait l'app et la couperait.
    if !can_quit(&state) {
        log::info!("mise à jour {} prête, pas installée : une session a commencé entre-temps", u.version);
        return Err(t("errors.session_open_install_later", &[]));
    }
    log::info!("mise à jour {} vérifiée, installation", u.version);
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    // Hors du « job » de l'app si on peut (un lanceur qui tue ses enfants à
    // la fermeture emporterait l'installateur), sinon tel quel.
    // Pas le dossier de l'app comme dossier courant : l'installateur doit pouvoir le remplacer.
    let spawn = |flags: u32| std::process::Command::new(&path).args(["/S", "/UPDATE"]).current_dir(&dir).creation_flags(flags).spawn();
    spawn(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP))
        .map_err(|e| t("errors.installer_launch", &[("error", &e.to_string())]))?;
    app.exit(0);
    Ok(())
}

fn console_for(c: &Computer) -> Console {
    // Un ordinateur d'un compte KyberNow : le jeton d'accès du relais, jamais de mot de passe.
    if let Some(ac) = &c.account {
        return account::console(c, ac);
    }
    let password = store::password(&c.id).unwrap_or_default();
    Console::new(&c.host, c.port, &c.user, &password, c.fingerprint.clone())
}

/// [`console_for`], par le relais quand `via_relay` et que l'on connaît la route
/// de cet ordinateur (son relais, son identifiant chez lui, l'empreinte d'Admin
/// de l'app) : Admin par le tunnel, épinglé.
fn console_via(c: &Computer, via_relay: bool) -> Console {
    let console = console_for(c);
    match relais::route_of(c.relay.as_ref(), c.fingerprint.as_deref()).filter(|_| via_relay) {
        Some(route) => console.with_relay(Some(route)),
        None => console,
    }
}

/// L'ordinateur ne répond pas sur le réseau : le relais le tient-il (« Hors de
/// chez vous ») ? Demandé au relais toutes les 30 s au plus par ordinateur
/// (`relais::Presence`) ; un relais qui ne répond pas, ou aucun relais connu
/// (ni `info.relay`, ni empreinte, ou `KYBERNOW_RELAY=off`) : non, l'ordinateur
/// reste « Hors ligne ».
fn away_by_relay(app: &AppHandle, c: &Computer) -> bool {
    if relais::off() {
        return false;
    }
    let Some(route) = relais::route_of(c.relay.as_ref(), c.fingerprint.as_deref()) else { return false };
    let state = app.state::<UiState>();
    let now = Instant::now();
    let ask = state.presence.locked().ask(&route.station, now);
    let present = match ask {
        relais::Ask::Known(present) => present,
        relais::Ask::Due => {
            let present = relais::present(&route);
            state.presence.locked().record(&route.station, now, present);
            present
        }
    };
    present == Some(true)
}

/// Interroge Admin d'un ordinateur : en ligne ou non, ce qu'il dit de lui
/// (`info`, Station comprise) et un aperçu de son écran, une fois par minute
/// au plus. Rend vrai si l'accueil doit le revoir.
fn refresh(app: &AppHandle, c: &Computer) -> bool {
    let state = app.state::<UiState>();
    // Un ordinateur d'un compte : son adresse sur le réseau, d'après la découverte.
    let located = account::locate(app, c);
    let c = located.as_ref().unwrap_or(c);
    let console = console_for(c);
    let mut problem = None;
    let info = if c.account.is_some() && c.host.is_empty() {
        // Jamais vu sur le réseau : seulement par le relais (présence, plus bas).
        None
    } else {
        match console.info() {
            Ok(i) => Some(i),
            // Le relais refuse le jeton d'accès : compte oublié, ou ordinateur retiré du compte.
            Err(e) if account::on_console_error(app, c, &e) => return true,
            // Il répond, mais refuse le compte, ou n'a plus le même certificat :
            // en ligne, l'erreur sous son nom.
            Err(console::ConsoleError::Http { status: 401 | 403, .. }) => {
                problem = Some(Problem::PasswordRefused);
                None
            }
            Err(console::ConsoleError::CertificateChanged { .. }) => {
                problem = Some(Problem::Reinstalled);
                None
            }
            Err(_) => None,
        }
    };
    // Hors de chez soi : pas de réponse sur le réseau, mais le relais tient l'ordinateur.
    let away = info.is_none() && problem.is_none() && away_by_relay(app, c);
    let online = Some(info.is_some() || away || problem.is_some());
    // Son relais, gardé avec lui pour l'ouvrir hors de chez soi (une Station sans
    // relais réglé, ou antérieure, l'efface). Un ordinateur d'un compte garde la
    // route du relais de son compte.
    let mut relay_changed = false;
    if let Some(i) = info.as_ref().filter(|i| c.account.is_none() && i.relay != c.relay) {
        let relay = i.relay.clone();
        relay_changed = state.with_store(|s| {
            let found = s.get_mut(&c.id).map(|m| m.relay = relay).is_some();
            if found {
                s.save();
            }
            found
        });
        // L'ancien identifiant chez le relais ne dit plus rien.
        if let (true, Some(old)) = (relay_changed, c.relay.as_ref()) {
            state.presence.locked().forget(&old.id);
        }
    }
    // Les cartes réseau pour le réveil, gardées avec l'ordinateur pour le
    // réveiller une fois éteint (le fichier n'est réécrit que si elles changent).
    let mut wake_changed = false;
    if let Some(i) = info.as_ref().filter(|i| !i.wake.is_empty() && i.wake != c.wake) {
        let wake = i.wake.clone();
        wake_changed = state.with_store(|s| {
            let found = s.get_mut(&c.id).map(|m| m.wake = wake).is_some();
            if found {
                s.save();
            }
            found
        });
    }
    if let Some(i) = &info {
        station_news(app, c, i);
        // Consoles PlayStation : celles que cette Station voit (hors de chez soi).
        playstation::learn(app, &c.id, &console);
    }
    // Prêté à un ami : « Prêté à {prénom} » sur l'accueil.
    let lend_view = info.as_ref().and_then(|i| lend::home_view(&console, i));
    let mut preview = None;
    let wants_preview = {
        let live = state.live.locked();
        // Pas d'aperçu quand la dernière image de la session est là (comme sur Mac).
        live.get(&c.id).is_none_or(|l| l.last.is_none() && l.preview_at.is_none_or(|t| t.elapsed() > Duration::from_secs(60)))
    };
    if info.is_some() && wants_preview {
        if let Ok(jpeg) = console.preview(1280) {
            if !jpeg.is_empty() {
                preview = Some(Image::new(jpeg));
            }
        }
    }
    let mut live = state.live.locked();
    let l = live.entry(c.id.clone()).or_default();
    // La Station suivie après un redémarrage ou une mise à jour : son état
    // (version, installation, redémarrage) compte aussi.
    let station_changed = info.as_ref().is_some_and(|i| l.info.as_ref().map(|o| &o.station) != Some(&i.station));
    let lend_changed = l.lend != lend_view;
    let changed = l.online != online
        || l.away != away
        || l.problem != problem
        || preview.is_some()
        || station_changed
        || wake_changed
        || relay_changed
        || lend_changed
        || located.is_some();
    l.online = online;
    l.problem = problem;
    l.away = away;
    l.lend = lend_view;
    let on_network = info.is_some();
    if info.is_some() {
        l.info = info;
    }
    if let Some(p) = preview {
        l.preview = Some(p);
        l.preview_at = Some(Instant::now());
    }
    drop(live);
    // La Station vérifie ses mises à jour quand l'app a changé de version (ou qu'elle est en retard sur l'app).
    if on_network {
        stations::check(app, c);
    }
    changed
}

/// Une version de la Station prête (installable depuis l'app, ou avec
/// l'installateur) : une notification, une fois par version et par
/// ordinateur, quand l'app n'est pas au premier plan.
fn station_news(app: &AppHandle, c: &Computer, info: &Info) {
    let Some(u) = info.station.as_ref().map(|s| &s.update) else { return };
    let ready = u.available && !u.installing && (u.installable || u.requires_setup);
    let Some(v) = u.version.as_deref().filter(|_| ready) else { return };
    let state = app.state::<UiState>();
    if c.station_notified.as_deref() == Some(v) || state.following.locked().contains_key(&c.id) || shell::app_in_front() {
        return;
    }
    let title = t("notify.station_update_title", &[("name", &c.name)]);
    let station = format!("kybernow://station?id={}", c.id);
    if u.installable {
        let button = t("notify.update", &[]);
        let link = format!("kybernow://station-update?id={}", c.id);
        shell::notify_to(&station, &title, &t("notify.station_update_body", &[("version", v)]), Some((&button, &link)));
    } else {
        shell::notify_to(&station, &title, &t("notify.station_update_setup", &[("version", v)]), None);
    }
    log::info!("notification : Station {v} prête pour {}", c.name);
    state.with_store(|s| {
        if let Some(m) = s.get_mut(&c.id) {
            m.station_notified = Some(v.to_string());
            s.save();
        }
    });
}

/// Ordinateurs interrogés en même temps par le relevé.
const POLL_AT_ONCE: usize = 6;

/// Chaque ordinateur, sauf ceux en session : toutes les 8 s quand
/// l'accueil est affiché, toutes les 30 s sans lui (l'app dans l'icône), et
/// tout de suite à l'ouverture du menu de l'icône (`poll_soon`).
fn poll(app: AppHandle) {
    loop {
        let state = app.state::<UiState>();
        let in_session = state.session_ids();
        let computers: Vec<Computer> = state.with_store(|s| s.computers.clone()).into_iter().filter(|c| !in_session.contains(&c.id)).collect();
        // Interrogés ensemble (six à la fois) : un ordinateur éteint (≈ 4 s
        // sans réponse) ne retarde plus les autres ; un seul envoi à la fin.
        let mut changed = false;
        for batch in computers.chunks(POLL_AT_ONCE) {
            changed |= std::thread::scope(|scope| {
                let jobs: Vec<_> = batch.iter().map(|c| scope.spawn(|| refresh(&app, c))).collect();
                jobs.into_iter().fold(false, |any, j| j.join().unwrap_or(false) || any)
            });
        }
        if changed {
            emit_computers(&app);
        }
        let shown = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(true);
        let wait = Duration::from_secs(if shown { 8 } else { 30 });
        let asked = state.poll_asked.locked();
        let (mut asked, _) = state.poll_signal.wait_timeout_while(asked, wait, |a| !*a).unwrap();
        *asked = false;
    }
}

/// Un relevé de tous les ordinateurs tout de suite (menu de l'icône ouvert).
pub fn poll_soon(app: &AppHandle) {
    let state = app.state::<UiState>();
    *state.poll_asked.locked() = true;
    state.poll_signal.notify_all();
}

/// Les suivis en cours (réveils, Station qui redémarre ou s'installe), toutes
/// les 2 s : l'ordinateur est interrogé jusqu'à la fin du suivi, qui se dit à
/// l'interface et, selon le cas, par une notification (docs/link-ui.md).
fn follow_loop(app: AppHandle) {
    loop {
        std::thread::sleep(follow::POLL);
        let state = app.state::<UiState>();
        let in_session = state.session_ids();
        let waking: Vec<(String, Instant)> = state.waking.locked().iter().map(|(k, v)| (k.clone(), *v)).collect();
        let following: Vec<String> = state.following.locked().keys().cloned().collect();
        if waking.is_empty() && following.is_empty() {
            continue;
        }
        let mut changed = false;
        let mut ids: Vec<&String> = waking.iter().map(|(id, _)| id).chain(&following).collect();
        ids.sort();
        ids.dedup();
        for id in ids {
            if in_session.contains(id) {
                continue;
            }
            let Some(c) = state.with_store(|s| s.get(id).cloned()) else {
                state.waking.locked().remove(id);
                state.following.locked().remove(id);
                continue;
            };
            changed |= refresh(&app, &c);
        }
        for (id, since) in waking {
            let Some(c) = state.with_store(|s| s.get(&id).cloned()) else { continue };
            let online = state.live.locked().get(&id).and_then(|l| l.online);
            let step = follow::wake(online, since.elapsed());
            if step == follow::Wake::Waiting {
                continue;
            }
            state.waking.locked().remove(&id);
            changed = true;
            log::info!("réveil de {} : {step:?}", c.name);
            if shell::app_in_front() {
                continue;
            }
            let home = format!("kybernow://open?id={id}");
            if step == follow::Wake::Online {
                let link = format!("kybernow://connect?id={id}");
                let button = t("notify.connect", &[]);
                shell::notify_to(&home, &t("notify.online_title", &[("name", &c.name)]), &t("notify.online_body", &[]), Some((&button, &link)));
            } else {
                shell::notify_to(&home, &t("notify.no_answer_title", &[("name", &c.name)]), &t("notify.no_answer_body", &[]), None);
            }
        }
        for id in following {
            let Some(c) = state.with_store(|s| s.get(&id).cloned()) else { continue };
            let (online, info) = state.live.locked().get(&id).map(|l| (l.online, l.info.clone())).unwrap_or_default();
            let seen = info.as_ref().and_then(|i| i.station.as_ref()).map(|s| follow::Seen {
                version: &s.version,
                restarting: s.restarting,
                installing: s.update.installing,
            });
            let outcome = {
                let mut f = state.following.locked();
                let Some((p, since)) = f.get_mut(&id) else { continue };
                let since = *since;
                let outcome = follow::station(p, online, seen, since.elapsed());
                outcome.map(|o| (o, f.remove(&id).map(|(p, _)| p.from_menu).unwrap_or(false)))
            };
            let Some((outcome, from_menu)) = outcome else { continue };
            changed = true;
            log::info!("Station de {} : {outcome:?}", c.name);
            #[derive(Serialize, Clone)]
            struct StationOutcome<'a> {
                id: &'a str,
                outcome: &'static str,
                version: Option<&'a str>,
            }
            let version = match &outcome {
                follow::Outcome::Updated(v) => Some(v.as_str()),
                _ => None,
            };
            let _ = app.emit_to("main", "station-outcome", StationOutcome { id: &id, outcome: outcome.code(), version });
            let text = match &outcome {
                follow::Outcome::Restarted => t("station.restarted", &[]),
                follow::Outcome::Updated(v) => t("station.updated", &[("version", v)]),
                follow::Outcome::UpdateFailed => t("station.update_failed", &[]),
                follow::Outcome::SlowBack => t("station.slow_back", &[]),
                follow::Outcome::Silent => continue,
            };
            if from_menu && !shell::app_in_front() {
                shell::notify_to(&format!("kybernow://station?id={id}"), &c.name, &text, None);
            }
        }
        if changed {
            emit_computers(&app);
        }
    }
}

/// Un ordinateur tout de suite : « Réessayer » à l'accueil, et le suivi de
/// la Station qui redémarre ou se met à jour (plus souvent que toutes les 8 s).
#[tauri::command(async)]
fn poll_now(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    if state.in_session(&id) {
        return Ok(());
    }
    let c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    refresh(&app, &c);
    // Toujours : l'accueil sort de « Recherche… » même si rien n'a changé.
    emit_computers(&app);
    Ok(())
}

/// Les écrans, vus par l'accueil et les réglages d'un ordinateur : ce que le
/// cœur en dit (`Screens::choice`, `screens_profile`, `session_plan_for`), le
/// même calcul que la session.
#[derive(Serialize)]
struct DisplaysInfo {
    /// Le réglage « Écrans » se change ; sinon il est inactif, sur « Cet
    /// écran » (définition sur mesure, ou écran virtuel pas demandé ou pas
    /// utilisable sur la Station).
    enabled: bool,
    /// Le réglage tel que les réglages le montrent : `auto`, `this` ou `all`.
    shown: &'static str,
    /// Écrans que la session prendra (1 : un seul).
    count: usize,
    /// L'écran virtuel servira : demandé et utilisable sur la Station ; sinon
    /// la ligne de l'accueil dit « Les écrans de l'ordinateur, tels quels ».
    virtual_display: bool,
    /// La puce « HDR » (docs/hdr.md) : cachée sans écran HDR, inactive avec la raison.
    hdr: kybernow_core::profile::HdrState,
    /// Le HDR sera demandé (allumé et possible, sur un écran HDR) : la ligne
    /// « Écran » dit « …, en HDR », « Texte net (4:4:4) » est inactif.
    hdr_on: bool,
    plan: PlanView,
}

/// Ce que la session demandera, calculé par le cœur (`plan::plan`, WINAPP-52 ;
/// l'interface n'en refait aucun calcul) : l'accueil et les réglages le
/// montrent tel quel.
#[derive(Serialize)]
struct PlanView {
    /// Le profil retouché est « Personnalisé » (règle du cœur).
    custom: bool,
    /// Les valeurs du profil choisi sur ce PC (`plan::profile_settings` ;
    /// Auto : celles du test, celles de Jeu sans test), en JSON du cœur
    /// (`StreamSettings::to_json`) : ce que les réglages montrent tant qu'ils
    /// ne sont pas retouchés.
    base: serde_json::Value,
    /// Le mode demandé à l'ordinateur (au plus 4K, H.264, sur batterie,
    /// définition sur mesure).
    width: u32,
    height: u32,
    refresh: u32,
    /// Le débit de la session pour ce mode (Mbit/s).
    mbps: u32,
    /// « Jeu · HEVC », « Personnalisé · HEVC 4:4:4 ».
    label: String,
}

/// `settings` : ceux de l'ordinateur `id`, enregistrés ou en cours de
/// réglage (la ligne de l'accueil et le réglage se mettent à jour à chaque
/// changement). Les écrans de ce PC sont relevés à chaque appel.
#[tauri::command(async)]
fn displays_info(app: AppHandle, id: String, settings: Settings) -> Result<DisplaysInfo, String> {
    let state = app.state::<UiState>();
    let mut c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    c.settings = settings;
    let pc = state.this_pc.locked().clone().unwrap_or_default();
    let lighten = battery_choice(&state) == "alleger" && pc::on_battery();
    let (stream, mode, label) = session_plan(&c, &pc, lighten, None);
    let base = kybernow_core::plan::profile_settings(Profile::parse(&c.settings.profile).unwrap_or(Profile::Jeu), c.auto.as_ref(), capabilities(&pc));
    let planned = PlanView {
        custom: stream.profile.is_none(),
        base: base.to_json(),
        width: mode.width,
        height: mode.height,
        refresh: mode.refresh,
        mbps: stream.bitrate(mode.width, mode.height, mode.refresh) / 1_000_000,
        label,
    };
    let info = state.live.locked().get(&id).and_then(|l| l.info.clone());
    let screens = Layout::now(displays_spec(&c.settings, info.as_ref(), lighten, &pc)).screens();
    let choice = screens.choice(screens::screens_profile(&stream, screens.capabilities), c.settings.virtual_display);
    let plan = screens::session_plan_for(&screens, &stream, c.settings.virtual_display);
    let count = plan.as_ref().map_or(1, |p| p.len());
    let devices: Option<Vec<usize>> = plan.map(|p| p.iter().map(|x| x.device).collect());
    let (screen_hdr, _) = screens.session_hdr(devices.as_deref());
    let hdr = hdr_chip(info.as_ref(), &stream, &pc, screen_hdr);
    let hdr_on = hdr.shown && hdr.available && c.settings.hdr_on();
    Ok(DisplaysInfo { enabled: choice.setting_enabled(), shown: choice.shown_setting().as_str(), count, virtual_display: choice.virtual_display, hdr, hdr_on, plan: planned })
}

/// « Envoyer le journal » (Réglages de l'app › À propos, docs/link-ui.md) :
/// le fichier `kybernow-journal.txt` (`journal.rs`), puis la fenêtre
/// « Partager » de Windows au-dessus de l'accueil. Le temps de la préparation,
/// l'interface dit « Préparation du journal… ».
#[tauri::command(async)]
fn share_log(app: AppHandle) -> Result<(), String> {
    let failed = |e: String| {
        log::warn!("journal : {e}");
        t("errors.log_failed", &[])
    };
    let path = crate::journal::write().map_err(|e| failed(e.to_string()))?;
    let window = app.get_webview_window("main").ok_or_else(|| failed("fenêtre de l'accueil absente".into()))?;
    let hwnd = window.hwnd().map_err(|e| failed(e.to_string()))?.0 as isize;
    log::info!("journal : préparé ({}), fenêtre « Partager » de Windows", path.display());
    crate::journal::share(&path, hwnd, |f| {
        let _ = app.run_on_main_thread(f);
    })
    .map_err(failed)
}

/// Les valeurs communes du cœur (`plan::values_json`, TRANS-40 à 42) : bornes
/// du débit réglé à la main, nombre de sessions, délais, port et exemple
/// d'adresse d'Admin, relais public… Lues par chaque fenêtre de l'interface
/// à son lancement : elle n'en garde aucune copie.
#[tauri::command(async)]
fn common_values(state: tauri::State<'_, UiState>) -> serde_json::Value {
    let pc = state.this_pc.locked().clone().unwrap_or_default();
    kybernow_core::plan::values_json(capabilities(&pc))
}

/// Les écrans de ce PC ont changé (branché, débranché, disposition) : l'accueil
/// revoit ce que la session prendra (événement `displays`), et la définition
/// de « Cet écran ».
pub fn displays_changed(app: &AppHandle) {
    refresh_this_screen(app);
    let _ = app.emit_to("main", "displays", ());
}

/// Le dernier « Cet écran » vu (nom du périphérique de l'écran de l'accueil).
static HOME_SCREEN: Mutex<Option<String>> = Mutex::new(None);

/// L'accueil a bougé : s'il a changé d'écran, « Cet écran » change, et avec
/// lui la définition que la session demandera.
fn home_moved(app: &AppHandle) {
    let key = system::home_monitor_key();
    {
        let mut last = HOME_SCREEN.locked();
        if *last == key {
            return;
        }
        *last = key;
    }
    refresh_this_screen(app);
}

/// La définition et la fréquence de « Ce PC » (ligne de l'accueil, débit des
/// réglages) relues sur « Cet écran », l'écran de l'accueil maintenant : la
/// session les relit au même endroit à la connexion (`system::screen_mode`),
/// l'accueil annonce donc ce qu'elle demandera.
fn refresh_this_screen(app: &AppHandle) {
    let mode = system::screen_mode();
    let state = app.state::<UiState>();
    let pc = {
        let mut cached = state.this_pc.locked();
        // Pas encore relevé : la sonde du lancement lira l'écran à sa fin.
        let Some(pc) = cached.as_mut() else { return };
        if (pc.width, pc.height, pc.refresh) == (mode.width, mode.height, mode.refresh) {
            return;
        }
        (pc.width, pc.height, pc.refresh) = (mode.width, mode.height, mode.refresh);
        pc.clone()
    };
    log::info!("cet écran : {} × {} à {} Hz", mode.width, mode.height, mode.refresh);
    let _ = app.emit_to("main", "this-pc", pc);
}

/// Ce que montre le fond de la connexion d'un écran (`fond.html`).
#[tauri::command]
fn fond_state(window: tauri::WebviewWindow, state: tauri::State<'_, UiState>) -> fonds::FondState {
    state.fonds.get().map_or(fonds::FondState { not_created: false }, |f| f.state_of(window.label()))
}

#[tauri::command(async)]
fn snapshot(app: AppHandle, state: tauri::State<'_, UiState>) -> Snapshot {
    let mut sessions = state.session_ids();
    sessions.extend(playstation::session_ids(&app));
    Snapshot {
        computers: state.views(),
        discovered: fresh_found(&state),
        this_pc: state.this_pc.locked().clone(),
        sessions,
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

/// Premier contact par adresse : l'empreinte de la console, à comparer à
/// celle qu'elle affiche.
#[tauri::command(async)]
fn probe_address(host: String, port: u16) -> Result<Candidate, String> {
    let console = Console::new(host.trim(), port, "", "", None);
    let info = console.probe().map_err(|e| e.to_string())?;
    let fingerprint = console.fingerprint_seen().ok_or_else(|| t("errors.no_certificate", &[]))?;
    Ok(Candidate::new(host.trim().into(), port, info.map(|i| i.hostname).filter(|n| !n.is_empty()), Some(fingerprint)))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                    out.push(h << 4 | l);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `kybernow://adresse:port?name=…&fp=sha256:…` (lien ou QR code de la console).
pub fn parse_link_text(link: &str) -> Result<Candidate, String> {
    let rest = link.trim().strip_prefix("kybernow://").ok_or_else(|| t("errors.not_a_link", &[]))?;
    let (authority, query) = rest.split_once('?').unwrap_or((rest, ""));
    let authority = authority.trim_end_matches('/');
    // L'adresse, par la règle du cœur (TRANS-35) ; le port est exigé : sans
    // lui (mais lisible avec), « no_port ».
    let (host, port) = console::parse_address(authority, None).ok_or_else(|| {
        if console::parse_address(authority, Some(console::ADMIN_PORT)).is_some() { t("errors.no_port", &[]) } else { t("errors.bad_address", &[]) }
    })?;
    let mut name = None;
    let mut fingerprint = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "name" => name = Some(percent_decode(v)),
            "fp" => fingerprint = Some(percent_decode(v)),
            _ => {}
        }
    }
    Ok(Candidate::new(host, port, name, fingerprint))
}

#[tauri::command(async)]
fn parse_link(link: String) -> Result<Candidate, String> {
    parse_link_text(&link)
}

/// Vérifie le compte auprès de la console (et son certificat), puis retient
/// l'ordinateur et son mot de passe.
fn add(app: &AppHandle, candidate: &Candidate, user: &str, password: &str, by_code: bool) -> Result<String, String> {
    let console = Console::new(&candidate.host, candidate.port, user, password, candidate.fingerprint.clone());
    let info = console.info().map_err(|e| e.to_string())?;
    let fingerprint = candidate.fingerprint.clone().or_else(|| console.fingerprint_seen());
    let state = app.state::<UiState>();
    // Le mot de passe d'abord : s'il ne peut pas être gardé, l'ordinateur
    // n'est pas ajouté (il apparaîtrait, et chaque connexion échouerait), et
    // un ordinateur déjà connu garde son compte.
    let id = state
        .with_store(|s| s.computers.iter().find(|o| o.host == candidate.host && o.port == candidate.port).map(|o| o.id.clone()))
        .unwrap_or_else(store::new_id);
    if !store::set_password(&id, user, password) {
        return Err(t("errors.password_store", &[]));
    }
    let id = state.with_store(|s| {
        s.upsert(Computer {
            id: id.clone(),
            name: if info.hostname.is_empty() {
                candidate.name.clone().unwrap_or_else(|| candidate.host.clone())
            } else {
                info.hostname.clone()
            },
            host: candidate.host.clone(),
            port: candidate.port,
            user: user.into(),
            fingerprint,
            os: info.os.clone(),
            gpu: info.gpu.clone(),
            settings: Settings::default(),
            last: None,
            by_code,
            wake: info.wake.clone(),
            station_notified: None,
            camera_offered: false,
            station_offer: Default::default(),
            relay: info.relay.clone(),
            account: None,
            auto: None,
            encoder_max: None,
        })
    });
    state.live.locked().insert(id.clone(), Live { online: Some(true), info: Some(info), ..Default::default() });
    emit_computers(app);
    Ok(id)
}

#[tauri::command(async)]
fn add_computer(app: AppHandle, candidate: Candidate, user: String, password: String) -> Result<String, String> {
    add(&app, &candidate, user.trim(), &password, false)
}

/// Code à six chiffres envoyé à la console, à y accepter.
#[tauri::command(async)]
fn pair_start(state: tauri::State<'_, UiState>, candidate: Candidate) -> Result<String, String> {
    let mut b = [0u8; 4];
    let _ = getrandom::getrandom(&mut b);
    let code = format!("{:06}", u32::from_le_bytes(b) % 1_000_000);
    let console = Console::new(&candidate.host, candidate.port, "", "", candidate.fingerprint.clone());
    let id = console.pair_request(&code, &system::device_info()).map_err(|e| e.to_string())?;
    *state.pairing.locked() = Some(Pairing { candidate, id });
    Ok(code)
}

/// `pending`, `refused`, `expired`, ou l'identifiant de l'ordinateur ajouté.
#[tauri::command(async)]
fn pair_poll(app: AppHandle) -> Result<String, String> {
    let state = app.state::<UiState>();
    let (candidate, id) = match state.pairing.locked().as_ref() {
        Some(p) => (p.candidate.clone(), p.id.clone()),
        None => return Ok("expired".into()),
    };
    let console = Console::new(&candidate.host, candidate.port, "", "", candidate.fingerprint.clone());
    match console.pair_status(&id).map_err(|e| e.to_string())? {
        PairStatus::Pending => Ok("pending".into()),
        PairStatus::Refused => Ok("refused".into()),
        PairStatus::Expired => Ok("expired".into()),
        PairStatus::Accepted { username, password } => {
            // La demande n'est oubliée qu'une fois l'ordinateur ajouté : la
            // console ne redonne pas ce compte.
            let id = add(&app, &candidate, &username, &password, true)?;
            *state.pairing.locked() = None;
            Ok(id)
        }
    }
}

#[tauri::command(async)]
fn pair_cancel(state: tauri::State<'_, UiState>) {
    *state.pairing.locked() = None;
}

/// Admin de l'ordinateur `id` (compte et empreinte retenus).
///
/// Par le relais quand la session de cet ordinateur y passe (« Partager », « Accepter »
/// d'une session ouverte hors de chez soi), ou quand il n'est joignable que par lui
/// (« Hors de chez vous ») ; sur le réseau sinon.
fn console_of(app: &AppHandle, id: &str) -> Result<Console, String> {
    let state = app.state::<UiState>();
    let c = state.with_store(|s| s.get(id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    Ok(console_via(&c, computer_via_relay(app, id)))
}

/// Par le relais : l'ordinateur n'est joignable que par lui (« Hors de chez vous »), ou sa
/// session y passe (puce « Par le relais »). Admin refuse alors le redémarrage et la mise à
/// jour de la Station.
fn computer_via_relay(app: &AppHandle, id: &str) -> bool {
    let state = app.state::<UiState>();
    let session_via_relay = state.with_session(id, |r| r.route.locked().as_ref().is_some_and(RouteInfo::via_relay)).unwrap_or(false);
    let away = state.live.locked().get(id).is_some_and(|l| l.away);
    session_via_relay || away
}

/// Refus d'une commande de la Station, comme `StationError::to_json` : `kind`
/// (`in_session` : redemander avec `force` coupe la session de `device` ;
/// `too_soon` : attendre `seconds` ; `nothing_to_install`, `requires_setup`,
/// `busy`, `unsupported`, `other`) et `message`, dans la langue de l'app.
fn station_refusal(message: String) -> serde_json::Value {
    serde_json::json!({ "kind": "other", "message": message, "device": null, "seconds": null })
}

/// L'état de la Station change tout de suite à l'accueil (redémarrage ou
/// installation en cours), sans attendre le relevé suivant.
fn mark_station(app: &AppHandle, id: &str, change: impl FnOnce(&mut kybernow_core::console::StationInfo)) {
    let state = app.state::<UiState>();
    if let Some(station) = state.live.locked().get_mut(id).and_then(|l| l.info.as_mut()).and_then(|i| i.station.as_mut()) {
        change(station);
    }
    emit_computers(app);
}

/// Refus d'un redémarrage ou d'une mise à jour : celui d'Admin, ou un
/// texte (ordinateur inconnu).
enum Refusal {
    Station(StationError),
    Text(String),
}

impl Refusal {
    fn to_json(&self) -> serde_json::Value {
        match self {
            Refusal::Station(e) => e.to_json(),
            Refusal::Text(m) => station_refusal(m.clone()),
        }
    }
}

/// Redémarre la Station de `id` (Admin et le serveur Kyber s'arrêtent puis
/// repartent, Kyber d'abord : environ 30 secondes, pendant lesquelles Admin
/// ne répond pas) ou la met à jour (la même installation qu'« Installer
/// maintenant » dans Admin ; bloque le temps qu'Admin télécharge le paquet).
/// `force` : même si un appareil est en session (elle est coupée). L'app suit
/// ensuite la Station jusqu'à son retour (`follow_loop`), que la demande
/// vienne des réglages ou du menu de l'icône (`from_menu` : une notification
/// à la fin). Rend la version installée.
fn station_call(app: &AppHandle, id: &str, kind: Kind, force: bool, from_menu: bool) -> Result<Option<String>, Refusal> {
    // Par le relais, Admin refuse le redémarrage et la mise à jour : l'app ne les demande pas, et dit pourquoi.
    if computer_via_relay(app, id) {
        return Err(Refusal::Text(t("station.via_relay", &[])));
    }
    let console = console_of(app, id).map_err(Refusal::Text)?;
    let device = system::device_info().id;
    let version = match kind {
        Kind::Restart => {
            console.station_restart(Some(&device), force).map_err(Refusal::Station)?;
            log::info!("redémarrage de la Station de {} demandé", console.host());
            None
        }
        Kind::Update => {
            let v = console.station_update(Some(&device), force).map_err(Refusal::Station)?;
            log::info!("mise à jour de la Station de {} vers {v} lancée", console.host());
            Some(v)
        }
    };
    let state = app.state::<UiState>();
    let known = state.live.locked().get(id).and_then(|l| l.info.as_ref()?.station.as_ref()?.update.version.clone());
    let expected = version.clone().filter(|v| !v.is_empty()).or(known).filter(|_| kind == Kind::Update);
    let pending = follow::Pending { kind, version: expected, down: false, from_menu };
    state.following.locked().insert(id.to_string(), (pending, Instant::now()));
    mark_station(app, id, |s| match kind {
        Kind::Restart => s.restarting = true,
        Kind::Update => s.update.installing = true,
    });
    Ok(version)
}

/// « Redémarrer la Station » dans les réglages de l'ordinateur `id`.
/// Refus : voir `station_refusal`.
#[tauri::command(async)]
fn station_restart(app: AppHandle, id: String, force: bool) -> Result<(), serde_json::Value> {
    station_call(&app, &id, Kind::Restart, force, false).map(|_| ()).map_err(|e| e.to_json())
}

/// « Mettre à jour la Station » : rend la version installée. Refus : voir `station_refusal`.
#[tauri::command(async)]
fn station_update(app: AppHandle, id: String, force: bool) -> Result<String, serde_json::Value> {
    station_call(&app, &id, Kind::Update, force, false).map(|v| v.unwrap_or_default()).map_err(|e| e.to_json())
}

/// Redémarrer ou mettre à jour depuis le menu de l'icône (ou le bouton d'une
/// notification), comme la rubrique Station : la même confirmation quand un
/// appareil est en session, les mêmes refus, le même suivi.
fn station_from_menu(app: &AppHandle, id: &str, kind: Kind) {
    let mut force = false;
    loop {
        let refusal = match station_call(app, id, kind, force, true) {
            Ok(_) => return,
            Err(r) => r,
        };
        let text = match &refusal {
            Refusal::Station(StationError::InSession { device, .. }) if !force => {
                let title = match device {
                    Some(d) => t("station.in_session_title", &[("device", d)]),
                    None => t("station.in_session_anonymous", &[]),
                };
                let (message, yes) = match kind {
                    Kind::Restart => (t("station.in_session_restart", &[]), t("station.restart_anyway", &[])),
                    Kind::Update => (t("station.in_session_update", &[]), t("station.update_anyway", &[])),
                };
                if !tray::confirm(&title, &message, &yes) {
                    return;
                }
                force = true;
                continue;
            }
            Refusal::Station(StationError::TooSoon { .. }) => t("station.too_soon", &[]),
            Refusal::Station(StationError::Unsupported) => t("station.too_old", &[]),
            Refusal::Station(StationError::RequiresSetup { .. }) => {
                let state = app.state::<UiState>();
                let v = state.live.locked().get(id).and_then(|l| l.info.as_ref()?.station.as_ref()?.update.version.clone());
                t("station.requires_setup", &[("version", &v.unwrap_or_default())])
            }
            Refusal::Station(e) => e.to_string(),
            Refusal::Text(m) => m.clone(),
        };
        tray::alert(&text);
        return;
    }
}

/// « Ouvrir Admin » : son adresse dans le navigateur par défaut.
#[tauri::command(async)]
fn open_admin(app: AppHandle, id: String) -> Result<(), String> {
    shell::open_url(&console_of(&app, &id)?.admin_url())
}

/// « Réveiller l'ordinateur » `id` (hors ligne) : le paquet magique vers ses
/// cartes réseau gardées (`wake` de `Computer`, voir `kybernow_core::wake`),
/// en diffusion et en direct. Rend le nombre de paquets partis. Refus, dans
/// la langue de l'app : aucune carte connue (Station d'avant la 0.5.0, ou
/// jamais vue allumée depuis), ou aucun paquet parti.
#[tauri::command(async)]
fn wake(app: AppHandle, id: String) -> Result<usize, String> {
    let c = app.state::<UiState>().with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    let sent = kybernow_core::wake::send(&c.wake).map_err(|e| e.to_string())?;
    log::info!("réveil de {} : {sent} paquets envoyés", c.name);
    // L'app l'attend (2 min au plus), que la fenêtre soit ouverte ou non.
    app.state::<UiState>().waking.locked().insert(id, Instant::now());
    emit_computers(&app);
    Ok(sent)
}

#[tauri::command(async)]
fn save_settings(app: AppHandle, id: String, settings: Settings) -> Result<(), String> {
    let state = app.state::<UiState>();
    // « Garder la dernière image » coupé : effacée tout de suite.
    if !settings.keep_last_image {
        snapshot::forget(&id);
        if let Some(l) = state.live.locked().get_mut(&id) {
            l.last = None;
        }
    }
    let ctrl_command = settings.ctrl_command;
    let immersive = settings.immersive.clone();
    let adaptive = settings.adaptive;
    let hdr = settings.hdr_on();
    let microphone = settings.microphone;
    let before = state.with_store(|s| {
        let c = s.get_mut(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        let before = std::mem::replace(&mut c.settings, settings);
        s.save();
        Ok::<_, String>(before)
    })?;
    let hdr_before = before.hdr_on();
    // Réglages synchronisés : les clés changées partent au relais du compte.
    synced::changed(&app, &id, &before);
    // « HDR » changé dans les réglages pendant sa session : tout de suite.
    if hdr != hdr_before {
        apply_hdr(&app, &id, hdr);
    }
    // « Micro » de la rubrique Son pendant sa session : tout de suite.
    if microphone != before.microphone && state.in_session(&id) {
        let _ = micro::apply(&app, &id, microphone);
    }
    // « Ctrl agit comme ⌘ » changé pendant sa session : tout de suite.
    let on = kybernow_core::input::keymap::control_as_command(ctrl_command, &computer_os(&state, &id));
    state.with_session(&id, |r| r.host.set_control_as_command(on));
    // « Mode immersif » : tout de suite aussi.
    state.with_session(&id, |r| r.host.set_immersive(&immersive));
    // « Débit adaptatif » : tout de suite aussi (le reste de l'image ne bouge pas).
    if adaptive != before.adaptive {
        let changed = state.with_session(&id, |r| {
            let mut sent = r.sent.locked();
            sent.settings.adaptive = adaptive;
            r.host.set_settings(sent.settings.clone())
        });
        if changed.is_some() {
            log::info!("débit adaptatif {} pendant la session", if adaptive { "allumé" } else { "coupé" });
        }
    }
    emit_computers(&app);
    Ok(())
}

/// Le système de l'ordinateur (« macOS 26.1 », « Windows 11 Pro ») : celui que
/// la Station donne maintenant, sinon celui gardé à l'ajout.
fn computer_os(state: &UiState, id: &str) -> String {
    let live = state.live.locked().get(id).and_then(|l| l.info.as_ref().map(|i| i.os.clone()));
    live.filter(|os| !os.is_empty())
        .or_else(|| state.with_store(|s| s.get(id).map(|c| c.os.clone())))
        .unwrap_or_default()
}

#[tauri::command(async)]
fn rename_computer(app: AppHandle, id: String, name: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    state.with_store(|s| {
        let c = s.get_mut(&id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        if !name.trim().is_empty() {
            c.name = name.trim().into();
        }
        s.save();
        Ok::<_, String>(())
    })?;
    emit_computers(&app);
    Ok(())
}

#[tauri::command(async)]
fn remove_computer(app: AppHandle, id: String) {
    let state = app.state::<UiState>();
    state.with_store(|s| s.remove(&id));
    state.live.locked().remove(&id);
    snapshot::forget(&id);
    playstation::forget_station(&app, &id);
    emit_computers(&app);
}

struct UiEvents {
    app: AppHandle,
    id: String,
    sent: Arc<Mutex<Sent>>,
    stats: Arc<Mutex<Option<Stats>>>,
    spec: Spec,
    /// « Cet écran » au lancement de la session (`Layout::this_screen`).
    this_screen: Option<String>,
    spot: Arc<Mutex<Spot>>,
    /// La première session (plusieurs ordinateurs) : passage de l'accueil,
    /// fonds des autres écrans. Les suivantes : leur fenêtre de connexion.
    first: bool,
    connexion: Arc<Connexion>,
    /// Session d'invité (KyberNow Share) : jamais l'ordinateur d'une liste de l'app.
    guest: Option<Arc<share::Guest>>,
    /// Session d'emprunt (prêt de l'ordinateur) : jamais l'ordinateur d'une liste de l'app.
    borrow: Option<Arc<lend::Borrow>>,
    /// Le partage de cette session d'hôte (le même que celui de `Running`).
    share: Arc<Mutex<Option<ShareState>>>,
    /// Le prêt de cette session d'hôte (le même que celui de `Running`).
    lend: Arc<Mutex<Option<kybernow_core::lend::LendView>>>,
    /// La première vue du prêt est arrivée : ses demandes déjà là ne sont pas dites.
    lend_seen: std::sync::atomic::AtomicBool,
    /// Le chemin de la session (le même que celui de `Running`).
    route: Arc<Mutex<Option<RouteInfo>>>,
    /// L'ordinateur ne crée pas de manettes (`gamepads` de `/api/stream/info`,
    /// Station Mac sans le droit d'Apple) ; jamais pour un invité ou un ami.
    pads_off: bool,
    /// « Manettes indisponibles sur cet ordinateur. » déjà dit dans cette session.
    pads_told: std::sync::atomic::AtomicBool,
    /// PyroWave en Wi-Fi ou par le relais : le bandeau du début de session
    /// déjà montré (une fois par session).
    pyrowave_told: std::sync::atomic::AtomicBool,
    /// PyroWave : « Le réseau sature » arrivé alors que la barre était
    /// ouverte sur cette session ; montré quand elle se cache
    /// ([`show_pending_strained`]), oublié si la session finit ou si l'image
    /// est déjà en HEVC.
    pyrowave_strained_pending: std::sync::atomic::AtomicBool,
    /// Profil Auto : le test gardé et le réseau de ce PC au lancement (nom du
    /// Wi-Fi, « Ethernet ») ; le bandeau « Autre réseau que celui du test »
    /// au début de la session (`autotest::tell_other_network`).
    auto_check: Option<autotest::Check>,
    /// Le bandeau « Autre réseau que celui du test » déjà montré.
    auto_told: std::sync::atomic::AtomicBool,
    /// Enregistrement de la session : le dernier état dit par le cœur (ou par
    /// la réponse d'Admin à la barre), avec le nom de l'ordinateur pour un
    /// invité ; `None` : la Station n'enregistre pas (ni bouton ni pastille).
    recording: Mutex<Option<recording::Seen>>,
    /// Le micro de la session : choix, état du cœur, allumage à annoncer.
    mic: Mutex<kybernow_core::micro::MicUi>,
}

/// `session-route` : par où passe la session `id` (la puce « Par le relais » de la
/// barre), à chaque changement du chemin.
#[derive(Clone, Serialize)]
struct RouteEvent {
    id: String,
    route: relais::RouteView,
}

/// `bar-shown` : la barre se montre pour la session `id` (une seule barre,
/// elle sert la session de la fenêtre où elle s'ouvre) ; `pinned` : demandée
/// par « Statistiques » (menu de l'icône), elle ne repart pas d'elle-même
/// tant que le pointeur n'y est pas passé ; `multi` : plusieurs écrans (pas
/// de « Plein écran ») ; `keep_sound` : « Garder le son » de cette session.
#[derive(Clone, Serialize)]
struct BarShown {
    id: String,
    #[serde(flatten)]
    label: BarLabel,
    /// Le nom de l'ordinateur à côté du symbole (« Invité sur {nom} » pour un invité).
    title: String,
    /// Le réglage « Souris » de l'ordinateur (`auto`, `bureau`, `jeu`) : le menu Souris le coche.
    mouse: String,
    /// Presse-papiers partagé (réglage de l'ordinateur) : la pastille, estompée sinon.
    clipboard: bool,
    /// L'ordinateur est une Station Mac : le menu Touches propose ses touches
    /// (« ⌘ ⇥ », « ⌘ Espace »…). Faux pour un invité ou un ami, qui ne
    /// connaissent pas son système.
    mac: bool,
    /// La largeur de la zone cliente de la session, en points (bornée par la
    /// fenêtre de la barre) : au-delà, la barre se fait étroite.
    room: f64,
    /// Ouverte par « Statistiques » (menu de l'icône, Ctrl+Alt+Maj+S) : son panneau est ouvert.
    stats: bool,
    pinned: bool,
    multi: bool,
    keep_sound: bool,
    /// Sessions ouvertes : « Garder le son » à partir de deux (ensuite,
    /// l'événement `sessions`).
    sessions: usize,
    /// Session d'invité (KyberNow Share) : la barre est réduite (rôle,
    /// « Quitter le partage »), sans profil ni « Ordinateurs ».
    guest: Option<GuestBar>,
    /// Session d'hôte : « Partager » et le panneau (partage ouvert, invités).
    share: Option<crate::sharing::ShareView>,
    /// Session d'hôte : « Prêter l'ordinateur… » et son panneau (code, demandes).
    lend: Option<crate::lending::LendPanel>,
    /// Enregistrement de la session : l'état et les textes du cœur
    /// (`RecordingState::ui_json`) ; absent : Station qui n'enregistre pas.
    recording: Option<serde_json::Value>,
    /// Session d'emprunt : la barre est réduite (« Prêté par {hôte} », temps
    /// restant, « Rendre l'ordinateur »).
    borrow: Option<crate::lending::BorrowBar>,
    /// Un avis bref (changement de rôle d'un invité) que la barre montre
    /// quelques secondes, sans prendre le clavier à la session.
    notice: Option<String>,
    /// L'avis est celui d'une demande à rejoindre : le choisir ouvre le panneau de
    /// « Partager ».
    notice_opens_share: bool,
    /// L'avis est celui d'une demande d'emprunt : le choisir ouvre le panneau de
    /// « Prêter l'ordinateur… ».
    notice_opens_lend: bool,
    /// L'avis est celui de la caméra refusée par Windows : le choisir ouvre
    /// ses paramètres de confidentialité (« Ouvrir les Réglages »).
    notice_opens_settings: bool,
    /// L'icône de l'avis dans sa capsule (`gamepad` : « Manettes indisponibles sur
    /// cet ordinateur. ») ; absente : celle d'un écran.
    #[serde(skip_serializing_if = "Option::is_none")]
    notice_icon: Option<&'static str>,
    /// Par où passe la session (puce « Par le relais ») ; absent tant que le cœur
    /// ne l'a pas dit.
    route: Option<relais::RouteView>,
    /// La puce « HDR » (écran HDR seulement, jamais pour un invité).
    hdr: Option<BarHdr>,
    /// La puce « Micro » (docs/link-ui.md « Micro ») : `MicUi::ui_json` du cœur ;
    /// absente sans micro sur ce PC.
    mic: Option<serde_json::Value>,
    /// PyroWave (docs/link-ui.md) : ce PC rejoint l'ordinateur par câble ;
    /// la barre en tire la puce ambre « PyroWave en Wi-Fi » quand l'image est
    /// en PyroWave (format des mesures) sans câble.
    #[serde(skip_serializing_if = "Option::is_none")]
    wired: Option<bool>,
    /// L'avis est le bandeau de PyroWave (`pyrowave_wifi`, `pyrowave_relay`,
    /// `pyrowave_strained` : le réseau sature) : titre, « Passer en HEVC » et
    /// croix, 8 s.
    #[serde(skip_serializing_if = "Option::is_none")]
    notice_kind: Option<&'static str>,
    /// Une partie sur console PlayStation : la barre de la partie (son, souris,
    /// réglages, plein écran, veille, « Se déconnecter »).
    #[serde(skip_serializing_if = "Option::is_none")]
    console: Option<playstation::BarView>,
}

/// `bar-shown` pour une partie sur console : la même barre, réduite à ce qui
/// a du sens pour une console.
fn console_bar_shown(id: &str, name: &str, pinned: bool, sessions: usize, route: Option<relais::RouteView>, console: playstation::BarView) -> BarShown {
    BarShown {
        id: id.to_string(),
        label: BarLabel { label: name.to_string(), profile: String::new(), custom: false },
        // La barre d'une console a ses propres boutons (son, souris, réglages…) :
        // pas de menu Souris, de presse-papiers ni de touches du Mac.
        title: name.to_string(),
        mouse: "auto".to_string(),
        clipboard: false,
        mac: false,
        room: BAR_WINDOW_WIDTH,
        stats: false,
        pinned,
        multi: false,
        keep_sound: false,
        sessions,
        guest: None,
        share: None,
        lend: None,
        borrow: None,
        recording: None,
        notice: None,
        notice_opens_share: false,
        notice_opens_lend: false,
        notice_opens_settings: false,
        notice_icon: None,
        route,
        hdr: None,
        mic: None,
        wired: None,
        notice_kind: None,
        console: Some(console),
    }
}

/// `share-request` : une demande à rejoindre arrive pendant que la barre est déjà
/// ouverte sur la session `id` : l'avis `text`, que la barre ne montre pas si son
/// panneau de « Partager » est ouvert (il montre la demande).
#[derive(Clone, Serialize)]
struct ShareRequestEvent {
    id: String,
    text: String,
    /// Le panneau que l'avis ouvre : `share` (« Partager ») ou `lend` (« Prêter l'ordinateur… »).
    panel: &'static str,
}

/// L'invité que sert la barre : son rôle (`spectator`, `player`, `kbm`,
/// `player_kbm`), l'hôte du partage, la « Qualité de l'image » demandée
/// (`auto`, `host`, `2160p60`, `1080p60`, `720p30`) et la qualité reçue (« 1080p · 60 Hz »,
/// absente tant que Kyber ne la connaît pas).
#[derive(Clone, Serialize)]
struct GuestBar {
    role: &'static str,
    host: String,
    quality: &'static str,
    image: Option<String>,
    /// Regarder avec un compte : « Arrêter de regarder », « Écran » et ses écrans.
    watch: Option<crate::sharing::WatchView>,
}

/// `bar-label` : le profil de la session `id` a changé.
#[derive(Clone, Serialize)]
struct BarLabelEvent {
    id: String,
    #[serde(flatten)]
    label: BarLabel,
}

/// `stats` : les mesures de la seconde écoulée de la session `id`.
#[derive(Clone, Serialize)]
struct StatsEvent<'a> {
    id: &'a str,
    #[serde(flatten)]
    stats: &'a Stats,
}

/// `session-keep-sound` : « Garder le son » de la session `id`.
#[derive(Clone, Serialize)]
struct KeepSoundEvent {
    id: String,
    on: bool,
}

/// `session-replan` : la connexion reprend en session (écrans de ce PC ou
/// profil changés) ; `screens` : le nombre d'écrans qu'elle prendra.
#[derive(Clone, Serialize)]
struct ReplanEvent {
    id: String,
    screens: usize,
}

#[derive(Clone, Serialize)]
struct BarLabel {
    label: String,
    profile: String,
    /// Les réglages s'écartent du profil : la barre dit « Personnalisé ».
    custom: bool,
}

#[derive(Clone, Serialize)]
struct Phase {
    id: String,
    phase: &'static str,
    detail: Option<String>,
}

/// `session-station-restarting` : la Station redémarre en dernier recours.
#[derive(Clone, Serialize)]
struct StationRestarting {
    id: String,
}

impl HostEvents for UiEvents {
    fn status(&self, s: &Status) {
        let (phase, detail) = match s {
            Status::Preparing => ("preparing", None),
            Status::Connecting => ("connecting", None),
            Status::FirstImages => ("first_images", None),
            Status::Streaming => ("streaming", None),
            Status::Reconnecting { attempt, max } => ("reconnecting", Some(format!("{attempt}/{max}"))),
            // L'étape « Connexion », puis `session-station-restarting` (ci-dessous).
            Status::StationRestarting => ("connecting", None),
            Status::Ending => ("ending", None),
            Status::Ended { error, .. } => ("ended", error.clone()),
        };
        {
            // Ce que montre la fenêtre de connexion d'une session suivante.
            let mut view = self.connexion.view.locked();
            view.phase = phase;
            view.restarting = matches!(s, Status::StationRestarting);
        }
        let _ = self.app.emit("session-status", Phase { id: self.id.clone(), phase, detail });
        if matches!(s, Status::StationRestarting) {
            // Dernier recours de la connexion : « La Station redémarre… »
            // jusqu'au prochain `session-status` (voir ui/src/lib/api.ts).
            let _ = self.app.emit("session-station-restarting", StationRestarting { id: self.id.clone() });
        }
        // La connexion reprend en session (écrans de ce PC ou profil changés) :
        // ses étapes reviennent sur l'écran principal, le fond de la connexion
        // sur les autres écrans, jusqu'à la première image de chacun.
        if matches!(s, Status::Preparing) && self.spot.locked().shown {
            self.begin_replan();
        }
    }

    fn stats(&self, s: &Stats) {
        // L'image telle que l'ordinateur l'envoie (sans écran virtuel, la
        // taille de son écran, pas celle demandée).
        if s.width > 0 && s.height > 0 {
            let mut sent = self.sent.locked();
            sent.mode.width = s.width;
            sent.mode.height = s.height;
        }
        let _ = self.app.emit_to("bar", "stats", StatsEvent { id: &self.id, stats: s });
        *self.stats.locked() = Some(s.clone());
        self.tell_pads_off();
        self.tell_pyrowave(s);
        self.tell_other_network();
        tray::refresh();
    }

    /// Première image à l'écran : l'accueil laisse la place à la session,
    /// même si Windows a refusé le premier plan à celle-ci (l'accueil,
    /// resté devant, montrait le passage ouvert sur du noir). Une session
    /// suivante (plusieurs ordinateurs) : sa fenêtre de connexion s'ouvre et
    /// part, l'accueil ne bouge pas.
    fn visible(&self, foreground: bool) {
        if !foreground {
            log::info!("session hors du premier plan : {}", if self.first { "accueil caché quand même" } else { "elle attend dans la barre des tâches" });
        }
        self.spot.locked().shown = true;
        // Le passage s'ouvre à la première image montrée, pas à la connexion.
        let _ = self.app.emit("session-shown", &self.id);
        if !self.first {
            close_connexion(&self.app, &self.connexion, Duration::from_millis(900));
            return;
        }
        if let Some(w) = self.app.get_webview_window("main") {
            let _ = w.hide();
        }
    }

    /// Les écrans de la session : le fond de la connexion sur ceux qui n'ont
    /// pas encore leur image, la ligne d'un écran que l'ordinateur n'a pas pu
    /// créer, rien pour ceux que la session ne prend pas (une Station qui ne
    /// gère qu'un écran, le principal seul accepté).
    fn screens(&self, set: &ScreenSet) {
        let target = |s: &crate::host::ScreenState| fonds::Target { key: s.key.clone(), x: s.x, y: s.y, width: s.width, height: s.height };
        let mut primary = None;
        let mut others = Vec::new();
        let mut not_created = Vec::new();
        let (mut count, mut pixel_rate) = (0u32, 0.0f64);
        for s in &set.screens {
            match &s.role {
                Role::Stream { session_primary, width, height, refresh } => {
                    count += 1;
                    pixel_rate += f64::from(*width) * f64::from(*height) * f64::from((*refresh).max(1));
                    if *session_primary {
                        primary = Some(target(s));
                        // Mesures de l'écran principal.
                        let mut sent = self.sent.locked();
                        sent.mode = kybernow_core::console::Mode { width: *width, height: *height, refresh: *refresh };
                    } else {
                        others.push(target(s));
                    }
                }
                Role::NotCreated => not_created.push(target(s)),
                Role::Unused => {}
            }
        }
        {
            let mut sent = self.sent.locked();
            sent.screens = count.max(1);
            sent.pixel_rate = pixel_rate;
        }
        let waiting: Vec<fonds::Target> = {
            let mut spot = self.spot.locked();
            spot.primary = primary;
            spot.others = others.clone();
            // Un écran qui n'est plus pris oublie qu'il a montré son image.
            let keys: Vec<String> = others.iter().map(|t| t.key.clone()).chain(spot.primary.iter().map(|t| t.key.clone())).collect();
            spot.visible.retain(|k| keys.contains(k));
            // Sans image encore (connexion, ou reprise), un fond sans texte.
            others.into_iter().filter(|t| !spot.visible.contains(&t.key)).collect()
        };
        let _ = self.app.emit("session-screens", ScreensEvent { id: self.id.clone(), screens: count.max(1) as usize });
        tray::refresh();
        // Les fonds de la connexion : seulement pour la première session (les
        // suivantes prennent un écran, en fenêtre).
        if !self.first {
            return;
        }
        let Some(fonds) = self.app.state::<UiState>().fonds.get().cloned() else { return };
        let keep: Vec<String> = waiting.iter().chain(&not_created).map(|t| t.key.clone()).collect();
        fonds.keep_only(&keep);
        fonds.show(&waiting);
        for t in &not_created {
            fonds.not_created(t);
        }
    }

    /// Un écran montre son image : son fond de connexion s'efface ; pour
    /// l'écran principal après une reprise, les étapes se referment et
    /// l'accueil rend la place à la session.
    fn screen_visible(&self, key: &str) {
        let (replanning, primary) = {
            let mut spot = self.spot.locked();
            if !spot.visible.iter().any(|k| k == key) {
                spot.visible.push(key.to_string());
            }
            (spot.replanning.is_some(), spot.primary.as_ref().is_some_and(|p| p.key == key))
        };
        if self.first
            && let Some(fonds) = self.app.state::<UiState>().fonds.get()
        {
            fonds.hide(key);
        }
        if replanning && primary {
            self.end_replan();
        }
    }

    fn top_edge(&self, hwnd: isize) {
        let app = self.app.clone();
        let id = self.id.clone();
        // Sur le fil de Tauri : la fenêtre de la session ne doit pas attendre.
        let _ = self.app.run_on_main_thread(move || show_bar(&app, &id, hwnd, false));
    }

    fn statistics(&self, hwnd: isize) {
        toggle_stats_on(&self.app, &self.id, hwnd);
    }

    /// « Afficher », « Se connecter » sur cet ordinateur ou Ctrl+Alt+Maj+O
    /// pendant qu'il se connecte : sa fenêtre de connexion passe devant (pour
    /// la première session, l'accueil et son passage).
    fn connexion_front(&self) {
        // La première session se connecte sur l'accueil (son passage).
        let label = if self.first { Some("main".to_string()) } else { self.connexion.label.locked().clone() };
        let Some(label) = label else { return };
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(w) = app.get_webview_window(&label) {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        });
    }

    /// Le partage de l'hôte et ses invités (signe de vie d'Admin) : gardés
    /// pour la barre, qui les montre dans son panneau et sa pastille.
    ///
    /// Une demande venue par le relais qui arrive pendant la session : l'avis bref
    /// « {prénom} demande à rejoindre. » (le choisir ouvre le panneau), pour la première
    /// seulement quand plusieurs arrivent d'un coup (la pastille « 2 demandes » dit le reste).
    fn guests(&self, share: Option<&ShareState>) {
        let after = share.filter(|s| s.open).cloned();
        let before = std::mem::replace(&mut *self.share.locked(), after.clone());
        share::emit_state(&self.app, &self.id, &self.share);
        if self.guest.is_none()
            && self.borrow.is_none()
            && let Some(name) = crate::sharing::new_requests(before.as_ref(), after.as_ref()).first()
        {
            log::info!("[{}] {name} demande à rejoindre (par le relais)", self.connexion.name);
            share::request_notice(&self.app, &self.id, &self.connexion.name, name);
        }
    }

    /// Le rôle de l'invité : au départ, puis chaque changement par l'hôte
    /// (étiquette de la barre, menu de l'icône, avis bref).
    fn share_role(&self, role: ShareRole, previous: Option<ShareRole>) {
        let Some(g) = &self.guest else { return };
        *g.role.locked() = role;
        log::info!("[{}] rôle d'invité : {role:?}", g.title);
        let _ = self.app.emit_to("bar", "share-role", share::RoleEvent { id: self.id.clone(), role: role.as_str() });
        tray::refresh();
        if let Some(key) = crate::sharing::role_notice(role, previous) {
            share::notice(&self.app, &self.id, &g.title, &t(key, &[]));
        }
    }

    /// L'image que reçoit l'invité : au départ, puis à chaque changement
    /// (signe de vie, « Qualité de l'image » changée). La barre montre la
    /// qualité reçue ; l'avis (« Image allégée pour suivre. », « L'ordinateur ne
    /// peut pas faire une image de plus… ») s'affiche comme celui d'un rôle.
    /// Un avis bref du cœur (HDR refusé) : dans la capsule de la session.
    fn notice(&self, text: &str) {
        share::notice(&self.app, &self.id, &self.connexion.name, text);
    }

    // Micro (docs/micro.md) : ui/micro.rs.
    fn microphone(&self, state: kybernow_core::micro::MicState) {
        self.mic_changed(state);
    }
    fn microphone_denied(&self) {
        self.mic_denied();
    }
    fn toggle_microphone(&self) {
        self.mic_toggle();
    }

    fn camera_offer(&self, offered: bool) {
        camera::offer(&self.app, &self.id, offered);
    }

    fn toggle_camera(&self) {
        camera::toggle(&self.app, &self.id);
    }

    /// PyroWave : le réseau sature (docs/link-ui.md « PyroWave ») ; le même
    /// bandeau que celui du début de session, titré « Le réseau sature »,
    /// avec « Passer en HEVC » ; une notification de Windows si l'app n'est
    /// pas devant. La barre ouverte sur cette session (menus, statistiques,
    /// autre avis) : il attend qu'elle se cache (`bar_hide`).
    fn pyrowave_strained(&self) {
        let (app, id, name) = (self.app.clone(), self.id.clone(), self.connexion.name.clone());
        let _ = self.app.run_on_main_thread(move || {
            let state = app.state::<UiState>();
            let bar_here = state.bar_session.locked().as_deref() == Some(id.as_str())
                && app.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false);
            if bar_here {
                log::info!("[{name}] PyroWave : le réseau sature, bandeau gardé jusqu'à la fermeture de la barre");
                state.with_session(&id, |r| r.events.pyrowave_strained_pending.store(true, std::sync::atomic::Ordering::Relaxed));
                return;
            }
            share::panel_notice(&app, &id, &name, &t("pyrowave.banner_text", &[]), "pyrowave_strained");
        });
    }

    /// Le plafond de l'encodeur trouvé par la session : gardé pour
    /// l'ordinateur (`Computer::encoder_max`, `computers.json`) et donné aux
    /// sessions suivantes. Pas pour un invité : l'image est celle de l'hôte.
    fn encoder_ceiling(&self, mbps: u32) {
        if self.guest.is_some() || mbps == 0 {
            return;
        }
        let state = self.app.state::<UiState>();
        state.with_store(|s| {
            let Some(c) = s.get_mut(&self.id) else { return };
            if c.encoder_max != Some(mbps) {
                log::info!("[{}] plafond de l'encodeur gardé : {mbps} Mbit/s par écran (avant : {:?})", self.connexion.name, c.encoder_max);
                c.encoder_max = Some(mbps);
                s.save();
            }
        });
    }

    fn share_image(&self, image: &ShareImage, _previous: Option<&ShareImage>, notice: Option<&str>) {
        let Some(g) = &self.guest else { return };
        let view = crate::sharing::ImageView::from(image);
        *g.quality.locked() = image.quality;
        *g.image.locked() = view.label.clone();
        let _ = self.app.emit_to("bar", "share-image", share::ImageEvent { id: self.id.clone(), quality: view.quality, label: view.label });
        if let Some(text) = notice {
            share::notice(&self.app, &self.id, &g.title, text);
        }
    }

    /// C'est le partage qui finit la session de l'invité : le texte est aussi
    /// la raison de sa fin (`Summary::error`), montrée telle quelle.
    fn share_end(&self, end: &ShareEnd, text: &str) {
        log::info!("{} : fin du partage ({}) : {text}", self.connexion.name, end.kind());
    }

    /// Par où passe la session (docs/relais.md) : au départ, puis à chaque
    /// changement (repli TCP, allers-retours). La barre montre « Par le relais » ;
    /// la fin de la session le garde pour la ligne « Chemin » des statistiques, et les
    /// appels à Admin d'une session par le relais passent eux aussi par lui.
    fn route(&self, route: &RouteInfo) {
        let before = self.route.locked().replace(route.clone());
        if before.as_ref() == Some(route) {
            return;
        }
        if before.as_ref().is_none_or(|b| b.transport != route.transport) {
            log::info!("[{}] chemin : {}", self.connexion.name, route.transport.label(kybernow_core::i18n::Lang::Fr));
        }
        let view = relais::RouteView::new(route, i18n::lang());
        let _ = self.app.emit_to("bar", "session-route", RouteEvent { id: self.id.clone(), route: view });
        // Par le relais ou non : les réglages de l'ordinateur et le menu de l'icône (redémarrage et mise à
        // jour de la Station) en dépendent.
        if before.as_ref().is_some_and(RouteInfo::via_relay) != route.via_relay() && self.guest.is_none() && self.borrow.is_none() {
            emit_computers(&self.app);
        }
    }

    fn lend_notice(&self, notice: &kybernow_core::lend::LendNotice, text: &str) {
        self.on_lend_notice(notice, text);
    }

    fn lend_end(&self, end: kybernow_core::lend::LendEnd, text: &str) {
        self.on_lend_end(end, text);
    }

    fn lend(&self, view: Option<&kybernow_core::lend::LendView>) {
        self.on_lend_view(view);
    }

    fn recording(&self, state: &kybernow_core::recording::RecordingState, notice: Option<&str>, computer: Option<&str>) {
        self.on_recording(state, notice, computer);
    }

    fn record_shortcut(&self) {
        recording::toggle_in_background(&self.app, &self.id);
    }

    fn ended(&self, s: &Summary) {
        camera::ended(&self.id);
        let state = self.app.state::<UiState>();
        // Cette session-là, pas une autre du même ordinateur.
        let (found, remaining) = {
            let mut sessions = state.sessions.locked();
            let before = sessions.len();
            sessions.retain(|r| r.host.id() != self.connexion.session);
            (sessions.len() < before, sessions.len())
        };
        // Déjà tenue pour finie (`watch_end`) : sa vraie fin n'y change plus rien.
        if !found {
            log::info!("[{}] session n°{} finie après avoir été tenue pour finie", self.connexion.name, self.connexion.session);
            return;
        }
        // Dernière image : le fond de cet ordinateur sur l'accueil.
        let keep = state.with_store(|st| st.get(&self.id).map(|c| c.settings.keep_last_image)).unwrap_or(false);
        if let (true, Some(jpeg)) = (keep, &s.last_image) {
            snapshot::save(&self.id, jpeg);
            state.live.locked().entry(self.id.clone()).or_default().last = Some(Image::new(jpeg.clone()));
        }
        // Une session d'au moins 3 s qui a mesuré sa latence : la rubrique
        // Statistiques de cet ordinateur la montre.
        if s.seconds >= 3 && s.median_ms.is_some() {
            let at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let last = {
                let sent = self.sent.locked();
                let m = sent.mode;
                LastSession {
                    median_ms: s.median_ms,
                    p95_ms: s.p95_ms,
                    seconds: s.seconds,
                    at,
                    width: Some(m.width),
                    height: Some(m.height),
                    refresh: Some(m.refresh),
                    screens: Some(sent.screens.max(1)),
                    codec: Some(sent.settings.codec.label().into()),
                    mbps: Some(sent.mbps()),
                    route: self.route.locked().clone(),
                    hdr: s.hdr,
                    label: String::new(),
                }
            };
            state.with_store(|st| {
                if let Some(c) = st.get_mut(&self.id) {
                    c.last = Some(last);
                    st.save();
                }
            });
        }
        // La barre servait cette session : elle part.
        let served = {
            let mut bar = state.bar_session.locked();
            let served = bar.as_deref() == Some(self.id.as_str());
            if served {
                *bar = None;
            }
            served
        };
        if served && let Some(bar) = self.app.get_webview_window("bar") {
            let _ = bar.hide();
        }
        let (before, replan) = {
            let mut spot = self.spot.locked();
            let before = spot.before.take();
            let replan = spot.replanning.take();
            *spot = Spot::default();
            (before, replan)
        };
        let quitting = state.quitting.load(std::sync::atomic::Ordering::Relaxed);
        // Le message de l'échec reste dans la fenêtre de connexion (« Fermer »).
        let mut kept_in_window = false;
        if self.first {
            // Plusieurs écrans : les fonds de la connexion partent (un échec se
            // lit sur l'écran principal, seul), l'accueil reprend sa place s'il
            // avait changé d'écran.
            if let Some(fonds) = state.fonds.get() {
                fonds.close_all();
            }
            if let Some(w) = self.app.get_webview_window("main").filter(|_| !quitting) {
                // Reprise de la connexion interrompue : l'accueil quitte l'écran principal.
                if let Some(r) = replan {
                    let _ = w.set_always_on_top(false);
                    if let Some(size) = r.size {
                        let _ = w.set_size(size);
                    }
                    if let Some(at) = r.position {
                        let _ = w.set_position(at);
                    }
                }
                // Revenu où il était, sauf après une erreur : elle se lit sur l'écran principal.
                if let (None, Some(at)) = (&s.error, before) {
                    let _ = w.set_position(at);
                }
            }
        } else {
            // Session suivante : sa fenêtre de connexion part, sauf un échec
            // pendant qu'elle est ouverte (avant la première image, ou quand
            // la connexion a repris en session), qu'elle montre jusqu'à « Fermer ».
            // Pas après « Annuler » (une fin demandée n'est pas un échec).
            let open = self.connexion.label.locked().is_some();
            kept_in_window = open && s.error.is_some() && !s.requested && !quitting;
            if kept_in_window {
                self.connexion.view.locked().error = s.error.clone();
                keep_failure_in_front(&self.app, &self.connexion);
            } else {
                close_connexion(&self.app, &self.connexion, Duration::ZERO);
            }
        }
        // La dernière session fermée ramène l'accueil, comme avant, s'il ne
        // reste ni connexion qui se lance ni fenêtre de connexion ouverte
        // (l'accueil ne passe pas devant elles) ; s'il reste des sessions,
        // l'accueil ne leur prend pas le premier plan, et si celle-ci était
        // devant, la session active reprend la place — sauf quand le message
        // de son échec reste dans sa fenêtre de connexion : c'est elle qui
        // reste devant.
        if remaining == 0 {
            show_home_if_idle(&self.app);
        } else if (s.was_front || served) && !quitting && !kept_in_window && let Some(next) = state.active_id() {
            // Sa fenêtre (ou sa barre : « Se déconnecter ») était devant.
            state.with_session(&next, |r| {
                log::info!("[{}] reprend le premier plan après la fin de {}", r.name, self.connexion.name);
                r.host.focus(0)
            });
        }
        // Une fin non demandée (pas « Se déconnecter », ni la fenêtre fermée)
        // dont la fenêtre disparaît : une notification si personne ne
        // regardait (autre app devant), et toujours quand d'autres sessions
        // continuent, même app au premier plan et même si c'était la session
        // active. Pas quand le message de l'échec reste dans sa fenêtre de
        // connexion avec « Fermer » (docs/link-ui.md, `notice.rs`). Un clic
        // dessus ouvre l'accueil sur cet ordinateur.
        let notify = crate::notice::end_notification(&crate::notice::End {
            requested: s.requested,
            quitting,
            app_in_front: shell::app_in_front(),
            others_continue: remaining > 0,
            started: self.first || s.shown,
            kept_in_window,
        });
        // Session d'emprunt : la carte de fin de l'accueil (le texte du prêt, ou le
        // retour possible pendant 15 min) ; l'ordinateur rendu : l'avis « Vous avez
        // rendu l'ordinateur. », sans notification (une fin voulue).
        let lend_end = self.borrow.as_ref().map(|b| crate::lending::end_view(&b.host, b.end.locked().clone()));
        if let Some(b) = &self.borrow {
            if notify && let Some(e) = lend_end.as_ref().filter(|e| !e.left) {
                let text = e.text.clone().or_else(|| s.error.clone()).unwrap_or_else(|| t("lend.come_back", &[]));
                shell::notify_to("kybernow://open", &b.title, &text, None);
            }
        } else if notify && let Some(g) = &self.guest {
            // Un invité : la raison du partage (« Léa a arrêté le partage. »), sous
            // son titre ; jamais « Reprendre la main », et pas de bouton (il
            // n'y a pas d'ordinateur à reprendre).
            if let Some(text) = &s.error {
                shell::notify_to("kybernow://open", &g.title, text, None);
            }
        } else if notify {
            let name = state.with_store(|st| st.get(&self.id).map(|c| c.name.clone())).unwrap_or_default();
            let home = format!("kybernow://open?id={}", self.id);
            let again = format!("kybernow://connect?id={}", self.id);
            match &s.error {
                Some(e) => shell::notify_to(&home, &t("notify.stopped_title", &[("name", &name)]), e, Some((&t("notify.resume", &[]), &again))),
                None => shell::notify_to(
                    &home,
                    &t("notify.ended_title", &[("name", &name)]),
                    &t("notify.ended_body", &[]),
                    Some((&t("notify.take_back", &[]), &again)),
                ),
            }
        }
        #[derive(Serialize, Clone)]
        struct Ended<'a> {
            id: &'a str,
            summary: &'a Summary,
            /// Session d'emprunt : la carte de fin, ou l'avis de l'ordinateur rendu (absente : pas un emprunt).
            lend: Option<crate::lending::EndView>,
        }
        let _ = self.app.emit("session-ended", Ended { id: &self.id, summary: s, lend: lend_end });
        emit_sessions(&self.app);
        emit_computers(&self.app);
        // L'app se fermait : la console est prévenue, on peut partir quand la
        // dernière session est finie et qu'aucune connexion ne se lance.
        if quitting {
            exit_when_idle(&self.app);
        }
    }
}

/// `session-screens` : le nombre d'écrans de la session (l'étape « Préparation
/// des écrans » de la connexion, la barre).
#[derive(Clone, Serialize)]
struct ScreensEvent {
    id: String,
    screens: usize,
}

/// Le nombre d'écrans que la session prendra avec ces réglages et les écrans
/// de ce PC maintenant : le calcul du cœur (`session_plan_for`), comme à
/// l'accueil et dans la session.
fn expected_screens(spec: Spec, this_screen: Option<String>, settings: &kybernow_core::profile::StreamSettings, virtual_display: bool) -> usize {
    let layout = Layout { monitors: displays::enumerate(), spec, this_screen, extra_session: false };
    screens::session_plan_for(&layout.screens(), settings, virtual_display).map_or(1, |p| p.len())
}

impl UiEvents {
    /// Une fois par session, à l'image montrée avec une manette branchée (ou
    /// dès que la première se branche) : l'ordinateur ne la verra pas
    /// (docs/link-ui.md, « Manettes indisponibles »). Vu à chaque mesure.
    /// PyroWave choisi et la session en Wi-Fi ou par le relais : le bandeau du
    /// début de session (docs/link-ui.md « PyroWave »), une fois, à la
    /// première seconde de PyroWave à l'écran ; « Passer en HEVC » relance
    /// l'image en HEVC pour cette session.
    fn tell_pyrowave(&self, s: &Stats) {
        use std::sync::atomic::Ordering;
        if self.pyrowave_told.load(Ordering::Relaxed) || !s.decoder.starts_with("PyroWave") || !self.spot.locked().shown {
            return;
        }
        let relayed = self.route.locked().as_ref().is_some_and(RouteInfo::via_relay);
        let wired = self.sent.locked().settings.wired;
        let Some(kind) = pyrowave_link(relayed, wired) else { return };
        if self.pyrowave_told.swap(true, Ordering::Relaxed) {
            return;
        }
        log::info!("[{}] PyroWave {} : bandeau", self.connexion.name, if relayed { "par le relais" } else { "en Wi-Fi" });
        share::panel_notice(&self.app, &self.id, &self.connexion.name, &t("pyrowave.banner_text", &[]), kind);
    }

    /// Profil Auto, réseau (ou relais) différent de celui du test : le bandeau
    /// discret du début de session (docs/link-ui.md, « Profil Auto et débit
    /// adaptatif »), une fois, à la première seconde d'image à l'écran.
    fn tell_other_network(&self) {
        use std::sync::atomic::Ordering;
        let Some(check) = &self.auto_check else { return };
        if self.auto_told.load(Ordering::Relaxed) || !self.spot.locked().shown {
            return;
        }
        let relayed = self.route.locked().as_ref().is_some_and(RouteInfo::via_relay);
        if !check.differs(relayed) || self.auto_told.swap(true, Ordering::Relaxed) {
            return;
        }
        log::info!("[{}] profil Auto : autre réseau que celui du test, bandeau", self.connexion.name);
        let lang = i18n::lang();
        share::panel_notice(
            &self.app,
            &self.id,
            &kybernow_core::i18n::tr(lang, "autotest.other_network", &[]),
            &kybernow_core::i18n::tr(lang, "autotest.other_network_text", &[]),
            "auto_network",
        );
    }

    fn tell_pads_off(&self) {
        use std::sync::atomic::Ordering;
        if !self.pads_off || self.pads_told.load(Ordering::Relaxed) || !self.spot.locked().shown {
            return;
        }
        if kybernow_core::gamepads::pads().is_empty() || self.pads_told.swap(true, Ordering::Relaxed) {
            return;
        }
        log::info!("[{}] manettes indisponibles sur cet ordinateur : avis", self.connexion.name);
        share::icon_notice(&self.app, &self.id, &self.connexion.name, &t("session.gamepads_unavailable", &[]), "gamepad");
    }

    /// La barre servait cette session : elle se referme.
    fn hide_bar_if_mine(&self) {
        let state = self.app.state::<UiState>();
        let mut bar = state.bar_session.locked();
        if bar.as_deref() != Some(self.id.as_str()) {
            return;
        }
        *bar = None;
        drop(bar);
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            window::set_bar_session(None);
            if let Some(bar) = app.get_webview_window("bar") {
                let _ = bar.hide();
            }
        });
    }

    /// La connexion reprend en session : l'accueil couvre l'écran principal
    /// avec ses étapes (le passage), les autres écrans reprennent le fond de
    /// la connexion, chacun jusqu'à sa première image. Une session suivante
    /// (plusieurs ordinateurs) : sa fenêtre de connexion revient par-dessus la
    /// sienne.
    fn begin_replan(&self) {
        if !self.first {
            let fresh = {
                let mut spot = self.spot.locked();
                let fresh = spot.replanning.is_none();
                spot.replanning = Some(Replan { visible: false, position: None, size: None });
                fresh
            };
            if !fresh {
                return;
            }
            log::info!("la connexion de {} reprend en session, dans sa fenêtre", self.id);
            let _ = self.app.emit("session-replan", ReplanEvent { id: self.id.clone(), screens: 1 });
            self.hide_bar_if_mine();
            let state = self.app.state::<UiState>();
            let hwnd = state.with_instance(self.connexion.session, |r| r.host.primary_window()).unwrap_or(0);
            let title = state.with_store(|s| s.get(&self.id).map(|c| c.name.clone())).unwrap_or_default();
            if let Some((at, client)) = window::window_frame(hwnd) {
                open_connexion(&self.app, &self.connexion, at, client, &title);
            }
            return;
        }
        let (primary, others) = {
            let mut spot = self.spot.locked();
            if spot.replanning.is_some() {
                return;
            }
            spot.replanning = Some(Replan { visible: false, position: None, size: None });
            spot.visible.clear();
            (spot.primary.clone(), spot.others.clone())
        };
        let screens = {
            let sent = self.sent.locked();
            expected_screens(self.spec, self.this_screen.clone(), &sent.settings, sent.virtual_display)
        };
        log::info!("la connexion reprend en session : {screens} écran(s)");
        let _ = self.app.emit("session-replan", ReplanEvent { id: self.id.clone(), screens });
        // La barre se referme.
        self.hide_bar_if_mine();
        let app = self.app.clone();
        let spot = self.spot.clone();
        let _ = self.app.run_on_main_thread(move || {
            let Some(main) = app.get_webview_window("main") else { return };
            let before = Replan { visible: main.is_visible().unwrap_or(false), position: main.outer_position().ok(), size: main.outer_size().ok() };
            if let Some(r) = spot.locked().replanning.as_mut() {
                *r = before;
            }
            if let Some(p) = primary {
                let _ = main.set_always_on_top(true);
                let _ = main.set_position(tauri::PhysicalPosition::new(p.x, p.y));
                let _ = main.set_size(tauri::PhysicalSize::new(p.width, p.height));
                let _ = main.show();
            }
            if let Some(fonds) = app.state::<UiState>().fonds.get() {
                fonds.show(&others);
            }
        });
    }

    /// L'écran principal a repris son image : les étapes se referment et
    /// l'accueil rend la place à la session.
    fn end_replan(&self) {
        let Some(r) = self.spot.locked().replanning.take() else { return };
        let _ = self.app.emit("session-shown", &self.id);
        if !self.first {
            close_connexion(&self.app, &self.connexion, Duration::from_millis(900));
            return;
        }
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            let Some(main) = app.get_webview_window("main") else { return };
            let _ = main.set_always_on_top(false);
            if let Some(size) = r.size {
                let _ = main.set_size(size);
            }
            if let Some(at) = r.position {
                let _ = main.set_position(at);
            }
            if !r.visible {
                let _ = main.hide();
            }
        });
    }
}

/// Réglages de la session et mode de l'écran pour un ordinateur, d'après ses
/// réglages, ce que ce PC sait décoder et « Sur batterie » des réglages de
/// l'app (`lighten` : alléger, et ce PC est sur batterie). `window` : le mode
/// de la fenêtre d'une session suivante (plusieurs ordinateurs,
/// `extra_session_window` du cœur), qui passe avant la définition des
/// réglages, sur mesure comprise ; sinon, le mode de « Cet écran »
/// (`system::screen_mode`).
fn session_plan(
    c: &Computer,
    pc: &ThisPc,
    lighten: bool,
    window: Option<kybernow_core::console::Mode>,
) -> (kybernow_core::profile::StreamSettings, kybernow_core::console::Mode, String) {
    let s = &c.settings;
    let choices = Choices {
        profile: Profile::parse(&s.profile),
        codec: s.codec.as_deref().and_then(Codec::parse),
        yuv444: s.yuv444,
        mbps: s.mbps,
        reliable: s.reliable,
        max_4k: s.max_4k && s.custom.is_none(),
        size: window.or_else(|| custom_mode(s)),
        lighten,
        auto: c.auto.clone(),
        fixed: !s.adaptive,
        encoder_max: c.encoder_max,
    };
    let (mut settings, mode) = plan(&choices, capabilities(pc), window.unwrap_or_else(system::screen_mode));
    // Le choix « HDR » de cet ordinateur (hors du profil) : la session ne s'en sert que si c'est possible.
    settings.hdr = s.hdr_on();
    let label = kybernow_core::plan::label(&settings);
    (settings, mode, label)
}

/// La définition sur mesure choisie, si elle est utilisable.
fn custom_mode(s: &Settings) -> Option<kybernow_core::console::Mode> {
    s.custom.filter(|m| m.width >= 640 && m.height >= 360 && m.refresh >= 24)
}

/// Ce qui décide du nombre d'écrans d'une session, d'après les réglages de
/// l'ordinateur, ce que sa Station sait créer (`info` ; sans elle, un écran)
/// et ce que ce PC décode (`pc`, la règle « Personnalisé » du cœur). Comme
/// sur Mac, l'écran virtuel compte comme utilisable tant que la Station n'a
/// pas répondu.
fn displays_spec(s: &Settings, info: Option<&Info>, lighten: bool, pc: &ThisPc) -> Spec {
    Spec {
        setting: DisplaysSetting::parse(&s.displays).unwrap_or_default(),
        limit_4k: s.max_4k && s.custom.is_none(),
        custom_resolution: custom_mode(s).is_some(),
        max_screens: displays::max_screens(info.map_or(1, |i| i.virtual_display.max_screens())),
        lighten,
        virtual_display_available: info.is_none_or(|i| i.virtual_display.available),
        capabilities: Some(capabilities(pc)),
    }
}

/// Ce que ce PC décode (sonde de la carte graphique).
fn capabilities(pc: &ThisPc) -> kybernow_core::profile::Capabilities {
    kybernow_core::profile::Capabilities { hevc: pc.hevc, hevc444: pc.hevc444, av1: pc.av1, hdr: pc.hdr, pyrowave: pc.pyrowave }
}

/// La puce « HDR » (docs/hdr.md, `profile::hdr_state`) : `info`, ce que la
/// Station annonce (pas encore lue : comme si elle le proposait, la puce ne
/// s'éteint pas avant sa réponse) ; `screen_hdr` : l'écran de la session est
/// HDR (sinon aucune trace du HDR).
pub(crate) fn hdr_chip(info: Option<&Info>, settings: &kybernow_core::profile::StreamSettings, pc: &ThisPc, screen_hdr: bool) -> kybernow_core::profile::HdrState {
    let unknown = kybernow_core::console::HdrInfo { available: true, reason: None };
    let station = match info {
        Some(i) => i.hdr.as_ref(),
        None => Some(&unknown),
    };
    let kind = kybernow_core::profile::hdr_block(station, settings, capabilities(pc));
    kybernow_core::profile::HdrState { shown: screen_hdr, available: kind.is_none(), reason: kind.map(|k| k.text(i18n::lang())), kind }
}

/// HDR : l'écran de l'accueil est HDR (puce des consoles ; revu à chaque
/// événement `displays`).
#[tauri::command(async)]
fn home_hdr() -> bool {
    home_screen_hdr().0
}

/// HDR : l'écran de la fenêtre de l'accueil (« Cet écran ») et ses luminosités.
fn home_screen_hdr() -> (bool, Option<kybernow_core::profile::HdrLuminance>) {
    let monitors = displays::enumerate();
    let key = system::home_monitor_key();
    let m = key.and_then(|k| monitors.iter().find(|m| m.key.eq_ignore_ascii_case(&k))).or_else(|| monitors.iter().find(|m| m.primary));
    m.filter(|m| m.hdr).map_or((false, None), |m| (true, m.luminance))
}

/// Réponse de `connect` : les écrans que la session prend (l'étape « Préparation
/// des écrans » de la connexion) ; `windowed` : une session suivante
/// (plusieurs ordinateurs), qui s'ouvre dans sa fenêtre sans passer par
/// l'accueil, ou un ordinateur déjà en session, dont la fenêtre passe devant.
#[derive(Serialize)]
struct ConnectInfo {
    screens: usize,
    windowed: bool,
}

/// La fenêtre de connexion d'une session suivante (plusieurs ordinateurs) :
/// le cadre `at` (pixels du bureau) et la zone cliente `client` de la fenêtre
/// de la session qui va s'y montrer, titrée du nom de l'ordinateur comme
/// elle. Rouverte quand la connexion reprend en session.
fn open_connexion(app: &AppHandle, conn: &Arc<Connexion>, at: (i32, i32), client: (u32, u32), name: &str) {
    if conn.label.locked().is_some() {
        return;
    }
    let label = format!("connexion-{}", CONNEXION_SEQ.fetch_add(1, Ordering::Relaxed) + 1);
    *conn.label.locked() = Some(label.clone());
    let id = conn.view.locked().id.clone();
    app.state::<UiState>().connexions.locked().insert(label.clone(), conn.clone());
    let title = format!("{name} · {}", shell::DISPLAY_NAME);
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let script = format!(
            "window.__KN_LANG = '{}'; window.__KN_SESSION = {};",
            i18n::lang().code(),
            serde_json::to_string(&id).unwrap_or_else(|_| "''".into())
        );
        let built = WebviewWindowBuilder::new(&a, &label, WebviewUrl::App("connexion.html".into()))
            .initialization_script(&script)
            .title(&title)
            .inner_size(960.0, 540.0)
            .theme(Some(tauri::Theme::Dark))
            // Le fond de l'écran de connexion, avant même que la page soit là.
            .background_color(Color(5, 7, 10, 255))
            .visible(false)
            .build();
        let w = match built {
            Ok(w) => w,
            Err(e) => {
                log::warn!("fenêtre de connexion de {id} : {e}");
                return;
            }
        };
        let place = move |w: &tauri::WebviewWindow| {
            let _ = w.set_size(tauri::PhysicalSize::new(client.0.max(320), client.1.max(180)));
            let _ = w.set_position(tauri::PhysicalPosition::new(at.0, at.1));
        };
        place(&w);
        let _ = w.show();
        let _ = w.set_focus();
        log::info!("fenêtre de connexion de {id} : {}×{} en ({}, {})", client.0, client.1, at.0, at.1);
        // Windows règle la taille d'une fenêtre à l'échelle de l'écran où elle
        // arrive : une seconde fois, une fois posée.
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            place(&w);
        });
    });
}

/// La fenêtre de connexion part (après `delay` : le passage s'ouvre sur la
/// fenêtre de la session, montrée par-dessus). Fermée d'un geste (« Fermer »,
/// « Annuler », sa croix), elle reste au moins le temps d'un double clic
/// depuis ce geste : le second clic tombe sur elle, pas sur la fenêtre d'une
/// autre session restée dessous, qui l'enverrait à son ordinateur.
fn close_connexion(app: &AppHandle, conn: &Arc<Connexion>, delay: Duration) {
    let Some(label) = conn.label.locked().take() else { return };
    app.state::<UiState>().connexions.locked().remove(&label);
    let delay = match *conn.closed_at.locked() {
        Some(at) => delay.max(double_click_time().saturating_sub(at.elapsed())),
        None => delay,
    };
    log::info!("fenêtre de connexion de {} fermée ({label}, dans {} ms)", conn.name, delay.as_millis());
    let a = app.clone();
    std::thread::spawn(move || {
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
        let b = a.clone();
        let _ = a.run_on_main_thread(move || {
            if let Some(w) = b.get_webview_window(&label) {
                let _ = w.destroy();
            }
        });
    });
}

/// Ce que montre la fenêtre de connexion qui le demande (à son ouverture).
#[tauri::command]
fn connexion_state(window: tauri::WebviewWindow, state: tauri::State<'_, UiState>) -> Option<ConnexionView> {
    state.connexions.locked().get(window.label()).map(|c| c.view.locked().clone())
}

/// « Fermer » sous un échec, ou la fenêtre de connexion fermée : « Annuler » si
/// la connexion se fait encore, sinon elle part.
#[tauri::command(async)]
fn connexion_close(app: AppHandle, window: tauri::WebviewWindow) {
    connexion_closed(&app, window.label());
}

/// La fenêtre de connexion `label` se ferme d'un geste : sa session, et
/// elle seule (`Connexion::session`), est annulée si elle tourne encore ;
/// sinon (le message d'un échec), la fenêtre part, rien d'autre ne s'arrête.
fn connexion_closed(app: &AppHandle, label: &str) {
    let state = app.state::<UiState>();
    let conn = state.connexions.locked().get(label).cloned();
    let Some(conn) = conn else {
        // Déjà en partance (second clic) : elle part, rien d'autre.
        log::info!("fenêtre de connexion {label} fermée : déjà en partance, aucun ordre");
        return;
    };
    *conn.closed_at.locked() = Some(Instant::now());
    let failed = conn.view.locked().error.is_some();
    let why = format!("fenêtre de connexion de {} fermée (« Annuler »)", conn.name);
    // Sa session pas encore inscrite (la fenêtre s'ouvre juste avant) : elle
    // s'arrêtera dès son inscription.
    let id = conn.view.locked().id.clone();
    let launching = state.launching.locked().contains(&id);
    let cancelled = !failed && (end_instance(app, conn.session, &why) || (launching && end_session(app, &id, &why)));
    if !cancelled {
        log::info!(
            "fenêtre de connexion de {} fermée{} : aucune session à arrêter (n°{} finie)",
            conn.name,
            if failed { " (« Fermer » sous l'échec)" } else { "" },
            conn.session
        );
        close_connexion(app, &conn, Duration::ZERO);
        // L'échec d'une session suivante refermé alors que plus rien ne
        // tourne : l'accueil revient.
        show_home_if_idle(app);
    }
}

/// Le temps d'un double clic de Windows (au plus 1 s).
fn double_click_time() -> Duration {
    let ms = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() };
    Duration::from_millis(u64::from(ms.clamp(200, 1000)))
}

/// Le message de l'échec d'une session suivante reste dans sa fenêtre de
/// connexion : elle reste devant (au-dessus des fenêtres des autres
/// sessions, même plein écran, même si l'une d'elles repasse au premier
/// plan) jusqu'à « Fermer » ; elle prend le clavier si l'app est devant.
fn keep_failure_in_front(app: &AppHandle, conn: &Arc<Connexion>) {
    let Some(label) = conn.label.locked().clone() else { return };
    let name = conn.name.clone();
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(w) = a.get_webview_window(&label) else { return };
        let _ = w.set_always_on_top(true);
        let _ = w.unminimize();
        let _ = w.show();
        let front = shell::app_in_front();
        if front {
            let _ = w.set_focus();
        }
        log::info!("échec de {name} : sa fenêtre de connexion reste devant jusqu'à « Fermer »{}", if front { ", avec le clavier" } else { "" });
    });
}

/// Une fenêtre de connexion passe au premier plan : sa session (et elle
/// seule) devient la session active du registre du cœur, si elle tourne.
fn connexion_focused(app: &AppHandle, label: &str) {
    let state = app.state::<UiState>();
    let session = state.connexions.locked().get(label).map(|c| c.session);
    if let Some(session) = session {
        state.with_instance(session, |r| r.host.activate());
    }
}

/// Plus rien ne tourne : ni session, ni connexion qui se lance, ni fenêtre de
/// connexion ouverte (l'échec d'une session suivante).
fn idle(state: &UiState) -> bool {
    state.sessions.locked().is_empty() && state.launching.locked().is_empty() && state.connexions.locked().is_empty() && !state.ps.playing()
}

/// La dernière session finie (ou la fenêtre d'un échec refermée) ramène
/// l'accueil, sauf si une connexion se lance ou qu'une fenêtre de connexion
/// est encore ouverte : il ne passe pas devant elles.
fn show_home_if_idle(app: &AppHandle) {
    let state = app.state::<UiState>();
    if state.quitting.load(Ordering::Relaxed) || !idle(&state) {
        return;
    }
    // Profil Auto : le test demandé pendant la session, maintenant qu'elle est finie.
    if let Some(id) = autotest::take_later() {
        show_action(app, Action::Autotest { id });
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// « Quitter » : l'app part quand la dernière session est finie et qu'aucune
/// connexion ne se lance (une fenêtre de connexion restée sur un échec part
/// avec elle).
/// Rien que quitter couperait : ni session, ni connexion qui démarre, ni
/// partie PlayStation (« Quitter », « Installer la mise à jour »). Une fenêtre
/// de connexion restée sur un échec part avec l'app.
fn can_quit(state: &UiState) -> bool {
    state.sessions.locked().is_empty() && state.launching.locked().is_empty() && !state.ps.playing()
}

fn exit_when_idle(app: &AppHandle) {
    let state = app.state::<UiState>();
    if can_quit(&state) {
        log::info!("quitter");
        app.exit(0);
    }
}

/// L'accueil passe sur l'écran principal de ce PC, où la connexion se lit
/// (docs/link-ui.md, « Plusieurs écrans ») ; rend où il était, pour qu'il y
/// revienne à la fin. Rien s'il y est déjà.
fn home_to_primary(app: &AppHandle, monitors: &[displays::Monitor]) -> Option<tauri::PhysicalPosition<i32>> {
    let main = app.get_webview_window("main")?;
    if main.is_minimized().unwrap_or(false) {
        return None;
    }
    let pos = main.outer_position().ok()?;
    let size = main.outer_size().ok()?;
    let on = displays::at(monitors, pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2)?;
    if monitors[on].primary {
        return None;
    }
    let m = &monitors[displays::primary(monitors)];
    let x = (m.x + (m.width - size.width as i32) / 2).max(m.x);
    let y = (m.y + (m.height - size.height as i32) / 2).max(m.y);
    let _ = main.set_position(tauri::PhysicalPosition::new(x, y));
    Some(pos)
}

/// La fenêtre d'une session suivante (plusieurs ordinateurs) : le mode de
/// l'écran de l'ordinateur, la zone cliente (pixels), le coin haut gauche du
/// cadre et l'écran de ce PC où elle s'ouvre.
struct Follow {
    mode: kybernow_core::console::Mode,
    client: (i32, i32),
    at: (i32, i32),
    monitor: String,
}

/// Les écrans et la fenêtre d'une session ouverte pendant qu'une autre tourne
/// (plusieurs ordinateurs, aussi la session d'un invité) : elle prend « Cet
/// écran » en fenêtre, celui de la fenêtre active, quel que soit le réglage
/// « Écrans » ; les deux règles du cœur (`screens_for_extra_session`, dans
/// `Layout::screens`, et `extra_session_window`).
fn follow_layout(state: &UiState, spec: Spec) -> (Layout, Follow) {
    let fallback = state.active_id().and_then(|a| state.with_session(&a, |r| r.host.bar_window())).unwrap_or(0);
    let layout = Layout { monitors: displays::enumerate(), spec, this_screen: system::active_monitor_key(fallback), extra_session: true };
    let screens = layout.screens();
    let w = screens::extra_session_window(&screens.list[screens.current()]);
    // Les écrans de ce PC sont donnés au cœur en pixels : cadre et mode aussi.
    let client = (w.mode.width as i32, w.mode.height as i32);
    let at = system::frame_origin((w.x.round() as i32, w.y.round() as i32), client);
    let monitor = layout.monitors.get(screens.current()).map(|m| m.key.clone()).unwrap_or_default();
    (layout, Follow { mode: w.mode, client, at, monitor })
}

/// Une connexion qui se lance : sa place (une des quatre) est rendue en
/// sortant de `connect`, session inscrite ou connexion refusée ; si l'app se
/// fermait pendant ce temps, elle part alors (s'il ne reste rien).
struct Launching<'a> {
    app: &'a AppHandle,
    id: String,
}

impl Drop for Launching<'_> {
    fn drop(&mut self) {
        let state = self.app.state::<UiState>();
        {
            let mut launching = state.launching.locked();
            if let Some(i) = launching.iter().position(|x| *x == self.id) {
                launching.remove(i);
            }
        }
        // Une annulation que la connexion n'a pas prise (refusée avant son
        // inscription) ne vaut pas pour la suivante.
        take_cancelled_launch(&state, &self.id);
        if state.quitting.load(Ordering::Relaxed) {
            exit_when_idle(self.app);
        }
    }
}

/// Un ordinateur déjà en session : sa fenêtre passe devant (« Afficher »).
fn show_session_of(state: &UiState, id: &str) -> bool {
    state.with_session(id, |r| r.host.focus(0)).is_some()
}

/// « Afficher la session » de l'accueil, « Afficher » de la barre et du menu
/// de l'icône (plusieurs ordinateurs).
#[tauri::command(async)]
fn show_session(app: AppHandle, state: tauri::State<'_, UiState>, id: String) -> Result<(), String> {
    if let Some(shown) = playstation::show(&app, &id) {
        return if shown { Ok(()) } else { Err(t("errors.no_session", &[])) };
    }
    if show_session_of(&state, &id) { Ok(()) } else { Err(t("errors.no_session", &[])) }
}

/// Lance la session vers un ordinateur avec ses réglages. Pendant une autre
/// session (plusieurs ordinateurs, quatre au plus), elle s'ouvre dans une
/// fenêtre ; un ordinateur déjà en session montre la sienne.
#[tauri::command(async)]
fn connect(app: AppHandle, id: String) -> Result<ConnectInfo, String> {
    let state = app.state::<UiState>();
    // L'app se ferme : rien de nouveau ne démarre.
    if state.quitting.load(Ordering::Relaxed) {
        return Ok(ConnectInfo { screens: 0, windowed: true });
    }
    // Une place parmi les quatre, jamais deux connexions vers le même
    // ordinateur, et tous les contrôles avant de rien montrer (fonds des
    // autres écrans, accueil passé sur l'écran principal, fenêtre de
    // connexion) : une connexion refusée ne laisse rien derrière elle.
    let first = {
        let sessions = state.sessions.locked();
        let mut launching = state.launching.locked();
        if sessions.iter().any(|r| r.id == id) {
            drop(launching);
            drop(sessions);
            show_session_of(&state, &id);
            return Ok(ConnectInfo { screens: 0, windowed: true });
        }
        if launching.contains(&id) {
            return Err(t("errors.session_already_open", &[]));
        }
        // Les sessions de l'app et les connexions qui se lancent, et le
        // registre du cœur (qui refuserait la cinquième de toute façon).
        if sessions.len() + launching.len() >= focus::MAX_SESSIONS || focus::count() >= focus::MAX_SESSIONS {
            return Err(t("errors.four_at_most", &[]));
        }
        let first = sessions.is_empty() && launching.is_empty();
        launching.push(id.clone());
        first
    };
    let _launching = Launching { app: &app, id: id.clone() };
    // Réglages synchronisés : ceux du compte, s'ils arrivent vite.
    synced::before_session(&app, &id);
    let c = state.with_store(|s| s.get(&id).cloned()).ok_or_else(|| t("errors.unknown_computer", &[]))?;
    // Sur lui-même, l'écran virtuel éteindrait les écrans où l'app s'affiche.
    let me = std::env::var("COMPUTERNAME").unwrap_or_default();
    let host_name = state.live.locked().get(&id).and_then(|l| l.info.clone()).map(|i| i.hostname).unwrap_or_default();
    // Un ordinateur d'un compte KyberNow : le jeton d'accès du relais (demandé
    // d'abord : ses refus se disent tout de suite), sur le réseau quand la
    // découverte le voit, par le relais du compte sinon ; ni compte ni mot de
    // passe. Un ordinateur appairé : son compte, sur le réseau d'abord, par le
    // relais sinon quand l'ordinateur y est connu (docs/relais.md) ; une
    // empreinte changée ne bascule jamais.
    let (password, access, route, host, port) = match &c.account {
        Some(ac) => {
            let (access, route, host, port) = account::session_params(&app, &c, ac)?;
            (String::new(), Some(access), route, host, port)
        }
        None => {
            let password = store::password(&id).ok_or_else(|| t("errors.password_missing", &[]))?;
            let route = kybernow_core::session::Route::Auto(relais::route_of(c.relay.as_ref(), c.fingerprint.as_deref()));
            (password, None, route, c.host.clone(), c.port)
        }
    };
    let local = !host.is_empty() && system::is_this_pc(&host);
    if local || (!me.is_empty() && (host_name.eq_ignore_ascii_case(&me) || c.name.eq_ignore_ascii_case(&me))) {
        log::warn!("session refusée : {} ({}) est ce PC", c.name, host);
        return Err(t("errors.this_pc", &[]));
    }
    let pc = {
        let cached = state.this_pc.locked().clone();
        match cached {
            Some(pc) => pc,
            None => {
                let pc = pc::this_pc();
                *state.this_pc.locked() = Some(pc.clone());
                pc
            }
        }
    };
    let s = &c.settings;
    let lighten = battery_choice(&state) == "alleger" && pc::on_battery();
    // Les écrans de ce PC et ce que la Station sait créer : le cœur en fait une
    // session à un ou plusieurs écrans (le même calcul que la ligne de
    // l'accueil). « Cet écran » est relevé ici, avant que l'accueil passe sur
    // l'écran principal : la session le garde (comme la définition, ci-dessous).
    let info = state.live.locked().get(&id).and_then(|l| l.info.clone());
    let spec = displays_spec(s, info.as_ref(), lighten, &pc);
    // Plusieurs ordinateurs : une session ouverte pendant qu'une autre tourne
    // prend « Cet écran » en fenêtre, quel que soit le réglage « Écrans » —
    // l'écran de la fenêtre active —, dans une fenêtre centrée aux deux tiers
    // de cet écran en 16:9, dont la zone cliente est l'écran de l'ordinateur :
    // les deux règles du cœur (`screens_for_extra_session`, dans
    // `Layout::screens`, et `extra_session_window`).
    let (layout, follow) = if first {
        (Layout::now(spec), None)
    } else {
        let (layout, follow) = follow_layout(&state, spec);
        (layout, Some(follow))
    };
    let (mut settings, mode, label) = session_plan(&c, &pc, lighten, follow.as_ref().map(|f| f.mode));
    // PyroWave (docs/link-ui.md « PyroWave ») : « Auto » le prend seulement par câble.
    settings.wired = !host.is_empty() && system::wired_to(&host);
    // Profil Auto : le réseau de ce PC, comparé à celui du test au début de la session.
    let auto_check = autotest::check(&c, &host);
    let plan = screens::session_plan_for(&layout.screens(), &settings, s.virtual_display);
    // HDR : l'écran de la session (« Cet écran », ou dès qu'un des écrans pris l'est) ;
    // le cœur relit chaque écran dans les écrans de ce PC.
    let devices: Option<Vec<usize>> = plan.as_ref().map(|p| p.iter().map(|x| x.device).collect());
    let (screen_hdr, screen_luminance) = layout.screens().session_hdr(devices.as_deref());
    let audio_channels = match s.audio.as_str() {
        "auto" => audio::default_output_channels(),
        other => other.parse().unwrap_or(2),
    };
    let params = SessionParams {
        console_host: host,
        console_port: port,
        user: c.user.clone(),
        password,
        // Un ordinateur d'un compte : l'empreinte d'Admin que le relais donne (vérifiée par le cœur).
        console_fingerprint: c.account.as_ref().and_then(|ac| ac.admin_fp.clone()).or_else(|| c.fingerprint.clone()),
        device: system::device_info(),
        mode,
        audio_channels,
        virtual_display: s.virtual_display,
        settings: settings.clone(),
        auto_reconnect: s.auto_reconnect,
        gamepad_xbox360: s.gamepad != "fidele",
        clipboard: s.clipboard,
        recovery: Default::default(),
        role: SessionRole::Host,
        route,
        screen_hdr,
        screen_luminance,
        capabilities: None,
        account: access,
    };
    let mouse = match s.mouse.as_str() {
        "jeu" => Some(true),
        "bureau" => Some(false),
        _ => None,
    };
    log::info!("session vers {} ({}) : {label}, {}×{} à {} Hz", c.name, c.host, mode.width, mode.height, mode.refresh);
    if let Some(p) = &plan {
        log::info!("session sur {} écrans de ce PC (la Station en crée {} au plus)", p.len(), layout.spec.max_screens);
    }
    let bar = Arc::new(Mutex::new(BarLabel { label: label.clone(), profile: s.profile.clone(), custom: is_custom(&c, Some(&pc)) }));
    let sent = Arc::new(Mutex::new(Sent {
        settings,
        mode,
        screens: plan.as_ref().map_or(1, |p| p.len() as u32),
        pixel_rate: 0.0,
        virtual_display: s.virtual_display,
    }));
    let stats = Arc::new(Mutex::new(None));
    // Plusieurs écrans : la connexion se lit sur l'écran principal (l'accueil
    // y passe), les autres montrent le fond de la connexion sans texte.
    let spot = Arc::new(Mutex::new(Spot::default()));
    if let Some(p) = &plan {
        let target = |i: usize| {
            let m = &layout.monitors[i];
            fonds::Target { key: m.key.clone(), x: m.x, y: m.y, width: m.width.max(0) as u32, height: m.height.max(0) as u32 }
        };
        let others: Vec<fonds::Target> = p.iter().filter(|x| !x.primary).map(|x| target(x.device)).collect();
        {
            let mut sp = spot.locked();
            sp.primary = p.iter().find(|x| x.primary).map(|x| target(x.device));
            sp.others = others.clone();
            sp.before = home_to_primary(&app, &layout.monitors);
        }
        if let Some(f) = state.fonds.get() {
            f.show(&others);
        }
    }
    let spec = layout.spec;
    let this_screen = layout.this_screen.clone();
    // Son identifiant, pris avant tout : sa fenêtre de connexion et sa fin la
    // désignent par lui (`SessionHost::id`).
    let session = window::new_app_id();
    // Session suivante : sa connexion se montre tout de suite dans une
    // fenêtre, là où la sienne va s'ouvrir.
    let connexion = Arc::new(Connexion { session, name: c.name.clone(), ..Default::default() });
    connexion.view.locked().id = id.clone();
    connexion.view.locked().phase = "preparing";
    let share_state = Arc::new(Mutex::new(None));
    let lend_state = Arc::new(Mutex::new(None));
    let route = Arc::new(Mutex::new(None));
    if let Some(f) = &follow {
        log::info!("session suivante : fenêtre de {}×{} sur {} (plusieurs ordinateurs)", f.client.0, f.client.1, f.monitor);
        open_connexion(&app, &connexion, f.at, (f.client.0 as u32, f.client.1 as u32), &c.name);
    }
    // Tenu jusqu'à l'inscription de la session : une fin très rapide (dans
    // `ended`) ne peut pas passer avant.
    let events = Arc::new(UiEvents {
        app: app.clone(),
        id: id.clone(),
        sent: sent.clone(),
        stats: stats.clone(),
        spec,
        this_screen,
        spot: spot.clone(),
        first,
        connexion: connexion.clone(),
        guest: None,
        borrow: None,
        share: share_state.clone(),
        lend: lend_state.clone(),
        lend_seen: std::sync::atomic::AtomicBool::new(false),
        route: route.clone(),
        pads_off: info.as_ref().is_some_and(|i| !i.gamepads_available()),
        pads_told: std::sync::atomic::AtomicBool::new(false),
        pyrowave_told: std::sync::atomic::AtomicBool::new(false),
        pyrowave_strained_pending: std::sync::atomic::AtomicBool::new(false),
        auto_check,
        auto_told: std::sync::atomic::AtomicBool::new(false),
        recording: Mutex::new(None),
        mic: Mutex::new(kybernow_core::micro::MicUi::new(s.microphone)),
    });
    // Face à un Mac, « Ctrl agit comme ⌘ » (avant de tenir la table des sessions).
    let control_as_command = kybernow_core::input::keymap::control_as_command(s.ctrl_command, &computer_os(&state, &id));
    let mut running = state.sessions.locked();
    let host = match SessionHost::start_as(
        session,
        c.name.clone(),
        params,
        HostOptions {
            vsync: s.vsync,
            hardware: true,
            fullscreen: first,
            mouse,
            window_size: match &follow {
                Some(f) => f.client,
                None => system::initial_window_size(mode),
            },
            at: follow.as_ref().map(|f| f.at),
            title_stats: false,
            layout,
            control_as_command,
            immersive: s.immersive.clone(),
            microphone: s.microphone,
        },
        events.clone(),
    ) {
        Ok(h) => h,
        Err(e) => {
            drop(running);
            log::error!("session : fil de la fenêtre refusé ({e})");
            return Err(t("errors.session_thread", &[("error", &e.to_string())]));
        }
    };
    let screens = plan.as_ref().map_or(1, |p| p.len());
    running.push(Running { id: id.clone(), name: c.name.clone(), host, bar, sent, stats, guest: None, borrow: None, share: share_state, lend: lend_state, route, events });
    let cancelled = take_cancelled_launch(&state, &id);
    drop(running);
    drop(_launching);
    // « Quitter » ou « Annuler » pendant le lancement : elle s'arrête tout de
    // suite (Admin prévenue) ; avec « Quitter », l'app part à sa fin.
    after_launch(&app, session, cancelled);
    emit_sessions(&app);
    Ok(ConnectInfo { screens, windowed: !first })
}

/// Un ordre de la barre (ou du passage, « Annuler ») à la session de cet ordinateur.
#[tauri::command(async)]
fn session_command(app: AppHandle, id: String, command: String) {
    // Une partie sur console PlayStation (`ps-…`).
    if playstation::command(&app, &id, &command) {
        return;
    }
    let state = app.state::<UiState>();
    let c = match command.as_str() {
        "disconnect" => {
            end_session(&app, &id, "« Se déconnecter » de la barre, ou « Annuler » du passage de l'accueil");
            return;
        }
        "fullscreen" => Command::Fullscreen,
        "mouse" => Command::MouseToggle,
        "keys:ctrl-alt-del" => Command::CtrlAltDel,
        "keys:win" => Command::WinKey,
        "keys:alt-tab" => Command::AltTab,
        "keys:print" => Command::PrintScreen,
        "keys:win-d" => Command::WinD,
        "keys:win-l" => Command::WinL,
        "keys:alt-f4" => Command::AltF4,
        "keys:esc" => Command::Escape,
        "keys:task-manager" => Command::TaskManager,
        "keys:f11" => Command::F11,
        // Face à une Station Mac.
        "keys:mac-switch" => Command::MacSwitchApp,
        "keys:mac-spotlight" => Command::MacSpotlight,
        "keys:mac-lock" => Command::MacLock,
        "keys:mac-screenshot" => Command::MacScreenshot,
        "keys:mac-force-quit" => Command::MacForceQuit,
        _ => return,
    };
    state.with_session(&id, |r| r.host.command(c));
}

/// Le menu Souris de la barre : « Auto », « Souris du bureau », « Souris des
/// jeux », appliqué tout de suite et gardé pour l'ordinateur, comme le menu de
/// l'icône.
#[tauri::command(async)]
fn session_mouse(app: AppHandle, id: String, mode: String) {
    let mode = match mode.as_str() {
        "jeu" => "jeu",
        "bureau" => "bureau",
        _ => "auto",
    };
    tray_command(&app, Cmd::Mouse(id, mode));
}

/// « Garder le son » de la barre (bouton du haut-parleur) ou du menu de
/// l'icône, pour la session de cet ordinateur.
#[tauri::command(async)]
fn session_keep_sound(app: AppHandle, id: String, on: bool) {
    set_keep_sound(&app, &id, on);
}

fn set_keep_sound(app: &AppHandle, id: &str, on: bool) {
    if app.state::<UiState>().with_session(id, |r| r.host.set_keep_sound(on)).is_some() {
        let _ = app.emit("session-keep-sound", KeepSoundEvent { id: id.to_string(), on });
        tray::refresh();
    }
}

/// « HDR » de la barre (docs/link-ui.md, « HDR ») : la puce, cochable, près
/// du profil ; seulement quand un écran de la session est HDR, jamais pour
/// un invité ; inactive avec la raison.
#[derive(Debug, Clone, Serialize)]
struct BarHdr {
    on: bool,
    available: bool,
    reason: Option<String>,
}

#[derive(Clone, Serialize)]
struct BarHdrEvent {
    id: String,
    hdr: Option<BarHdr>,
}

/// La puce « HDR » de la barre pour la session `id`.
fn bar_hdr(state: &UiState, id: &str) -> Option<BarHdr> {
    let (settings, screen_hdr, guest, borrow) =
        state.with_session(id, |r| (r.sent.locked().settings.clone(), r.host.screen_hdr(), r.guest.is_some(), r.borrow.is_some()))?;
    let pc = state.this_pc.locked().clone().unwrap_or_default();
    let info = state.live.locked().get(id).and_then(|l| l.info.clone());
    bar_hdr_with(&pc, info.as_ref(), &settings, screen_hdr, guest, borrow)
}

/// [`bar_hdr`] d'après ce que la session a déjà donné (sans prendre de
/// verrou : le menu de l'icône l'appelle en parcourant les sessions) : ce PC,
/// l'annonce de la Station, les réglages de la session, son écran, invité ou
/// ami qui emprunte.
fn bar_hdr_with(pc: &ThisPc, info: Option<&Info>, settings: &kybernow_core::profile::StreamSettings, screen_hdr: bool, guest: bool, borrow: bool) -> Option<BarHdr> {
    if guest || !screen_hdr {
        return None;
    }
    // L'ami d'un prêt ne lit pas l'annonce de la Station : Kyber repasse en SDR ce qu'il ne peut pas faire.
    let chip = hdr_chip(if borrow { None } else { info }, settings, pc, true);
    Some(BarHdr { on: chip.available && settings.hdr, available: chip.available, reason: chip.reason })
}

/// « HDR » coché ou décoché en session (barre, menu de l'icône) : gardé
/// pour cet ordinateur (pas pour un ami qui emprunte), et appliqué tout de
/// suite (le cœur relance l'image, étape « Premières images »).
#[tauri::command(async)]
fn session_hdr(app: AppHandle, id: String, on: bool) -> Result<(), String> {
    let state = app.state::<UiState>();
    if !state.in_session(&id) {
        return Err(t("errors.no_session", &[]));
    }
    let before = state.with_store(|s| match s.get_mut(&id) {
        Some(c) => {
            let before = c.settings.clone();
            c.settings.hdr = Some(on);
            s.save();
            Some(before)
        }
        None => None,
    });
    if let Some(before) = before {
        synced::changed(&app, &id, &before);
        emit_computers(&app);
    }
    if apply_hdr(&app, &id, on) { Ok(()) } else { Err(t("errors.session_not_ready", &[])) }
}

/// Le choix « HDR » donné à la session en cours de cet ordinateur (rien
/// sans session) ; la barre et le menu de l'icône suivent.
fn apply_hdr(app: &AppHandle, id: &str, on: bool) -> bool {
    let state = app.state::<UiState>();
    let Some(applied) = state.with_session(id, |r| {
        r.sent.locked().settings.hdr = on;
        r.host.set_hdr(on)
    }) else {
        return false;
    };
    let _ = app.emit_to("bar", "bar-hdr", BarHdrEvent { id: id.to_string(), hdr: bar_hdr(&state, id) });
    tray::refresh();
    applied
}

/// Autre profil depuis la barre : enregistré pour cet ordinateur (comme
/// depuis l'accueil) et appliqué tout de suite à sa session.
#[tauri::command(async)]
fn session_profile(app: AppHandle, id: String, profile: String) -> Result<autotest::Chosen, String> {
    let state = app.state::<UiState>();
    if state.with_session(&id, |_| ()).is_none() {
        return Err(t("errors.no_session", &[]));
    }
    Profile::parse(&profile).ok_or_else(|| t("errors.unknown_profile", &[]))?;
    // Auto (docs/profil-auto.md) : avec un test gardé, ses réglages tout de
    // suite ; sans test, le panneau du test s'ouvre à la fin de la session
    // (`later` : la barre le dit dans sa capsule).
    if profile == "auto" {
        return autotest::choose(&app, &id);
    }
    let c = save_profile(&app, &id, &profile)?;
    emit_computers(&app);
    apply_to_session(&app, &id, &c).map(|_| autotest::Chosen::Applied)
}

/// Le profil de l'ordinateur `id`, tel qu'il est enregistré (`c`), appliqué
/// tout de suite à sa session (les réglages du profil seulement : la taille
/// de l'écran ne change pas) et à la barre. Rien sans session.
fn apply_to_session(app: &AppHandle, id: &str, c: &Computer) -> Result<(), String> {
    let state = app.state::<UiState>();
    let Some((bar, sent)) = state.with_session(id, |r| (r.bar.clone(), r.sent.clone())) else {
        return Ok(());
    };
    let profile = c.settings.profile.clone();
    let pc = state.this_pc.locked().clone().unwrap_or_else(pc::this_pc);
    let lighten = battery_choice(&state) == "alleger" && pc::on_battery();
    // Les réglages du profil seulement : la taille de l'écran ne change pas.
    let (mut settings, _, label) = session_plan(c, &pc, lighten, None);
    // Le câble ne change pas avec le profil.
    settings.wired = sent.locked().settings.wired;
    sent.locked().settings = settings.clone();
    // Un profil choisi remet ses réglages (`save_profile`) : plus de « Personnalisé ».
    let shown = BarLabel { label, profile, custom: false };
    *bar.locked() = shown.clone();
    let _ = app.emit_to("bar", "bar-label", BarLabelEvent { id: id.to_string(), label: shown });
    let applied = state.with_session(id, |r| r.host.set_settings(settings)).unwrap_or(false);
    if applied { Ok(()) } else { Err(t("errors.session_not_ready", &[])) }
}

/// PyroWave (« Format vidéo » des réglages) : ce PC joint l'ordinateur `id`
/// par câble (Ethernet), pour les aides d'« Auto » et l'avertissement Wi-Fi.
#[tauri::command(async)]
fn computer_wired(app: AppHandle, id: String) -> bool {
    let state = app.state::<UiState>();
    let host = state.with_store(|s| s.get(&id).map(|c| c.host.clone())).unwrap_or_default();
    !host.is_empty() && system::wired_to(&host)
}

/// « Passer en HEVC » du bandeau de PyroWave (docs/link-ui.md) : l'image
/// repart en HEVC pour cette session seulement (le choix de l'ordinateur
/// reste PyroWave) ; le cœur relance l'image.
#[tauri::command(async)]
fn session_hevc(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<UiState>();
    let Some((bar, sent)) = state.with_session(&id, |r| (r.bar.clone(), r.sent.clone())) else {
        return Err(t("errors.no_session", &[]));
    };
    let pc = state.this_pc.locked().clone().unwrap_or_default();
    let settings = {
        let mut sent = sent.locked();
        sent.settings.codec = kybernow_core::profile::Codec::Hevc;
        // 4:4:4 en HEVC seulement si la carte le décode.
        sent.settings.yuv444 = sent.settings.yuv444 && pc.hevc444;
        sent.settings.clone()
    };
    let shown = {
        let mut b = bar.locked();
        b.label = format!("{} · {}{}", settings.label(), settings.codec.label(), if settings.yuv444 { " 4:4:4" } else { "" });
        b.custom = true;
        b.clone()
    };
    let _ = app.emit_to("bar", "bar-label", BarLabelEvent { id: id.clone(), label: shown });
    log::info!("PyroWave : « Passer en HEVC » pour cette session");
    let applied = state.with_session(&id, |r| r.host.set_settings(settings)).unwrap_or(false);
    if applied { Ok(()) } else { Err(t("errors.session_not_ready", &[])) }
}

/// Le bandeau de PyroWave à montrer : `pyrowave_relay` par le relais,
/// `pyrowave_wifi` sans câble ; rien par câble sur le réseau de l'ordinateur.
fn pyrowave_link(relayed: bool, wired: bool) -> Option<&'static str> {
    if relayed {
        Some("pyrowave_relay")
    } else if !wired {
        Some("pyrowave_wifi")
    } else {
        None
    }
}

/// Le profil d'un ordinateur, retouches effacées (comme l'accueil).
fn save_profile(app: &AppHandle, id: &str, profile: &str) -> Result<Computer, String> {
    let state = app.state::<UiState>();
    let (c, before) = state.with_store(|s| {
        let c = s.get_mut(id).ok_or_else(|| t("errors.unknown_computer", &[]))?;
        let before = c.settings.clone();
        c.settings.profile = profile.into();
        c.settings.codec = None;
        c.settings.yuv444 = None;
        c.settings.mbps = None;
        c.settings.reliable = None;
        let c = c.clone();
        s.save();
        Ok::<_, String>((c, before))
    })?;
    // Réglages synchronisés : `{profile}` part au relais du compte.
    synced::changed(app, id, &before);
    Ok(c)
}

/// Un profil choisi sur l'accueil ou dans le menu de l'icône : enregistré
/// pour cet ordinateur ; pendant sa session, appliqué tout de suite, comme
/// depuis la barre. Auto (`autotest::choose`) : avec un test gardé, ses
/// réglages (`applied`) ; sans test, l'interface ouvre le panneau du test
/// (`test`), ou il s'ouvrira à la fin de la session (`later`).
#[tauri::command(async)]
fn set_profile(app: AppHandle, id: String, profile: String) -> Result<autotest::Chosen, String> {
    Profile::parse(&profile).ok_or_else(|| t("errors.unknown_profile", &[]))?;
    if profile == "auto" {
        let chosen = autotest::choose(&app, &id)?;
        if chosen == autotest::Chosen::Later {
            autotest::later_told(&app, &id);
        }
        return Ok(chosen);
    }
    let state = app.state::<UiState>();
    if state.in_session(&id) {
        session_profile(app.clone(), id, profile)?;
        return Ok(autotest::Chosen::Applied);
    }
    save_profile(&app, &id, &profile)?;
    emit_computers(&app);
    Ok(autotest::Chosen::Applied)
}

/// Largeur de la fenêtre de la barre de session à sa création, en points ;
/// ensuite, celle de la barre (`bar_width`), bornée par la session.
const BAR_WINDOW_WIDTH: f64 = 1240.0;

/// La fenêtre de la barre n'est jamais plus étroite (ses menus et panneaux,
/// 400 points au plus, s'ouvrent sous leur bouton), en points.
const BAR_WINDOW_MIN: f64 = 720.0;

/// Où la fenêtre de la barre se pose : le milieu et le haut de la session, les
/// bords de son écran (pixels), l'échelle de l'écran et la taille de la
/// fenêtre (points).
#[derive(Clone, Copy)]
struct BarPlace {
    center: i32,
    top: i32,
    left: i32,
    right: i32,
    scale: f64,
    width: f64,
    height: f64,
}

static BAR_PLACE: Mutex<Option<BarPlace>> = Mutex::new(None);

/// La fenêtre de la barre à sa place : centrée sur la session, sans sortir de
/// son écran.
fn place_bar(bar: &tauri::WebviewWindow) {
    let Some(p) = *BAR_PLACE.locked() else { return };
    let w = (p.width * p.scale).round() as i32;
    let x = (p.center - w / 2).max(p.left).min(p.right - w);
    let _ = bar.set_size(tauri::Size::Physical(tauri::PhysicalSize::new(w as u32, (p.height * p.scale).round() as u32)));
    let _ = bar.set_position(tauri::PhysicalPosition::new(x, p.top));
}

/// Pose la fenêtre de la barre sur la session de la fenêtre `hwnd`, comme
/// [`show_bar_notice`] : centrée, à la largeur de la session (sans dépasser
/// son écran), à sa taille sans menu. Pour la barre d'une partie sur console.
pub(crate) fn place_bar_for(bar: &tauri::WebviewWindow, hwnd: isize) {
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let m = MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(m, &mut mi);
    }
    let r = mi.rcMonitor;
    let (center, top, scale, client) = window::bar_anchor(hwnd).unwrap_or((
        (r.left + r.right) / 2,
        r.top,
        bar.scale_factor().unwrap_or(1.0) as f32,
        r.right - r.left,
    ));
    let scale = scale as f64;
    let room = (client.min(r.right - r.left) as f64 / scale).floor();
    *BAR_PLACE.locked() = Some(BarPlace {
        center,
        top: top.max(r.top),
        left: r.left,
        right: r.right,
        scale,
        width: room.max(BAR_WINDOW_MIN.min((r.right - r.left) as f64 / scale)),
        height: 76.0,
    });
    place_bar(bar);
}

/// La barre de session au bord haut de la zone cliente de la session (en
/// plein écran, le haut de l'écran), centrée sur elle : là où le trait la
/// signale et où le pointeur l'ouvre. À sa taille sans menu. Une seule barre :
/// elle sert la session de l'ordinateur `id`, celle de la fenêtre `hwnd`.
fn show_bar(app: &AppHandle, id: &str, hwnd: isize, pinned: bool) {
    show_bar_with(app, id, hwnd, pinned, None);
}

/// [`show_bar`] ; `notice` : un avis bref à montrer (changement de rôle d'un
/// invité). La barre se montre alors sans prendre le clavier à la session (une
/// manette ou une touche en cours ne s'interrompt pas) et repart d'elle-même.
fn show_bar_with(app: &AppHandle, id: &str, hwnd: isize, pinned: bool, notice: Option<String>) {
    show_bar_notice(app, id, hwnd, pinned, notice, None, None);
}

/// [`show_bar_with`] ; `panel` : l'avis est celui d'une demande (à rejoindre :
/// `share`, à emprunter : `lend`), que la barre rend choisissable (il ouvre le
/// panneau de « Partager » ou de « Prêter l'ordinateur… ») ; `icon` : celle de
/// l'avis dans sa capsule (`BarShown::notice_icon`).
pub(crate) fn show_bar_notice(app: &AppHandle, id: &str, hwnd: isize, pinned: bool, notice: Option<String>, panel: Option<&str>, icon: Option<&'static str>) {
    let state = app.state::<UiState>();
    let Some((title, label, multi, keep_sound, session, guest, share, route, borrow, lend_view)) = state.with_session(id, |r| {
        (
            r.name.clone(),
            r.bar.locked().clone(),
            r.host.stream_count() > 1,
            r.host.keep_sound(),
            r.host.id(),
            r.guest.clone(),
            r.share.locked().clone(),
            r.route.locked().clone(),
            r.borrow.clone(),
            r.lend.locked().clone(),
        )
    }) else {
        return;
    };
    // KyberNow Share : l'invité (rôle, hôte) ou, pour une session d'hôte, « Partager ».
    let guest = guest.map(|g| GuestBar {
        role: g.role.locked().as_str(),
        host: g.host.clone(),
        quality: g.quality.locked().as_str(),
        image: g.image.locked().clone(),
        watch: g.watch.clone(),
    });
    // Prêt de l'ordinateur : l'ami qui emprunte (barre réduite) ou, pour l'hôte, « Prêter l'ordinateur… ».
    let host_session = guest.is_none() && borrow.is_none();
    let share = host_session.then(|| share::view(app, id, share.as_ref()));
    let lend = host_session.then(|| lend::panel(app, id, lend_view.as_ref()));
    let borrow = borrow.map(|b| b.bar());
    // La souris et le presse-papiers de l'ordinateur (un invité : Auto, et le presse-papiers n'est pas partagé).
    let (mouse, clipboard) = match &guest {
        Some(_) => ("auto".to_string(), false),
        None => state
            .with_store(|s| s.get(id).map(|c| (c.settings.mouse.clone(), c.settings.clipboard)))
            .unwrap_or_else(|| ("auto".to_string(), false)),
    };
    let mac = host_session && kybernow_core::input::keymap::station_is_mac(&computer_os(&state, id));
    let before = state.bar_session.locked().replace(id.to_string());
    // La barre servait une partie sur console : la console reçoit de nouveau la manette.
    if let Some(prev) = before.filter(|p| p != id && crate::consoles::host_of(p).is_some()) {
        playstation::bar_hidden(app, &prev, false);
    }
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe {
        let m = MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST);
        let _ = GetMonitorInfoW(m, &mut mi);
    }
    let r = mi.rcMonitor;
    let Some(bar) = app.get_webview_window("bar") else { return };
    // Session réduite ou sans fenêtre : le haut de l'écran.
    let (center, top, scale, client) = window::bar_anchor(hwnd).unwrap_or((
        (r.left + r.right) / 2,
        r.top,
        bar.scale_factor().unwrap_or(1.0) as f32,
        r.right - r.left,
    ));
    // La place de la barre : la largeur de la session (sans dépasser son écran), à
    // l'échelle de l'écran où elle va. La fenêtre prend d'abord toute cette place, puis
    // la largeur de la barre (`bar_width`) ; toujours à sa taille sans menu en se montrant.
    let scale = scale as f64;
    let room = (client.min(r.right - r.left) as f64 / scale).floor();
    *BAR_PLACE.locked() = Some(BarPlace {
        center,
        top: top.max(r.top),
        left: r.left,
        right: r.right,
        scale,
        width: room.max(BAR_WINDOW_MIN.min((r.right - r.left) as f64 / scale)),
        height: 76.0,
    });
    place_bar(&bar);
    let _ = bar.set_always_on_top(true);
    if notice.is_some() {
        share::show_without_focus(&bar);
    } else {
        let _ = bar.show();
    }
    window::set_bar_session(Some(session));
    let sessions = state.sessions.locked().len();
    let route = route.map(|r| relais::RouteView::new(&r, i18n::lang()));
    let shown = BarShown {
        id: id.to_string(),
        label,
        title,
        mouse,
        clipboard,
        mac,
        room,
        stats: panel == Some("stats"),
        pinned,
        multi,
        keep_sound,
        sessions,
        guest,
        share,
        lend,
        borrow,
        recording: recording::bar_view(app, id),
        notice,
        notice_opens_share: panel == Some("share"),
        notice_opens_lend: panel == Some("lend"),
        notice_opens_settings: panel == Some("camera_settings"),
        notice_icon: icon,
        route,
        hdr: bar_hdr(&state, id),
        mic: state.with_session(id, |r| r.events.bar_mic()).flatten(),
        wired: state.with_session(id, |r| r.sent.locked().settings.wired),
        notice_kind: panel.and_then(|p| ["pyrowave_wifi", "pyrowave_relay", "pyrowave_strained", "auto_network"].into_iter().find(|k| *k == p)),
        console: None,
    };
    let _ = app.emit_to("bar", "bar-shown", shown);
    let _ = app.emit_to("bar", "bar-computers", bar_computer_list(app));
}

/// « Statistiques » du menu de l'icône : la barre et ses mesures en direct
/// pour la session de cet ordinateur (la session active), ou cachée si elle
/// est déjà là ; le clavier reste à la session.
fn toggle_stats(app: &AppHandle, id: &str) {
    let state = app.state::<UiState>();
    // Sur la fenêtre de cette session où est le pointeur (une seule barre).
    let Some(hwnd) = state.with_session(id, |r| r.host.bar_window()) else {
        return;
    };
    toggle_stats_on(app, id, hwnd);
}

/// [`toggle_stats`] sur la fenêtre `hwnd` (Ctrl+Alt+Maj+S : celle qui a le clavier).
fn toggle_stats_on(app: &AppHandle, id: &str, hwnd: isize) {
    let (a, id) = (app.clone(), id.to_string());
    let _ = app.run_on_main_thread(move || {
        let shown = a.get_webview_window("bar").and_then(|b| b.is_visible().ok()).unwrap_or(false);
        let same = a.state::<UiState>().bar_session.locked().as_deref() == Some(id.as_str());
        if shown && same {
            bar_hide(a.clone(), Some(id), None);
            return;
        }
        // La barre et son panneau « Statistiques ».
        show_bar_notice(&a, &id, hwnd, true, None, Some("stats"), None);
        // La fenêtre où la barre s'ouvre devient la fenêtre active.
        a.state::<UiState>().with_session(&id, |r| r.host.focus(hwnd));
    });
}

/// Un menu de la barre s'ouvre : la fenêtre grandit vers le bas le temps du
/// menu (transparente, elle ne doit pas cacher la session le reste du temps),
/// de `height` points quand le menu le dit (« Ordinateurs », selon leur nombre).
#[tauri::command(async)]
fn bar_menu(app: AppHandle, open: bool, height: Option<f64>) {
    if let Some(bar) = app.get_webview_window("bar") {
        let scale = bar.scale_factor().unwrap_or(1.0);
        // Pas plus bas que l'écran où la barre est posée (le panneau de partage, haut,
        // défile alors dans la fenêtre).
        let room = match (bar.current_monitor(), bar.outer_position()) {
            (Ok(Some(m)), Ok(p)) => f64::from(m.position().y + m.size().height as i32 - p.y) / scale,
            _ => 900.0,
        };
        let top = 900.0f64.min(room.max(300.0));
        let h = if open { height.filter(|h| h.is_finite()).unwrap_or(300.0).clamp(300.0, top) } else { 76.0 };
        let placed = BAR_PLACE.locked().as_mut().map(|p| p.height = h).is_some();
        if placed {
            place_bar(&bar);
        } else {
            let _ = bar.set_size(tauri::Size::Physical(tauri::PhysicalSize::new((BAR_WINDOW_WIDTH * scale) as u32, (h * scale) as u32)));
        }
    }
}

/// La barre dit sa largeur en points (le texte des mesures et la langue la
/// font varier) : le pointeur l'ouvre dans une zone du bord haut centrée de
/// cette largeur, et la fenêtre de la barre la suit (la session n'est cachée
/// que là où est la barre).
#[tauri::command]
fn bar_width(app: AppHandle, width: f64) {
    window::set_bar_width(width);
    if !width.is_finite() || width < 1.0 {
        return;
    }
    let changed = BAR_PLACE.locked().as_mut().is_some_and(|p| {
        let screen = (p.right - p.left) as f64 / p.scale;
        let w = (width + 24.0).max(BAR_WINDOW_MIN).min(screen);
        let changed = (w - p.width).abs() >= 1.0;
        p.width = w;
        changed
    });
    if changed && let Some(bar) = app.get_webview_window("bar") {
        place_bar(&bar);
    }
}

/// La barre se cache : le clavier revient à la session qu'elle servait
/// (`id`, sinon celle qu'elle sert pour l'app), sauf `focus: false` (une
/// session de plus ouverte depuis la barre : sa fenêtre de connexion le garde).
#[tauri::command(async)]
fn bar_hide(app: AppHandle, id: Option<String>, focus: Option<bool>) {
    if let Some(bar) = app.get_webview_window("bar") {
        let _ = bar.hide();
    }
    window::set_bar_session(None);
    // Un menu ouvert laissait la fenêtre agrandie pour la fois suivante.
    bar_menu(app.clone(), false, None);
    let state = app.state::<UiState>();
    let served = state.bar_session.locked().take();
    show_pending_strained(&app);
    // Une partie sur console : la console reçoit de nouveau la manette.
    if let Some(ps) = served.as_deref().or(id.as_deref()).filter(|i| crate::consoles::host_of(i).is_some()) {
        playstation::bar_hidden(&app, ps, focus != Some(false));
        return;
    }
    if focus == Some(false) {
        return;
    }
    if let Some(id) = id.or(served) {
        state.with_session(&id, |r| r.host.command(Command::Focus));
    }
}

/// PyroWave : la barre vient de se cacher ; le bandeau « Le réseau sature »
/// qui attendait (arrivé barre ouverte) se montre, pour une session à la
/// fois (une autre attend la fermeture suivante). Oublié si l'image n'est
/// plus en PyroWave (« Passer en HEVC » choisi entre-temps) ; une session
/// finie n'est plus dans la liste.
fn show_pending_strained(app: &AppHandle) {
    use std::sync::atomic::Ordering;
    let state = app.state::<UiState>();
    let found = state.sessions.locked().iter().find_map(|r| {
        if !r.events.pyrowave_strained_pending.swap(false, Ordering::Relaxed) {
            return None;
        }
        let pyrowave = r.stats.locked().as_ref().is_some_and(|s| s.decoder.starts_with("PyroWave"));
        if !pyrowave {
            log::info!("[{}] PyroWave : l'image n'est plus en PyroWave, bandeau « Le réseau sature » oublié", r.name);
            return None;
        }
        Some((r.id.clone(), r.events.connexion.name.clone()))
    });
    if let Some((id, name)) = found {
        log::info!("[{name}] PyroWave : la barre s'est cachée, bandeau « Le réseau sature »");
        share::panel_notice(app, &id, &name, &t("pyrowave.banner_text", &[]), "pyrowave_strained");
    }
}

/// Les réglages s'écartent du profil (« Personnalisé ») : `isCustom` de
/// l'interface (ui/src/lib/api.ts). Le format du profil Auto est celui du test.
fn is_custom(c: &Computer, pc: Option<&ThisPc>) -> bool {
    let s = &c.settings;
    let base = match c.auto.as_ref().filter(|_| s.profile == "auto") {
        Some(r) => r.codec.key(),
        None if pc.is_none_or(|p| p.hevc) => "hevc",
        None => "h264",
    };
    s.codec.as_deref().is_some_and(|c| c != base) || s.yuv444.is_some() || s.mbps.is_some() || s.reliable.is_some()
}

/// Ce que le menu de l'icône montre (`menu.rs`), lu à chaque ouverture et,
/// menu ouvert, chaque seconde.
pub fn tray_state(app: &AppHandle) -> menu::State {
    let state = app.state::<UiState>();
    let pc = state.this_pc.locked().clone();
    let computers = state.with_store(|s| s.computers.clone());
    let waking: Vec<String> = state.waking.locked().keys().cloned().collect();
    let following: HashMap<String, Kind> = state.following.locked().iter().map(|(k, (p, _))| (k.clone(), p.kind)).collect();
    // HDR : ce que chaque Station annonce, lu avant la liste des sessions (pas deux verrous à la fois).
    let infos: HashMap<String, Info> = state.live.locked().iter().filter_map(|(k, l)| l.info.clone().map(|i| (k.clone(), i))).collect();
    let pc_now = pc.clone().unwrap_or_default();
    let sessions: Vec<menu::Session> = state
        .sessions
        .lock()
        .unwrap()
        .iter()
        .map(|r| {
            let stats = r.stats.locked().clone();
            let sent = r.sent.locked();
            // Un invité : l'ordinateur qui partage et son rôle, sans réglages de l'app.
            let guest = r.guest.as_ref().map(|g| (g.computer.clone(), *g.role.locked()));
            menu::Session {
                id: r.id.clone(),
                name: match (&guest, &r.borrow) {
                    (Some((computer, _)), _) => computer.clone(),
                    // L'ami qui emprunte : « Ordinateur de {hôte} ».
                    (None, Some(b)) => b.title.clone(),
                    (None, None) => computers.iter().find(|c| c.id == r.id).map(|c| c.name.clone()).unwrap_or_default(),
                },
                latency_ms: stats.as_ref().and_then(|s| s.latency_ms),
                fps: stats.as_ref().map(|s| s.fps),
                mbps: Some(sent.mbps()),
                fullscreen: r.host.fullscreen_now(),
                multi: r.host.stream_count() > 1,
                mouse: match &guest {
                    Some(_) => "auto".into(),
                    None => computers.iter().find(|c| c.id == r.id).map(|c| c.settings.mouse.clone()).unwrap_or_default(),
                },
                keep_sound: r.host.keep_sound(),
                hdr: bar_hdr_with(&pc_now, infos.get(&r.id), &sent.settings, r.host.screen_hdr(), guest.is_some(), r.borrow.is_some())
                    .filter(|h| h.available)
                    .map(|h| h.on),
                guest: guest.map(|(_, role)| role),
                watch: r.guest.as_ref().and_then(|g| g.watch.as_ref().map(|w| menu::WatchSession { title: g.title.clone(), stop: w.stop.clone() })),
                recording: if r.guest.is_none() && r.borrow.is_none() { recording::menu_item(&r.events) } else { None },
            }
        })
        .collect();
    let active = state.active_id();
    let full = state.full();
    let relayed = state.sessions_via_relay();
    let live = state.live.locked();
    let computers = computers
        .iter()
        .map(|c| {
            let l = live.get(&c.id);
            let station = l.and_then(|l| l.info.as_ref()?.station.as_ref()).map(|s| menu::Station {
                busy: match following.get(&c.id) {
                    Some(Kind::Restart) => Some(menu::Busy::Restarting),
                    Some(Kind::Update) => Some(menu::Busy::Updating),
                    None if s.restarting => Some(menu::Busy::Restarting),
                    None if s.update.installing => Some(menu::Busy::Updating),
                    None => None,
                },
                update: s.update.version.clone().filter(|_| s.update.available && s.update.installable),
            });
            menu::Computer {
                id: c.id.clone(),
                name: c.name.clone(),
                online: l.and_then(|l| l.online),
                away: l.is_some_and(|l| l.away),
                via_relay: l.is_some_and(|l| l.away) || relayed.contains(&c.id),
                profile: c.settings.profile.clone(),
                custom: is_custom(c, pc.as_ref()),
                can_wake: !c.wake.is_empty(),
                waking: waking.contains(&c.id),
                station,
                // Droit « Regarder » d'un compte : « Regarder » (à la place de « Se connecter » sans « Ouvrir le bureau »).
                watch: c.account.as_ref().filter(|ac| ac.can_watch()).map(|ac| menu::Watch {
                    only: ac.watch_only(),
                    word: kybernow_core::share::watch_word(i18n::lang(), "action"),
                }),
            }
        })
        .collect();
    menu::State { sessions, active, full, computers, app_update: state.update.locked().as_ref().map(|u| u.version.clone()) }
}

/// Ce qui est choisi dans le menu de l'icône, sur son propre fil (les appels
/// à Admin et les questions ne bloquent pas l'icône). Les mêmes commandes que
/// l'accueil, les réglages et la barre.
pub fn tray_command(app: &AppHandle, cmd: Cmd) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<UiState>();
        // Les commandes d'une session portent son ordinateur.
        let session = |id: &str, c: Command| {
            state.with_session(id, |r| r.host.command(c));
        };
        let fail = |r: Result<(), String>| {
            if let Err(e) = r {
                tray::alert(&e);
            }
        };
        match cmd {
            // Le clavier revient ensuite à la session (le menu l'avait pris).
            Cmd::Fullscreen(id) => {
                session(&id, Command::Fullscreen);
                session(&id, Command::Focus);
            }
            Cmd::Mouse(id, mode) => {
                // Gardé pour l'ordinateur de la session, comme dans ses réglages.
                if state.in_session(&id) {
                    state.with_store(|s| {
                        if let Some(c) = s.get_mut(&id) {
                            c.settings.mouse = mode.into();
                            s.save();
                        }
                    });
                    emit_computers(&app);
                }
                session(
                    &id,
                    match mode {
                        "jeu" => Command::MouseGame,
                        "bureau" => Command::MouseDesktop,
                        _ => Command::MouseAuto,
                    },
                );
                session(&id, Command::Focus);
            }
            Cmd::Statistics(id) => toggle_stats(&app, &id),
            Cmd::Recording(id) => recording::toggle(&app, &id),
            Cmd::Disconnect(id) => {
                end_session(&app, &id, "« Se déconnecter » du menu de l'icône");
            }
            Cmd::Show(id) => {
                show_session_of(&state, &id);
            }
            Cmd::KeepSound(id) => {
                let on = state.with_session(&id, |r| r.host.keep_sound()).unwrap_or(false);
                set_keep_sound(&app, &id, !on);
            }
            Cmd::Hdr(id) => {
                let on = state.with_session(&id, |r| r.sent.locked().settings.hdr).unwrap_or(false);
                let _ = session_hdr(app.clone(), id, !on);
            }
            Cmd::Connect(id) => show_action(&app, Action::Connect { id }),
            Cmd::Watch(id) => show_action(&app, Action::Watch { id }),
            Cmd::Wake(id) => fail(wake(app.clone(), id).map(|_| ())),
            Cmd::Retry(id) => {
                let _ = poll_now(app.clone(), id);
            }
            // Auto sans test gardé : l'accueil revient sur le panneau du test.
            Cmd::Profile(id, "auto") => match autotest::choose(&app, &id) {
                Ok(autotest::Chosen::Test) => show_action(&app, Action::Autotest { id }),
                Ok(autotest::Chosen::Later) => autotest::later_told(&app, &id),
                r => fail(r.map(|_| ())),
            },
            Cmd::Profile(id, p) => fail(set_profile(app.clone(), id, p.into()).map(|_| ())),
            Cmd::Settings(id) => show_action(&app, Action::Settings { id, rubric: None }),
            Cmd::Admin(id) => fail(open_admin(app.clone(), id)),
            Cmd::Restart(id) => station_from_menu(&app, &id, Kind::Restart),
            Cmd::Update(id) => station_from_menu(&app, &id, Kind::Update),
            Cmd::Open => show_action(&app, Action::Open),
            Cmd::Add => show_action(&app, Action::Add),
            Cmd::Join => show_action(&app, Action::Join { link: None }),
            Cmd::AppSettings => show_action(&app, Action::App { check: false }),
            Cmd::CheckUpdates => show_action(&app, Action::App { check: true }),
            Cmd::Quit => quit(&app),
        }
    });
}

/// « Quitter KyberNow Link » : les sessions en cours se terminent d'abord
/// (Admin de chaque ordinateur prévenue, écrans rendus), puis l'app se ferme
/// à la fin de la dernière.
pub fn quit(app: &AppHandle) {
    let state = app.state::<UiState>();
    // D'abord : une connexion qui se lance voit qu'il faut s'arrêter. Et
    // quitter finit toujours : `QUIT_WITHIN` au plus, avant tout verrou.
    if !state.quitting.swap(true, Ordering::SeqCst) {
        quit_deadline(app);
    }
    // Une demande en attente de l'accord d'un hôte (par le relais) est retirée.
    share::cancel_wait(&state, None);
    lend::cancel_wait(&state, None);
    {
        let sessions = state.sessions.locked();
        if !sessions.is_empty() {
            log::info!("quitter : fin des {} session(s) d'abord", sessions.len());
        }
        for r in sessions.iter() {
            log::info!("[{}] Quitter : ordre de fin à sa session (n°{})", r.name, r.host.id());
            r.host.command(Command::Disconnect);
        }
    }
    playstation::stop_all(app);
    // Rien en cours : tout de suite. Sinon à la fin de la dernière session,
    // ou de la connexion qui se lance (`ended`, `Launching`), `QUIT_WITHIN` au plus.
    exit_when_idle(app);
}

/// « Quitter » finit toujours : si les sessions n'ont pas fini dans
/// `QUIT_WITHIN` (un ordinateur qui ne répond pas, une session bloquée),
/// l'app se ferme quand même ; si elle n'y arrive pas non plus (fil principal
/// pris), le processus s'arrête 2 s plus tard. Une Station sans nouvelles
/// rend d'elle-même ses écrans, faute de signal de vie.
fn quit_deadline(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new().name("kybernow-quit".into()).spawn(move || {
        std::thread::sleep(QUIT_WITHIN);
        let state = app.state::<UiState>();
        let left = state.sessions.try_lock().map(|s| s.len().to_string()).unwrap_or_else(|_| "?".into());
        log::warn!("quitter : {left} session(s) pas finie(s) en {} s, l'app se ferme quand même", QUIT_WITHIN.as_secs());
        tray::remove();
        app.exit(0);
        std::thread::sleep(Duration::from_secs(2));
        log::warn!("quitter : l'app ne s'est pas fermée, arrêt du processus");
        log::logger().flush();
        std::process::exit(0);
    });
    if let Err(e) = spawned {
        log::error!("quitter : fil du délai refusé ({e}), arrêt du processus");
        std::process::exit(0);
    }
}

/// Juste après l'inscription d'une session (`connect`, partage, prêt) :
/// « Quitter » ou « Annuler » (`cancelled`) arrivé pendant son lancement
/// l'arrête tout de suite.
pub(crate) fn after_launch(app: &AppHandle, session: u64, cancelled: bool) {
    let state = app.state::<UiState>();
    if state.quitting.load(Ordering::SeqCst) {
        end_instance(app, session, "Quitter pendant le lancement");
    } else if cancelled {
        end_instance(app, session, "« Annuler » pendant le lancement");
    }
}

/// La première fermeture de l'accueil : une notification dit où l'app
/// continue, une seule fois.
fn first_close(app: &AppHandle) {
    let state = app.state::<UiState>();
    let first = update_prefs(&state, |p| !std::mem::replace(&mut p.close_hint, true));
    if !first {
        return;
    }
    shell::notify(&t("notify.close_title", &[]), &t("notify.close_body", &[]), None);
}

/// « Ouvrir à l'ouverture de session » : coché une fois, au premier lancement
/// de la version qui l'apporte ; ensuite le choix de l'utilisateur (et de
/// Windows, voir `login.rs`) est gardé.
fn open_at_login_default(state: &UiState) {
    let first = {
        let mut p = state.prefs.locked();
        let first = p.open_at_login.is_none();
        if first {
            p.open_at_login = Some(true);
            write_prefs(&p);
        }
        first
    };
    if first {
        login::set(true);
    } else {
        login::refresh();
    }
}

pub fn run(args: Vec<String>) {
    // Ouverte à l'ouverture de session : sans fenêtre, dans l'icône seule.
    let background = args.iter().any(|a| a == login::BACKGROUND);
    // Bouton « Mettre à jour » d'une notification, l'app fermée : la mise à
    // jour de la Station suit le lancement.
    let update_asked = match args.first().and_then(|a| crate::link::action(a)) {
        Some(("station-update", Some(id))) => Some(id),
        _ => None,
    };
    let state = UiState::default();
    *state.action.locked() = parse_action(&args);
    let mut prefs = load_prefs();
    // Premier lancement d'une nouvelle version : l'avis des Stations à mettre à jour.
    let app_updated = stations::launched(&mut prefs);
    // Micro : celui choisi et le traitement voix, pour toutes les sessions.
    crate::micro::set_prefs(prefs.microphone.clone(), prefs.voice_processing.unwrap_or(true));
    *state.prefs.locked() = prefs;
    let commands: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> = Box::new(tauri::generate_handler![
            snapshot,
            camera::session_camera,
            camera::session_camera_pick,
            camera::session_camera_state,
            camera::open_camera_settings,
            probe_address,
            parse_link,
            add_computer,
            pair_start,
            pair_poll,
            pair_cancel,
            save_settings,
            rename_computer,
            remove_computer,
            connect,
            show_session,
            session_command,
            session_mouse,
            session_keep_sound,
            bar_hide,
            bar_menu,
            bar_width,
            bar_computers,
            connexion_state,
            connexion_close,
            session_profile,
            take_action,
            app_info,
            check_update,
            set_channel,
            set_language,
            set_battery,
            micro::session_microphone,
            micro::mic_settings,
            micro::set_mic_device,
            micro::set_mic_voice,
            synced::set_sync_settings,
            autotest::autotest_run,
            autotest::autotest_cancel,
            autotest::autotest_apply,
            autotest::autotest_later,
            install_update,
            poll_now,
            station_restart,
            station_update,
            stations::station_later,
            stations::take_station_notice,
            open_admin,
            wake,
            set_profile,
            set_open_at_login,
            displays_info,
            common_values,
            share_log,
            session_hdr,
            session_hevc,
            computer_wired,
            fond_state,
            share::share_defaults,
            share::share_link_code,
            share::share_join,
            share::share_wait,
            share::share_cancel,
            share::share_connect,
            share::share_open,
            share::share_code,
            share::share_speak,
            share::share_accept,
            share::share_refuse,
            share::share_guest,
            share::share_quality,
            share::share_watch,
            share::share_watch_words,
            share::share_watch_screen,
            share::share_watch_display,
            share::share_kick,
            recording::session_record,
            recording::recording_clock,
            share::share_close,
            share::copy_text,
            lend::lend_friends,
            lend::lend_invite,
            lend::lend_add,
            lend::lend_remove,
            lend::lend_borrow,
            lend::lend_wait,
            lend::lend_cancel,
            lend::lend_come_back,
            lend::lend_connect,
            lend::lend_give_back,
            lend::lend_state,
            lend::lend_code,
            lend::lend_cancel_code,
            lend::lend_answer,
            lend::lend_take_back,
            invite::invite_redeem,
            account::account_list,
            account::account_sign_in,
            account::account_sign_in_cancel,
            account::account_manage,
            account::account_sign_out,
            welcome::welcome_state,
            welcome::welcome_done,
            welcome::welcome_close_card,
            welcome::welcome_paired_hint,
            welcome::welcome_away_hint,
            welcome::welcome_sign_up,
            welcome::welcome_station_download,
            playstation::ps_state,
            playstation::ps_search_now,
            playstation::ps_login,
            playstation::ps_account_manual,
            playstation::ps_account_rename,
            playstation::ps_account_remove,
            playstation::ps_choose_account,
            playstation::ps_set_hdr,
            home_hdr,
            playstation::ps_pair,
            playstation::ps_probe_address,
            playstation::ps_settings,
            playstation::ps_save_settings,
            playstation::ps_save_layout,
            playstation::ps_default_layout,
            playstation::ps_remove,
            playstation::ps_play,
            playstation::ps_handoff_discover,
            playstation::ps_handoff_send,
            playstation::ps_handoff_receive
    ]);
    let app = tauri::Builder::default()
        .manage(state)
        .register_uri_scheme_protocol(IMAGE_SCHEME, |ctx, request| serve_image(ctx.app_handle(), request.uri().path()))
        // Les commandes de l'interface : une panique d'une commande qui tourne
        // sur le fil de l'interface (les commandes synchrones, qui appellent
        // souvent le cœur) ne traverse pas WebView2, qui fermerait l'app ; elle
        // est rattrapée et notée (`ffi::guard` du cœur). Les commandes
        // `async` tournent dans leurs propres tâches.
        .invoke_handler(move |invoke| {
            let name = invoke.message.command().to_string();
            kybernow_core::ffi::guard(&name, true, || commands(invoke))
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            // Langue connue de l'interface dès son premier affichage (ensuite : l'événement `lang`).
            let lang_script = format!("window.__KN_LANG = '{}';", i18n::lang().code());
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .initialization_script(&lang_script)
                .title(shell::DISPLAY_NAME)
                .inner_size(1280.0, 800.0)
                .min_inner_size(960.0, 600.0)
                .decorations(false)
                .center()
                .visible(!background)
                .build()?;
            // « Cet écran » : celui de la fenêtre de l'accueil (docs/multi-ecrans.md).
            if let Some(home) = app.get_webview_window("main")
                && let Ok(hwnd) = home.hwnd()
            {
                system::set_home_window(hwnd.0 as isize);
            }
            WebviewWindowBuilder::new(app, "bar", WebviewUrl::App("bar.html".into()))
                .initialization_script(&lang_script)
                .title(t("app.bar_title", &[]))
                .inner_size(BAR_WINDOW_WIDTH, 76.0)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .focused(false)
                .visible(false)
                .build()?;

            log::info!("app : fenêtres créées{}", if background { " (ouverture de session : accueil caché)" } else { "" });
            let _ = handle.state::<UiState>().fonds.set(Fonds::new(handle.clone()));
            // La session active ou « Garder le son » change (plusieurs
            // ordinateurs) : le menu de l'icône suit (son haut est pour la
            // session active).
            focus::on_change(tray::refresh);
            shell::register();
            open_at_login_default(&handle.state::<UiState>());
            tray::start(handle.clone());
            if let Some(id) = update_asked.clone() {
                tray_command(&handle, Cmd::Update(id));
            }
            let h = handle.clone();
            instance::set_sink(move |args| handle_args(&h, args));
            let views = handle.state::<UiState>().views();
            shell::update_jump_list(views.iter().map(|v| (v.computer.id.clone(), v.computer.name.clone())).collect());
            // Mises à jour : 15 s après le lancement, puis toutes les 6 heures.
            let h = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(15));
                loop {
                    if let Err(e) = check_now(&h) {
                        log::warn!("mises à jour : {e}");
                    }
                    std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
                }
            });
            let h = handle.clone();
            std::thread::spawn(move || {
                let pc = pc::this_pc();
                *h.state::<UiState>().this_pc.locked() = Some(pc.clone());
                let _ = h.emit_to("main", "this-pc", pc);
            });
            let h = handle.clone();
            let d = discovery::start(move |_| emit_discovered(&h));
            let _ = handle.state::<UiState>().discovery.set(d);
            let h = handle.clone();
            std::thread::spawn(move || poll(h));
            // Comptes KyberNow : leurs ordinateurs relevés au lancement.
            account::sync_soon(&handle, true);
            // Les ordinateurs empruntés (« Ordinateur de {hôte} ») : leur état.
            let h = handle.clone();
            std::thread::spawn(move || lend::poll_friends(h));
            let h = handle.clone();
            std::thread::spawn(move || follow_loop(h));
            if app_updated {
                let h = handle.clone();
                std::thread::spawn(move || stations::notice_after_update(h));
            }
            // Consoles PlayStation : le fichier, les secrets, la recherche.
            playstation::start(&handle);
            log::info!("app : prête");
            Ok(())
        })
        .on_window_event(|window, event| {
            // « Cet écran » suit l'accueil d'un écran à l'autre.
            if matches!(event, tauri::WindowEvent::Moved(_) | tauri::WindowEvent::ScaleFactorChanged { .. }) {
                if window.label() == "main" {
                    home_moved(window.app_handle());
                }
                return;
            }
            // La fenêtre de connexion d'une session suivante passe devant :
            // sa session devient la session active (plusieurs ordinateurs).
            if let tauri::WindowEvent::Focused(true) = event {
                if window.label().starts_with("connexion-") {
                    connexion_focused(window.app_handle(), window.label());
                }
                // L'accueil revient : les ordinateurs des comptes sont relevés (30 s au plus souvent).
                if window.label() == "main" {
                    account::sync_soon(window.app_handle(), false);
                }
                return;
            }
            let tauri::WindowEvent::CloseRequested { api, .. } = event else { return };
            let app = window.app_handle();
            match window.label() {
                // Fermer l'accueil le cache : l'app continue dans l'icône près
                // de l'horloge (« Quitter KyberNow Link » la quitte). Une
                // session en cours continue.
                "main" => {
                    api.prevent_close();
                    let _ = window.hide();
                    first_close(app);
                }
                // Fenêtre de connexion d'une session suivante (plusieurs
                // ordinateurs) : la fermer, c'est « Annuler » (ou « Fermer » sous
                // un échec).
                label if label.starts_with("connexion-") => {
                    api.prevent_close();
                    connexion_closed(app, label);
                }
                // La connexion de Sony (consoles PlayStation) se ferme pour de bon.
                "psn-login" => {}
                // La barre ne se ferme pas (Alt+F4) : elle se cache, le
                // clavier revient à la session qu'elle servait.
                _ => {
                    api.prevent_close();
                    bar_hide(app.clone(), None, None);
                }
            }
        })
        .build(tauri::generate_context!());
    match app {
        Ok(app) => app.run(|_, event| {
            // L'icône part avec l'app (sinon elle reste jusqu'au passage de la souris).
            if let tauri::RunEvent::Exit = event {
                tray::remove();
            }
        }),
        Err(e) => system::error_box(&t("app.start_failed", &[("error", &e.to_string())])),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_link_text;

    #[test]
    fn lien_de_la_console() {
        let c = parse_link_text("kybernow://192.168.1.100:31544?name=Bureau%20Salon&fp=sha256:aa:bb").unwrap();
        assert_eq!((c.host.as_str(), c.port), ("192.168.1.100", 31544));
        assert_eq!(c.name.as_deref(), Some("Bureau Salon"));
        assert_eq!(c.fingerprint.as_deref(), Some("sha256:aa:bb"));
        let v6 = parse_link_text("kybernow://[fe80::1]:9443").unwrap();
        assert_eq!((v6.host.as_str(), v6.port), ("fe80::1", 9443));
    }
}
