//! Profils Jeu, Bureau et Max, et débit selon les pixels par seconde, comme
//! l'app Mac (`Model.swift`).

/// Format vidéo demandé à l'ordinateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
    Av1,
}

impl Codec {
    /// Code du format dans les paquets de Kyber (`media::FOURCC_*`).
    pub fn fourcc(self) -> u32 {
        match self {
            Self::H264 => crate::media::FOURCC_H264,
            Self::Hevc => crate::media::FOURCC_HEVC,
            Self::Av1 => crate::media::FOURCC_AV1,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::H264 => "H.264",
            Self::Hevc => "HEVC",
            Self::Av1 => "AV1",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "h264" | "h.264" | "avc" => Some(Self::H264),
            "hevc" | "h265" | "h.265" => Some(Self::Hevc),
            "av1" => Some(Self::Av1),
            _ => None,
        }
    }

    pub(crate) fn kyber(self) -> kyclient::VideoCodec {
        match self {
            Self::H264 => kyclient::VideoCodec::H264,
            Self::Hevc => kyclient::VideoCodec::HEVC,
            Self::Av1 => kyclient::VideoCodec::AV1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Jeu,
    Bureau,
    Max,
}

impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jeu => "Jeu",
            Self::Bureau => "Bureau",
            Self::Max => "Max",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "jeu" | "game" => Some(Self::Jeu),
            "bureau" | "desktop" => Some(Self::Bureau),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    /// Ce que la console fait d'un changement de mode de l'ordinateur.
    pub fn mode_policy(self) -> &'static str {
        match self {
            Self::Jeu => "follow",
            Self::Bureau => "restore",
            Self::Max => "default",
        }
    }
}

/// Ce que l'appareil sait décoder par sa carte graphique.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Capabilities {
    pub hevc: bool,
    pub hevc444: bool,
    pub av1: bool,
    /// HDR (docs/hdr.md) : le décodeur lit le HEVC Main 10 ou l'AV1 10 bits
    /// et l'app sait afficher une image PQ / BT.2020.
    pub hdr: bool,
}

/// Réglages d'une session en JSON ([`StreamSettings::from_json`]).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SettingsJson {
    pub profile: Option<String>,
    pub codec: String,
    #[serde(default)]
    pub yuv444: bool,
    pub mbps: u32,
    #[serde(default)]
    pub reliable: bool,
    /// Le choix « HDR » de l'app pour cet ordinateur (absent : `false`).
    #[serde(default)]
    pub hdr: bool,
}

impl SettingsJson {
    pub fn settings(&self) -> Result<StreamSettings, String> {
        let codec = Codec::parse(&self.codec).ok_or_else(|| format!("format inconnu : {}", self.codec))?;
        Ok(StreamSettings {
            profile: self.profile.as_deref().and_then(Profile::parse),
            codec,
            yuv444: self.yuv444,
            mbps: self.mbps,
            reliable: self.reliable,
            hdr: self.hdr,
        })
    }
}

/// Réglages d'une session.
#[derive(Debug, Clone)]
pub struct StreamSettings {
    pub profile: Option<Profile>,
    pub codec: Codec,
    pub yuv444: bool,
    /// Débit de base, calé sur la 4K à 60 Hz pour un profil.
    pub mbps: u32,
    /// Transport fiable (Bureau) ou rapide avec correction d'erreurs.
    pub reliable: bool,
    /// Le choix « HDR » de l'app pour cet ordinateur (docs/hdr.md), gardé
    /// par l'app (allumé par défaut sur un écran HDR). Hors du profil : il ne
    /// fait jamais passer en « Personnalisé ». La session ne s'en sert que si
    /// [`hdr_state`] le permet ; elle impose alors 4:2:0 et HEVC ou AV1.
    pub hdr: bool,
}

impl StreamSettings {
    /// Réglages d'un profil, ramenés à ce que l'appareil décode.
    pub fn for_profile(profile: Profile, caps: Capabilities) -> Self {
        let (yuv444, mut mbps, reliable) = match profile {
            Profile::Jeu => (false, 60, false),
            Profile::Bureau => (true, 90, true),
            Profile::Max => (true, 300, false),
        };
        let codec = if caps.hevc { Codec::Hevc } else { Codec::H264 };
        let mut yuv = yuv444;
        if yuv444 && !(codec == Codec::Hevc && caps.hevc444) {
            yuv = false;
            mbps += mbps / 3;
        }
        if codec == Codec::H264 {
            mbps += mbps / 2;
        }
        Self {
            profile: Some(profile),
            codec,
            yuv444: yuv,
            mbps,
            reliable,
            hdr: false,
        }
    }

