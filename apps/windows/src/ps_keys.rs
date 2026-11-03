//! Touches du clavier pour les consoles PlayStation : le cœur désigne une
//! touche par son usage USB HID (page 7, la place de la touche :
//! `playstation::keymouse`), Windows par son scancode (jeu 1, avec le
//! drapeau « étendue » des touches préfixées E0). La disposition de la
//! console suit ainsi celle du clavier de ce PC (ZQSD en AZERTY) ; le nom
//! montré dans les réglages vient de Windows (`GetKeyNameTextW`, dans la
//! disposition et la langue de ce PC) à partir du scancode d'un usage.
//!
//! Testé sur le Mac.

/// Un scancode de Windows : `(code, étendue)`, tel que `WM_KEYDOWN` le donne
/// dans `lParam` (bits 16 à 23, et bit 24).
pub type Scan = (u16, bool);

/// Scancode → usage HID, et l'inverse. Les touches que la console ne peut pas
/// recevoir utilement (Pause, touches japonaises…) restent dans la table : la
/// disposition peut les prendre.
const TABLE: &[(u16, bool, u16)] = &[
    (0x01, false, 0x29), // Échap
    (0x02, false, 0x1E), // 1
    (0x03, false, 0x1F),
    (0x04, false, 0x20),
    (0x05, false, 0x21),
    (0x06, false, 0x22),
    (0x07, false, 0x23),
    (0x08, false, 0x24),
    (0x09, false, 0x25),
    (0x0A, false, 0x26),
    (0x0B, false, 0x27), // 0
    (0x0C, false, 0x2D), // -
    (0x0D, false, 0x2E), // =
    (0x0E, false, 0x2A), // Effacement
    (0x0F, false, 0x2B), // Tab
    (0x10, false, 0x14), // Q (A en AZERTY)
    (0x11, false, 0x1A), // W (Z en AZERTY)
    (0x12, false, 0x08), // E
    (0x13, false, 0x15), // R
    (0x14, false, 0x17), // T
    (0x15, false, 0x1C), // Y
    (0x16, false, 0x18), // U
    (0x17, false, 0x0C), // I
    (0x18, false, 0x12), // O
    (0x19, false, 0x13), // P
    (0x1A, false, 0x2F), // [
    (0x1B, false, 0x30), // ]
    (0x1C, false, 0x28), // Entrée
    (0x1D, false, 0xE0), // Ctrl gauche
    (0x1E, false, 0x04), // A (Q en AZERTY)
    (0x1F, false, 0x16), // S
    (0x20, false, 0x07), // D
    (0x21, false, 0x09), // F
    (0x22, false, 0x0A), // G
    (0x23, false, 0x0B), // H
    (0x24, false, 0x0D), // J
    (0x25, false, 0x0E), // K
    (0x26, false, 0x0F), // L
    (0x27, false, 0x33), // ;
    (0x28, false, 0x34), // '
    (0x29, false, 0x35), // ` (² en AZERTY)
    (0x2A, false, 0xE1), // Maj gauche
    (0x2B, false, 0x31), // \ (la touche près d'Entrée des claviers ISO aussi)
    (0x2C, false, 0x1D), // Z (W en AZERTY)
    (0x2D, false, 0x1B), // X
    (0x2E, false, 0x06), // C
    (0x2F, false, 0x19), // V
    (0x30, false, 0x05), // B
    (0x31, false, 0x11), // N
    (0x32, false, 0x10), // M
    (0x33, false, 0x36), // ,
    (0x34, false, 0x37), // .
    (0x35, false, 0x38), // /
    (0x36, false, 0xE5), // Maj droite
    (0x37, false, 0x55), // * du pavé numérique
    (0x38, false, 0xE2), // Alt gauche
    (0x39, false, 0x2C), // Espace
    (0x3A, false, 0x39), // Verr. maj.
    (0x3B, false, 0x3A), // F1
    (0x3C, false, 0x3B),
    (0x3D, false, 0x3C),
    (0x3E, false, 0x3D),
    (0x3F, false, 0x3E),
    (0x40, false, 0x3F),
    (0x41, false, 0x40),
    (0x42, false, 0x41),
    (0x43, false, 0x42),
    (0x44, false, 0x43), // F10
    (0x45, false, 0x48), // Pause
    (0x46, false, 0x47), // Arrêt défil.
    (0x47, false, 0x5F), // 7 du pavé
    (0x48, false, 0x60), // 8
    (0x49, false, 0x61), // 9
    (0x4A, false, 0x56), // -
    (0x4B, false, 0x5C), // 4
    (0x4C, false, 0x5D), // 5
    (0x4D, false, 0x5E), // 6
    (0x4E, false, 0x57), // +
    (0x4F, false, 0x59), // 1
    (0x50, false, 0x5A), // 2
    (0x51, false, 0x5B), // 3
    (0x52, false, 0x62), // 0
    (0x53, false, 0x63), // . du pavé
    (0x56, false, 0x64), // < > des claviers ISO
    (0x57, false, 0x44), // F11
    (0x58, false, 0x45), // F12
    (0x64, false, 0x68), // F13
    (0x65, false, 0x69),
    (0x66, false, 0x6A),
    (0x67, false, 0x6B),
    (0x68, false, 0x6C),
    (0x69, false, 0x6D),
    (0x6A, false, 0x6E),
    (0x6B, false, 0x6F),
    (0x6C, false, 0x70),
    (0x6D, false, 0x71),
    (0x6E, false, 0x72), // F23
    (0x76, false, 0x73), // F24
    (0x73, false, 0x87), // Ro (japonais)
    (0x7D, false, 0x89), // Yen
    (0x79, false, 0x8A), // Henkan
    (0x7B, false, 0x8B), // Muhenkan
    (0x1C, true, 0x58),  // Entrée du pavé
    (0x1D, true, 0xE4),  // Ctrl droite
    (0x35, true, 0x54),  // / du pavé
    (0x37, true, 0x46),  // Impr. écran
    (0x38, true, 0xE6),  // Alt droite (AltGr)
    (0x45, true, 0x53),  // Verr. num.
    (0x47, true, 0x4A),  // Origine
    (0x48, true, 0x52),  // Haut
    (0x49, true, 0x4B),  // Page préc.
    (0x4B, true, 0x50),  // Gauche
    (0x4D, true, 0x4F),  // Droite
    (0x4F, true, 0x4D),  // Fin
    (0x50, true, 0x51),  // Bas
    (0x51, true, 0x4E),  // Page suiv.
    (0x52, true, 0x49),  // Inser
    (0x53, true, 0x4C),  // Suppr
    (0x5B, true, 0xE3),  // Windows gauche
    (0x5C, true, 0xE7),  // Windows droite
    (0x5D, true, 0x65),  // Menu
];

