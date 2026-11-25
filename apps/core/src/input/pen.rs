// Stylet (docs/link-ui.md, « Stylet » ; docs/stylet.md) : ce que l'app lit
// de son stylet (tablette graphique, Apple Pencil, stylet d'Android, Pointer
// Events), mis au format du paquet kynput `Pen`.
//
// Le stylet ne part en stylet que si l'ordinateur l'a annoncé
// (`PenAvailable`, reçu à l'arrivée des entrées) et en souris du bureau ;
// sinon [`crate::input::InputPipeline::pen_on`] rend `false` et l'app le
// traite en souris, comme avant.

use kynput_rs::Pen;

/// Un état du stylet, tel que l'app le lit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PenInput {
    /// Près de la surface (survol ou contact). Faux : le stylet s'en va.
    pub in_range: bool,
    /// La pointe (ou la gomme) touche.
    pub contact: bool,
    /// Pression, de 0 à 1.
    pub pressure: f32,
    /// Inclinaison en degrés (-90 à 90 ; X vers la droite, Y vers soi), si
    /// l'appareil la donne.
    pub tilt: Option<(f32, f32)>,
    /// Rotation en degrés (0 à 360), si l'appareil la donne.
    pub rotation: Option<f32>,
    /// Le bout gomme est celui qui sert.
    pub eraser: bool,
    /// Le bouton du stylet est tenu.
    pub barrel: bool,
}

impl PenInput {
    /// Le paquet kynput, à (`x`, `y`) de l'écran `display_id` de l'ordinateur.
    pub fn to_kynput(&self, display_id: u32, x: i16, y: i16) -> Pen {
        let mut flags = 0;
        if self.in_range {
            flags |= Pen::IN_RANGE;
            if self.contact {
                flags |= Pen::CONTACT;
            }
        }
        if self.eraser {
            flags |= Pen::ERASER;
        }
        if self.barrel {
            flags |= Pen::BARREL;
        }
        let (tilt_x, tilt_y) = match self.tilt {
            Some((tx, ty)) => {
                flags |= Pen::TILT;
                (degrees_i8(tx), degrees_i8(ty))
            }
            None => (0, 0),
        };
        let rotation = match self.rotation {
            Some(r) if r.is_finite() => {
                flags |= Pen::ROTATION;
                (r.rem_euclid(360.0).round() as u16) % 360
            }
            _ => 0,
        };
        let pressure = if self.in_range && self.contact { Pen::pressure_from_fraction(self.pressure) } else { 0 };
        Pen { display_id, x, y, pressure, tilt_x, tilt_y, rotation, flags }
    }
}

fn degrees_i8(v: f32) -> i8 {
    if v.is_finite() { v.clamp(-90.0, 90.0).round() as i8 } else { 0 }
}

/// Inclinaison X / Y (degrés) d'après l'altitude (angle avec la surface :
/// π/2 debout) et l'azimut (direction vers laquelle penche le stylet, 0 vers
/// la droite, sens horaire à l'écran), en radians : Apple Pencil, Android.
/// Formule des Pointer Events (W3C).
pub fn tilt_from_altitude_azimuth(altitude: f32, azimuth: f32) -> (f32, f32) {
    use std::f32::consts::FRAC_PI_2;
    let altitude = altitude.clamp(0.0, FRAC_PI_2);
    if altitude >= FRAC_PI_2 - 1e-4 {
        return (0.0, 0.0);
    }
    if altitude <= 1e-4 {
        // À plat : penché tout entier dans la direction de l'azimut.
        let (s, c) = azimuth.sin_cos();
        let pick = |v: f32| if v > 1e-4 { 90.0 } else if v < -1e-4 { -90.0 } else { 0.0 };
        return (pick(c), pick(s));
    }
    let t = altitude.tan();
    let tx = (azimuth.cos() / t).atan().to_degrees();
    let ty = (azimuth.sin() / t).atan().to_degrees();
    (tx, ty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    #[test]
    fn hover_has_no_pressure_and_contact_does() {
        let hover = PenInput { in_range: true, pressure: 0.7, ..Default::default() };
        let p = hover.to_kynput(2, 10, 20);
        assert_eq!((p.display_id, p.x, p.y, p.pressure), (2, 10, 20, 0));
        assert!(p.has(Pen::IN_RANGE) && !p.has(Pen::CONTACT));

        let down = PenInput { in_range: true, contact: true, pressure: 1.0, ..Default::default() };
        let p = down.to_kynput(2, 10, 20);
        assert_eq!(p.pressure, Pen::MAX_PRESSURE);
        assert!(p.has(Pen::CONTACT));
    }

    #[test]
    fn out_of_range_is_never_in_contact() {
        let gone = PenInput { contact: true, pressure: 1.0, ..Default::default() };
        let p = gone.to_kynput(0, 0, 0);
        assert_eq!((p.flags, p.pressure), (0, 0));
    }

    #[test]
    fn tilt_rotation_eraser_and_barrel() {
        let pen = PenInput {
            in_range: true,
            contact: true,
            pressure: 0.5,
            tilt: Some((-120.0, 30.4)),
            rotation: Some(-90.0),
            eraser: true,
            barrel: true,
        };
        let p = pen.to_kynput(0, 0, 0);
        assert_eq!((p.tilt_x, p.tilt_y, p.rotation), (-90, 30, 270));
        for f in [Pen::TILT, Pen::ROTATION, Pen::ERASER, Pen::BARREL] {
            assert!(p.has(f));
        }
        let plain = PenInput { in_range: true, ..Default::default() }.to_kynput(0, 0, 0);
        assert!(!plain.has(Pen::TILT) && !plain.has(Pen::ROTATION));
    }

    #[test]
    fn altitude_azimuth() {
        assert_eq!(tilt_from_altitude_azimuth(FRAC_PI_2, 1.0), (0.0, 0.0));
        let (tx, ty) = tilt_from_altitude_azimuth(FRAC_PI_4, 0.0);
        assert!((tx - 45.0).abs() < 0.01 && ty.abs() < 0.01, "{tx} {ty}");
        let (tx, ty) = tilt_from_altitude_azimuth(FRAC_PI_4, PI / 2.0);
        assert!(tx.abs() < 0.01 && (ty - 45.0).abs() < 0.01, "{tx} {ty}");
        assert_eq!(tilt_from_altitude_azimuth(0.0, PI), (-90.0, 0.0));
    }
}
