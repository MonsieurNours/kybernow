//! libopus : par pkg-config (Mac : `PKG_CONFIG_PATH` vers le rootfs de Kyber ;
//! Linux : libopus-dev), ou en statique depuis `KYBERNOW_OPUS_LIB` (le dossier
//! de `libopus.a`, Windows : le rootfs de Kyber pour Windows).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=KYBERNOW_OPUS_LIB");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    if let Some(dir) = std::env::var_os("KYBERNOW_OPUS_LIB") {
        println!("cargo:rustc-link-search=native={}", dir.to_string_lossy());
        println!("cargo:rustc-link-lib=static=opus");
        return;
    }
    if pkg_config::Config::new().probe("opus").is_err() {
        println!("cargo:rustc-link-lib=opus");
    }
}
