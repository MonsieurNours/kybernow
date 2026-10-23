//! Les fonctions de SDL2 dont l'app se sert (celle du runtime de freedesktop),
//! déclarées à la main : fenêtre et rendu, textures YUV, événements du
//! clavier, de la souris et des manettes, curseur, presse-papiers, son.

#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_int, c_void, CStr, CString};

pub const SDL_INIT_AUDIO: u32 = 0x10;
pub const SDL_INIT_VIDEO: u32 = 0x20;
pub const SDL_INIT_JOYSTICK: u32 = 0x200;
pub const SDL_INIT_HAPTIC: u32 = 0x1000;
pub const SDL_INIT_GAMECONTROLLER: u32 = 0x2000;
pub const SDL_INIT_EVENTS: u32 = 0x4000;

pub const SDL_WINDOW_FULLSCREEN: u32 = 0x1;
pub const SDL_WINDOW_SHOWN: u32 = 0x4;
pub const SDL_WINDOW_BORDERLESS: u32 = 0x10;
pub const SDL_WINDOW_RESIZABLE: u32 = 0x20;
pub const SDL_WINDOW_MINIMIZED: u32 = 0x40;
pub const SDL_WINDOW_MAXIMIZED: u32 = 0x80;
pub const SDL_WINDOW_INPUT_FOCUS: u32 = 0x200;
pub const SDL_WINDOW_FULLSCREEN_DESKTOP: u32 = 0x1001;
pub const SDL_WINDOW_ALLOW_HIGHDPI: u32 = 0x2000;
pub const SDL_WINDOWPOS_CENTERED: c_int = 0x2FFF0000;
pub const SDL_RENDERER_ACCELERATED: u32 = 0x2;
pub const SDL_RENDERER_PRESENTVSYNC: u32 = 0x4;
/// Octets R, G, B, A en mémoire (comme tiny-skia).
pub const SDL_PIXELFORMAT_ABGR8888: u32 = 0x16762004;
pub const SDL_PIXELFORMAT_ARGB8888: u32 = 0x16362004;
pub const SDL_PIXELFORMAT_NV12: u32 = 0x3231564E;
pub const SDL_PIXELFORMAT_IYUV: u32 = 0x56555949;
pub const SDL_TEXTUREACCESS_STREAMING: c_int = 1;
pub const SDL_BLENDMODE_NONE: c_int = 0;
pub const SDL_BLENDMODE_BLEND: c_int = 1;
pub const AUDIO_F32SYS: u16 = 0x8120;
pub const SDL_YUV_CONVERSION_JPEG: c_int = 0;
pub const SDL_YUV_CONVERSION_BT601: c_int = 1;
pub const SDL_YUV_CONVERSION_BT709: c_int = 2;

pub const SDL_QUIT: u32 = 0x100;
pub const SDL_APP_WILLENTERBACKGROUND: u32 = 0x103;
pub const SDL_WINDOWEVENT: u32 = 0x200;
pub const SDL_KEYDOWN: u32 = 0x300;
pub const SDL_KEYUP: u32 = 0x301;
pub const SDL_TEXTINPUT: u32 = 0x303;
pub const SDL_MOUSEMOTION: u32 = 0x400;
pub const SDL_MOUSEBUTTONDOWN: u32 = 0x401;
pub const SDL_MOUSEBUTTONUP: u32 = 0x402;
pub const SDL_MOUSEWHEEL: u32 = 0x403;
pub const SDL_CONTROLLERAXISMOTION: u32 = 0x650;
pub const SDL_CONTROLLERBUTTONDOWN: u32 = 0x651;
pub const SDL_CONTROLLERBUTTONUP: u32 = 0x652;
pub const SDL_CONTROLLERDEVICEADDED: u32 = 0x653;
pub const SDL_CONTROLLERDEVICEREMOVED: u32 = 0x654;
pub const SDL_FINGERDOWN: u32 = 0x700;
pub const SDL_CLIPBOARDUPDATE: u32 = 0x900;
pub const SDL_AUDIODEVICEADDED: u32 = 0x1100;
pub const SDL_RENDER_TARGETS_RESET: u32 = 0x2000;
pub const SDL_RENDER_DEVICE_RESET: u32 = 0x2001;
pub const SDL_USEREVENT: u32 = 0x8000;

