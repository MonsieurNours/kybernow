//! Pont C du module d'entrées du cœur (`kybernow_core::input`, voir
//! docs/coeur-apple.md) : la couche de pointeur de la vue de session
//! (`KnPointer`, une par vue, qui survit aux relances de l'image) et la
//! chaîne d'entrées d'une image (`KnInput`, prêtée à la session par
//! `input_pipeline`). Swift ne garde que la capture des événements de sa
//! plateforme (AppKit, GameController), le dessin du pointeur et la table
//! kVK → HID du Mac.
//!
//! Rappels : `on_pointer` sur le fil réseau de kynput (ou le fil qui appelle
//! `kn_pointer_*`) ; `on_combo` sur le fil des entrées du cœur ; `on_navigate`
//! sur le fil de kynput, sous le verrou de son routeur (ne rien y envoyer) ;
//! `on_connected` sur le fil de `kn_input_network_connected` ; le
//! presse-papiers sur le fil des entrées du cœur. Swift remonte sur le fil
//! principal ce qui touche l'interface.

use std::ffi::{c_char, c_void};
use std::sync::Arc;
use std::time::Duration;

use kybernow_core::input::keymap;
use kybernow_core::input::{
    Clock, GamepadRumble, InputPipeline, InputPlatform, MonotonicClock, MouseMode, MouseModeRules, Options, PadAxis, PadButton, PadCombo, PadId, PadKey, Pasteboard,
    PointerLayer,
};
use kybernow_core::kynput_rs::MouseButtonType;

use crate::text;

unsafe extern "C" {
    fn free(p: *mut c_void);
}

// MARK: - Pointeur

/// État du pointeur de l'ordinateur. `pixels` : la forme, ARGB prémultiplié
/// sur 32 bits (BGRA en mémoire), `width` × `height` ; seulement dans
/// `on_pointer` quand la forme a changé, nul sinon (et dans `kn_pointer_state`).
#[repr(C)]
pub struct KnPointerState {
    pub has_position: bool,
    pub x: f64,
    pub y: f64,
    pub remote_hidden: bool,
    /// Depuis combien de temps l'ordinateur montre son pointeur (0 : caché).
    pub visible_for_ms: f64,
    pub has_shape: bool,
    pub width: u32,
    pub height: u32,
    pub hot_x: u32,
    pub hot_y: u32,
    pub pixels: *const u32,
}

#[repr(C)]
pub struct KnPointerCallbacks {
    pub userdata: *mut c_void,
    /// La couche a changé : `visibility` (pointeur montré ou caché), `shape`
    /// (nouvelle forme, `pixels` donnés). Sans ces deux drapeaux : position.
    pub on_changed: Option<unsafe extern "C" fn(userdata: *mut c_void, state: *const KnPointerState, visibility: bool, shape: bool)>,
}

unsafe impl Send for KnPointerCallbacks {}
unsafe impl Sync for KnPointerCallbacks {}

/// La couche de pointeur d'une vue de session.
pub struct KnPointer {
    layer: Arc<PointerLayer>,
    clock: Arc<MonotonicClock>,
}

fn pointer_state(state: &kybernow_core::input::pointer_layer::State, now: Duration, pixels: *const u32) -> KnPointerState {
    let (has_position, x, y) = match state.position() {
        Some((x, y)) => (true, x, y),
        None => (false, 0.0, 0.0),
    };
    let shape = state.shape.as_ref();
    KnPointerState {
        has_position,
        x,
        y,
        remote_hidden: state.remote_hidden,
        visible_for_ms: state.visible_for(now).as_secs_f64() * 1000.0,
        has_shape: shape.is_some(),
        width: shape.map_or(0, |s| s.width),
        height: shape.map_or(0, |s| s.height),
        hot_x: shape.map_or(0, |s| s.hot_x),
        hot_y: shape.map_or(0, |s| s.hot_y),
        pixels,
    }
}

