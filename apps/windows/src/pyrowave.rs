//! Décodage PyroWave (docs/pyrowave-client-windows.md) : le chemin Vulkan de
//! PyroWave (`libpyrowave-shared-1.dll`, ouverte au lancement), sur la même
//! carte que l'appareil Direct3D 11 de la session (LUID), qui écrit chaque
//! image dans trois textures Direct3D 11 partagées (Y, Cb, Cr ; R8 en SDR,
//! R16 en HDR10). Une fence Direct3D 11 partagée (timeline côté Vulkan)
//! ordonne les deux : le décodage attend que le rendu ait copié l'image
//! précédente, le rendu attend la fin du décodage avant de copier.
//!
//! Sans la DLL, sans le Vulkan du pilote (`vulkan-1.dll`, ouverte par volk
//! dans PyroWave) ou sur une carte qui ne le fait pas tourner, PyroWave est
//! seulement indisponible : la sonde le dit au cœur, rien d'autre ne change.
//!
//! Contrat (docs/pyrowave.md) : format `pwv1` ; paquet de configuration de
//! 16 octets `KNPW` ; un paquet média = une image entière (paquets PyroWave
//! bout à bout), toujours image clé ; image incomplète ou illisible sautée,
//! jamais d'image clé demandée.

#![allow(non_camel_case_types, dead_code)]

use crate::sync::Locked;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use windows::Win32::Foundation::{CloseHandle, GENERIC_ALL, HANDLE, HMODULE, WAIT_OBJECT_0};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGIResource1};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::{Interface, PCSTR, PCWSTR, w};

// --- Vulkan : le peu qu'il en faut, d'après vulkan_core.h ---

type VkImage = u64;
type VkSemaphore = u64;

const VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO: u32 = 14;
const VK_IMAGE_TYPE_2D: u32 = 1;
const VK_FORMAT_R8_UNORM: u32 = 9;
const VK_FORMAT_R16_UNORM: u32 = 70;
const VK_SAMPLE_COUNT_1_BIT: u32 = 1;
const VK_IMAGE_TILING_OPTIMAL: u32 = 0;
const VK_IMAGE_CREATE_MUTABLE_FORMAT_BIT: u32 = 0x8;
const VK_IMAGE_USAGE_TRANSFER_SRC_BIT: u32 = 0x1;
const VK_IMAGE_USAGE_TRANSFER_DST_BIT: u32 = 0x2;
const VK_IMAGE_USAGE_SAMPLED_BIT: u32 = 0x4;
const VK_IMAGE_USAGE_STORAGE_BIT: u32 = 0x8;
const VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT: u32 = 0x10;
const VK_SHARING_MODE_EXCLUSIVE: u32 = 0;
const VK_IMAGE_LAYOUT_UNDEFINED: u32 = 0;
const VK_IMAGE_ASPECT_PLANE_0_BIT: u32 = 0x10;
const VK_IMAGE_ASPECT_PLANE_1_BIT: u32 = 0x20;
const VK_IMAGE_ASPECT_PLANE_2_BIT: u32 = 0x40;
const VK_EXTERNAL_MEMORY_HANDLE_TYPE_D3D11_TEXTURE_BIT: u32 = 0x8;
const VK_EXTERNAL_SEMAPHORE_HANDLE_TYPE_D3D12_FENCE_BIT: u32 = 0x8;
const VK_SEMAPHORE_TYPE_TIMELINE: u32 = 1;
const VK_QUEUE_GRAPHICS_BIT: u32 = 0x1;
const VK_QUEUE_GLOBAL_PRIORITY_MEDIUM: u32 = 256;
const VK_QUEUE_FAMILY_IGNORED: u32 = !0;
const VK_QUEUE_FAMILY_EXTERNAL: u32 = !0 - 1;

#[repr(C)]
struct VkExtent3D {
    width: u32,
    height: u32,
    depth: u32,
}

#[repr(C)]
struct VkImageCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    image_type: u32,
    format: u32,
    extent: VkExtent3D,
    mip_levels: u32,
    array_layers: u32,
    samples: u32,
    tiling: u32,
    usage: u32,
    sharing_mode: u32,
    queue_family_index_count: u32,
    p_queue_family_indices: *const u32,
    initial_layout: u32,
}

