//! Fenêtres de session Win32 : une fenêtre plein écran sans bordure par écran
//! de la session (docs/multi-ecrans.md), clavier et souris vers l'ordinateur,
//! pointeur de l'ordinateur, raccourcis de l'app (Ctrl+Alt+Maj + lettre,
//! comme ⌃⌥⌘ sur Mac).
//!
//! Les entrées partent par le module d'entrées du cœur
//! (`kybernow_core::input::InputPipeline`, une chaîne par démarrage de
//! l'image, voir `host.rs`) ; le pointeur de l'ordinateur et le mode de la
//! souris viennent des `PointerLayer` (une par écran) et de `MouseModeRules`.
//! Les fenêtres ne font que capter les événements de Windows et dessiner le
//! pointeur.
//!
//! | Ctrl+Alt+Maj+D | se déconnecter |
//! | Ctrl+Alt+Maj+F | plein écran ou fenêtre (un seul écran) |
//! | Ctrl+Alt+Maj+M | souris des jeux (relative) ou du bureau |
//! | Ctrl+Alt+Maj+O | ordinateur suivant (plusieurs ordinateurs) |
//!
//! Tout vit sur le fil de la session (voir `host.rs`) : l'accueil de l'app
//! garde le fil principal. Une fenêtre invisible (« de contrôle ») reçoit ce
//! qui concerne toute la session (état, ordres de l'app, écrans) ; chaque
//! fenêtre d'écran reste cachée jusqu'à la première image de son flux.
//!
//! Plusieurs ordinateurs à la fois (docs/plusieurs-ordinateurs.md) : une
//! `App` par session, chacune sur son fil, toutes dans un registre (`APPS`,
//! dans l'ordre d'ouverture) où chaque fenêtre retrouve la sienne. La session
//! active est tenue par le registre du cœur (`kybernow_core::focus`, par
//! `focus.rs`) : une fenêtre de session qui passe au premier plan l'y active,
//! et le cœur coupe les entrées et le son des autres. Le crochet clavier (un
//! seul pour l'app, sur son propre fil) et les déplacements bruts de la souris
//! (Raw Input, réenregistré à chaque activation) suivent la fenêtre active.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use kybernow_core::focus::FocusId;
use kybernow_core::i18n::Lang;
use kybernow_core::input::{Clock, InputPipeline, MouseMode, MouseModeRules, PointerLayer, ScreenTarget, Shape};
use kybernow_core::kynput_rs::MouseButtonType;
use kybernow_core::media::PacketHandler;
use kybernow_core::session::{Session, Status, StreamScreen};

use crate::displays::{Layout, Monitor};
use crate::focus;
use crate::gpu::Gpu;
use crate::host::{HostEvents, Role, ScreenSet, ScreenState};
use crate::i18n::t;
use crate::render::{Renderer, video_rect};
use crate::video::{self, Discard, LateFeed, Link, Shared, VideoHandler, VideoOptions};

/// Fenêtre de contrôle : l'état de la session a changé (titre, fin).
pub const WM_APP_STATUS: u32 = WM_APP + 1;
/// Fenêtre d'un écran : le pointeur de l'ordinateur a changé.
pub const WM_APP_CURSOR: u32 = WM_APP + 2;
/// Fenêtre de contrôle : le mode de la souris est à revoir (jeu en plein écran).
pub const WM_APP_MOUSE: u32 = WM_APP + 3;
/// Fenêtre d'un écran : première image présentée (de ce flux), la fenêtre se montre.
pub const WM_APP_FIRST_IMAGE: u32 = WM_APP + 4;
/// Ordre de l'app (barre de session) : `wParam` = `Command`.
pub const WM_APP_COMMAND: u32 = WM_APP + 5;
/// Fenêtre de contrôle : les écrans de la session ont changé (envoyé par le
/// fil du cœur, qui attend que les fenêtres soient prêtes).
const WM_APP_SCREENS: u32 = WM_APP + 6;
/// Fenêtre de contrôle : les écrans de ce PC ont changé (annoncé par une fenêtre d'écran).
const WM_APP_DISPLAYS: u32 = WM_APP + 7;

/// Minuterie (fenêtre de contrôle) du recontrôle du mode de la souris
/// (pointeur remontré en jeu).
const TIMER_MOUSE: usize = 1;
/// Minuterie d'une fenêtre d'écran : le pointeur reste contre le bord haut.
const TIMER_EDGE: usize = 2;
/// Minuterie (fenêtre de contrôle) : les écrans de ce PC changent par rafales
/// (un écran qu'on branche en change plusieurs fois), on relève une fois posé.
const TIMER_DISPLAYS: usize = 3;
const DISPLAYS_SETTLE_MS: u32 = 800;
/// Le pointeur reste ce temps contre le bord haut avant que la barre s'ouvre
/// (un passage rapide ne l'ouvre pas).
const EDGE_DWELL_MS: u32 = 250;
/// Boutons de la souris enfoncés dans le `wParam` d'un `WM_MOUSEMOVE`
/// (MK_LBUTTON, MK_RBUTTON, MK_MBUTTON, MK_XBUTTON1, MK_XBUTTON2).
const MK_BUTTONS: usize = 0x01 | 0x02 | 0x10 | 0x20 | 0x40;
/// « Contre le bord haut » : 2 points au plus, mis à l'échelle de l'écran.
const EDGE_SLACK_POINTS: f32 = 2.0;
/// Largeur de la barre de session en points, tant que son interface ne l'a
/// pas mesurée (sa largeur naturelle, à quelques points près).
const BAR_WIDTH_GUESS: u32 = 1080;
/// Largeur de la barre de session en points, telle que son interface la
/// mesure (elle change avec le texte des mesures et la langue).
static BAR_WIDTH: AtomicU32 = AtomicU32::new(BAR_WIDTH_GUESS);

/// La barre de session fait cette largeur (en points) : la zone du bord haut
/// où le pointeur l'ouvre est centrée de cette largeur.
pub fn set_bar_width(points: f64) {
    if points.is_finite() && points >= 1.0 {
        BAR_WIDTH.store(points.round() as u32, Ordering::Relaxed);
    }
}

/// La barre de session vient de se montrer pour la session `id` (une seule
/// barre : elle sert la session de la fenêtre où elle s'ouvre), ou de se
/// cacher (`None`) : le trait du bord haut de cette session s'efface pendant
/// qu'elle est là, et l'ouverture au pointeur attend.
pub fn set_bar_session(id: Option<u64>) {
    for a in apps() {
        a.bar_visible.store(Some(a.id) == id, Ordering::Relaxed);
        a.refresh_hints();
    }
}

/// Où se place la barre de session : le haut de la zone cliente de la session,
/// en pixels de l'écran (son milieu, son bord haut) et l'échelle de son
/// écran. Rien si la fenêtre n'existe pas ou est réduite.
pub fn bar_anchor(hwnd: isize) -> Option<(i32, i32, f32)> {
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        if hwnd.0.is_null() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client).ok()?;
        let mut tl = POINT { x: client.left, y: client.top };
        let mut br = POINT { x: client.right, y: client.bottom };
        let _ = ClientToScreen(hwnd, &mut tl);
        let _ = ClientToScreen(hwnd, &mut br);
        Some(((tl.x + br.x) / 2, tl.y, dpi_scale(hwnd)))
    }
}

/// Le cadre extérieur d'une fenêtre (pixels du bureau) et la taille de sa
/// zone cliente : là où la connexion d'une session en fenêtre se montre de
/// nouveau quand elle reprend. Rien si la fenêtre n'existe pas ou est réduite.
pub fn window_frame(hwnd: isize) -> Option<((i32, i32), (u32, u32))> {
    let hwnd = HWND(hwnd as *mut _);
    unsafe {
        if hwnd.0.is_null() || IsIconic(hwnd).as_bool() {
            return None;
        }
        let (mut outer, mut client) = (RECT::default(), RECT::default());
        GetWindowRect(hwnd, &mut outer).ok()?;
        GetClientRect(hwnd, &mut client).ok()?;
        Some(((outer.left, outer.top), ((client.right - client.left).max(0) as u32, (client.bottom - client.top).max(0) as u32)))
    }
}

/// Échelle de l'écran de la fenêtre (1,5 : 150 %) ; 1 sans fenêtre.
fn dpi_scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// Auto : un jeu en plein écran qui cache son pointeur (comme sur Mac).
const MOUSE_RULES: MouseModeRules = MouseModeRules { needs_hidden_pointer: true };

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Command {
    Disconnect = 1,
    Fullscreen = 2,
    MouseToggle = 3,
    Focus = 4,
    // Mode de la souris choisi dans le menu de l'icône.
    MouseAuto = 5,
    MouseDesktop = 6,
    MouseGame = 7,
    // Menu Touches de la barre : combinaisons que ce PC garderait pour lui.
    CtrlAltDel = 10,
    WinKey = 11,
    AltTab = 12,
    PrintScreen = 13,
}

/// Codes de touche (jeu 1, `0xE000` : touche étendue).
mod sc {
    pub const CTRL: u16 = 0x1D;
    pub const ALT: u16 = 0x38;
    pub const TAB: u16 = 0x0F;
    pub const DEL: u16 = 0xE053;
    pub const LWIN: u16 = 0xE05B;
    pub const PRINT: u16 = 0xE037;
}

/// Envoie une combinaison : touches enfoncées dans l'ordre, relâchées à l'envers.
fn send_combo(app: &App, keys: &[u16]) {
    for &k in keys {
        send_key(app, k, true);
    }
    for &k in keys.iter().rev() {
        send_key(app, k, false);
    }
}

/// Touche vers l'ordinateur. Le relâchement d'une touche dont l'appui n'est
/// pas parti (raccourci de l'app, touche tenue avant la session ou avant une
/// relance de l'image) ne part pas.
fn send_key(app: &App, scancode: u16, pressed: bool) {
    let Some(input) = app.input() else { return };
    if !pressed && !input.held().lock().unwrap().is_held(scancode) {
        return;
    }
    input.key(scancode, pressed);
}

/// La touche est-elle tenue sur l'ordinateur ?
fn key_held(app: &App, scancode: u16) -> bool {
    app.input().is_some_and(|i| i.held().lock().unwrap().is_held(scancode))
}

/// Le fil du crochet clavier : combien de sessions le demandent, et le fil
/// en cours (son identifiant, et s'il doit s'arrêter).
struct HookState {
    users: usize,
    thread: Option<Arc<HookThread>>,
}

struct HookThread {
    id: AtomicU32,
    quit: AtomicBool,
    generation: u64,
}

static HOOK: Mutex<HookState> = Mutex::new(HookState { users: 0, thread: None });
/// Génération du crochet en place : un crochet qui s'arrête (dernière session
/// finie, puis une nouvelle aussitôt) laisse passer les touches, jamais deux
/// crochets ne prennent la même.
static HOOK_GENERATION: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// La génération du crochet de ce fil.
    static MY_HOOK: Cell<u64> = const { Cell::new(0) };
}

