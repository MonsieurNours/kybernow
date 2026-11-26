//! Débit automatique (docs/debit-automatique.md) : chaque seconde, la session
//! regarde l'aller-retour réseau, les paquets perdus et les trous de l'image,
//! et ajuste la part du débit du profil qu'elle demande à l'ordinateur.
//!
//! Le calcul tient ici, sans horloge ni réseau : la session lui donne la
//! seconde écoulée (`Second`) et applique la part rendue. On garde une part
//! (entre 0 et 1) plutôt qu'un débit, pour qu'un changement de définition ou
//! d'écrans garde la même marge par rapport au débit du profil.

use std::collections::VecDeque;

use crate::latency::Second;

/// Débit sous lequel on ne descend pas (bit/s) : le plancher de l'encodeur
/// de la Station (Admin › Débit minimum, 5 Mbit/s par défaut). Plus bas, il
/// le remonte de toute façon.
pub const FLOOR: u32 = 5_000_000;

/// Pourquoi le débit visé est ce qu'il est (Statistiques, ligne « Débit
/// visé », docs/link-ui.md). Quand plusieurs raisons jouent, c'est celle qui
/// fixe la valeur : le plafond de l'app, puis le repli, puis le débit
/// automatique, puis la nature du réglage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetReason {
    /// Un profil, au débit du profil (pas baissé).
    Auto,
    /// Le débit automatique l'a baissé sous celui du profil.
    Lowered,
    /// Débit réglé à la main (« Personnalisé »).
    Manual,
    /// PyroWave avec un profil : le débit automatique est en pause.
    PyroWavePaused,
    /// L'app plafonne le débit (`Session::set_bitrate_cap` : fenêtre
    /// cachée, app en arrière-plan).
    AppCapped,
    /// Flux allégé après un repli (`Recovery::fallback_bitrate`).
    Fallback,
}

impl TargetReason {
    /// Code des ponts (C, JNI) : 0 Auto, 1 baissé, 2 à la main, 3 PyroWave,
    /// 4 plafond de l'app, 5 repli.
    pub fn code(self) -> u32 {
        match self {
            Self::Auto => 0,
            Self::Lowered => 1,
            Self::Manual => 2,
            Self::PyroWavePaused => 3,
            Self::AppCapped => 4,
            Self::Fallback => 5,
        }
    }

    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            0 => Self::Auto,
            1 => Self::Lowered,
            2 => Self::Manual,
            3 => Self::PyroWavePaused,
            4 => Self::AppCapped,
            5 => Self::Fallback,
            _ => return None,
        })
    }
}

/// Le débit que la session demande à l'ordinateur, et pourquoi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TargetBitrate {
    /// Débit demandé à Kyber, en bit/s.
    pub bitrate: u32,
    /// Débit du profil (ou réglé à la main), en bit/s, avant le débit
    /// automatique, le repli et le plafond de l'app ; PyroWave : celui dont
    /// l'image a besoin.
    pub profile: u32,
    pub reason: TargetReason,
}

/// Ce qui fixe le débit d'une session, dans l'ordre où la session
/// l'applique ([`TargetBitrate::compute`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TargetInputs {
    /// Débit du profil, ou réglé à la main (bit/s).
    pub base: u32,
    /// Le débit automatique joue : son débit pour `base`.
    pub adaptive: Option<u32>,
    /// Plafond du repli (flux allégé), bit/s.
    pub fallback_cap: Option<u32>,
    /// Plafond de l'app (fenêtre cachée), bit/s.
    pub app_cap: Option<u32>,
    /// Pas de profil : débit réglé à la main.
    pub manual: bool,
    /// PyroWave avec un profil : débit automatique en pause.
    pub pyrowave_paused: bool,
}

