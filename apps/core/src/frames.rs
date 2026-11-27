//! Le flux d'images vu du décodeur de l'app (WINAPP-51) : ce qu'il faut
//! faire de chaque paquet de Kyber, écrit une fois. Avant, chaque app avait
//! sa machine : Linux n'avait pas le repli « format jamais annoncé » ajouté
//! sur Windows contre l'écran noir, Windows remplaçait les paramètres du
//! flux là où Linux les mettait bout à bout.
//!
//! [`FrameFlow`] ne décode rien et n'appelle rien : chaque entrée rend des
//! [`Step`] que l'app exécute dans l'ordre (ouvrir son décodeur, le
//! configurer, décoder, écarter, dire une perte à la session…), puis l'app
//! lui rend compte ([`FrameFlow::fed`], [`FrameFlow::failed`],
//! [`FrameFlow::unavailable`]). Le temps est donné par l'app : tout se
//! vérifie sans horloge.

use std::time::{Duration, Instant};

use crate::media::{FOURCC_AV1, FOURCC_PYROWAVE, ResumeGate};
use crate::pyrowave::Config;

/// Une image clé est redemandée au plus à ce rythme par le décodeur.
pub const KEYFRAME_EVERY: Duration = Duration::from_millis(300);

/// Ce que l'app fait, dans l'ordre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Fermer le décodeur s'il y en a un et en ouvrir un pour ce format
    /// (`media::FOURCC_*` ; PyroWave : son propre décodeur). S'il ne s'ouvre
    /// pas : [`FrameFlow::unavailable`].
    Open(u32),
    /// PyroWave : la configuration du flux, à donner au décodeur (gardée
    /// pour celui qui s'ouvrira).
    Configure(Config),
    /// Décoder cette image, `head` d'abord (les paramètres du flux, devant
    /// une image clé) ; `first` : la première depuis une attente. Puis
    /// [`FrameFlow::fed`], ou [`FrameFlow::failed`] si le décodeur la refuse.
    Decode { head: Vec<u8>, first: bool },
    /// Écarter cette image.
    Skip,
    /// Une perte : [`crate::session::Session::keyframe_lost`] (réparation,
    /// puis image clé ; une fois par perte).
    Lost,
    /// Le décodeur n'a plus rien d'avant la perte : une image clé, tout de
    /// suite ([`crate::session::Session::request_keyframe`], au plus toutes
    /// les [`KEYFRAME_EVERY`]).
    Keyframe,
    /// L'image est revenue : [`crate::session::Session::keyframe_recovered`] ;
    /// `resumed` : sur une image de reprise, sans image clé.
    Recovered { resumed: bool },
}

/// L'état du flux d'images d'un décodeur.
#[derive(Debug, Clone, Default)]
pub struct FrameFlow {
    /// Le format annoncé par Kyber (0 : aucun).
    fourcc: u32,
    /// Le format que la session a demandé (0 : inconnu).
    wanted: u32,
    /// Un format a été annoncé depuis le dernier changement de `wanted`.
    announced: bool,
    /// Les paramètres du flux (SPS, PPS…), bout à bout tant qu'aucune image
    /// ne les sépare ; remplacés par les suivants.
    config: Vec<u8>,
    in_config: bool,
    /// Le décodeur est ouvert, avec ces paramètres.
    loaded: Option<Vec<u8>>,
    /// PyroWave : la dernière configuration lue.
    pyrowave: Option<Config>,
    /// Le décodeur de ce format ne s'ouvre pas : tout est écarté jusqu'au
    /// prochain format.
    unavailable: bool,
    waiting_key: bool,
    lost_reported: bool,
    seen_frame: bool,
    gate: ResumeGate,
    last_request: Option<Instant>,
}

impl FrameFlow {
    /// `wanted` : le format que la session demande (`Codec::fourcc`, 0 :
    /// inconnu), pour ouvrir le bon décodeur si Kyber ne l'annonce pas.
    pub fn new(wanted: u32) -> Self {
        Self { wanted, waiting_key: true, ..Self::default() }
    }

    /// Le format annoncé (0 : aucun encore).
    pub fn fourcc(&self) -> u32 {
        self.fourcc
    }

    /// La session demande un autre format (profil changé) : si Kyber ne
    /// l'annonce pas, la prochaine image l'ouvre d'office.
    pub fn set_wanted(&mut self, fourcc: u32) {
        if fourcc != self.wanted {
            self.wanted = fourcc;
            self.announced = false;
        }
    }

    /// Kyber annonce le format du flux (`Packet::Codec`).
    pub fn codec(&mut self, fourcc: u32) -> Vec<Step> {
        self.switch(fourcc);
        self.announced = true;
        if fourcc == FOURCC_PYROWAVE { vec![Step::Open(fourcc)] } else { Vec::new() }
    }

