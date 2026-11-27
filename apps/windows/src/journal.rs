//! « Envoyer le journal » (docs/link-ui.md, « Réglages de l'app », « À
//! propos ») : le journal de l'app, dans un fichier texte
//! `kybernow-journal.txt`, part par la fenêtre « Partager » de Windows
//! (`DataTransferManager`, comme la feuille de partage des autres apps) ;
//! la personne choisit à qui l'envoyer. Rien ne part sans ce geste.
//!
//! Le fichier : l'appareil et son système, la version de l'app, la fin du
//! lancement précédent quand son journal la dit (plantage, panique), puis le
//! journal du lancement précédent et celui de ce lancement (paniques du cœur
//! comprises : elles vont au même journal). L'app n'écrit ni mot de passe, ni
//! jeton, ni code dans son journal. La composition se teste sur le Mac ; le
//! partage est propre à Windows.

/// Le nom du fichier, le même dans toutes les apps.
pub const FILE_NAME: &str = "kybernow-journal.txt";

/// Comment le lancement précédent a fini, d'après son journal : la ligne du
/// plantage (exception native) ou de la panique, sinon rien (fin normale, ou
/// fin que le journal ne dit pas : Windows éteint, app arrêtée de force).
pub fn previous_end(previous: &str) -> Option<String> {
    previous
        .lines()
        .rev()
        .find(|l| l.contains("plantage : exception") || l.contains("panique dans le fil"))
        .map(|l| l.trim().to_string())
}

/// Le fichier envoyé : l'en-tête, puis les deux journaux (le précédent
/// d'abord, absent au premier lancement).
pub fn compose(device: &str, system: &str, version: &str, arch: &str, previous: Option<&str>, current: &str) -> String {
    let mut out = String::new();
    out.push_str("KyberNow Link — journal\n");
    out.push_str(&format!("Appareil : {device}\n"));
    out.push_str(&format!("Système : {system} ({arch})\n"));
    out.push_str(&format!("Version : KyberNow Link {version}\n"));
    match previous.and_then(previous_end) {
        Some(end) => out.push_str(&format!("Fin du lancement précédent : {end}\n")),
        None if previous.is_some() => out.push_str("Fin du lancement précédent : rien de noté (fin normale, ou arrêt que le journal ne dit pas)\n"),
        None => {}
    }
    if let Some(p) = previous.filter(|p| !p.trim().is_empty()) {
        out.push_str("\n=== Lancement précédent (kybernow.prev.log) ===\n");
        out.push_str(p);
        if !p.ends_with('\n') {
            out.push('\n');
        }
    }
    out.push_str("\n=== Ce lancement (kybernow.log) ===\n");
    out.push_str(current);
    out
}

/// Écrit le fichier dans le dossier temporaire de l'app et rend son chemin.
#[cfg(windows)]
pub fn write() -> std::io::Result<std::path::PathBuf> {
    let (current, previous) = crate::system::log_files();
    let read = |p: &std::path::Path| std::fs::read(p).ok().map(|b| String::from_utf8_lossy(&b).into_owned());
    let device = std::env::var("COMPUTERNAME").unwrap_or_default();
    let arch = if cfg!(target_arch = "aarch64") { "ARM64" } else { "x64" };
    let text = compose(&device, &crate::system::os_version(), env!("CARGO_PKG_VERSION"), arch, read(&previous).as_deref(), &read(&current).unwrap_or_default());
    let dir = std::env::temp_dir().join("KyberNow");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(FILE_NAME);
    std::fs::write(&path, text)?;
    Ok(path)
}

