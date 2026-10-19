// Ressources Windows : icône, manifeste (sans élévation, contrôles communs v6
// pour la boîte de dialogue « Quitter »), informations de version.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=resources");
        println!("cargo:rerun-if-changed=../installer/resources/kybernow.ico");
        embed_resource::compile("resources/tray.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
