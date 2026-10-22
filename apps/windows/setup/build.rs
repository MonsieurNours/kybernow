// Ressources Windows (manifeste, icône, version tirée de Cargo.toml), binaires
// embarqués par le lanceur, et fichiers de l'interface (ui/dist) embarqués
// dans l'interface.

use std::path::{Path, PathBuf};

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let here = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // Sans les vrais fichiers (cargo check, tests), on embarque des fichiers vides.
    for var in ["KYBERNOW_SETUP_UI_EXE", "KYBERNOW_WEBVIEW2_LOADER"] {
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

    embed_ui(&here.join("ui/dist"), &out.join("assets.rs"));

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=resources");
        let setup = version_resources("setup", "kybernow.ico");
        let ui = version_resources("ui", "kybernow.ico");
        embed_resource::compile_for(&setup, ["KyberNow-Link-Setup"], embed_resource::NONE)
            .manifest_required()
            .unwrap();
        embed_resource::compile_for(&ui, ["kybernow-client-setup-ui"], embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}

/// `assets.rs` : la liste (chemin, contenu) des fichiers de `ui/dist`.
fn embed_ui(dist: &Path, dest: &Path) {
    println!("cargo:rerun-if-changed={}", dist.display());
    let mut files = Vec::new();
    collect(dist, dist, &mut files);
    files.sort();
    let mut code = String::from("pub static ASSETS: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in files {
        println!("cargo:rerun-if-changed={abs}");
        code.push_str(&format!("    ({rel:?}, include_bytes!({abs:?})),\n"));
    }
    code.push_str("];\n");
    std::fs::write(dest, code).unwrap();
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(root, &p, out);
        } else {
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.push((rel, p.display().to_string()));
        }
    }
}

/// Écrit `<nom>.rc` et `<nom>.manifest` dans OUT_DIR à partir des gabarits
/// `resources/<nom>.rc.in` et `.manifest.in` (comme l'installateur du serveur).
fn version_resources(name: &str, icon: &str) -> PathBuf {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("resources");
    let part = |k: &str| std::env::var(k).unwrap().parse::<u16>().expect(k);
    let (major, minor, patch) = (part("CARGO_PKG_VERSION_MAJOR"), part("CARGO_PKG_VERSION_MINOR"), part("CARGO_PKG_VERSION_PATCH"));
    let path = |p: PathBuf| p.display().to_string().replace('\\', "/");
    let manifest = out.join(format!("{name}.manifest"));
    let fill = |template: &str| {
        let text = std::fs::read_to_string(dir.join(template)).unwrap_or_else(|e| panic!("{template} : {e}"));
        text.replace("@VERSION_COMMAS@", &format!("{major},{minor},{patch},0"))
            .replace("@VERSION4@", &format!("{major}.{minor}.{patch}.0"))
            .replace("@VERSION@", &format!("{major}.{minor}.{patch}"))
            .replace("@ICON@", &path(dir.join(icon)))
            .replace("@MANIFEST@", &path(manifest.clone()))
    };
    std::fs::write(&manifest, fill(&format!("{name}.manifest.in"))).unwrap();
    let rc = out.join(format!("{name}.rc"));
    std::fs::write(&rc, fill(&format!("{name}.rc.in"))).unwrap();
    rc
}
