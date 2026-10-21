// Ressources Windows : icône, manifeste (sans élévation, contrôles communs v6
// pour la boîte de dialogue « Quitter »), informations de version tirées de
// Cargo.toml.

use std::path::PathBuf;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=resources");
        println!("cargo:rerun-if-changed=../installer/resources/kybernow.ico");
        let rc = version_resources("tray", "../../installer/resources/kybernow.ico");
        embed_resource::compile(&rc, embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}

/// Écrit `<nom>.rc` et `<nom>.manifest` dans OUT_DIR à partir des gabarits
/// `resources/<nom>.rc.in` et `.manifest.in` : version du paquet Cargo (celle
/// que release.sh aligne) au lieu d'un numéro figé, chemins absolus (le .rc
/// n'est plus à côté de l'icône).
fn version_resources(name: &str, icon: &str) -> PathBuf {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("resources");
    let part = |k: &str| std::env::var(k).unwrap().parse::<u16>().expect(k);
    let (major, minor, patch) = (part("CARGO_PKG_VERSION_MAJOR"), part("CARGO_PKG_VERSION_MINOR"), part("CARGO_PKG_VERSION_PATCH"));
    // windres et rc.exe acceptent les « / » ; une « \ » serait un échappement.
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