pub const SDL_WINDOWEVENT_SIZE_CHANGED: u8 = 6;
pub const SDL_WINDOWEVENT_MAXIMIZED: u8 = 8;
pub const SDL_WINDOWEVENT_RESTORED: u8 = 9;
pub const SDL_WINDOWEVENT_LEAVE: u8 = 11;
pub const SDL_WINDOWEVENT_FOCUS_GAINED: u8 = 12;
pub const SDL_WINDOWEVENT_FOCUS_LOST: u8 = 13;
pub const SDL_WINDOWEVENT_DISPLAY_CHANGED: u8 = 16;

/// Modificateurs (`SDL_Keymod`).
pub const KMOD_SHIFT: u16 = 0x0003;
pub const KMOD_CTRL: u16 = 0x00C0;
pub const KMOD_ALT: u16 = 0x0300;
pub const KMOD_GUI: u16 = 0x0C00;

pub const SC_A: i32 = 4;
pub const SC_D: i32 = 7;
pub const SC_F: i32 = 9;
pub const SC_M: i32 = 16;
pub const SC_RETURN: i32 = 40;
pub const SC_ESCAPE: i32 = 41;
pub const SC_BACKSPACE: i32 = 42;
pub const SC_TAB: i32 = 43;
pub const SC_F11: i32 = 68;
pub const SC_RIGHT: i32 = 79;
pub const SC_LEFT: i32 = 80;
pub const SC_DOWN: i32 = 81;
pub const SC_UP: i32 = 82;
pub const SC_KP_ENTER: i32 = 88;
pub const SC_AC_BACK: i32 = 270;

/// Réponse du test de la fenêtre sans bordure (`SDL_HitTestResult`).
pub const HITTEST_NORMAL: c_int = 0;
pub const HITTEST_DRAGGABLE: c_int = 1;
pub const HITTEST_RESIZE_TOPLEFT: c_int = 2;
pub const HITTEST_RESIZE_TOP: c_int = 3;
pub const HITTEST_RESIZE_TOPRIGHT: c_int = 4;
pub const HITTEST_RESIZE_RIGHT: c_int = 5;
pub const HITTEST_RESIZE_BOTTOMRIGHT: c_int = 6;
pub const HITTEST_RESIZE_BOTTOM: c_int = 7;
pub const HITTEST_RESIZE_BOTTOMLEFT: c_int = 8;
pub const HITTEST_RESIZE_LEFT: c_int = 9;

pub enum SDL_Window {}
pub enum SDL_Renderer {}
pub enum SDL_Texture {}
pub enum SDL_GameController {}
pub enum SDL_Joystick {}
pub enum SDL_Surface {}
pub enum SDL_Cursor {}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SDL_Rect {
    pub x: c_int,
    pub y: c_int,
    pub w: c_int,
    pub h: c_int,
}

#[repr(C)]
pub struct SDL_Point {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
#[derive(Default)]
pub struct SDL_DisplayMode {
    pub format: u32,
    pub w: c_int,
    pub h: c_int,
    pub refresh_rate: c_int,
    pub driverdata: usize,
}

#[repr(C)]
pub struct SDL_RendererInfo {
    pub name: *const c_char,
    pub flags: u32,
    pub num_texture_formats: u32,
    pub texture_formats: [u32; 16],
    pub max_texture_width: c_int,
    pub max_texture_height: c_int,
}

/// `SDL_Event` : 56 octets ; les champs sont lus par décalage.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct SDL_Event {
    pub raw: [u8; 56],
}

impl Default for SDL_Event {
    fn default() -> Self {
        Self { raw: [0; 56] }
    }
}