// --- pyrowave.h (API 1.0) ---

pub(crate) type pw_device = *mut c_void;
type pw_decoder = *mut c_void;
type pw_sync = *mut c_void;
type pw_image = *mut c_void;
pub(crate) type pw_result = i32;
pub(crate) const PYROWAVE_SUCCESS: pw_result = 0;
const PYROWAVE_ERROR_NO_VULKAN: pw_result = -5;

#[repr(C)]
struct pw_luid {
    luid: [u8; 8],
}

#[repr(C)]
struct pw_sync_create_info {
    device: pw_device,
    external_handle: usize,
    handle_type: u32,
    semaphore_type: u32,
    import_flags: u32,
}

#[repr(C)]
struct pw_image_create_info {
    device: pw_device,
    external_handle: usize,
    handle_type: u32,
    image_create_info: *const VkImageCreateInfo,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct pw_image_view {
    image: VkImage,
    width: u32,
    height: u32,
    image_format: u32,
    view_format: u32,
    mip_level: u32,
    layer: u32,
    aspect: u32,
    swizzle: u32,
    layout: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct pw_gpu_buffers {
    planes: [pw_image_view; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct pw_external_reference {
    image: pw_image,
    queue_family_index: u32,
}

#[repr(C)]
struct pw_sync_point {
    semaphore: VkSemaphore,
    value: u64,
}

#[repr(C)]
struct pw_sync_operation {
    images: *const pw_external_reference,
    num_images: usize,
    sync: pw_sync_point,
}

#[repr(C)]
pub(crate) struct pw_encoder_create_info {
    pub device: pw_device,
    pub width: i32,
    pub height: i32,
    pub chroma: u32,
}

/// `pyrowave_cpu_buffer` (encodeur de l'essai hors ligne).
#[repr(C)]
pub(crate) struct pw_cpu_buffer {
    pub data: [*mut c_void; 3],
    pub row_stride_in_bytes: [usize; 3],
    pub plane_size_in_bytes: [usize; 3],
    pub width: i32,
    pub height: i32,
    pub format: u32,
}

pub(crate) const PYROWAVE_CPU_BUFFER_FORMAT_YUV420P: u32 = 1;
pub(crate) const PYROWAVE_CPU_BUFFER_FORMAT_YUV444P: u32 = 2;
pub(crate) const PYROWAVE_CPU_BUFFER_FORMAT_YUV420P16: u32 = 5;
pub(crate) const PYROWAVE_CPU_BUFFER_FORMAT_YUV444P16: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct pw_packet {
    pub offset: usize,
    pub size: usize,
}

#[repr(C)]
struct pw_decoder_create_info {
    device: pw_device,
    width: i32,
    height: i32,
    chroma: u32,
    fragment_path: bool,
}

/// Les fonctions de la DLL, lues une fois par processus.
pub(crate) struct Api {
    get_api_version: unsafe extern "C" fn(*mut u32, *mut u32, *mut u32),
    create_device_by_compat: unsafe extern "C" fn(u32, u32, *const c_void, *const c_void, *const pw_luid, u32, *mut pw_device) -> pw_result,
    device_destroy: unsafe extern "C" fn(pw_device),
    device_set_queue_type: unsafe extern "C" fn(pw_device, u32) -> pw_result,
    device_confirm_interop_support: unsafe extern "C" fn(pw_device) -> bool,
    prefers_fragment_path: unsafe extern "C" fn(pw_device) -> bool,
    sync_create: unsafe extern "C" fn(*const pw_sync_create_info, *mut pw_sync) -> pw_result,
    sync_get_semaphore: unsafe extern "C" fn(pw_sync) -> VkSemaphore,
    sync_destroy: unsafe extern "C" fn(pw_sync),
    image_create: unsafe extern "C" fn(*const pw_image_create_info, *mut pw_image) -> pw_result,
    image_get_view: unsafe extern "C" fn(pw_image, u32, u32, *mut pw_image_view) -> pw_result,
    image_destroy: unsafe extern "C" fn(pw_image),
    decoder_create: unsafe extern "C" fn(*const pw_decoder_create_info, *mut pw_decoder) -> pw_result,
    decoder_clear: unsafe extern "C" fn(pw_decoder),
    decoder_push_packet: unsafe extern "C" fn(pw_decoder, *const c_void, usize) -> pw_result,
    decoder_is_ready: unsafe extern "C" fn(pw_decoder, bool) -> bool,
    decoder_decode_gpu: unsafe extern "C" fn(pw_decoder, *const pw_sync_operation, *const pw_sync_operation, *const pw_gpu_buffers) -> pw_result,
    decoder_destroy: unsafe extern "C" fn(pw_decoder),
    // Encodeur : seulement pour l'essai hors ligne (`pyrowave_test.rs`).
    pub(crate) encoder_create: unsafe extern "C" fn(*const pw_encoder_create_info, *mut *mut c_void) -> pw_result,
    pub(crate) encoder_encode_cpu: unsafe extern "C" fn(*mut c_void, *const pw_cpu_buffer, *const usize) -> pw_result,
    pub(crate) encoder_compute_num_packets: unsafe extern "C" fn(*mut c_void, usize, *mut usize) -> pw_result,
    pub(crate) encoder_packetize: unsafe extern "C" fn(*mut c_void, *mut pw_packet, usize, *mut usize, *mut c_void, usize) -> pw_result,
    pub(crate) encoder_destroy: unsafe extern "C" fn(*mut c_void),
}

unsafe impl Send for Api {}
unsafe impl Sync for Api {}

const DLL: PCWSTR = w!("libpyrowave-shared-1.dll");

fn load() -> Result<Api, String> {
    unsafe {
        let lib: HMODULE = LoadLibraryW(DLL).map_err(|e| format!("libpyrowave-shared-1.dll absente ({e})"))?;
        macro_rules! sym {
            ($name:literal) => {{
                let p = GetProcAddress(lib, PCSTR(concat!($name, "\0").as_ptr())).ok_or(concat!("fonction ", $name, " absente"))?;
                std::mem::transmute(p)
            }};
        }
        let api = Api {
            get_api_version: sym!("pyrowave_get_api_version"),
            create_device_by_compat: sym!("pyrowave_create_device_by_compat"),
            device_destroy: sym!("pyrowave_device_destroy"),
            device_set_queue_type: sym!("pyrowave_device_set_queue_type"),
            device_confirm_interop_support: sym!("pyrowave_device_confirm_interop_support"),
            prefers_fragment_path: sym!("pyrowave_decoder_device_prefers_fragment_path"),
            sync_create: sym!("pyrowave_sync_object_create"),
            sync_get_semaphore: sym!("pyrowave_sync_object_get_semaphore"),
            sync_destroy: sym!("pyrowave_sync_object_destroy"),
            image_create: sym!("pyrowave_image_create"),
            image_get_view: sym!("pyrowave_image_get_image_view"),
            image_destroy: sym!("pyrowave_image_destroy"),
            decoder_create: sym!("pyrowave_decoder_create"),
            decoder_clear: sym!("pyrowave_decoder_clear"),
            decoder_push_packet: sym!("pyrowave_decoder_push_packet"),
            decoder_is_ready: sym!("pyrowave_decoder_decode_is_ready"),
            decoder_decode_gpu: sym!("pyrowave_decoder_decode_gpu_buffer"),
            decoder_destroy: sym!("pyrowave_decoder_destroy"),
            encoder_create: sym!("pyrowave_encoder_create"),
            encoder_encode_cpu: sym!("pyrowave_encoder_encode_cpu"),
            encoder_compute_num_packets: sym!("pyrowave_encoder_compute_num_packets"),
            encoder_packetize: sym!("pyrowave_encoder_packetize"),
            encoder_destroy: sym!("pyrowave_encoder_destroy"),
        };
        let (mut major, mut minor, mut patch) = (0, 0, 0);
        (api.get_api_version)(&mut major, &mut minor, &mut patch);
        if major != 1 {
            return Err(format!("API {major}.{minor}.{patch}, 1.x attendue"));
        }
        Ok(api)
    }
}

/// La DLL, ouverte au premier appel (`None` : absente ou d'une autre API).
pub(crate) fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| match load() {
        Ok(a) => Some(a),
        Err(e) => {
            log::info!("PyroWave : {e}");
            None
        }
    })
    .as_ref()
}

fn check(r: pw_result, what: &str) -> Result<(), String> {
    if r == PYROWAVE_SUCCESS { Ok(()) } else { Err(format!("{what} : erreur PyroWave {r}")) }
}

pub use crate::pyrowave_config::Config;

// --- Le device PyroWave d'une carte ---

/// Le device PyroWave (Vulkan) d'une carte graphique : créé une fois par
/// appareil Direct3D 11 (la création compile les pipelines de PyroWave).
pub struct Device {
    api: &'static Api,
    pub(crate) handle: pw_device,
    /// Chemin par fragments (cartes mobiles, Adreno), si PyroWave le recommande.
    pub fragment: bool,
    /// Les appels qui soumettent au GPU, d'un fil à la fois (plusieurs
    /// écrans d'une session partagent le device).
    submit: Mutex<()>,
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { (self.api.device_destroy)(self.handle) };
    }
}

/// Le LUID de la carte de l'appareil Direct3D 11 (8 octets, comme
/// `VkPhysicalDeviceIDProperties::deviceLUID`).
fn adapter_luid(device: &ID3D11Device) -> Result<pw_luid, String> {
    unsafe {
        let dxgi = device.cast::<IDXGIDevice>().map_err(|e| e.to_string())?;
        let desc = dxgi.GetAdapter().and_then(|a| a.GetDesc()).map_err(|e| e.to_string())?;
        let mut luid = [0u8; 8];
        luid[..4].copy_from_slice(&desc.AdapterLuid.LowPart.to_le_bytes());
        luid[4..].copy_from_slice(&desc.AdapterLuid.HighPart.to_le_bytes());
        Ok(pw_luid { luid })
    }
}

impl Device {
    /// Le device PyroWave sur la carte de `d3d` ; une erreur si la DLL, le
    /// Vulkan du pilote ou la carte manquent.
    pub fn new(d3d: &ID3D11Device) -> Result<Arc<Self>, String> {
        let api = api().ok_or("libpyrowave-shared-1.dll indisponible")?;
        let luid = adapter_luid(d3d)?;
        let started = Instant::now();
        let mut handle = std::ptr::null_mut();
        let r = unsafe {
            (api.create_device_by_compat)(0, 0, std::ptr::null(), std::ptr::null(), &luid, VK_QUEUE_GLOBAL_PRIORITY_MEDIUM, &mut handle)
        };
        if r == PYROWAVE_ERROR_NO_VULKAN {
            return Err("pas de Vulkan (vulkan-1.dll du pilote absente)".into());
        }
        check(r, "device Vulkan sur la carte de Direct3D")?;
        let mut dev = Self { api, handle, fragment: false, submit: Mutex::new(()) };
        unsafe {
            if !(api.device_confirm_interop_support)(handle) {
                return Err("la carte ne partage pas ses textures avec Vulkan".into());
            }
            // Le décodage sert au rendu tout de suite : file graphique.
            let _ = (api.device_set_queue_type)(handle, VK_QUEUE_GRAPHICS_BIT);
        }
        dev.fragment = unsafe { (api.prefers_fragment_path)(handle) };
        log::info!(
            "PyroWave : device prêt en {} ms{}",
            started.elapsed().as_millis(),
            if dev.fragment { ", chemin par fragments" } else { "" }
        );
        Ok(Arc::new(dev))
    }
}

// --- Décodeur ---

/// Une image décodée, prête à être copiée par le rendu (`Decoder::copy_into`).
pub struct Decoded {
    pub config: Config,
}

/// Ce qui s'est passé pour un paquet média.
pub enum Outcome {
    /// Image décodée (GPU terminé) : à copier et montrer.
    Ready(Decoded),
    /// Image incomplète, illisible, ou GPU en retard : sautée, rien n'est
    /// demandé à l'ordinateur.
    Skipped(&'static str),
}

/// Trois textures partagées d'une image et leurs images PyroWave.
struct Planes {
    textures: [ID3D11Texture2D; 3],
    images: [pw_image; 3],
    buffers: pw_gpu_buffers,
}

pub struct Decoder {
    device: Arc<Device>,
    d3d: ID3D11Device,
    ctx4: ID3D11DeviceContext4,
    fence: ID3D11Fence,
    /// La fence vue de PyroWave (semaphore timeline importée).
    sync: pw_sync,
    semaphore: VkSemaphore,
    /// Valeurs de la fence : dernière posée par Vulkan (décodage fini) et par
    /// Direct3D (copie faite) ; `next` les donne dans l'ordre.
    next: u64,
    vk_done: u64,
    d3d_done: u64,
    event: HANDLE,
    config: Option<Config>,
    decoder: pw_decoder,
    planes: Option<Planes>,
}

unsafe impl Send for Decoder {}

/// Attente au plus du GPU pour une image (au-delà, elle est sautée).
const GPU_TIMEOUT_MS: u32 = 250;

impl Decoder {
    pub fn new(device: Arc<Device>, d3d: &ID3D11Device) -> Result<Self, String> {
        let api = device.api;
        unsafe {
            let d5 = d3d.cast::<ID3D11Device5>().map_err(|e| format!("ID3D11Device5 : {e}"))?;
            let ctx4 = d3d.GetImmediateContext().map_err(|e| e.to_string())?.cast::<ID3D11DeviceContext4>().map_err(|e| format!("ID3D11DeviceContext4 : {e}"))?;
            let mut fence: Option<ID3D11Fence> = None;
            d5.CreateFence(0, D3D11_FENCE_FLAG_SHARED, &mut fence).map_err(|e| format!("fence partagée : {e}"))?;
            let fence = fence.ok_or("fence partagée absente")?;
            let handle = fence.CreateSharedHandle(None, GENERIC_ALL.0, PCWSTR::null()).map_err(|e| format!("handle de la fence : {e}"))?;
            // PyroWave prend le handle (et le ferme).
            let info = pw_sync_create_info {
                device: device.handle,
                external_handle: handle.0 as usize,
                handle_type: VK_EXTERNAL_SEMAPHORE_HANDLE_TYPE_D3D12_FENCE_BIT,
                semaphore_type: VK_SEMAPHORE_TYPE_TIMELINE,
                import_flags: 0,
            };
            let mut sync = std::ptr::null_mut();
            let r = (api.sync_create)(&info, &mut sync);
            if r != PYROWAVE_SUCCESS {
                let _ = CloseHandle(handle);
                return Err(format!("fence importée dans Vulkan : erreur PyroWave {r}"));
            }
            let semaphore = (api.sync_get_semaphore)(sync);
            let event = CreateEventW(None, false, false, PCWSTR::null()).map_err(|e| e.to_string())?;
            Ok(Self {
                device,
                d3d: d3d.clone(),
                ctx4,
                fence,
                sync,
                semaphore,
                next: 0,
                vk_done: 0,
                d3d_done: 0,
                event,
                config: None,
                decoder: std::ptr::null_mut(),
                planes: None,
            })
        }
    }

