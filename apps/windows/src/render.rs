//! Rendu Direct3D 11 : l'image décodée (sur la carte graphique ou venue du
//! processeur) est convertie en RVB par un petit shader, à la taille de la
//! fenêtre en gardant ses proportions, puis présentée aussitôt (chaîne
//! d'échange « flip », déchirure permise sans synchro verticale). Le trait
//! du bord haut, qui signale où s'ouvre la barre de session, est tracé dans
//! la même passe, par-dessus l'image.
//!
//! Un `Renderer` par écran (fenêtre), tous sur l'appareil de la session
//! (`gpu.rs`) : le tracé d'une image est une suite d'appels sur le contexte
//! partagé, tenue sous son verrou ; la présentation se fait dehors, et
//! n'attend jamais la synchro verticale à l'intérieur de Direct3D (la chaîne
//! d'échange a son objet d'attente, attendu avant le tracé) : un écran à 60 Hz
//! ne retient pas celui à 160 Hz.

use std::ffi::c_void;
use std::sync::Arc;

use windows::Win32::Foundation::{HANDLE, HMODULE, HWND, RECT};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_0,
    D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
    D3D_SRV_DIMENSION_TEXTURE2D, ID3DBlob,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::WaitForSingleObjectEx;
use windows::core::{BOOL, Interface, PCSTR, s};

use crate::decoder::ff::*;
use crate::decoder::is_hw_frame;
use crate::gpu::{ContextLock, Gpu};

const SHADER: &str = r#"
struct VSOut { float4 pos : SV_Position; float2 uv : TEXCOORD0; };
VSOut vs(uint id : SV_VertexID) {
    VSOut o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.uv = uv;
    o.pos = float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return o;
}
Texture2D t0 : register(t0);
Texture2D t1 : register(t1);
Texture2D t2 : register(t2);
SamplerState smp : register(s0);
cbuffer Params : register(b0) { float4 m0; float4 m1; float4 m2; uint mode; float2 hint_px; float pad; };
float4 ps(VSOut i) : SV_Target {
    float3 yuv;
    if (mode == 0) { yuv = float3(t0.Sample(smp, i.uv).r, t1.Sample(smp, i.uv).rg); }
    else if (mode == 1) { yuv = float3(t0.Sample(smp, i.uv).r, t1.Sample(smp, i.uv).r, t2.Sample(smp, i.uv).r); }
    else { float4 a = t0.Sample(smp, i.uv); yuv = float3(a.b, a.g, a.r); }
    float4 v = float4(yuv, 1);
    return float4(saturate(float3(dot(m0, v), dot(m1, v), dot(m2, v))), 1);
}
// Trait du bord haut : une capsule blanche de `hint_px` pixels (le tracé
// couvre exactement sa zone), bord adouci sur un pixel, opacité de 35 %.
float4 ps_hint(VSOut i) : SV_Target {
    float2 p = (i.uv - 0.5) * hint_px;
    float d = length(float2(max(abs(p.x) - (hint_px.x - hint_px.y) * 0.5, 0.0), p.y)) - hint_px.y * 0.5;
    return float4(1.0, 1.0, 1.0, saturate(0.5 - d) * 0.35);
}
"#;

/// Trait du bord haut, en points : 220 × 3, comme sur Mac (capsule blanche à
/// 35 % d'opacité, dans le shader).
const HINT_WIDTH: f32 = 220.0;
const HINT_HEIGHT: f32 = 3.0;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Params {
    m: [[f32; 4]; 3],
    mode: u32,
    /// Taille du trait du bord haut en pixels (lue par `ps_hint` seulement).
    hint_px: [f32; 2],
    pad: f32,
}

/// Disposition des plans de l'image dans nos textures.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Layout {
    /// NV12 : luminance, puis chrominance entrelacée à demi-résolution.
    Nv12,
    /// Trois plans (Y, U, V), en 4:2:0 ou 4:4:4.
    Planar { chroma_shift: u32 },
    /// AYUV 4:4:4 (carte graphique) : V, U, Y, A.
    Ayuv,
}

struct Planes {
    layout: Layout,
    width: u32,
    height: u32,
    textures: Vec<ID3D11Texture2D>,
    views: Vec<ID3D11ShaderResourceView>,
}

