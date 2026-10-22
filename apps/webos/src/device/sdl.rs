//! Les fonctions de SDL2 dont l'app se sert (la version webOS de webosbrew,
//! embarquée dans le paquet), déclarées à la main : fenêtre et rendu, événements
//! de la télécommande, du clavier et des manettes, vibrations, son.

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
pub const SDL_WINDOWPOS_UNDEFINED: c_int = 0x1FFF0000;
pub const SDL_RENDERER_ACCELERATED: u32 = 0x2;
pub const SDL_RENDERER_PRESENTVSYNC: u32 = 0x4;
/// Octets R, G, B, A en mémoire (comme tiny-skia).
pub const SDL_PIXELFORMAT_ABGR8888: u32 = 0x16762004;
pub const SDL_PIXELFORMAT_ARGB8888: u32 = 0x16362004;
pub const SDL_TEXTUREACCESS_STREAMING: c_int = 1;
pub const SDL_BLENDMODE_NONE: c_int = 0;
pub const SDL_BLENDMODE_BLEND: c_int = 1;
pub const AUDIO_F32SYS: u16 = 0x8120;

pub const SDL_QUIT: u32 = 0x100;
pub const SDL_APP_WILLENTERBACKGROUND: u32 = 0x103;
pub const SDL_APP_DIDENTERFOREGROUND: u32 = 0x106;
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

pub const SDL_WINDOWEVENT_FOCUS_GAINED: u8 = 12;
pub const SDL_WINDOWEVENT_FOCUS_LOST: u8 = 13;

/// Touches de la Magic Remote (SDL-webOS, include/SDL_scancode.h).
pub const SC_WEBOS_HOME: i32 = 384;
pub const SC_WEBOS_BACK: i32 = 482;
pub const SC_WEBOS_CURSOR_SHOW: i32 = 484;
pub const SC_WEBOS_CURSOR_HIDE: i32 = 485;
pub const SC_WEBOS_RED: i32 = 486;
pub const SC_WEBOS_GREEN: i32 = 487;
pub const SC_WEBOS_YELLOW: i32 = 488;
pub const SC_WEBOS_BLUE: i32 = 489;
pub const SC_WEBOS_EXIT: i32 = 505;
pub const SC_RETURN: i32 = 40;
pub const SC_ESCAPE: i32 = 41;
pub const SC_BACKSPACE: i32 = 42;
pub const SC_RIGHT: i32 = 79;
pub const SC_LEFT: i32 = 80;
pub const SC_DOWN: i32 = 81;
pub const SC_UP: i32 = 82;
pub const SC_KP_ENTER: i32 = 88;
pub const SC_AC_BACK: i32 = 270;

pub enum SDL_Window {}
pub enum SDL_Renderer {}
pub enum SDL_Texture {}
pub enum SDL_GameController {}
pub enum SDL_Joystick {}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SDL_Rect {
    pub x: c_int,
    pub y: c_int,
    pub w: c_int,
    pub h: c_int,
}

/// `SDL_Event` : 56 octets ; les champs sont lus par décalage (`Event`).
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
    pub fn u8(&self, at: usize) -> u8 {
        self.raw[at]
    }
    pub fn f32(&self, at: usize) -> f32 {
        f32::from_ne_bytes(self.raw[at..at + 4].try_into().unwrap())
    }
    /// Clavier : scancode, répétition.
    pub fn key(&self) -> (i32, bool) {
        (self.i32(16), self.u8(13) != 0)
    }
    /// Texte tapé (UTF-8, 32 octets au plus).
    pub fn text(&self) -> String {
        let bytes = &self.raw[12..44];
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(32);
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
    /// Souris : x, y (et déplacements relatifs pour le mouvement).
    pub fn motion(&self) -> (i32, i32, i32, i32) {
        (self.i32(20), self.i32(24), self.i32(28), self.i32(32))
    }
    /// Bouton de souris : bouton (1 gauche, 2 milieu, 3 droit…), x, y.
    pub fn button(&self) -> (u8, i32, i32) {
        (self.u8(16), self.i32(20), self.i32(24))
    }
    /// Molette : crans x, y.
    pub fn wheel(&self) -> (i32, i32) {
        (self.i32(16), self.i32(20))
    }
    /// Manette : identifiant (instance, ou index pour un ajout), bouton ou axe, valeur d'axe.
    pub fn pad(&self) -> (i32, u8, i16) {
        (self.i32(8), self.u8(12), self.i16(16))
    }
}