    pub fn config(&self) -> Option<Config> {
        self.config
    }

    /// Paquet de configuration : décodeur et textures recréés s'il change.
    pub fn configure(&mut self, c: Config) -> Result<(), String> {
        if self.config == Some(c) && !self.decoder.is_null() {
            return Ok(());
        }
        self.release();
        let api = self.device.api;
        let info = pw_decoder_create_info {
            device: self.device.handle,
            width: c.width as i32,
            height: c.height as i32,
            chroma: c.chroma444 as u32,
            fragment_path: self.device.fragment,
        };
        let mut dec = std::ptr::null_mut();
        check(unsafe { (api.decoder_create)(&info, &mut dec) }, "décodeur")?;
        self.decoder = dec;
        match self.create_planes(&c) {
            Ok(p) => self.planes = Some(p),
            Err(e) => {
                self.release();
                return Err(e);
            }
        }
        self.config = Some(c);
        log::info!("PyroWave : décodeur {}", c.label());
        Ok(())
    }

    fn create_planes(&self, c: &Config) -> Result<Planes, String> {
        let api = self.device.api;
        let (dxgi, vk) = if c.hdr { (DXGI_FORMAT_R16_UNORM, VK_FORMAT_R16_UNORM) } else { (DXGI_FORMAT_R8_UNORM, VK_FORMAT_R8_UNORM) };
        let (cw, ch) = c.chroma_size();
        let sizes = [(c.width, c.height), (cw, ch), (cw, ch)];
        let aspects = [VK_IMAGE_ASPECT_PLANE_0_BIT, VK_IMAGE_ASPECT_PLANE_1_BIT, VK_IMAGE_ASPECT_PLANE_2_BIT];
        let mut textures: Vec<ID3D11Texture2D> = Vec::new();
        let mut images: Vec<pw_image> = Vec::new();
        let mut buffers = pw_gpu_buffers::default();
        let cleanup = |images: &[pw_image]| {
            for i in images {
                unsafe { (api.image_destroy)(*i) };
            }
        };
        for (i, (w, h)) in sizes.into_iter().enumerate() {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: dxgi,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: (D3D11_BIND_SHADER_RESOURCE.0 | D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_UNORDERED_ACCESS.0) as u32,
                CPUAccessFlags: 0,
                MiscFlags: (D3D11_RESOURCE_MISC_SHARED.0 | D3D11_RESOURCE_MISC_SHARED_NTHANDLE.0) as u32,
            };
            let made = (|| -> Result<(ID3D11Texture2D, pw_image), String> {
                let mut t = None;
                unsafe { self.d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.map_err(|e| format!("texture partagée {w}×{h} : {e}"))?;
                let t = t.ok_or("texture partagée absente")?;
                let res = t.cast::<IDXGIResource1>().map_err(|e| e.to_string())?;
                let handle = unsafe { res.CreateSharedHandle(None, GENERIC_ALL.0, PCWSTR::null()) }.map_err(|e| format!("handle de la texture : {e}"))?;
                let mut usage = VK_IMAGE_USAGE_SAMPLED_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_STORAGE_BIT;
                if self.device.fragment {
                    usage |= VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
                }
                let ici = VkImageCreateInfo {
                    s_type: VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO,
                    p_next: std::ptr::null(),
                    flags: VK_IMAGE_CREATE_MUTABLE_FORMAT_BIT,
                    image_type: VK_IMAGE_TYPE_2D,
                    format: vk,
                    extent: VkExtent3D { width: w, height: h, depth: 1 },
                    mip_levels: 1,
                    array_layers: 1,
                    samples: VK_SAMPLE_COUNT_1_BIT,
                    tiling: VK_IMAGE_TILING_OPTIMAL,
                    usage,
                    sharing_mode: VK_SHARING_MODE_EXCLUSIVE,
                    queue_family_index_count: 0,
                    p_queue_family_indices: std::ptr::null(),
                    initial_layout: VK_IMAGE_LAYOUT_UNDEFINED,
                };
                let info = pw_image_create_info {
                    device: self.device.handle,
                    external_handle: handle.0 as usize,
                    handle_type: VK_EXTERNAL_MEMORY_HANDLE_TYPE_D3D11_TEXTURE_BIT,
                    image_create_info: &ici,
                };
                let mut img = std::ptr::null_mut();
                let r = unsafe { (api.image_create)(&info, &mut img) };
                if r != PYROWAVE_SUCCESS {
                    unsafe {
                        let _ = CloseHandle(handle);
                    }
                    return Err(format!("texture importée dans Vulkan : erreur PyroWave {r}"));
                }
                Ok((t, img))
            })();
            let (t, img) = match made {
                Ok(v) => v,
                Err(e) => {
                    cleanup(&images);
                    return Err(e);
                }
            };
            let r = unsafe { (api.image_get_view)(img, aspects[i], VK_IMAGE_USAGE_STORAGE_BIT, &mut buffers.planes[i]) };
            images.push(img);
            textures.push(t);
            if r != PYROWAVE_SUCCESS {
                cleanup(&images);
                return Err(format!("vue du plan {i} : erreur PyroWave {r}"));
            }
        }
        let textures: [ID3D11Texture2D; 3] = textures.try_into().map_err(|_| "textures")?;
        let images: [pw_image; 3] = images.try_into().map_err(|_| "images")?;
        Ok(Planes { textures, images, buffers })
    }

