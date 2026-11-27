//! Réglages synchronisés par le compte, côté interface (docs/comptes.md, « Réglages synchronisés et déplacer un
//! ordinateur » ; ce que fait chaque app : apps/core/README.md, « Réglages synchronisés »). Sans le cœur : le pilote
//! (`device/accounts.rs`) lit et envoie par `account::fetch_settings` et `put_settings`, et tient la mémoire des envois
//! avec `account::AccountSync` du cœur (LINAPP-40) ; l'interface la garde dans le même JSON (`SyncMemory`, dans
//! `kybernow.json`) et pose ici les valeurs reçues dans les réglages des ordinateurs (`apply`).
//!
//! - **Ce qui suit la personne** : le profil (`jeu`, `bureau`, `max`, `auto` ; « Personnalisé » → `null`), le format
//!   vidéo (`codec`), le texte net (`yuv444`), le débit (`mbps`, Mbit/s), le HDR (`hdr`) et le mode immersif
//!   (`immersive`).
//! - **Envoyer** : à chaque réglage gardé (`Action::Save`), `App::sync_changes` compare chaque ordinateur d'un compte à
//!   ce qui a été lu, appliqué ou envoyé (`AccountsUi::sync_base`) et rend les clés changées (et toutes, pour un
//!   premier envoi) ; le pilote les pose sur ce qui attend encore (`AccountSync::to_send`, `unsent`) et les envoie.
//! - **Lire** : au lancement, à chaque rafraîchissement des comptes et juste avant une session ; ce qui est plus récent
//!   que ce qui a déjà été appliqué (`AccountSync::fetched`, une clé encore en attente gardant la valeur de
//!   l'appareil) passe dans les réglages de l'ordinateur (`App::sync_apply`), sans être renvoyé.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::app::{snap_mbps, App, Machine};
use crate::model::{Host, HostSettings, Profile, Store};

/// Une valeur présente, même `null` (`Some(None)`) ; absente : `None`.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

/// Les clés synchronisées d'un ordinateur, toutes facultatives : le JSON de `account::SyncedValues` du cœur (les
/// absentes omises, `"profile": null` pour « Personnalisé »).
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SyncKeys {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present")]
    pub profile: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yuv444: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immersive: Option<String>,
}

impl SyncKeys {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Les réglages d'un ordinateur tels que le relais les rend (`account::SyncedSettings` du cœur, même JSON) : clés
/// inconnues comprises, la base de l'envoi suivant.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncedRaw {
    /// La Station (26 lettres).
    pub station: String,
    /// Dernier changement chez le relais (secondes Unix, horloge du relais).
    pub updated_at: u64,
    pub values: Map<String, Value>,
}

/// La mémoire des réglages synchronisés d'un compte : le JSON de `account::AccountSync` du cœur, qui seul la change
/// (le pilote) ; l'interface la garde dans `kybernow.json`.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncMemory {
    /// La dernière liste lue (ou envoyée).
    pub list: Vec<SyncedRaw>,
    /// Par Station : le `updated_at` déjà appliqué (ou envoyé) sur cet appareil.
    pub applied: HashMap<String, u64>,
    /// Par Station : les changements que le relais n'a pas reçus.
    pub pending: HashMap<String, SyncKeys>,
}

/// La mémoire d'un compte (`KnAccount::key`).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountMemory {
    pub account: String,
    #[serde(flatten)]
    pub memory: SyncMemory,
}

/// La mémoire d'avant la 0.11.8 (`synced`, `sync_pending` de `kybernow.json`), lue une fois et reprise dans
/// `SyncMemory` (`Store::adopt_legacy_sync`).
#[derive(Clone, Debug, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct LegacySynced {
    pub account: String,
    pub station: String,
    pub updated_at: u64,
    pub values: Map<String, Value>,
    pub applied: u64,
}

#[derive(Clone, Debug, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct LegacyPending {
    pub account: String,
    pub station: String,
    pub keys: SyncKeys,
}

/// Un changement à envoyer : les clés changées (`keys`), et toutes (`full`) pour un ordinateur que le relais n'a
/// jamais reçu (`AccountSync::to_send` choisit).
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub account: String,
    pub station: String,
    pub keys: SyncKeys,
    pub full: SyncKeys,
}