/// Crochet clavier de bas niveau : en plein écran et au premier plan, les
/// raccourcis que Windows garderait pour ce PC partent à l'ordinateur (touche
/// ⊞ et ses combinaisons, Alt+Tab, Alt+Échap, Ctrl+Échap, touche Menu).
/// Ctrl+Alt+Suppr reste à Windows : menu Touches de la barre. Un seul crochet
/// pour toutes les sessions et tous leurs écrans, sur son propre fil (posé à
/// la première session, retiré à la fin de la dernière) : il regarde la
/// fenêtre qui a le clavier, donc la session active. Chaque session demande
/// le crochet au début et le rend à la fin.
pub fn keyboard_hook_acquire() {
    let mut h = HOOK.lock().unwrap();
    h.users += 1;
    if h.users > 1 {
        return;
    }
    let generation = HOOK_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let thread = Arc::new(HookThread { id: AtomicU32::new(0), quit: AtomicBool::new(false), generation });
    h.thread = Some(thread.clone());
    let spawned = std::thread::Builder::new().name("kybernow-clavier".into()).spawn(move || hook_thread(thread));
    if let Err(e) = spawned {
        log::warn!("raccourcis de Windows : fil du crochet refusé ({e}), ils restent à ce PC");
    }
}

pub fn keyboard_hook_release() {
    let mut h = HOOK.lock().unwrap();
    h.users = h.users.saturating_sub(1);
    if h.users > 0 {
        return;
    }
    HOOK_GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Some(t) = h.thread.take() {
        t.quit.store(true, Ordering::SeqCst);
        let id = t.id.load(Ordering::SeqCst);
        if id != 0 {
            unsafe {
                let _ = PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }
}

fn hook_thread(me: Arc<HookThread>) {
    MY_HOOK.with(|g| g.set(me.generation));
    unsafe {
        // La file de messages du fil existe avant qu'on lui poste WM_QUIT.
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        me.id.store(windows::Win32::System::Threading::GetCurrentThreadId(), Ordering::SeqCst);
        if me.quit.load(Ordering::SeqCst) {
            return;
        }
        let hook = GetModuleHandleW(None).ok().and_then(|m| SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard), Some(m.into()), 0).ok());
        if hook.is_none() {
            log::warn!("raccourcis de Windows : crochet clavier refusé, ils restent à ce PC");
        }
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if let Some(h) = hook {
            let _ = UnhookWindowsHookEx(h);
        }
    }
}

unsafe extern "system" fn ll_keyboard(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let mine = MY_HOOK.with(|g| g.get()) == HOOK_GENERATION.load(Ordering::Relaxed);
        if code == HC_ACTION as i32 && mine {
            let k = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            // Touches simulées (par une autre app, ou par Windows) : pas les nôtres.
            if (k.flags.0 & LLKHF_INJECTED.0) == 0 && grab_key(k, wparam.0 as u32) {
                return LRESULT(1);
            }
        }
        CallNextHookEx(None, code, wparam, lparam)
    }
}

/// Vrai : la touche part à l'ordinateur et Windows ne la voit pas.
fn grab_key(k: &KBDLLHOOKSTRUCT, msg: u32) -> bool {
    let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
    let sc = (k.scanCode & 0xFF) as u16;
    let code = if (k.flags.0 & LLKHF_EXTENDED.0) != 0 { 0xE000 | sc } else { sc };
    if !down {
        // Le relâchement va à la session qui a pris l'enfoncement, active ou non.
        for app in apps() {
            let mut grabbed = app.grabbed.lock().unwrap();
            if let Some(i) = grabbed.iter().position(|&v| v == k.vkCode) {
                grabbed.remove(i);
                drop(grabbed);
                send_key(&app, code, false);
                return true;
            }
        }
        return false;
    }
    // Répétition d'une touche déjà prise : prise aussi, même si la session a
    // perdu le premier plan entre-temps (sinon Windows verrait l'enfoncement
    // sans le relâchement, et la touche resterait tenue sur ce PC).
    if apps().iter().any(|a| a.grabbed.lock().unwrap().contains(&k.vkCode)) {
        return true;
    }
    // La session active, en plein écran au premier plan. Sans verrou d'une
    // fenêtre ici : `set_fullscreen` tient le sien pendant SetWindowPos.
    let Some((app, win)) = foreground_app() else { return false };
    if !win.fullscreen_now.load(Ordering::Relaxed) {
        return false;
    }
    let alt = (k.flags.0 & LLKHF_ALTDOWN.0) != 0;
    let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } < 0;
    let vk = VIRTUAL_KEY(k.vkCode as u16);
    let grab = match vk {
        VK_LWIN | VK_RWIN | VK_APPS => true,
        VK_TAB => alt,
        VK_ESCAPE => alt || ctrl,
        _ => false,
    };
    if !grab {
        return false;
    }
    {
        let mut grabbed = app.grabbed.lock().unwrap();
        if !grabbed.contains(&k.vkCode) {
            grabbed.push(k.vkCode);
        }
    }
    // Les répétitions de Windows ne partent pas (l'ordinateur répète lui-même).
    if !key_held(&app, code) {
        send_key(&app, code, true);
    }
    true
}

/// Pointeur de Windows fait de la forme annoncée par l'ordinateur.
#[derive(Default)]
struct CursorState {
    current: isize,
    /// Pixels de la forme en place et l'échelle à laquelle il a été construit :
    /// les annonces de position (jusqu'à 120 par seconde) ne le refont pas.
    built: Option<(Arc<[u32]>, f32)>,
}

/// Une fenêtre de session : un écran de ce PC.
pub struct ScreenWin {
    pub hwnd: isize,
    /// L'écran de ce PC auquel la fenêtre est destinée (nom du périphérique).
    pub key: String,
    /// Son index dans la liste d'écrans donnée au cœur.
    index: AtomicUsize,
    pub shared: Arc<Shared>,
    pub video: Arc<VideoHandler>,
    /// Pointeur de l'ordinateur sur cet écran.
    pub pointer: Arc<PointerLayer>,
    /// Écran de l'ordinateur affiché (`Display::id`) et sa taille en pixels.
    display_id: AtomicU32,
    host_size: Mutex<(u32, u32)>,
    /// Écran principal de la session.
    primary: AtomicBool,
    cursor: Mutex<CursorState>,
    saved_placement: Mutex<Option<(WINDOWPLACEMENT, isize)>>,
    /// Plein écran en ce moment (lu par le crochet clavier, sans verrou).
    fullscreen_now: AtomicBool,
    shown: AtomicBool,
    /// La fenêtre est la fenêtre active (premier plan).
    active: AtomicBool,
    /// Le pointeur est dans la zone du bord haut et la minuterie court.
    edge_armed: AtomicBool,
}

impl ScreenWin {
    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut _)
    }

    pub fn index(&self) -> usize {
        self.index.load(Ordering::Relaxed)
    }

    fn host_size(&self) -> (u32, u32) {
        *self.host_size.lock().unwrap()
    }

    pub fn is_primary(&self) -> bool {
        self.primary.load(Ordering::Relaxed)
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen_now.load(Ordering::Relaxed)
    }
}

/// Les écrans de la session, dits par le cœur (`Platform::on_screens`).
struct ScreenUpdate {
    streams: Vec<StreamScreen>,
    not_created: Vec<usize>,
}

/// Réglages de l'image pour les fenêtres créées en cours de session.
pub struct VideoSettings {
    pub hardware: bool,
    pub vsync: bool,
    /// Format demandé au départ (`media::FOURCC_*`).
    pub codec: u32,
}

pub struct App {
    /// Identifiant de la session dans l'app (`APPS`, barre de session).
    pub id: u64,
    /// Son inscription au registre des sessions du cœur.
    focus: OnceLock<FocusId>,
    /// Fenêtre de contrôle (invisible).
    control: AtomicIsize,
    pub host_name: String,
    /// Chaîne d'entrées de l'image en cours (cœur) ; aucune entre deux images.
    input: Mutex<Option<Arc<InputPipeline>>>,
    pub clock: Arc<dyn Clock>,
    /// Souris relative (jeux), appliquée à chaque nouvelle chaîne d'entrées.
    relative: AtomicBool,
    /// Ce que les écrans partagent : session du cœur, latence, format.
    pub link: Arc<Link>,
    pub session: OnceLock<Arc<Session>>,
    /// Écran principal de la session pour les entrées : id de l'écran de
    /// l'ordinateur et sa taille (`Platform::on_display`).
    primary_display: Mutex<(u32, (u32, u32))>,
    pub status: Mutex<Option<Status>>,
    pub latency: Mutex<Option<f64>>,
    pub fps: Mutex<u64>,
    /// Un jeu en plein écran sur l'ordinateur (console).
    pub game: AtomicBool,
    /// Auto, ou forcé par Ctrl+Alt+Maj+M et la barre (Jeu, Bureau).
    pub mouse_mode: Mutex<MouseMode>,
    pub closing: AtomicBool,
    pub error: Mutex<Option<String>>,
    pub events: Arc<dyn HostEvents>,
    /// Plein écran quand la fenêtre se montre (une session à un écran ;
    /// plusieurs écrans : toujours).
    fullscreen: bool,
    /// Mesures dans le titre (ligne de commande) ; sinon le nom seul.
    title_stats: bool,
    /// La barre de session est affichée pour cette session (une seule barre).
    bar_visible: AtomicBool,
    /// Dernière fenêtre active (le clavier y revient quand la barre se cache).
    last_active: AtomicIsize,
    /// Touches dont l'enfoncement a été pris par le crochet pour cette
    /// session (leur relâchement suit, même si elle n'est plus active).
    grabbed: Mutex<Vec<u32>>,
    /// Fin demandée par l'utilisateur (« Se déconnecter », fermeture d'une
    /// fenêtre, Ctrl+Alt+Maj+D) : pas de notification de fin.
    user_ended: AtomicBool,
    /// C'était la session active quand elle s'est arrêtée (sinon, la
    /// notification de fin vient même app au premier plan).
    was_active: AtomicBool,
    /// La fenêtre de l'écran principal s'est montrée (première image).
    ever_shown: AtomicBool,
    /// Une de ses fenêtres était au premier plan quand elle s'est arrêtée :
    /// la session active qui reste reprend la place.
    was_front: AtomicBool,
    windows: Mutex<Vec<Arc<ScreenWin>>>,
    /// Les écrans de ce PC tels que le cœur les a reçus.
    layout: Mutex<Layout>,
    gpu: Arc<Gpu>,
    video: VideoSettings,
    /// Nombre de flux de la session.
    streams: AtomicUsize,
    /// Les écrans dits par le cœur, pas encore pris par le fil des fenêtres.
    pending: Mutex<Option<ScreenUpdate>>,
    /// Écrans dont le cœur a demandé le gestionnaire avant que leur fenêtre
    /// existe (fil des fenêtres en retard) : leur fenêtre les prend en arrivant.
    late: Mutex<Vec<(String, Arc<LateFeed>)>>,
    /// Écrans dont la fenêtre n'a pas pu se créer (dernier `apply_screens`) :
    /// traités comme des écrans que l'ordinateur n'a pas pu créer.
    failed: Mutex<Vec<String>>,
    /// Dernière image de l'écran principal, gardée à la fermeture de sa fenêtre.
    last_image: Mutex<Option<Vec<u8>>>,
}

