//! Capture d'un écran secondaire par Windows Graphics Capture (WGC).
//!
//! Sur les écrans SudoVDA créés après le premier, la duplication DXGI que
//! Kyber utilise rend une texture où l'image est placée à la position de
//! l'écran sur le bureau (le reste est noir), alors que la composition de
//! Windows est juste. WGC capture ce que DWM compose. Mais WGC refuse un
//! processus SYSTEM (kyavserver), même dans la session de l'utilisateur : cet
//! agent (`display-agent wgc-capture`) tourne donc avec le compte de
//! l'utilisateur, un par écran secondaire d'une session à plusieurs écrans,
//! et dépose chaque image dans des textures D3D11 partagées sur la carte de
//! l'écran (disposition dans `wgc_shared.rs`). Le correctif Kyber 0018
//! (txproto) les reprend sur le GPU à la place de la duplication.
//!
//! Le pointeur est exclu de l'image (l'app le dessine elle-même ; txproto
//! le lit à part). La bordure jaune est refusée quand Windows le permet.
//! L'agent s'arrête quand le fichier `run` disparaît, change de jeton ou
//! n'est plus rafraîchi par Admin depuis 15 s.

use std::path::Path;
use std::ptr::{addr_of, addr_of_mut, null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;
use windows::Foundation::{TimeSpan, TypedEventHandler};
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HMODULE};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX, D3D11_RESOURCE_MISC_SHARED_NTHANDLE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIAdapter, IDXGIAdapter1, IDXGIDevice,
    IDXGIFactory1, IDXGIKeyedMutex, IDXGIResource1,
};
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::System::WinRT::Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::{HRESULT, HSTRING, IInspectable, Interface};

use super::wgc_shared::{
    FORMAT_BGRA8, HEADER_SIZE, Header, MAGIC, SLOTS, STATE_RUNNING, STATE_STARTING, STATE_STOPPED, VERSION, event_name,
    header_name, screen_id, texture_name,
};

mod sys {
    pub use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE, LPARAM, LocalFree, RECT};
    pub use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW, RDW_ALLCHILDREN, RDW_INVALIDATE, RedrawWindow,
    };
    pub use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    pub use windows_sys::Win32::Security::PSECURITY_DESCRIPTOR;
    pub use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_READWRITE,
        UnmapViewOfFile,
    };
    pub use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    pub use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent};
    pub use windows_sys::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
}

/// Sans nouvelles d'Admin (fichier `run`) depuis ce délai, l'agent s'arrête.
const STALE: Duration = Duration::from_secs(15);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Horloge de `QueryPerformanceCounter`, en unités de 100 ns (celle de
/// `SystemRelativeTime`).
fn now_100ns() -> i64 {
    let (mut count, mut freq) = (0i64, 0i64);
    unsafe {
        sys::QueryPerformanceCounter(&mut count);
        sys::QueryPerformanceFrequency(&mut freq);
    }
    if freq <= 0 {
        return 0;
    }
    (i128::from(count) * 10_000_000 / i128::from(freq)) as i64
}

/// Accès aux objets partagés : SYSTEM (kyavserver), le propriétaire et
/// l'utilisateur interactif.
struct Security {
    descriptor: sys::PSECURITY_DESCRIPTOR,
}

impl Security {
    fn new() -> Result<Self> {
        let sddl = wide("D:P(A;;GA;;;SY)(A;;GA;;;OW)(A;;GA;;;IU)");
        let mut descriptor: sys::PSECURITY_DESCRIPTOR = null_mut();
        let ok = unsafe {
            sys::ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), sys::SDDL_REVISION_1, &mut descriptor, null_mut())
        };
        if ok == 0 {
            bail!("descripteur de sécurité : {}", std::io::Error::last_os_error());
        }
        Ok(Self { descriptor })
    }

    fn sys_attributes(&self) -> windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
        windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor,
            bInheritHandle: 0,
        }
    }

    fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.descriptor,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for Security {
    fn drop(&mut self) {
        unsafe { sys::LocalFree(self.descriptor) };
    }
}