/// Nom de la carte graphique de l'appareil Direct3D.
pub fn adapter_name(device: &ID3D11Device) -> String {
    unsafe {
        let Ok(dxgi) = device.cast::<IDXGIDevice>() else { return String::new() };
        let Ok(adapter) = dxgi.GetAdapter() else { return String::new() };
        let Ok(desc) = adapter.GetDesc() else { return String::new() };
        let n = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
        String::from_utf16_lossy(&desc.Description[..n])
    }
}

pub fn create_device() -> windows::core::Result<(ID3D11Device, ID3D11DeviceContext)> {
    let levels = [
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_10_0,
    ];
    let attempts = [
        (D3D_DRIVER_TYPE_HARDWARE, D3D11_CREATE_DEVICE_BGRA_SUPPORT | D3D11_CREATE_DEVICE_VIDEO_SUPPORT),
        (D3D_DRIVER_TYPE_HARDWARE, D3D11_CREATE_DEVICE_BGRA_SUPPORT),
        (D3D_DRIVER_TYPE_WARP, D3D11_CREATE_DEVICE_BGRA_SUPPORT),
    ];
    let mut last = None;
    for (driver, flags) in attempts {
        let mut device = None;
        let mut context = None;
        let mut level = D3D_FEATURE_LEVEL::default();
        let r = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                flags,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut level),
                Some(&mut context),
            )
        };
        match r {
            Ok(()) => {
                let device: ID3D11Device = device.unwrap();
                // FFmpeg décode sur le même appareil depuis ses fils.
                if let Ok(mt) = device.cast::<ID3D11Multithread>() {
                    unsafe { let _ = mt.SetMultithreadProtected(true); }
                }
                log::info!("Direct3D 11 : niveau {:#x}, {:?}", level.0, driver);
                return Ok((device, context.unwrap()));
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap())
}

fn compile(entry: PCSTR, target: PCSTR) -> windows::core::Result<ID3DBlob> {
    let mut code = None;
    let mut errors = None;
    let r = unsafe {
        D3DCompile(
            SHADER.as_ptr() as *const c_void,
            SHADER.len(),
            s!("kybernow"),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    if let Err(e) = r {
        if let Some(b) = errors {
            let text = unsafe {
                std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize())
            };
            log::error!("shader : {}", String::from_utf8_lossy(text));
        }
        return Err(e);
    }
    Ok(code.unwrap())
}

/// Shader et mélange du trait du bord haut (couleur au prorata de son
/// opacité, image dessous pour le reste).
fn hint_pipeline(device: &ID3D11Device) -> windows::core::Result<(ID3D11PixelShader, ID3D11BlendState)> {
    unsafe {
        let code = compile(s!("ps_hint"), s!("ps_4_0"))?;
        let mut ps = None;
        device.CreatePixelShader(blob(&code), None, Some(&mut ps))?;
        let mut desc = D3D11_BLEND_DESC::default();
        desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: BOOL(1),
            SrcBlend: D3D11_BLEND_SRC_ALPHA,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_ZERO,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        let mut blend = None;
        device.CreateBlendState(&desc, Some(&mut blend))?;
        Ok((ps.unwrap(), blend.unwrap()))
    }
}

fn blob(b: &ID3DBlob) -> &[u8] {
    unsafe { std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()) }
}

/// Matrice YUV → RVB (lignes, constante en 4e colonne) selon l'espace et la
/// plage de couleurs du flux ; BT.709 en plage limitée par défaut.
fn matrix(colorspace: i32, full_range: bool) -> [[f32; 4]; 3] {
    let (r_cr, g_cb, g_cr, b_cb) = match colorspace {
        5 | 6 => (1.402, -0.344_136, -0.714_136, 1.772),
        9 | 10 => (1.4746, -0.164_553, -0.571_353, 1.8814),
        _ => (1.5748, -0.187_324, -0.468_124, 1.8556),
    };
    let (sy, oy, sc) = if full_range {
        (1.0, 0.0, 1.0)
    } else {
        (255.0 / 219.0, 16.0 / 255.0, 255.0 / 224.0)
    };
    let oc = 128.0 / 255.0;
    let row = |a: f32, b: f32, c: f32| -> [f32; 4] {
        [a * sy, b * sc, c * sc, -(a * sy * oy + b * sc * oc + c * sc * oc)]
    };
    [row(1.0, 0.0, r_cr), row(1.0, g_cb, g_cr), row(1.0, b_cb, 0.0)]
}

