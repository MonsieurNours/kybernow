//! Règles communes des apps, tenues par le cœur (phase 2b de l'audit du
//! 27/11, `docs/audit-2026-11-27/migration-coeur-2b.md`) : Swift n'en garde
//! plus de copie. Empreinte affichée et comparée, adresse tapée, adresse
//! d'Admin, valeurs communes (profils, bornes du débit, délais), plan d'une
//! session, débit de PyroWave, « Définition ramenée », lien d'invitation,
//! paquet de configuration `KNPW` (versions 1 et 2), image qui n'est plus
//! regardée et repli de PyroWave d'une session.

use std::ffi::c_char;

use kybernow_core::console::{self, Mode};
use kybernow_core::profile::{self, Capabilities, Codec, Device, StreamSettings};
use kybernow_core::{i18n, lend, plan, pyrowave};

use crate::{KnSession, give, text, with};

/// L'appareil de cette bibliothèque : le Mac est un ordinateur (20 Mbit/s
/// fenêtre cachée), l'iPhone et l'iPad un appareil mobile, l'Apple TV la
/// sienne (8 Mbit/s en arrière-plan).
#[cfg(target_os = "macos")]
pub const DEVICE: Device = Device::Computer;
#[cfg(target_os = "tvos")]
pub const DEVICE: Device = Device::AppleTv;
#[cfg(not(any(target_os = "macos", target_os = "tvos")))]
pub const DEVICE: Device = Device::AppleMobile;

fn caps(json: *const c_char) -> Capabilities {
    serde_json::from_str(&text(json)).unwrap_or_default()
}

// MARK: - Empreinte d'Admin

/// L'empreinte d'Admin telle que link-ui la montre : les quatre premiers
/// groupes, en capitales (`7C41 E09B AABB CCDD`) ; vide sans chiffre.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_short_fingerprint(fp: *const c_char) -> *mut c_char {
    kybernow_core::ffi_guard!(null => { give(console::short_fingerprint(&text(fp))) })
}

/// Deux empreintes sont la même (préfixe `sha256:`, deux-points, espaces et
/// casse ignorés) ; une empreinte vide ne l'est jamais (MAC-20).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_fingerprint_same(a: *const c_char, b: *const c_char) -> bool {
    kybernow_core::ffi_guard!({ console::same_fingerprint(&text(a), &text(b)) })
}

// MARK: - Adresses

/// Une adresse tapée (« 192.168.1.20:9443 », « [fe80::1]:9443 », un nom) :
/// `{"host","port"}`, ou nul si elle ne se lit pas. `default_port` : le port
/// pris quand il manque (9443 pour Admin) ; 0 l'exige. Une IPv6 nue est un
/// hôte entier.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_parse_address(address: *const c_char, default_port: u16) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        match console::parse_address(&text(address), (default_port > 0).then_some(default_port)) {
            Some((host, port)) => give(serde_json::json!({ "host": host, "port": port }).to_string()),
            None => std::ptr::null_mut(),
        }
    })
}

/// L'adresse d'Admin d'un hôte (crochets d'une IPv6 posés une seule fois) :
/// `https://192.168.1.20:9443/`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_admin_url(host: *const c_char, port: u16) -> *mut c_char {
    kybernow_core::ffi_guard!(null => { give(console::admin_url(&text(host), port)) })
}

// MARK: - Valeurs communes et plan