    fn switch(&mut self, fourcc: u32) {
        *self = Self { wanted: self.wanted, seen_frame: self.seen_frame, last_request: self.last_request, ..Self::new(self.wanted) };
        self.fourcc = fourcc;
        self.waiting_key = fourcc != FOURCC_PYROWAVE;
    }

    /// Paramètres du flux (`Packet::Media` de configuration) ; en PyroWave,
    /// le paquet `KNPW`.
    pub fn config(&mut self, data: &[u8]) -> Vec<Step> {
        if self.fourcc == FOURCC_PYROWAVE || Config::is_config(data) {
            return match Config::parse(data) {
                Some(c) if self.pyrowave != Some(c) => {
                    log::info!("PyroWave : configuration {}", c.label());
                    self.pyrowave = Some(c);
                    vec![Step::Configure(c)]
                }
                Some(_) => Vec::new(),
                None => {
                    log::warn!("PyroWave : paquet de configuration illisible ({} octets)", data.len());
                    Vec::new()
                }
            };
        }
        // AV1 : un `av1C` (4 octets de résumé, puis les OBU) n'entre pas tel
        // quel dans un flux d'OBU.
        let data = if self.fourcc == FOURCC_AV1 && data.len() > 4 && data[0] == 0x81 { &data[4..] } else { data };
        if !self.in_config {
            self.config.clear();
        }
        self.config.extend_from_slice(data);
        self.in_config = true;
        Vec::new()
    }

    /// Une image (`Packet::Media`) : `key` image clé, `resume` la marque de
    /// reprise, `now` l'heure de l'app.
    pub fn frame(&mut self, key: bool, data: &[u8], resume: Option<u64>, now: Instant) -> Vec<Step> {
        self.in_config = false;
        self.seen_frame = true;
        let mut steps = Vec::new();
        // Format jamais annoncé (paquet perdu, profil changé) : celui que la
        // session a demandé, ouvert d'office (contre l'écran noir).
        if !self.announced && self.wanted != 0 && self.fourcc != self.wanted {
            log::warn!("image : format jamais annoncé, décodeur {:#x} ouvert d'office", self.wanted);
            // Les paramètres déjà reçus sont gardés : seul le paquet de format
            // s'est perdu.
            let wanted = self.wanted;
            self.fourcc = wanted;
            self.announced = true;
            self.loaded = None;
            self.unavailable = false;
            self.pyrowave = None;
            self.gate.reset();
            self.waiting_key = wanted != FOURCC_PYROWAVE;
            if wanted == FOURCC_PYROWAVE {
                steps.push(Step::Open(wanted));
            }
        }
        if self.unavailable || self.fourcc == 0 {
            steps.push(Step::Skip);
            return steps;
        }
        if self.fourcc == FOURCC_PYROWAVE {
            // Intra seul : chaque image se décode, pas d'image clé à attendre.
            if Config::is_config(data) {
                steps.extend(self.config(data));
            } else {
                steps.push(Step::Decode { head: Vec::new(), first: false });
            }
            return steps;
        }
        // Paramètres changés depuis l'ouverture : le décodeur est refait sur
        // l'image clé qui les suit.
        let stale = key && !self.config.is_empty() && self.loaded.as_ref().is_some_and(|c| *c != self.config);
        if self.loaded.is_none() || stale {
            if !key {
                steps.extend(self.keyframe(now, false));
                steps.push(Step::Skip);
                return steps;
            }
            steps.push(Step::Open(self.fourcc));
            self.loaded = Some(self.config.clone());
            self.gate.reset();
            self.waiting_key = true;
        }
        if self.waiting_key && !key && !self.gate.resumes(resume) {
            if !self.gate.intact() {
                steps.extend(self.keyframe(now, false));
            }
            steps.push(Step::Skip);
            return steps;
        }
        let first = self.waiting_key;
        // Une attente qui suit une perte dite à la session (la première image
        // d'un flux, ou après une erreur du décodeur, n'est pas une reprise).
        if first && self.lost_reported {
            steps.push(Step::Recovered { resumed: !key });
        }
        self.waiting_key = false;
        self.lost_reported = false;
        steps.push(Step::Decode { head: if key { self.config.clone() } else { Vec::new() }, first });
        steps
    }

    /// Un trou dans le flux (`Packet::Hole`) : attente d'une image clé ou
    /// d'une reprise ; la perte est dite une fois.
    pub fn lost(&mut self) -> Vec<Step> {
        if self.fourcc == FOURCC_PYROWAVE {
            return Vec::new();
        }
        self.waiting_key = true;
        if self.lost_reported {
            return Vec::new();
        }
        self.lost_reported = true;
        vec![Step::Lost]
    }

