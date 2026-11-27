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
use std::sync::{Arc, Mutex, OnceLock};

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
///
/// Rappel de Windows ou de FFmpeg : une panique ne le traverse pas (rattrapée
/// et notée au journal, `ffi_guard!` du cœur), le corps est `ffmpeg_lock_body`.
pub unsafe extern "C" fn ffmpeg_lock(ctx: *mut c_void) {
    kybernow_core::ffi_guard!({ unsafe { ffmpeg_lock_body(ctx) } })
}

unsafe fn ffmpeg_lock_body(ctx: *mut c_void) {
    unsafe { EnterCriticalSection(ctx as *mut CRITICAL_SECTION) };
}

/// Rappel de Windows ou de FFmpeg : une panique ne le traverse pas (rattrapée
/// et notée au journal, `ffi_guard!` du cœur), le corps est `ffmpeg_unlock_body`.
pub unsafe extern "C" fn ffmpeg_unlock(ctx: *mut c_void) {
    kybernow_core::ffi_guard!({ unsafe { ffmpeg_unlock_body(ctx) } })
}

unsafe fn ffmpeg_unlock_body(ctx: *mut c_void) {
    unsafe { LeaveCriticalSection(ctx as *mut CRITICAL_SECTION) };
}

/// La carte graphique de la session : l'appareil, son contexte immédiat et le
/// verrou de ce contexte.
pub struct Gpu {
    pub device: ID3D11Device,
    pub ctx: ID3D11DeviceContext,
    pub lock: Arc<ContextLock>,
    /// Le device PyroWave (Vulkan) de cette carte, créé au premier flux
    /// PyroWave et partagé par tous les écrans ; `None` s'il n'a pas pu l'être.
    pyrowave: OnceLock<Option<Arc<crate::pyrowave::Device>>>,
}

impl Gpu {
    pub fn new() -> windows::core::Result<Arc<Self>> {
        let (device, ctx) = crate::render::create_device()?;
        Ok(Arc::new(Self { device, ctx, lock: ContextLock::new(), pyrowave: OnceLock::new() }))
    }

    /// La carte est perdue (`GetDeviceRemovedReason` en échec).
    pub fn is_lost(&self) -> bool {
        unsafe { self.device.GetDeviceRemovedReason() }.is_err()
    }

    /// Le device PyroWave de la carte de la session (créé une fois).
    pub fn pyrowave(&self) -> Option<Arc<crate::pyrowave::Device>> {
        self.pyrowave
            .get_or_init(|| match crate::pyrowave::Device::new(&self.device) {
                Ok(d) => Some(d),
                Err(e) => {
                    log::error!("PyroWave : {e}");
                    None
                }
            })
            .clone()
    }
}

/// La carte graphique d'une session, remplaçable : après la perte de la carte
/// (pilote mis à jour, TDR, eGPU débranché, certaines sorties de veille), le
/// premier fil de l'image qui la voit en recrée une (`recover`), que les
/// autres écrans et les fenêtres créées ensuite reprennent (`current`).
pub struct SessionGpu {
    current: Mutex<Arc<Gpu>>,
}

impl SessionGpu {
    pub fn new() -> windows::core::Result<Arc<Self>> {
        Ok(Arc::new(Self { current: Mutex::new(Gpu::new()?) }))
    }

    /// La carte en service.
    pub fn current(&self) -> Arc<Gpu> {
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// `lost` est perdue : une carte neuve si c'est encore celle en service
    /// (sinon celle qu'un autre écran a déjà recréée).
    pub fn recover(&self, lost: &Arc<Gpu>) -> windows::core::Result<Arc<Gpu>> {
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        if Arc::ptr_eq(&current, lost) {
            *current = Gpu::new()?;
            log::info!("carte graphique recréée pour la session");
        }
        Ok(current.clone())
    }
}
