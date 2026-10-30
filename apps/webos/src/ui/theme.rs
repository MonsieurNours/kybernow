//! Couleurs de la charte KyberNow v2 (brand/v2/tokens/kybernow.css), comme
//! les maquettes webOS : encre, givre, signal Spectre en néon.

use super::canvas::Rgba;

/// Encre : le fond.
pub const INK: Rgba = Rgba::hex(0x05070A);
/// Fond un peu relevé (sous les images).
pub const INK_2: Rgba = Rgba::hex(0x0B0F16);
/// Cartes et panneaux.
pub const PANEL: Rgba = Rgba(14, 19, 28, 230);
pub const PANEL_SOLID: Rgba = Rgba::hex(0x0E131C);
/// Givre : le texte.
pub const FROST: Rgba = Rgba::hex(0xE3EDF0);
pub const FROST_2: Rgba = Rgba::hex(0xB8C6CC);
pub const FROST_3: Rgba = Rgba::hex(0x8C9BA2);
pub const FROST_4: Rgba = Rgba::hex(0x5E6B71);
/// Signal Spectre (néon), aplat, halo.
pub const SPECTRE: Rgba = Rgba::hex(0x9A8CFF);
pub const FLAT: Rgba = Rgba::hex(0x6D5BFF);
pub const HALO: Rgba = Rgba::hex(0x6D5BFF);
/// Cœur du néon : le bouton qui a le focus.
pub const CORE: Rgba = Rgba::hex(0xF4F1FF);
/// Lignes fines.
pub const LINE: Rgba = Rgba(227, 237, 240, 26);
pub const BORDER: Rgba = Rgba(227, 237, 240, 40);
/// Fin de session.
pub const DANGER_BG: Rgba = Rgba(60, 18, 20, 200);
pub const DANGER: Rgba = Rgba::hex(0xFFD9D4);
pub const DANGER_LINE: Rgba = Rgba(255, 138, 128, 102);
/// Attention (prêt : l'avertissement de confiance, l'ordinateur occupé ou prêté), la teinte `--kn-warn` des autres apps.
pub const WARN: Rgba = Rgba::hex(0xF5B841);
