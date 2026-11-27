//! PyroWave (docs/pyrowave.md, docs/pyrowave-client-android.md) : le
//! décodage par Vulkan de `libkybernow_pyrowave.so` (apps/android/jni/pyrowave),
//! à côté de MediaCodec. Ici : le paquet de configuration `KNPW`, la sonde du
//! lancement et une session d'affichage sur la surface de la `SurfaceView`.
//!
//! Sans la fonctionnalité Cargo `pyrowave` (armeabi-v7a, ou bibliothèque non
//! construite), la sonde répond « indisponible » et rien n'est lié.

use kybernow_core::pyrowave::Config;

/// Sans paquet de configuration (perdu, ou Station qui ne l'envoie pas) :
/// la définition et la chroma de l'en-tête de séquence de l'image, sinon la
/// taille de la session ; SDR plage pleine, sans métadonnées.
pub fn fallback(frame: &[u8], width: u32, height: u32) -> Config {
    Config::from_sequence_header(frame).unwrap_or_else(|| Config::fallback(width, height))
}

#[cfg(all(target_os = "android", feature = "pyrowave"))]
mod native {
    use std::ffi::{c_char, c_int, c_void, CStr};

    use super::Config;

    #[link(name = "kybernow_pyrowave")]
    unsafe extern "C" {
        fn kn_pw_probe(report: *mut c_char, size: usize) -> c_int;
        fn kn_pw_open(window: *mut c_void, error: *mut c_char, size: usize) -> *mut c_void;
        fn kn_pw_configure(s: *mut c_void, w: u32, h: u32, c444: c_int, hdr: c_int, full: c_int, max: f32, fall: f32, min: f32, content: *const ContentHdr) -> c_int;
        fn kn_pw_frame(s: *mut c_void, data: *const u8, size: usize) -> c_int;
        fn kn_pw_describe(s: *mut c_void) -> *const c_char;
        fn kn_pw_close(s: *mut c_void);
    }

    fn text(buf: &[u8]) -> String {
        CStr::from_bytes_until_nul(buf).map(|c| c.to_string_lossy().into_owned()).unwrap_or_default()
    }

    pub fn probe() -> (bool, String) {
        let mut buf = [0u8; 256];
        let ok = unsafe { kn_pw_probe(buf.as_mut_ptr() as *mut c_char, buf.len()) } != 0;
        (ok, text(&buf))
    }

    pub struct Raw(pub *mut c_void);
    unsafe impl Send for Raw {}

    pub fn open(window: *mut c_void) -> Result<Raw, String> {
        let mut buf = [0u8; 256];
        let s = unsafe { kn_pw_open(window, buf.as_mut_ptr() as *mut c_char, buf.len()) };
        if s.is_null() { Err(text(&buf)) } else { Ok(Raw(s)) }
    }

    /// `kn_pw_content_hdr` de kn_pyrowave.h.
    #[repr(C)]
    struct ContentHdr {
        has_mastering: c_int,
        primaries: [f32; 6],
        white: [f32; 2],
        max_luminance: f32,
        min_luminance: f32,
        has_light: c_int,
        max_cll: f32,
        max_fall: f32,
    }

    /// `c.hdr10` : les métadonnées HDR10 du contenu (en-tête `KNPW` version 2,
    /// lues par le cœur), données au décodeur dans `kn_pw_content_hdr`.
    pub fn configure(s: &Raw, c: &Config, l: Option<kybernow_core::profile::HdrLuminance>) -> bool {
        let (max, fall, min) = l.map_or((0.0, 0.0, 0.0), |l| (l.max as f32, l.full_frame as f32, l.min as f32));
        let m = c.hdr10.mastering;
        let light = c.hdr10.light;
        let content = ContentHdr {
            has_mastering: m.is_some() as c_int,
            primaries: m.map_or([0.0; 6], |m| m.primaries_xy()),
            white: m.map_or([0.0; 2], |m| m.white_xy()),
            max_luminance: m.map_or(0.0, |m| m.max_nits() as f32),
            min_luminance: m.map_or(0.0, |m| m.min_nits() as f32),
            has_light: light.is_some() as c_int,
            max_cll: light.map_or(0.0, |l| f32::from(l.max_cll)),
            max_fall: light.map_or(0.0, |l| f32::from(l.max_fall)),
        };
        let p = if c.hdr10.is_empty() { std::ptr::null() } else { &content as *const ContentHdr };
        unsafe {
            kn_pw_configure(s.0, c.width, c.height, c.chroma444 as c_int, c.hdr as c_int, c.full_range as c_int, max, fall, min, p) != 0
        }
    }