impl TargetBitrate {
    /// Le débit visé et la raison qui le fixe : la dernière étape qui l'a
    /// baissé l'emporte (plafond de l'app, puis repli, puis débit
    /// automatique) ; sinon la nature du réglage (PyroWave en pause, à la
    /// main, Auto).
    pub fn compute(i: TargetInputs) -> Self {
        let mut b = i.base;
        let mut reason = if i.pyrowave_paused {
            TargetReason::PyroWavePaused
        } else if i.manual {
            TargetReason::Manual
        } else {
            TargetReason::Auto
        };
        if let Some(a) = i.adaptive
            && a < b
        {
            b = a;
            reason = TargetReason::Lowered;
        }
        if let Some(cap) = i.fallback_cap
            && cap < b
        {
            b = cap;
            reason = TargetReason::Fallback;
        }
        if let Some(cap) = i.app_cap
            && cap < b
        {
            b = cap;
            reason = TargetReason::AppCapped;
        }
        Self { bitrate: b, profile: i.base, reason }
    }

    /// Mbit/s arrondis, comme les Statistiques les montrent.
    pub fn mbps(bitrate: u32) -> u32 {
        (bitrate + 500_000) / 1_000_000
    }

    /// La ligne « Débit visé » des Statistiques : « 42 / 60 Mbit/s · baissé,
    /// réseau saturé », « 60 Mbit/s · Auto », « 80 Mbit/s · réglé à la
    /// main », « 320 Mbit/s · PyroWave, Auto en pause », « 20 Mbit/s ·
    /// fenêtre cachée », « 15 Mbit/s · flux allégé ».
    pub fn text(&self, lang: crate::i18n::Lang) -> String {
        let target = Self::mbps(self.bitrate).to_string();
        let profile = Self::mbps(self.profile).to_string();
        let key = match self.reason {
            TargetReason::Auto => "bitrate.target_auto",
            TargetReason::Lowered => "bitrate.target_lowered",
            TargetReason::Manual => "bitrate.target_manual",
            TargetReason::PyroWavePaused => "bitrate.target_pyrowave",
            TargetReason::AppCapped => "bitrate.target_capped",
            TargetReason::Fallback => "bitrate.target_fallback",
        };
        crate::i18n::tr(lang, key, &[("target", &target), ("profile", &profile)])
    }

    /// Le libellé de la ligne : « Débit visé » / “Target bitrate”.
    pub fn label(lang: crate::i18n::Lang) -> String {
        crate::i18n::tr(lang, "bitrate.target_label", &[])
    }
}

/// Fenêtre où l'on cherche l'aller-retour du réseau au repos.
const BASE_WINDOW: usize = 10;
/// Aller-retour au-dessus du repos qui annonce une file qui se remplit : le
/// plus grand de 15 ms et de la moitié du repos.
const QUEUE_MS: f64 = 15.0;
/// Perte (en % des paquets reçus) qui compte comme une seconde chargée.
const LOSS_PCT: f64 = 2.0;
/// Perte qui fait baisser tout de suite, sans attendre la seconde suivante.
const HEAVY_LOSS_PCT: f64 = 10.0;
/// Paquets perdus en dessous desquels une perte ne compte pas (une image
/// de bureau immobile envoie peu : un paquet perdu y pèse trop lourd).
const MIN_LOST: u32 = 3;
/// Taille moyenne d'un datagramme d'image, pour estimer le nombre de paquets.
const PACKET_BYTES: u64 = 1200;
/// Baisse d'un cran.
const DOWN: f64 = 0.7;
/// Hausse d'un cran, et hausse prudente près du débit qui a déjà chargé.
const UP: f64 = 1.15;
const UP_NEAR: f64 = 1.05;
/// Secondes propres avant une hausse.
const CLEAN_BEFORE_UP: u32 = 4;
/// Secondes sans décision après un changement (le réseau et l'encodeur
/// s'y font).
const HOLD: u32 = 2;
/// Durée pendant laquelle on se souvient du débit qui a chargé.
const REMEMBER: u32 = 60;

/// Ce que la seconde écoulée dit du réseau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    /// Rien à signaler.
    Clean,
    /// File qui se remplit ou pertes.
    Loaded,
    /// Pertes lourdes : on baisse sans attendre.
    Heavy,
    /// Pas assez de mesures pour juger (pas d'image, pas d'aller-retour).
    Unknown,
}