/// `holds_position` : l'appareil tient une position à lui (Apple TV) ; le Mac non.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pointer_new(holds_position: bool, callbacks: KnPointerCallbacks) -> *mut KnPointer {
    let clock = Arc::new(MonotonicClock::new());
    let layer = Arc::new(PointerLayer::new(clock.clone(), holds_position));
    let c = clock.clone();
    // Un `Arc` : la fermeture capture l'ensemble (Send), pas ses champs.
    let callbacks = Arc::new(callbacks);
    layer.set_on_changed(move |state, change| {
        let Some(f) = callbacks.on_changed else { return };
        let premultiplied = if change.shape { state.shape.as_ref().map(|s| s.premultiplied()) } else { None };
        let pixels = premultiplied.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
        let s = pointer_state(state, c.now(), pixels);
        unsafe { f(callbacks.userdata, &s, change.visibility, change.shape) };
    });
    Box::into_raw(Box::new(KnPointer { layer, clock }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pointer_free(p: *mut KnPointer) {
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p) });
    }
}

/// État en ce moment (sans les pixels de la forme).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pointer_state(p: *const KnPointer, out: *mut KnPointerState) {
    let (Some(p), Some(out)) = (unsafe { p.as_ref() }, unsafe { out.as_mut() }) else { return };
    *out = pointer_state(&p.layer.state(), p.clock.now(), std::ptr::null());
}

/// L'écran de l'ordinateur a changé de taille en cours de session.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pointer_set_host_size(p: *const KnPointer, width: f64, height: f64) {
    if let Some(p) = unsafe { p.as_ref() } {
        p.layer.set_host_size((width, height));
    }
}

/// Position tenue par l'appareil déplacée (Apple TV : télécommande, manette).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pointer_move_by(p: *const KnPointer, dx: f64, dy: f64) {
    if let Some(p) = unsafe { p.as_ref() } {
        p.layer.move_by(dx, dy);
    }
}

/// Fin de session : plus de forme ni de position.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pointer_reset(p: *const KnPointer) {
    if let Some(p) = unsafe { p.as_ref() } {
        p.layer.reset();
    }
}

/// Mode de la souris (`KN_MOUSE_*`).
pub const KN_MOUSE_AUTO: u32 = 0;
pub const KN_MOUSE_DESKTOP: u32 = 1;
pub const KN_MOUSE_GAME: u32 = 2;

/// Délais des règles Auto : pointeur remontré toléré, recontrôle.
pub const KN_MOUSE_SHOWN_TOLERANCE_MS: u32 = 600;
pub const KN_MOUSE_RECHECK_DELAY_MS: u32 = 700;

/// La souris doit-elle être relative (règles Auto / Bureau / Jeu).
/// `needs_hidden_pointer` : Mac et Windows (en Auto, le jeu doit aussi cacher
/// son pointeur) ; Apple TV : le plein écran suffit.
#[unsafe(no_mangle)]
pub extern "C" fn kn_mouse_wants_relative(
    needs_hidden_pointer: bool,
    mode: u32,
    game_fullscreen: bool,
    remote_hidden: bool,
    visible_for_ms: f64,
    relative_active: bool,
) -> bool {
    let mode = match mode {
        KN_MOUSE_DESKTOP => MouseMode::Desktop,
        KN_MOUSE_GAME => MouseMode::Game,
        _ => MouseMode::Auto,
    };
    let visible_for = Duration::from_secs_f64(visible_for_ms.max(0.0) / 1000.0);
    MouseModeRules { needs_hidden_pointer }.wants_relative(mode, game_fullscreen, remote_hidden, visible_for, relative_active)
}

/// Facteur d'échelle d'un pointeur de `width` × `height` pixels pour tenir
/// dans une case de `box_` points, au plus `max_factor`.
#[unsafe(no_mangle)]
pub extern "C" fn kn_pointer_scale(width: u32, height: u32, box_: f64, max_factor: f64) -> f64 {
    PointerLayer::scale((width, height), box_, max_factor)
}

// MARK: - Clavier

/// Scancode à envoyer pour une touche donnée en usage HID (page 7), après
/// « ⌘ envoyée comme Ctrl » et l'échange des claviers Apple ISO ; -1 :
/// touche inconnue de kynput. `iso` n'est lu que pour les deux touches concernées.
#[unsafe(no_mangle)]
pub extern "C" fn kn_keymap_scancode(hid: u16, command_as_control: bool, iso: bool) -> i32 {
    keymap::scancode(hid, command_as_control, || iso).map_or(-1, i32::from)
}

