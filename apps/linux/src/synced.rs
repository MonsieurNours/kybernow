//! Réglages synchronisés par le compte, côté interface (docs/comptes.md, « Réglages synchronisés et déplacer un
//! ordinateur » ; ce que fait chaque app : apps/core/README.md, « Réglages synchronisés »). Sans le cœur : le pilote
//! (`device/accounts.rs`) lit et envoie par `account::fetch_settings` et `put_settings`, et passe ici les valeurs déjà
//! vérifiées par le cœur (`SyncedSettings::known`), dans le même JSON (`SyncKeys` ↔ `SyncedValues`).
//!
//! - **Ce qui suit la personne** : le profil (`jeu`, `bureau`, `max` ; « Personnalisé » → `null`), le format vidéo
//!   (`codec`), le texte net (`yuv444`), le débit (`mbps`, Mbit/s), le HDR (`hdr`) et le mode immersif (`immersive`).
//! - **Envoyer** : à chaque réglage gardé (`Action::Save`), le pilote compare chaque ordinateur d'un compte à ce qui a
//!   été lu, appliqué ou envoyé (`AccountsUi::sync_base`) et envoie seulement les clés changées ; un changement pas
//!   encore reçu par le relais attend dans `Store::sync_pending`, renvoyé au rafraîchissement suivant.
//! - **Lire** : au lancement, à chaque rafraîchissement des comptes et juste avant une session ; ce qui est plus récent
//!   que ce qui a déjà été appliqué (`SyncedEntry::applied`) passe dans les réglages de l'ordinateur, sans être renvoyé.

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

    /// Les clés de `later` posées par-dessus celles-ci (un changement de plus avant l'envoi).
    pub fn merge(&mut self, later: &SyncKeys) {
        if later.profile.is_some() {
            self.profile = later.profile.clone();
        }
        if later.codec.is_some() {
            self.codec = later.codec.clone();
        }
        if later.yuv444.is_some() {
            self.yuv444 = later.yuv444;
        }
        if later.mbps.is_some() {
            self.mbps = later.mbps;
        }
        if later.hdr.is_some() {
            self.hdr = later.hdr;
        }
        if later.immersive.is_some() {
            self.immersive = later.immersive.clone();
        }
    }

    /// Le relais a reçu `sent` : les clés qui n'ont pas changé depuis ne sont plus à envoyer.
    pub fn remove_sent(&mut self, sent: &SyncKeys) {
        if sent.profile.is_some() && self.profile == sent.profile {
            self.profile = None;
        }
        if sent.codec.is_some() && self.codec == sent.codec {
            self.codec = None;
        }
        if sent.yuv444.is_some() && self.yuv444 == sent.yuv444 {
            self.yuv444 = None;
        }
        if sent.mbps.is_some() && self.mbps == sent.mbps {
            self.mbps = None;
        }
        if sent.hdr.is_some() && self.hdr == sent.hdr {
            self.hdr = None;
        }
        if sent.immersive.is_some() && self.immersive == sent.immersive {
            self.immersive = None;
        }
    }

    /// Une clé encore à envoyer n'est pas remplacée par ce qui arrive du relais.
    fn without(&self, pending: Option<&SyncKeys>) -> SyncKeys {
        let Some(p) = pending else { return self.clone() };
        let mut k = self.clone();
        if p.profile.is_some() {
            k.profile = None;
        }
        if p.codec.is_some() {
            k.codec = None;
        }
        if p.yuv444.is_some() {
            k.yuv444 = None;
        }
        if p.mbps.is_some() {
            k.mbps = None;
        }
        if p.hdr.is_some() {
            k.hdr = None;
        }
        if p.immersive.is_some() {
            k.immersive = None;
        }
        k
    }
}

/// Les réglages d'un ordinateur gardés par le relais, pour un compte : tels qu'il les rend (`values`, clés inconnues
/// comprises, la base de l'envoi suivant), et le dernier changement déjà appliqué ici.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncedEntry {
    /// La clé du compte (`KnAccount::key`).
    pub account: String,
    /// La Station (26 lettres).
    pub station: String,
    /// Dernier changement chez le relais (secondes Unix, horloge du relais).
    pub updated_at: u64,
    pub values: Map<String, Value>,
    /// `updated_at` déjà appliqué aux réglages de l'appareil (ou envoyé par lui).
    pub applied: u64,
}

/// Un changement pas encore reçu par le relais (relais muet) : renvoyé au rafraîchissement suivant.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PendingSync {
    pub account: String,
    pub station: String,
    pub keys: SyncKeys,
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

/// Un envoi à faire : les clés en attente pour cet ordinateur, posées par le pilote sur les dernières valeurs lues.
#[derive(Clone, Debug, PartialEq)]
pub struct Push {
    pub account: String,
    pub station: String,
    pub keys: SyncKeys,
    /// Les dernières valeurs lues ou envoyées (`None` : aucune).
    pub last: Option<SyncedEntry>,
}