/// Ce que l'app compare d'un réglage gardé à l'autre pour savoir quoi envoyer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tracked {
    pub custom: bool,
    pub profile: Profile,
    pub codec: String,
    pub mbps: u32,
    pub yuv444: bool,
    pub hdr: bool,
    pub immersive: String,
}

impl Tracked {
    pub fn of(s: &HostSettings) -> Self {
        Self { custom: s.custom(), profile: s.profile, codec: s.codec.clone(), mbps: s.mbps, yuv444: s.yuv444, hdr: s.hdr, immersive: s.immersive.clone() }
    }
}

/// Le format et le débit réels d'un réglage « Personnalisé » : le format choisi, ou celui qu'« Auto » prend sur cet
/// appareil ; le débit choisi, ou celui du profil au mode de la session (en Mbit/s).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effective {
    pub codec: String,
    pub mbps: u32,
}

/// Toutes les clés (un ordinateur sans réglages au relais, la première fois).
pub fn full_keys(t: &Tracked, e: &Effective) -> SyncKeys {
    let mut k = if t.custom {
        SyncKeys { profile: Some(None), codec: Some(e.codec.clone()), yuv444: Some(t.yuv444), mbps: Some(e.mbps), ..Default::default() }
    } else {
        SyncKeys { profile: Some(Some(t.profile.id().into())), ..Default::default() }
    };
    k.hdr = Some(t.hdr);
    k.immersive = Some(t.immersive.clone());
    k
}

/// Seulement les clés changées : un profil → `{profile}` ; passer en « Personnalisé » → `{profile: null, codec,
/// yuv444, mbps}` ; le format, le 4:4:4 ou le débit d'un réglage personnalisé → la clé et `profile: null` ; le HDR →
/// `{hdr}` ; le mode immersif → `{immersive}`.
pub fn changed_keys(old: &Tracked, new: &Tracked, e: &Effective) -> SyncKeys {
    let mut k = SyncKeys::default();
    if new.custom {
        if !old.custom {
            k = SyncKeys { profile: Some(None), codec: Some(e.codec.clone()), yuv444: Some(new.yuv444), mbps: Some(e.mbps), ..Default::default() };
        } else {
            if old.codec != new.codec {
                k.codec = Some(e.codec.clone());
            }
            if old.yuv444 != new.yuv444 {
                k.yuv444 = Some(new.yuv444);
            }
            if old.mbps != new.mbps {
                k.mbps = Some(e.mbps);
            }
            if !k.is_empty() {
                k.profile = Some(None);
            }
        }
    } else if old.custom || old.profile != new.profile {
        k.profile = Some(Some(new.profile.id().into()));
    }
    if old.hdr != new.hdr {
        k.hdr = Some(new.hdr);
    }
    if old.immersive != new.immersive {
        k.immersive = Some(new.immersive.clone());
    }
    k
}

/// Le format se décode ici (`Codec::decodable` du cœur, vu de l'app).
fn decodable(m: &Machine, codec: &str) -> bool {
    match codec {
        "h264" => true,
        "hevc" => m.decodes("hevc"),
        "av1" => m.decodes("av1"),
        "pyrowave" => m.pyrowave,
        _ => false,
    }
}

/// Les valeurs reçues posées dans les réglages de l'ordinateur, comme `SyncedValues::apply_to` du cœur : un profil
/// prend ses réglages sur cet appareil (ce qui s'en écartait revient au profil, comme le choisir ici) ;
/// « Personnalisé » : le format s'il se décode ici, le 4:4:4 s'il existe dans ce format, le débit ; le HDR et le mode
/// immersif tels quels. Rend vrai si quelque chose a changé. Auto reçu d'un autre appareil : les réglages du test de
/// cet appareil (docs/profil-auto.md) ; sans test fait ici, le profil reste celui de cet appareil.
pub fn apply(s: &mut HostSettings, k: &SyncKeys, m: &Machine) -> bool {
    let before = s.clone();
    match &k.profile {
        Some(Some(p)) => {
            if let Some(p) = Profile::ALL.into_iter().find(|x| x.id() == p).filter(|p| *p != Profile::Auto || s.auto_view().is_some()) {
                s.pick_profile(p);
            }
        }
        Some(None) => {}
        None => {}
    }
    let custom = matches!(k.profile, Some(None)) || (k.profile.is_none() && s.custom());
    if custom {
        if let Some(c) = k.codec.as_deref().filter(|c| decodable(m, c)) {
            s.codec = c.to_string();
        }
        if let Some(y) = k.yuv444 {
            let codec = if s.codec == "auto" && m.decodes("hevc") { "hevc" } else { s.codec.as_str() };
            s.yuv444 = y && matches!(codec, "hevc" | "pyrowave");
        }
        if let Some(mbps) = k.mbps {
            s.mbps = snap_mbps(mbps, s.codec == "pyrowave");
        }
    }
    if let Some(h) = k.hdr {
        s.hdr = h;
    }
    if let Some(i) = k.immersive.as_deref().filter(|i| crate::app::IMMERSIVE.contains(i)) {
        s.immersive = i.to_string();
    }
    *s != before
}