    /// Réglages en JSON, comme les donnent les ponts des apps (C, JNI) :
    /// `profile` (`jeu`, `bureau`, `max` ; absent : personnalisé), `codec`
    /// (`h264`, `hevc`, `av1`), `yuv444`, `mbps`, `reliable`, `hdr`.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let wire: SettingsJson = serde_json::from_str(json).map_err(|e| e.to_string())?;
        wire.settings()
    }

    pub fn label(&self) -> &'static str {
        self.profile.map(Profile::label).unwrap_or("Personnalisé")
    }

    pub fn mode_policy(&self) -> &'static str {
        self.profile.map(Profile::mode_policy).unwrap_or("default")
    }

    /// Débit en bit/s pour un mode : un profil suit les pixels par seconde
    /// (puissance 0,75, entre 0,4 et 3 fois), un débit réglé à la main reste.
    pub fn bitrate(&self, width: u32, height: u32, refresh: u32) -> u32 {
        self.bitrate_for_pixel_rate(width as f64 * height as f64 * refresh.max(1) as f64)
    }

    /// Débit pour des écrans dont les pixels par seconde (largeur × hauteur ×
    /// fréquence) font ensemble `pixel_rate` : la même règle, avec les mêmes
    /// plafonds (multi-écrans : la somme des écrans de la session).
    pub fn bitrate_for_pixel_rate(&self, pixel_rate: f64) -> u32 {
        let mbps = if self.profile.is_some() {
            let ratio = pixel_rate.max(1.0) / (3840.0 * 2160.0 * 60.0);
            let scale = ratio.powf(0.75).clamp(0.4, 3.0);
            (self.mbps as f64 * scale).round() as u32
        } else {
            self.mbps
        };
        mbps.clamp(5, 4294).saturating_mul(1_000_000)
    }
}

/// Pourquoi le HDR n'est pas possible (docs/hdr.md, « Quand le HDR n'est pas
/// possible »), dans l'ordre où les raisons se vérifient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HdrReason {
    /// Station sans HDR (version d'avant, ou Admin réglé sur « Jamais »).
    Off,
    /// Windows d'avant Windows 11 24H2.
    Windows,
    /// Encodeur sans 10 bits (x264, carte trop ancienne).
    Encoder,
    /// Format vidéo réglé sur H.264.
    Codec,
    /// Le décodeur de l'appareil ne lit ni le HEVC 10 bits ni l'AV1 10 bits.
    Decoder,
    /// Sans écran virtuel, l'écran du PC est en SDR (Admin le dit par
    /// `reason: "display"`).
    Display,
}

impl HdrReason {
    /// La raison de `GET /api/stream/info` (`hdr.reason`) ; une raison
    /// inconnue compte comme « L'ordinateur ne propose pas le HDR. ».
    pub fn from_station(reason: Option<&str>) -> Self {
        match reason.map(str::trim) {
            Some("windows") => Self::Windows,
            Some("encoder") => Self::Encoder,
            Some("display") => Self::Display,
            _ => Self::Off,
        }
    }

    /// Clé du catalogue du cœur (`hdr.*`).
    pub fn key(self) -> &'static str {
        match self {
            Self::Off => "hdr.off",
            Self::Windows => "hdr.windows",
            Self::Encoder => "hdr.encoder",
            Self::Codec => "hdr.codec",
            Self::Decoder => "hdr.decoder",
            Self::Display => "hdr.display",
        }
    }

    /// Le texte de docs/link-ui.md (aide de la puce inactive), dans `lang`.
    pub fn text(self, lang: crate::i18n::Lang) -> String {
        crate::i18n::tr(lang, self.key(), &[])
    }
}

/// La puce « HDR » de l'accueil, la ligne de Réglages › Image et celle de la
/// barre ([`hdr_state`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HdrState {
    /// L'écran de l'appareil est HDR : la puce se montre (ailleurs, aucune
    /// trace du HDR).
    pub shown: bool,
    /// Le HDR est possible : la puce est active. Sinon elle est inactive,
    /// avec `reason`.
    pub available: bool,
    /// La raison, traduite (langue en cours de l'app), quand le HDR n'est
    /// pas possible.
    pub reason: Option<String>,
    /// La même raison, codée (`off`, `windows`, `encoder`, `codec`,
    /// `decoder`, `display`), pour une app qui la regarde.
    pub kind: Option<HdrReason>,
}

