//! Réglages synchronisés par le compte (docs/comptes.md, « Réglages
//! synchronisés et déplacer un ordinateur » ; apps/core/README.md,
//! « Réglages synchronisés ») : le profil, le format vidéo, le texte net
//! (4:4:4), le débit, le HDR et le mode immersif de chaque ordinateur d'un
//! compte suivent la personne d'un appareil à l'autre.
//!
//! Ce qui se décide sans Windows, pour se tester sur le Mac :
//!
//! - [`Local`] : les réglages d'un ordinateur sur ce PC qui se synchronisent
//!   (`store::Settings` en a d'autres, qui restent ici). Sur Windows, le profil
//!   est toujours `jeu`, `bureau` ou `max` ; « Personnalisé » vient des
//!   retouches (`codec`, `yuv444`, `mbps`, `reliable` : `plan::plan`).
//! - [`changed`] : les seules clés changées par la personne, à envoyer ;
//!   [`everything`] la première fois (rien au relais pour cet ordinateur).
//! - [`apply`] : ce qui arrive du relais, posé sur les réglages de ce PC par
//!   `SyncedValues::apply_to` du cœur (ce que la carte graphique décode).
//! - [`AccountSync`] : ce qu'on garde par compte (`synced.json`) — la
//!   dernière liste lue, le `updated_at` déjà appliqué par ordinateur, et les
//!   changements que le relais n'a pas reçus (renvoyés au rafraîchissement
//!   suivant, avant d'appliquer ce qui arrive).
//!
//! Les appels au relais et l'écriture des réglages sont dans `ui/synced.rs`.

use std::collections::HashMap;

use kybernow_core::account::{SyncedSettings, SyncedValues, settings};
use kybernow_core::profile::{Capabilities, StreamSettings};
use serde::{Deserialize, Serialize};

/// Les réglages synchronisés d'un ordinateur sur ce PC (ceux de
/// `store::Settings`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Local {
    /// `jeu`, `bureau` ou `max`.
    pub profile: String,
    /// `None` : celui du profil (« Auto »).
    pub codec: Option<String>,
    pub yuv444: Option<bool>,
    pub mbps: Option<u32>,
    /// Pas synchronisé, mais rend le profil « Personnalisé ».
    pub reliable: Option<bool>,
    /// `None` : jamais changé, allumé.
    pub hdr: Option<bool>,
    /// `fullscreen`, `always` ou `never`.
    pub immersive: String,
}

impl Local {
    pub fn hdr_on(&self) -> bool {
        self.hdr.unwrap_or(true)
    }

    fn immersive(&self) -> &str {
        if settings::IMMERSIVE.contains(&self.immersive.as_str()) { &self.immersive } else { "fullscreen" }
    }
}

/// Tout, la première fois qu'un ordinateur part au relais : les réglages de
/// la session tels que ce PC les demanderait (`effective`, `plan::plan`), le
/// HDR et le mode immersif.
pub fn everything(local: &Local, effective: &StreamSettings) -> SyncedValues {
    let s = StreamSettings { hdr: local.hdr_on(), ..effective.clone() };
    SyncedValues::from_stream(&s).with_immersive(Some(local.immersive()))
}

/// Les clés que la personne vient de changer (docs/comptes.md, « Envoyer ») :
/// un profil → `{profile}` ; « Personnalisé » (un réglage du profil retouché)
/// → `{profile: null, codec, yuv444, mbps}` ; le format, le 4:4:4 ou le débit
/// d'un réglage déjà personnalisé → la clé changée et `profile: null` ; le
/// HDR → `{hdr}` ; le mode immersif → `{immersive}`. Les valeurs sont celles
/// que la session prendrait (`before_eff`, `after_eff` : `plan::plan`).
pub fn changed(before: &Local, after: &Local, before_eff: &StreamSettings, after_eff: &StreamSettings) -> SyncedValues {
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
    if before.hdr_on() != after.hdr_on() {
        v.hdr = Some(after.hdr_on());
    }
    if before.immersive() != after.immersive() {
        v.immersive = Some(after.immersive().to_string());
    }
    v
}