/// Un ordinateur tel que le relais le rend, déjà lu par le cœur.
#[derive(Clone, Debug, PartialEq)]
pub struct Fetched {
    pub station: String,
    pub updated_at: u64,
    pub values: Map<String, Value>,
    /// `SyncedSettings::known` : les clés que cet appareil sait lire.
    pub known: SyncKeys,
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

    pub fn synced(&self, account: &str, station: &str) -> Option<&SyncedEntry> {
        self.synced.iter().find(|e| e.account == account && e.station == station)
    }

    fn synced_mut(&mut self, account: &str, station: &str) -> &mut SyncedEntry {
        if let Some(i) = self.synced.iter().position(|e| e.account == account && e.station == station) {
            return &mut self.synced[i];
        }
        self.synced.push(SyncedEntry { account: account.into(), station: station.into(), ..Default::default() });
        self.synced.last_mut().unwrap()
    }

    pub fn pending(&self, account: &str, station: &str) -> Option<&SyncKeys> {
        self.sync_pending.iter().find(|p| p.account == account && p.station == station).map(|p| &p.keys)
    }

    /// Les envois en attente d'un compte (au rafraîchissement, avant de lire).
    pub fn pushes(&self, account: &str) -> Vec<Push> {
        self.sync_pending
            .iter()
            .filter(|p| p.account == account && !p.keys.is_empty())
            .map(|p| Push { account: p.account.clone(), station: p.station.clone(), keys: p.keys.clone(), last: self.synced(&p.account, &p.station).cloned() })
            .collect()
    }

    /// Le relais a reçu `sent` pour cet ordinateur et rend ses valeurs (`updated_at`, `values`) : elles remplacent les
    /// précédentes et comptent comme appliquées ; les clés envoyées ne sont plus en attente.
    pub fn sync_sent(&mut self, account: &str, station: &str, sent: &SyncKeys, updated_at: u64, values: Map<String, Value>) {
        let e = self.synced_mut(account, station);
        e.updated_at = updated_at;
        e.applied = e.applied.max(updated_at);
        e.values = values;
        self.drop_sent(account, station, sent);
    }

    /// Les clés envoyées ne sont plus en attente (reçues, ou refusées pour de bon).
    pub fn drop_sent(&mut self, account: &str, station: &str, sent: &SyncKeys) {
        if let Some(p) = self.sync_pending.iter_mut().find(|p| p.account == account && p.station == station) {
            p.keys.remove_sent(sent);
        }
        self.sync_pending.retain(|p| !p.keys.is_empty());
    }

    /// L'ordinateur n'est plus au compte (ou plus ouvert à lui) : ses réglages gardés et ses envois en attente sont
    /// oubliés ; ses réglages locaux restent.
    pub fn sync_forget(&mut self, account: &str, station: &str) {
        self.synced.retain(|e| !(e.account == account && e.station == station));
        self.sync_pending.retain(|p| !(p.account == account && p.station == station));
    }

