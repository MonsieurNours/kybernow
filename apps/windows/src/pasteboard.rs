//! Presse-papiers de Windows (texte) pour le pont du cœur
//! (`kybernow_core::input::ClipboardBridge`) : un par chaîne d'entrées, en
//! pause quand sa session n'est pas active (plusieurs ordinateurs à la fois).
//! Il remplace celui de kynput, retiré de la kynput.dll du client
//! (`--cfg kybernow_app_clipboard`, correctif 0004) : sa fenêtre cachée
//! n'existait qu'une fois par processus.
//!
//! Le pont l'appelle depuis le fil des entrées (deux fois par seconde au
//! plus, et quand l'ordinateur envoie ou demande du texte). Le presse-papiers
//! de Windows ne s'ouvre que pour un programme à la fois : quelques essais
//! espacés de 5 ms, puis on laisse tomber jusqu'au tour suivant.

#[cfg(windows)]
pub use imp::WinPasteboard;

#[cfg(windows)]
mod imp {
    use std::time::Duration;

    use kybernow_core::input::Pasteboard;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    const FORMAT: u32 = CF_UNICODETEXT.0 as u32;

    /// Le presse-papiers ouvert, refermé à la fin.
    struct Open;

    impl Open {
        fn new() -> Option<Self> {
            for _ in 0..10 {
                if unsafe { OpenClipboard(None) }.is_ok() {
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

    pub struct WinPasteboard;

    impl Pasteboard for WinPasteboard {
        fn change_count(&self) -> u64 {
            u64::from(unsafe { GetClipboardSequenceNumber() })
        }

        fn text(&self) -> Option<String> {
            let _open = Open::new()?;
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

        fn set_text(&self, text: &str) {
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let Some(_open) = Open::new() else {
                log::warn!("presse-papiers : occupé par une autre app, texte de l'ordinateur pas écrit");
                return;
            };
            unsafe {
                if EmptyClipboard().is_err() {
                    return;
                }
                let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) else { return };
                let p = GlobalLock(global) as *mut u16;
                if p.is_null() {
                    let _ = GlobalFree(Some(global));
                    return;
                }
                std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                let _ = GlobalUnlock(global);
                // Accepté : Windows garde la mémoire ; refusé : à nous de la rendre.
                if SetClipboardData(FORMAT, Some(HANDLE(global.0))).is_err() {
                    let _ = GlobalFree(Some(global));
                }
            }
        }

        fn has_text(&self) -> bool {
            unsafe { IsClipboardFormatAvailable(FORMAT) }.is_ok()
        }
    }
}