impl SDL_Event {
    pub fn kind(&self) -> u32 {
        self.u32(0)
    }
    pub fn u32(&self, at: usize) -> u32 {
        u32::from_ne_bytes(self.raw[at..at + 4].try_into().unwrap())
    }
    pub fn i32(&self, at: usize) -> i32 {
        i32::from_ne_bytes(self.raw[at..at + 4].try_into().unwrap())
    }
    pub fn i16(&self, at: usize) -> i16 {
        i16::from_ne_bytes(self.raw[at..at + 2].try_into().unwrap())
    }
    pub fn u16(&self, at: usize) -> u16 {
        u16::from_ne_bytes(self.raw[at..at + 2].try_into().unwrap())
    }
    pub fn u8(&self, at: usize) -> u8 {
        self.raw[at]
    }
    pub fn f32(&self, at: usize) -> f32 {
        f32::from_ne_bytes(self.raw[at..at + 4].try_into().unwrap())
    }
    /// Clavier : scancode, répétition, modificateurs.
    pub fn key(&self) -> (i32, bool, u16) {
        (self.i32(16), self.u8(13) != 0, self.u16(24))
    }
    /// Texte tapé (UTF-8, 32 octets au plus).
    pub fn text(&self) -> String {
        let bytes = &self.raw[12..44];
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(32);
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
    /// Souris : x, y, déplacements relatifs ; `which` (écran tactile : 0xFFFFFFFF).
    pub fn motion(&self) -> (i32, i32, i32, i32, u32) {
        (self.i32(20), self.i32(24), self.i32(28), self.i32(32), self.u32(12))
    }
    /// Bouton de souris : bouton (1 gauche, 2 milieu, 3 droit, 4 et 5 latéraux), x, y, `which`.
    pub fn button(&self) -> (u8, i32, i32, u32) {
        (self.u8(16), self.i32(20), self.i32(24), self.u32(12))
    }
    /// Molette : crans x, y (précis, SDL 2.0.18 et après).
    pub fn wheel(&self) -> (f32, f32) {
        let (px, py) = (self.f32(28), self.f32(32));
        if px != 0.0 || py != 0.0 { (px, py) } else { (self.i32(16) as f32, self.i32(20) as f32) }
    }
    /// Manette : identifiant (instance, ou index pour un ajout), bouton ou axe, valeur d'axe.
    pub fn pad(&self) -> (i32, u8, i16) {
        (self.i32(8), self.u8(12), self.i16(16))
    }
    /// Fenêtre : l'événement et ses deux valeurs.
    pub fn window(&self) -> (u8, i32, i32) {
        (self.u8(12), self.i32(16), self.i32(20))
    }
}

pub type AudioCallback = extern "C" fn(userdata: *mut c_void, stream: *mut u8, len: c_int);
pub type HitTest = extern "C" fn(win: *mut SDL_Window, area: *const SDL_Point, data: *mut c_void) -> c_int;

#[repr(C)]
pub struct SDL_AudioSpec {
    pub freq: c_int,
    pub format: u16,
    pub channels: u8,
    pub silence: u8,
    pub samples: u16,
    pub padding: u16,
    pub size: u32,
    pub callback: Option<AudioCallback>,
    pub userdata: *mut c_void,
}

impl SDL_AudioSpec {
    pub fn empty() -> Self {
        Self { freq: 0, format: 0, channels: 0, silence: 0, samples: 0, padding: 0, size: 0, callback: None, userdata: std::ptr::null_mut() }
    }
}

#[repr(C)]
pub struct SDL_Locale {
    pub language: *const c_char,
    pub country: *const c_char,
}

unsafe extern "C" {
    pub fn SDL_Init(flags: u32) -> c_int;
    pub fn SDL_InitSubSystem(flags: u32) -> c_int;
    pub fn SDL_Quit();
    pub fn SDL_GetError() -> *const c_char;
    pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> c_int;
    pub fn SDL_GetCurrentVideoDriver() -> *const c_char;
    pub fn SDL_GetCurrentAudioDriver() -> *const c_char;
    pub fn SDL_CreateWindow(title: *const c_char, x: c_int, y: c_int, w: c_int, h: c_int, flags: u32) -> *mut SDL_Window;
    pub fn SDL_GetWindowSize(w: *mut SDL_Window, width: *mut c_int, height: *mut c_int);
    pub fn SDL_SetWindowMinimumSize(w: *mut SDL_Window, width: c_int, height: c_int);
    pub fn SDL_GetWindowFlags(w: *mut SDL_Window) -> u32;
    pub fn SDL_SetWindowFullscreen(w: *mut SDL_Window, flags: u32) -> c_int;
    pub fn SDL_MinimizeWindow(w: *mut SDL_Window);
    pub fn SDL_MaximizeWindow(w: *mut SDL_Window);
    pub fn SDL_RestoreWindow(w: *mut SDL_Window);
    pub fn SDL_RaiseWindow(w: *mut SDL_Window);
    pub fn SDL_SetWindowHitTest(w: *mut SDL_Window, cb: Option<HitTest>, data: *mut c_void) -> c_int;
    pub fn SDL_SetWindowKeyboardGrab(w: *mut SDL_Window, grabbed: c_int);
    pub fn SDL_SetWindowMouseGrab(w: *mut SDL_Window, grabbed: c_int);
    pub fn SDL_GetWindowDisplayIndex(w: *mut SDL_Window) -> c_int;
    pub fn SDL_GetCurrentDisplayMode(index: c_int, mode: *mut SDL_DisplayMode) -> c_int;
    pub fn SDL_GetDisplayName(index: c_int) -> *const c_char;
    pub fn SDL_CreateRenderer(w: *mut SDL_Window, index: c_int, flags: u32) -> *mut SDL_Renderer;
    pub fn SDL_GetRendererInfo(r: *mut SDL_Renderer, info: *mut SDL_RendererInfo) -> c_int;
    pub fn SDL_GetRendererOutputSize(r: *mut SDL_Renderer, w: *mut c_int, h: *mut c_int) -> c_int;
    pub fn SDL_RenderSetVSync(r: *mut SDL_Renderer, vsync: c_int) -> c_int;
    pub fn SDL_CreateTexture(r: *mut SDL_Renderer, format: u32, access: c_int, w: c_int, h: c_int) -> *mut SDL_Texture;
    pub fn SDL_DestroyTexture(t: *mut SDL_Texture);
    pub fn SDL_UpdateTexture(t: *mut SDL_Texture, rect: *const SDL_Rect, pixels: *const c_void, pitch: c_int) -> c_int;
    pub fn SDL_UpdateNVTexture(t: *mut SDL_Texture, rect: *const SDL_Rect, y: *const u8, ypitch: c_int, uv: *const u8, uvpitch: c_int) -> c_int;
    pub fn SDL_UpdateYUVTexture(t: *mut SDL_Texture, rect: *const SDL_Rect, y: *const u8, ypitch: c_int, u: *const u8, upitch: c_int, v: *const u8, vpitch: c_int) -> c_int;
    pub fn SDL_SetTextureBlendMode(t: *mut SDL_Texture, mode: c_int) -> c_int;
    pub fn SDL_SetYUVConversionMode(mode: c_int);
    pub fn SDL_ComposeCustomBlendMode(src_color: c_int, dst_color: c_int, color_op: c_int, src_alpha: c_int, dst_alpha: c_int, alpha_op: c_int) -> c_int;
    pub fn SDL_SetRenderDrawColor(r: *mut SDL_Renderer, red: u8, g: u8, b: u8, a: u8) -> c_int;
    pub fn SDL_SetRenderDrawBlendMode(r: *mut SDL_Renderer, mode: c_int) -> c_int;
    pub fn SDL_RenderClear(r: *mut SDL_Renderer) -> c_int;
    pub fn SDL_RenderCopy(r: *mut SDL_Renderer, t: *mut SDL_Texture, src: *const SDL_Rect, dst: *const SDL_Rect) -> c_int;
    pub fn SDL_RenderPresent(r: *mut SDL_Renderer);
    pub fn SDL_WaitEventTimeout(e: *mut SDL_Event, timeout: c_int) -> c_int;
    pub fn SDL_PollEvent(e: *mut SDL_Event) -> c_int;
    pub fn SDL_PushEvent(e: *mut SDL_Event) -> c_int;
    pub fn SDL_RegisterEvents(n: c_int) -> u32;
    pub fn SDL_ShowCursor(toggle: c_int) -> c_int;
    pub fn SDL_SetRelativeMouseMode(on: c_int) -> c_int;
    pub fn SDL_CreateRGBSurfaceWithFormatFrom(pixels: *mut c_void, w: c_int, h: c_int, depth: c_int, pitch: c_int, format: u32) -> *mut SDL_Surface;
    pub fn SDL_FreeSurface(s: *mut SDL_Surface);
    pub fn SDL_CreateColorCursor(s: *mut SDL_Surface, hot_x: c_int, hot_y: c_int) -> *mut SDL_Cursor;
    pub fn SDL_SetCursor(c: *mut SDL_Cursor);
    pub fn SDL_FreeCursor(c: *mut SDL_Cursor);
    pub fn SDL_GetDefaultCursor() -> *mut SDL_Cursor;
    pub fn SDL_StartTextInput();
    pub fn SDL_StopTextInput();
    pub fn SDL_GetClipboardText() -> *mut c_char;
    pub fn SDL_SetClipboardText(text: *const c_char) -> c_int;
    pub fn SDL_HasClipboardText() -> c_int;
    pub fn SDL_GetPreferredLocales() -> *mut SDL_Locale;
    pub fn SDL_free(p: *mut c_void);
    pub fn SDL_GetTicks() -> u32;
    pub fn SDL_DisableScreenSaver();
    pub fn SDL_EnableScreenSaver();
    pub fn SDL_GetPowerInfo(seconds: *mut c_int, percent: *mut c_int) -> c_int;

    pub fn SDL_NumJoysticks() -> c_int;
    pub fn SDL_IsGameController(index: c_int) -> c_int;
    pub fn SDL_GameControllerOpen(index: c_int) -> *mut SDL_GameController;
    pub fn SDL_GameControllerClose(c: *mut SDL_GameController);
    pub fn SDL_GameControllerName(c: *mut SDL_GameController) -> *const c_char;
    pub fn SDL_GameControllerGetJoystick(c: *mut SDL_GameController) -> *mut SDL_Joystick;
    pub fn SDL_GameControllerGetVendor(c: *mut SDL_GameController) -> u16;
    pub fn SDL_GameControllerGetProduct(c: *mut SDL_GameController) -> u16;
    pub fn SDL_GameControllerRumble(c: *mut SDL_GameController, low: u16, high: u16, ms: u32) -> c_int;
    pub fn SDL_GameControllerRumbleTriggers(c: *mut SDL_GameController, left: u16, right: u16, ms: u32) -> c_int;
    pub fn SDL_GameControllerHasRumble(c: *mut SDL_GameController) -> c_int;
    pub fn SDL_GameControllerHasRumbleTriggers(c: *mut SDL_GameController) -> c_int;
    pub fn SDL_JoystickInstanceID(j: *mut SDL_Joystick) -> i32;
    pub fn SDL_JoystickCurrentPowerLevel(j: *mut SDL_Joystick) -> c_int;

    pub fn SDL_OpenAudioDevice(device: *const c_char, iscapture: c_int, desired: *const SDL_AudioSpec, obtained: *mut SDL_AudioSpec, allowed: c_int) -> u32;
    pub fn SDL_PauseAudioDevice(dev: u32, pause_on: c_int);
    pub fn SDL_CloseAudioDevice(dev: u32);
    pub fn SDL_GetDefaultAudioInfo(name: *mut *mut c_char, spec: *mut SDL_AudioSpec, iscapture: c_int) -> c_int;
}

pub const SDL_AUDIO_ALLOW_CHANNELS_CHANGE: c_int = 0x4;

pub fn error() -> String {
    unsafe { CStr::from_ptr(SDL_GetError()).to_string_lossy().into_owned() }
}

pub fn hint(name: &str, value: &str) {
    let (n, v) = (CString::new(name).unwrap(), CString::new(value).unwrap());
    unsafe { SDL_SetHint(n.as_ptr(), v.as_ptr()) };
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() } }
}