impl Store {
    /// Le compte et la Station par lesquels un ordinateur de l'accueil se synchronise : un ordinateur d'un compte, ou
    /// un ordinateur appairé dont la Station est aussi dans un compte (`merge` du cœur : même Station).
    pub fn sync_target(&self, host: &str) -> Option<(String, String)> {
        if let Some(a) = self.account_host(host) {
            return self.account(&a.account).map(|_| (a.account.clone(), a.station.clone()));
        }
        let station = self.host(host)?.relay.as_ref().map(|r| r.id.clone()).filter(|s| !s.is_empty())?;
        self.account_computers.iter().find(|c| c.station == station && self.account(&c.account).is_some()).map(|c| (c.account.clone(), station))
    }

    /// Les ordinateurs de l'accueil qui suivent les réglages de cette Station pour ce compte.
    pub fn sync_hosts(&self, account: &str, station: &str) -> Vec<String> {
        self.hosts.iter().filter(|h| self.sync_target(&h.id).is_some_and(|(a, s)| a == account && s == station)).map(|h| h.id.clone()).collect()
    }

    /// La mémoire des réglages synchronisés d'un compte (vide s'il n'en a pas encore).
    pub fn sync_memory(&self, account: &str) -> SyncMemory {
        self.account_sync.iter().find(|m| m.account == account).map(|m| m.memory.clone()).unwrap_or_default()
    }

    /// Remplace la mémoire d'un compte (vide : retirée).
    pub fn set_sync_memory(&mut self, account: &str, memory: SyncMemory) {
        self.account_sync.retain(|m| m.account != account);
        if memory != SyncMemory::default() {
            self.account_sync.push(AccountMemory { account: account.to_string(), memory });
        }
    }

    /// Un compte oublié : ce qu'il gardait pour la synchronisation aussi.
    pub fn sync_forget_account(&mut self, account: &str) {
        self.account_sync.retain(|m| m.account != account);
    }

    /// La mémoire d'avant la 0.11.8 reprise dans celle du cœur (une fois, au chargement) : rien d'appliqué n'est
    /// réappliqué, rien d'attendu n'est perdu.
    pub fn adopt_legacy_sync(&mut self) {
        let (synced, pending) = (std::mem::take(&mut self.legacy_synced), std::mem::take(&mut self.legacy_sync_pending));
        for e in synced {
            let mut m = self.sync_memory(&e.account);
            m.list.retain(|r| r.station != e.station);
            m.list.push(SyncedRaw { station: e.station.clone(), updated_at: e.updated_at, values: e.values });
            m.applied.insert(e.station, e.applied);
            self.set_sync_memory(&e.account, m);
        }
        for p in pending.into_iter().filter(|p| !p.keys.is_empty()) {
            let mut m = self.sync_memory(&p.account);
            m.pending.insert(p.station, p.keys);
            self.set_sync_memory(&p.account, m);
        }
    }
}

impl App {
    /// La synchronisation joue : l'interrupteur allumé et un compte connecté.
    pub fn sync_on(&self) -> bool {
        self.store.app.sync_settings && !self.store.accounts.is_empty()
    }

    /// Ce qui est gardé à présent devient la base des comparaisons (au lancement) : rien n'est envoyé.
    pub fn sync_baseline(&mut self) {
        self.accounts.sync_base = self.store.hosts.iter().map(|h| (h.id.clone(), Tracked::of(&h.settings))).collect();
    }