/// Les sessions en cours, dans l'ordre d'ouverture (plusieurs ordinateurs à
/// la fois) : chaque fenêtre de session et chaque fenêtre de contrôle
/// appartient à l'une d'elles.
static APPS: Mutex<Vec<Arc<App>>> = Mutex::new(Vec::new());

/// Fenêtre qui reçoit les déplacements bruts de la souris (Raw Input) : un
/// seul enregistrement pour tout le processus, celui de la session active.
static RAW_TARGET: AtomicIsize = AtomicIsize::new(0);

fn apps() -> Vec<Arc<App>> {
    APPS.lock().unwrap().clone()
}

/// La session et la fenêtre d'écran de cette fenêtre.
fn app_of(hwnd: isize) -> Option<(Arc<App>, Arc<ScreenWin>)> {
    APPS.lock().unwrap().iter().find_map(|a| a.window(hwnd).map(|w| (a.clone(), w)))
}

/// La session de cette fenêtre de contrôle.
fn app_of_control(hwnd: isize) -> Option<Arc<App>> {
    APPS.lock().unwrap().iter().find(|a| a.control.load(Ordering::Relaxed) == hwnd).cloned()
}

/// La session dont une fenêtre a le clavier (le premier plan de Windows), et cette fenêtre.
fn foreground_app() -> Option<(Arc<App>, Arc<ScreenWin>)> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.0.is_null() { None } else { app_of(fg.0 as isize) }
}

