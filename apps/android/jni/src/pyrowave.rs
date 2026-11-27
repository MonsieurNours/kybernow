//! PyroWave (docs/pyrowave.md, docs/pyrowave-client-android.md) : le
//! décodage par Vulkan de `libkybernow_pyrowave.so` (apps/android/jni/pyrowave),
//! à côté de MediaCodec. Ici : le paquet de configuration `KNPW`, la sonde du
//! lancement et une session d'affichage sur la surface de la `SurfaceView`.
//!
//! Sans la fonctionnalité Cargo `pyrowave` (armeabi-v7a, ou bibliothèque non
//! construite), la sonde répond « indisponible » et rien n'est lié.

use kybernow_core::pyrowave::Config;

/// Les métadonnées HDR10 du contenu, dans l'en-tête `KNPW` version 2
/// (docs/kyber-changes.md, « En-tête KNPW version 2 », KYB-29) : le
/// mastering display et les niveaux de lumière de la capture de la Station.
/// Le cœur lit la tête du paquet ([`Config`]) ; ces octets en plus ne servent
/// qu'à la swapchain de l'app, d'où leur lecture ici.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ContentHdr {
    /// Primaires rouge, vert, bleu puis blanc (x, y CIE 1931), luminances
    /// max et min (nits).
    pub mastering: Option<Mastering>,
    /// MaxCLL, MaxFALL (nits).
    pub light: Option<(u16, u16)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mastering {
    pub primaries: [f32; 6],
    pub white: [f32; 2],
    pub max_nits: f32,
    pub min_nits: f32,
}

/// Taille de l'en-tête version 2.
pub const SIZE_V2: usize = 48;

impl ContentHdr {
    /// Les métadonnées d'un paquet de version 2 ou plus récente (au moins
    /// [`SIZE_V2`] octets, les suivants ignorés) ; vides pour la version 1 ou
    /// un paquet trop court.
    pub fn parse(d: &[u8]) -> Self {
        if d.len() < SIZE_V2 || d[4] < 2 {
            return Self::default();
        }
        let u16le = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
        let u32le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
        // Unités de 0,00002 pour les coordonnées, de 0,0001 nit pour les luminances.
        let xy = |o: usize| u16le(o) as f32 * 0.000_02;
        let flags = d[16];
        let mastering = (flags & 1 != 0).then(|| Mastering {
            primaries: [xy(20), xy(22), xy(24), xy(26), xy(28), xy(30)],
            white: [xy(32), xy(34)],
            max_nits: u32le(36) as f32 * 0.0001,
            min_nits: u32le(40) as f32 * 0.0001,
        });
        let light = (flags & 2 != 0).then(|| (u16le(44), u16le(46)));
        Self { mastering, light }
    }
}

/// Le format d'un flux PyroWave : la tête du paquet `KNPW` lue par le cœur
/// (version 1 ou 2), et les métadonnées HDR10 de la version 2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PwFormat {
    pub config: Config,
    pub content: ContentHdr,
}

impl PwFormat {
    /// Un paquet de configuration ; `None` s'il n'en est pas un, s'il décrit
    /// une image impossible, ou s'il se dit de version 2 sans en avoir la taille.
    pub fn parse(d: &[u8]) -> Option<Self> {
        let config = Config::parse(d)?;
        if d[4] >= 2 && d.len() < SIZE_V2 {
            return None;
        }
        Some(Self { config, content: ContentHdr::parse(d) })
    }

    /// Sans paquet de configuration (perdu, ou Station qui ne l'envoie pas) :
    /// la définition et la chroma de l'en-tête de séquence de l'image, sinon
    /// la taille de la session ; SDR plage pleine.
    pub fn fallback(frame: &[u8], width: u32, height: u32) -> Self {
        let config = Config::from_sequence_header(frame).unwrap_or_else(|| Config::fallback(width, height));
        Self { config, content: ContentHdr::default() }
    }
}

#[cfg(all(target_os = "android", feature = "pyrowave"))]
mod native {
    use std::ffi::{c_char, c_int, c_void, CStr};

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