/// En-tête partagé et événement « nouvelle image ».
struct Shared {
    mapping: sys::HANDLE,
    view: *mut Header,
    event: sys::HANDLE,
}

// La mémoire partagée est écrite par un seul fil à la fois (verrou de
// l'anneau) ; le compteur d'images est atomique.
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    fn create(id: &str, security: &Security) -> Result<Self> {
        let attrs = security.sys_attributes();
        let name = wide(&header_name(id));
        let mapping = unsafe {
            sys::CreateFileMappingW(sys::INVALID_HANDLE_VALUE, &attrs, sys::PAGE_READWRITE, 0, HEADER_SIZE as u32, name.as_ptr())
        };
        if mapping.is_null() {
            bail!("mémoire partagée {} : {}", header_name(id), std::io::Error::last_os_error());
        }
        let view = unsafe { sys::MapViewOfFile(mapping, sys::FILE_MAP_ALL_ACCESS, 0, 0, HEADER_SIZE) };
        if view.Value.is_null() {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(mapping) };
            bail!("mémoire partagée {} : {}", header_name(id), std::io::Error::last_os_error());
        }
        let event_name = wide(&event_name(id));
        let event = unsafe { sys::CreateEventW(&attrs, 0, 0, event_name.as_ptr()) };
        if event.is_null() {
            bail!("événement {} : {}", self::event_name(id), std::io::Error::last_os_error());
        }
        let shared = Self { mapping, view: view.Value as *mut Header, event };
        unsafe {
            shared.view.write_volatile(Header {
                magic: MAGIC,
                version: VERSION,
                state: STATE_STARTING,
                slots: SLOTS,
                format: FORMAT_BGRA8,
                producer_pid: std::process::id(),
                ..Header::default()
            });
        }
        Ok(shared)
    }

    fn set<T>(&self, field: impl FnOnce(*mut Header) -> *mut T, value: T) {
        unsafe { field(self.view).write_volatile(value) };
    }

    fn heartbeat(&self) {
        self.set(|h| unsafe { addr_of_mut!((*h).heartbeat_time) }, now_100ns());
    }

    /// Nouvelle image dans `slot` : les champs d'abord, le compteur en dernier.
    fn publish(&self, slot: u32, generation: u32, frame_time: i64) {
        self.set(|h| unsafe { addr_of_mut!((*h).latest_slot) }, slot);
        self.set(|h| unsafe { addr_of_mut!((*h).generation) }, generation);
        self.set(|h| unsafe { addr_of_mut!((*h).frame_time) }, frame_time);
        self.set(|h| unsafe { addr_of_mut!((*h).publish_time) }, now_100ns());
        let counter = unsafe { &*(addr_of!((*self.view).frame_counter) as *const AtomicU64) };
        counter.fetch_add(1, Ordering::Release);
        unsafe { sys::SetEvent(self.event) };
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.set(|h| unsafe { addr_of_mut!((*h).state) }, STATE_STOPPED);
        unsafe {
            sys::UnmapViewOfFile(sys::MEMORY_MAPPED_VIEW_ADDRESS { Value: self.view as *mut _ });
            windows_sys::Win32::Foundation::CloseHandle(self.mapping);
            windows_sys::Win32::Foundation::CloseHandle(self.event);
        }
    }
}

/// Les textures partagées d'une génération (une taille d'écran).
struct Ring {
    generation: u32,
    width: u32,
    height: u32,
    textures: Vec<ID3D11Texture2D>,
    mutexes: Vec<IDXGIKeyedMutex>,
    handles: Vec<HANDLE>,
}

