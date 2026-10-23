//! Icônes au trait (grille de 24, comme Phosphor et les maquettes), décrites
//! par de courts chemins SVG (M, L, H, V, C, Q, Z, en absolu ou en relatif) et
//! des cercles.

use tiny_skia::{Path, PathBuilder};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Play,
    Monitor,
    Speaker,
    Pointer,
    Gamepad,
    Keyboard,
    Keys,
    Chart,
    Image,
    Logout,
    Plus,
    Info,
    Back,
    Dots,
    Wifi,
    Ethernet,
    Check,
    Power,
    Gear,
    Remote,
    Usb,
    Refresh,
    Close,
    Square,
    Minus,
    Mouse,
    Fullscreen,
    Battery,
    Sound,
    Cpu,
}

impl Icon {
    fn spec(self) -> (&'static [&'static str], &'static [(f32, f32, f32)], &'static [(f32, f32, f32)]) {
        // (chemins au trait, cercles au trait (cx, cy, r), points pleins)
        match self {
            Icon::Play => (&["M7 4 L19 12 L7 20 Z"], &[], &[]),
            Icon::Monitor => (&["M5 5 H19 Q21 5 21 7 V15 Q21 17 19 17 H5 Q3 17 3 15 V7 Q3 5 5 5 Z", "M8 21 H16", "M12 17 V21"], &[], &[]),
            Icon::Speaker => (&["M4 9 H8 L13 5 V19 L8 15 H4 Z", "M16 9 Q18 12 16 15", "M18.5 6.5 Q23 12 18.5 17.5"], &[], &[]),
            Icon::Pointer => (&["M5 3 L17 12 L11 13 L14 20 L11.5 21 L8.5 14 L5 18 Z"], &[], &[]),
            Icon::Gamepad => (
                &["M6 9 H18 Q21 9 21 12 L20 17 Q19.5 19.3 16.5 17.8 L14.5 15 H9.5 L7.5 17.8 Q4.5 19.3 4 17 L3 12 Q3 9 6 9 Z", "M8 11 V14", "M6.5 12.5 H9.5"],
                &[],
                &[(15.5, 11.5, 0.9), (17.5, 13.5, 0.9)],
            ),
            Icon::Keyboard => (&["M4 6 H20 Q22 6 22 8 V16 Q22 18 20 18 H4 Q2 18 2 16 V8 Q2 6 4 6 Z", "M7 14 H17"], &[], &[(6.0, 10.0, 0.8), (10.0, 10.0, 0.8), (14.0, 10.0, 0.8), (18.0, 10.0, 0.8)]),
            Icon::Keys => (&["M5 4 H19 Q21 4 21 6 V18 Q21 20 19 20 H5 Q3 20 3 18 V6 Q3 4 5 4 Z", "M7 9 H9", "M11 9 H13", "M15 9 H17", "M7 13 H17"], &[], &[]),
            Icon::Chart => (&["M3 20 H21", "M6 16 V11", "M11 16 V7", "M16 16 V8", "M20 16 V13"], &[], &[]),
            Icon::Image => (&["M12 3 V6", "M12 18 V21", "M3 12 H6", "M18 12 H21"], &[(12.0, 12.0, 4.0)], &[]),
            Icon::Logout => (&["M10 4 H5 Q4 4 4 5 V19 Q4 20 5 20 H10", "M15 8 L19 12 L15 16", "M19 12 H9"], &[], &[]),
            Icon::Plus => (&["M12 5 V19", "M5 12 H19"], &[], &[]),
            Icon::Info => (&["M11 12 H12 V17 H13"], &[(12.0, 12.0, 9.0)], &[(12.0, 8.0, 1.0)]),
            Icon::Back => (&["M9 14 L4 9 L9 4", "M4 9 H15 Q20 9 20 14 Q20 19 15 19 H12"], &[], &[]),
            Icon::Dots => (&[], &[], &[(5.0, 12.0, 1.6), (12.0, 12.0, 1.6), (19.0, 12.0, 1.6)]),
            Icon::Wifi => (&["M2 9 Q12 0 22 9", "M5.5 12.5 Q12 6.5 18.5 12.5", "M9 16 Q12 13.5 15 16"], &[], &[(12.0, 19.5, 1.3)]),
            Icon::Ethernet => (&["M5 4 H19 Q20 4 20 5 V15 Q20 16 19 16 H5 Q4 16 4 15 V5 Q4 4 5 4 Z", "M8 16 V20 H16 V16", "M8 8 V11", "M11 8 V11", "M13 8 V11", "M16 8 V11"], &[], &[]),
            Icon::Check => (&["M5 12.5 L10 17 L19 7"], &[], &[]),
            Icon::Power => (&["M12 3 V11", "M7 6.5 Q3 10 4.5 15 Q6.5 20.5 12 20.5 Q17.5 20.5 19.5 15 Q21 10 17 6.5"], &[], &[]),
            Icon::Gear => (&["M12 2.5 V5", "M12 19 V21.5", "M2.5 12 H5", "M19 12 H21.5", "M5.3 5.3 L7 7", "M17 17 L18.7 18.7", "M5.3 18.7 L7 17", "M17 7 L18.7 5.3"], &[(12.0, 12.0, 5.5)], &[]),
            Icon::Remote => (&["M9.5 2 H14.5 Q17 2 17 4.5 V19.5 Q17 22 14.5 22 H9.5 Q7 22 7 19.5 V4.5 Q7 2 9.5 2 Z", "M10 14 H14", "M10 17 H14"], &[(12.0, 8.0, 2.0)], &[]),
            Icon::Usb => (&["M12 3 V18", "M9 6 L12 3 L15 6", "M12 14 L7 11 V9", "M12 12 L17 9 V7"], &[(12.0, 19.5, 1.8)], &[]),
            Icon::Refresh => (&["M20 11 Q19 5 12.5 4.5 Q7 4.5 5 9", "M4 13 Q5 19 11.5 19.5 Q17 19.5 19 15", "M20 5 V11 H14", "M4 19 V13 H10"], &[], &[]),
            Icon::Close => (&["M6 6 L18 18", "M18 6 L6 18"], &[], &[]),
            Icon::Square => (&["M6.5 6.5 H17.5 V17.5 H6.5 Z"], &[], &[]),
            Icon::Minus => (&["M5 12.5 H19"], &[], &[]),
            Icon::Mouse => (&["M12 3 Q17 3 17 8 V16 Q17 21 12 21 Q7 21 7 16 V8 Q7 3 12 3 Z", "M12 3 V9"], &[], &[]),
            Icon::Fullscreen => (&["M4 9 V4 H9", "M20 9 V4 H15", "M4 15 V20 H9", "M20 15 V20 H15"], &[], &[]),
            Icon::Battery => (&["M3 8 H18 Q19 8 19 9 V15 Q19 16 18 16 H3 Q2 16 2 15 V9 Q2 8 3 8 Z", "M21 10.5 V13.5"], &[], &[]),
            Icon::Sound => (&["M4 9 H8 L13 5 V19 L8 15 H4 Z", "M16 9 Q18 12 16 15", "M18.5 6.5 Q23 12 18.5 17.5"], &[], &[]),
            Icon::Cpu => (&["M5 5 H19 Q21 5 21 7 V17 Q21 19 19 19 H5 Q3 19 3 17 V7 Q3 5 5 5 Z", "M8 9 L11 12 L8 15", "M13 15 H16"], &[], &[]),
        }
    }

    pub fn strokes(self) -> Vec<Path> {
        let (paths, circles, _) = self.spec();
        let mut out: Vec<Path> = paths.iter().filter_map(|d| parse(d)).collect();
        out.extend(circles.iter().filter_map(|&(x, y, r)| PathBuilder::from_circle(x, y, r)));
        out
    }

    pub fn fills(self) -> Vec<Path> {
        let (_, _, dots) = self.spec();
        dots.iter().filter_map(|&(x, y, r)| PathBuilder::from_circle(x, y, r)).collect()
    }
}