/// Les valeurs communes (`plan::values_json` du cœur) pour ce que décode
/// l'appareil (`capabilities_json`, comme au départ d'une session) :
/// `profiles` (`jeu`, `bureau`, `max` : `codec`, `yuv444`, `mbps`,
/// `reliable`…), `pyrowave_mbps`, `pyrowave_mbps_444`, `manual_mbps`
/// (`min`, `step`, `max`, `max_pyrowave`, `lg_tv_max`), `background_bitrate`,
/// `max_guests`, `max_sessions`, `relay_link_wait_secs`,
/// `station_notice_wait_secs`, `station_notice_before`, `admin_port`,
/// `address_example`, `default_relay` ; plus `device_background_bitrate` et
/// `device_manual_mbps` (`max`, `max_pyrowave`) pour cet appareil.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_values_json(capabilities_json: *const c_char) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        let mut v = plan::values_json(caps(capabilities_json));
        if let Some(map) = v.as_object_mut() {
            map.insert("device_background_bitrate".into(), kybernow_core::session::background_bitrate(DEVICE).into());
            let (_, max) = profile::manual_mbps(DEVICE, Codec::Hevc);
            let (_, max_pyrowave) = profile::manual_mbps(DEVICE, Codec::PyroWave);
            map.insert("device_manual_mbps".into(), serde_json::json!({ "max": max, "max_pyrowave": max_pyrowave }));
        }
        give(v.to_string())
    })
}

/// Le plan d'une session (`plan::plan_json`) : `choices_json` (`profile`,
/// `codec`, `yuv444`, `mbps`, `reliable`, `max_4k`, `size`, `lighten`,
/// `auto`, `fixed`, `encoder_max`), ce que décode l'appareil, l'écran ;
/// rend `{settings, mode, mbps, label}` ou nul pour un JSON illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_plan_json(
    choices_json: *const c_char,
    capabilities_json: *const c_char,
    width: u32,
    height: u32,
    refresh: u32,
) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        let screen = Mode { width, height, refresh };
        match plan::plan_json(&text(choices_json), caps(capabilities_json), screen) {
            Some(v) => give(v.to_string()),
            None => std::ptr::null_mut(),
        }
    })
}

/// Le débit d'une session pour ces réglages (même JSON que `settings` au
/// départ) et ce mode, en Mbit/s : un profil suit les pixels par seconde, un
/// débit réglé à la main reste. 0 pour un JSON illisible.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_settings_bitrate(settings_json: *const c_char, width: u32, height: u32, refresh: u32) -> u32 {
    kybernow_core::ffi_guard!({
        StreamSettings::from_json(&text(settings_json)).map_or(0, |s| s.bitrate(width, height, refresh) / 1_000_000)
    })
}

/// Le même débit pour des écrans dont les pixels par seconde font ensemble
/// `pixel_rate` (multi-écrans : la somme des écrans de la session).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_settings_bitrate_for_pixel_rate(settings_json: *const c_char, pixel_rate: f64) -> u32 {
    kybernow_core::ffi_guard!({
        StreamSettings::from_json(&text(settings_json)).map_or(0, |s| s.bitrate_for_pixel_rate(pixel_rate) / 1_000_000)
    })
}

/// Un débit réglé à la main (ancien réglage, reçu d'un autre appareil)
/// ramené dans les bornes de cet appareil et de ce format (Mbit/s).
#[unsafe(no_mangle)]
pub extern "C" fn kn_clamp_manual_mbps(mbps: u32, pyrowave: bool) -> u32 {
    kybernow_core::ffi_guard!({
        let (min, max) = profile::manual_mbps(DEVICE, if pyrowave { Codec::PyroWave } else { Codec::Hevc });
        mbps.clamp(min, max)
    })
}

// MARK: - PyroWave

/// Le débit de PyroWave d'un profil pour ce mode (Mbit/s), en 4:2:0 ou en
/// 4:4:4 (`profile::pyrowave_mbps`).
#[unsafe(no_mangle)]
pub extern "C" fn kn_pyrowave_mbps(width: u32, height: u32, refresh: u32, yuv444: bool) -> u32 {
    kybernow_core::ffi_guard!({ profile::pyrowave_mbps(width, height, refresh, yuv444) })
}

/// « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. » dans la
/// langue de l'app, avec les nombres de `KnSecond` (`lowered_*`).
#[unsafe(no_mangle)]
pub extern "C" fn kn_pyrowave_lowered_text(width: u32, height: u32, refresh: u32, mbps: u32) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        give(pyrowave::Lowered { width, height, refresh, mbps }.text(i18n::current()))
    })
}