/// Ce qui arrive du relais (`SyncedSettings::known`), posé sur les réglages
/// de ce PC : `SyncedValues::apply_to` du cœur sur les réglages de session
/// d'aujourd'hui (`effective` : `plan::plan` de réglages donnés), puis remis
/// dans le modèle de l'app — un profil efface les retouches ; « Personnalisé »
/// devient des retouches (format, 4:4:4 et, s'il le faut pour rester
/// personnalisé, le débit) ; le HDR ; le mode immersif.
pub fn apply(local: &Local, values: &SyncedValues, caps: Capabilities, effective: impl Fn(&Local) -> StreamSettings) -> Local {
    let base = StreamSettings { hdr: local.hdr_on(), ..effective(local) };
    let r = values.apply_to(&base, caps);
    let mut out = local.clone();
    match values.profile() {
        Some(Some(p)) => {
            out.profile = p.key().to_string();
            out.codec = None;
            out.yuv444 = None;
            out.mbps = None;
            out.reliable = None;
        }
        Some(None) => {
            out.codec = Some(r.codec.key().to_string());
            out.yuv444 = Some(r.yuv444);
            if values.mbps.is_some() || effective(&out).profile.is_some() {
                out.mbps = Some(r.mbps);
            }
        }
        None if base.profile.is_none() => {
            if values.codec.is_some() && r.codec != base.codec {
                out.codec = Some(r.codec.key().to_string());
            }
            if values.yuv444.is_some() || r.yuv444 != base.yuv444 {
                out.yuv444 = Some(r.yuv444);
            }
            if values.mbps.is_some() {
                out.mbps = Some(r.mbps);
            }
        }
        None => {}
    }
    if values.hdr.is_some() {
        out.hdr = Some(r.hdr);
    }
    if let Some(i) = values.immersive.as_ref().filter(|i| settings::IMMERSIVE.contains(&i.as_str())) {
        out.immersive = i.clone();
    }
    out
}

/// `top` posé sur `base` (les clés de `top` l'emportent).
pub fn overlay(base: &SyncedValues, top: &SyncedValues) -> SyncedValues {
    SyncedValues {
        profile: top.profile.clone().or_else(|| base.profile.clone()),
        codec: top.codec.clone().or_else(|| base.codec.clone()),
        yuv444: top.yuv444.or(base.yuv444),
        mbps: top.mbps.or(base.mbps),
        hdr: top.hdr.or(base.hdr),
        immersive: top.immersive.clone().or_else(|| base.immersive.clone()),
    }
}

/// Ce qu'on garde pour un compte (`synced.json`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountSync {
    /// La dernière liste lue (ou envoyée), clés inconnues comprises.
    pub list: Vec<SyncedSettings>,
    /// Par Station : le `updated_at` déjà appliqué (ou envoyé) sur ce PC.
    pub applied: HashMap<String, u64>,
    /// Par Station : les changements que le relais n'a pas reçus.
    pub pending: HashMap<String, SyncedValues>,
}

impl AccountSync {
    /// Les dernières valeurs gardées pour cet ordinateur (`last` de `put_settings`).
    pub fn last(&self, station: &str) -> Option<&SyncedSettings> {
        settings::find(&self.list, station)
    }

    /// Ce qui part pour ce changement : ce qui attendait encore, puis le
    /// changement ; tout (`full`) si le relais n'a rien pour cet ordinateur.
    pub fn to_send(&self, station: &str, change: &SyncedValues, full: impl FnOnce() -> SyncedValues) -> SyncedValues {
        let waiting = self.pending.get(station).cloned().unwrap_or_default();
        let base = if self.last(station).is_none() { overlay(&full(), &waiting) } else { waiting };
        overlay(&base, change)
    }

    /// Le relais a pris ces réglages : gardés à la place des anciens, leur
    /// `updated_at` compte comme appliqué (ce PC ne les reprend pas).
    pub fn sent(&mut self, s: SyncedSettings) {
        self.pending.remove(&s.station);
        self.applied.insert(s.station.clone(), s.updated_at);
        match self.list.iter_mut().find(|x| x.station == s.station) {
            Some(x) => *x = s,
            None => self.list.push(s),
        }
    }

    /// Le relais n'a pas répondu : renvoyé au rafraîchissement suivant.
    pub fn unsent(&mut self, station: &str, values: &SyncedValues) {
        let waiting = self.pending.get(station).cloned().unwrap_or_default();
        self.pending.insert(station.to_string(), overlay(&waiting, values));
    }

    /// L'ordinateur n'est plus au compte : ses réglages gardés sont oubliés
    /// (les réglages de ce PC restent).
    pub fn forget(&mut self, station: &str) {
        self.list.retain(|s| s.station != station);
        self.applied.remove(station);
        self.pending.remove(station);
    }