/// La fenêtre « Partager » de Windows pour ce fichier, au-dessus de la
/// fenêtre `hwnd`. À appeler hors du fil de la fenêtre (le fichier est lu
/// par Windows), qui fait ensuite le reste sur son fil (`on_main`).
#[cfg(windows)]
pub fn share(path: &std::path::Path, hwnd: isize, on_main: impl FnOnce(Box<dyn FnOnce() + Send>)) -> Result<(), String> {
    use windows::Storage::{IStorageItem, StorageFile};
    use windows::core::{AgileReference, HSTRING, Interface};
    // Le fichier comme objet de Windows, passé d'un fil à l'autre par une
    // référence agile (les objets de WinRT ne se passent pas tels quels).
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))
        .and_then(|op| op.join())
        .and_then(|f| f.cast::<IStorageItem>())
        .and_then(|item| AgileReference::new(&item))
        .map_err(|e| format!("fichier du journal : {e}"))?;
    on_main(Box::new(move || {
        if let Err(e) = show(hwnd, file) {
            log::warn!("journal : fenêtre « Partager » impossible ({e})");
        }
    }));
    Ok(())
}

#[cfg(windows)]
thread_local! {
    /// Le gestionnaire de partage de la fenêtre et l'abonnement en place :
    /// un seul à la fois (le précédent est retiré au partage suivant).
    static REQUESTED: std::cell::RefCell<Option<(windows::ApplicationModel::DataTransfer::DataTransferManager, i64)>> = const { std::cell::RefCell::new(None) };
}

#[cfg(windows)]
fn show(hwnd: isize, file: windows::core::AgileReference<windows::Storage::IStorageItem>) -> windows::core::Result<()> {
    use windows::ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager};
    use windows::Foundation::TypedEventHandler;
    use windows::Storage::IStorageItem;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::IDataTransferManagerInterop;
    use windows::core::{HSTRING, Ref};

    let hwnd = HWND(hwnd as *mut _);
    let interop = windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()?;
    let manager: DataTransferManager = unsafe { interop.GetForWindow(hwnd)? };
    REQUESTED.with(|r| {
        if let Some((m, token)) = r.borrow_mut().take() {
            let _ = m.RemoveDataRequested(token);
        }
    });
    let handler = TypedEventHandler::<DataTransferManager, DataRequestedEventArgs>::new(move |_, args: Ref<DataRequestedEventArgs>| {
        let data = args.ok()?.Request()?.Data()?;
        data.Properties()?.SetTitle(&HSTRING::from(FILE_NAME))?;
        let items: windows_collections::IIterable<IStorageItem> = vec![Some(file.resolve()?)].into();
        data.SetStorageItemsReadOnly(&items)
    });
    let token = manager.DataRequested(&handler)?;
    REQUESTED.with(|r| *r.borrow_mut() = Some((manager, token)));
    unsafe { interop.ShowShareUIForWindow(hwnd) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_tete_et_journaux() {
        let prev = "[INFO] KyberNow Link 0.11.7\n[ERROR] plantage : exception 0xc0000005 à 0x7ff6 (kybernow.exe+0x1234), fil 42\n";
        let text = compose("PC-SALON", "Windows 11 24H2", "0.11.8", "x64", Some(prev), "[INFO] KyberNow Link 0.11.8\n");
        assert!(text.starts_with("KyberNow Link — journal\nAppareil : PC-SALON\nSystème : Windows 11 24H2 (x64)\nVersion : KyberNow Link 0.11.8\n"));
        assert!(text.contains("Fin du lancement précédent : [ERROR] plantage : exception 0xc0000005"));
        // Le précédent d'abord, puis celui-ci.
        let (p, c) = (text.find("=== Lancement précédent").unwrap(), text.find("=== Ce lancement").unwrap());
        assert!(p < c && text.ends_with("[INFO] KyberNow Link 0.11.8\n"));
    }

    #[test]
    fn fin_precedente() {
        assert_eq!(previous_end("[INFO] quitter\n"), None);
        assert!(previous_end("[ERROR] panique dans le fil « main » : x\npile\n").unwrap().contains("panique"));
        // Premier lancement : ni ligne de fin, ni journal précédent.
        let text = compose("PC", "Windows 10 22H2", "0.11.8", "ARM64", None, "");
        assert!(!text.contains("précédent"));
        // Un journal précédent sans plantage : dit tel quel.
        assert!(compose("PC", "W", "1", "x64", Some("[INFO] quitter\n"), "").contains("rien de noté"));
    }
}