impl Ring {
    fn create(device: &ID3D11Device, id: &str, generation: u32, width: u32, height: u32, security: &Security) -> Result<Self> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: (D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0 | D3D11_RESOURCE_MISC_SHARED_KEYEDMUTEX.0) as u32,
        };
        let attrs = security.attributes();
        let mut ring = Ring { generation, width, height, textures: Vec::new(), mutexes: Vec::new(), handles: Vec::new() };
        for slot in 0..SLOTS {
            let mut texture = None;
            unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.context("texture partagée")?;
            let texture = texture.ok_or_else(|| anyhow!("texture partagée absente"))?;
            let resource: IDXGIResource1 = texture.cast()?;
            let name = HSTRING::from(texture_name(id, generation, slot));
            let handle = unsafe {
                resource.CreateSharedHandle(Some(&attrs), DXGI_SHARED_RESOURCE_READ.0 | DXGI_SHARED_RESOURCE_WRITE.0, &name)
            }
            .with_context(|| format!("partage de {name}"))?;
            ring.mutexes.push(texture.cast()?);
            ring.textures.push(texture);
            ring.handles.push(handle);
        }
        Ok(ring)
    }
}

impl Drop for Ring {
    fn drop(&mut self) {
        for handle in &self.handles {
            let _ = unsafe { CloseHandle(*handle) };
        }
    }
}

/// `AcquireSync` rend `WAIT_TIMEOUT`, un code de réussite : on compare au S_OK.
fn acquire(mutex: &IDXGIKeyedMutex, ms: u32) -> bool {
    unsafe { (Interface::vtable(mutex).AcquireSync)(Interface::as_raw(mutex), 0, ms) == HRESULT(0) }
}

struct Device {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt: IDirect3DDevice,
    luid: (u32, i32),
}

// Le contexte immédiat est protégé (`SetMultithreadProtected`) et n'est
// utilisé que sous le verrou de l'état.
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

/// Carte graphique qui affiche l'écran `gdi_name` (celle de Kyber).
fn adapter_for(gdi_name: &str) -> Result<(IDXGIAdapter1, (u32, i32))> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    for i in 0.. {
        let Ok(adapter) = (unsafe { factory.EnumAdapters1(i) }) else { break };
        for j in 0.. {
            let Ok(output) = (unsafe { adapter.EnumOutputs(j) }) else { break };
            let desc = unsafe { output.GetDesc()? };
            let len = desc.DeviceName.iter().position(|c| *c == 0).unwrap_or(desc.DeviceName.len());
            if String::from_utf16_lossy(&desc.DeviceName[..len]).eq_ignore_ascii_case(gdi_name) {
                let luid = unsafe { adapter.GetDesc1()? }.AdapterLuid;
                return Ok((adapter, (luid.LowPart, luid.HighPart)));
            }
        }
    }
    bail!("aucune carte n'affiche {gdi_name}")
}

fn create_device(gdi_name: &str) -> Result<Device> {
    let (adapter, luid) = adapter_for(gdi_name)?;
    let adapter: IDXGIAdapter = adapter.cast()?;
    let (mut device, mut context) = (None, None);
    unsafe {
        D3D11CreateDevice(
            &adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .context("périphérique D3D11")?;
    let device = device.ok_or_else(|| anyhow!("périphérique D3D11 absent"))?;
    let context = context.ok_or_else(|| anyhow!("contexte D3D11 absent"))?;
    // WGC se sert du périphérique depuis ses propres fils.
    if let Ok(multithread) = context.cast::<ID3D11Multithread>() {
        let _ = unsafe { multithread.SetMultithreadProtected(true) };
    }
    let dxgi: IDXGIDevice = device.cast()?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi)? };
    let winrt: IDirect3DDevice = inspectable.cast()?;
    Ok(Device { device, context, winrt, luid })
}