/// L'usage HID d'un scancode ; `None` : touche inconnue (rien ne part).
pub fn usage_of(scan: Scan) -> Option<u16> {
    TABLE.iter().find(|(c, e, _)| (*c, *e) == scan).map(|(_, _, u)| *u)
}

/// L'usage HID de la touche d'un `WM_KEYDOWN`/`WM_KEYUP` (son `lParam`).
pub fn usage_of_lparam(lparam: isize) -> Option<u16> {
    let code = ((lparam >> 16) & 0xff) as u16;
    let extended = (lparam >> 24) & 1 == 1;
    usage_of((code, extended))
}

/// Le scancode d'un usage HID (nom de la touche).
pub fn scan_of(usage: u16) -> Option<Scan> {
    TABLE.iter().find(|(_, _, u)| *u == usage).map(|(c, e, _)| (*c, *e))
}

/// Le `lParam` à donner à `GetKeyNameTextW` pour cet usage.
pub fn name_lparam(usage: u16) -> Option<i32> {
    let (code, extended) = scan_of(usage)?;
    Some(((code as i32) << 16) | if extended { 1 << 24 } else { 0 })
}

/// Tous les usages de la table (noms des touches pour les réglages).
pub fn usages() -> impl Iterator<Item = u16> {
    TABLE.iter().map(|(_, _, u)| *u)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kybernow_core::playstation::keymouse::{Input, Layout};

    #[test]
    fn places_des_touches() {
        // W A S D (Z Q S D en AZERTY) : le stick gauche par défaut.
        assert_eq!(usage_of((0x11, false)), Some(0x1A));
        assert_eq!(usage_of((0x1E, false)), Some(0x04));
        assert_eq!(usage_of((0x1F, false)), Some(0x16));
        assert_eq!(usage_of((0x20, false)), Some(0x07));
        // Les flèches sont étendues ; sans le drapeau, ce sont celles du pavé.
        assert_eq!(usage_of((0x48, true)), Some(0x52));
        assert_eq!(usage_of((0x48, false)), Some(0x60));
        assert_eq!(usage_of((0x1D, true)), Some(0xE4));
        assert_eq!(usage_of((0x99, false)), None);
        // lParam de WM_KEYDOWN : flèche gauche (scancode 0x4B, étendue).
        assert_eq!(usage_of_lparam((0x4B << 16) | (1 << 24) | 1), Some(0x50));
    }

    #[test]
    fn chaque_touche_une_seule_fois() {
        for (i, (c, e, u)) in TABLE.iter().enumerate() {
            for (c2, e2, u2) in &TABLE[i + 1..] {
                assert!((c, e) != (c2, e2), "scancode {c:#x} en double");
                assert_ne!(u, u2, "usage {u:#x} en double");
            }
        }
        assert_eq!(scan_of(0x50), Some((0x4B, true)));
        assert_eq!(name_lparam(0x04), Some(0x1E << 16));
    }

    #[test]
    fn la_disposition_par_defaut_est_au_clavier() {
        for (input, _) in Layout::default().bindings {
            if let Input::Key(k) = input {
                assert!(scan_of(k).is_some(), "usage {k:#x} de la disposition par défaut sans scancode");
            }
        }
    }
}
