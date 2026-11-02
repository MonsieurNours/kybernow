//! Les mires du « Réglage du HDR » à l'écran, en HDR : l'image fixe de la mire
//! (`crate::mires`, HEVC Main 10 PQ avec ses métadonnées HDR10) donnée au
//! décodeur du téléviseur (NDL), qui passe la dalle en mode HDR comme pour une
//! session. La même image est redonnée toutes les 100 ms (une image seule
//! peut rester dans le décodeur sans être montrée) ; elle change avec le
//! palier. À la fin, NDL est déchargé : la dalle revient en SDR, et une
//! session en cours recharge son image.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

use super::ndl;
use crate::mires::Step;

/// La mire voulue (`None` : plus de mire), et le fil qui la montre.
struct State {
    wanted: Option<(Step, usize)>,
    running: bool,
}

static STATE: Mutex<State> = Mutex::new(State { wanted: None, running: false });
static CHANGED: Condvar = Condvar::new();

/// Taille des mires (le téléviseur les met à la taille de sa dalle).
const SIZE: (u32, u32) = (1920, 1080);
const REPEAT: Duration = Duration::from_millis(100);

/// Montre cette mire, ou plus aucune.
pub fn show(mire: Option<(Step, usize)>) {
    let mut g = STATE.lock().unwrap();
    g.wanted = mire;
    if mire.is_some() && !g.running {
        g.running = true;
        if !ndl::available() {
            log::warn!("réglage du HDR : pas de décodeur");
            g.running = false;
            return;
        }
        ndl::begin_mires();
        log::info!("réglage du HDR : mires sur le décodeur");
        if std::thread::Builder::new().name("kybernow-mires".into()).spawn(run).is_err() {
            g.running = false;
            ndl::end_mires();
        }
    }
    CHANGED.notify_all();
}

fn run() {
    let mut shown: Option<(Step, usize)> = None;
    loop {
        let wanted = {
            let g = STATE.lock().unwrap();
            let (g, _) = CHANGED.wait_timeout_while(g, REPEAT, |s| s.wanted.is_some() && s.wanted == shown).unwrap();
            g.wanted
        };
        let Some(image) = wanted.and_then(|(step, i)| crate::mires::image(step, i)) else {
            // Plus de mire : le décodeur est rendu sous le verrou, pour qu'une mire redemandée entre-temps relance un fil.
            let mut g = STATE.lock().unwrap();
            if g.wanted.is_some_and(|(step, i)| crate::mires::image(step, i).is_some()) {
                continue;
            }
            g.wanted = None;
            ndl::end_mires();
            g.running = false;
            log::info!("réglage du HDR : décodeur rendu");
            return;
        };
        let (step, i) = wanted.unwrap_or((Step::Peak, 0));
        if shown != wanted {
            log::info!("réglage du HDR : {step:?}, {}", crate::mires::values(step).get(i).map(|v| format!("{v} nits")).unwrap_or_default());
        }
        // Toutes les mires portent leurs métadonnées HDR10 : le décodeur les donne au téléviseur une fois par chargement.
        let meta = crate::hdr::decide(&crate::hdr::scan_hevc(image), true, true);
        if let Err(e) = ndl::ensure_mire(SIZE.0, SIZE.1, meta) {
            log::warn!("réglage du HDR : {e}");
            std::thread::sleep(REPEAT);
            continue;
        }
        match ndl::play_mire(image) {
            Ok(_) => shown = wanted,
            Err(e) if e == ndl::NOT_READY => {}
            Err(e) => log::warn!("réglage du HDR : {e}"),
        }
    }
}