    /// Après un réglage gardé : les clés changées de chaque ordinateur d'un compte (et toutes, pour un premier envoi),
    /// à poser sur ce qui attend encore (le pilote, `AccountSync::to_send`). Interrupteur coupé : la base suit, rien ne
    /// part. `auto_mbps` : le débit du profil au mode de la session (Mbit/s), pour un réglage personnalisé dont le
    /// débit est « Auto ».
    pub fn sync_changes(&mut self, auto_mbps: &dyn Fn(&App, &Host) -> u32) -> Vec<Change> {
        let on = self.sync_on();
        let mut changes: Vec<Change> = Vec::new();
        let mut base: HashMap<String, Tracked> = HashMap::new();
        for h in &self.store.hosts {
            let now = Tracked::of(&h.settings);
            let old = self.accounts.sync_base.get(&h.id);
            if on
                && let Some(old) = old
                && *old != now
                && let Some((account, station)) = self.store.sync_target(&h.id)
            {
                let codec = if h.settings.codec == "auto" { self.codec_for(h).to_string() } else { h.settings.codec.clone() };
                let mbps = if h.settings.mbps == 0 { auto_mbps(self, h) } else { h.settings.mbps };
                let e = Effective { codec, mbps };
                let keys = changed_keys(old, &now, &e);
                if !keys.is_empty() {
                    changes.push(Change { account, station, keys, full: full_keys(&now, &e) });
                }
            }
            base.insert(h.id.clone(), now);
        }
        self.accounts.sync_base = base;
        changes
    }

