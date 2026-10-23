//! Compilation pour le téléviseur (armv7-unknown-linux-gnueabi, chaîne webOS
//! de webosbrew, voir apps/webos/README.md) :
//!
//! - fonctions vides de VLC : kyvlcplayer (dépendance de kyclient) référence
//!   libvlc, que l'app n'appelle jamais (l'image et le son vont à l'app par le
//!   correctif 0001), comme sur Android et iOS ;
//! - libopus en statique (moteur de son du cœur), du rootfs de la cible ;
//! - cales de glibc : le téléviseur a la glibc 2.12, Rust en demande trois
//!   fonctions plus récentes (`src/glibc_compat.c`, reprises de punktfunk) ;
//! - SDL2 (la version webOS de webosbrew, compilée par scripts/build.sh)
//!   embarquée dans le paquet (`lib/`, trouvée par le RPATH).

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/glibc_compat.c");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR");
    println!("cargo:rerun-if-env-changed=WEBOS_SYSROOT");
    if env::var("CARGO_FEATURE_DEVICE").is_err() {
        return;
    }
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target = env::var("TARGET").unwrap_or_default();
    let webos = target == "armv7-unknown-linux-gnueabi";

    // VLC : une fonction vide par fonction `libvlc_*` de vlc-rs.
    let vlc_rs = manifest.join("../../third_party/kyber-desktop/kysdk/kymedia/subprojects/vlc-rs/src");
    let mut names = Vec::new();
    collect(&vlc_rs, &mut names);
    names.sort();
    names.dedup();
    assert!(!names.is_empty(), "aucune fonction libvlc_ trouvée dans {}", vlc_rs.display());
    let mut c = String::from("// Généré par apps/webos/build.rs : VLC n'est jamais appelé sur webOS.\n");
    for n in &names {
        c.push_str(&format!("void *{n}(void) {{ return 0; }}\n"));
    }
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out.join("vlc_stubs.c"), c).unwrap();
    // En fin de ligne (link-arg), après les rlibs de vlc-rs et kyvlcplayer qui
    // les appellent : en `-l`, l'archive passe avant elles et l'éditeur de liens
    // n'en retient rien.
    cc::Build::new()
        .file(out.join("vlc_stubs.c"))
        .warnings(false)
        .cargo_metadata(false)
        .compile("kybernow_vlc_stubs");
    println!("cargo:rustc-link-arg={}", out.join("libkybernow_vlc_stubs.a").display());

    if let Err(e) = pkg_config::Config::new().statik(true).probe("opus") {
        panic!("libopus introuvable ({e}) : lancez apps/webos/scripts/build.sh");
    }

    if webos {
        // Après la bibliothèque standard dans l'ordre de l'édition de liens : un objet, pas une archive.
        let cc = env::var("CC_armv7_unknown_linux_gnueabi").unwrap_or_else(|_| "cc".into());
        let obj = out.join("glibc_compat.o");
        let ok = std::process::Command::new(&cc)
            .args(["-fPIC", "-O2", "-c", "src/glibc_compat.c", "-o"])
            .arg(&obj)
            .status()
            .unwrap_or_else(|e| panic!("{cc} : {e}"));
        assert!(ok.success(), "{cc} : src/glibc_compat.c ne compile pas");
        println!("cargo:rustc-link-arg={}", obj.display());
        // Crochet sur le lecteur média de LG (src/starfish_hook.c) : un objet
        // lié tel quel et exporté, pour passer devant libplayerAPIs.
        let hook = out.join("starfish_hook.o");
        let ok = std::process::Command::new(&cc)
            .args(["-fPIC", "-O2", "-c", "src/starfish_hook.c", "-o"])
            .arg(&hook)
            .status()
            .unwrap_or_else(|e| panic!("{cc} : {e}"));
        assert!(ok.success(), "{cc} : src/starfish_hook.c ne compile pas");
        println!("cargo:rerun-if-changed=src/starfish_hook.c");
        println!("cargo:rustc-link-arg={}", hook.display());
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=_ZN17StarfishMediaAPIs4LoadEPKcPFvixS1_PvES2_");
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=_ZN17StarfishMediaAPIs4LoadEPKcPFvixS1_E");
        println!("cargo:rustc-link-arg=-Wl,--export-dynamic-symbol=_ZN17StarfishMediaAPIs4FeedB5cxx11EPKc");
        // Encodeur E-AC-3 : libavcodec et libavutil (statiques, du rootfs) derrière src/eac3_shim.c.
        let prefix = PathBuf::from(env::var("PKG_CONFIG_LIBDIR").expect("PKG_CONFIG_LIBDIR")).join("../..");
        let prefix = prefix.canonicalize().unwrap_or(prefix);
        let eac3 = out.join("eac3_shim.o");
        let ok = std::process::Command::new(&cc)
            .args(["-fPIC", "-O2", "-c", "src/eac3_shim.c", "-I"])
            .arg(prefix.join("include"))
            .arg("-o")
            .arg(&eac3)
            .status()
            .unwrap_or_else(|e| panic!("{cc} : {e}"));
        assert!(ok.success(), "{cc} : src/eac3_shim.c ne compile pas");
        println!("cargo:rerun-if-changed=src/eac3_shim.c");
        println!("cargo:rustc-link-arg={}", eac3.display());
        // Après l'objet qui les appelle, dans l'ordre de l'édition de liens (comme les fonctions vides de VLC).
        println!("cargo:rustc-link-arg={}", prefix.join("lib/libavcodec.a").display());
        println!("cargo:rustc-link-arg={}", prefix.join("lib/libavutil.a").display());
        let sdl = env::var("SDL_PREFIX").expect("SDL_PREFIX (posé par scripts/build.sh)");
        println!("cargo:rerun-if-env-changed=SDL_PREFIX");
        println!("cargo:rustc-link-search=native={sdl}/lib");
        println!("cargo:rustc-link-lib=dylib=SDL2-2.0");
        // DT_RPATH (pas RUNPATH) : passe avant LD_LIBRARY_PATH=/usr/lib du
        // bac à sable, où se trouve l'ancienne SDL du téléviseur.
        println!("cargo:rustc-link-arg=-Wl,--disable-new-dtags,-rpath,$ORIGIN/../lib");
    } else {
        // Sur l'ordinateur de développement : la SDL2 du système.
        println!("cargo:rustc-link-lib=dylib=SDL2-2.0");
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
