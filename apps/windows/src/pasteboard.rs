//! Presse-papiers de Windows (texte) pour le pont du cœur
//! (`kybernow_core::input::ClipboardBridge`) : un par chaîne d'entrées, en
//! pause quand sa session n'est pas active (plusieurs ordinateurs à la fois).
//! Il remplace celui de kynput, retiré de la kynput.dll du client
//! (`--cfg kybernow_app_clipboard`, correctif 0004) : sa fenêtre cachée
//! n'existait qu'une fois par processus.
//!
//! Le pont l'appelle depuis le fil des entrées (deux fois par seconde au
//! plus, et quand l'ordinateur envoie ou demande du texte). Tout passe par un
//! fil à lui (`kybernow-presse-papiers`), qui tient une fenêtre de messages
//! cachée : le presse-papiers s'ouvre avec elle (vidé puis rempli, il doit
//! appartenir à une fenêtre, voir `OpenClipboard`), et Windows peut lui
//! envoyer ses messages (`WM_DESTROYCLIPBOARD` quand une autre app le vide)
//! sans attendre un fil qui ne les lirait pas. Le presse-papiers ne s'ouvre
//! que pour un programme à la fois : quelques essais espacés de 5 ms, puis on
//! laisse tomber jusqu'au tour suivant.
//!
//! Ce que l'ordinateur y écrit (un mot de passe copié là-bas, par exemple)
//! n'entre ni dans l'historique du presse-papiers de Windows (Win+V) ni dans
//! sa synchronisation entre appareils, comme le faisait kynput : formats
//! `ExcludeClipboardContentFromMonitorProcessing`,
//! `CanIncludeInClipboardHistory` et `CanUploadToCloudClipboard` (à 0).
//! Le lien d'un partage (« Copier le lien », `set_plain_text`) y entre, lui :
//! on le copie pour le coller ailleurs.

#[cfg(windows)]
pub use imp::WinPasteboard;

