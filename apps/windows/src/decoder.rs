//! Décodage avec FFmpeg : par la carte graphique (D3D11VA, sans copie hors du
//! GPU) quand elle sait, sinon par le processeur. FFmpeg retombe seul sur le
//! processeur si la carte refuse un profil (HEVC 4:4:4 sur une carte qui ne
//! le décode pas, par exemple).

#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code)]

use std::ffi::c_void;
use std::ptr;

use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11VideoContext, ID3D11VideoDevice};
use windows::core::Interface;

use kybernow_core::media::{FOURCC_AV1, FOURCC_H264, FOURCC_HEVC};
use kybernow_core::profile::Capabilities;

pub mod ff {
    #![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code, unnecessary_transmutes, clippy::all)]
    include!(concat!(env!("OUT_DIR"), "/ffmpeg.rs"));
}

use ff::*;

pub const EAGAIN: i32 = -11;
pub const AVERROR_EOF: i32 = -(((b'E' as i32) | ((b'O' as i32) << 8) | ((b'F' as i32) << 16) | ((b' ' as i32) << 24)));

/// `AVD3D11VADeviceContext` (libavutil/hwcontext_d3d11va.h) : des pointeurs,
/// stable d'une version de FFmpeg à l'autre.
#[repr(C)]
struct AVD3D11VADeviceContext {
    device: *mut c_void,
    device_context: *mut c_void,
    video_device: *mut c_void,
    video_context: *mut c_void,
    lock: Option<unsafe extern "C" fn(*mut c_void)>,
    unlock: Option<unsafe extern "C" fn(*mut c_void)>,
    lock_ctx: *mut c_void,
}

pub fn error_text(code: i32) -> String {
    let mut buf = [0i8; 128];
    unsafe { av_strerror(code, buf.as_mut_ptr(), buf.len()) };
    let s = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    format!("{} ({code})", s.to_string_lossy())
}

pub fn codec_name(fourcc: u32) -> &'static str {
    match fourcc {
        FOURCC_H264 => "H.264",
        FOURCC_HEVC => "HEVC",
        FOURCC_AV1 => "AV1",
        _ => "?",
    }
}

unsafe extern "C" fn get_format_hw(_ctx: *mut AVCodecContext, fmts: *const AVPixelFormat) -> AVPixelFormat {
    let mut p = fmts;
    let mut first = AV_PIX_FMT_NONE;
    unsafe {
        while *p != AV_PIX_FMT_NONE {
            if *p == AV_PIX_FMT_D3D11 {
                return AV_PIX_FMT_D3D11;
            }
            if first == AV_PIX_FMT_NONE {
                first = *p;
            }
            p = p.add(1);
        }
    }
    first
}

pub struct Decoder {
    ctx: *mut AVCodecContext,
    pkt: *mut AVPacket,
    frame: *mut AVFrame,
    pub fourcc: u32,
    pub hardware: bool,
}

unsafe impl Send for Decoder {}

impl Decoder {
    /// `device` : la carte graphique du rendu, pour un décodage sans copie.
    pub fn new(fourcc: u32, device: Option<&ID3D11Device>) -> Result<Self, String> {
        let id = match fourcc {
            FOURCC_H264 => AV_CODEC_ID_H264,
            FOURCC_HEVC => AV_CODEC_ID_HEVC,
            FOURCC_AV1 => AV_CODEC_ID_AV1,
            other => return Err(format!("format vidéo inconnu {other:#x}")),
        };
        unsafe {
            let codec = avcodec_find_decoder(id);
            if codec.is_null() {
                return Err(format!("pas de décodeur {}", codec_name(fourcc)));
            }
            let ctx = avcodec_alloc_context3(codec);
            if ctx.is_null() {
                return Err("avcodec_alloc_context3".into());
            }
            (*ctx).flags |= AV_CODEC_FLAG_LOW_DELAY as i32;
            let mut hardware = false;
            // Interfaces vidéo de l'appareil : sans elles (machine virtuelle,
            // pilote de base), pas de décodage par la carte.
            let video = device.and_then(|dev| {
                let vdev = dev.cast::<ID3D11VideoDevice>().ok()?;
                let vctx = dev.GetImmediateContext().ok()?.cast::<ID3D11VideoContext>().ok()?;
                Some((dev, vdev, vctx))
            });
            if device.is_some() && video.is_none() {
                log::info!("décodeur : pas d'interface vidéo Direct3D 11, décodage par le processeur");
            }
            if let Some((dev, vdev, vctx)) = video {
                let hw = av_hwdevice_ctx_alloc(AV_HWDEVICE_TYPE_D3D11VA);
                if !hw.is_null() {
                    let devctx = (*hw).data as *mut AVHWDeviceContext;
                    let d3d = (*devctx).hwctx as *mut AVD3D11VADeviceContext;
                    // FFmpeg 8.1 ne cherche pas lui-même les interfaces vidéo d'un
                    // appareil fourni (d3d11va_device_init) : sans elles, il plante
                    // à la création du décodeur. Il relâche tout à la fin : on lui
                    // en donne des références.
                    (*d3d).device = dev.clone().into_raw();
                    (*d3d).video_device = vdev.into_raw();
                    (*d3d).video_context = vctx.into_raw();
                    let mut hw = hw;
                    if av_hwdevice_ctx_init(hw) == 0 {
                        (*ctx).hw_device_ctx = hw;
                        (*ctx).get_format = Some(get_format_hw);
                        (*ctx).thread_count = 1;
                        hardware = true;
                    } else {
                        av_buffer_unref(&mut hw);
                    }
                }
            }
            if !hardware {
                // Fils par tranches seulement : les fils par image ajoutent
                // une image de retard chacun.
                (*ctx).thread_type = FF_THREAD_SLICE as i32;
                (*ctx).thread_count = 0;
            }
            let r = avcodec_open2(ctx, codec, ptr::null_mut());
            if r < 0 {
                let mut ctx = ctx;
                avcodec_free_context(&mut ctx);
                return Err(format!("ouverture du décodeur : {}", error_text(r)));
            }
            Ok(Self {
                ctx,
                pkt: av_packet_alloc(),
                frame: av_frame_alloc(),
                fourcc,
                hardware,
            })
        }
    }