/// Verr. maj. ou Verr. num. : une bascule, envoyée en appui + relâchement.
#[unsafe(no_mangle)]
pub extern "C" fn kn_keymap_is_lock(hid: u16) -> bool {
    keymap::is_lock(hid)
}

/// Les deux touches échangées par les claviers Apple ISO (la disposition
/// n'est à demander que pour elles).
#[unsafe(no_mangle)]
pub extern "C" fn kn_keymap_needs_iso(hid: u16) -> bool {
    hid == keymap::GRAVE || hid == keymap::ISO_SECTION
}

/// Clavier Apple, ou de fabricant inconnu (réglage « Clavier Apple ISO » de l'Apple TV).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_keymap_apple_keyboard(vendor_name: *const c_char) -> bool {
    let name = text(vendor_name);
    keymap::apple_keyboard((!name.is_empty()).then_some(name.as_str()))
}

// MARK: - Chaîne d'entrées

#[repr(C)]
pub struct KnInputOptions {
    /// Écran de l'ordinateur affiché et sa taille au départ de l'image.
    pub display_id: u32,
    pub host_width: u32,
    pub host_height: u32,
    /// Presse-papiers partagé (les rappels `pasteboard_*` doivent être donnés).
    pub clipboard: bool,
    /// Manettes produites par l'app (Apple TV, iOS) ; `false` : kynput les lit
    /// via SDL (Mac), avec la combinaison et la prise par la barre.
    pub gamepads_from_app: bool,
}

#[repr(C)]
pub struct KnInputCallbacks {
    pub userdata: *mut c_void,
    /// Combinaison L1 + R1 + Start + Select tenue 250 ms.
    pub on_combo: Option<unsafe extern "C" fn(userdata: *mut c_void, pad: u32)>,
    /// Touche de navigation d'une manette prise (`KN_PAD_KEY_*`).
    pub on_navigate: Option<unsafe extern "C" fn(userdata: *mut c_void, key: u32)>,
    /// Les entrées rejoignent l'ordinateur (début de session, reconnexion).
    pub on_connected: Option<unsafe extern "C" fn(userdata: *mut c_void)>,
    /// Vibration demandée par le jeu (manettes de l'app).
    pub on_rumble: Option<unsafe extern "C" fn(userdata: *mut c_void, pad: u32, large: u16, small: u16, duration_ms: i32)>,
    /// Presse-papiers de l'appareil : compteur de changements, texte (chaîne
    /// allouée par `malloc`, libérée par le cœur ; nulle : pas de texte), écriture.
    pub pasteboard_change_count: Option<unsafe extern "C" fn(userdata: *mut c_void) -> u64>,
    pub pasteboard_text: Option<unsafe extern "C" fn(userdata: *mut c_void) -> *mut c_char>,
    pub pasteboard_set_text: Option<unsafe extern "C" fn(userdata: *mut c_void, text: *const c_char)>,
    /// Du texte est là, sans le lire (facultatif : sinon `pasteboard_text`).
    pub pasteboard_has_text: Option<unsafe extern "C" fn(userdata: *mut c_void) -> bool>,
    /// Lire le texte demande l'accord de l'utilisateur (iOS) : il n'est lu
    /// que quand l'ordinateur le demande.
    pub pasteboard_reading_asks_user: bool,
}

unsafe impl Send for KnInputCallbacks {}
unsafe impl Sync for KnInputCallbacks {}

/// Touches de navigation d'une manette prise par l'app.
pub const KN_PAD_KEY_LEFT: u32 = 0;
pub const KN_PAD_KEY_RIGHT: u32 = 1;
pub const KN_PAD_KEY_UP: u32 = 2;
pub const KN_PAD_KEY_DOWN: u32 = 3;
pub const KN_PAD_KEY_ACCEPT: u32 = 4;
pub const KN_PAD_KEY_BACK: u32 = 5;

