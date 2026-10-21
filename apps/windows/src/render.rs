//! Rendu Direct3D 11 : l'image décodée (sur la carte graphique ou venue du
//! processeur) est convertie en RVB par un petit shader, à la taille de la
//! fenêtre en gardant ses proportions, puis présentée aussitôt (chaîne
//! d'échange « flip », déchirure permise sans synchro verticale).

use std::ffi::c_void;

use windows::Win32::Foundation::{HMODULE, HWND, RECT};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_0,
    D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
    D3D_SRV_DIMENSION_TEXTURE2D, ID3DBlob,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{BOOL, Interface, PCSTR, s};

use crate::decoder::ff::*;
use crate::decoder::is_hw_frame;

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
cbuffer Params : register(b0) { float4 m0; float4 m1; float4 m2; uint mode; uint3 pad; };
float4 ps(VSOut i) : SV_Target {
    float3 yuv;
    if (mode == 0) { yuv = float3(t0.Sample(smp, i.uv).r, t1.Sample(smp, i.uv).rg); }
    else if (mode == 1) { yuv = float3(t0.Sample(smp, i.uv).r, t1.Sample(smp, i.uv).r, t2.Sample(smp, i.uv).r); }
    else { float4 a = t0.Sample(smp, i.uv); yuv = float3(a.b, a.g, a.r); }
    float4 v = float4(yuv, 1);
    return float4(saturate(float3(dot(m0, v), dot(m1, v), dot(m2, v))), 1);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Params {
    m: [[f32; 4]; 3],
    mode: u32,
    pad: [u32; 3],
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

pub struct Renderer {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    swapchain: IDXGISwapChain1,
    rtv: Option<ID3D11RenderTargetView>,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
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
    pub fn new(
        device: ID3D11Device,
        ctx: ID3D11DeviceContext,
        hwnd: HWND,
        size: (u32, u32),
        vsync: bool,
    ) -> windows::core::Result<Self> {
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
                Flags: if tearing { DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING.0 as u32 } else { 0 },
                ..Default::default()
            };
            let swapchain = factory.CreateSwapChainForHwnd(&device, hwnd, &desc, None, None)?;
            let _ = factory.MakeWindowAssociation(hwnd, DXGI_MWA_NO_ALT_ENTER);

            let vsb = compile(s!("vs"), s!("vs_4_0"))?;
            let psb = compile(s!("ps"), s!("ps_4_0"))?;
            let mut vs = None;
            device.CreateVertexShader(blob(&vsb), None, Some(&mut vs))?;
            let mut ps = None;
            device.CreatePixelShader(blob(&psb), None, Some(&mut ps))?;
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
            log::info!("chaîne d'échange : déchirure {}", if tearing { "permise" } else { "non prise en charge" });
            let mut r = Self {
                device,
                ctx,
                swapchain,
                rtv: None,
                vs: vs.unwrap(),
                ps: ps.unwrap(),
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
        unsafe {
            self.ctx.ClearState();
            self.ctx.Flush();
            let flags = if self.tearing { DXGI_SWAP_CHAIN_FLAG_ALLOW_TEARING } else { DXGI_SWAP_CHAIN_FLAG(0) };
            match self.swapchain.ResizeBuffers(0, size.0, size.1, DXGI_FORMAT_UNKNOWN, flags) {
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

    /// Montre une image décodée et la présente aussitôt.
    pub fn draw(&mut self, f: &AVFrame) -> Result<(), Unsupported> {
        self.upload(f)?;
        let planes = self.planes.as_ref().unwrap();
        self.params.mode = match planes.layout {
            Layout::Nv12 => 0,
            Layout::Planar { .. } => 1,
            Layout::Ayuv => 2,
        };
        self.params.m = matrix(f.colorspace as i32, f.color_range as i32 == 2);
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
            let Some(rtv) = self.rtv.clone() else { return Ok(()) };
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
            let (interval, flags) = if self.vsync {
                (1, DXGI_PRESENT(0))
            } else if self.tearing {
                (0, DXGI_PRESENT_ALLOW_TEARING)
            } else {
                (0, DXGI_PRESENT(0))
            };
            let hr = self.swapchain.Present(interval, flags);
            if hr.is_err() && !self.present_failed {
                // Carte graphique perdue (pilote mis à jour, mise en veille…).
                log::error!("Present : {hr:?}");
                self.present_failed = true;
            }
        }
        // Après la présentation : rien ne retarde l'image à l'écran.
        self.snapshot(fw as u32, fh as u32);
        Ok(())
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
        unsafe {
            if let Some(rtv) = &self.rtv {
                self.ctx.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0]);
                let _ = self.swapchain.Present(0, DXGI_PRESENT(0));
            }
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
