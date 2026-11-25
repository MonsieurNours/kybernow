//! Réglages synchronisés (docs/comptes.md, « Réglages synchronisés et
//! déplacer un ordinateur ») : le profil, le format vidéo, le débit, le
//! texte net (4:4:4), le HDR et le mode immersif de chaque ordinateur d'un
//! compte suivent la personne d'un appareil à l'autre.
//!
//! Le relais les garde par compte et par ordinateur, un petit objet JSON
//! (4 Ko au plus), le dernier changement l'emporte :
//!
//! - `GET /v1/settings` → `{"settings": [{"station", "updated_at", "values"}]}` ;
//! - `PUT /v1/settings/<station>` `{"values": {…}}` → `{"updated_at"}` ;
//!   refus : `404 unknown_station` ([`AccountError::UnknownComputer`] : l'app
//!   oublie les réglages gardés de cet ordinateur), `413 too_large`,
//!   `400 bad_values` ([`AccountError::Other`]), `401` (session fermée).
//!
//! Clés connues : `profile` (`jeu`, `bureau`, `max`, ou `null` :
//! personnalisé), `codec` (`h264`, `hevc`, `av1`, `pyrowave`), `yuv444`,
//! `mbps`, `hdr`, `immersive` (`fullscreen`, `always`, `never`). Une clé que
//! l'app ne connaît pas (écrite par une version plus récente) est gardée :
//! [`put_settings`] part des dernières valeurs lues et n'y change que ce que
//! l'app donne. Une valeur que l'appareil ne sait pas appliquer (format
//! qu'il ne décode pas…) reste au relais telle quelle ; l'appareil applique
//! ce qu'il peut ([`SyncedValues::apply_to`]).
//!
//! Ce que chaque app en fait (quand lire, quand écrire, quel réglage va
//! dans quelle clé) : apps/core/README.md, « Réglages synchronisés ».

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value, json};

use super::{Account, AccountError, RelayHttp, call, http, refusal};
use crate::profile::{Capabilities, Codec, Profile, StreamSettings};

/// Taille la plus grande des valeurs d'un ordinateur chez le relais (octets
/// du JSON de `values`).
pub const MAX_BYTES: usize = 4096;

/// Les valeurs du mode immersif (`immersive`) : plein écran seulement,
/// toujours, jamais.
pub const IMMERSIVE: [&str; 3] = ["fullscreen", "always", "never"];

/// Les réglages d'un ordinateur gardés par le relais, tels quels : `values`
/// garde aussi les clés que l'app ne connaît pas. À garder par l'app (avec
/// le compte) pour les relire hors ligne et pour [`put_settings`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SyncedSettings {
    /// L'identifiant de la Station (26 lettres).
    pub station: String,
    /// Dernier changement, en secondes Unix (horloge du relais).
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub values: Map<String, Value>,
}

/// Une valeur présente, même `null` (`Some(None)`) ; absente : `None`.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// Les clés connues, lues et vérifiées. `None` : absente (ou illisible pour
/// cet appareil). En JSON pour les ponts, les clés absentes sont omises et
/// `"profile": null` dit « Personnalisé ».
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SyncedValues {
    /// `Some(Some("jeu" | "bureau" | "max"))` : un profil ; `Some(None)` :
    /// personnalisé (format, 4:4:4 et débit réglés à la main).
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
    pub profile: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yuv444: Option<bool>,
    /// Débit, en Mbit/s (le débit de base d'un profil, calé sur la 4K à 60 Hz,
    /// ou celui réglé à la main).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdr: Option<bool>,
    /// `fullscreen`, `always` ou `never` ([`IMMERSIVE`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immersive: Option<String>,
}

