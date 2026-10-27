//! QR code du lien d'un partage (KyberNow Share), dessiné en SVG pour le
//! panneau : sans réseau ni fichier, du même dessin que celui d'Admin
//! (`server/web/src/lib/Qr.svelte` : fond clair, marge de deux modules).

use qrcodegen::{QrCode, QrCodeEcc};

/// Le SVG du QR code de `text`, `None` si le texte est trop long pour un QR code.
pub fn svg(text: &str) -> Option<String> {
    let code = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let n = code.size();
    let mut path = String::new();
    for y in 0..n {
        for x in 0..n {
            if code.get_module(x, y) {
                path.push_str(&format!("M{x} {y}h1v1h-1z"));
            }
        }
    }
    let side = n + 4;
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 {side} {side}" shape-rendering="crispEdges"><rect x="-2" y="-2" width="{side}" height="{side}" fill="#F2F8FA" rx="2"/><path d="{path}" fill="#05070A"/></svg>"##
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn lien_du_partage() {
        let svg = super::svg("kybernow://join?host=192.168.1.20&port=24811&fp=sha256:ab&code=123456").unwrap();
        assert!(svg.starts_with("<svg") && svg.contains("<path d=\"M"));
    }
}