    /// Un compte oublié : ce qu'il gardait pour la synchronisation aussi.
    pub fn sync_forget_account(&mut self, account: &str) {
        self.synced.retain(|e| e.account != account);
        self.sync_pending.retain(|p| p.account != account);
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

    /// Après un réglage gardé : les clés changées de chaque ordinateur d'un compte, ajoutées aux envois en attente ;
    /// rend les envois à faire. Interrupteur coupé : la base suit, rien ne part. `auto_mbps` : le débit du profil au
    /// mode de la session (Mbit/s), pour un réglage personnalisé dont le débit est « Auto ».
    pub fn sync_changes(&mut self, auto_mbps: &dyn Fn(&App, &Host) -> u32) -> Vec<Push> {
        let on = self.sync_on();
        let mut changes: Vec<(String, String, SyncKeys)> = Vec::new();
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
                // Rien au relais pour cet ordinateur (ni lu, ni en attente) : tout, la première fois.
                let first = self.store.synced(&account, &station).is_none() && self.store.pending(&account, &station).is_none();
                let keys = if first { full_keys(&now, &e) } else { changed_keys(old, &now, &e) };
                if !keys.is_empty() {
                    changes.push((account, station, keys));
                }
            }
            base.insert(h.id.clone(), now);
        }
        self.accounts.sync_base = base;
        let mut stations: Vec<(String, String)> = Vec::new();
        for (account, station, keys) in changes {
            match self.store.sync_pending.iter_mut().find(|p| p.account == account && p.station == station) {
                Some(p) => p.keys.merge(&keys),
                None => self.store.sync_pending.push(PendingSync { account: account.clone(), station: station.clone(), keys }),
            }
            if !stations.contains(&(account.clone(), station.clone())) {
                stations.push((account, station));
            }
        }
        stations
            .into_iter()
            .filter_map(|(a, s)| {
                let keys = self.store.pending(&a, &s)?.clone();
                Some(Push { last: self.store.synced(&a, &s).cloned(), account: a, station: s, keys })
            })
            .collect()
    }

    /// Les réglages lus pour un compte : ceux qui sont plus récents que ce qui a déjà été appliqué passent dans les
    /// réglages des ordinateurs (sans être renvoyés : la base suit) ; une clé encore en attente d'envoi reste celle de
    /// l'appareil. Un ordinateur absent de la liste perd ses réglages gardés. Rend vrai si le magasin a changé.
    pub fn sync_fetched(&mut self, account: &str, list: Vec<Fetched>) -> bool {
        if self.store.account(account).is_none() {
            return false;
        }
        let mut changed = false;
        let before = self.store.synced.len();
        self.store.synced.retain(|e| e.account != account || list.iter().any(|f| f.station == e.station));
        changed |= before != self.store.synced.len();
        for f in list {
            let applied = self.store.synced(account, &f.station).map_or(0, |e| e.applied);
            let fresh = f.updated_at > applied;
            let keys = f.known.without(self.store.pending(account, &f.station));
            let e = self.store.synced_mut(account, &f.station);
            if e.updated_at != f.updated_at || e.values != f.values {
                changed = true;
            }
            e.updated_at = f.updated_at;
            e.values = f.values;
            if !fresh {
                continue;
            }
            e.applied = f.updated_at;
            changed = true;
            for id in self.store.sync_hosts(account, &f.station) {
                let machine = &self.machine;
                if let Some(h) = self.store.hosts.iter_mut().find(|h| h.id == id) {
                    if apply(&mut h.settings, &keys, machine) {
                        log::info!("comptes : réglages de {} reçus ({})", h.name, f.station);
                    }
                    self.accounts.sync_base.insert(id.clone(), Tracked::of(&h.settings));
                }
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
    fn envoi_a_la_premiere_fois_tout_puis_les_cles_changees() {
        let mut app = app();
        let k = key(&app);
        // Un réglage hors synchronisation : rien.
        app.store.hosts[0].settings.audio = "stereo".into();
        assert!(app.sync_changes(&auto).is_empty());
        // Rien au relais : tout.
        app.store.hosts[0].settings.hdr = false;
        let p = app.sync_changes(&auto);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].keys, SyncKeys { profile: Some(Some("jeu".into())), hdr: Some(false), immersive: Some("fullscreen".into()), ..Default::default() });
        assert!(p[0].last.is_none());
        // Reçu : plus rien en attente ; ensuite, seulement les clés changées.
        app.store.sync_sent(&k, STATION, &p[0].keys, 100, Map::new());
        assert!(app.store.sync_pending.is_empty());
        app.store.hosts[0].settings.immersive = "always".into();
        let p = app.sync_changes(&auto);
        assert_eq!(p[0].keys, SyncKeys { immersive: Some("always".into()), ..Default::default() });
        assert_eq!(p[0].last.as_ref().map(|l| l.updated_at), Some(100));
        // Relais muet : le changement attend, le suivant s'y ajoute.
        app.store.hosts[0].settings.mbps = 80;
        let p = app.sync_changes(&auto);
        assert_eq!(p[0].keys, SyncKeys { profile: Some(None), codec: Some("hevc".into()), yuv444: Some(false), mbps: Some(80), immersive: Some("always".into()), ..Default::default() });
        assert_eq!(app.store.pushes(&k).len(), 1);
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
    fn lecture_applique_le_plus_recent_sans_renvoyer() {
        let mut app = app();
        let k = key(&app);
        let known = SyncKeys { profile: Some(Some("max".into())), hdr: Some(false), immersive: Some("never".into()), ..Default::default() };
        let f = Fetched { station: STATION.into(), updated_at: 200, values: Map::new(), known: known.clone() };
        assert!(app.sync_fetched(&k, vec![f.clone()]));
        let s = &app.store.hosts[0].settings;
        assert_eq!((s.profile, s.hdr, s.immersive.as_str()), (Profile::Max, false, "never"));
        // Pas renvoyé.
        assert!(app.sync_changes(&auto).is_empty());
        // Déjà appliqué : un changement local reste.
        app.store.hosts[0].settings.hdr = true;
        app.sync_baseline();
        app.sync_fetched(&k, vec![f]);
        assert!(app.store.hosts[0].settings.hdr);
        // Absent de la liste : oublié.
        app.sync_fetched(&k, Vec::new());
        assert!(app.store.synced(&k, STATION).is_none());
    }

    #[test]
    fn une_cle_en_attente_reste_celle_de_l_appareil() {
        let mut app = app();
        let k = key(&app);
        app.store.sync_pending.push(PendingSync { account: k.clone(), station: STATION.into(), keys: SyncKeys { hdr: Some(false), ..Default::default() } });
        app.store.hosts[0].settings.hdr = false;
        app.sync_baseline();
        let f = Fetched { station: STATION.into(), updated_at: 300, values: Map::new(), known: SyncKeys { hdr: Some(true), immersive: Some("always".into()), ..Default::default() } };
        app.sync_fetched(&k, vec![f]);
        let s = &app.store.hosts[0].settings;
        assert_eq!((s.hdr, s.immersive.as_str()), (false, "always"));
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