impl SyncedSettings {
    /// Les clés connues, vérifiées : une valeur illisible (profil ou format
    /// inconnus, mode immersif d'une version plus récente) compte comme
    /// absente, et reste au relais.
    pub fn known(&self) -> SyncedValues {
        let v = &self.values;
        let profile = match v.get("profile") {
            Some(Value::Null) => Some(None),
            Some(Value::String(p)) => Profile::parse(p).map(|p| Some(p.key().to_string())),
            _ => None,
        };
        let codec = v.get("codec").and_then(Value::as_str).and_then(Codec::parse).map(|c| c.key().to_string());
        let mbps = v.get("mbps").and_then(Value::as_f64).filter(|m| m.is_finite() && *m >= 1.0 && *m <= 4294.0).map(|m| m.round() as u32);
        let immersive = v.get("immersive").and_then(Value::as_str).filter(|i| IMMERSIVE.contains(i)).map(str::to_string);
        SyncedValues {
            profile,
            codec,
            yuv444: v.get("yuv444").and_then(Value::as_bool),
            mbps,
            hdr: v.get("hdr").and_then(Value::as_bool),
            immersive,
        }
    }
}

impl SyncedValues {
    /// Rien à appliquer ni à envoyer.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Le profil lu : `Some(None)` personnalisé, `None` absent.
    pub fn profile(&self) -> Option<Option<Profile>> {
        match &self.profile {
            Some(None) => Some(None),
            Some(Some(p)) => Profile::parse(p).map(Some),
            None => None,
        }
    }

    pub fn codec(&self) -> Option<Codec> {
        self.codec.as_deref().and_then(Codec::parse)
    }

    /// Les clés de réglages d'une session (`profile`, `codec`, `yuv444`,
    /// `mbps`, `hdr`) d'après les réglages de l'app pour cet ordinateur ;
    /// sans `immersive` ([`Self::with_immersive`]).
    pub fn from_stream(s: &StreamSettings) -> Self {
        Self {
            profile: Some(s.profile.map(|p| p.key().to_string())),
            codec: Some(s.codec.key().to_string()),
            yuv444: Some(s.yuv444),
            mbps: Some(s.mbps),
            hdr: Some(s.hdr),
            immersive: None,
        }
    }

    /// Avec le mode immersif (`fullscreen`, `always`, `never` ; une autre
    /// valeur est ignorée).
    pub fn with_immersive(mut self, immersive: Option<&str>) -> Self {
        self.immersive = immersive.filter(|i| IMMERSIVE.contains(i)).map(str::to_string);
        self
    }

    /// Les valeurs données posées dans `raw` (les autres clés restent).
    pub fn merge_into(&self, raw: &mut Map<String, Value>) {
        if let Some(p) = &self.profile {
            raw.insert("profile".into(), p.as_ref().map_or(Value::Null, |p| Value::String(p.clone())));
        }
        if let Some(c) = &self.codec {
            raw.insert("codec".into(), json!(c));
        }
        if let Some(y) = self.yuv444 {
            raw.insert("yuv444".into(), json!(y));
        }
        if let Some(m) = self.mbps {
            raw.insert("mbps".into(), json!(m));
        }
        if let Some(h) = self.hdr {
            raw.insert("hdr".into(), json!(h));
        }
        if let Some(i) = &self.immersive {
            raw.insert("immersive".into(), json!(i));
        }
    }