pub fn video_driver() -> String {
    cstr(unsafe { SDL_GetCurrentVideoDriver() })
}

pub fn audio_driver() -> String {
    cstr(unsafe { SDL_GetCurrentAudioDriver() })
}

/// La première langue préférée du système (`fr-FR`).
pub fn locale() -> String {
    unsafe {
        let l = SDL_GetPreferredLocales();
        if l.is_null() || (*l).language.is_null() {
            return std::env::var("LANG").unwrap_or_default();
        }
        let lang = CStr::from_ptr((*l).language).to_string_lossy().into_owned();
        let country = if (*l).country.is_null() { String::new() } else { format!("-{}", CStr::from_ptr((*l).country).to_string_lossy()) };
        SDL_free(l as *mut c_void);
        format!("{lang}{country}")
    }
}

/// Mode de l'écran de la fenêtre : définition, fréquence, nom.
pub fn display_mode(window: *mut SDL_Window) -> ((u32, u32), u32, String) {
    unsafe {
        let i = SDL_GetWindowDisplayIndex(window).max(0);
        let mut m = SDL_DisplayMode::default();
        let ok = SDL_GetCurrentDisplayMode(i, &mut m) == 0 && m.w > 0;
        let name = cstr(SDL_GetDisplayName(i));
        if ok { ((m.w as u32, m.h as u32), m.refresh_rate.max(30) as u32, name) } else { ((1920, 1080), 60, name) }
    }
}