/// Boutons de souris (`kn_input_mouse_button`).
pub const KN_MOUSE_LEFT: u32 = 0;
pub const KN_MOUSE_RIGHT: u32 = 1;
pub const KN_MOUSE_MIDDLE: u32 = 2;
pub const KN_MOUSE_SIDE: u32 = 3;
pub const KN_MOUSE_EXTRA: u32 = 4;

struct Callbacks(KnInputCallbacks);

impl InputPlatform for Callbacks {
    fn rumble(&self, r: GamepadRumble) {
        if let Some(f) = self.0.on_rumble {
            unsafe { f(self.0.userdata, r.id, r.large_motor, r.small_motor, r.duration_ms) };
        }
    }

    fn combo_formed(&self, pad: PadId) {
        if let Some(f) = self.0.on_combo {
            unsafe { f(self.0.userdata, pad) };
        }
    }

    fn connected(&self) {
        if let Some(f) = self.0.on_connected {
            unsafe { f(self.0.userdata) };
        }
    }

    fn navigate(&self, key: PadKey) {
        let Some(f) = self.0.on_navigate else { return };
        let k = match key {
            PadKey::Left => KN_PAD_KEY_LEFT,
            PadKey::Right => KN_PAD_KEY_RIGHT,
            PadKey::Up => KN_PAD_KEY_UP,
            PadKey::Down => KN_PAD_KEY_DOWN,
            PadKey::Accept => KN_PAD_KEY_ACCEPT,
            PadKey::Back => KN_PAD_KEY_BACK,
        };
        unsafe { f(self.0.userdata, k) };
    }
}

struct Board(Arc<Callbacks>);

impl Pasteboard for Board {
    fn change_count(&self) -> u64 {
        self.0 .0.pasteboard_change_count.map_or(0, |f| unsafe { f(self.0 .0.userdata) })
    }

    fn text(&self) -> Option<String> {
        let f = self.0 .0.pasteboard_text?;
        let p = unsafe { f(self.0 .0.userdata) };
        if p.is_null() {
            return None;
        }
        let s = text(p);
        unsafe { free(p.cast()) };
        Some(s)
    }

    fn set_text(&self, text: &str) {
        let Some(f) = self.0 .0.pasteboard_set_text else { return };
        let Ok(c) = std::ffi::CString::new(text.replace('\0', " ")) else { return };
        unsafe { f(self.0 .0.userdata, c.as_ptr()) };
    }

    fn has_text(&self) -> bool {
        match self.0 .0.pasteboard_has_text {
            Some(f) => unsafe { f(self.0 .0.userdata) },
            None => self.text().is_some(),
        }
    }

    fn reading_asks_user(&self) -> bool {
        self.0 .0.pasteboard_reading_asks_user
    }
}

/// La chaîne d'entrées d'une image.
pub struct KnInput {
    pipeline: Arc<InputPipeline>,
}