/// Moniteur dont le nom GDI est `gdi_name`.
fn monitor_for(gdi_name: &str) -> Option<HMONITOR> {
    unsafe extern "system" fn each(monitor: sys::HMONITOR, _: sys::HDC, _: *mut sys::RECT, data: sys::LPARAM) -> i32 {
        let found = unsafe { &mut *(data as *mut (Vec<u16>, sys::HMONITOR)) };
        let mut info: sys::MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = size_of::<sys::MONITORINFOEXW>() as u32;
        if unsafe { sys::GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _) } != 0 {
            let len = info.szDevice.iter().position(|c| *c == 0).unwrap_or(info.szDevice.len());
            if info.szDevice[..len] == found.0[..] {
                found.1 = monitor;
                return 0;
            }
        }
        1
    }
    let mut data: (Vec<u16>, sys::HMONITOR) = (gdi_name.encode_utf16().collect(), null_mut());
    unsafe { sys::EnumDisplayMonitors(null_mut(), null(), Some(each), &mut data as *mut _ as sys::LPARAM) };
    (!data.1.is_null()).then(|| HMONITOR(data.1))
}

/// État partagé avec le gestionnaire `FrameArrived`.
struct State {
    id: String,
    device: Device,
    security: Security,
    shared: Shared,
    ring: Mutex<Ring>,
    next_slot: AtomicU32,
    /// La taille de l'écran a changé : l'anneau et la réserve sont à refaire.
    resized: AtomicBool,
    /// L'élément capturé a été fermé (écran retiré) : la capture est à refaire.
    closed: AtomicBool,
    frames: AtomicU64,
}

unsafe impl Send for State {}
unsafe impl Sync for State {}

impl State {
    /// Copie l'image `source` (taille utile `size`) dans la prochaine texture
    /// de l'anneau, puis la publie.
    fn copy(&self, source: &ID3D11Texture2D, size: SizeInt32, frame_time: i64) {
        let ring = self.ring.lock().unwrap();
        if size.Width as u32 != ring.width || size.Height as u32 != ring.height {
            self.resized.store(true, Ordering::SeqCst);
        }
        let slot = self.next_slot.fetch_add(1, Ordering::SeqCst) % SLOTS;
        let mutex = &ring.mutexes[slot as usize];
        if !acquire(mutex, 50) {
            return;
        }
        let area = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: ring.width.min(size.Width.max(0) as u32),
            bottom: ring.height.min(size.Height.max(0) as u32),
            back: 1,
        };
        unsafe {
            self.device.context.CopySubresourceRegion(&ring.textures[slot as usize], 0, 0, 0, 0, source, 0, Some(&area));
            let _ = mutex.ReleaseSync(0);
        }
        self.shared.publish(slot, ring.generation, frame_time);
        self.frames.fetch_add(1, Ordering::Relaxed);
    }

    /// Nouvelle taille : nouvel anneau (génération suivante), annoncé dans l'en-tête.
    fn resize(&self, width: u32, height: u32) -> Result<()> {
        let mut ring = self.ring.lock().unwrap();
        let generation = ring.generation + 1;
        *ring = Ring::create(&self.device.device, &self.id, generation, width, height, &self.security)?;
        self.shared.set(|h| unsafe { addr_of_mut!((*h).width) }, width);
        self.shared.set(|h| unsafe { addr_of_mut!((*h).height) }, height);
        self.shared.set(|h| unsafe { addr_of_mut!((*h).generation) }, generation);
        Ok(())
    }
}

/// Ce que Windows a accepté pour la session de capture.
struct Options {
    cursor_excluded: bool,
    border_required: bool,
}

struct Capture {
    item: GraphicsCaptureItem,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    size: SizeInt32,
    options: Options,
}

