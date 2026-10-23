//! Compilation de l'app Linux (dans le SDK de freedesktop, par flatpak-builder,
//! ou sur un Linux de développement ; voir apps/linux/README.md) :
//!
//! - fonctions vides de VLC : kyvlcplayer (dépendance de kyclient) référence
//!   libvlc, que l'app n'appelle jamais (l'image et le son vont à l'app par le
//!   correctif 0001), comme sur Android, iOS et webOS ;
//! - libopus (moteur de son du cœur) et SDL2 par pkg-config, liées
//!   dynamiquement (le runtime de freedesktop les fournit) ;
//! - FFmpeg réduit au décodage (H.264, HEVC, AV1 par VA-API, H.264 et HEVC
//!   par le processeur), en statique, derrière `src/native/decoder.c` ;
//! - libva et libdrm (runtime) : sonde des décodeurs et du VRR, `src/native/`.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/native");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-env-changed=KYBERNOW_PREFIX");
    println!("cargo:rerun-if-env-changed=KYBERNOW_V4L2_REQUEST");
    if env::var("CARGO_FEATURE_DEVICE").is_err() {
        return;
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    // VLC : une fonction vide par fonction `libvlc_*` de vlc-rs.
    let vlc_rs = manifest.join("../../third_party/kyber-desktop/kysdk/kymedia/subprojects/vlc-rs/src");
    let mut names = Vec::new();
    collect(&vlc_rs, &mut names);
    names.sort();
    names.dedup();
    assert!(!names.is_empty(), "aucune fonction libvlc_ trouvée dans {}", vlc_rs.display());
    let mut c = String::from("// Généré par apps/linux/build.rs : VLC n'est jamais appelé sur Linux.\n");
    for n in &names {
        c.push_str(&format!("void *{n}(void) {{ return 0; }}\n"));
    }
    fs::write(out.join("vlc_stubs.c"), c).unwrap();
    // En fin de ligne (link-arg), après les rlibs de vlc-rs et kyvlcplayer qui
    // les appellent : en `-l`, l'archive passe avant elles et l'éditeur de liens
    // n'en retient rien.
    cc::Build::new().file(out.join("vlc_stubs.c")).warnings(false).cargo_metadata(false).compile("kybernow_vlc_stubs");
    println!("cargo:rustc-link-arg={}", out.join("libkybernow_vlc_stubs.a").display());

    // FFmpeg (statique) : préfixe posé par scripts/build.sh.
    let prefix = PathBuf::from(env::var("KYBERNOW_PREFIX").expect("KYBERNOW_PREFIX (posé par apps/linux/scripts/build.sh)"));
    let va = pkg_config::Config::new().cargo_metadata(false).probe("libva").expect("libva introuvable (pkg-config libva)");
    let drm = pkg_config::Config::new().cargo_metadata(false).probe("libdrm").expect("libdrm introuvable (pkg-config libdrm)");
    // Le FFmpeg de Raspberry Pi (build.sh --rpi) : hwaccel HEVC V4L2 request, avec libudev.
    let v4l2_request = env::var("KYBERNOW_V4L2_REQUEST").is_ok_and(|v| v == "1");
    let mut native = cc::Build::new();
    native
        .file("src/native/decoder.c")
        .file("src/native/display.c")
        .include(prefix.join("include"))
        .flag_if_supported("-Wno-deprecated-declarations")
        .opt_level(2)
        .cargo_metadata(false);
    if v4l2_request {
        native.define("KN_V4L2_REQUEST", None);
    }
    for p in va.include_paths.iter().chain(drm.include_paths.iter()) {
        native.include(p);
    }
    native.compile("kybernow_native");
    println!("cargo:rustc-link-arg={}", out.join("libkybernow_native.a").display());
    // Après l'objet qui les appelle, dans l'ordre des dépendances (libavcodec.pc).
    for lib in ["avcodec", "avutil"] {
        println!("cargo:rustc-link-arg={}", prefix.join(format!("lib/lib{lib}.a")).display());
    }
    println!("cargo:rustc-link-arg=-lva");
    println!("cargo:rustc-link-arg=-lva-drm");
    println!("cargo:rustc-link-arg=-ldrm");
    if v4l2_request {
        println!("cargo:rustc-link-arg=-ludev");
    }
    println!("cargo:rustc-link-arg=-lm");
    println!("cargo:rustc-link-arg=-lpthread");
    // aarch64 : GCC compile les opérations atomiques de FFmpeg en appels à
    // libgcc (`__aarch64_ldadd4_acq_rel`, -moutline-atomics), statique.
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") {
        println!("cargo:rustc-link-arg=-lgcc");
    }

    if let Err(e) = pkg_config::Config::new().probe("opus") {
        panic!("libopus introuvable ({e})");
    }
    if let Err(e) = pkg_config::Config::new().probe("sdl2") {
        panic!("SDL2 introuvable ({e})");
    }
    println!("cargo:rerun-if-changed={}", vlc_rs.display());
}

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