/// Crée la chaîne d'entrées de l'image qui démarre, sur la couche de
/// pointeur de la vue. Un fil du cœur tient ses minuteries (réessais, axes,
/// combinaison, presse-papiers) jusqu'à `kn_input_stop`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_new(options: KnInputOptions, pointer: *const KnPointer, callbacks: KnInputCallbacks) -> *mut KnInput {
    let Some(pointer) = (unsafe { pointer.as_ref() }) else { return std::ptr::null_mut() };
    let callbacks = Arc::new(Callbacks(callbacks));
    let has_board = callbacks.0.pasteboard_change_count.is_some() && callbacks.0.pasteboard_text.is_some() && callbacks.0.pasteboard_set_text.is_some();
    let clipboard: Option<Arc<dyn Pasteboard>> = (options.clipboard && has_board).then(|| Arc::new(Board(callbacks.clone())) as Arc<dyn Pasteboard>);
    let clock: Arc<dyn Clock> = pointer.clock.clone();
    let platform: Arc<dyn InputPlatform> = callbacks;
    let pipeline = InputPipeline::new(
        Options {
            display_id: options.display_id,
            host_size: (options.host_width, options.host_height),
            clipboard,
            gamepads_from_app: options.gamepads_from_app,
        },
        pointer.layer.clone(),
        platform,
        clock,
    );
    if !options.gamepads_from_app {
        pipeline.install_gamepad_gate();
    }
    if let Err(e) = pipeline.spawn_driver() {
        log::error!("entrées : fil des minuteries impossible ({e})");
    }
    Box::into_raw(Box::new(KnInput { pipeline }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_free(i: *mut KnInput) {
    if !i.is_null() {
        let i = unsafe { Box::from_raw(i) };
        // Le fil des minuteries s'arrête avec la chaîne.
        i.pipeline.wake();
    }
}

fn with(i: *const KnInput, f: impl FnOnce(&Arc<InputPipeline>)) {
    if let Some(i) = unsafe { i.as_ref() } {
        f(&i.pipeline);
    }
}

/// La chaîne kynput, avec une référence prise, à rendre par `input_pipeline`
/// de `KnPlatform` (la session la passe à Kyber).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_kynput(i: *const KnInput) -> *mut c_void {
    match unsafe { i.as_ref() } {
        Some(i) => i.pipeline.kynput().to_ptr().cast(),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_key(i: *const KnInput, scancode: u16, pressed: bool) {
    with(i, |p| p.key(scancode, pressed));
}

/// Position absolue sur l'écran de l'ordinateur affiché, en pixels.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_mouse(i: *const KnInput, x: i16, y: i16) {
    with(i, |p| p.mouse(x, y));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_mouse_move(i: *const KnInput, dx: i16, dy: i16) {
    with(i, |p| p.mouse_move(dx, dy));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_mouse_button(i: *const KnInput, button: u32, pressed: bool) {
    let button = match button {
        KN_MOUSE_RIGHT => MouseButtonType::Right,
        KN_MOUSE_MIDDLE => MouseButtonType::Middle,
        KN_MOUSE_SIDE => MouseButtonType::Side,
        KN_MOUSE_EXTRA => MouseButtonType::Extra,
        _ => MouseButtonType::Left,
    };
    with(i, |p| p.mouse_button(button, pressed));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_wheel(i: *const KnInput, dx: f32, dy: f32) {
    with(i, |p| p.wheel(dx, dy));
}

/// Texte tapé (clavier de l'écran), un caractère à la fois.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_type_text(i: *const KnInput, s: *const c_char) {
    let s = text(s);
    with(i, |p| p.type_text(&s));
}

/// L'ordinateur relâche tout ce qu'il tient de nous, manettes comprises.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_flush_keys(i: *const KnInput) {
    with(i, |p| p.flush_keys());
}

/// Relâche les touches et clics que nous tenons (la fenêtre perd le clavier).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_release_all(i: *const KnInput) {
    with(i, |p| p.release_all());
}

/// Touche de modification de la plateforme (code kVK sur Mac) tenue ou
/// relâchée ; `false` : déjà dans cet état (rien à envoyer).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_modifier(i: *const KnInput, id: u16, pressed: bool) -> bool {
    let mut changed = false;
    with(i, |p| changed = p.held().lock().unwrap().modifier(id, pressed));
    changed
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_modifier_held(i: *const KnInput, id: u16) -> bool {
    let mut held = false;
    with(i, |p| held = p.held().lock().unwrap().modifiers().contains(&id));
    held
}

/// Modificateurs tenus chez nous (codes de la plateforme), au plus `cap` dans `out`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_held_modifiers(i: *const KnInput, out: *mut u16, cap: usize) -> usize {
    let mut n = 0;
    with(i, |p| {
        for id in p.held().lock().unwrap().modifiers() {
            if n == cap || out.is_null() {
                break;
            }
            unsafe { *out.add(n) = *id };
            n += 1;
        }
    });
    n
}

/// Les modificateurs repartent de zéro (service d'entrées neuf sur l'ordinateur).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_forget_modifiers(i: *const KnInput) {
    with(i, |p| p.held().lock().unwrap().forget_modifiers());
}