impl Capture {
    fn start(state: &Arc<State>, monitor: HMONITOR, refresh: u32) -> Result<Self> {
        let interop = windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .context("Windows Graphics Capture indisponible")?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(monitor) }.context("élément de capture")?;
        let size = item.Size()?;
        // L'anneau à la taille de l'écran avant la première image.
        state.resize(size.Width as u32, size.Height as u32)?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &state.device.winrt,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .context("réserve d'images")?;
        let session = pool.CreateCaptureSession(&item).context("session de capture")?;
        let cursor_excluded = session.SetIsCursorCaptureEnabled(false).is_ok();
        let border_required = session.SetIsBorderRequired(false).is_err();
        if refresh > 0 {
            // Windows 11 24H2 et suivants ; sinon, Windows plafonne à 60 i/s.
            let _ = session.SetMinUpdateInterval(TimeSpan { Duration: 10_000_000 / i64::from(refresh) });
        }
        let handler_state = state.clone();
        pool.FrameArrived(&TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(move |pool, _| {
            let pool = pool.ok()?;
            let frame = pool.TryGetNextFrame()?;
            let size = frame.ContentSize()?;
            let time = frame.SystemRelativeTime()?.Duration;
            let access: IDirect3DDxgiInterfaceAccess = frame.Surface()?.cast()?;
            let texture: ID3D11Texture2D = unsafe { access.GetInterface()? };
            handler_state.copy(&texture, size, time);
            frame.Close()?;
            Ok(())
        }))?;
        let closed_state = state.clone();
        item.Closed(&TypedEventHandler::<GraphicsCaptureItem, IInspectable>::new(move |_, _| {
            closed_state.closed.store(true, Ordering::SeqCst);
            Ok(())
        }))?;
        session.StartCapture().context("début de la capture")?;
        Ok(Self { item, pool, session, size, options: Options { cursor_excluded, border_required } })
    }

    fn stop(&self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
        let _ = &self.item;
    }
}

/// Le fichier `run` porte-t-il encore notre jeton, rafraîchi récemment ?
fn still_wanted(run_file: &Path, token: &str) -> bool {
    let ours = std::fs::read_to_string(run_file).is_ok_and(|t| t.trim() == token);
    let fresh = run_file
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < STALE);
    ours && fresh
}

fn write_status(path: &Path, value: serde_json::Value) {
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, value.to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, path);
    }
}

/// Agent : capture WGC de l'écran `gdi_name` jusqu'à ce qu'Admin n'en veuille
/// plus. `status` reçoit « ready » (ou « failed » et la raison) dès que la
/// capture tourne : Admin l'attend avant de répondre à l'app.
pub fn run_agent(gdi_name: &str, run_file: &Path, token: &str, status: &Path, refresh: u32) -> Result<()> {
    let result = run(gdi_name, run_file, token, status, refresh);
    if let Err(e) = &result {
        write_status(status, json!({ "state": "failed", "error": format!("{e:#}") }));
    }
    result
}