    pub fn send(&mut self, data: &[u8], pts: u64, key: bool) -> Result<(), i32> {
        unsafe {
            let r = av_new_packet(self.pkt, data.len() as i32);
            if r < 0 {
                return Err(r);
            }
            ptr::copy_nonoverlapping(data.as_ptr(), (*self.pkt).data, data.len());
            (*self.pkt).pts = pts as i64;
            if key {
                (*self.pkt).flags |= AV_PKT_FLAG_KEY as i32;
            }
            let r = avcodec_send_packet(self.ctx, self.pkt);
            av_packet_unref(self.pkt);
            if r < 0 && r != EAGAIN { Err(r) } else { Ok(()) }
        }
    }

    /// Plus d'images à venir (sonde).
    pub fn flush(&mut self) {
        unsafe { avcodec_send_packet(self.ctx, ptr::null()) };
    }

    /// Image suivante, `None` s'il faut d'autres paquets.
    pub fn receive(&mut self) -> Option<Result<&AVFrame, i32>> {
        unsafe {
            av_frame_unref(self.frame);
            let r = avcodec_receive_frame(self.ctx, self.frame);
            if r == EAGAIN || r == AVERROR_EOF {
                None
            } else if r < 0 {
                Some(Err(r))
            } else {
                Some(Ok(&*self.frame))
            }
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            av_frame_free(&mut self.frame);
            av_packet_free(&mut self.pkt);
            avcodec_free_context(&mut self.ctx);
        }
    }
}

/// L'image est-elle restée sur la carte graphique ?
pub fn is_hw_frame(f: &AVFrame) -> bool {
    f.format == AV_PIX_FMT_D3D11
}

/// L'image dépend d'une image perdue (à redemander).
pub fn is_corrupt(f: &AVFrame) -> bool {
    (f.flags & AV_FRAME_FLAG_CORRUPT as i32) != 0 || f.decode_error_flags != 0
}

const PROBE_H264: &[u8] = include_bytes!("probe/h264.bin");
const PROBE_HEVC: &[u8] = include_bytes!("probe/hevc.bin");
const PROBE_HEVC444: &[u8] = include_bytes!("probe/hevc444.bin");
const PROBE_AV1: &[u8] = include_bytes!("probe/av1.bin");

/// Décode pour de bon une image de chaque format et dit lesquels la carte
/// graphique prend en charge (comme l'app Mac au lancement).
fn probe_one(fourcc: u32, data: &[u8], device: &ID3D11Device) -> bool {
    let Ok(mut dec) = Decoder::new(fourcc, Some(device)) else { return false };
    if !dec.hardware || dec.send(data, 0, true).is_err() {
        return false;
    }
    dec.flush();
    while let Some(r) = dec.receive() {
        match r {
            Ok(f) => return is_hw_frame(f),
            Err(_) => return false,
        }
    }
    false
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Probe {
    pub h264: bool,
    pub caps: Capabilities,
}

pub fn probe(device: &ID3D11Device) -> Probe {
    unsafe { av_log_set_level(AV_LOG_QUIET as i32) };
    let p = Probe {
        h264: probe_one(FOURCC_H264, PROBE_H264, device),
        caps: Capabilities {
            hevc: probe_one(FOURCC_HEVC, PROBE_HEVC, device),
            hevc444: probe_one(FOURCC_HEVC, PROBE_HEVC444, device),
            av1: probe_one(FOURCC_AV1, PROBE_AV1, device),
        },
    };
    unsafe { av_log_set_level(AV_LOG_ERROR as i32) };
    p
}