/// Une session a une fenêtre au premier plan (la souris des jeux peut y être retenue).
fn any_in_front() -> bool {
    foreground_app().is_some()
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: u64,
        host_name: String,
        clock: Arc<dyn Clock>,
        link: Arc<Link>,
        events: Arc<dyn HostEvents>,
        gpu: Arc<Gpu>,
        layout: Layout,
        video: VideoSettings,
        fullscreen: bool,
        title_stats: bool,
    ) -> Arc<Self> {
        let app = Arc::new(Self {
            id,
            focus: OnceLock::new(),
            control: AtomicIsize::new(0),
            host_name,
            input: Mutex::new(None),
            clock,
            relative: AtomicBool::new(false),
            link,
            session: OnceLock::new(),
            primary_display: Mutex::new((0, (1920, 1080))),
            status: Mutex::new(None),
            latency: Mutex::new(None),
            fps: Mutex::new(0),
            game: AtomicBool::new(false),
            mouse_mode: Mutex::new(MouseMode::Auto),
            closing: AtomicBool::new(false),
            error: Mutex::new(None),
            events,
            fullscreen,
            title_stats,
            bar_visible: AtomicBool::new(false),
            last_active: AtomicIsize::new(0),
            grabbed: Mutex::new(Vec::new()),
            user_ended: AtomicBool::new(false),
            was_active: AtomicBool::new(false),
            ever_shown: AtomicBool::new(false),
            was_front: AtomicBool::new(false),
            windows: Mutex::new(Vec::new()),
            layout: Mutex::new(layout),
            gpu,
            video,
            streams: AtomicUsize::new(0),
            pending: Mutex::new(None),
            late: Mutex::new(Vec::new()),
            failed: Mutex::new(Vec::new()),
            last_image: Mutex::new(None),
        });
        APPS.lock().unwrap().push(app.clone());
        app
    }

    /// Fin de la session : ses fenêtres n'ont plus d'état, elle quitte le
    /// registre (les autres sessions continuent).
    pub fn unregister(&self) {
        APPS.lock().unwrap().retain(|a| a.id != self.id);
        self.forget_raw_target();
    }

    /// Ses fenêtres partent : les déplacements bruts n'y vont plus (la
    /// prochaine fenêtre activée les reprend).
    fn forget_raw_target(&self) {
        if self.window(RAW_TARGET.load(Ordering::Relaxed)).is_some() {
            RAW_TARGET.store(0, Ordering::Relaxed);
        }
    }

    /// Son inscription au registre des sessions du cœur (une fois la session
    /// du cœur démarrée).
    pub fn set_focus_id(&self, id: FocusId) {
        let _ = self.focus.set(id);
    }

    pub fn focus_id(&self) -> Option<FocusId> {
        self.focus.get().copied()
    }

    /// Fin demandée par l'utilisateur.
    pub fn user_ended(&self) -> bool {
        self.user_ended.load(Ordering::Relaxed)
    }

    pub fn was_active(&self) -> bool {
        self.was_active.load(Ordering::Relaxed)
    }

    /// Sa fenêtre principale s'est montrée au moins une fois.
    pub fn ever_shown(&self) -> bool {
        self.ever_shown.load(Ordering::Relaxed)
    }

    /// Une de ses fenêtres était au premier plan à sa fin.
    pub fn was_front(&self) -> bool {
        self.was_front.load(Ordering::Relaxed)
    }

    /// Un ordre à la fenêtre de contrôle de cette session (tout fil).
    pub fn post_command(&self, c: Command, arg: isize) {
        unsafe {
            let _ = PostMessageW(Some(self.control()), WM_APP_COMMAND, WPARAM(c as usize), LPARAM(arg));
        }
    }

    /// La fenêtre de session où la barre s'ouvre quand l'app la demande sans
    /// geste (« Statistiques » du menu de l'icône) : celle de cette session où
    /// est le pointeur, sinon sa dernière active, sinon son écran principal.
    pub fn bar_window(&self) -> isize {
        let mut p = POINT::default();
        if unsafe { GetCursorPos(&mut p) }.is_ok() {
            if let Some(w) = self.window_at(p) {
                return w.hwnd;
            }
        }
        let last = self.last_active.load(Ordering::Relaxed);
        if last != 0 && self.window(last).is_some() {
            return last;
        }
        self.primary_window().map_or(0, |w| w.hwnd)
    }

    /// Format demandé après un changement de profil (décodeur de secours).
    pub fn set_codec(&self, fourcc: u32) {
        self.link.codec.store(fourcc, Ordering::Relaxed);
    }

    /// La session est en plein écran (menu de l'icône : « Plein écran » coché).
    pub fn fullscreen_now(&self) -> bool {
        self.primary_window().is_some_and(|w| w.fullscreen_now.load(Ordering::Relaxed))
    }

    /// Écrans de la session (flux en cours) ; avec plusieurs, pas de « Plein
    /// écran » : les fenêtres y restent.
    pub fn stream_count(&self) -> usize {
        self.streams.load(Ordering::Relaxed)
    }

    /// La session du cœur (changement de profil).
    pub fn current_session(&self) -> Option<Arc<Session>> {
        self.session.get().cloned()
    }

    pub fn set_mouse_mode(&self, mode: MouseMode) {
        *self.mouse_mode.lock().unwrap() = mode;
    }

    fn control(&self) -> HWND {
        HWND(self.control.load(Ordering::Relaxed) as *mut _)
    }

    pub fn control_hwnd(&self) -> isize {
        self.control.load(Ordering::Relaxed)
    }

    /// Un message à la fenêtre de contrôle (tout fil).
    pub fn post(&self, msg: u32) {
        unsafe {
            let _ = PostMessageW(Some(self.control()), msg, WPARAM(0), LPARAM(0));
        }
    }

    pub fn windows(&self) -> Vec<Arc<ScreenWin>> {
        self.windows.lock().unwrap().clone()
    }

    fn window(&self, hwnd: isize) -> Option<Arc<ScreenWin>> {
        self.windows.lock().unwrap().iter().find(|w| w.hwnd == hwnd).cloned()
    }

    fn window_by_key(&self, key: &str) -> Option<Arc<ScreenWin>> {
        self.windows.lock().unwrap().iter().find(|w| w.key == key).cloned()
    }

    /// La fenêtre de l'écran principal de la session (la première à défaut).
    pub fn primary_window(&self) -> Option<Arc<ScreenWin>> {
        let windows = self.windows.lock().unwrap();
        windows.iter().find(|w| w.is_primary()).or_else(|| windows.first()).cloned()
    }

    /// La fenêtre de session qui a le clavier (le premier plan de Windows).
    fn foreground(&self) -> Option<Arc<ScreenWin>> {
        let fg = unsafe { GetForegroundWindow() };
        if fg.0.is_null() { None } else { self.window(fg.0 as isize) }
    }

    /// L'app est au premier plan : une de ses fenêtres de session a le clavier.
    fn in_front(&self) -> bool {
        self.foreground().is_some()
    }

    /// La fenêtre montrée qui contient ce point de l'écran.
    fn window_at(&self, p: POINT) -> Option<Arc<ScreenWin>> {
        self.windows
            .lock()
            .unwrap()
            .iter()
            .find(|w| unsafe {
                let mut r = RECT::default();
                IsWindowVisible(w.hwnd()).as_bool() && GetWindowRect(w.hwnd(), &mut r).is_ok() && p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
            })
            .cloned()
    }

    pub fn shown(&self) -> bool {
        self.windows.lock().unwrap().iter().any(|w| w.shown.load(Ordering::Relaxed))
    }

    /// La barre de session peut s'ouvrir au pointeur : une session montrée
    /// (la ligne de commande n'a pas de barre), la souris du bureau (en souris
    /// des jeux, le pointeur est capturé), la barre pas déjà là.
    fn edge_ready(&self) -> bool {
        !self.title_stats && self.shown() && !self.relative() && !self.bar_visible.load(Ordering::Relaxed)
    }

    /// Le trait du bord haut se montre, sur chaque écran montré, quand le
    /// geste est possible et que l'app est au premier plan ; le fil de l'image
    /// de chaque écran le trace à sa prochaine image (ou dans les 100 ms si
    /// l'écran est fixe).
    fn refresh_hints(&self) {
        let ready = self.edge_ready() && self.windows().iter().any(|w| w.active.load(Ordering::Relaxed));
        for w in self.windows() {
            let on = ready && w.shown.load(Ordering::Relaxed);
            let scale = if on { (dpi_scale(w.hwnd()) * 100.0).round() as u32 } else { 0 };
            w.shared.edge_hint.store(scale, Ordering::Relaxed);
        }
    }

    /// Chaîne d'entrées de l'image en cours.
    pub fn input(&self) -> Option<Arc<InputPipeline>> {
        self.input.lock().unwrap().clone()
    }

    /// Nouvelle chaîne d'entrées (début ou relance de l'image), au mode de
    /// souris en cours ; rend la précédente, à arrêter.
    pub fn attach_input(&self, input: Arc<InputPipeline>) -> Option<Arc<InputPipeline>> {
        let mut current = self.input.lock().unwrap();
        input.set_relative(self.relative());
        current.replace(input)
    }

    /// Image arrêtée : plus de chaîne d'entrées ; rend celle qui était là, à arrêter.
    pub fn detach_input(&self) -> Option<Arc<InputPipeline>> {
        self.input.lock().unwrap().take()
    }

    /// Souris relative (jeux) en ce moment.
    pub fn relative(&self) -> bool {
        self.relative.load(Ordering::Relaxed)
    }

    fn set_relative(&self, on: bool) {
        let current = self.input.lock().unwrap();
        self.relative.store(on, Ordering::Relaxed);
        if let Some(input) = current.as_ref() {
            input.set_relative(on);
        }
        drop(current);
        self.refresh_hints();
    }

    /// L'ordinateur cache-t-il son pointeur, et depuis combien de temps le
    /// montre-t-il, sur tous les écrans : la règle du cœur
    /// (`PointerLayer::summary`, docs/multi-ecrans.md : un écran où le
    /// pointeur n'est jamais passé ne compte pas).
    fn pointer_summary(&self) -> (bool, Duration) {
        let windows = self.windows();
        let s = PointerLayer::summary(windows.iter().map(|w| w.pointer.as_ref()));
        (s.hidden, s.visible_for)
    }

    /// Les écrans de la session et leur position pour les entrées.
    pub fn targets(&self) -> Vec<ScreenTarget> {
        let mut out: Vec<ScreenTarget> = self
            .windows()
            .iter()
            .filter(|w| w.display_id.load(Ordering::Relaxed) != 0 || w.is_primary())
            .map(|w| ScreenTarget {
                screen: w.index(),
                display_id: w.display_id.load(Ordering::Relaxed),
                host_size: w.host_size(),
                pointer: Some(w.pointer.clone()),
            })
            .collect();
        out.sort_by_key(|t| t.screen);
        out
    }

    /// Écran principal pour les entrées : l'écran de l'ordinateur et sa taille.
    pub fn set_primary_display(&self, id: u32, size: (u32, u32)) {
        *self.primary_display.lock().unwrap() = (id, size);
    }

    pub fn primary_display(&self) -> (u32, (u32, u32)) {
        *self.primary_display.lock().unwrap()
    }

    /// La couche de pointeur de l'écran principal (celle que `InputPipeline`
    /// reçoit à sa création).
    pub fn primary_pointer(&self) -> Option<Arc<PointerLayer>> {
        self.primary_window().map(|w| w.pointer.clone())
    }

    /// Le gestionnaire des paquets d'image de l'écran `screen` du cœur : celui
    /// de sa fenêtre (d'ordinaire déjà là : le cœur a attendu `on_screens`).
    /// Fenêtre pas encore créée (fil des fenêtres en retard) : un gestionnaire
    /// d'attente, que la fenêtre prend en arrivant (`attach_late`) ; fenêtre
    /// qui n'a pas pu se créer : ses images sont jetées.
    pub fn video_for(&self, screen: usize) -> Arc<dyn PacketHandler> {
        let key = self.layout.lock().unwrap().monitors.get(screen).map(|m| m.key.clone());
        let Some(key) = key else {
            log::warn!("image : l'écran {screen} n'est plus un écran de ce PC, ses images sont jetées");
            return Arc::new(Discard);
        };
        // Sous ce verrou : une fenêtre créée au même moment trouve le
        // gestionnaire d'attente (`attach_late` le prend après l'avoir créée).
        let mut late = self.late.lock().unwrap();
        if let Some(w) = self.window_by_key(&key) {
            return w.video.clone();
        }
        if self.failed.lock().unwrap().contains(&key) {
            log::warn!("image : la fenêtre de l'écran {key} n'a pas pu se créer, ses images sont jetées");
            return Arc::new(Discard);
        }
        log::warn!("image : pas encore de fenêtre pour l'écran {key}, ses images attendent qu'elle arrive");
        let feed = Arc::new(LateFeed::default());
        late.push((key, feed.clone()));
        feed
    }

    /// L'image s'est arrêtée : ses gestionnaires d'attente ne servent plus (le
    /// cœur redemande ceux de chaque écran au prochain démarrage).
    pub fn stream_stopped(&self) {
        self.late.lock().unwrap().clear();
    }

    /// Les fenêtres créées après que le cœur a demandé leur gestionnaire
    /// prennent leurs paquets (le fil de l'image repart d'une image clé).
    fn attach_late(&self) {
        let mut late = self.late.lock().unwrap();
        late.retain(|(key, feed)| match self.window_by_key(key) {
            Some(w) => {
                log::info!("image : la fenêtre de l'écran {key} est arrivée en retard, elle reçoit maintenant son image");
                feed.attach(w.video.clone());
                false
            }
            None => true,
        });
    }

    pub fn primary_video(&self) -> Arc<dyn PacketHandler> {
        match self.primary_window() {
            Some(w) => w.video.clone(),
            None => Arc::new(Discard),
        }
    }

    pub fn primary_hwnd(&self) -> u64 {
        self.primary_window().map_or(0, |w| w.hwnd as u64)
    }

    /// Les écrans de la session, dits par le cœur (à chaque démarrage de
    /// l'image) : les fenêtres suivent, sur leur fil ; rend quand c'est fait
    /// (le cœur demande ensuite le gestionnaire de chaque écran).
    pub fn screens_changed(&self, streams: &[StreamScreen], not_created: &[usize]) {
        *self.pending.lock().unwrap() = Some(ScreenUpdate { streams: streams.to_vec(), not_created: not_created.to_vec() });
        let control = self.control();
        if control.0.is_null() {
            return;
        }
        let mut result = 0usize;
        let sent = unsafe {
            SendMessageTimeoutW(control, WM_APP_SCREENS, WPARAM(0), LPARAM(0), SMTO_ABORTIFHUNG, 5000, Some(&mut result))
        };
        if sent.0 == 0 {
            log::warn!(
                "écrans : le fil des fenêtres n'a pas répondu en 5 s ; la session continue, les écrans sans fenêtre la prendront à son arrivée (et repartiront d'une image clé)"
            );
            // Windows peut ne pas remettre le message à un fil qu'il juge
            // bloqué : posé de nouveau, il passera dès que le fil répond (sans
            // effet si les écrans ont déjà été pris).
            unsafe {
                let _ = PostMessageW(Some(control), WM_APP_SCREENS, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// Arrête les fils de l'image (fin de session) et garde la dernière image
    /// de l'écran principal.
    pub fn stop_video(&self) {
        for w in self.windows() {
            w.video.stop_and_wait(Duration::from_secs(2));
            self.keep_last_image(&w);
        }
    }

    fn keep_last_image(&self, w: &ScreenWin) {
        if w.is_primary() {
            if let Some(img) = w.shared.last_image.lock().unwrap().take() {
                *self.last_image.lock().unwrap() = Some(img);
            }
        }
    }

    pub fn take_last_image(&self) -> Option<Vec<u8>> {
        self.last_image.lock().unwrap().take()
    }

    /// « {nom} · KyberNow Link » ; en ligne de commande, suivi de l'étape ou
    /// des mesures (latence, définition, décodeur, images par seconde). Le
    /// même titre sur toutes les fenêtres.
    fn title(&self) -> String {
        let name = format!("{} · {}", self.host_name, crate::shell::DISPLAY_NAME);
        if !self.title_stats {
            return name;
        }
        let status = self.status.lock().unwrap().clone();
        let detail = match status {
            None | Some(Status::Preparing) => t("window.preparing", &[]),
            // La Station redémarre en dernier recours : l'étape reste « Connexion ».
            Some(Status::Connecting | Status::StationRestarting) => t("window.connecting", &[]),
            Some(Status::FirstImages) => t("window.first_images", &[]),
            Some(Status::Streaming) => {
                let lat = self
                    .latency
                    .lock()
                    .unwrap()
                    .map(|l| {
                        let v = format!("{:.1} ms", l);
                        // Virgule décimale en français.
                        if crate::i18n::lang() == Lang::Fr { v.replace('.', ",") } else { v }
                    })
                    .unwrap_or_else(|| "— ms".into());
                let dec = self.primary_window().map(|w| w.shared.decoder.lock().unwrap().clone()).unwrap_or_default();
                let (w, h) = self.primary_window().map(|w| w.host_size()).unwrap_or((0, 0));
                let fps = self.fps.lock().unwrap().to_string();
                t(
                    "window.streaming",
                    &[("latency", &lat), ("width", &w.to_string()), ("height", &h.to_string()), ("decoder", &dec), ("fps", &fps)],
                )
            }
            Some(Status::Reconnecting { attempt, max }) => {
                t("window.reconnecting", &[("attempt", &attempt.to_string()), ("max", &max.to_string())])
            }
            Some(Status::Ending) => t("window.ending", &[]),
            Some(Status::Ended { .. }) => t("window.ended", &[]),
        };
        format!("{name} · {detail}")
    }
}

/// Identifiant d'une nouvelle session dans l'app.
pub fn new_app_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Enregistre les classes de fenêtre (une fois par processus).
fn register_classes() -> windows::core::Result<()> {
    static DONE: OnceLock<()> = OnceLock::new();
    if DONE.get().is_some() {
        return Ok(());
    }
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let screen = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(screen_proc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            lpszClassName: w!("KyberNowSession"),
            ..Default::default()
        };
        RegisterClassExW(&screen);
        let control = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(control_proc),
            hInstance: instance.into(),
            lpszClassName: w!("KyberNowSessionControl"),
            ..Default::default()
        };
        RegisterClassExW(&control);
    }
    let _ = DONE.set(());
    Ok(())
}

/// La fenêtre de contrôle : invisible, elle reçoit ce qui concerne toute la
/// session. À créer avant le cœur (il lui envoie les écrans).
pub fn create_control(app: &App) -> windows::core::Result<()> {
    register_classes()?;
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("KyberNowSessionControl"),
            w!("KyberNow Link"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        )?;
        app.control.store(hwnd.0 as isize, Ordering::Relaxed);
    }
    Ok(())
}