/// L'objet d'attente d'une chaîne d'échange (un handle de Windows, valable
/// de tout fil tant que la chaîne vit).
struct WaitHandle(HANDLE);

unsafe impl Send for WaitHandle {}

pub struct Renderer {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    /// Verrou du contexte partagé (`gpu.rs`) : tenu pendant une suite d'appels.
    lock: Arc<ContextLock>,
    swapchain: IDXGISwapChain1,
    /// Drapeaux de la chaîne d'échange, repris à chaque `ResizeBuffers`.
    swap_flags: DXGI_SWAP_CHAIN_FLAG,
    /// Objet d'attente de la chaîne : signalé quand une image de plus peut
    /// partir sans attendre (`None` : la chaîne n'en a pas).
    latency_wait: Option<WaitHandle>,
    rtv: Option<ID3D11RenderTargetView>,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    /// Trait du bord haut : son shader, et le mélange qui le pose sur l'image
    /// (rien si l'un des deux n'a pas pu être créé : l'image se montre sans le trait).
    hint_gpu: Option<(ID3D11PixelShader, ID3D11BlendState)>,
    /// Échelle (en centièmes) du trait en place sur le dernier tracé ; 0 : aucun.
    hint: u32,
    sampler: ID3D11SamplerState,
    cbuf: ID3D11Buffer,
    planes: Option<Planes>,
    params: Params,
    size: (u32, u32),
    tearing: bool,
    pub vsync: bool,
    present_failed: bool,
    /// Copie réduite de l'image (accueil : dernière image de la session).
    snap: Option<(ID3D11Texture2D, ID3D11RenderTargetView, u32, u32)>,
    last_snap: Option<std::time::Instant>,
}

/// Largeur de la dernière image gardée pour l'accueil.
const SNAP_WIDTH: u32 = 1280;

/// L'image ne peut pas être montrée telle quelle (format de la carte non pris
/// en charge) : il faut décoder par le processeur.
#[derive(Debug)]
pub struct Unsupported(pub String);

impl Renderer {
    pub fn new(gpu: &Gpu, hwnd: HWND, size: (u32, u32), vsync: bool) -> windows::core::Result<Self> {
        let (device, ctx) = (gpu.device.clone(), gpu.ctx.clone());
        unsafe {
            let dxgi_device: IDXGIDevice1 = device.cast()?;
            let _ = dxgi_device.SetMaximumFrameLatency(1);
            let adapter = dxgi_device.GetAdapter()?;
            let factory: IDXGIFactory2 = adapter.GetParent()?;
            let mut tearing = BOOL(0);
            if let Ok(f5) = factory.cast::<IDXGIFactory5>() {
                let _ = f5.CheckFeatureSupport(
                    DXGI_FEATURE_PRESENT_ALLOW_TEARING,
                    &mut tearing as *mut _ as *mut c_void,
                    std::mem::size_of::<BOOL>() as u32,
                );
            }
            let tearing = tearing.as_bool();
            let base_flags = if tearing { DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 } else { 0 };
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size.0.max(1),
                Height: size.1.max(1),
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_NONE,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: base_flags as u32,
                ..Default::default()
            };
            // Avec son objet d'attente d'abord ; sans lui si la carte le refuse.
            let waitable = base_flags | DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0;
            let (swapchain, swap_flags, latency_wait) =
                match factory.CreateSwapChainForHwnd(&device, hwnd, &DXGI_SWAP_CHAIN_DESC1 { Flags: waitable as u32, ..desc }, None, None) {
                    Ok(sc) => {
                        let wait = sc.cast::<IDXGISwapChain2>().ok().and_then(|sc2| {
                            sc2.SetMaximumFrameLatency(1).ok()?;
                            let h = sc2.GetFrameLatencyWaitableObject();
                            (!h.is_invalid()).then_some(WaitHandle(h))
                        });
                        (sc, waitable, wait)
                    }
                    Err(e) => {
                        log::warn!("chaîne d'échange sans objet d'attente : {e}");
                        (factory.CreateSwapChainForHwnd(&device, hwnd, &desc, None, None)?, base_flags, None)
                    }
                };
            let _ = factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER);