    /// Des images écartées avant le décodeur (file de l'app pleine) : rien
    /// d'avant ne suffit plus, seule une image clé relance.
    pub fn dropped(&mut self) -> Vec<Step> {
        self.gate.reset();
        self.lost()
    }

    /// L'image (`pts`) est confiée au décodeur, rien ne manquant avant elle.
    pub fn fed(&mut self, pts: u64) {
        self.gate.fed(pts);
    }

    /// Le décodeur a refusé l'image ou rendu une erreur : il n'a plus rien,
    /// une image clé tout de suite.
    pub fn failed(&mut self, now: Instant) -> Vec<Step> {
        self.gate.reset();
        self.waiting_key = self.fourcc != FOURCC_PYROWAVE;
        self.keyframe(now, true).into_iter().collect()
    }

    /// Le décodeur de ce format ne s'ouvre pas : les images sont écartées
    /// jusqu'au prochain format (PyroWave : dire aussi
    /// [`crate::session::Session::pyrowave_failed`], qui en change).
    pub fn unavailable(&mut self) {
        self.unavailable = true;
        self.loaded = None;
    }

    /// Le décodeur est perdu ou refait par l'app (carte graphique perdue,
    /// passage au décodage par le processeur) : il se rouvre sur la
    /// prochaine image clé, demandée tout de suite.
    pub fn reset(&mut self, now: Instant) -> Vec<Step> {
        self.loaded = None;
        self.unavailable = false;
        self.failed(now)
    }

    /// À appeler régulièrement (100 ms) : en attente, sans rien d'avant la
    /// perte, l'image clé est redemandée.
    pub fn idle(&mut self, now: Instant) -> Vec<Step> {
        if self.waiting_key && self.seen_frame && !self.gate.intact() && self.fourcc != FOURCC_PYROWAVE {
            return self.keyframe(now, false).into_iter().collect();
        }
        Vec::new()
    }