/// Crée la fenêtre d'un écran, cachée, avec son rendu et son fil de l'image
/// (sur le fil des fenêtres : la chaîne d'échange parle à sa fenêtre depuis
/// le même fil). `at` : où la poser (sinon Windows choisit) ; `size` : sa taille
/// tant qu'elle n'est pas en plein écran.
pub fn create_screen(app: &Arc<App>, key: &str, primary: bool, at: Option<(i32, i32)>, size: (i32, i32)) -> Result<Arc<ScreenWin>, String> {
    register_classes().map_err(|e| e.to_string())?;
    let title = to_wide(&app.title());
    let hwnd = unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let mut rect = RECT { left: 0, top: 0, right: size.0, bottom: size.1 };
        match at {
            // Le cadre à l'échelle de l'écran où elle se pose : la zone
            // cliente a la taille demandée (celle de l'écran de l'ordinateur
            // d'une session en fenêtre).
            Some(p) => {
                let (_, _, w, h) = crate::system::session_frame(size, p);
                rect = RECT { left: 0, top: 0, right: w, bottom: h };
            }
            None => {
                let _ = AdjustWindowRectEx(&mut rect, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0));
            }
        }
        let (x, y) = at.unwrap_or((CW_USEDEFAULT, CW_USEDEFAULT));
        // Une seule entrée dans la barre des tâches et Alt+Tab : celle de
        // l'écran principal ; les autres fenêtres de la session n'en ont pas.
        let ex = if primary { WINDOW_EX_STYLE(0) } else { WS_EX_TOOLWINDOW };
        CreateWindowExW(
            ex,
            w!("KyberNowSession"),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            x,
            y,
            rect.right - rect.left,
            rect.bottom - rect.top,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| e.to_string())?
    };
    let shared = Shared::new(app.link.clone(), primary);
    update_size(&shared, hwnd);
    let renderer = match Renderer::new(&app.gpu, hwnd, shared.client_size(), app.video.vsync) {
        Ok(r) => r,
        Err(e) => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(format!("rendu : {e}"));
        }
    };
    let video = match video::spawn(
        renderer,
        hwnd.0 as isize,
        app.gpu.clone(),
        VideoOptions { hardware: app.video.hardware, codec: app.video.codec },
        shared.clone(),
    ) {
        Ok(v) => v,
        Err(e) => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(e);
        }
    };
    // Déplacements bruts de la souris (mode jeu) : à une seule fenêtre de
    // tout le processus, la première ; elle est reprise par une autre si elle
    // disparaît, et par la session active à chaque activation (plusieurs
    // ordinateurs). Seulement une fois la fenêtre sûre de rester (rendu et
    // fil de l'image en place) : une fenêtre détruite ne les recevrait pas,
    // et la souris des jeux ne marcherait plus.
    if RAW_TARGET.load(Ordering::Relaxed) == 0 {
        register_raw_input(hwnd);
    }
    let pointer = Arc::new(PointerLayer::new(app.clock.clone(), false));
    let weak_hwnd = hwnd.0 as isize;
    pointer.set_on_changed(move |_, _| unsafe {
        // Fil de kynput : la fenêtre redessine sur son fil.
        let _ = PostMessageW(Some(HWND(weak_hwnd as *mut _)), WM_APP_CURSOR, WPARAM(0), LPARAM(0));
    });
    let index = app.layout.lock().unwrap().find(key).map_or(0, |(i, _)| i);
    let win = Arc::new(ScreenWin {
        hwnd: hwnd.0 as isize,
        key: key.to_string(),
        index: AtomicUsize::new(index),
        shared,
        video,
        pointer,
        display_id: AtomicU32::new(0),
        host_size: Mutex::new((1920, 1080)),
        primary: AtomicBool::new(primary),
        cursor: Mutex::new(CursorState::default()),
        saved_placement: Mutex::new(None),
        fullscreen_now: AtomicBool::new(false),
        shown: AtomicBool::new(false),
        active: AtomicBool::new(false),
        edge_armed: AtomicBool::new(false),
    });
    app.windows.lock().unwrap().push(win.clone());
    log::info!("fenêtre de session : écran {key}{}", if primary { " (principal)" } else { "" });
    Ok(win)
}

/// Déplacements bruts de la souris (mode jeu) vers cette fenêtre (sur son fil).
fn register_raw_input(hwnd: HWND) {
    let rid = RAWINPUTDEVICE { usUsagePage: 0x01, usUsage: 0x02, dwFlags: RAWINPUTDEVICE_FLAGS(0), hwndTarget: hwnd };
    unsafe {
        let _ = RegisterRawInputDevices(&[rid], std::mem::size_of::<RAWINPUTDEVICE>() as u32);
    }
    RAW_TARGET.store(hwnd.0 as isize, Ordering::Relaxed);
}

pub fn run_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Taille de la zone cliente d'une fenêtre, dite à son fil de l'image.
fn update_size(shared: &Shared, hwnd: HWND) {
    let mut r = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut r);
    }
    shared.set_client_size((r.right - r.left) as u32, (r.bottom - r.top) as u32);
}

fn update_client_size(app: &App, win: &ScreenWin) {
    update_size(&win.shared, win.hwnd());
    // Le fil de l'image redessine l'image en place à la nouvelle taille.
    win.video.resized();
    if app.relative() && app.foreground().is_some_and(|f| f.hwnd == win.hwnd) {
        clip_cursor(win.hwnd(), true);
    }
    // Un changement d'écran change l'échelle du trait du bord haut.
    app.refresh_hints();
}

/// Met la fenêtre au premier plan. Windows le refuse à un fil qui n'a pas
/// reçu la dernière entrée (lancement par la barre des tâches, une
/// notification, un lien, un outil à distance) : le fil se rattache alors un
/// instant à celui de la fenêtre de premier plan, la manière admise de lui
/// passer la main. Sinon, la fenêtre clignote dans la barre des tâches.
fn bring_to_front(hwnd: HWND) -> bool {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    unsafe {
        if SetForegroundWindow(hwnd).as_bool() && GetForegroundWindow() == hwnd {
            return true;
        }
        let fg = GetForegroundWindow();
        let fg_thread = if fg.0.is_null() { 0 } else { GetWindowThreadProcessId(fg, None) };
        let me = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
        if GetForegroundWindow() == hwnd {
            return true;
        }
        let flash = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd,
            dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
            uCount: 0,
            dwTimeout: 0,
        };
        let _ = FlashWindowEx(&flash);
        false
    }
}

/// Plein écran sans bordure sur l'écran de la fenêtre, ou retour à la fenêtre.
/// `activate` : la fenêtre prend le clavier (pas pour les écrans qui
/// viennent après le principal : ils ne lui volent pas le premier plan).
fn set_fullscreen(win: &ScreenWin, on: bool, activate: bool) {
    let hwnd = win.hwnd();
    let mut saved = win.saved_placement.lock().unwrap();
    unsafe {
        if on && saved.is_none() {
            let mut wp = WINDOWPLACEMENT {
                length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
                ..Default::default()
            };
            let _ = GetWindowPlacement(hwnd, &mut wp);
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            *saved = Some((wp, style));
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(monitor, &mut mi);
            SetWindowLongPtrW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE).0 as isize);
            let r = mi.rcMonitor;
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOOWNERZORDER | SWP_FRAMECHANGED | if activate { SWP_SHOWWINDOW } else { SWP_SHOWWINDOW | SWP_NOACTIVATE },
            );
        } else if !on {
            if let Some((wp, style)) = saved.take() {
                SetWindowLongPtrW(hwnd, GWL_STYLE, style);
                let _ = SetWindowPlacement(hwnd, &wp);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
            }
        }
    }
    win.fullscreen_now.store(saved.is_some(), Ordering::Relaxed);
}

/// Replace une fenêtre plein écran sur son écran (qui a changé de place ou de
/// taille).
fn refit(win: &ScreenWin, monitor: &Monitor) {
    if !win.is_fullscreen() {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            win.hwnd(),
            Some(HWND_TOP),
            monitor.x,
            monitor.y,
            monitor.width,
            monitor.height,
            SWP_NOOWNERZORDER | SWP_NOACTIVATE,
        );
    }
}

fn clip_cursor(hwnd: HWND, on: bool) {
    unsafe {
        if on {
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            let mut tl = POINT { x: r.left, y: r.top };
            let mut br = POINT { x: r.right, y: r.bottom };
            let _ = ClientToScreen(hwnd, &mut tl);
            let _ = ClientToScreen(hwnd, &mut br);
            let _ = ClipCursor(Some(&RECT { left: tl.x, top: tl.y, right: br.x, bottom: br.y }));
        } else {
            let _ = ClipCursor(None);
        }
    }
}

/// Souris relative quand on la force, ou (Auto) quand un jeu est en plein
/// écran sur l'ordinateur et y cache son pointeur (règles du cœur : un
/// pointeur remontré moins de 0,6 s ne fait pas quitter le mode jeu, revu
/// 0,7 s après). Plusieurs écrans : le pointeur n'est caché que s'il l'est
/// sur tous ; la souris des jeux est retenue dans la fenêtre active.
fn apply_mouse_mode(app: &Arc<App>) {
    let mode = *app.mouse_mode.lock().unwrap();
    let (remote_hidden, visible_for) = app.pointer_summary();
    let current = app.relative();
    let relative = MOUSE_RULES.wants_relative(mode, app.game.load(Ordering::Relaxed), remote_hidden, visible_for, current);
    if mode == MouseMode::Auto && current && relative && !remote_hidden {
        // Pointeur remontré depuis peu : on reste en jeu, et on revoit à 0,7 s.
        let left = MouseModeRules::RECHECK_DELAY.saturating_sub(visible_for).max(Duration::from_millis(10));
        unsafe {
            SetTimer(Some(app.control()), TIMER_MOUSE, left.as_millis() as u32, None);
        }
    }
    if relative == current {
        return;
    }
    log::info!("souris : {}", if relative { "jeu (relative)" } else { "bureau" });
    app.set_relative(relative);
    let focused = app.foreground();
    if relative {
        for w in app.windows() {
            edge_cancel(&w);
        }
    }
    // Le pointeur retenu (ClipCursor) vaut pour tout Windows : une session qui
    // n'est pas au premier plan ne touche pas à celui d'une autre.
    match &focused {
        Some(w) => clip_cursor(w.hwnd(), relative),
        None if !relative && !any_in_front() => clip_cursor(HWND::default(), false),
        None => {}
    }
    if let Some(w) = &focused {
        unsafe {
            if relative {
                SetCursor(None);
            } else {
                let cur = w.cursor.lock().unwrap().current;
                SetCursor(if cur != 0 { Some(HCURSOR(cur as *mut _)) } else { LoadCursorW(None, IDC_ARROW).ok() });
            }
        }
    }
    // Pointeur visible sur l'ordinateur (menus d'un jeu) : montré tout de suite.
    if relative {
        draw_game_pointer(app);
    }
}