/// La puce « HDR » pour cet ordinateur (`info` : `GET /api/stream/info`),
/// ces réglages (seul le format compte : le choix `hdr` lui-même ne change
/// pas la puce), ce que l'appareil décode et son écran (`screen_hdr` :
/// l'écran de l'appareil est HDR). Raisons dans l'ordre de docs/hdr.md : la
/// Station, Windows, l'encodeur, le format H.264 choisi à la main (un profil
/// prend lui-même HEVC ou AV1 en HDR : seul un format sans profil compte),
/// le décodeur, l'écran du PC sans écran virtuel. La session passe par la
/// même règle pour envoyer `hdr` à `prepare` et à Kyber.
pub fn hdr_state(info: &crate::console::Info, settings: &StreamSettings, caps: Capabilities, screen_hdr: bool) -> HdrState {
    let kind = hdr_block(info.hdr.as_ref(), settings, caps);
    HdrState {
        shown: screen_hdr,
        available: kind.is_none(),
        reason: kind.map(|k| k.text(crate::i18n::current())),
        kind,
    }
}

/// La première raison qui empêche le HDR (`None` : possible). `station` :
/// `info.hdr` (absent : Station d'avant le HDR).
pub fn hdr_block(station: Option<&crate::console::HdrInfo>, settings: &StreamSettings, caps: Capabilities) -> Option<HdrReason> {
    match station {
        None => return Some(HdrReason::Off),
        Some(h) if !h.available => return Some(HdrReason::from_station(h.reason.as_deref())),
        Some(_) => {}
    }
    if settings.codec == Codec::H264 && settings.profile.is_none() {
        return Some(HdrReason::Codec);
    }
    (!caps.hdr).then_some(HdrReason::Decoder)
}

/// Le format d'une image HDR : 4:2:0 toujours ; « Auto » (le format d'un
/// profil) et H.264 passent à HEVC, ou à l'AV1 si l'appareil ne lit pas le
/// HEVC ; HEVC et AV1 restent.
pub fn hdr_format(settings: &StreamSettings, caps: Capabilities) -> StreamSettings {
    let mut s = settings.clone();
    s.yuv444 = false;
    if s.codec == Codec::H264 {
        s.codec = if !caps.hevc && caps.av1 { Codec::Av1 } else { Codec::Hevc };
    }
    s
}

