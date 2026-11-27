//! « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app › À propos », étapes par QR code) : « Préparation du
//! journal… », « Envoi du journal… », puis le QR code à gauche avec sa phrase et, en petit, ce qui le protège ;
//! « Journal ouvert sur le téléphone. », « Ce QR code a expiré. » et « Nouveau QR code », ou le refus du relais et
//! « Réessayer » ; « Terminé » toujours.

use super::canvas::{Canvas, Font, Text, H};
use super::theme::*;
use super::widgets::{self, Key};
use super::Frame;
use crate::app::{App, Target};
use crate::i18n::tr;
use crate::log_send::{self, LogStep};

pub fn draw(app: &App, cv: &mut Canvas, f: &mut Frame) {
    cv.clear(INK);
    cv.line(1300.0, 1080.0, 1920.0, 460.0, 3.0, SPECTRE.alpha(0.08));
    widgets::label(cv, 96.0, 104.0, "KyberNow Link");
    cv.text(96.0, 190.0, Text::new(Font::Title, 56.0, FROST), log_send::title());
    let Some(l) = &app.log else { return };
    let (x, w) = (96.0, 1000.0);
    let mut y = 300.0;
    match &l.step {
        LogStep::Preparing | LogStep::Uploading => {
            let text = if l.step == LogStep::Preparing { log_send::preparing() } else { log_send::uploading() };
            cv.text(x, y, Text::new(Font::Regular, 30.0, FROST_2), text);
        }
        LogStep::Qr(d) => {
            let side = 420.0;
            widgets::qr(cv, &d.url, x, y - 40.0, side);
            let tx = x + side + 72.0;
            let used = cv.paragraph(tx, y, Text::new(Font::Regular, 30.0, FROST), log_send::scan(), 760.0, 42.0);
            cv.paragraph(tx, y + used + 30.0, Text::new(Font::Regular, 22.0, FROST_3), log_send::privacy(), 760.0, 32.0);
            y += side;
        }
        LogStep::Opened => {
            cv.text(x, y, Text::new(Font::Regular, 30.0, FROST), log_send::opened());
        }
        LogStep::Expired => {
            cv.text(x, y, Text::new(Font::Regular, 30.0, FROST), log_send::expired());
        }
        LogStep::Failed(fail) => {
            cv.paragraph(x, y, Text::new(Font::Regular, 30.0, FROST), fail.text(), w, 42.0);
        }
    }
    // « Nouveau QR code » ou « Réessayer », puis « Terminé ».
    let by = y + 100.0;
    let mut bx = x;
    if let Some(again) = l.step.again() {
        bx += widgets::primary(cv, f, Target::LogAgain, bx, by, again) + 28.0;
        widgets::secondary(cv, f, Target::LogDone, bx, by + 10.0, log_send::done(), 26.0);
    } else {
        widgets::primary(cv, f, Target::LogDone, bx, by, log_send::done());
    }
    let hy = H - 48.0;
    let hx = 96.0 + widgets::hint(cv, 96.0, hy, Key::Ok, tr("Choisir", "Choose")) + 40.0;
    widgets::hint(cv, hx, hy, Key::Back, log_send::done());
}