/// Échelle de l'image : pixels de ce PC par pixel de l'ordinateur. Le
/// pointeur la suit, pour garder sa taille par rapport à l'image (bornée).
fn image_scale(win: &ScreenWin) -> f32 {
    let host = win.host_size();
    let r = video_rect(win.shared.client_size(), host);
    let s = (r.right - r.left) as f32 / host.0.max(1) as f32;
    if s.is_finite() { s.clamp(0.5, 3.0) } else { 1.0 }
}

/// Image du pointeur agrandie ou réduite (au plus proche : il reste net).
fn scaled(px: &[u32], w: u32, h: u32, scale: f32) -> (Vec<u32>, u32, u32) {
    if (scale - 1.0).abs() < 0.05 {
        return (px.to_vec(), w, h);
    }
    let (nw, nh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
    let mut out = Vec::with_capacity((nw * nh) as usize);
    for y in 0..nh {
        let sy = ((y as f32 + 0.5) / scale) as u32;
        for x in 0..nw {
            let sx = ((x as f32 + 0.5) / scale) as u32;
            out.push(px[(sy.min(h - 1) * w + sx.min(w - 1)) as usize]);
        }
    }
    (out, nw, nh)
}

/// Construit un pointeur Windows depuis l'image ARVB de l'ordinateur (alpha
/// non prémultiplié, comme dans les pointeurs de Windows), à l'échelle de
/// l'image. Les formes vides ou entièrement transparentes sont déjà écartées
/// par le cœur.
fn make_cursor(shape: &Shape, scale: f32) -> Option<HCURSOR> {
    let (width, height) = (shape.width, shape.height);
    if width == 0 || height == 0 || shape.argb.len() < (width * height) as usize {
        return None;
    }
    let (px, w, h) = scaled(&shape.argb[..(width * height) as usize], width, height, scale);
    let (w, h) = (w as i32, h as i32);
    let k = w as f32 / width as f32;
    let (xhot, yhot) = (((shape.hot_x as f32) * k) as u32, ((shape.hot_y as f32) * k) as u32);
    unsafe {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let hdc = GetDC(None);
        let mut bits = std::ptr::null_mut();
        let color = CreateDIBSection(Some(hdc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok();
        ReleaseDC(None, hdc);
        let color = color?;
        std::ptr::copy_nonoverlapping(px.as_ptr(), bits as *mut u32, (w * h) as usize);
        let mask = CreateBitmap(w, h, 1, 1, None);
        let info = ICONINFO {
            fIcon: false.into(),
            xHotspot: xhot,
            yHotspot: yhot,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&info).ok();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon.map(|i| HCURSOR(i.0))
    }
}

/// Le pointeur de l'ordinateur a changé (`PointerLayer` de cet écran, fil de
/// kynput) : nouvelle forme ou nouvelle échelle de l'image, mode de la souris.
fn update_cursor(app: &Arc<App>, win: &Arc<ScreenWin>) {
    let pointer = win.pointer.state();
    if let Some(shape) = &pointer.shape {
        let scale = image_scale(win);
        let mut st = win.cursor.lock().unwrap();
        // Le cœur ne livre une forme neuve que si elle a changé : les mêmes
        // pixels sont le même tableau.
        let same = st.built.as_ref().is_some_and(|(px, s)| Arc::ptr_eq(px, &shape.argb) && (scale - s).abs() <= 0.01);
        if !same && let Some(new) = make_cursor(shape, scale) {
            st.built = Some((shape.argb.clone(), scale));
            let old = std::mem::replace(&mut st.current, new.0 as isize);
            unsafe {
                let mut p = POINT::default();
                let _ = GetCursorPos(&mut p);
                if WindowFromPoint(p) == win.hwnd() && (!app.relative() || !pointer.remote_hidden) {
                    SetCursor(Some(new));
                }
                if old != 0 {
                    let _ = DestroyCursor(HCURSOR(old as *mut _));
                }
            }
        }
    }
    apply_mouse_mode(app);
    draw_game_pointer(app);
}

/// Mode jeu (souris relative) avec un pointeur visible sur l'ordinateur
/// (menus d'un jeu) : le pointeur de ce PC le montre là où l'ordinateur
/// l'annonce, dans la fenêtre active. Sinon, caché.
fn draw_game_pointer(app: &App) {
    let Some(win) = app.foreground() else { return };
    if !app.relative() {
        return;
    }
    let pointer = win.pointer.state();
    let st = win.cursor.lock().unwrap();
    unsafe {
        let shown = pointer.remote_position.filter(|_| !pointer.remote_hidden && st.current != 0);
        let Some((x, y)) = shown else {
            SetCursor(None);
            return;
        };
        let host = win.host_size();
        let r = video_rect(win.shared.client_size(), host);
        let (vw, vh) = (f64::from(r.right - r.left), f64::from(r.bottom - r.top));
        let mut p = POINT {
            x: r.left + (x * vw / f64::from(host.0.max(1))) as i32,
            y: r.top + (y * vh / f64::from(host.1.max(1))) as i32,
        };
        let _ = ClientToScreen(win.hwnd(), &mut p);
        let _ = SetCursorPos(p.x, p.y);
        SetCursor(Some(HCURSOR(st.current as *mut _)));
    }
}

/// Ctrl+Alt+Maj+M, ou la barre : souris des jeux ou du bureau, forcée.
fn toggle_mouse(app: &Arc<App>) {
    app.set_mouse_mode(if app.relative() { MouseMode::Desktop } else { MouseMode::Game });
    apply_mouse_mode(app);
}

/// Menu de l'icône : Auto, souris du bureau ou des jeux.
fn set_mouse(app: &Arc<App>, mode: MouseMode) {
    app.set_mouse_mode(mode);
    apply_mouse_mode(app);
}

/// Plein écran ou fenêtre, à un seul écran : celui de l'écran principal.
fn toggle_fullscreen(app: &Arc<App>) {
    if app.streams.load(Ordering::Relaxed) > 1 {
        // Plusieurs écrans : les fenêtres restent en plein écran (pour
        // travailler dans une fenêtre, « Cet écran »).
        return;
    }
    let Some(win) = app.primary_window() else { return };
    let full = win.saved_placement.lock().unwrap().is_some();
    set_fullscreen(&win, !full, true);
}

fn shortcut_held() -> bool {
    unsafe {
        GetKeyState(VK_CONTROL.0 as i32) < 0 && GetKeyState(VK_MENU.0 as i32) < 0 && GetKeyState(VK_SHIFT.0 as i32) < 0
    }
}

/// Fin de session demandée par l'utilisateur (« Se déconnecter » de la barre
/// ou du menu de l'icône, fermeture d'une fenêtre, Ctrl+Alt+Maj+D) : seule
/// cette session s'arrête, sans notification de fin.
fn user_close(app: &Arc<App>) {
    app.user_ended.store(true, Ordering::Relaxed);
    request_close(app);
}

/// Ctrl+Alt+Maj+O, « Ordinateur suivant » : la session ouverte après
/// l'active (le registre du cœur le dit) ; sa fenêtre passe devant, même en
/// plein écran, et l'y active en passant au premier plan.
fn next_computer(app: &Arc<App>) {
    let Some(next) = focus::next().filter(|n| Some(*n) != app.focus_id()) else { return };
    if let Some(target) = apps().into_iter().find(|a| a.focus_id() == Some(next)) {
        target.post_command(Command::Focus, 0);
    }
}

/// Fin de session demandée (n'importe quelle fenêtre, la barre, l'icône).
fn request_close(app: &Arc<App>) {
    if app.closing.swap(true, Ordering::Relaxed) {
        return;
    }
    match app.session.get() {
        Some(s) => s.stop(),
        None => quit(app),
    }
}

/// Toutes les fenêtres ferment, la boucle de messages s'arrête.
fn quit(app: &Arc<App>) {
    // Avant que ses fenêtres partent (une autre session prendrait la place).
    app.was_active.store(app.focus_id().is_some() && focus::active() == app.focus_id(), Ordering::Relaxed);
    app.was_front.store(app.in_front(), Ordering::Relaxed);
    app.forget_raw_target();
    if app.relative() && app.foreground().is_some() {
        clip_cursor(HWND::default(), false);
    }
    for w in app.windows() {
        unsafe {
            let _ = DestroyWindow(w.hwnd());
        }
    }
    unsafe {
        let _ = KillTimer(Some(app.control()), TIMER_MOUSE);
        let _ = KillTimer(Some(app.control()), TIMER_DISPLAYS);
        PostQuitMessage(0);
    }
}

/// Le point (pixels de la zone cliente) est contre le bord haut de la fenêtre,
/// à 2 points au plus, dans la zone centrée de la largeur de la barre de
/// session : l'endroit que signale le trait.
fn in_edge_zone(win: &ScreenWin, x: i32, y: i32) -> bool {
    let scale = dpi_scale(win.hwnd());
    let width = win.shared.client_size().0 as i32;
    let half = ((BAR_WIDTH.load(Ordering::Relaxed) as f32 * scale / 2.0).round() as i32).min(width / 2);
    (0..=(EDGE_SLACK_POINTS * scale).round() as i32).contains(&y) && (x - width / 2).abs() <= half
}

/// Le pointeur est, en ce moment, sur la fenêtre de cet écran et dans la zone
/// du bord haut.
fn cursor_in_edge_zone(win: &ScreenWin) -> bool {
    unsafe {
        let mut p = POINT::default();
        if GetCursorPos(&mut p).is_err() || WindowFromPoint(p) != win.hwnd() {
            return false;
        }
        let _ = ScreenToClient(win.hwnd(), &mut p);
        in_edge_zone(win, p.x, p.y)
    }
}

/// Le pointeur ne reste plus contre le bord haut : plus d'attente.
fn edge_cancel(win: &ScreenWin) {
    if win.edge_armed.swap(false, Ordering::Relaxed) {
        unsafe {
            let _ = KillTimer(Some(win.hwnd()), TIMER_EDGE);
        }
    }
}

/// Mouvement du pointeur sur la fenêtre d'un écran : arrivé dans la zone du
/// bord haut (app au premier plan, souris du bureau, aucun bouton enfoncé :
/// un glisser vers le haut ne l'ouvre pas), il lance une attente de 0,25 s ;
/// sorti de la zone, il l'annule. Un pointeur immobile n'envoie plus rien :
/// c'est la minuterie qui décide (`edge_fired`).
fn edge_pointer(app: &App, win: &ScreenWin, wparam: WPARAM, lparam: LPARAM) {
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let on = wparam.0 & MK_BUTTONS == 0 && app.edge_ready() && in_edge_zone(win, x, y) && app.in_front();
    if !on {
        edge_cancel(win);
    } else if !win.edge_armed.swap(true, Ordering::Relaxed) {
        // Les petits mouvements dans la zone ne relancent pas l'attente.
        unsafe {
            SetTimer(Some(win.hwnd()), TIMER_EDGE, EDGE_DWELL_MS, None);
        }
    }
}

/// Un bouton de la souris est tenu (une fenêtre de l'ordinateur glissée contre
/// le bord haut, par exemple).
fn mouse_button_held() -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2]
        .iter()
        .any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } < 0)
}