pub type AudioCallback = extern "C" fn(userdata: *mut c_void, stream: *mut u8, len: c_int);

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
    pub fn SDL_CreateWindow(title: *const c_char, x: c_int, y: c_int, w: c_int, h: c_int, flags: u32) -> *mut SDL_Window;
    pub fn SDL_GetWindowSize(w: *mut SDL_Window, width: *mut c_int, height: *mut c_int);
    pub fn SDL_CreateRenderer(w: *mut SDL_Window, index: c_int, flags: u32) -> *mut SDL_Renderer;
    pub fn SDL_CreateTexture(r: *mut SDL_Renderer, format: u32, access: c_int, w: c_int, h: c_int) -> *mut SDL_Texture;
    pub fn SDL_DestroyTexture(t: *mut SDL_Texture);
    pub fn SDL_UpdateTexture(t: *mut SDL_Texture, rect: *const SDL_Rect, pixels: *const c_void, pitch: c_int) -> c_int;
    pub fn SDL_SetTextureBlendMode(t: *mut SDL_Texture, mode: c_int) -> c_int;
    pub fn SDL_SetRenderDrawColor(r: *mut SDL_Renderer, red: u8, g: u8, b: u8, a: u8) -> c_int;
    pub fn SDL_SetRenderDrawBlendMode(r: *mut SDL_Renderer, mode: c_int) -> c_int;
    pub fn SDL_RenderClear(r: *mut SDL_Renderer) -> c_int;
    pub fn SDL_RenderCopy(r: *mut SDL_Renderer, t: *mut SDL_Texture, src: *const SDL_Rect, dst: *const SDL_Rect) -> c_int;
    pub fn SDL_RenderPresent(r: *mut SDL_Renderer);
    pub fn SDL_RenderSetLogicalSize(r: *mut SDL_Renderer, w: c_int, h: c_int) -> c_int;
    pub fn SDL_WaitEventTimeout(e: *mut SDL_Event, timeout: c_int) -> c_int;
    pub fn SDL_PollEvent(e: *mut SDL_Event) -> c_int;
    pub fn SDL_ShowCursor(toggle: c_int) -> c_int;
    pub fn SDL_StartTextInput();
    pub fn SDL_StopTextInput();
    pub fn SDL_GetPreferredLocales() -> *mut SDL_Locale;
    pub fn SDL_free(p: *mut c_void);
    pub fn SDL_GetTicks() -> u32;

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
}

#[cfg(target_arch = "arm")]
unsafe extern "C" {
    pub fn SDL_webOSGetPanelResolution(w: *mut c_int, h: *mut c_int) -> c_int;
    pub fn SDL_webOSGetRefreshRate(rate: *mut c_int) -> c_int;
    pub fn SDL_webOSCursorVisibility(visible: c_int) -> c_int;
}

pub fn error() -> String {
    unsafe { CStr::from_ptr(SDL_GetError()).to_string_lossy().into_owned() }
}

pub fn hint(name: &str, value: &str) {
    let (n, v) = (CString::new(name).unwrap(), CString::new(value).unwrap());
    unsafe { SDL_SetHint(n.as_ptr(), v.as_ptr()) };
}

/// La première langue préférée du système (`fr-FR`).
pub fn locale() -> String {
    unsafe {
        let l = SDL_GetPreferredLocales();
        if l.is_null() || (*l).language.is_null() {
            return String::new();
        }
        let lang = CStr::from_ptr((*l).language).to_string_lossy().into_owned();
        let country = if (*l).country.is_null() { String::new() } else { format!("-{}", CStr::from_ptr((*l).country).to_string_lossy()) };
        SDL_free(l as *mut c_void);
        format!("{lang}{country}")
    }
}

/// Définition et fréquence de la dalle (webOS) ; sur l'ordinateur, 1080p à 60 Hz.
pub fn panel() -> ((u32, u32), u32) {
    #[cfg(target_arch = "arm")]
    unsafe {
        let (mut w, mut h, mut r) = (0, 0, 0);
        let res = if SDL_webOSGetPanelResolution(&mut w, &mut h) != 0 && w > 0 && h > 0 { (w as u32, h as u32) } else { (3840, 2160) };
        let rate = if SDL_webOSGetRefreshRate(&mut r) != 0 && r > 0 { r as u32 } else { 60 };
        (res, rate)
    }
    #[cfg(not(target_arch = "arm"))]
    ((1920, 1080), 60)
}

/// Montre ou cache le pointeur de la Magic Remote (celui du système).
pub fn system_cursor(visible: bool) {
    #[cfg(target_arch = "arm")]
    unsafe {
        SDL_webOSCursorVisibility(visible as c_int);
    }
    unsafe {
        SDL_ShowCursor(visible as c_int);
    }
}

pub fn pad_name(c: *mut SDL_GameController) -> String {
    unsafe {
        let n = SDL_GameControllerName(c);
        if n.is_null() { "Manette".into() } else { CStr::from_ptr(n).to_string_lossy().into_owned() }
    }
}