    /// Une liste lue (après le renvoi de ce qui attendait) : les ordinateurs
    /// absents sont oubliés ; rend, pour chaque ordinateur changé depuis le
    /// dernier `updated_at` appliqué, les valeurs à appliquer (`known`). Un
    /// ordinateur dont un changement attend encore le relais garde celui de
    /// ce PC.
    pub fn fetched(&mut self, list: Vec<SyncedSettings>) -> Vec<(String, SyncedValues)> {
        self.applied.retain(|station, _| settings::find(&list, station).is_some());
        // Un changement jamais reçu par le relais (premier envoi) n'est pas
        // encore dans la liste : il attend toujours.
        let old = std::mem::take(&mut self.list);
        self.pending.retain(|station, _| settings::find(&list, station).is_some() || settings::find(&old, station).is_none());
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

/// `synced.json` : par compte (`Account::key`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncFile {
    pub accounts: HashMap<String, AccountSync>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::console::Mode;
    use kybernow_core::profile::Profile;
    use serde_json::json;

    /// Les réglages de session de ces réglages : `plan::plan` (comme `session_plan` de l'app).
    fn plan_of(local: &Local, caps: Capabilities, screen: Mode) -> StreamSettings {
        let choices = crate::plan::Choices {
            profile: Profile::parse(&local.profile),
            codec: local.codec.as_deref().and_then(kybernow_core::profile::Codec::parse),
            yuv444: local.yuv444,
            mbps: local.mbps,
            reliable: local.reliable,
            ..Default::default()
        };
        crate::plan::plan(&choices, caps, screen).0
    }

    const STATION: &str = "abcdefghijklmnopqrstuvwxyz";

    fn caps() -> Capabilities {
        Capabilities { hevc: true, hevc444: true, av1: false, hdr: true, pyrowave: false }
    }

    fn screen() -> Mode {
        Mode { width: 3840, height: 2160, refresh: 60 }
    }

    fn eff(l: &Local) -> StreamSettings {
        plan_of(l, caps(), screen())
    }

    fn local(profile: &str) -> Local {
        Local { profile: profile.into(), immersive: "fullscreen".into(), ..Default::default() }
    }

    fn values(v: serde_json::Value) -> SyncedValues {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn premiere_fois_tout() {
        let l = Local { hdr: Some(false), immersive: "always".into(), ..local("bureau") };
        let v = everything(&l, &eff(&l));
        assert_eq!(v.profile, Some(Some("bureau".into())));
        assert_eq!(v.codec.as_deref(), Some("hevc"));
        assert_eq!(v.hdr, Some(false));
        assert_eq!(v.immersive.as_deref(), Some("always"));
        assert!(v.mbps.is_some() && v.yuv444.is_some());
    }

    #[test]
    fn seulement_les_cles_changees() {
        let jeu = local("jeu");
        // Un profil choisi : `{profile}` seul.
        let bureau = local("bureau");
        let v = changed(&jeu, &bureau, &eff(&jeu), &eff(&bureau));
        assert_eq!(serde_json::to_value(&v).unwrap(), json!({"profile": "bureau"}));
        // Un réglage retouché : « Personnalisé », avec format, 4:4:4 et débit.
        let perso = Local { mbps: Some(80), ..jeu.clone() };
        let v = changed(&jeu, &perso, &eff(&jeu), &eff(&perso));
        assert_eq!(serde_json::to_value(&v).unwrap(), json!({"profile": null, "codec": "hevc", "yuv444": false, "mbps": 80}));
        // Déjà personnalisé, le débit seul : la clé et `profile: null`.
        let plus = Local { mbps: Some(120), ..perso.clone() };
        let v = changed(&perso, &plus, &eff(&perso), &eff(&plus));
        assert_eq!(serde_json::to_value(&v).unwrap(), json!({"profile": null, "mbps": 120}));
        // Le format seul.
        let h264 = Local { codec: Some("h264".into()), ..plus.clone() };
        let v = changed(&plus, &h264, &eff(&plus), &eff(&h264));
        assert_eq!(serde_json::to_value(&v).unwrap(), json!({"profile": null, "codec": "h264"}));
        // Le HDR, le mode immersif.
        let hdr = Local { hdr: Some(false), ..jeu.clone() };
        assert_eq!(serde_json::to_value(changed(&jeu, &hdr, &eff(&jeu), &eff(&hdr))).unwrap(), json!({"hdr": false}));
        let imm = Local { immersive: "never".into(), ..jeu.clone() };
        assert_eq!(serde_json::to_value(changed(&jeu, &imm, &eff(&jeu), &eff(&imm))).unwrap(), json!({"immersive": "never"}));
        // Rien de synchronisé (le HDR jamais changé vaut allumé).
        let same = Local { hdr: Some(true), ..jeu.clone() };
        assert!(changed(&jeu, &same, &eff(&jeu), &eff(&same)).is_empty());
    }

    #[test]
    fn application() {
        let jeu = local("jeu");
        // Un profil : ses réglages sur ce PC, retouches effacées.
        let retouched = Local { mbps: Some(50), codec: Some("h264".into()), ..jeu.clone() };
        let out = apply(&retouched, &values(json!({"profile": "max", "codec": "av1", "mbps": 999})), caps(), eff);
        assert_eq!(out, local("max"));
        // « Personnalisé » : format décodé ici, 4:4:4, débit.
        let out = apply(&jeu, &values(json!({"profile": null, "codec": "hevc", "yuv444": true, "mbps": 70})), caps(), eff);
        assert_eq!((out.codec.as_deref(), out.yuv444, out.mbps), (Some("hevc"), Some(true), Some(70)));
        assert!(eff(&out).profile.is_none());
        // Un format que ce PC ne décode pas (AV1) : celui d'ici, le reste pris.
        let out = apply(&jeu, &values(json!({"profile": null, "codec": "av1", "yuv444": false, "mbps": 40})), caps(), eff);
        assert_eq!((out.codec.as_deref(), out.mbps), (Some("hevc"), Some(40)));
        // `profile: null` seul reste « Personnalisé » sur ce PC.
        let out = apply(&jeu, &values(json!({"profile": null})), caps(), eff);
        assert!(eff(&out).profile.is_none());
        // HDR et mode immersif ; un mode inconnu est ignoré.
        let out = apply(&jeu, &values(json!({"hdr": false, "immersive": "always"})), caps(), eff);
        assert_eq!((out.hdr, out.immersive.as_str()), (Some(false), "always"));
        let out = apply(&jeu, &values(json!({"immersive": "plus-tard"})), caps(), eff);
        assert_eq!(out, jeu);
        // Le débit seul sur un profil : rien (il suit le profil).
        assert_eq!(apply(&jeu, &values(json!({"mbps": 33})), caps(), eff), jeu);
    }

    fn synced(station: &str, at: u64, v: serde_json::Value) -> SyncedSettings {
        SyncedSettings { station: station.into(), updated_at: at, values: v.as_object().unwrap().clone() }
    }

    #[test]
    fn lecture_et_envoi() {
        let mut a = AccountSync::default();
        let full = || values(json!({"profile": "jeu", "codec": "hevc", "yuv444": false, "mbps": 60, "hdr": true, "immersive": "fullscreen"}));
        // Rien au relais : tout part, le changement par-dessus.
        let v = a.to_send(STATION, &values(json!({"hdr": false})), full);
        assert_eq!(v.hdr, Some(false));
        assert_eq!(v.profile, Some(Some("jeu".into())));
        // Relais muet : gardé, renvoyé avec le changement suivant.
        a.unsent(STATION, &v);
        let again = a.to_send(STATION, &values(json!({"immersive": "never"})), full);
        assert_eq!((again.hdr, again.immersive.as_deref()), (Some(false), Some("never")));
        // Un changement qui attend : ce qui arrive ne l'écrase pas.
        assert!(a.fetched(vec![synced(STATION, 10, json!({"hdr": true}))]).is_empty());
        // Envoyé : `updated_at` appliqué, plus rien n'attend.
        a.sent(synced(STATION, 12, json!({"hdr": false, "immersive": "never"})));
        assert!(a.pending.is_empty());
        assert_eq!(a.applied[STATION], 12);
        // Déjà là : seulement les clés changées.
        let v = a.to_send(STATION, &values(json!({"mbps": 90})), || panic!("pas tout"));
        assert_eq!(serde_json::to_value(&v).unwrap(), json!({"mbps": 90}));
        // Rien de nouveau : rien à appliquer ; plus récent : ses clés connues.
        assert!(a.fetched(vec![synced(STATION, 12, json!({"hdr": false}))]).is_empty());
        let got = a.fetched(vec![synced(STATION, 20, json!({"profile": "bureau", "futur": 1}))]);
        assert_eq!(got, vec![(STATION.to_string(), values(json!({"profile": "bureau"})))]);
        assert_eq!(a.last(STATION).unwrap().values["futur"], 1);
        // Absent de la liste : oublié.
        assert!(a.fetched(Vec::new()).is_empty());
        assert!(a.applied.is_empty() && a.list.is_empty());
    }

    #[test]
    fn oubli() {
        let mut a = AccountSync::default();
        a.sent(synced(STATION, 5, json!({"hdr": true})));
        a.unsent(STATION, &values(json!({"mbps": 3})));
        a.forget(STATION);
        assert_eq!(a, AccountSync::default());
        // En JSON (`synced.json`), relu tel quel.
        let mut f = SyncFile::default();
        f.accounts.insert("relais|compte".into(), {
            let mut a = AccountSync::default();
            a.sent(synced(STATION, 7, json!({"profile": null, "x": [1]})));
            a
        });
        let back: SyncFile = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(back, f);
    }
}