/// 0,25 s contre le bord haut : la barre s'ouvre si tout tient encore (le
/// pointeur dans la zone et sur cet écran, app au premier plan, souris du
/// bureau, aucun bouton tenu), sur l'écran où le geste a lieu ; la fenêtre où
/// elle s'ouvre devient la fenêtre active (le clavier y va, la barre s'y pose).
fn edge_fired(app: &App, win: &ScreenWin) {
    edge_cancel(win);
    if app.edge_ready() && app.in_front() && !mouse_button_held() && cursor_in_edge_zone(win) {
        unsafe {
            // Une autre fenêtre de la même app : Windows l'accepte.
            let _ = SetForegroundWindow(win.hwnd());
        }
        app.events.top_edge(win.hwnd);
    }
}

/// Position de la souris sur ce PC, vers l'ordinateur : l'écran de
/// l'ordinateur de la fenêtre sous le pointeur. Pendant un glisser (bouton
/// tenu, souris capturée par la fenêtre du clic), les mouvements arrivent à
/// la fenêtre du clic même au-dessus d'une autre : l'écran qui est vraiment
/// sous le pointeur compte.
fn mouse_position(app: &App, win: &Arc<ScreenWin>, lparam: LPARAM) {
    // Plusieurs ordinateurs : sur la fenêtre d'une session inactive, rien ne
    // part (la chaîne d'entrées du cœur l'arrête, `Session::set_active`).
    if app.relative() {
        return;
    }
    let Some(input) = app.input() else { return };
    let x = (lparam.0 & 0xFFFF) as i16 as i32;
    let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
    let mut p = POINT { x, y };
    unsafe {
        let _ = ClientToScreen(win.hwnd(), &mut p);
    }
    let target = app.window_at(p).unwrap_or_else(|| win.clone());
    if target.hwnd != win.hwnd {
        unsafe {
            let _ = ScreenToClient(target.hwnd(), &mut p);
        }
    } else {
        p = POINT { x, y };
    }
    let host = target.host_size();
    let r = video_rect(target.shared.client_size(), host);
    let (vw, vh) = ((r.right - r.left).max(1), (r.bottom - r.top).max(1));
    let hx = ((p.x - r.left) as i64 * host.0 as i64 / vw as i64).clamp(0, host.0 as i64 - 1);
    let hy = ((p.y - r.top) as i64 * host.1 as i64 / vh as i64).clamp(0, host.1 as i64 - 1);
    input.mouse_on(target.index(), hx.min(i16::MAX as i64) as i16, hy.min(i16::MAX as i64) as i16);
}

fn mouse_button(app: &App, hwnd: HWND, button: MouseButtonType, pressed: bool) {
    unsafe {
        if pressed {
            SetCapture(hwnd);
        } else {
            let _ = ReleaseCapture();
        }
    }
    let Some(input) = app.input() else { return };
    // Relâchement d'un bouton dont l'appui n'est pas parti : rien. Un clic
    // sur la fenêtre d'une session inactive l'active d'abord (WM_ACTIVATE
    // vient avant le bouton) : il part à son ordinateur.
    if !pressed && !input.held().lock().unwrap().mouse_buttons().contains(&button) {
        return;
    }
    input.mouse_button(button, pressed);
}

fn raw_input(app: &App, lparam: LPARAM) {
    if !app.relative() {
        return;
    }
    let Some(input) = app.input() else { return };
    unsafe {
        let mut raw = RAWINPUT::default();
        let mut size = std::mem::size_of::<RAWINPUT>() as u32;
        let n = GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_INPUT,
            Some(&mut raw as *mut _ as *mut _),
            &mut size,
            std::mem::size_of::<RAWINPUTHEADER>() as u32,
        );
        if n == u32::MAX || raw.header.dwType != RIM_TYPEMOUSE.0 {
            return;
        }
        let m = raw.data.mouse;
        if (m.usFlags.0 & MOUSE_MOVE_ABSOLUTE.0) == 0 && (m.lLastX != 0 || m.lLastY != 0) {
            input.mouse_move(m.lLastX.clamp(-32768, 32767) as i16, m.lLastY.clamp(-32768, 32767) as i16);
        }
    }
}

/// Une fenêtre d'écran ferme : son fil de l'image s'arrête, la fenêtre part
/// (la souris relative et les déplacements bruts passent à une autre).
fn remove_window(app: &Arc<App>, win: &Arc<ScreenWin>) {
    log::info!("fenêtre de session fermée : écran {}", win.key);
    app.windows.lock().unwrap().retain(|w| w.hwnd != win.hwnd);
    edge_cancel(win);
    // Sans attendre son fil de l'image (DXGI peut avoir besoin de ce fil-ci
    // pendant ce temps) : il s'arrête de lui-même et lâche sa chaîne d'échange.
    win.video.stop();
    unsafe {
        if app.relative() && !any_in_front() {
            let _ = ClipCursor(None);
        }
        let _ = DestroyWindow(win.hwnd());
    }
    let mut cur = win.cursor.lock().unwrap();
    if cur.current != 0 {
        unsafe {
            let _ = DestroyCursor(HCURSOR(cur.current as *mut _));
        }
        cur.current = 0;
    }
    drop(cur);
    if RAW_TARGET.load(Ordering::Relaxed) == win.hwnd {
        match app.windows().first() {
            Some(other) => register_raw_input(other.hwnd()),
            None => RAW_TARGET.store(0, Ordering::Relaxed),
        }
    }
    if app.last_active.load(Ordering::Relaxed) == win.hwnd {
        app.last_active.store(0, Ordering::Relaxed);
    }
}

