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

use super::{Account, AccountError, RelayHttp, Reply, call, http, refusal};
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
    /// `Some(Some("jeu" | "bureau" | "max" | "auto"))` : un profil ; `Some(None)` :
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
    /// - Auto : ceux de `base` (le test est propre à chaque appareil) ;
    /// - un autre profil : ses réglages pour cet appareil
    ///   ([`StreamSettings::for_profile`]) ; le format, le 4:4:4 et le débit
    ///   reçus ne servent pas (ils suivent le profil sur chaque appareil) ;
    /// - personnalisé (ou profil absent et `base` personnalisé) : le format
    ///   s'il se décode ici, sinon celui de `base` ; le 4:4:4 s'il existe
    ///   dans ce format sur cet appareil ; le débit ;
    /// - le HDR reçu, tel quel (la session ne s'en sert que si l'écran et
    ///   l'ordinateur le permettent).
    ///
    /// `reliable`, `wired` et `encoder_max` (plafond de l'encodeur, propre à
    /// cet appareil) restent ceux de `base`.
    pub fn apply_to(&self, base: &StreamSettings, caps: Capabilities) -> StreamSettings {
        let mut s = match self.profile() {
            // Auto : les réglages viennent du test de chaque appareil
            // (docs/profil-auto.md) ; reçu d'un autre appareil, on garde ceux
            // de cet appareil pour cet ordinateur.
            Some(Some(Profile::Auto)) => base.clone(),
            Some(Some(p)) => StreamSettings { hdr: base.hdr, wired: base.wired, encoder_max: base.encoder_max, ..StreamSettings::for_profile(p, caps) },
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
            // Ramené aux bornes du débit à la main (un autre appareil a pu en
            // garder un plus haut, d'avant les bornes communes : TRANS-41).
            if let Some(m) = self.mbps {
                s.mbps = kybernow_rules::limits::clamp_manual_mbps(m, kybernow_rules::limits::Device::Computer, s.codec == Codec::PyroWave);
            }
        }
        if let Some(h) = self.hdr {
            s.hdr = h;
        }
        s
    }
}

impl SyncedValues {
    /// Ces valeurs, avec celles de `top` par-dessus (une clé de `top` gagne).
    pub fn overlay(&self, top: &SyncedValues) -> SyncedValues {
        SyncedValues {
            profile: top.profile.clone().or_else(|| self.profile.clone()),
            codec: top.codec.clone().or_else(|| self.codec.clone()),
            yuv444: top.yuv444.or(self.yuv444),
            mbps: top.mbps.or(self.mbps),
            hdr: top.hdr.or(self.hdr),
            immersive: top.immersive.clone().or_else(|| self.immersive.clone()),
        }
    }

    /// Retire les clés que `sent` a envoyées avec la même valeur (une clé
    /// changée depuis reste à envoyer).
    pub fn remove_sent(&mut self, sent: &SyncedValues) {
        fn drop_same<T: PartialEq>(mine: &mut Option<T>, sent: &Option<T>) {
            if sent.is_some() && mine == sent {
                *mine = None;
            }
        }
        drop_same(&mut self.profile, &sent.profile);
        drop_same(&mut self.codec, &sent.codec);
        drop_same(&mut self.yuv444, &sent.yuv444);
        drop_same(&mut self.mbps, &sent.mbps);
        drop_same(&mut self.hdr, &sent.hdr);
        drop_same(&mut self.immersive, &sent.immersive);
    }

    /// Ces valeurs sans les clés encore en attente d'envoi (`pending`) :
    /// reçues du relais, elles ne remplacent pas un changement de l'appareil
    /// pas encore parti.
    pub fn without(&self, pending: &SyncedValues) -> SyncedValues {
        let mut v = self.clone();
        if pending.profile.is_some() {
            v.profile = None;
        }
        if pending.codec.is_some() {
            v.codec = None;
        }
        if pending.yuv444.is_some() {
            v.yuv444 = None;
        }
        if pending.mbps.is_some() {
            v.mbps = None;
        }
        if pending.hdr.is_some() {
            v.hdr = None;
        }
        if pending.immersive.is_some() {
            v.immersive = None;
        }
        v
    }
}

/// Les clés de session que la personne vient de changer (docs/comptes.md,
/// « Envoyer ») : un profil → `{profile}` ; « Personnalisé » (un réglage du
/// profil retouché) → `{profile: null, codec, yuv444, mbps}` ; le format,
/// le 4:4:4 ou le débit d'un réglage déjà personnalisé → la clé changée et
/// `profile: null`. `before` et `after` : les choix de l'utilisateur ;
/// `*_eff` : les réglages que la session prendrait ([`crate::plan::plan`]).
/// Le HDR et le mode immersif sont ajoutés par l'app quand ils changent.
pub fn changed(before: &crate::plan::Choices, after: &crate::plan::Choices, before_eff: &StreamSettings, after_eff: &StreamSettings) -> SyncedValues {
    let mut v = SyncedValues::default();
    match (before_eff.profile, after_eff.profile) {
        (_, Some(p)) => {
            if before_eff.profile != Some(p) {
                v.profile = Some(Some(p.key().to_string()));
            }
        }
        (Some(_), None) => {
            v.profile = Some(None);
            v.codec = Some(after_eff.codec.key().to_string());
            v.yuv444 = Some(after_eff.yuv444);
            v.mbps = Some(after_eff.mbps);
        }
        (None, None) => {
            if before.codec != after.codec || before_eff.codec != after_eff.codec {
                v.codec = Some(after_eff.codec.key().to_string());
            }
            if before.yuv444 != after.yuv444 || before_eff.yuv444 != after_eff.yuv444 {
                v.yuv444 = Some(after_eff.yuv444);
            }
            if before.mbps != after.mbps {
                v.mbps = Some(after_eff.mbps);
            }
            if !v.is_empty() {
                v.profile = Some(None);
            }
        }
    }
    v
}

