//! Fonctions vides de VLC pour Android : kyvlcplayer (dépendance de
//! kyclient) référence l'API de libvlc, que l'app n'appelle jamais (l'image
//! et le son vont à l'app par le correctif 0001). Comme sur iOS
//! (apps/apple/scripts/build-apple-mobile.sh), chaque fonction `libvlc_*`
//! déclarée par vlc-rs devient une fonction qui rend 0 / NULL, compilée
//! dans la bibliothèque JNI : sans elles, l'éditeur de liens d'Android
//! refuse les symboles indéfinis. Et libopus, liée ici en statique.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("android") {
        return;
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let vlc_rs = manifest.join("../../../third_party/kyber-desktop/kysdk/kymedia/subprojects/vlc-rs/src");
    let mut names = Vec::new();
    collect(&vlc_rs, &mut names);
    names.sort();
    names.dedup();
    assert!(!names.is_empty(), "aucune fonction libvlc_ trouvée dans {}", vlc_rs.display());
    let mut c = String::from("// Généré par apps/android/jni/build.rs : VLC n'est jamais appelé sur Android.\n");
    for n in &names {
        c.push_str(&format!("void *{n}(void) {{ return 0; }}\n"));
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("vlc_stubs.c");
    fs::write(&out, c).unwrap();
    cc::Build::new().file(&out).warnings(false).compile("kybernow_vlc_stubs");
    // libopus (moteur de son du cœur), statique, du rootfs de la cible
    // (PKG_CONFIG_LIBDIR posé par build-native.sh) : le cœur la laisse à
    // l'exécutable final.
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR");
    if let Err(e) = pkg_config::Config::new().statik(true).probe("opus") {
        panic!("libopus introuvable pour Android ({e}) : lancez apps/android/scripts/build-native.sh");
    }
    println!("cargo:rerun-if-changed={}", vlc_rs.display());
}

/// Les noms `libvlc_*` déclarés par les blocs `extern` de vlc-rs.
fn collect(dir: &std::path::Path, names: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, names);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let Ok(text) = fs::read_to_string(&path) else { continue };
            for part in text.split("pub fn ").skip(1) {
                let name: String = part.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                if name.starts_with("libvlc_") {
                    names.push(name);
                }
            }
        }
    }
}