    /// Décode un paquet média (une image entière) dans les textures
    /// partagées, et attend la fin du GPU (au plus `GPU_TIMEOUT_MS`).
    pub fn decode(&mut self, data: &[u8]) -> Result<Outcome, String> {
        if self.config.is_none() {
            if let Some(c) = Config::from_sequence_header(data) {
                log::info!("PyroWave : pas de paquet de configuration, définition lue dans le flux ({})", c.label());
                self.configure(c)?;
            }
        }
        let (Some(config), Some(planes)) = (self.config, self.planes.as_ref()) else {
            return Ok(Outcome::Skipped("pas de configuration"));
        };
        let api = self.device.api;
        let _submit = self.device.submit.locked();
        unsafe {
            // Chaque paquet est une image entière : rien à garder de la précédente.
            (api.decoder_clear)(self.decoder);
            if (api.decoder_push_packet)(self.decoder, data.as_ptr() as *const c_void, data.len()) != PYROWAVE_SUCCESS {
                return Ok(Outcome::Skipped("image illisible"));
            }
            if !(api.decoder_is_ready)(self.decoder, false) && !(api.decoder_is_ready)(self.decoder, true) {
                return Ok(Outcome::Skipped("image incomplète"));
            }
            // Contenu jeté à l'entrée (QUEUE_FAMILY_IGNORED : passage depuis
            // UNDEFINED), rendu à Direct3D à la sortie.
            let acquire_refs = planes.images.map(|image| pw_external_reference { image, queue_family_index: VK_QUEUE_FAMILY_IGNORED });
            let release_refs = planes.images.map(|image| pw_external_reference { image, queue_family_index: VK_QUEUE_FAMILY_EXTERNAL });
            // L'image précédente doit avoir été copiée par le rendu.
            let acquire = pw_sync_operation {
                images: acquire_refs.as_ptr(),
                num_images: 3,
                sync: pw_sync_point { semaphore: if self.d3d_done > 0 { self.semaphore } else { 0 }, value: self.d3d_done },
            };
            self.next += 1;
            let signal = self.next;
            let release = pw_sync_operation { images: release_refs.as_ptr(), num_images: 3, sync: pw_sync_point { semaphore: self.semaphore, value: signal } };
            check((api.decoder_decode_gpu)(self.decoder, &acquire, &release, &planes.buffers), "décodage")?;
            self.vk_done = signal;
        }
        drop(_submit);
        // La fin du GPU, pour le temps de décodage et pour ne jamais laisser
        // Direct3D attendre une fence qui ne vient pas.
        if !self.wait_fence(self.vk_done) {
            return Ok(Outcome::Skipped("GPU en retard"));
        }
        Ok(Outcome::Ready(Decoded { config }))
    }

