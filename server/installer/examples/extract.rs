//! Vérifie le contenu d'un KyberNow-Setup.exe : `cargo run --example extract -- <exe> <dossier>`
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let (exe, dest) = (std::path::Path::new(&args[1]), std::path::Path::new(&args[2]));
    let (footer, exe_len) = kybernow::payload::read_footer(exe)?;
    println!("lanceur {} o, archive {} o, {} fichiers, {} o décompressés", exe_len, footer.archive_len, footer.files, footer.unpacked_bytes);
    let mut last = 0;
    let n = kybernow::payload::extract(exe, dest, |p, _| {
        let pct = (p * 100.0) as u32;
        if pct >= last + 25 { println!("  {pct} %"); last = pct; }
    })?;
    println!("{n} entrées extraites");
    kybernow::payload::copy_stub(exe, &dest.join("uninstall.exe"))?;
    println!("désinstalleur : {} o", std::fs::metadata(dest.join("uninstall.exe"))?.len());
    Ok(())
}