    /// Les réglages de session de l'app (`base` : ceux qu'elle a pour cet
    /// ordinateur) avec les valeurs synchronisées, ramenés à ce que
    /// l'appareil décode (`caps`) :
    ///
    /// - un profil : ses réglages pour cet appareil
    ///   ([`StreamSettings::for_profile`]) ; le format, le 4:4:4 et le débit
    ///   reçus ne servent pas (ils suivent le profil sur chaque appareil) ;
    /// - personnalisé (ou profil absent et `base` personnalisé) : le format
    ///   s'il se décode ici, sinon celui de `base` ; le 4:4:4 s'il existe
    ///   dans ce format sur cet appareil ; le débit ;
    /// - le HDR reçu, tel quel (la session ne s'en sert que si l'écran et
    ///   l'ordinateur le permettent).
    ///
    /// `reliable` et `wired` restent ceux de `base`.
    pub fn apply_to(&self, base: &StreamSettings, caps: Capabilities) -> StreamSettings {
        let mut s = match self.profile() {
            Some(Some(p)) => StreamSettings { hdr: base.hdr, wired: base.wired, ..StreamSettings::for_profile(p, caps) },
            Some(None) => StreamSettings { profile: None, ..base.clone() },
            None => base.clone(),
        };
        if s.profile.is_none() {
            if let Some(c) = self.codec().filter(|c| c.decodable(caps)) {
                s.codec = c;
            }
            if let Some(y) = self.yuv444 {
                s.yuv444 = y;
            }
            let can444 = match s.codec {
                Codec::Hevc => caps.hevc444,
                Codec::PyroWave => caps.pyrowave,
                _ => false,
            };
            s.yuv444 &= can444;
            if let Some(m) = self.mbps {
                s.mbps = m;
            }
        }
        if let Some(h) = self.hdr {
            s.hdr = h;
        }
        s
    }
}

/// Les réglages d'un ordinateur dans une liste lue par [`fetch_settings`].
pub fn find<'a>(list: &'a [SyncedSettings], station: &str) -> Option<&'a SyncedSettings> {
    list.iter().find(|s| s.station == station)
}

/// Les réglages de tous les ordinateurs du compte (`GET /v1/settings`).
/// [`AccountError::SignedOut`] : oublier le compte ;
/// [`AccountError::NoAccounts`] : relais d'avant les réglages synchronisés
/// (ou sans comptes), l'app garde ses réglages locaux. Bloquant.
pub fn fetch_settings(account: &Account) -> Result<Vec<SyncedSettings>, AccountError> {
    fetch_settings_with(&*http(), account)
}

pub(super) fn fetch_settings_with(http: &dyn RelayHttp, account: &Account) -> Result<Vec<SyncedSettings>, AccountError> {
    let addr = account.addr()?;
    let r = call(http, &addr, "GET", "/v1/settings", Some(&account.token), None)?;
    if r.status != 200 {
        return Err(refusal(&r, true));
    }
    let list = r.body.get("settings").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list
        .into_iter()
        .filter_map(|s| serde_json::from_value::<SyncedSettings>(s).ok())
        .filter(|s| kybernow_tunnel::id::is_station_id(&s.station))
        .collect())
}

/// Envoie les réglages d'un ordinateur (`PUT /v1/settings/<station>`) :
/// `values` posées sur `last` (les dernières valeurs lues ou envoyées pour
/// cet ordinateur, `None` s'il n'y en a pas), les autres clés gardées.
/// Rend les réglages à garder à la place de `last`. Bloquant.
pub fn put_settings(account: &Account, station: &str, last: Option<&SyncedSettings>, values: &SyncedValues) -> Result<SyncedSettings, AccountError> {
    put_settings_with(&*http(), account, station, last, values)
}

pub(super) fn put_settings_with(
    http: &dyn RelayHttp,
    account: &Account,
    station: &str,
    last: Option<&SyncedSettings>,
    values: &SyncedValues,
) -> Result<SyncedSettings, AccountError> {
    if !kybernow_tunnel::id::is_station_id(station) {
        return Err(AccountError::UnknownComputer);
    }
    let mut raw = last.filter(|l| l.station == station).map(|l| l.values.clone()).unwrap_or_default();
    values.merge_into(&mut raw);
    let addr = account.addr()?;
    let body = json!({ "values": Value::Object(raw.clone()) });
    let r = call(http, &addr, "PUT", &format!("/v1/settings/{station}"), Some(&account.token), Some(&body))?;
    if r.status != 200 {
        let e = refusal(&r, true);
        log::warn!("comptes : réglages de {station} pas envoyés ({})", e.kind());
        return Err(e);
    }
    let updated_at = r.body.get("updated_at").and_then(Value::as_u64).unwrap_or_else(super::now_secs);
    Ok(SyncedSettings { station: station.to_string(), updated_at, values: raw })
}
