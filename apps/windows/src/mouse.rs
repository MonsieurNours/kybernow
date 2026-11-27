//! Souris en mode relatif (jeu) : écarts d'une souris absolue (VM, Bureau à
//! distance, tablette) et découpe d'un écart cumulé en déplacements de 16
//! bits. Rien de Windows : se teste sur le Mac.

/// Coordonnées absolues de Raw Input (0-65535) en pixels d'une surface `w`×`h`.
pub fn absolute_to_pixels(x: i32, y: i32, w: i32, h: i32) -> (i32, i32) {
    let px = (x.clamp(0, 65535) as i64 * w.max(1) as i64 / 65535) as i32;
    let py = (y.clamp(0, 65535) as i64 * h.max(1) as i64 / 65535) as i32;
    (px, py)
}

/// Écart depuis la position absolue précédente (rien au premier point).
pub fn absolute_delta(last: Option<(i32, i32)>, now: (i32, i32)) -> (i64, i64) {
    match last {
        Some((x, y)) => ((now.0 - x) as i64, (now.1 - y) as i64),
        None => (0, 0),
    }
}

/// Un écart cumulé en déplacements de 16 bits (rien s'il est nul).
pub fn split_motion(mut dx: i64, mut dy: i64) -> Vec<(i16, i16)> {
    let mut out = Vec::new();
    while dx != 0 || dy != 0 {
        let x = dx.clamp(i16::MIN as i64, i16::MAX as i64);
        let y = dy.clamp(i16::MIN as i64, i16::MAX as i64);
        out.push((x as i16, y as i16));
        dx -= x;
        dy -= y;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolue_en_pixels() {
        assert_eq!(absolute_to_pixels(0, 0, 1920, 1080), (0, 0));
        assert_eq!(absolute_to_pixels(65535, 65535, 1920, 1080), (1920, 1080));
        assert_eq!(absolute_to_pixels(32768, -5, 1920, 1080), (960, 0));
        assert_eq!(absolute_delta(None, (10, 10)), (0, 0));
        assert_eq!(absolute_delta(Some((10, 10)), (4, 30)), (-6, 20));
    }

    #[test]
    fn decoupe() {
        assert!(split_motion(0, 0).is_empty());
        assert_eq!(split_motion(3, -2), vec![(3, -2)]);
        let parts = split_motion(70000, -40000);
        assert_eq!(parts.iter().map(|p| p.0 as i64).sum::<i64>(), 70000);
        assert_eq!(parts.iter().map(|p| p.1 as i64).sum::<i64>(), -40000);
        assert_eq!(parts.len(), 3);
    }
}