/// « L'ordinateur n'a pas pu passer en HDR : image en SDR. » (avis
/// [`crate::session::Notice::HdrRefused`]), dans `lang`.
pub fn hdr_refused_text(lang: crate::i18n::Lang) -> String {
    crate::i18n::tr(lang, "hdr.refused", &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn puce_hdr_et_raisons_dans_l_ordre() {
        use crate::console::{HdrInfo, Info};
        use crate::i18n::Lang;
        let all = Capabilities { hevc: true, hevc444: true, av1: true, hdr: true };
        let jeu = StreamSettings::for_profile(Profile::Jeu, all);
        let on = Info { hdr: Some(HdrInfo { available: true, reason: None }), ..Info::default() };
        let state = hdr_state(&on, &jeu, all, true);
        assert_eq!((state.shown, state.available, state.reason.clone()), (true, true, None));
        assert!(!hdr_state(&on, &jeu, all, false).shown, "écran SDR : pas de puce");
        let station = |reason: Option<&str>| Some(HdrInfo { available: false, reason: reason.map(str::to_string) });
        let custom_h264 = StreamSettings { profile: None, codec: Codec::H264, ..jeu.clone() };
        let no_decoder = Capabilities { hdr: false, ..all };
        let cases = [
            (None, &jeu, all, HdrReason::Off),
            (station(Some("off")), &jeu, all, HdrReason::Off),
            (station(None), &jeu, all, HdrReason::Off),
            (station(Some("inconnue")), &jeu, all, HdrReason::Off),
            // La Station passe avant le format et le décodeur.
            (station(Some("windows")), &custom_h264, no_decoder, HdrReason::Windows),
            (station(Some("encoder")), &custom_h264, no_decoder, HdrReason::Encoder),
            (station(Some("display")), &jeu, all, HdrReason::Display),
            (Some(HdrInfo { available: true, reason: None }), &custom_h264, no_decoder, HdrReason::Codec),
            (Some(HdrInfo { available: true, reason: None }), &jeu, no_decoder, HdrReason::Decoder),
        ];
        for (hdr, settings, caps, want) in cases {
            let info = Info { hdr: hdr.clone(), ..Info::default() };
            let state = hdr_state(&info, settings, caps, true);
            assert_eq!((state.available, state.kind), (false, Some(want)), "{hdr:?}");
            assert!(state.reason.is_some());
        }
        // Un profil en H.264 (appareil sans HEVC) prend lui-même HEVC ou AV1.
        let h264_profile = StreamSettings { codec: Codec::H264, ..jeu.clone() };
        assert!(hdr_state(&on, &h264_profile, all, true).available);
        // Textes de link-ui, en français et en anglais.
        assert_eq!(HdrReason::Off.text(Lang::Fr), "L'ordinateur ne propose pas le HDR.");
        assert_eq!(HdrReason::Windows.text(Lang::Fr), "Le HDR demande Windows 11 24H2 sur l'ordinateur.");
        assert_eq!(HdrReason::Encoder.text(Lang::Fr), "La carte graphique de l'ordinateur ne fait pas le HDR.");
        assert_eq!(HdrReason::Codec.text(Lang::Fr), "Le HDR demande le format HEVC ou AV1.");
        assert_eq!(HdrReason::Decoder.text(Lang::Fr), "Cet appareil ne décode pas le HDR.");
        assert_eq!(HdrReason::Display.text(Lang::Fr), "L'écran de l'ordinateur n'est pas en HDR.");
        assert_eq!(HdrReason::Off.text(Lang::En), "The computer doesn't offer HDR.");
        assert_eq!(HdrReason::Codec.text(Lang::En), "HDR needs the HEVC or AV1 format.");
        assert_eq!(hdr_refused_text(Lang::Fr), "L'ordinateur n'a pas pu passer en HDR : image en SDR.");
        assert_eq!(hdr_refused_text(Lang::En), "The computer couldn't switch to HDR: picture in SDR.");
    }

    #[test]
    fn format_d_une_image_hdr() {
        let all = Capabilities { hevc: true, hevc444: true, av1: true, hdr: true };
        let bureau = StreamSettings::for_profile(Profile::Bureau, all);
        assert!(bureau.yuv444);
        let s = hdr_format(&bureau, all);
        assert_eq!((s.codec, s.yuv444, s.mbps, s.profile), (Codec::Hevc, false, bureau.mbps, Some(Profile::Bureau)));
        let h264 = StreamSettings { codec: Codec::H264, ..bureau.clone() };
        assert_eq!(hdr_format(&h264, all).codec, Codec::Hevc);
        assert_eq!(hdr_format(&h264, Capabilities { hevc: false, ..all }).codec, Codec::Av1, "AV1 seul");
        let av1 = StreamSettings { codec: Codec::Av1, ..bureau.clone() };
        assert_eq!(hdr_format(&av1, all).codec, Codec::Av1);
        // Le choix voyage en JSON, absent : éteint.
        assert!(StreamSettings::from_json(r#"{"codec":"hevc","mbps":60,"hdr":true}"#).unwrap().hdr);
        assert!(!StreamSettings::from_json(r#"{"codec":"hevc","mbps":60}"#).unwrap().hdr);
    }

    #[test]
    fn debits_comme_sur_mac() {
        let caps = Capabilities { hevc: true, hevc444: true, av1: false, hdr: false };
        let jeu = StreamSettings::for_profile(Profile::Jeu, caps);
        assert_eq!(jeu.bitrate(3840, 2160, 60), 60_000_000);
        assert_eq!(jeu.bitrate(2560, 1440, 60), 33_000_000);
        assert_eq!(jeu.bitrate(1920, 1080, 60), 24_000_000);
        assert_eq!(jeu.bitrate(3840, 2160, 120), 101_000_000);
        let bureau = StreamSettings::for_profile(Profile::Bureau, Capabilities { hevc: true, hevc444: false, av1: false, hdr: false });
        assert!(!bureau.yuv444);
        assert_eq!(bureau.mbps, 120);
        let h264 = StreamSettings::for_profile(Profile::Jeu, Capabilities::default());
        assert_eq!(h264.codec, Codec::H264);
        assert_eq!(h264.mbps, 90);
        // Plusieurs écrans : la somme des pixels par seconde, mêmes plafonds.
        let sum = 3840.0 * 2160.0 * 160.0 + 5120.0 * 2880.0 * 60.0 + 3456.0 * 2234.0 * 60.0;
        let b = jeu.bitrate_for_pixel_rate(sum);
        assert!(b > jeu.bitrate(3840, 2160, 160) && b <= 180_000_000, "{b}");
        assert_eq!(jeu.bitrate_for_pixel_rate(3840.0 * 2160.0 * 60.0), jeu.bitrate(3840, 2160, 60));
    }
}