/// Chemin SVG simple : M L H V C Q Z (et leurs relatifs).
pub fn parse(d: &str) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let tokens = tokenize(d);
    let mut i = 0;
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    let mut cmd = 'M';
    let num = |i: &mut usize| -> Option<f32> {
        match tokens.get(*i) {
            Some(Tok::Num(v)) => {
                *i += 1;
                Some(*v)
            }
            _ => None,
        }
    };
    while i < tokens.len() {
        if let Tok::Cmd(c) = tokens[i] {
            cmd = c;
            i += 1;
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let (a, b) = (num(&mut i)? + ox, num(&mut i)? + oy);
                pb.move_to(a, b);
                (x, y, sx, sy) = (a, b, a, b);
                cmd = if rel { 'l' } else { 'L' };
            }
            'L' => {
                let (a, b) = (num(&mut i)? + ox, num(&mut i)? + oy);
                pb.line_to(a, b);
                (x, y) = (a, b);
            }
            'H' => {
                let a = num(&mut i)? + ox;
                pb.line_to(a, y);
                x = a;
            }
            'V' => {
                let b = num(&mut i)? + oy;
                pb.line_to(x, b);
                y = b;
            }
            'C' => {
                let (x1, y1, x2, y2, a, b) =
                    (num(&mut i)? + ox, num(&mut i)? + oy, num(&mut i)? + ox, num(&mut i)? + oy, num(&mut i)? + ox, num(&mut i)? + oy);
                pb.cubic_to(x1, y1, x2, y2, a, b);
                (x, y) = (a, b);
            }
            'Q' => {
                let (x1, y1, a, b) = (num(&mut i)? + ox, num(&mut i)? + oy, num(&mut i)? + ox, num(&mut i)? + oy);
                pb.quad_to(x1, y1, a, b);
                (x, y) = (a, b);
            }
            'Z' => {
                pb.close();
                (x, y) = (sx, sy);
                if let Some(Tok::Num(_)) = tokens.get(i) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    pb.finish()
}

enum Tok {
    Cmd(char),
    Num(f32),
}

fn tokenize(d: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<Tok>| {
        if !cur.is_empty() {
            if let Ok(v) = cur.parse() {
                out.push(Tok::Num(v));
            }
            cur.clear();
        }
    };
    for c in d.chars() {
        if c.is_ascii_alphabetic() {
            flush(&mut cur, &mut out);
            out.push(Tok::Cmd(c));
        } else if c == ' ' || c == ',' {
            flush(&mut cur, &mut out);
        } else if c == '-' && !cur.is_empty() {
            flush(&mut cur, &mut out);
            cur.push(c);
        } else {
            cur.push(c);
        }
    }
    flush(&mut cur, &mut out);
    out
}