#[derive(Debug, Clone)]
pub struct AdaptiveBitrate {
    /// Part du débit du profil demandée, entre `floor` et 1.
    share: f64,
    /// Allers-retours des dernières secondes (ms), pour le repos.
    rtts: VecDeque<f64>,
    /// Secondes chargées d'affilée, secondes propres d'affilée.
    loaded: u32,
    clean: u32,
    /// Secondes restantes sans décision.
    hold: u32,
    /// Part qui a chargé le réseau la dernière fois, et depuis combien de
    /// secondes.
    loaded_at: Option<(f64, u32)>,
}

impl Default for AdaptiveBitrate {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveBitrate {
    /// Départ au débit du profil : sur le réseau local, il passe d'emblée.
    pub fn new() -> Self {
        Self { share: 1.0, rtts: VecDeque::new(), loaded: 0, clean: 0, hold: 0, loaded_at: None }
    }

    /// Part du débit du profil à demander.
    pub fn share(&self) -> f64 {
        self.share
    }

    /// Débit à demander pour un débit de profil `need` (bit/s).
    pub fn bitrate(&self, need: u32) -> u32 {
        let floor = FLOOR.min(need);
        ((need as f64 * self.share).round() as u32).clamp(floor, need)
    }

    /// Le chemin a changé (flux relancé, relais) : l'aller-retour au repos
    /// est à réapprendre ; la part reste.
    pub fn new_path(&mut self) {
        self.rtts.clear();
        self.loaded = 0;
        self.clean = 0;
        self.hold = HOLD;
    }

    /// Ce que dit une seconde, d'après l'aller-retour au repos connu.
    pub fn judge(&self, s: &Second) -> Load {
        let packets = s.bytes / PACKET_BYTES + u64::from(s.lost_packets);
        let loss = if packets > 0 { f64::from(s.lost_packets) * 100.0 / packets as f64 } else { 0.0 };
        let lost = s.lost_packets >= MIN_LOST;
        if lost && loss >= HEAVY_LOSS_PCT {
            return Load::Heavy;
        }
        if (lost && loss >= LOSS_PCT) || s.holes > 0 {
            return Load::Loaded;
        }
        let Some(rtt) = s.rtt_ms else {
            return if s.bytes == 0 { Load::Unknown } else { Load::Clean };
        };
        if let Some(base) = self.base() {
            if rtt > base + QUEUE_MS.max(base / 2.0) {
                return Load::Loaded;
            }
        }
        if s.bytes == 0 { Load::Unknown } else { Load::Clean }
    }

    fn base(&self) -> Option<f64> {
        self.rtts.iter().copied().reduce(f64::min)
    }

    /// Juge une seconde et apprend son aller-retour, sans toucher à la part :
    /// en pause (PyroWave), le réseau reste observé.
    pub fn watch(&mut self, s: &Second) -> Load {
        let load = self.judge(s);
        // L'aller-retour entre dans le repos après le jugement : une seconde
        // chargée ne se compare pas à elle-même.
        if let Some(rtt) = s.rtt_ms {
            self.rtts.push_back(rtt);
            while self.rtts.len() > BASE_WINDOW {
                self.rtts.pop_front();
            }
        }
        load
    }

    /// Une seconde de plus. Rend la nouvelle part quand elle change.
    /// `need` est le débit du profil (bit/s), pour le plancher.
    pub fn second(&mut self, s: &Second, need: u32) -> Option<f64> {
        let load = self.watch(s);
        if let Some((_, age)) = &mut self.loaded_at {
            *age += 1;
            if *age > REMEMBER {
                self.loaded_at = None;
            }
        }
        match load {
            Load::Unknown => return None,
            Load::Clean => {
                self.loaded = 0;
                self.clean += 1;
            }
            Load::Loaded | Load::Heavy => {
                self.clean = 0;
                self.loaded += 1;
            }
        }
        if self.hold > 0 {
            self.hold -= 1;
            // Des pertes lourdes n'attendent pas la fin du répit.
            if load != Load::Heavy {
                return None;
            }
        }
        let floor = (f64::from(FLOOR.min(need)) / f64::from(need.max(1))).min(1.0);
        let before = self.share;
        if load == Load::Heavy || self.loaded >= 2 {
            self.loaded_at = Some((self.share, 0));
            self.share = (self.share * DOWN).max(floor);
            self.loaded = 0;
        } else if self.clean >= CLEAN_BEFORE_UP && self.share < 1.0 {
            let near = self.loaded_at.is_some_and(|(at, _)| self.share * UP >= at * 0.9);
            self.share = (self.share * if near { UP_NEAR } else { UP }).min(1.0);
            self.clean = 0;
        }
        if (self.share - before).abs() < 1e-9 {
            return None;
        }
        self.hold = HOLD;
        Some(self.share)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEED: u32 = 60_000_000;

    fn sec(rtt: f64, lost: u32, mbps: u64) -> Second {
        Second { rtt_ms: Some(rtt), lost_packets: lost, bytes: mbps * 1_000_000 / 8, ..Default::default() }
    }

    fn run(a: &mut AdaptiveBitrate, s: &Second, n: usize) {
        for _ in 0..n {
            a.second(s, NEED);
        }
    }

    #[test]
    fn reseau_local_propre_debit_du_profil() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 60);
        assert_eq!(a.bitrate(NEED), NEED);
    }

