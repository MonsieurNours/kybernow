//! Intégration à Windows, sans droits d'administrateur (tout dans HKCU) :
//! - le lien `kybernow://` (lien et QR code d'Admin, actions des
//!   notifications) ouvre l'app ;
//! - vos ordinateurs au clic droit de l'icône de la barre des tâches (et du
//!   menu Démarrer pour l'app épinglée) : un clic lance la session ;
//! - des notifications de Windows, seulement quand l'app n'est pas devant.

use std::path::PathBuf;

use crate::i18n::t;

use windows::Win32::Foundation::{HWND, PROPERTYKEY};
use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegSetValueExW,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::Variant::VT_LPWSTR;
use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, SHStrDupW, ShellLink,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
use windows::core::{HSTRING, Interface, PCWSTR};

/// Identité des notifications (inscrite dans HKCU\Software\Classes\AppUserModelId).
/// Ne pas la changer : c'est celle déjà inscrite sur les PC où l'app est
/// installée ; le nom montré est [`DISPLAY_NAME`].
pub const AUMID: &str = "LesLignesClaires.KyberNow";
/// Nom affiché de l'app : notifications, lien `kybernow://`, raccourcis.
pub const DISPLAY_NAME: &str = "KyberNow Link";
/// Raccourcis du menu Démarrer et du Bureau.
const SHORTCUT: &str = "KyberNow Link.lnk";
/// Nom des raccourcis posés par les versions 0.3.1 et avant : retiré quand les
/// nouveaux sont posés.
const OLD_SHORTCUT: &str = "KyberNow.lnk";

fn exe() -> PathBuf {
    std::env::current_exe().unwrap_or_default()
}

fn set_string(root: HKEY, path: &str, name: Option<&str>, value: &str) -> bool {
    unsafe {
        let mut key = HKEY::default();
        let r = RegCreateKeyExW(
            root,
            &HSTRING::from(path),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        );
        if r.is_err() {
            return false;
        }
        let data: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
        let name = name.map(HSTRING::from);
        let r = RegSetValueExW(key, name.as_ref().map(|n| PCWSTR(n.as_ptr())).unwrap_or(PCWSTR::null()), None, REG_SZ, Some(bytes));
        let _ = RegCloseKey(key);
        r.is_ok()
    }
}

/// Lien `kybernow://` et identité des notifications, pour cet utilisateur.
/// Refait à chaque lancement : l'app a pu changer de dossier.
pub fn register() -> bool {
    let exe = exe();
    let exe_s = exe.display().to_string();
    let base = r"Software\Classes\kybernow";
    let ok = set_string(HKEY_CURRENT_USER, base, None, &format!("URL:{DISPLAY_NAME}"))
        && set_string(HKEY_CURRENT_USER, base, Some("URL Protocol"), "")
        && set_string(HKEY_CURRENT_USER, &format!(r"{base}\DefaultIcon"), None, &format!("\"{exe_s}\",0"))
        && set_string(HKEY_CURRENT_USER, &format!(r"{base}\shell\open\command"), None, &format!("\"{exe_s}\" \"%1\""));
    let aumid = format!(r"Software\Classes\AppUserModelId\{AUMID}");
    let mut ok2 = set_string(HKEY_CURRENT_USER, &aumid, Some("DisplayName"), DISPLAY_NAME);
    let png = exe.with_file_name("kybernow.png");
    if png.exists() {
        ok2 &= set_string(HKEY_CURRENT_USER, &aumid, Some("IconUri"), &png.display().to_string());
    }
    log::info!("intégration : lien kybernow:// {}, notifications {}", if ok { "inscrit" } else { "refusé" }, if ok2 { "inscrites" } else { "refusées" });
    ok && ok2
}

/// Dossier connu de Windows (le Bureau peut être déplacé, dans OneDrive par exemple).
fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let dir = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        dir.map(PathBuf::from)
    }
}

/// Raccourci `KyberNow Link.lnk` vers l'app.
fn save_shortcut(path: &std::path::Path) -> windows::core::Result<()> {
    use windows::Win32::System::Com::IPersistFile;
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let exe = exe();
        link.SetPath(&HSTRING::from(exe.as_os_str()))?;
        if let Some(dir) = exe.parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str()))?;
        }
        link.SetDescription(&HSTRING::from(t("shell.shortcut", &[])))?;
        link.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0)?;
        link.cast::<IPersistFile>()?.Save(&HSTRING::from(path.as_os_str()), true)
    }
}

/// Installation (`kybernow.exe --setup-shell [--desktop]`, lancé par
/// l'installateur) : lien `kybernow://`, notifications, raccourci du menu
/// Démarrer et, si demandé, du Bureau. L'ancien raccourci `KyberNow.lnk`
/// (0.3.1 et avant) est retiré là où le nouveau a été posé.
pub fn install(desktop: bool) -> bool {
    use windows::Win32::UI::Shell::FOLDERID_Desktop;
    let mut ok = register();
    let start_menu = std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join(r"Microsoft\Windows\Start Menu\Programs"));
    let desktop_dir = if desktop { known_folder(&FOLDERID_Desktop) } else { None };
    let dirs: Vec<PathBuf> = [start_menu, desktop_dir].into_iter().flatten().collect();
    ok &= std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut ok = true;
        for dir in &dirs {
            let _ = std::fs::create_dir_all(dir);
            let link = dir.join(SHORTCUT);
            match save_shortcut(&link) {
                Ok(()) => {
                    log::info!("raccourci : {}", link.display());
                    let old = dir.join(OLD_SHORTCUT);
                    if old.exists() {
                        match std::fs::remove_file(&old) {
                            Ok(()) => log::info!("ancien raccourci retiré : {}", old.display()),
                            Err(e) => log::warn!("ancien raccourci {} : {e}", old.display()),
                        }
                    }
                }
                Err(e) => {
                    log::warn!("raccourci {} : {e}", link.display());
                    ok = false;
                }
            }
        }
        CoUninitialize();
        ok
    })
    .join()
    .unwrap_or(false);
    ok
}