/// Version la plus récente de l'en-tête `KNPW` que lit le décodeur de
/// l'app, pour toute l'app (KYB-29) : 2, la version avec les métadonnées
/// HDR10. Une Station qui ne connaît pas la demande envoie la version 1,
/// lue elle aussi. Sans le correctif kyclient de PyroWave : sans effet.
#[unsafe(no_mangle)]
pub extern "C" fn kn_pyrowave_set_header(version: u8) {
    kybernow_core::ffi_guard!({
        #[cfg(feature = "kyber-pyrowave")]
        kyclient::set_pyrowave_header(version);
        #[cfg(not(feature = "kyber-pyrowave"))]
        let _ = version;
    })
}

/// Taille de l'en-tête `KNPW` version 2 (docs/kyber-changes.md, « En-tête
/// KNPW version 2 »).
const KNPW_V2_SIZE: usize = 48;

/// Les métadonnées HDR10 de l'en-tête version 2 : écran de mastering et
/// niveaux de lumière, chacun seulement si son drapeau est levé.
fn knpw_v2_metadata(d: &[u8]) -> (Option<serde_json::Value>, Option<serde_json::Value>) {
    if d.len() < KNPW_V2_SIZE || d[4] < 2 {
        return (None, None);
    }
    let u16le = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let u32le = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let flags = d[16];
    let mastering = (flags & 1 != 0).then(|| {
        serde_json::json!({
            // Rouge, vert, bleu, puis le blanc (unités de 0,00002).
            "primaries": [u16le(20), u16le(22), u16le(24), u16le(26), u16le(28), u16le(30)],
            "white": [u16le(32), u16le(34)],
            // Unités de 0,0001 cd/m².
            "max_luminance": u32le(36),
            "min_luminance": u32le(40),
        })
    });
    let light = (flags & 2 != 0).then(|| serde_json::json!({ "max_cll": u16le(44), "max_fall": u16le(46) }));
    (mastering, light)
}

/// Un paquet de configuration PyroWave (`KNPW`, versions 1 et 2) :
/// `{version, chroma444, hdr, full_range, width, height, label, mastering,
/// light}` (`mastering` : `primaries` [rx, ry, gx, gy, bx, by], `white`
/// [x, y], `max_luminance`, `min_luminance`, aux unités du SEI ; `light` :
/// `max_cll`, `max_fall` ; nuls sans leur drapeau ou en version 1), ou nul
/// si ce n'en est pas un (définition impossible, version 2 de moins de 48
/// octets). Les octets au-delà de ce que l'app connaît sont ignorés.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pyrowave_config(data: *const u8, len: usize) -> *mut c_char {
    kybernow_core::ffi_guard!(null => {
        if data.is_null() {
            return std::ptr::null_mut();
        }
        let d = unsafe { std::slice::from_raw_parts(data, len) };
        let Some(c) = pyrowave::Config::parse(d) else { return std::ptr::null_mut() };
        // Le cœur lit l'en-tête commun ; une version 2 doit avoir ses 48
        // octets, sans quoi ses métadonnées seraient lues hors du paquet.
        if d[4] >= 2 && d.len() < KNPW_V2_SIZE {
            return std::ptr::null_mut();
        }
        let (mastering, light) = knpw_v2_metadata(d);
        give(
            serde_json::json!({
                "version": d[4], "chroma444": c.chroma444, "hdr": c.hdr, "full_range": c.full_range,
                "width": c.width, "height": c.height, "label": c.label(),
                "mastering": mastering, "light": light,
            })
            .to_string(),
        )
    })
}

/// Le paquet de configuration `KNPW` version 1 de ces valeurs (16 octets
/// écrits dans `out`), pour les outils et les essais.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_pyrowave_config_bytes(chroma444: bool, hdr: bool, full_range: bool, width: u32, height: u32, out: *mut u8) {
    kybernow_core::ffi_guard!({
        if out.is_null() {
            return;
        }
        let bytes = pyrowave::Config { chroma444, hdr, full_range, width, height }.bytes();
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    })
}

// MARK: - Lien d'invitation