            let vsb = compile(s!("vs"), s!("vs_4_0"))?;
            let psb = compile(s!("ps"), s!("ps_4_0"))?;
            let mut vs = None;
            device.CreateVertexShader(blob(&vsb), None, Some(&mut vs))?;
            let mut ps = None;
            device.CreatePixelShader(blob(&psb), None, Some(&mut ps))?;
            let hint_gpu = hint_pipeline(&device)
                .inspect_err(|e| log::warn!("trait du bord haut : {e}"))
                .ok();
            let sdesc = D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ..Default::default()
            };
            let mut sampler = None;
            device.CreateSamplerState(&sdesc, Some(&mut sampler))?;
            let bdesc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<Params>() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let mut cbuf = None;
            device.CreateBuffer(&bdesc, None, Some(&mut cbuf))?;
            log::info!(
                "chaîne d'échange : déchirure {}, objet d'attente {}",
                if tearing { "permise" } else { "non prise en charge" },
                if latency_wait.is_some() { "oui" } else { "non" }
            );
            let mut r = Self {
                device,
                ctx,
                lock: gpu.lock.clone(),
                swapchain,
                swap_flags: DXGI_SWAP_CHAIN_FLAG(swap_flags),
                latency_wait,
                rtv: None,
                vs: vs.unwrap(),
                ps: ps.unwrap(),
                hint_gpu,
                hint: 0,
                sampler: sampler.unwrap(),
                cbuf: cbuf.unwrap(),
                planes: None,
                params: Params::default(),
                size,
                tearing,
                vsync,
                present_failed: false,
                snap: None,
                last_snap: None,
            };
            r.create_rtv()?;
            Ok(r)
        }
    }

    fn create_rtv(&mut self) -> windows::core::Result<()> {
        unsafe {
            let back: ID3D11Texture2D = self.swapchain.GetBuffer(0)?;
            let mut rtv = None;
            self.device.CreateRenderTargetView(&back, None, Some(&mut rtv))?;
            self.rtv = rtv;
        }
        Ok(())
    }

    /// Nouvelle taille de la fenêtre (pixels).
    pub fn resize(&mut self, size: (u32, u32)) {
        if size == self.size || size.0 == 0 || size.1 == 0 {
            return;
        }
        // Toutes les vues de la chaîne d'échange relâchées avant
        // ResizeBuffers ; la taille n'est retenue qu'en cas de réussite (sinon
        // on réessaie à l'image suivante).
        self.rtv = None;
        let lock = self.lock.clone();
        let _held = lock.lock();
        unsafe {
            self.ctx.ClearState();
            self.ctx.Flush();
            match self.swapchain.ResizeBuffers(0, size.0, size.1, DXGI_FORMAT_UNKNOWN, self.swap_flags) {
                Ok(()) => self.size = size,
                Err(e) => log::warn!("ResizeBuffers : {e}"),
            }
        }
        if let Err(e) = self.create_rtv() {
            log::warn!("vue de la chaîne d'échange : {e}");
        }
    }

    fn texture(&self, w: u32, h: u32, format: DXGI_FORMAT) -> windows::core::Result<ID3D11Texture2D> {
        self.texture_with(w, h, format, D3D11_BIND_SHADER_RESOURCE.0 as u32)
    }

    fn texture_with(&self, w: u32, h: u32, format: DXGI_FORMAT, bind: u32) -> windows::core::Result<ID3D11Texture2D> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: w,
            Height: h,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: bind,
            ..Default::default()
        };
        let mut t = None;
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut t))? };
        Ok(t.unwrap())
    }

    fn view(&self, t: &ID3D11Texture2D, format: DXGI_FORMAT) -> windows::core::Result<ID3D11ShaderResourceView> {
        let desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: format,
            ViewDimension: D3D_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1 },
            },
        };
        let mut v = None;
        unsafe { self.device.CreateShaderResourceView(t, Some(&desc), Some(&mut v))? };
        Ok(v.unwrap())
    }

    fn ensure_planes(&mut self, layout: Layout, w: u32, h: u32) -> windows::core::Result<()> {
        if let Some(p) = &self.planes {
            if p.layout == layout && p.width == w && p.height == h {
                return Ok(());
            }
        }
        let (textures, views) = match layout {
            Layout::Nv12 => {
                let t = self.texture(w, h, DXGI_FORMAT_NV12)?;
                let y = self.view(&t, DXGI_FORMAT_R8_UNORM)?;
                let uv = self.view(&t, DXGI_FORMAT_R8G8_UNORM)?;
                (vec![t], vec![y, uv])
            }
            Layout::Ayuv => {
                let t = self.texture(w, h, DXGI_FORMAT_AYUV)?;
                let v = self.view(&t, DXGI_FORMAT_R8G8B8A8_UNORM)?;
                (vec![t], vec![v])
            }
            Layout::Planar { chroma_shift } => {
                let (cw, ch) = ((w + chroma_shift) >> chroma_shift, (h + chroma_shift) >> chroma_shift);
                let mut ts = Vec::new();
                let mut vs = Vec::new();
                for (pw, ph) in [(w, h), (cw, ch), (cw, ch)] {
                    let t = self.texture(pw, ph, DXGI_FORMAT_R8_UNORM)?;
                    vs.push(self.view(&t, DXGI_FORMAT_R8_UNORM)?);
                    ts.push(t);
                }
                (ts, vs)
            }
        };
        log::info!("textures de l'image : {layout:?} {w}×{h}");
        self.planes = Some(Planes { layout, width: w, height: h, textures, views });
        Ok(())
    }

    /// Copie l'image décodée dans nos textures.
    fn upload(&mut self, f: &AVFrame) -> Result<(), Unsupported> {
        let (w, h) = (f.width as u32, f.height as u32);
        let fail = |e: windows::core::Error| Unsupported(e.to_string());
        if is_hw_frame(f) {
            let src = f.data[0] as *mut c_void;
            let index = f.data[1] as usize as u32;
            let src = unsafe { ID3D11Texture2D::from_raw_borrowed(&src) }
                .ok_or_else(|| Unsupported("texture absente".into()))?;
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            unsafe { src.GetDesc(&mut desc) };
            let layout = match desc.Format {
                DXGI_FORMAT_NV12 => Layout::Nv12,
                DXGI_FORMAT_AYUV => Layout::Ayuv,
                other => return Err(Unsupported(format!("format de la carte {other:?}"))),
            };
            // Largeur et hauteur paires pour le NV12.
            let (w, h) = if layout == Layout::Nv12 { (w & !1, h & !1) } else { (w, h) };
            self.ensure_planes(layout, w, h).map_err(fail)?;
            let dst = &self.planes.as_ref().unwrap().textures[0];
            let bx = D3D11_BOX { left: 0, top: 0, front: 0, right: w, bottom: h, back: 1 };
            unsafe { self.ctx.CopySubresourceRegion(dst, 0, 0, 0, 0, src, index, Some(&bx)) };
            return Ok(());
        }
        // Décodage par le processeur : trois plans.
        let layout = match f.format {
            AV_PIX_FMT_YUV420P | AV_PIX_FMT_YUVJ420P => Layout::Planar { chroma_shift: 1 },
            AV_PIX_FMT_YUV444P | AV_PIX_FMT_YUVJ444P => Layout::Planar { chroma_shift: 0 },
            other => return Err(Unsupported(format!("format d'image {other}"))),
        };
        self.ensure_planes(layout, w, h).map_err(fail)?;
        let planes = self.planes.as_ref().unwrap();
        for i in 0..3 {
            unsafe {
                self.ctx.UpdateSubresource(&planes.textures[i], 0, None, f.data[i] as *const c_void, f.linesize[i] as u32, 0);
            }
        }
        Ok(())
    }

    /// Montre une image décodée et la présente aussitôt. `hint` : échelle (en
    /// centièmes) du trait du bord haut posé par-dessus ; 0 : aucun.
    pub fn draw(&mut self, f: &AVFrame, hint: u32) -> Result<(), Unsupported> {
        // L'attente de la chaîne avant de prendre le verrou : un autre écran
        // continue de tracer pendant qu'on attend la synchro de celui-ci.
        self.wait_ready();
        let lock = self.lock.clone();
        let (fw, fh, painted) = {
            let _held = lock.lock();
            self.upload(f)?;
            let planes = self.planes.as_ref().unwrap();
            self.params.mode = match planes.layout {
                Layout::Nv12 => 0,
                Layout::Planar { .. } => 1,
                Layout::Ayuv => 2,
            };
            self.params.m = matrix(f.colorspace as i32, f.color_range as i32 == 2);
            let (fw, fh) = (planes.width, planes.height);
            (fw, fh, self.encode(hint))
        };
        if painted {
            self.present();
            // Après la présentation : rien ne retarde l'image à l'écran.
            let _held = lock.lock();
            self.snapshot(fw, fh);
        }
        Ok(())
    }

    /// Le trait du bord haut a changé alors qu'aucune image ne vient (écran
    /// fixe) : l'image en place est redessinée, avec ou sans le trait.
    pub fn repaint(&mut self, hint: u32) {
        if hint != self.hint && self.planes.is_some() {
            self.wait_ready();
            let lock = self.lock.clone();
            let painted = {
                let _held = lock.lock();
                self.encode(hint)
            };
            if painted {
                self.present();
            }
        }
    }

    /// Synchro verticale : une image de plus peut partir sans que `Present`
    /// attende (jusqu'à 100 ms, sinon on présente quand même : fenêtre
    /// cachée ou couverte, qui ne consomme plus).
    fn wait_ready(&self) {
        if let (true, Some(h)) = (self.vsync, self.latency_wait.as_ref()) {
            unsafe { WaitForSingleObjectEx(h.0, 100, false) };
        }
    }

    /// Dessine l'image en place (textures de la dernière image) à la taille de
    /// la fenêtre, le trait du bord haut par-dessus (faux : rien à dessiner ou
    /// sur quoi dessiner). Appelé sous le verrou du contexte ; `present` suit,
    /// dehors.
    fn encode(&mut self, hint: u32) -> bool {
        let Some(planes) = self.planes.as_ref() else { return false };
        let (fw, fh) = (planes.width as f32, planes.height as f32);
        let (sw, sh) = (self.size.0 as f32, self.size.1 as f32);
        let scale = (sw / fw).min(sh / fh);
        let (vw, vh) = (fw * scale, fh * scale);
        let viewport = D3D11_VIEWPORT {
            TopLeftX: ((sw - vw) / 2.0).round(),
            TopLeftY: ((sh - vh) / 2.0).round(),
            Width: vw.round(),
            Height: vh.round(),
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        unsafe {
            let Some(rtv) = self.rtv.clone() else { return false };
            self.ctx.UpdateSubresource(&self.cbuf, 0, None, &self.params as *const _ as *const c_void, 0, 0);
            self.ctx.ClearRenderTargetView(&rtv, &[0.0, 0.0, 0.0, 1.0]);
            self.ctx.OMSetRenderTargets(Some(&[Some(rtv)]), None);
            self.ctx.RSSetViewports(Some(&[viewport]));
            self.ctx.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            self.ctx.VSSetShader(&self.vs, None);
            self.ctx.PSSetShader(&self.ps, None);
            let views: Vec<Option<ID3D11ShaderResourceView>> = planes.views.iter().cloned().map(Some).collect();
            self.ctx.PSSetShaderResources(0, Some(&views));
            self.ctx.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            self.ctx.PSSetConstantBuffers(0, Some(&[Some(self.cbuf.clone())]));
            self.ctx.Draw(3, 0);
            if let (true, Some((hint_ps, hint_blend))) = (hint > 0, &self.hint_gpu) {
                self.draw_hint(hint_ps, hint_blend, hint, sw);
            }
            self.hint = hint;
        }
        true
    }

    /// Présente l'image tracée par `encode` (hors du verrou du contexte).
    fn present(&mut self) {
        let (interval, flags) = if self.vsync {
            (1, DXGI_PRESENT(0))
        } else if self.tearing {
            (0, DXGI_PRESENT_ALLOW_TEARING)
        } else {
            (0, DXGI_PRESENT(0))
        };
        let hr = unsafe { self.swapchain.Present(interval, flags) };
        if hr.is_err() && !self.present_failed {
            // Carte graphique perdue (pilote mis à jour, mise en veille…).
            log::error!("Present : {hr:?}");
            self.present_failed = true;
        }
    }

    /// Le trait du bord haut : capsule de 220 × 3 points (`hint` : échelle de
    /// l'écran en centièmes), centrée en haut de la fenêtre, posée sur l'image
    /// déjà tracée. L'état du contexte est remis comme l'image le laisse (le
    /// tracé de l'aperçu, qui suit, en part).
    unsafe fn draw_hint(&self, hint_ps: &ID3D11PixelShader, hint_blend: &ID3D11BlendState, hint: u32, width: f32) {
        let k = hint as f32 / 100.0;
        let (w, h) = ((HINT_WIDTH * k).round().max(1.0), (HINT_HEIGHT * k).round().max(1.0));
        let viewport = D3D11_VIEWPORT {
            TopLeftX: ((width - w) / 2.0).round(),
            TopLeftY: 0.0,
            Width: w,
            Height: h,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let params = Params { hint_px: [w, h], ..self.params };
        unsafe {
            self.ctx.UpdateSubresource(&self.cbuf, 0, None, &params as *const _ as *const c_void, 0, 0);
            self.ctx.OMSetBlendState(hint_blend, None, 0xFFFF_FFFF);
            self.ctx.PSSetShader(hint_ps, None);
            self.ctx.RSSetViewports(Some(&[viewport]));
            self.ctx.Draw(3, 0);
            self.ctx.PSSetShader(&self.ps, None);
            self.ctx.OMSetBlendState(None, None, 0xFFFF_FFFF);
        }
    }

    /// Toutes les 2 s, l'image redessinée en petit dans une texture à part
    /// (un tracé de plus, sans attente) : la dernière servira à l'accueil.
    fn snapshot(&mut self, fw: u32, fh: u32) {
        if self.last_snap.is_some_and(|t| t.elapsed().as_secs() < 2) || fw == 0 || fh == 0 {
            return;
        }
        self.last_snap = Some(std::time::Instant::now());
        let w = SNAP_WIDTH.min(fw);
        let h = ((fh as u64 * w as u64 / fw as u64) as u32 / 2 * 2).max(2);
        if self.snap.as_ref().is_none_or(|s| s.2 != w || s.3 != h) {
            let made = (|| unsafe {
                let t = self.texture_with(w, h, DXGI_FORMAT_B8G8R8A8_UNORM, D3D11_BIND_RENDER_TARGET.0 as u32)?;
                let mut rtv = None;
                self.device.CreateRenderTargetView(&t, None, Some(&mut rtv))?;
                Ok::<_, windows::core::Error>((t, rtv.unwrap()))
            })();
            match made {
                Ok((t, rtv)) => self.snap = Some((t, rtv, w, h)),
                Err(_) => return,
            }
        }
        let Some((_, rtv, w, h)) = &self.snap else { return };
        let viewport = D3D11_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: *w as f32, Height: *h as f32, MinDepth: 0.0, MaxDepth: 1.0 };
        unsafe {
            self.ctx.OMSetRenderTargets(Some(&[Some(rtv.clone())]), None);
            self.ctx.RSSetViewports(Some(&[viewport]));
            self.ctx.Draw(3, 0);
        }
    }

    /// Dernière image gardée, en BGRA (lue une fois, à la fin de la session).
    pub fn take_snapshot(&mut self) -> Option<(Vec<u8>, u32, u32)> {
        let (tex, _, w, h) = self.snap.take()?;
        let lock = self.lock.clone();
        let _held = lock.lock();
        unsafe {
            let desc = D3D11_TEXTURE2D_DESC {
                Width: w,
                Height: h,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                ..Default::default()
            };
            let mut staging = None;
            self.device.CreateTexture2D(&desc, None, Some(&mut staging)).ok()?;
            let staging = staging?;
            self.ctx.CopyResource(&staging, &tex);
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut map)).ok()?;
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                let row = std::slice::from_raw_parts((map.pData as *const u8).add(y * map.RowPitch as usize), w as usize * 4);
                out.extend_from_slice(row);
            }
            self.ctx.Unmap(&staging, 0);
            Some((out, w, h))
        }
    }

    /// Fond noir (avant la première image).
    pub fn clear(&mut self) {
        let cleared = {
            let lock = self.lock.clone();
            let _held = lock.lock();
            match &self.rtv {
                Some(rtv) => {
                    unsafe { self.ctx.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0]) };
                    true
                }
                None => false,
            }
        };
        if cleared {
            let _ = unsafe { self.swapchain.Present(0, DXGI_PRESENT(0)) };
        }
    }
}

/// Rectangle occupé par l'image dans la fenêtre (mêmes proportions que le
/// rendu), pour ramener la souris aux pixels de l'ordinateur.
pub fn video_rect(client: (u32, u32), host: (u32, u32)) -> RECT {
    let (cw, ch) = (client.0 as f32, client.1 as f32);
    let (hw, hh) = (host.0.max(1) as f32, host.1.max(1) as f32);
    let scale = (cw / hw).min(ch / hh);
    let (vw, vh) = ((hw * scale).round(), (hh * scale).round());
    let left = ((cw - vw) / 2.0).round() as i32;
    let top = ((ch - vh) / 2.0).round() as i32;
    RECT { left, top, right: left + vw as i32, bottom: top + vh as i32 }
}