/// Une fenêtre de cette app est-elle au premier plan ?
pub fn app_in_front() -> bool {
    unsafe {
        let w: HWND = GetForegroundWindow();
        if w.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(w, Some(&mut pid));
        pid == GetCurrentProcessId()
    }
}

fn link(args: &str, title: &str, description: &str) -> windows::core::Result<IShellLinkW> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let exe = HSTRING::from(exe().as_os_str());
        link.SetPath(&exe)?;
        link.SetArguments(&HSTRING::from(args))?;
        link.SetDescription(&HSTRING::from(description))?;
        link.SetIconLocation(&exe, 0)?;
        let store: IPropertyStore = link.cast()?;
        let mut pv = PROPVARIANT::default();
        (*pv.Anonymous.Anonymous).vt = VT_LPWSTR;
        (*pv.Anonymous.Anonymous).Anonymous.pwszVal = SHStrDupW(&HSTRING::from(title))?;
        let key: PROPERTYKEY = PKEY_Title;
        let r = store.SetValue(&key, &pv).and_then(|_| store.Commit());
        let _ = PropVariantClear(&mut pv);
        r?;
        Ok(link)
    }
}

/// Arguments des éléments que l'utilisateur a retirés de la liste : Windows
/// refuse toute la catégorie si l'un d'eux y revient.
fn removed_args(removed: &IObjectArray) -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let n = removed.GetCount().unwrap_or(0);
        for i in 0..n {
            if let Ok(l) = removed.GetAt::<IShellLinkW>(i) {
                let mut buf = [0u16; 512];
                if l.GetArguments(&mut buf).is_ok() {
                    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                    out.push(String::from_utf16_lossy(&buf[..len]));
                }
            }
        }
    }
    out
}

fn build_jump_list(computers: &[(String, String)]) -> windows::core::Result<()> {
    unsafe {
        let list: ICustomDestinationList = CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
        let mut slots = 0u32;
        let removed: IObjectArray = list.BeginList(&mut slots)?;
        let removed = removed_args(&removed);
        // `slots` : les éléments que Windows montre au plus (réglage du
        // nombre d'éléments récents, 10 par défaut) ; les suivants seraient
        // coupés.
        let wanted: Vec<&(String, String)> =
            computers.iter().filter(|(id, _)| !removed.contains(&format!("--connect {id}"))).take(slots as usize).collect();
        if !wanted.is_empty() {
            let items: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
            for (id, name) in wanted {
                items.AddObject(&link(&format!("--connect {id}"), name, &t("shell.connect_to", &[("name", name)]))?)?;
            }
            // Refusée si « Afficher les éléments récemment ouverts » est coupé
            // dans Windows : les tâches restent, la liste est validée quand même.
            if let Err(e) = list.AppendCategory(&HSTRING::from(t("shell.computers", &[])), &items.cast::<IObjectArray>()?) {
                // Une fois : la liste est refaite à chaque changement.
                static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log::info!("liste de la barre des tâches : catégorie refusée ({e}), « éléments récemment ouverts » coupé dans Windows ?");
                }
            }
        }
        let tasks: IObjectCollection = CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
        tasks.AddObject(&link("--add", &t("shell.add", &[]), &t("shell.add_description", &[]))?)?;
        list.AddUserTasks(&tasks.cast::<IObjectArray>()?)?;
        list.CommitList()
    }
}

/// Vide la liste de la barre des tâches (désinstallation), en attendant la fin.
pub fn clear_jump_list() {
    let _ = std::thread::spawn(|| unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Ok(list) = CoCreateInstance::<_, ICustomDestinationList>(&DestinationList, None, CLSCTX_INPROC_SERVER) {
            let _ = list.DeleteList(None);
        }
        CoUninitialize();
    })
    .join();
}

/// Vos ordinateurs (identifiant, nom) au clic droit de l'icône de l'app.
pub fn update_jump_list(computers: Vec<(String, String)>) {
    std::thread::spawn(move || unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Err(e) = build_jump_list(&computers) {
            log::warn!("liste de la barre des tâches : {e}");
        }
        CoUninitialize();
    });
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Une notification de Windows ; `action` : (libellé, lien `kybernow://…`).
pub fn notify(title: &str, body: &str, action: Option<(&str, &str)>) {
    let actions = action
        .map(|(label, link)| {
            format!(
                r#"<actions><action content="{}" activationType="protocol" arguments="{}"/><action content="OK" activationType="system" arguments="dismiss"/></actions>"#,
                xml_escape(label),
                xml_escape(link)
            )
        })
        .unwrap_or_default();
    let xml = format!(
        r#"<toast activationType="protocol" launch="kybernow://open"><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>{actions}</toast>"#,
        xml_escape(title),
        xml_escape(body)
    );
    std::thread::spawn(move || {
        let r = (|| -> windows::core::Result<()> {
            use windows::Data::Xml::Dom::XmlDocument;
            use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
            let doc = XmlDocument::new()?;
            doc.LoadXml(&HSTRING::from(xml))?;
            let toast = ToastNotification::CreateToastNotification(&doc)?;
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&toast)
        })();
        if let Err(e) = r {
            log::warn!("notification : {e}");
        }
    });
}