    #[test]
    fn file_qui_monte_baisse_apres_deux_secondes() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 10);
        assert_eq!(a.second(&sec(40.0, 0, 60), NEED), None, "une seconde seule ne suffit pas");
        assert_eq!(a.second(&sec(40.0, 0, 60), NEED), Some(0.7));
        assert_eq!(a.bitrate(NEED), 42_000_000);
    }

    #[test]
    fn pic_wifi_isole_sans_effet() {
        let mut a = AdaptiveBitrate::new();
        for i in 0..60 {
            let rtt = if i % 7 == 0 { 80.0 } else { 3.0 };
            a.second(&sec(rtt, 0, 60), NEED);
        }
        assert_eq!(a.bitrate(NEED), NEED);
    }

    #[test]
    fn pertes_lourdes_baisse_immediate() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 5);
        // 60 Mb/s ≈ 6 250 paquets : 1 000 perdus, c'est 14 %.
        assert_eq!(a.second(&sec(2.0, 1000, 60), NEED), Some(0.7));
        assert!(a.second(&sec(2.0, 1000, 42), NEED).is_some(), "pas de répit pour des pertes lourdes");
    }

    #[test]
    fn quelques_pertes_bureau_immobile_ignorees() {
        let mut a = AdaptiveBitrate::new();
        // Image immobile : 0,1 Mb/s (10 paquets), 2 paquets perdus (17 %).
        let still = Second { bytes: 12_500, ..sec(2.0, 2, 0) };
        run(&mut a, &still, 20);
        assert_eq!(a.share(), 1.0);
    }

    #[test]
    fn jamais_sous_le_plancher() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 3);
        run(&mut a, &sec(2.0, 2000, 60), 30);
        assert_eq!(a.bitrate(NEED), FLOOR);
        assert_eq!(a.bitrate(3_000_000), 3_000_000, "un profil sous le plancher garde son débit");
    }

    #[test]
    fn remonte_quand_le_reseau_redevient_propre() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 10);
        run(&mut a, &sec(40.0, 0, 60), 2);
        assert!(a.share() < 1.0);
        run(&mut a, &sec(2.0, 0, 42), 60);
        assert_eq!(a.share(), 1.0);
    }

    #[test]
    fn remonte_doucement_pres_du_debit_qui_a_charge() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 10);
        run(&mut a, &sec(40.0, 0, 60), 2); // 1,0 → 0,7, chargé à 1,0
        let mut steps = Vec::new();
        for _ in 0..40 {
            if let Some(s) = a.second(&sec(2.0, 0, 42), NEED) {
                steps.push(s);
            }
        }
        // 0,7 × 1,15 = 0,805 ; puis 0,805 × 1,15 ≥ 0,9 : pas de 5 %.
        assert!((steps[0] - 0.805).abs() < 1e-9, "{steps:?}");
        assert!((steps[1] - 0.805 * 1.05).abs() < 1e-9, "{steps:?}");
    }

    #[test]
    fn sans_image_aucune_decision() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &Second::default(), 20);
        assert_eq!(a.share(), 1.0);
    }

    #[test]
    fn nouveau_chemin_aller_retour_reappris() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 10);
        // Passage par le relais : 30 ms au repos, ce n'est pas une file.
        a.new_path();
        run(&mut a, &sec(30.0, 0, 60), 20);
        assert_eq!(a.share(), 1.0);
    }

    #[test]
    fn trous_de_l_image_comptent() {
        let mut a = AdaptiveBitrate::new();
        run(&mut a, &sec(2.0, 0, 60), 10);
        let s = Second { holes: 2, ..sec(2.0, 0, 60) };
        a.second(&s, NEED);
        assert_eq!(a.second(&s, NEED), Some(0.7));
    }

    #[test]
    fn debit_vise_et_sa_raison() {
        use crate::i18n::Lang;
        let m = |v: u32| v * 1_000_000;
        let base = TargetInputs { base: m(60), adaptive: Some(m(60)), ..Default::default() };
        let t = TargetBitrate::compute(base);
        assert_eq!((t.bitrate, t.profile, t.reason), (m(60), m(60), TargetReason::Auto));
        assert_eq!(t.text(Lang::Fr), "60 Mbit/s · adaptatif");
        assert_eq!(t.text(Lang::En), "60 Mbit/s · adaptive");
        // Baissé par le débit automatique.
        let t = TargetBitrate::compute(TargetInputs { adaptive: Some(m(42)), ..base });
        assert_eq!((t.bitrate, t.reason), (m(42), TargetReason::Lowered));
        assert_eq!(t.text(Lang::Fr), "42 / 60 Mbit/s · baissé, réseau saturé");
        assert_eq!(t.text(Lang::En), "42 / 60 Mbit/s · lowered, network saturated");
        // Débit adaptatif coupé (fixe), PyroWave en pause.
        let t = TargetBitrate::compute(TargetInputs { base: m(80), manual: true, ..Default::default() });
        assert_eq!(t.reason, TargetReason::Manual);
        assert_eq!(t.text(Lang::Fr), "80 Mbit/s · fixe");
        assert_eq!(t.text(Lang::En), "80 Mbit/s · fixed");
        let t = TargetBitrate::compute(TargetInputs { base: m(320), pyrowave_paused: true, ..Default::default() });
        assert_eq!(t.reason, TargetReason::PyroWavePaused);
        assert_eq!(t.text(Lang::Fr), "320 Mbit/s · PyroWave, adaptatif en pause");
        assert_eq!(t.text(Lang::En), "320 Mbit/s · PyroWave, adaptive paused");
        // Plafond de l'app, repli : celui qui fixe la valeur l'emporte.
        let t = TargetBitrate::compute(TargetInputs { adaptive: Some(m(42)), fallback_cap: Some(m(15)), app_cap: Some(m(20)), ..base });
        assert_eq!((t.bitrate, t.reason), (m(15), TargetReason::Fallback));
        assert_eq!(t.text(Lang::Fr), "15 Mbit/s · flux allégé");
        assert_eq!(t.text(Lang::En), "15 Mbit/s · lightened stream");
        let t = TargetBitrate::compute(TargetInputs { adaptive: Some(m(42)), fallback_cap: Some(m(30)), app_cap: Some(m(20)), ..base });
        assert_eq!((t.bitrate, t.reason), (m(20), TargetReason::AppCapped));
        assert_eq!(t.text(Lang::Fr), "20 Mbit/s · fenêtre cachée");
        assert_eq!(t.text(Lang::En), "20 Mbit/s · window hidden");
        // Un plafond au-dessus du débit ne fixe rien.
        let t = TargetBitrate::compute(TargetInputs { adaptive: Some(m(42)), fallback_cap: Some(m(50)), app_cap: Some(m(42)), ..base });
        assert_eq!((t.bitrate, t.reason), (m(42), TargetReason::Lowered));
        let t = TargetBitrate::compute(TargetInputs { manual: true, app_cap: Some(m(90)), ..base });
        assert_eq!(t.reason, TargetReason::Manual);
        assert_eq!(TargetBitrate::label(Lang::Fr), "Débit visé");
        assert_eq!(TargetBitrate::label(Lang::En), "Target bitrate");
        for c in 0..6 {
            assert_eq!(TargetReason::from_code(c).map(TargetReason::code), Some(c));
        }
        assert_eq!(TargetReason::from_code(6), None);
    }
}