    /// Les valeurs reçues pour un ordinateur d'un compte (plus récentes que ce qui était appliqué, sans les clés encore
    /// en attente d'envoi : `AccountSync::fetched` du pilote) posées dans les réglages des ordinateurs de l'accueil qui
    /// le suivent, sans être renvoyées (la base suit). Rend vrai si un réglage a changé.
    pub fn sync_apply(&mut self, account: &str, station: &str, keys: &SyncKeys) -> bool {
        let mut changed = false;
        for id in self.store.sync_hosts(account, station) {
            let machine = &self.machine;
            if let Some(h) = self.store.hosts.iter_mut().find(|h| h.id == id) {
                if apply(&mut h.settings, keys, machine) {
                    log::info!("comptes : réglages de {} reçus ({station})", h.name);
                    changed = true;
                }
                self.accounts.sync_base.insert(id.clone(), Tracked::of(&h.settings));
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountHost, HostRelay, KnAccount, KnComputer, KnUser};

    const STATION: &str = "abcdefghijklmnopqrstuvwxyz";

    fn host(id: &str, station: &str) -> Host {
        Host {
            id: id.into(),
            name: "Tour".into(),
            address: String::new(),
            port: 0,
            fingerprint: None,
            user: String::new(),
            password: String::new(),
            os: None,
            gpu: None,
            av1: false,
            pyrowave: false,
            mac: None,
            wake: Vec::new(),
            relay: Some(HostRelay { server: "share.kybernow.app".into(), id: station.into(), remote_access: true }),
            settings: HostSettings::default(),
            last: None,
        }
    }

    fn app() -> App {
        let mut app = App::new(Store::default(), crate::app::Layout::Desktop);
        let account = KnAccount { relay: "share.kybernow.app".into(), account: KnUser { id: "u1".into(), ..Default::default() }, ..Default::default() };
        let key = account.key();
        app.store.accounts.push(account);
        app.store.account_hosts.push(AccountHost { id: "h1".into(), account: key.clone(), relay: "share.kybernow.app".into(), station: STATION.into() });
        app.store.hosts.push(host("h1", STATION));
        app.sync_baseline();
        app
    }

    fn key(app: &App) -> String {
        app.store.accounts[0].key()
    }

    fn auto(_: &App, _: &Host) -> u32 {
        42
    }

    #[test]
    fn json_comme_le_coeur() {
        let k = SyncKeys { profile: Some(None), codec: Some("hevc".into()), mbps: Some(80), ..Default::default() };
        assert_eq!(serde_json::to_string(&k).unwrap(), r#"{"profile":null,"codec":"hevc","mbps":80}"#);
        let back: SyncKeys = serde_json::from_str(r#"{"profile":null,"hdr":true}"#).unwrap();
        assert_eq!(back, SyncKeys { profile: Some(None), hdr: Some(true), ..Default::default() });
        let none: SyncKeys = serde_json::from_str("{}").unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn cles_changees() {
        let base = Tracked::of(&HostSettings::default());
        let e = Effective { codec: "hevc".into(), mbps: 42 };
        // Un profil : lui seul.
        let mut s = HostSettings::default();
        s.pick_profile(Profile::Bureau);
        assert_eq!(changed_keys(&base, &Tracked::of(&s), &e), SyncKeys { profile: Some(Some("bureau".into())), ..Default::default() });
        // Passer en personnalisé : profil nul, format, 4:4:4 et débit réels.
        let mut s = HostSettings::default();
        s.mbps = 80;
        let e80 = Effective { codec: "hevc".into(), mbps: 80 };
        assert_eq!(changed_keys(&base, &Tracked::of(&s), &e80), SyncKeys { profile: Some(None), codec: Some("hevc".into()), yuv444: Some(false), mbps: Some(80), ..Default::default() });
        // Déjà personnalisé : la clé changée et le profil nul.
        let old = Tracked::of(&s);
        s.codec = "av1".into();
        let eav1 = Effective { codec: "av1".into(), mbps: 80 };
        assert_eq!(changed_keys(&old, &Tracked::of(&s), &eav1), SyncKeys { profile: Some(None), codec: Some("av1".into()), ..Default::default() });
        // Le HDR, le mode immersif : eux seuls.
        let mut s = HostSettings::default();
        s.hdr = false;
        s.immersive = "never".into();
        assert_eq!(changed_keys(&base, &Tracked::of(&s), &e), SyncKeys { hdr: Some(false), immersive: Some("never".into()), ..Default::default() });
        // Le son, l'écran : rien.
        let mut s = HostSettings::default();
        s.audio = "5.1".into();
        s.resolution = 1080;
        assert!(changed_keys(&base, &Tracked::of(&s), &e).is_empty());
    }

    #[test]
    fn changements_rendus_au_pilote() {
        let mut app = app();
        // Un réglage hors synchronisation : rien.
        app.store.hosts[0].settings.audio = "stereo".into();
        assert!(app.sync_changes(&auto).is_empty());
        // Le HDR : la clé changée, et toutes pour un premier envoi (`AccountSync::to_send` choisit).
        app.store.hosts[0].settings.hdr = false;
        let c = app.sync_changes(&auto);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].keys, SyncKeys { hdr: Some(false), ..Default::default() });
        assert_eq!(c[0].full, SyncKeys { profile: Some(Some("jeu".into())), hdr: Some(false), immersive: Some("fullscreen".into()), ..Default::default() });
        assert_eq!(c[0].station, STATION);
        // La base suit : rien de plus sans nouveau changement.
        assert!(app.sync_changes(&auto).is_empty());
        // Personnalisé : profil nul, format, 4:4:4 et débit réels.
        app.store.hosts[0].settings.mbps = 80;
        let c = app.sync_changes(&auto);
        assert_eq!(c[0].keys, SyncKeys { profile: Some(None), codec: Some("hevc".into()), yuv444: Some(false), mbps: Some(80), ..Default::default() });
    }

    #[test]
    fn interrupteur_coupe_rien_ne_part() {
        let mut app = app();
        app.store.app.sync_settings = false;
        app.store.hosts[0].settings.hdr = false;
        assert!(app.sync_changes(&auto).is_empty());
        // Rallumé : le changement fait pendant la coupure ne part pas après coup.
        app.store.app.sync_settings = true;
        assert!(app.sync_changes(&auto).is_empty());
    }

    #[test]
    fn valeurs_recues_appliquees_sans_renvoyer() {
        let mut app = app();
        let k = key(&app);
        let known = SyncKeys { profile: Some(Some("max".into())), hdr: Some(false), immersive: Some("never".into()), ..Default::default() };
        assert!(app.sync_apply(&k, STATION, &known));
        let s = &app.store.hosts[0].settings;
        assert_eq!((s.profile, s.hdr, s.immersive.as_str()), (Profile::Max, false, "never"));
        // Pas renvoyé.
        assert!(app.sync_changes(&auto).is_empty());
        // Une autre Station : rien.
        assert!(!app.sync_apply(&k, "zyxwvutsrqponmlkjihgfedcba", &known));
    }

    #[test]
    fn memoire_d_avant_reprise() {
        let mut app = app();
        let k = key(&app);
        let old = format!(r#"{{"synced":[{{"account":"{k}","station":"{STATION}","updated_at":100,"values":{{"hdr":true,"x":1}},"applied":90}}],"sync_pending":[{{"account":"{k}","station":"{STATION}","keys":{{"immersive":"never"}}}}]}}"#);
        let mut store: Store = serde_json::from_str(&old).unwrap();
        store.adopt_legacy_sync();
        let m = store.sync_memory(&k);
        assert_eq!(m.list, vec![SyncedRaw { station: STATION.into(), updated_at: 100, values: serde_json::from_str(r#"{"hdr":true,"x":1}"#).unwrap() }]);
        assert_eq!(m.applied.get(STATION), Some(&90));
        assert_eq!(m.pending.get(STATION), Some(&SyncKeys { immersive: Some("never".into()), ..Default::default() }));
        // Écrite sous la forme d'`AccountSync` du cœur, sans les anciens champs.
        let json = serde_json::to_value(&store).unwrap();
        assert!(json.get("synced").is_none() && json.get("sync_pending").is_none());
        assert_eq!(json["account_sync"][0]["pending"][STATION]["immersive"], "never");
        assert_eq!(json["account_sync"][0]["applied"][STATION], 90);
        // Vide : retirée.
        store.set_sync_memory(&k, SyncMemory::default());
        assert!(store.account_sync.is_empty());
        app.store.sync_forget_account(&k);
    }

    #[test]
    fn personnalise_selon_ce_que_l_appareil_decode() {
        let m = Machine::default();
        let mut s = HostSettings::default();
        // AV1 pas décodé ici : le format reste ; le débit ramené au choix le plus proche.
        let k = SyncKeys { profile: Some(None), codec: Some("av1".into()), yuv444: Some(true), mbps: Some(75), ..Default::default() };
        assert!(apply(&mut s, &k, &m));
        assert_eq!((s.codec.as_str(), s.yuv444, s.mbps), ("auto", true, 80));
        assert!(s.custom());
        // H.264 : pas de 4:4:4.
        let k = SyncKeys { profile: Some(None), codec: Some("h264".into()), yuv444: Some(true), ..Default::default() };
        apply(&mut s, &k, &m);
        assert_eq!((s.codec.as_str(), s.yuv444), ("h264", false));
        // Un profil : ce qui s'en écartait revient au profil.
        apply(&mut s, &SyncKeys { profile: Some(Some("bureau".into())), ..Default::default() }, &m);
        assert_eq!((s.profile, s.codec.as_str(), s.mbps), (Profile::Bureau, "auto", 0));
        assert!(!s.custom());
        // Une valeur que l'app ne connaît pas : rien.
        assert!(!apply(&mut s, &SyncKeys { immersive: Some("bientot".into()), ..Default::default() }, &m));
    }

    #[test]
    fn ordinateur_appaire_et_dans_un_compte() {
        let mut app = app();
        let k = key(&app);
        app.store.hosts.push(host("p1", "zyxwvutsrqponmlkjihgfedcba"));
        assert_eq!(app.store.sync_target("p1"), None);
        app.store.account_computers.push(KnComputer { account: k.clone(), relay: "share.kybernow.app".into(), station: "zyxwvutsrqponmlkjihgfedcba".into(), ..Default::default() });
        assert_eq!(app.store.sync_target("p1"), Some((k.clone(), "zyxwvutsrqponmlkjihgfedcba".into())));
        assert_eq!(app.store.sync_target("h1"), Some((k, STATION.into())));
    }

    #[test]
    fn debit_ramene_aux_choix() {
        // Bornes de `kybernow_rules::limits::manual_mbps` : 500 Mbit/s, 2 500 en PyroWave ; puis le cran le plus proche.
        assert_eq!(snap_mbps(1, false), 10);
        assert_eq!(snap_mbps(60, false), 60);
        assert_eq!(snap_mbps(4000, false), 500);
        assert_eq!(snap_mbps(4000, true), 2500);
        assert_eq!(snap_mbps(1200, true), 1000);
        assert_eq!(snap_mbps(0, true), 0, "Auto reste Auto");
        assert_eq!(crate::app::bitrates(false).last(), Some(&500));
        assert_eq!(crate::app::bitrates(true).last(), Some(&2500));
    }
}