fn run(gdi_name: &str, run_file: &Path, token: &str, status: &Path, refresh: u32) -> Result<()> {
    // Tailles et positions en pixels réels, comme celles de Kyber.
    unsafe { sys::SetProcessDpiAwarenessContext(sys::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.context("WinRT")?;
    let id = screen_id(gdi_name);
    let monitor = monitor_for(gdi_name).ok_or_else(|| anyhow!("écran {gdi_name} introuvable"))?;
    let device = create_device(gdi_name)?;
    let security = Security::new()?;
    let shared = Shared::create(&id, &security)?;
    shared.set(|h| unsafe { addr_of_mut!((*h).luid_low) }, device.luid.0);
    shared.set(|h| unsafe { addr_of_mut!((*h).luid_high) }, device.luid.1);
    shared.set(|h| unsafe { addr_of_mut!((*h).refresh) }, refresh);
    // Anneau provisoire : la taille vient de l'élément de capture.
    let ring = Ring::create(&device.device, &id, 1, 64, 64, &security)?;
    let state = Arc::new(State {
        id: id.clone(),
        device,
        security,
        shared,
        ring: Mutex::new(ring),
        next_slot: AtomicU32::new(0),
        resized: AtomicBool::new(false),
        closed: AtomicBool::new(false),
        frames: AtomicU64::new(0),
    });
    let mut capture = Capture::start(&state, monitor, refresh)?;
    let size = capture.size;
    state.shared.set(|h| unsafe { addr_of_mut!((*h).border_required) }, u32::from(capture.options.border_required));
    state.shared.heartbeat();
    state.shared.set(|h| unsafe { addr_of_mut!((*h).state) }, STATE_RUNNING);
    write_status(
        status,
        json!({
            "state": "ready",
            "width": size.Width, "height": size.Height,
            "cursor_excluded": capture.options.cursor_excluded,
            "border_required": capture.options.border_required,
            "luid": [state.device.luid.0, state.device.luid.1],
            "first_frame": state.frames.load(Ordering::Relaxed) > 0,
            "restarts": 0,
        }),
    );

    let ready = json!({
        "state": "ready",
        "width": size.Width, "height": size.Height,
        "cursor_excluded": capture.options.cursor_excluded,
        "border_required": capture.options.border_required,
        "luid": [state.device.luid.0, state.device.luid.1],
    });
    let started = Instant::now();
    let mut last_retry = Instant::now();
    let mut first_frame_noted = false;
    // Sans image de Windows : écran redessiné à 300 ms, capture refaite à 1 s
    // (trois fois au plus). Un écran tout juste créé et immobile peut ne
    // donner aucune image à WGC.
    let mut capture_started = Instant::now();
    let mut nudged = false;
    let mut restarts = 0u32;
    while still_wanted(run_file, token) && started.elapsed() < Duration::from_secs(86_400) {
        std::thread::sleep(Duration::from_millis(100));
        state.shared.heartbeat();
        let frames = state.frames.load(Ordering::Relaxed);
        if frames == 0 {
            if !nudged && capture_started.elapsed() >= Duration::from_millis(300) {
                nudged = true;
                // Toutes les fenêtres redessinées : Windows recompose l'écran.
                unsafe { sys::RedrawWindow(null_mut(), null(), null_mut(), sys::RDW_INVALIDATE | sys::RDW_ALLCHILDREN) };
            }
            if restarts < 3 && capture_started.elapsed() >= Duration::from_secs(1) {
                restarts += 1;
                capture.stop();
                if let Some(next) = monitor_for(gdi_name).and_then(|m| Capture::start(&state, m, refresh).ok()) {
                    capture = next;
                }
                capture_started = Instant::now();
                nudged = false;
                let mut status_value = ready.clone();
                status_value["first_frame"] = json!(false);
                status_value["restarts"] = json!(restarts);
                write_status(status, status_value);
            }
        }
        // Première image de Windows : Admin l'écrit au journal.
        if !first_frame_noted && frames > 0 {
            first_frame_noted = true;
            let mut status_value = ready.clone();
            status_value["first_frame"] = json!(true);
            status_value["restarts"] = json!(restarts);
            write_status(status, status_value);
        }
        if state.resized.swap(false, Ordering::SeqCst) {
            if let Ok(size) = capture.item.Size() {
                if state.resize(size.Width as u32, size.Height as u32).is_ok() {
                    let _ = capture.pool.Recreate(&state.device.winrt, DirectXPixelFormat::B8G8R8A8UIntNormalized, 2, size);
                }
            }
        }
        // Écran retiré puis rendu (disposition refaite) : nouvelle capture.
        if state.closed.load(Ordering::SeqCst) && last_retry.elapsed() >= Duration::from_secs(1) {
            last_retry = Instant::now();
            capture.stop();
            if let Some(monitor) = monitor_for(gdi_name) {
                if let Ok(next) = Capture::start(&state, monitor, refresh) {
                    state.closed.store(false, Ordering::SeqCst);
                    capture = next;
                }
            }
        }
    }
    capture.stop();
    write_status(status, json!({ "state": "stopped", "frames": state.frames.load(Ordering::Relaxed) }));
    Ok(())
}
