//! PyroWave (docs/pyrowave.md, docs/pyrowave-client-android.md) : le
//! décodage par Vulkan de `libkybernow_pyrowave.so` (apps/android/jni/pyrowave),
//! à côté de MediaCodec. Ici : le paquet de configuration `KNPW`, la sonde du
//! lancement et une session d'affichage sur la surface de la `SurfaceView`.
//!
//! Sans la fonctionnalité Cargo `pyrowave` (armeabi-v7a, ou bibliothèque non
//! construite), la sonde répond « indisponible » et rien n'est lié.

/// Paquet de configuration PyroWave (contrat de docs/pyrowave.md), 16 octets :
/// `"KNPW"` | version 1 | chroma (0 4:2:0, 1 4:4:4) | couleur (0 SDR BT.709,
/// 1 HDR10 BT.2020 PQ) | plage (0 pleine, 1 réduite) | largeur u32 LE | hauteur u32 LE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PwConfig {
    pub chroma444: bool,
    pub hdr: bool,
    pub full_range: bool,
    pub width: u32,
    pub height: u32,
}

impl PwConfig {
    pub const SIZE: usize = 16;

    pub fn parse(b: &[u8]) -> Option<Self> {
        if b.len() < Self::SIZE || &b[..4] != b"KNPW" || b[4] != 1 {
            return None;
        }
        let width = u32::from_le_bytes([b[8], b[9], b[10], b[11]]);
        let height = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        if width == 0 || height == 0 || width > 16384 || height > 16384 {
            return None;
        }
        Some(Self { chroma444: b[5] == 1, hdr: b[6] == 1, full_range: b[7] == 0, width, height })
    }

    /// Sans paquet de configuration (perdu, ou Station qui ne l'envoie pas) :
    /// la taille de la session, 4:2:0, SDR plage pleine (ce qu'envoie la Station en SDR).
    pub fn fallback(width: u32, height: u32) -> Self {
        Self { chroma444: false, hdr: false, full_range: true, width: width & !1, height: height & !1 }
    }
}

#[cfg(all(target_os = "android", feature = "pyrowave"))]
mod native {
    use std::ffi::{c_char, c_int, c_void, CStr};

    #[link(name = "kybernow_pyrowave")]
    unsafe extern "C" {
        fn kn_pw_probe(report: *mut c_char, size: usize) -> c_int;
        fn kn_pw_open(window: *mut c_void, error: *mut c_char, size: usize) -> *mut c_void;
        fn kn_pw_configure(s: *mut c_void, w: u32, h: u32, c444: c_int, hdr: c_int, full: c_int, max: f32, fall: f32, min: f32) -> c_int;
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

    pub fn configure(s: &Raw, c: &super::PwConfig, l: Option<kybernow_core::profile::HdrLuminance>) -> bool {
        let (max, fall, min) = l.map_or((0.0, 0.0, 0.0), |l| (l.max as f32, l.full_frame as f32, l.min as f32));
        unsafe { kn_pw_configure(s.0, c.width, c.height, c.chroma444 as c_int, c.hdr as c_int, c.full_range as c_int, max, fall, min) != 0 }
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

/// Résultat d'une image (`kn_pw_frame`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub config: Option<PwConfig>,
}

#[cfg(target_os = "android")]
impl PwSession {
    #[allow(unused_variables)]
    pub fn open(window: ndk::native_window::NativeWindow) -> Result<Self, String> {
        #[cfg(feature = "pyrowave")]
        {
            let raw = native::open(window.ptr().as_ptr() as *mut _)?;
            Ok(Self { raw, _window: window, config: None })
        }
        #[cfg(not(feature = "pyrowave"))]
        {
            Err("PyroWave not built for this ABI".into())
        }
    }

    /// Décodeur et plans pour ce format (rien si inchangé).
    #[allow(unused_variables)]
    pub fn configure(&mut self, c: PwConfig, luminance: Option<kybernow_core::profile::HdrLuminance>) -> bool {
        if self.config == Some(c) {
            return true;
        }
        #[cfg(feature = "pyrowave")]
        let ok = native::configure(&self.raw, &c, luminance);
        #[cfg(not(feature = "pyrowave"))]
        let ok = false;
        self.config = ok.then_some(c);
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
    fn paquet_de_configuration() {
        let mut b = b"KNPW\x01\x00\x00\x00".to_vec();
        b.extend_from_slice(&3840u32.to_le_bytes());
        b.extend_from_slice(&2160u32.to_le_bytes());
        let c = PwConfig::parse(&b).unwrap();
        assert_eq!(c, PwConfig { chroma444: false, hdr: false, full_range: true, width: 3840, height: 2160 });
        b[5] = 1;
        b[6] = 1;
        b[7] = 1;
        let c = PwConfig::parse(&b).unwrap();
        assert!(c.chroma444 && c.hdr && !c.full_range);
        b[4] = 2;
        assert!(PwConfig::parse(&b).is_none(), "version inconnue");
        assert!(PwConfig::parse(&b[..12]).is_none(), "trop court");
        assert_eq!(PwConfig::fallback(1921, 1081).width, 1920);
    }
}
