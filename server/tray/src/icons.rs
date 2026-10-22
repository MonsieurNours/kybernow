//! Icônes de KyberNow Station dans la zone de notification, tirées de `kybernow.ico` à la taille
//! demandée par Windows, avec une pastille selon l'état.

use anyhow::{Context, Result};
use tray_icon::Icon;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSMICON};

const ICO: &[u8] = include_bytes!("../../installer/resources/kybernow.ico");

/// Pastille « en marche » (vert des états).
const RUNNING: [u8; 3] = [0x4c, 0xd6, 0x8a];
/// Pastille « session en cours » (Spectre, le signal de la marque).
const SESSION: [u8; 3] = [0x9a, 0x8c, 0xff];
/// Pastille « démarrage ou arrêt en cours ».
const BUSY: [u8; 3] = [0xf5, 0xb9, 0x42];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Station en marche, aucune session.
    Running,
    /// Un appareil est connecté.
    Session,
    /// Démarrage, arrêt ou redémarrage en cours.
    Busy,
    /// Station arrêtée : icône grisée.
    Stopped,
}

pub struct Icons {
    size: u32,
    base: Vec<u8>,
}

impl Icons {
    pub fn load() -> Result<Self> {
        let size = (unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32).min(64);
        let base = base_rgba(size)?;
        Ok(Icons { size, base })
    }

    pub fn get(&self, look: Look) -> Result<Icon> {
        let mut px = self.base.clone();
        match look {
            Look::Running => badge(&mut px, self.size, RUNNING),
            Look::Session => badge(&mut px, self.size, SESSION),
            Look::Busy => badge(&mut px, self.size, BUSY),
            Look::Stopped => grey(&mut px),
        }
        Icon::from_rgba(px, self.size, self.size).context("icône")
    }
}

/// L'image de l'ICO la plus proche de `size` (PNG), ramenée à `size`.
fn base_rgba(size: u32) -> Result<Vec<u8>> {
    let count = u16::from_le_bytes([ICO[4], ICO[5]]) as usize;
    let mut entries: Vec<(u32, &[u8])> = (0..count)
        .filter_map(|i| {
            let e = ICO.get(6 + 16 * i..22 + 16 * i)?;
            let w = if e[0] == 0 { 256 } else { e[0] as u32 };
            let len = u32::from_le_bytes(e[8..12].try_into().ok()?) as usize;
            let off = u32::from_le_bytes(e[12..16].try_into().ok()?) as usize;
            Some((w, ICO.get(off..off + len)?))
        })
        .collect();
    entries.sort_by_key(|(w, _)| *w);
    let (w, data) = entries
        .iter()
        .find(|(w, _)| *w >= size)
        .or(entries.last())
        .copied()
        .context("kybernow.ico vide")?;

    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().context("PNG trop grand")?];
    let info = reader.next_frame(&mut buf)?;
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buf[..info.buffer_size()].chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..info.buffer_size()].chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf[..info.buffer_size()].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => anyhow::bail!("PNG indexé non pris en charge"),
    };
    Ok(if w == size { rgba } else { resize(&rgba, w, size) })
}

/// Réduction par moyenne de zone, en alpha prémultiplié.
fn resize(src: &[u8], from: u32, to: u32) -> Vec<u8> {
    let ratio = from as f32 / to as f32;
    let mut out = vec![0u8; (to * to * 4) as usize];
    for y in 0..to {
        for x in 0..to {
            let (x0, x1) = ((x as f32 * ratio) as u32, (((x + 1) as f32 * ratio).ceil() as u32).min(from));
            let (y0, y1) = ((y as f32 * ratio) as u32, (((y + 1) as f32 * ratio).ceil() as u32).min(from));
            let mut acc = [0f32; 4];
            let mut n = 0f32;
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let p = &src[((sy * from + sx) * 4) as usize..][..4];
                    let a = p[3] as f32 / 255.0;
                    acc[0] += p[0] as f32 * a;
                    acc[1] += p[1] as f32 * a;
                    acc[2] += p[2] as f32 * a;
                    acc[3] += a;
                    n += 1.0;
                }
            }
            let o = &mut out[((y * to + x) * 4) as usize..][..4];
            if acc[3] > 0.0 {
                for c in 0..3 {
                    o[c] = (acc[c] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
                o[3] = (acc[3] / n * 255.0).round() as u8;
            }
        }
    }
    out
}

/// Pastille en bas à droite, détourée d'un liseré transparent.
fn badge(px: &mut [u8], size: u32, color: [u8; 3]) {
    let s = size as f32;
    let r = s * 0.22;
    let gap = (s / 16.0).max(1.0);
    let c = s - r - s * 0.02;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            let p = &mut px[((y * size + x) * 4) as usize..][..4];
            let keep = (d - r - gap + 0.5).clamp(0.0, 1.0);
            let mut a = p[3] as f32 / 255.0 * keep;
            let cover = (r - d + 0.5).clamp(0.0, 1.0);
            let out_a = cover + a * (1.0 - cover);
            for i in 0..3 {
                let v = color[i] as f32 * cover + p[i] as f32 * a * (1.0 - cover);
                p[i] = if out_a > 0.0 { (v / out_a).round() as u8 } else { 0 };
            }
            a = out_a;
            p[3] = (a * 255.0).round() as u8;
        }
    }
}

/// Station arrêtée : niveaux de gris, estompé.
fn grey(px: &mut [u8]) {
    for p in px.chunks_mut(4) {
        let l = (p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114) as u8;
        p[0] = l;
        p[1] = l;
        p[2] = l;
        p[3] = (p[3] as f32 * 0.55) as u8;
    }
}
