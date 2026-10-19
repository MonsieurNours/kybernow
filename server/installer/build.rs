// Ressources Windows (manifeste, icône, version) et chemins des binaires
// embarqués par le lanceur.

use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    // Sans les vrais fichiers (cargo check, tests), on embarque des fichiers vides.
    for var in ["KYBERNOW_UI_EXE", "KYBERNOW_WEBVIEW2_LOADER"] {
        println!("cargo:rerun-if-env-changed={var}");
        let path = match std::env::var(var) {
            Ok(p) => p,
            Err(_) => {
                let p = out.join(format!("{var}.placeholder"));
                std::fs::write(&p, b"").unwrap();
                p.display().to_string()
            }
        };
        println!("cargo:rustc-env={var}={path}");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=resources");
        embed_resource::compile_for("resources/setup.rc", ["KyberNow-Setup"], embed_resource::NONE)
            .manifest_required()
            .unwrap();
        embed_resource::compile_for("resources/ui.rc", ["kybernow-ui"], embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