/// Les écrans de la session, dits par le cœur : une fenêtre par flux (celles
/// des écrans qui n'en ont plus ferment, les nouvelles se créent cachées), et
/// l'état de chacune donné à l'app (son interface montre le fond de la
/// connexion sur les écrans sans image).
fn apply_screens(app: &Arc<App>) {
    let Some(update) = app.pending.lock().unwrap().take() else { return };
    let layout = app.layout.lock().unwrap().clone();
    // Chaque flux avec l'écran de ce PC où il va.
    let streams: Vec<(Monitor, StreamScreen)> =
        update.streams.iter().filter_map(|s| layout.monitors.get(s.screen).map(|m| (m.clone(), s.clone()))).collect();
    if streams.len() != update.streams.len() {
        log::warn!("écrans : la liste du cœur ne correspond plus aux écrans de ce PC, flux ignorés");
    }
    // Les fenêtres des écrans qui n'ont plus de flux ferment.
    for w in app.windows() {
        if !streams.iter().any(|(m, _)| m.key == w.key) {
            remove_window(app, &w);
        }
    }
    // Les fenêtres des nouveaux écrans, cachées jusqu'à leur première image.
    let mut failed: Vec<String> = Vec::new();
    for (m, s) in &streams {
        if app.window_by_key(&m.key).is_some() {
            continue;
        }
        let size = (((m.width as f32) * 0.8) as i32, ((m.height as f32) * 0.8) as i32);
        if let Err(e) = create_screen(app, &m.key, s.primary, Some((m.x + 60, m.y + 60)), size) {
            log::error!("fenêtre de l'écran {} : {e}", m.key);
            if s.primary {
                // Sans la fenêtre de l'écran principal, ni image ni fin de la
                // connexion : la session s'arrête avec la raison.
                *app.error.lock().unwrap() = Some(t("errors.session_window", &[("error", &e)]));
                request_close(app);
                return;
            }
            failed.push(m.key.clone());
        }
    }
    // Une fenêtre qui n'a pas pu se créer : l'écran est traité comme un écran
    // que l'ordinateur n'a pas pu créer (le fond de la connexion avec sa
    // ligne, docs/link-ui.md), ses images sont jetées.
    *app.failed.lock().unwrap() = failed.clone();
    app.attach_late();
    let taken = streams.len() - failed.len();
    let multi = taken > 1;
    app.streams.store(taken, Ordering::Relaxed);
    for (m, s) in &streams {
        let Some(w) = app.window_by_key(&m.key) else { continue };
        w.index.store(s.screen, Ordering::Relaxed);
        w.primary.store(s.primary, Ordering::Relaxed);
        w.shared.primary.store(s.primary, Ordering::Relaxed);
        w.display_id.store(s.display.id, Ordering::Relaxed);
        let size = (s.display.width.max(1) as u32, s.display.height.max(1) as u32);
        *w.host_size.lock().unwrap() = size;
        w.pointer.set_host_size((f64::from(size.0), f64::from(size.1)));
        if s.primary {
            app.set_primary_display(s.display.id, size);
        }
        if multi {
            // Plusieurs écrans : toujours en plein écran, chacun sur le sien
            // (le principal garde le clavier).
            if w.shown.load(Ordering::Relaxed) {
                if w.is_fullscreen() {
                    refit(&w, m);
                } else {
                    unsafe {
                        let _ = SetWindowPos(w.hwnd(), None, m.x + 60, m.y + 60, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                    }
                    set_fullscreen(&w, true, false);
                }
            }
        }
    }
    // L'écran principal est la référence des mesures.
    let set = ScreenSet {
        screens: layout
            .monitors
            .iter()
            .enumerate()
            .map(|(i, m)| ScreenState {
                key: m.key.clone(),
                x: m.x,
                y: m.y,
                width: m.width.max(0) as u32,
                height: m.height.max(0) as u32,
                role: match streams.iter().find(|(sm, _)| sm.key == m.key) {
                    Some(_) if failed.contains(&m.key) => Role::NotCreated,
                    Some((_, s)) => Role::Stream {
                        session_primary: s.primary,
                        width: s.display.width.max(0) as u32,
                        height: s.display.height.max(0) as u32,
                        refresh: s.refresh,
                    },
                    None if update.not_created.contains(&i) => Role::NotCreated,
                    None => Role::Unused,
                },
            })
            .collect(),
    };
    app.refresh_hints();
    app.events.screens(&set);
}

/// Les écrans de ce PC ont changé (branché, débranché, disposition, échelle,
/// recopie) : le cœur refait la session avec les nouveaux écrans, sans
/// revenir à l'accueil, et les fenêtres se replacent.
fn displays_changed(app: &Arc<App>) {
    let now = crate::displays::enumerate();
    let screens = {
        let mut layout = app.layout.lock().unwrap();
        if layout.monitors == now {
            return;
        }
        log::info!(
            "écrans de ce PC : {} → {}",
            layout.monitors.iter().map(|m| format!("{} {}×{}+{}+{}", m.key, m.width, m.height, m.x, m.y)).collect::<Vec<_>>().join(", "),
            now.iter().map(|m| format!("{} {}×{}+{}+{}", m.key, m.width, m.height, m.x, m.y)).collect::<Vec<_>>().join(", ")
        );
        layout.monitors = now.clone();
        layout.screens()
    };
    for w in app.windows() {
        if let Some(m) = now.iter().find(|m| m.key == w.key) {
            refit(&w, m);
        }
    }
    if let Some(s) = app.session.get() {
        s.set_screens(screens);
    }
}

unsafe extern "system" fn control_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let app = || app_of_control(hwnd.0 as isize);
        match msg {
            WM_APP_SCREENS => {
                if let Some(a) = app() {
                    apply_screens(&a);
                }
                LRESULT(0)
            }
            WM_APP_DISPLAYS => {
                SetTimer(Some(hwnd), TIMER_DISPLAYS, DISPLAYS_SETTLE_MS, None);
                LRESULT(0)
            }
            WM_APP_MOUSE => {
                if let Some(a) = app() {
                    apply_mouse_mode(&a);
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_MOUSE => {
                let _ = KillTimer(Some(hwnd), TIMER_MOUSE);
                if let Some(a) = app() {
                    apply_mouse_mode(&a);
                }
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_DISPLAYS => {
                let _ = KillTimer(Some(hwnd), TIMER_DISPLAYS);
                if let Some(a) = app() {
                    displays_changed(&a);
                }
                LRESULT(0)
            }
            WM_APP_STATUS => {
                if let Some(a) = app() {
                    let title = to_wide(&a.title());
                    for w in a.windows() {
                        let _ = SetWindowTextW(w.hwnd(), PCWSTR(title.as_ptr()));
                    }
                    if matches!(*a.status.lock().unwrap(), Some(Status::Ended { .. })) {
                        quit(&a);
                    }
                }
                LRESULT(0)
            }
            WM_APP_COMMAND => {
                if let Some(a) = app() {
                    command(&a, wparam.0, lparam.0);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Un ordre de l'app (barre de session, menu de l'icône). `arg` : la fenêtre
/// visée par `Focus` (0 : la dernière active).
fn command(app: &Arc<App>, c: usize, arg: isize) {
    match c {
        x if x == Command::Disconnect as usize => user_close(app),
        x if x == Command::Fullscreen as usize => toggle_fullscreen(app),
        x if x == Command::MouseToggle as usize => toggle_mouse(app),
        x if x == Command::MouseAuto as usize => set_mouse(app, MouseMode::Auto),
        x if x == Command::MouseDesktop as usize => set_mouse(app, MouseMode::Desktop),
        x if x == Command::MouseGame as usize => set_mouse(app, MouseMode::Game),
        // Le clavier revient à la session : la fenêtre demandée (celle où la
        // barre s'est ouverte), sinon la dernière active, sinon l'écran
        // principal. Aussi « Afficher » et « Ordinateur suivant » (plusieurs
        // ordinateurs) : sa fenêtre passe devant, rouverte si elle était réduite.
        x if x == Command::Focus as usize => {
            let last = app.last_active.load(Ordering::Relaxed);
            let target = app.window(arg).or_else(|| app.window(last)).or_else(|| app.primary_window());
            if let Some(w) = target.filter(|w| w.shown.load(Ordering::Relaxed)) {
                unsafe {
                    if IsIconic(w.hwnd()).as_bool() {
                        let _ = ShowWindow(w.hwnd(), SW_RESTORE);
                    }
                }
                bring_to_front(w.hwnd());
            }
        }
        x if x == Command::CtrlAltDel as usize => send_combo(app, &[sc::CTRL, sc::ALT, sc::DEL]),
        x if x == Command::WinKey as usize => send_combo(app, &[sc::LWIN]),
        x if x == Command::AltTab as usize => send_combo(app, &[sc::ALT, sc::TAB]),
        x if x == Command::PrintScreen as usize => send_combo(app, &[sc::PRINT]),
        _ => {}
    }
}

/// Première image de ce flux : la fenêtre de l'écran se montre (la première
/// fois), et l'app le sait, pour le fond de la connexion de cet écran.
fn first_image(app: &Arc<App>, win: &Arc<ScreenWin>) {
    let hwnd = win.hwnd();
    if !win.shown.swap(true, Ordering::Relaxed) {
        let multi = app.streams.load(Ordering::Relaxed) > 1;
        let primary = win.is_primary();
        unsafe {
            let _ = ShowWindow(hwnd, if primary { SW_SHOW } else { SW_SHOWNA });
        }
        if app.fullscreen || multi {
            set_fullscreen(win, true, primary);
        }
        update_client_size(app, win);
        if primary {
            app.ever_shown.store(true, Ordering::Relaxed);
            let front = bring_to_front(hwnd);
            if !front {
                log::warn!("Windows refuse le premier plan à la session : elle attend dans la barre des tâches");
            }
            app.refresh_hints();
            // L'accueil se cache dans tous les cas : laissé devant, il
            // montrait le passage ouvert sur du noir.
            app.events.visible(front);
        } else {
            unsafe {
                let _ = SetWindowPos(hwnd, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            app.refresh_hints();
        }
    }
    app.events.screen_visible(&win.key);
}

unsafe extern "system" fn screen_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        let found = app_of(hwnd.0 as isize);
        let Some((app, win)) = found else {
            // Fenêtre en création ou déjà retirée.
            return match msg {
                WM_ERASEBKGND => LRESULT(1),
                WM_CLOSE => LRESULT(0),
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            };
        };
        match msg {
            WM_SIZE => {
                update_client_size(&app, &win);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_DISPLAYCHANGE => {
                app.post(WM_APP_DISPLAYS);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
                let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let repeat = (lparam.0 >> 30) & 1 == 1;
                if pressed && shortcut_held() {
                    let handled = match wparam.0 as u8 {
                        b'D' => {
                            user_close(&app);
                            true
                        }
                        b'O' => {
                            if !repeat {
                                next_computer(&app);
                            }
                            true
                        }
                        b'F' => {
                            if !repeat {
                                toggle_fullscreen(&app);
                            }
                            true
                        }
                        b'M' => {
                            if !repeat {
                                toggle_mouse(&app);
                            }
                            true
                        }
                        _ => false,
                    };
                    if handled {
                        return LRESULT(0);
                    }
                }
                if pressed && repeat {
                    // L'ordinateur répète lui-même les touches tenues.
                    return LRESULT(0);
                }
                let sc = ((lparam.0 >> 16) & 0xFF) as u16;
                let extended = (lparam.0 >> 24) & 1 == 1;
                let code = if extended { 0xE000 | sc } else { sc };
                if sc != 0 {
                    send_key(&app, code, pressed);
                }
                // Pas de menu système sur Alt ni F10 : tout part à l'ordinateur.
                LRESULT(0)
            }
            WM_SYSCHAR | WM_CHAR => LRESULT(0),
            WM_MOUSEMOVE => {
                mouse_position(&app, &win, lparam);
                edge_pointer(&app, &win, wparam, lparam);
                LRESULT(0)
            }
            WM_ACTIVATE => {
                // Active, et pas réduite : le trait du bord haut se montre.
                let active = wparam.0 & 0xFFFF != WA_INACTIVE as usize && (wparam.0 >> 16) & 0xFFFF == 0;
                win.active.store(active, Ordering::Relaxed);
                if active {
                    app.last_active.store(win.hwnd, Ordering::Relaxed);
                    // Plusieurs ordinateurs : cette session devient la session
                    // active du registre du cœur (clavier, souris, manettes,
                    // presse-papiers, son) ; les déplacements bruts de la
                    // souris viennent à ses fenêtres.
                    if let Some(id) = app.focus_id() {
                        focus::activate(id);
                    }
                    if app.window(RAW_TARGET.load(Ordering::Relaxed)).is_none() {
                        register_raw_input(hwnd);
                    }
                } else {
                    edge_cancel(&win);
                }
                app.refresh_hints();
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_APP_FIRST_IMAGE => {
                first_image(&app, &win);
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_LBUTTONUP => {
                mouse_button(&app, hwnd, MouseButtonType::Left, msg == WM_LBUTTONDOWN);
                LRESULT(0)
            }
            WM_RBUTTONDOWN | WM_RBUTTONUP => {
                mouse_button(&app, hwnd, MouseButtonType::Right, msg == WM_RBUTTONDOWN);
                LRESULT(0)
            }
            WM_MBUTTONDOWN | WM_MBUTTONUP => {
                mouse_button(&app, hwnd, MouseButtonType::Middle, msg == WM_MBUTTONDOWN);
                LRESULT(0)
            }
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let b = if (wparam.0 >> 16) & 0xFFFF == 1 { MouseButtonType::Side } else { MouseButtonType::Extra };
                mouse_button(&app, hwnd, b, msg == WM_XBUTTONDOWN);
                LRESULT(1)
            }
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                // Windows envoie la molette à la fenêtre sous le pointeur, même
                // inactive : la chaîne d'une session inactive ne la laisse pas partir.
                if let Some(input) = app.input() {
                    let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as f32 / 120.0;
                    if msg == WM_MOUSEWHEEL {
                        input.wheel(0.0, -delta);
                    } else {
                        // Comme le client de Kyber : les deux axes inversés.
                        input.wheel(-delta, 0.0);
                    }
                }
                LRESULT(0)
            }
            WM_INPUT => {
                raw_input(&app, lparam);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_SETCURSOR => {
                if (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
                    if app.relative() && win.pointer.state().remote_hidden {
                        SetCursor(None);
                    } else {
                        let cur = win.cursor.lock().unwrap().current;
                        SetCursor(if cur != 0 { Some(HCURSOR(cur as *mut _)) } else { LoadCursorW(None, IDC_ARROW).ok() });
                    }
                    return LRESULT(1);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_KILLFOCUS => {
                // Rien ne reste enfoncé sur l'ordinateur quand on change d'app
                // (pas quand le clavier passe à une autre fenêtre de la session).
                let to_other_screen = app.window(wparam.0 as isize).is_some();
                if !to_other_screen && let Some(input) = app.input() {
                    input.release_all();
                }
                clip_cursor(hwnd, false);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if app.relative() {
                    clip_cursor(hwnd, true);
                    // Le pointeur de l'ordinateur à montrer vient de cet écran.
                    draw_game_pointer(&app);
                }
                LRESULT(0)
            }
            WM_APP_CURSOR => {
                update_cursor(&app, &win);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_EDGE => {
                edge_fired(&app, &win);
                LRESULT(0)
            }
            WM_CLOSE => {
                // Fermer une fenêtre, c'est se déconnecter : toute sa session
                // s'arrête (les autres ordinateurs continuent).
                user_close(&app);
                LRESULT(0)
            }
            WM_DESTROY => {
                if win.edge_armed.load(Ordering::Relaxed) {
                    edge_cancel(&win);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
