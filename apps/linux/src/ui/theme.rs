#![allow(dead_code)]
//! Couleurs de la charte KyberNow v2 (brand/v2/tokens/kybernow.css), celles
//! des maquettes Linux et de l'app Windows (apps/windows/ui/src/app.css) :
//! encre, givre, signal Spectre en néon.

use super::canvas::Rgba;

/// Encre : le fond, et ses paliers (panneaux, champs, segments).
pub const INK: Rgba = Rgba::hex(0x05070A);
pub const INK_1: Rgba = Rgba::hex(0x090D12);
pub const INK_2: Rgba = Rgba::hex(0x0E141A);
pub const INK_3: Rgba = Rgba::hex(0x141B23);
/// Fond de l'accueil sous la signature.
pub const INK_BG: Rgba = Rgba::hex(0x070A0E);
/// Panneau translucide (dernière session, menus).
pub const PANEL: Rgba = Rgba(9, 13, 18, 214);
pub const CARD: Rgba = Rgba(14, 20, 26, 218);
/// Lignes.
pub const LINE: Rgba = Rgba::hex(0x1E2832);
pub const LINE_2: Rgba = Rgba::hex(0x2B3742);
/// Givre : le texte, et ses paliers.
pub const FROST: Rgba = Rgba::hex(0xE3EDF0);
pub const MUTED: Rgba = Rgba::hex(0xB4C2C9);
pub const FAINT: Rgba = Rgba::hex(0x8A99A3);
pub const GHOST: Rgba = Rgba::hex(0x65737E);
pub const DIM: Rgba = Rgba::hex(0x4A5560);
/// Signal Spectre (néon), aplat, halo.
pub const SPECTRE: Rgba = Rgba::hex(0x9A8CFF);
pub const FLAT: Rgba = Rgba::hex(0x6D5BFF);
pub const HALO: Rgba = Rgba::hex(0x6D5BFF);
/// Cœur du néon : l'anneau du focus.
pub const CORE: Rgba = Rgba::hex(0xF4F1FF);
/// États.
pub const OK: Rgba = Rgba::hex(0x4CD68A);
pub const WARN: Rgba = Rgba::hex(0xF5B841);
pub const BAD: Rgba = Rgba::hex(0xFF6B6B);
pub const BAD_TEXT: Rgba = Rgba::hex(0xFFC9C4);
pub const BAD_LINE: Rgba = Rgba::hex(0x4A2A2A);
