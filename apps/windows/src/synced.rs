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
//! - [`changed`] : les seules clés changées par la personne, à envoyer
//!   (`settings::changed` du cœur, plus le HDR et le mode immersif) ;
//!   [`everything`] la première fois (rien au relais pour cet ordinateur).
//! - [`apply`] : ce qui arrive du relais, posé sur les réglages de ce PC par
//!   `SyncedValues::apply_to` du cœur (ce que la carte graphique décode, le
//!   débit ramené aux bornes du débit à la main).
//! - Ce qu'on garde par compte (`synced.json`) : l'`AccountSync` du cœur
//!   (WINAPP-55, le même pour Linux) — la dernière liste lue, le
//!   `updated_at` déjà appliqué par ordinateur, et les changements que le
//!   relais n'a pas reçus (renvoyés au rafraîchissement suivant, avant
//!   d'appliquer ce qui arrive).
//!
//! Les appels au relais et l'écriture des réglages sont dans `ui/synced.rs`.

use std::collections::HashMap;

pub use kybernow_core::account::AccountSync;
use kybernow_core::account::{SyncedValues, settings};
use kybernow_core::plan::Choices;
use kybernow_core::profile::{Capabilities, Codec, Profile, StreamSettings};
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

    /// Les choix de session de ces réglages, pour le cœur (`plan`,
    /// `settings::changed`).
    pub fn choices(&self) -> Choices {
        Choices {
            profile: Profile::parse(&self.profile),
            codec: self.codec.as_deref().and_then(Codec::parse),
            yuv444: self.yuv444,
            mbps: self.mbps,
            reliable: self.reliable,
            ..Default::default()
        }
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
/// celles de la session par le cœur (`settings::changed` : un profil →
/// `{profile}` ; « Personnalisé » → `{profile: null, codec, yuv444, mbps}` ;
/// un réglage déjà personnalisé → la clé changée et `profile: null`), puis
/// le HDR → `{hdr}` et le mode immersif → `{immersive}`, que l'app ajoute.
/// Les valeurs sont celles que la session prendrait (`before_eff`,
/// `after_eff` : `plan::plan`).
pub fn changed(before: &Local, after: &Local, before_eff: &StreamSettings, after_eff: &StreamSettings) -> SyncedValues {
    let mut v = settings::changed(&before.choices(), &after.choices(), before_eff, after_eff);
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
        // Auto : ses réglages viennent du test de chaque appareil
        // (docs/profil-auto.md) ; reçu d'un autre appareil, ce PC garde les
        // siens pour cet ordinateur (`apply_to` du cœur fait de même).
        Some(Some(Profile::Auto)) => {}
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

/// `synced.json` : par compte (`Account::key`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncFile {
    pub accounts: HashMap<String, AccountSync>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::account::SyncedSettings;
    use kybernow_core::console::Mode;
    use serde_json::json;

    /// Les réglages de session de ces réglages : `plan::plan` (comme `session_plan` de l'app).
    fn plan_of(local: &Local, caps: Capabilities, screen: Mode) -> StreamSettings {
        kybernow_core::plan::plan(&local.choices(), caps, screen).0
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
        // Auto, choisi sur un autre appareil : ce PC garde ses réglages (le test est
        // propre à chaque appareil) ; le HDR, lui, est pris.
        assert_eq!(apply(&retouched, &values(json!({"profile": "auto", "codec": "pyrowave", "mbps": 400})), caps(), eff), retouched);
        let out = apply(&jeu, &values(json!({"profile": "auto", "hdr": false})), caps(), eff);
        assert_eq!((out.profile.as_str(), out.hdr), ("jeu", Some(false)));
    }

    fn synced(station: &str, at: u64, v: serde_json::Value) -> SyncedSettings {
        SyncedSettings { station: station.into(), updated_at: at, values: v.as_object().unwrap().clone() }
    }

    #[test]
    fn fichier_comme_avant() {
        // `synced.json` d'une version d'avant (l'`AccountSync` de l'app) : relu
        // par celui du cœur, mêmes champs.
        let before = json!({"accounts": {"relais|compte": {"list": [{"station": STATION, "updated_at": 7, "values": {"profile": null, "x": [1]}}], "applied": {STATION: 7}, "pending": {STATION: {"mbps": 90}}}}});
        let f: SyncFile = serde_json::from_value(before.clone()).unwrap();
        let a = &f.accounts["relais|compte"];
        assert_eq!((a.applied[STATION], a.pending[STATION].mbps, a.last(STATION).unwrap().values["x"][0].as_i64()), (7, Some(90), Some(1)));
        assert_eq!(serde_json::to_value(&f).unwrap(), before);
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