    pub fn frame(s: &Raw, data: &[u8]) -> i32 {
        unsafe { kn_pw_frame(s.0, data.as_ptr(), data.len()) }
    }

    pub fn describe(s: &Raw) -> String {
        unsafe { CStr::from_ptr(kn_pw_describe(s.0)) }.to_string_lossy().into_owned()
    }

    pub fn close(s: &Raw) {
        unsafe { kn_pw_close(s.0) }
    }
}

/// Résultat d'une image (`kn_pw_frame`). Sans la bibliothèque (armeabi-v7a),
/// seule la perte est rendue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(feature = "pyrowave"), allow(dead_code))]
pub enum Shown {
    /// Décodée et présentée, GPU terminé.
    Shown,
    /// Incomplète, illisible ou pas d'image de swapchain à temps : sautée.
    Skipped,
    /// Surface perdue ou device en erreur : session à refaire.
    Lost,
}

/// La sonde du lancement : `(disponible, compte rendu)`. Le compte rendu dit
/// le GPU et le chemin, ou pourquoi PyroWave est indisponible (anglais, journal).
pub fn probe() -> (bool, String) {
    #[cfg(all(target_os = "android", feature = "pyrowave"))]
    {
        native::probe()
    }
    #[cfg(not(all(target_os = "android", feature = "pyrowave")))]
    {
        (false, "PyroWave not built for this ABI".into())
    }
}

/// Une session d'affichage PyroWave sur une surface.
#[cfg(target_os = "android")]
pub struct PwSession {
    #[cfg(feature = "pyrowave")]
    raw: native::Raw,
    /// La surface, gardée tant que la swapchain l'utilise.
    _window: ndk::native_window::NativeWindow,
    pub format: Option<Config>,
}

#[cfg(target_os = "android")]
impl PwSession {
    #[allow(unused_variables)]
    pub fn open(window: ndk::native_window::NativeWindow) -> Result<Self, String> {
        #[cfg(feature = "pyrowave")]
        {
            let raw = native::open(window.ptr().as_ptr() as *mut _)?;
            Ok(Self { raw, _window: window, format: None })
        }
        #[cfg(not(feature = "pyrowave"))]
        {
            Err("PyroWave not built for this ABI".into())
        }
    }

    /// Décodeur et plans pour ce format (rien si inchangé) ; des métadonnées
    /// HDR10 seules changées sont redonnées à la swapchain.
    #[allow(unused_variables)]
    pub fn configure(&mut self, f: Config, luminance: Option<kybernow_core::profile::HdrLuminance>) -> bool {
        if self.format == Some(f) {
            return true;
        }
        #[cfg(feature = "pyrowave")]
        let ok = native::configure(&self.raw, &f, luminance);
        #[cfg(not(feature = "pyrowave"))]
        let ok = false;
        self.format = ok.then_some(f);
        ok
    }

    #[allow(unused_variables)]
    pub fn frame(&self, data: &[u8]) -> Shown {
        #[cfg(feature = "pyrowave")]
        {
            match native::frame(&self.raw, data) {
                0 => Shown::Shown,
                r if r > 0 => Shown::Skipped,
                _ => Shown::Lost,
            }
        }
        #[cfg(not(feature = "pyrowave"))]
        {
            Shown::Lost
        }
    }

    pub fn describe(&self) -> String {
        #[cfg(feature = "pyrowave")]
        {
            native::describe(&self.raw)
        }
        #[cfg(not(feature = "pyrowave"))]
        {
            String::new()
        }
    }
}

#[cfg(target_os = "android")]
impl Drop for PwSession {
    fn drop(&mut self) {
        #[cfg(feature = "pyrowave")]
        native::close(&self.raw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repli_sans_paquet() {
        // En-tête de séquence : bit de poids fort, largeur − 1 et hauteur − 1 sur 14 bits.
        let w0: u32 = (1 << 31) | ((1080 - 1) << 14) | (1920 - 1);
        let mut frame = w0.to_le_bytes().to_vec();
        frame.extend_from_slice(&0u32.to_le_bytes());
        let f = fallback(&frame, 3840, 2160);
        assert_eq!((f.width, f.height, f.hdr), (1920, 1080, false));
        assert!(f.hdr10.is_empty());
        assert_eq!(fallback(&[1, 2], 1921, 1081).width, 1920);
    }
}