    fn wait_fence(&self, value: u64) -> bool {
        unsafe {
            if self.fence.GetCompletedValue() >= value {
                return true;
            }
            if self.fence.SetEventOnCompletion(value, self.event).is_err() {
                return false;
            }
            WaitForSingleObject(self.event, GPU_TIMEOUT_MS) == WAIT_OBJECT_0
        }
    }

    /// Copie l'image décodée dans `dst` (Y, Cb, Cr, mêmes formats et tailles),
    /// sur le contexte de Direct3D, verrou du contexte pris par l'appelant ;
    /// puis le décodage suivant peut réécrire les textures partagées.
    pub fn copy_into(&mut self, ctx: &ID3D11DeviceContext, dst: &[ID3D11Texture2D]) -> Result<(), String> {
        let planes = self.planes.as_ref().ok_or("pas de textures")?;
        unsafe {
            self.ctx4.Wait(&self.fence, self.vk_done).map_err(|e| format!("attente de la fence : {e}"))?;
            for (d, s) in dst.iter().zip(planes.textures.iter()) {
                ctx.CopyResource(d, s);
            }
            self.next += 1;
            self.ctx4.Signal(&self.fence, self.next).map_err(|e| format!("fence : {e}"))?;
            self.d3d_done = self.next;
            ctx.Flush();
        }
        Ok(())
    }