/// La fenêtre retrouve le clavier : `held` (codes de la plateforme) sont les
/// modificateurs réellement tenus ; écrit dans `out` (au plus `cap`) ceux à
/// renvoyer appuyés, et les note tenus.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_resync_modifiers(i: *const KnInput, held: *const u16, count: usize, out: *mut u16, cap: usize) -> usize {
    let held = if held.is_null() || count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(held, count) } };
    let mut n = 0;
    with(i, |p| {
        p.held().lock().unwrap().resync_modifiers(held, |id| {
            if n < cap && !out.is_null() {
                unsafe { *out.add(n) = id };
                n += 1;
            }
        });
    });
    n
}

/// Souris relative (jeux) : seuls les déplacements partent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_set_relative(i: *const KnInput, on: bool) {
    with(i, |p| p.set_relative(on));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_is_relative(i: *const KnInput) -> bool {
    let mut on = false;
    with(i, |p| on = p.is_relative());
    on
}

/// Une fois la chaîne branchée au réseau.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_start_clipboard(i: *const KnInput) {
    with(i, |p| p.start_clipboard());
}

/// Les entrées rejoignent l'ordinateur (début de session, reconnexion).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_network_connected(i: *const KnInput) {
    with(i, |p| p.network_connected());
}

/// Les manettes SDL prises par l'app (barre ouverte) ou rendues.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_set_gamepad_captured(i: *const KnInput, captured: bool) {
    with(i, |p| p.set_gamepad_captured(captured));
}

/// Fin de l'image : plus rien ne part, l'ordinateur relâche tout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_stop(i: *const KnInput) {
    with(i, |p| p.stop());
}

/// Arrête les gestionnaires de kynput (manettes SDL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_shutdown(i: *const KnInput) {
    with(i, |p| p.shutdown());
}

// MARK: - Manettes produites par l'app (Apple TV, iOS)

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_gamepad_plug(i: *const KnInput, pad: u32, vendor_id: i32, product_id: i32) {
    let v = u16::try_from(vendor_id).ok();
    let pr = u16::try_from(product_id).ok();
    with(i, |p| p.gamepad_plug(pad, v, pr));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_gamepad_unplug(i: *const KnInput, pad: u32) {
    with(i, |p| p.gamepad_unplug(pad));
}

/// `button` : valeur kynput (`KYNPUT_GAMEPAD_BUTTON_*`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_gamepad_button(i: *const KnInput, pad: u32, button: u32, pressed: bool) {
    let Some(b) = PadButton::from_u32(button) else { return };
    with(i, |p| p.gamepad_button(pad, b, pressed));
}

/// `axis` : valeur kynput (`KYNPUT_GAMEPAD_AXIS_*`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_gamepad_axis(i: *const KnInput, pad: u32, axis: u32, value: i16) {
    let Some(a) = PadAxis::from_u32(axis) else { return };
    with(i, |p| p.gamepad_axis(pad, a, value));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_gamepad_axes_rest(i: *const KnInput, pad: u32) {
    with(i, |p| p.gamepad_axes_rest(pad));
}

/// Réglage « la combinaison ouvre le menu » (Apple TV).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_input_set_combo_enabled(i: *const KnInput, enabled: bool) {
    with(i, |p| p.combo().set_enabled(enabled));
}

// MARK: - Combinaison seule (Apple TV, iOS)

/// La combinaison L1 + R1 + Start + Select d'un pont de manettes qui la
/// suit lui-même avant d'envoyer quoi que ce soit (GameController : barre ou
/// menu, souris à la manette, appui long) : la même règle que la chaîne
/// d'entrées (250 ms tenues), avec son propre fil pour l'échéance.
pub struct KnCombo {
    combo: Arc<PadCombo>,
    driver: Arc<ComboDriver>,
}

struct ComboDriver {
    /// (réveillé, arrêt demandé)
    woken: std::sync::Mutex<(bool, bool)>,
    cvar: std::sync::Condvar,
}

impl ComboDriver {
    fn wake(&self, stop: bool) {
        let mut w = self.woken.lock().unwrap();
        w.0 = true;
        w.1 |= stop;
        self.cvar.notify_one();
    }
}

#[repr(C)]
pub struct KnComboCallbacks {
    pub userdata: *mut c_void,
    /// Combinaison formée pour cette manette (fil de la combinaison).
    pub on_formed: Option<unsafe extern "C" fn(userdata: *mut c_void, pad: u32)>,
}