/// La sortie son par défaut : nom et voies (SDL 2.24 et après).
pub fn default_audio() -> Option<(String, u32, u32)> {
    unsafe {
        let mut name: *mut c_char = std::ptr::null_mut();
        let mut spec = SDL_AudioSpec::empty();
        if SDL_GetDefaultAudioInfo(&mut name, &mut spec, 0) != 0 {
            return None;
        }
        let n = cstr(name);
        if !name.is_null() {
            SDL_free(name as *mut c_void);
        }
        Some((n, spec.channels as u32, spec.freq.max(0) as u32))
    }
}

pub fn clipboard() -> Option<String> {
    unsafe {
        if SDL_HasClipboardText() == 0 {
            return None;
        }
        let p = SDL_GetClipboardText();
        if p.is_null() {
            return None;
        }
        let s = CStr::from_ptr(p).to_string_lossy().into_owned();
        SDL_free(p as *mut c_void);
        (!s.is_empty()).then_some(s)
    }
}

pub fn set_clipboard(text: &str) {
    if let Ok(c) = CString::new(text.replace('\0', "")) {
        unsafe { SDL_SetClipboardText(c.as_ptr()) };
    }
}

/// Batterie : pourcentage, sur secteur (`None` sans batterie).
pub fn power() -> Option<(u32, bool)> {
    let (mut secs, mut pct) = (-1, -1);
    let state = unsafe { SDL_GetPowerInfo(&mut secs, &mut pct) };
    // 1 sur batterie, 2 pas de batterie, 3 en charge, 4 chargée.
    match state {
        1 => Some((pct.max(0) as u32, false)),
        3 | 4 => Some((pct.max(0) as u32, true)),
        _ => None,
    }
}

pub fn pad_name(c: *mut SDL_GameController) -> String {
    unsafe {
        let n = SDL_GameControllerName(c);
        if n.is_null() { "Manette".into() } else { CStr::from_ptr(n).to_string_lossy().into_owned() }
    }
}