/// Le texte a la forme d'un lien d'invitation (`kybernow://borrow…`,
/// `…/link?borrow=…`), lisible ou non : sans lecture possible, « Ce lien
/// d'invitation est illisible. ».
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_looks_like_invite(link: *const c_char) -> bool {
    kybernow_core::ffi_guard!({ lend::looks_like_invite(&text(link)) })
}

// MARK: - Session

/// L'image n'est plus regardée (fenêtre cachée ou réduite sur Mac, app en
/// arrière-plan sur iPhone, iPad et Apple TV), puis l'est de nouveau
/// (TRANS-40) : débit plafonné à celui de cet appareil (20 Mbit/s Mac, 8
/// iPhone, iPad, Apple TV) et image figée qui n'est pas une panne ; tout est
/// rendu au retour. Une image encore montrée sur un écran externe n'est pas
/// en arrière-plan.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_set_background(s: *const KnSession, background: bool) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.set_background(background, DEVICE));
    })
}

/// Le décodeur PyroWave ne s'ouvre pas sur cet appareil (GPU sans ce qu'il
/// faut, pipelines refusés) : l'image repart, pour cette session, dans le
/// format de repli de l'appareil ; l'avis vient par `on_pyrowave_fallback`.
/// À dire une fois, après avoir retenu l'échec.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kn_session_pyrowave_failed(s: *const KnSession) {
    kybernow_core::ffi_guard!({
        with(s, |s| s.pyrowave_failed());
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CStr, CString};

    fn take(p: *mut c_char) -> Option<String> {
        if p.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        unsafe { crate::kn_string_free(p) };
        Some(s)
    }

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn empreinte_courte_et_meme() {
        let fp = c("sha256:7c:41:e0:9b:aa:bb:cc:dd:ee:ff");
        assert_eq!(take(unsafe { kn_short_fingerprint(fp.as_ptr()) }).as_deref(), Some("7C41 E09B AABB CCDD"));
        let other = c("7C41E09BAABBCCDDEEFF");
        assert!(unsafe { kn_fingerprint_same(fp.as_ptr(), other.as_ptr()) });
        let empty = c("");
        assert!(!unsafe { kn_fingerprint_same(empty.as_ptr(), empty.as_ptr()) });
    }

    #[test]
    fn adresse_tapee_et_admin() {
        let a = c("[fe80::1]:9443");
        let v: serde_json::Value = serde_json::from_str(&take(unsafe { kn_parse_address(a.as_ptr(), 0) }).unwrap()).unwrap();
        assert_eq!(v["host"], "fe80::1");
        assert_eq!(v["port"], 9443);
        let bare = c("192.168.1.20");
        assert!(take(unsafe { kn_parse_address(bare.as_ptr(), 0) }).is_none(), "port exigé");
        let v: serde_json::Value = serde_json::from_str(&take(unsafe { kn_parse_address(bare.as_ptr(), 9443) }).unwrap()).unwrap();
        assert_eq!(v["port"], 9443);
        let h = c("fe80::1");
        assert_eq!(take(unsafe { kn_admin_url(h.as_ptr(), 9443) }).as_deref(), Some("https://[fe80::1]:9443/"));
    }

    #[test]
    fn valeurs_de_cet_appareil() {
        let caps = c(r#"{"hevc":true,"hevc444":true,"av1":false,"hdr":false,"pyrowave":true}"#);
        let v: serde_json::Value = serde_json::from_str(&take(unsafe { kn_values_json(caps.as_ptr()) }).unwrap()).unwrap();
        assert_eq!(v["profiles"]["jeu"]["mbps"], 60);
        assert_eq!(v["device_manual_mbps"]["max"], 500);
        assert_eq!(v["device_manual_mbps"]["max_pyrowave"], 2500);
        let bg = v["device_background_bitrate"].as_u64().unwrap();
        assert!(bg == 20_000_000 || bg == 8_000_000);
        assert_eq!(kn_clamp_manual_mbps(4294, false), 500);
        assert_eq!(kn_clamp_manual_mbps(4294, true), 2500);
        assert_eq!(kn_clamp_manual_mbps(1, false), 5);
    }

    #[test]
    fn debit_d_un_profil_et_a_la_main() {
        let jeu = c(r#"{"profile":"jeu","codec":"hevc","yuv444":false,"mbps":60,"reliable":false}"#);
        assert_eq!(unsafe { kn_settings_bitrate(jeu.as_ptr(), 3840, 2160, 60) }, 60);
        let main = c(r#"{"codec":"hevc","yuv444":false,"mbps":80,"reliable":false}"#);
        assert_eq!(unsafe { kn_settings_bitrate(main.as_ptr(), 1280, 720, 30) }, 80);
        let rate = 2.0 * 3840.0 * 2160.0 * 60.0;
        let one = unsafe { kn_settings_bitrate(jeu.as_ptr(), 3840, 2160, 120) };
        assert_eq!(unsafe { kn_settings_bitrate_for_pixel_rate(jeu.as_ptr(), rate) }, one, "deux écrans à 60 Hz = un écran à 120 Hz");
    }

    fn knpw_v2(flags: u8) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(b"KNPW");
        d.extend_from_slice(&[2, 0, 1, 1]);
        d.extend_from_slice(&3840u32.to_le_bytes());
        d.extend_from_slice(&2160u32.to_le_bytes());
        d.extend_from_slice(&[flags, 0, 0, 0]);
        for v in [35400u16, 14600, 8500, 39850, 6550, 2300, 15635, 16450] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&10_000_000u32.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&1000u16.to_le_bytes());
        d.extend_from_slice(&400u16.to_le_bytes());
        d
    }

    fn config(d: &[u8]) -> Option<serde_json::Value> {
        take(unsafe { kn_pyrowave_config(d.as_ptr(), d.len()) }).map(|s| serde_json::from_str(&s).unwrap())
    }

    #[test]
    fn knpw_versions_1_et_2() {
        let mut v1 = [0u8; 16];
        unsafe { kn_pyrowave_config_bytes(false, false, true, 1920, 1080, v1.as_mut_ptr()) };
        let c1 = config(&v1).unwrap();
        assert_eq!((c1["version"].as_u64(), c1["width"].as_u64(), c1["hdr"].as_bool()), (Some(1), Some(1920), Some(false)));
        assert!(c1["mastering"].is_null() && c1["light"].is_null());

        let c2 = config(&knpw_v2(3)).unwrap();
        assert_eq!(c2["version"], 2);
        assert_eq!(c2["hdr"], true);
        assert_eq!(c2["full_range"], false);
        assert_eq!(c2["mastering"]["primaries"][0], 35400);
        assert_eq!(c2["mastering"]["white"][1], 16450);
        assert_eq!(c2["mastering"]["max_luminance"], 10_000_000);
        assert_eq!(c2["light"]["max_cll"], 1000);
        assert_eq!(c2["light"]["max_fall"], 400);

        // Drapeau du niveau de lumière seul.
        let c3 = config(&knpw_v2(2)).unwrap();
        assert!(c3["mastering"].is_null() && c3["light"]["max_cll"] == 1000);

        // Version 2 tronquée : refusée ; octets de plus : ignorés.
        assert!(config(&knpw_v2(3)[..40]).is_none());
        let mut longer = knpw_v2(3);
        longer.extend_from_slice(&[9; 8]);
        assert_eq!(config(&longer).unwrap()["light"]["max_fall"], 400);
    }

    #[test]
    fn invitation_et_definition_ramenee() {
        let link = c("kybernow://borrow?login=lea");
        assert!(unsafe { kn_looks_like_invite(link.as_ptr()) });
        let other = c("https://example.org");
        assert!(!unsafe { kn_looks_like_invite(other.as_ptr()) });
        i18n::set_current(i18n::Lang::Fr);
        let t = take(kn_pyrowave_lowered_text(2560, 1440, 120, 900)).unwrap();
        assert!(t.contains("2560") && t.contains("900"), "{t}");
    }
}