unsafe impl Send for KnComboCallbacks {}
unsafe impl Sync for KnComboCallbacks {}

#[unsafe(no_mangle)]
pub extern "C" fn kn_combo_new(callbacks: KnComboCallbacks) -> *mut KnCombo {
    let clock: Arc<dyn Clock> = Arc::new(MonotonicClock::new());
    let combo = Arc::new(PadCombo::new(clock.clone()));
    let callbacks = Arc::new(callbacks);
    combo.set_on_formed(move |pad| {
        if let Some(f) = callbacks.on_formed {
            unsafe { f(callbacks.userdata, pad) };
        }
    });
    let driver = Arc::new(ComboDriver {
        woken: std::sync::Mutex::new((false, false)),
        cvar: std::sync::Condvar::new(),
    });
    let (c, d) = (Arc::downgrade(&combo), driver.clone());
    let spawned = std::thread::Builder::new().name("kybernow-combinaison".into()).spawn(move || {
        loop {
            let Some(combo) = c.upgrade() else { break };
            combo.poll();
            let wait = combo.next_deadline().map(|at| at.saturating_sub(clock.now()));
            drop(combo);
            let mut w = d.woken.lock().unwrap();
            if !w.0 {
                w = match wait {
                    Some(t) => d.cvar.wait_timeout(w, t).unwrap().0,
                    None => d.cvar.wait(w).unwrap(),
                };
            }
            if w.1 {
                break;
            }
            w.0 = false;
        }
    });
    if let Err(e) = spawned {
        log::error!("combinaison : fil impossible ({e})");
    }
    Box::into_raw(Box::new(KnCombo { combo, driver }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_combo_free(c: *mut KnCombo) {
    if !c.is_null() {
        let c = unsafe { Box::from_raw(c) };
        c.driver.wake(true);
    }
}

fn combo<'a>(c: *const KnCombo) -> Option<&'a KnCombo> {
    unsafe { c.as_ref() }
}

/// Réglage « la combinaison ouvre le menu » ; `false` : rien n'est armé.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_combo_set_enabled(c: *const KnCombo, enabled: bool) {
    if let Some(c) = combo(c) {
        c.combo.set_enabled(enabled);
    }
}

/// Touche d'une manette (`button` : valeur kynput).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_combo_button(c: *const KnCombo, pad: u32, button: u32, pressed: bool) {
    let (Some(c), Some(b)) = (combo(c), PadButton::from_u32(button)) else { return };
    c.combo.button(pad, b, pressed);
    if c.combo.next_deadline().is_some() {
        c.driver.wake(false);
    }
}

/// Touches tenues de cette manette (valeurs kynput), au plus `cap` dans `out`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_combo_pressed(c: *const KnCombo, pad: u32, out: *mut u32, cap: usize) -> usize {
    let Some(c) = combo(c) else { return 0 };
    let mut n = 0;
    for b in c.combo.pressed(pad) {
        if n == cap || out.is_null() {
            break;
        }
        unsafe { *out.add(n) = b.to_u32() };
        n += 1;
    }
    n
}

/// Les touches tenues d'une manette, d'après son état réel (resynchronisation).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_combo_set_pressed(c: *const KnCombo, pad: u32, buttons: *const u32, count: usize) {
    let Some(c) = combo(c) else { return };
    let list = if buttons.is_null() || count == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(buttons, count) } };
    c.combo.set_pressed(pad, list.iter().filter_map(|b| PadButton::from_u32(*b)).collect());
}

/// La combinaison s'est formée et ses touches ne sont pas toutes relâchées.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_combo_formed(c: *const KnCombo, pad: u32) -> bool {
    combo(c).is_some_and(|c| c.combo.formed(pad))
}

/// Manette débranchée.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_combo_forget(c: *const KnCombo, pad: u32) {
    if let Some(c) = combo(c) {
        c.combo.forget(pad);
    }
}

/// Position tenue renvoyée à l'ordinateur (entrées reliées de nouveau).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pointer_resend_held(p: *const KnPointer) {
    if let Some(p) = unsafe { p.as_ref() } {
        p.layer.resend_held();
    }
}