    /// Libère décodeur et textures (GPU au repos d'abord).
    fn release(&mut self) {
        let api = self.device.api;
        // Le décodeur attend lui-même la fin du GPU avant de partir.
        if !self.decoder.is_null() {
            unsafe { (api.decoder_destroy)(self.decoder) };
            self.decoder = std::ptr::null_mut();
        }
        if let Some(p) = self.planes.take() {
            // Direct3D a pu garder une copie en file : attendue avant de
            // rendre les textures.
            self.wait_fence(self.d3d_done);
            for i in p.images {
                unsafe { (api.image_destroy)(i) };
            }
        }
        self.config = None;
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        self.release();
        unsafe {
            (self.device.api.sync_destroy)(self.sync);
            let _ = CloseHandle(self.event);
        }
    }
}

// --- Sonde du lancement ---

/// Image de test : 128 × 128 (le plus petit que PyroWave accepte), 4:2:0,
/// rampe de Y de 16 à 235, Cb et Cr à 128 ; la même que celle du Mac
/// (`PyroWaveDecoder.probe`, encodée par l'encodeur Metal du port), 1 812 octets.
const PROBE: &[u8] = include_bytes!("probe/pyrowave.bin");

/// Au lancement (processus de sonde) : device PyroWave sur la carte de
/// `d3d`, décodeur 128 × 128, décodage pour de bon de l'image de test par le
/// chemin de la session (textures partagées, fence), relecture de quelques
/// points, à 4 près. `Err` dit pourquoi PyroWave est indisponible.
pub fn probe(d3d: &ID3D11Device) -> Result<(), String> {
    let started = Instant::now();
    let device = Device::new(d3d)?;
    let mut dec = Decoder::new(device, d3d)?;
    dec.configure(Config { chroma444: false, hdr: false, full_range: false, width: 128, height: 128 })?;
    match dec.decode(PROBE)? {
        Outcome::Ready(_) => {}
        Outcome::Skipped(why) => return Err(format!("image de test : {why}")),
    }
    let ctx = unsafe { d3d.GetImmediateContext() }.map_err(|e| e.to_string())?;
    let staging = |w: u32, h: u32| -> Result<ID3D11Texture2D, String> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut t = None;
        unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.map_err(|e| e.to_string())?;
        t.ok_or_else(|| "texture de relecture".into())
    };
    let dst = [staging(128, 128)?, staging(64, 64)?, staging(64, 64)?];
    dec.copy_into(&ctx, &dst)?;
    let read = |t: &ID3D11Texture2D, x: u32, y: u32| -> Result<u8, String> {
        let mut m = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            ctx.Map(t, 0, D3D11_MAP_READ, 0, Some(&mut m)).map_err(|e| e.to_string())?;
            let v = *(m.pData as *const u8).add((y * m.RowPitch + x) as usize);
            ctx.Unmap(t, 0);
            Ok(v)
        }
    };
    let near = |v: u8, want: f64| (f64::from(v) - want).abs() <= 4.0;
    for x in [0u32, 32, 64, 96, 127] {
        let v = read(&dst[0], x, 64)?;
        let want = 16.0 + f64::from(x) * 219.0 / 127.0;
        if !near(v, want) {
            return Err(format!("image de test : Y = {v} en ({x}, 64), {want:.0} attendu"));
        }
    }
    for x in [0u32, 31, 63] {
        for plane in [1, 2] {
            let v = read(&dst[plane], x, 32)?;
            if !near(v, 128.0) {
                return Err(format!("image de test : chroma = {v} en ({x}, 32), 128 attendu"));
            }
        }
    }
    log::info!("PyroWave : image de test décodée et relue en {} ms", started.elapsed().as_millis());
    Ok(())
}
