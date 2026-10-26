//! L'appareil Direct3D 11 d'une session, partagé par tous ses écrans
//! (docs/multi-ecrans.md : un seul appareil, une chaîne d'échange et un
//! décodeur par flux).
//!
//! Chaque écran a son fil (décodeur, rendu, présentation) ; tous parlent à
//! la même carte par le même contexte immédiat. Direct3D protège chaque appel
//! (`ID3D11Multithread`), mais pas une suite d'appels : le contexte garde
//! l'état (cible de rendu, shaders, textures, décodage commencé), qu'un autre
//! fil changerait entre deux appels. Un seul verrou, tenu pendant une suite,
//! l'évite : le rendu d'une image (copie, tracé), et le décodage d'une image
//! par FFmpeg (qui prend le même verrou autour de ses appels de décodage,
//! voir `decoder.rs`). La présentation (qui peut attendre la synchro
//! verticale) se fait toujours hors du verrou.

use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::sync::Arc;

use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext};
use windows::Win32::System::Threading::{CRITICAL_SECTION, DeleteCriticalSection, EnterCriticalSection, InitializeCriticalSection, LeaveCriticalSection};

/// Section critique de Windows : récursive, et utilisable de FFmpeg (C)
/// comme de Rust. Elle ne bouge jamais après sa création.
pub struct ContextLock {
    cs: Box<UnsafeCell<CRITICAL_SECTION>>,
}

unsafe impl Send for ContextLock {}
unsafe impl Sync for ContextLock {}

impl ContextLock {
    pub fn new() -> Arc<Self> {
        let cs = Box::new(UnsafeCell::new(CRITICAL_SECTION::default()));
        unsafe { InitializeCriticalSection(cs.get()) };
        Arc::new(Self { cs })
    }

    /// Prend le verrou jusqu'à la fin de la garde.
    pub fn lock(&self) -> ContextGuard<'_> {
        unsafe { EnterCriticalSection(self.cs.get()) };
        ContextGuard(self)
    }

    /// Le verrou pour FFmpeg (`AVD3D11VADeviceContext::lock_ctx`).
    pub fn raw(&self) -> *mut c_void {
        self.cs.get() as *mut c_void
    }
}

impl Drop for ContextLock {
    fn drop(&mut self) {
        unsafe { DeleteCriticalSection(self.cs.get()) };
    }
}

pub struct ContextGuard<'a>(&'a ContextLock);

impl Drop for ContextGuard<'_> {
    fn drop(&mut self) {
        unsafe { LeaveCriticalSection(self.0.cs.get()) };
    }
}

/// `AVD3D11VADeviceContext::lock` : FFmpeg prend le verrou autour de chaque
/// image décodée.
pub unsafe extern "C" fn ffmpeg_lock(ctx: *mut c_void) {
    unsafe { EnterCriticalSection(ctx as *mut CRITICAL_SECTION) };
}

pub unsafe extern "C" fn ffmpeg_unlock(ctx: *mut c_void) {
    unsafe { LeaveCriticalSection(ctx as *mut CRITICAL_SECTION) };
}

/// La carte graphique de la session : l'appareil, son contexte immédiat et le
/// verrou de ce contexte.
pub struct Gpu {
    pub device: ID3D11Device,
    pub ctx: ID3D11DeviceContext,
    pub lock: Arc<ContextLock>,
}

impl Gpu {
    pub fn new() -> windows::core::Result<Arc<Self>> {
        let (device, ctx) = crate::render::create_device()?;
        Ok(Arc::new(Self { device, ctx, lock: ContextLock::new() }))
    }
}