/// Ce que l'app garde des réglages synchronisés d'un compte (WINAPP-55,
/// LINAPP-40 ; avant, une copie dans l'app Windows et une dans l'app Linux) :
/// la dernière liste lue ou envoyée, ce qui est déjà appliqué, et ce qui
/// attend d'être envoyé, par Station. En JSON, gardé par l'app (un par
/// compte).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountSync {
    /// La dernière liste lue (ou envoyée), clés inconnues comprises.
    pub list: Vec<SyncedSettings>,
    /// Par Station : le `updated_at` déjà appliqué (ou envoyé) sur cet appareil.
    pub applied: std::collections::HashMap<String, u64>,
    /// Par Station : les changements que le relais n'a pas reçus.
    pub pending: std::collections::HashMap<String, SyncedValues>,
}

impl AccountSync {
    /// Les dernières valeurs connues d'un ordinateur (pour [`put_settings`]).
    pub fn last(&self, station: &str) -> Option<&SyncedSettings> {
        find(&self.list, station)
    }

    /// Ce qu'il faut envoyer pour ce changement : ce qui attend encore, puis
    /// le changement ; la première fois qu'un ordinateur part au relais (rien
    /// de lu ni d'envoyé), tout (`full`, par exemple
    /// `SyncedValues::from_stream` des réglages de la session, avec le mode
    /// immersif).
    pub fn to_send(&self, station: &str, change: &SyncedValues, full: impl FnOnce() -> SyncedValues) -> SyncedValues {
        let waiting = self.pending.get(station).cloned().unwrap_or_default();
        let base = if self.last(station).is_none() { full().overlay(&waiting) } else { waiting };
        base.overlay(change)
    }

    /// Envoyé ([`put_settings`] a rendu `s`) : plus rien n'attend, et ce
    /// changement ne revient pas comme une nouveauté.
    pub fn sent(&mut self, s: SyncedSettings) {
        self.pending.remove(&s.station);
        self.applied.insert(s.station.clone(), s.updated_at);
        match self.list.iter_mut().find(|x| x.station == s.station) {
            Some(x) => *x = s,
            None => self.list.push(s),
        }
    }

    /// Pas envoyé (hors ligne, relais muet) : gardé pour le prochain envoi.
    pub fn unsent(&mut self, station: &str, values: &SyncedValues) {
        let waiting = self.pending.get(station).cloned().unwrap_or_default();
        self.pending.insert(station.to_string(), waiting.overlay(values));
    }

    /// L'ordinateur n'est plus dans le compte (`AccountError::UnknownComputer`).
    pub fn forget(&mut self, station: &str) {
        self.list.retain(|s| s.station != station);
        self.applied.remove(station);
        self.pending.remove(station);
    }

    /// Une liste lue ([`fetch_settings`]) : rend, par Station, les valeurs
    /// plus récentes que celles déjà appliquées, à appliquer
    /// ([`SyncedValues::apply_to`]) ; une Station dont un changement attend
    /// d'être envoyé garde le sien. Les Stations sorties du compte sont
    /// oubliées (sauf un changement fait avant qu'elles y soient).
    pub fn fetched(&mut self, list: Vec<SyncedSettings>) -> Vec<(String, SyncedValues)> {
        self.applied.retain(|station, _| find(&list, station).is_some());
        let old = std::mem::take(&mut self.list);
        self.pending.retain(|station, _| find(&list, station).is_some() || find(&old, station).is_none());
        let mut out = Vec::new();
        for s in &list {
            if self.pending.contains_key(&s.station) {
                continue;
            }
            if s.updated_at > self.applied.get(&s.station).copied().unwrap_or(0) {
                self.applied.insert(s.station.clone(), s.updated_at);
                let values = s.known();
                if !values.is_empty() {
                    out.push((s.station.clone(), values));
                }
            }
        }
        self.list = list;
        out
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
    // Trop grand pour le relais : refusé ici, comme il le ferait (413).
    if serde_json::to_vec(&raw).map_or(0, |v| v.len()) > MAX_BYTES {
        log::warn!("comptes : réglages de {station} trop grands pour le relais, pas envoyés");
        return Err(refusal(&Reply { status: 413, body: json!({ "error": "too_large" }), retry_after: None }, true));
    }
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