#[cfg(windows)]
mod imp {
    use std::sync::mpsc;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    use kybernow_core::input::Pasteboard;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard,
        RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG, PostMessageW, RegisterClassExW, TranslateMessage,
        WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WNDCLASSEXW,
    };
    use windows::core::{PCWSTR, w};

    const FORMAT: u32 = CF_UNICODETEXT.0 as u32;
    /// Fenêtre du presse-papiers : un travail attend dans la file.
    const WM_APP_JOB: u32 = WM_APP + 1;
    /// Au-delà, on laisse tomber (le pont réessaie au tour suivant).
    const JOB_TIMEOUT: Duration = Duration::from_secs(1);

    type Job = Box<dyn FnOnce(HWND) + Send>;

    /// Le fil du presse-papiers : sa fenêtre et les travaux qui l'attendent.
    struct Owner {
        hwnd: isize,
        jobs: Mutex<Vec<Job>>,
    }

    static OWNER: OnceLock<Option<Owner>> = OnceLock::new();

    fn owner() -> Option<&'static Owner> {
        OWNER.get_or_init(spawn).as_ref()
    }

    /// Lance le fil et sa fenêtre de messages (une fois par processus).
    fn spawn() -> Option<Owner> {
        let (tx, rx) = mpsc::channel::<isize>();
        let spawned = std::thread::Builder::new().name("kybernow-presse-papiers".into()).spawn(move || unsafe {
            let Ok(instance) = GetModuleHandleW(None) else {
                let _ = tx.send(0);
                return;
            };
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: w!("KyberNowClipboard"),
                ..Default::default()
            };
            RegisterClassExW(&class);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("KyberNowClipboard"),
                w!("KyberNow Link"),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                None,
            );
            let _ = tx.send(hwnd.as_ref().map_or(0, |h| h.0 as isize));
            if hwnd.is_err() {
                return;
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
        if let Err(e) = spawned {
            log::warn!("presse-papiers : fil refusé ({e})");
            return None;
        }
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(h) if h != 0 => Some(Owner { hwnd: h, jobs: Mutex::new(Vec::new()) }),
            _ => {
                log::warn!("presse-papiers : fenêtre cachée refusée, le presse-papiers partagé ne marchera pas");
                None
            }
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_APP_JOB {
            let jobs: Vec<Job> = OWNER.get().and_then(|o| o.as_ref()).map(|o| std::mem::take(&mut *o.jobs.lock().unwrap())).unwrap_or_default();
            for job in jobs {
                job(hwnd);
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// `f` sur le fil du presse-papiers, avec sa fenêtre ; `None` s'il n'a pas
    /// répondu à temps.
    fn on_owner<T: Send + 'static>(f: impl FnOnce(HWND) -> T + Send + 'static) -> Option<T> {
        let owner = owner()?;
        let (tx, rx) = mpsc::sync_channel(1);
        owner.jobs.lock().unwrap().push(Box::new(move |hwnd| {
            let _ = tx.send(f(hwnd));
        }));
        unsafe {
            PostMessageW(Some(HWND(owner.hwnd as *mut _)), WM_APP_JOB, WPARAM(0), LPARAM(0)).ok()?;
        }
        rx.recv_timeout(JOB_TIMEOUT).ok()
    }

    /// Le presse-papiers ouvert par la fenêtre du fil, refermé à la fin.
    struct Open;

    impl Open {
        fn new(hwnd: HWND) -> Option<Self> {
            for _ in 0..10 {
                if unsafe { OpenClipboard(Some(hwnd)) }.is_ok() {
                    return Some(Open);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            None
        }
    }

    impl Drop for Open {
        fn drop(&mut self) {
            let _ = unsafe { CloseClipboard() };
        }
    }

    /// Des octets dans un format du presse-papiers (ouvert et vidé par nous).
    unsafe fn put(format: u32, bytes: &[u8]) -> bool {
        unsafe {
            let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) else { return false };
            let p = GlobalLock(global) as *mut u8;
            if p.is_null() {
                let _ = GlobalFree(Some(global));
                return false;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            let _ = GlobalUnlock(global);
            // Accepté : Windows garde la mémoire ; refusé : à nous de la rendre.
            if SetClipboardData(format, Some(HANDLE(global.0))).is_err() {
                let _ = GlobalFree(Some(global));
                return false;
            }
            true
        }
    }

    /// Un format enregistré par son nom (0 s'il est refusé).
    fn named(name: &str) -> u32 {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
    }

    /// Ce que nous écrivons ne va ni dans l'historique (Win+V) ni dans la
    /// synchronisation du presse-papiers de Windows.
    unsafe fn keep_out_of_history() {
        for (name, data) in [
            ("ExcludeClipboardContentFromMonitorProcessing", &b":3"[..]),
            ("CanIncludeInClipboardHistory", &[0u8, 0, 0, 0][..]),
            ("CanUploadToCloudClipboard", &[0u8, 0, 0, 0][..]),
        ] {
            let format = named(name);
            if format == 0 || !unsafe { put(format, data) } {
                log::warn!("presse-papiers : format {name} refusé");
            }
        }
    }

    fn read(hwnd: HWND) -> Option<String> {
        let _open = Open::new(hwnd)?;
        unsafe {
            let handle = GetClipboardData(FORMAT).ok()?;
            let global = HGLOBAL(handle.0);
            let p = GlobalLock(global) as *const u16;
            if p.is_null() {
                return None;
            }
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            let _ = GlobalUnlock(global);
            Some(text)
        }
    }

    fn write(hwnd: HWND, text: String, private: bool) {
        let Some(_open) = Open::new(hwnd) else {
            log::warn!("presse-papiers : occupé par une autre app, texte de l'ordinateur pas écrit");
            return;
        };
        unsafe {
            if EmptyClipboard().is_err() {
                return;
            }
            if private {
                keep_out_of_history();
            }
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);
            put(FORMAT, bytes);
        }
    }

    pub struct WinPasteboard;

    impl WinPasteboard {
        /// Texte copié par l'utilisateur dans l'app (« Copier le lien ») : il
        /// entre dans l'historique (Win+V) et la synchronisation de Windows.
        pub fn set_plain_text(&self, text: &str) {
            let text = text.to_string();
            if on_owner(move |hwnd| write(hwnd, text, false)).is_none() {
                log::warn!("presse-papiers : le fil du presse-papiers n'a pas répondu, lien pas copié");
            }
        }
    }

    impl Pasteboard for WinPasteboard {
        fn change_count(&self) -> u64 {
            u64::from(unsafe { GetClipboardSequenceNumber() })
        }

        fn text(&self) -> Option<String> {
            on_owner(read).flatten()
        }

        fn set_text(&self, text: &str) {
            let text = text.to_string();
            if on_owner(move |hwnd| write(hwnd, text, true)).is_none() {
                log::warn!("presse-papiers : le fil du presse-papiers n'a pas répondu, texte de l'ordinateur pas écrit");
            }
        }

        fn has_text(&self) -> bool {
            unsafe { IsClipboardFormatAvailable(FORMAT) }.is_ok()
        }
    }
}
