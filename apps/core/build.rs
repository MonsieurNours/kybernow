//! libopus pour le module du son (`audio`) : trouvée par pkg-config quand il
//! la connaît (sur le Mac : `PKG_CONFIG_PATH` vers le rootfs de Kyber, voir
//! apps/windows/README.md § Construire). Sinon, rien n'est émis : c'est à
//! l'exécutable final de lier libopus (l'app Windows la lie en statique
//! depuis son rootfs, `apps/windows/build.rs`).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_LIBDIR");
    if pkg_config::Config::new().probe("opus").is_err() {
        println!("cargo:warning=libopus non trouvée par pkg-config : l'exécutable final doit la lier lui-même");
    }
}