    fn keyframe(&mut self, now: Instant, force: bool) -> Option<Step> {
        if force || self.last_request.is_none_or(|t| now.saturating_duration_since(t) >= KEYFRAME_EVERY) {
            self.last_request = Some(now);
            return Some(Step::Keyframe);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{FOURCC_H264, FOURCC_HEVC};

    fn decode(head: &[u8], first: bool) -> Step {
        Step::Decode { head: head.to_vec(), first }
    }

    #[test]
    fn flux_annonce_puis_images() {
        let t = Instant::now();
        let mut f = FrameFlow::new(FOURCC_HEVC);
        assert_eq!(f.codec(FOURCC_HEVC), []);
        // Paramètres en plusieurs paquets : bout à bout.
        assert_eq!(f.config(b"VPS"), []);
        assert_eq!(f.config(b"SPS"), []);
        // Une image avant l'image clé : écartée, image clé demandée (une fois par 300 ms).
        assert_eq!(f.frame(false, b"p", None, t), [Step::Keyframe, Step::Skip]);
        assert_eq!(f.frame(false, b"p", None, t + Duration::from_millis(100)), [Step::Skip]);
        // L'image clé ouvre le décodeur, paramètres devant.
        assert_eq!(f.frame(true, b"I", None, t), [Step::Open(FOURCC_HEVC), decode(b"VPSSPS", true)]);
        f.fed(1);
        assert_eq!(f.frame(false, b"p", None, t), [decode(b"", false)]);
        f.fed(2);
        // De nouveaux paramètres (séparés par une image) remplacent les anciens ;
        // l'image clé suivante refait le décodeur.
        assert_eq!(f.config(b"SPS2"), []);
        assert_eq!(f.frame(true, b"I", None, t), [Step::Open(FOURCC_HEVC), decode(b"SPS2", true)]);
        // Mêmes paramètres : pas refait.
        assert_eq!(f.frame(true, b"I", None, t), [decode(b"SPS2", false)]);
    }

    #[test]
    fn perte_reparee_ou_image_cle() {
        let t = Instant::now();
        let mut f = FrameFlow::new(FOURCC_HEVC);
        f.codec(FOURCC_HEVC);
        f.config(b"P");
        f.frame(true, b"I", None, t);
        f.fed(10);
        // Perte : dite une fois ; le décodeur a ce qu'il faut, pas d'image clé de lui.
        assert_eq!(f.lost(), [Step::Lost]);
        assert_eq!(f.lost(), []);
        assert_eq!(f.frame(false, b"p", None, t), [Step::Skip]);
        assert_eq!(f.idle(t), []);
        // Image de reprise qui ne dépend que de ce qu'il a : reprise sans image clé.
        assert_eq!(f.frame(false, b"r", Some(10), t), [Step::Recovered { resumed: true }, decode(b"", true)]);
        f.fed(11);
        // Images écartées par l'app : seule une image clé relance.
        assert_eq!(f.dropped(), [Step::Lost]);
        assert_eq!(f.frame(false, b"r", Some(11), t), [Step::Keyframe, Step::Skip]);
        assert_eq!(f.frame(true, b"I", None, t + KEYFRAME_EVERY), [Step::Recovered { resumed: false }, decode(b"P", true)]);
        // Erreur du décodeur : image clé tout de suite, même juste après une demande.
        assert_eq!(f.failed(t + KEYFRAME_EVERY), [Step::Keyframe]);
        assert_eq!(f.idle(t + KEYFRAME_EVERY + Duration::from_millis(50)), []);
        assert_eq!(f.idle(t + KEYFRAME_EVERY * 2), [Step::Keyframe]);
    }

    #[test]
    fn format_jamais_annonce() {
        let t = Instant::now();
        // Rien d'annoncé : le format demandé s'ouvre sur la première image clé.
        let mut f = FrameFlow::new(FOURCC_H264);
        f.config(b"S");
        assert_eq!(f.frame(true, b"I", None, t), [Step::Open(FOURCC_H264), decode(b"S", true)], "paramètres reçus gardés");
        assert_eq!(f.fourcc(), FOURCC_H264);
        // Profil changé vers HEVC, paquet de format perdu : HEVC d'office.
        f.set_wanted(FOURCC_HEVC);
        f.config(b"V");
        let steps = f.frame(true, b"I", None, t);
        assert_eq!(steps, [Step::Open(FOURCC_HEVC), decode(b"V", true)], "les paramètres du nouveau format");
        assert_eq!(f.fourcc(), FOURCC_HEVC);
        // Annoncé ensuite : suivi tel quel.
        f.set_wanted(FOURCC_HEVC);
        assert_eq!(f.codec(FOURCC_AV1), []);
        // AV1 : le résumé `av1C` retiré des paramètres.
        f.config(&[0x81, 1, 2, 3, 0xAA, 0xBB]);
        assert_eq!(f.frame(true, b"I", None, t), [Step::Open(FOURCC_AV1), decode(&[0xAA, 0xBB], true)]);
        // Sans format demandé ni annoncé : rien à ouvrir.
        let mut f = FrameFlow::new(0);
        assert_eq!(f.frame(true, b"I", None, t), [Step::Skip]);
    }

    #[test]
    fn pyrowave_intra_et_configuration() {
        let t = Instant::now();
        let mut f = FrameFlow::new(FOURCC_PYROWAVE);
        assert_eq!(f.codec(FOURCC_PYROWAVE), [Step::Open(FOURCC_PYROWAVE)]);
        let c = Config::new(false, false, true, 1920, 1080);
        assert_eq!(f.config(&c.bytes()), [Step::Configure(c)]);
        assert_eq!(f.config(&c.bytes()), [], "la même : rien");
        // Chaque image se décode ; un paquet KNPW dans le flux des images configure.
        assert_eq!(f.frame(false, b"i", None, t), [decode(b"", false)]);
        let c2 = Config { width: 1280, height: 720, ..c };
        assert_eq!(f.frame(false, &c2.bytes(), None, t), [Step::Configure(c2)]);
        assert_eq!(f.lost(), [], "pas de perte à réparer en intra");
        // Le décodeur ne s'ouvre pas : tout est écarté jusqu'au format suivant.
        f.unavailable();
        assert_eq!(f.frame(false, b"i", None, t), [Step::Skip]);
        assert_eq!(f.codec(FOURCC_HEVC), []);
        f.config(b"P");
        assert_eq!(f.frame(true, b"I", None, t), [Step::Open(FOURCC_HEVC), decode(b"P", true)]);
        // Format demandé PyroWave jamais annoncé : ouvert d'office.
        let mut f = FrameFlow::new(FOURCC_PYROWAVE);
        assert_eq!(f.frame(false, b"i", None, t), [Step::Open(FOURCC_PYROWAVE), decode(b"", false)]);
    }

    #[test]
    fn decodeur_refait_par_l_app() {
        let t = Instant::now();
        let mut f = FrameFlow::new(FOURCC_HEVC);
        f.codec(FOURCC_HEVC);
        f.config(b"P");
        f.frame(true, b"I", None, t);
        f.fed(1);
        assert_eq!(f.reset(t), [Step::Keyframe]);
        assert_eq!(f.frame(false, b"p", None, t + Duration::from_millis(10)), [Step::Skip]);
        assert_eq!(f.frame(true, b"I", None, t), [Step::Open(FOURCC_HEVC), decode(b"P", true)]);
    }
}