    pub fn configure(s: &Raw, f: &super::PwFormat, l: Option<kybernow_core::profile::HdrLuminance>) -> bool {
        let c = &f.config;
        let (max, fall, min) = l.map_or((0.0, 0.0, 0.0), |l| (l.max as f32, l.full_frame as f32, l.min as f32));
        let m = f.content.mastering;
        let content = ContentHdr {
            has_mastering: m.is_some() as c_int,
            primaries: m.map_or([0.0; 6], |m| m.primaries),
            white: m.map_or([0.0; 2], |m| m.white),
            max_luminance: m.map_or(0.0, |m| m.max_nits),
            min_luminance: m.map_or(0.0, |m| m.min_nits),
            has_light: f.content.light.is_some() as c_int,
            max_cll: f.content.light.map_or(0.0, |l| l.0 as f32),
            max_fall: f.content.light.map_or(0.0, |l| l.1 as f32),
        };
        let has = f.content != super::ContentHdr::default();
        let p = if has { &content as *const ContentHdr } else { std::ptr::null() };
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
    pub format: Option<PwFormat>,
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
    pub fn configure(&mut self, f: PwFormat, luminance: Option<kybernow_core::profile::HdrLuminance>) -> bool {
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

    fn v1(chroma444: bool, hdr: bool, full: bool, w: u32, h: u32) -> Vec<u8> {
        Config { chroma444, hdr, full_range: full, width: w, height: h }.bytes().to_vec()
    }

    /// L'en-tête v2 de docs/kyber-changes.md : 48 octets, petit-boutiste.
    fn v2(flags: u8) -> Vec<u8> {
        let mut b = v1(false, true, false, 3840, 2160);
        b[4] = 2;
        b.extend_from_slice(&[flags, 0, 0, 0]);
        for v in [35400u16, 14600, 8500, 39850, 6550, 2300, 15635, 16450] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&10_000_000u32.to_le_bytes()); // 1 000 nits
        b.extend_from_slice(&1u32.to_le_bytes()); // 0,0001 nit
        b.extend_from_slice(&1600u16.to_le_bytes());
        b.extend_from_slice(&400u16.to_le_bytes());
        assert_eq!(b.len(), SIZE_V2);
        b
    }

    #[test]
    fn version_1_lue_par_le_coeur() {
        let f = PwFormat::parse(&v1(false, false, true, 3840, 2160)).unwrap();
        assert_eq!(f.config, Config { chroma444: false, hdr: false, full_range: true, width: 3840, height: 2160 });
        assert_eq!(f.content, ContentHdr::default());
        assert!(PwFormat::parse(&v1(false, false, true, 1921, 1080)).is_none(), "4:2:0 de taille impaire");
        assert!(PwFormat::parse(&v1(false, false, true, 3840, 2160)[..12]).is_none(), "trop court");
    }

    #[test]
    fn version_2_et_ses_metadonnees() {
        let f = PwFormat::parse(&v2(3)).unwrap();
        assert!(f.config.hdr && !f.config.full_range && f.config.width == 3840);
        let m = f.content.mastering.unwrap();
        assert!((m.primaries[0] - 0.708).abs() < 1e-4 && (m.primaries[3] - 0.797).abs() < 1e-4);
        assert!((m.white[0] - 0.3127).abs() < 1e-4 && (m.white[1] - 0.329).abs() < 1e-4);
        assert!((m.max_nits - 1000.0).abs() < 1e-3 && (m.min_nits - 0.0001).abs() < 1e-6);
        assert_eq!(f.content.light, Some((1600, 400)));
        // Un champ sans son drapeau ne veut rien dire.
        let f = PwFormat::parse(&v2(2)).unwrap();
        assert!(f.content.mastering.is_none() && f.content.light == Some((1600, 400)));
        assert_eq!(PwFormat::parse(&v2(0)).unwrap().content, ContentHdr::default());
        // Octets en plus ignorés (version plus récente) ; version 2 trop courte refusée.
        let mut longer = v2(3);
        longer[4] = 3;
        longer.extend_from_slice(&[9; 8]);
        assert_eq!(PwFormat::parse(&longer).unwrap().content.light, Some((1600, 400)));
        assert!(PwFormat::parse(&v2(3)[..32]).is_none());
    }

    #[test]
    fn repli_sans_paquet() {
        // En-tête de séquence : bit de poids fort, largeur − 1 et hauteur − 1 sur 14 bits.
        let w0: u32 = (1 << 31) | ((1080 - 1) << 14) | (1920 - 1);
        let mut frame = w0.to_le_bytes().to_vec();
        frame.extend_from_slice(&0u32.to_le_bytes());
        let f = PwFormat::fallback(&frame, 3840, 2160);
        assert_eq!((f.config.width, f.config.height, f.config.hdr), (1920, 1080, false));
        assert_eq!(PwFormat::fallback(&[1, 2], 1921, 1081).config.width, 1920);
    }
}
