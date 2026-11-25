//! Essai hors ligne de PyroWave (`kybernow --pyrowave-test`,
//! docs/pyrowave-client-windows.md) : sans ordinateur ni réseau, une mire
//! animée est encodée par l'encodeur de l'amont (celui de la DLL, sur le même
//! device Vulkan), une image = paquets de 1 152 octets bout à bout comme un
//! paquet média de la Station, puis décodée par le décodeur de l'app
//! (`pyrowave::Decoder`, textures Direct3D 11 partagées, fence) et copiée
//! comme le fait le rendu. Mesures : temps de décodage (GPU terminé) cadencé
//! à 60 i/s, débit, PSNR de la première image contre la source relue sur la
//! carte, image tronquée sautée sans casser la suivante ; et la sonde du
//! lancement.

use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::pyrowave::{self, Config, Decoder, Device, Outcome, PYROWAVE_SUCCESS, pw_cpu_buffer, pw_encoder_create_info, pw_packet};

const USAGE: &str = "\
Essai hors ligne de PyroWave : encode une mire avec l'encodeur de PyroWave,
la décode avec le décodeur de KyberNow (textures Direct3D 11 partagées).

  kybernow --pyrowave-test [--size <L>x<H>]… [--420|--444] [--sdr|--hdr]
                           [--frames <n>] [--bpp <bits par pixel>]

Sans option : 1920×1080, 2560×1440 et 3840×2160, en 4:2:0 et 4:4:4, SDR et
HDR10, 120 images à 1,2 bit par pixel (le débit d'une session PyroWave).
";

/// Taille d'un paquet PyroWave (celle de la Station, docs/pyrowave.md).
const PACKET: usize = 1152;

struct Case {
    width: u32,
    height: u32,
    chroma444: bool,
    hdr: bool,
}

pub fn run(args: &[String]) -> i32 {
    let mut sizes = Vec::new();
    let (mut chromas, mut hdrs) = (vec![false, true], vec![false, true]);
    let (mut frames, mut bpp) = (120usize, 1.2f64);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--size" => match it.next().and_then(|v| v.split_once('x')).and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))) {
                Some(s) => sizes.push(s),
                None => {
                    eprintln!("--size <L>x<H>, par exemple 1920x1080");
                    return 2;
                }
            },
            "--420" => chromas = vec![false],
            "--444" => chromas = vec![true],
            "--sdr" => hdrs = vec![false],
            "--hdr" => hdrs = vec![true],
            "--frames" => frames = it.next().and_then(|v| v.parse().ok()).unwrap_or(frames).max(2),
            "--bpp" => bpp = it.next().and_then(|v| v.parse().ok()).unwrap_or(bpp),
            "--help" | "-h" => {
                print!("{USAGE}");
                return 0;
            }
            other => {
                eprintln!("option inconnue : {other}\n\n{USAGE}");
                return 2;
            }
        }
    }
    if sizes.is_empty() {
        sizes = vec![(1920, 1080), (2560, 1440), (3840, 2160)];
    }
    let (d3d, _ctx) = match crate::render::create_device() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Direct3D 11 : {e}");
            return 1;
        }
    };
    println!("Carte graphique : {}", crate::render::adapter_name(&d3d));
    let started = Instant::now();
    match pyrowave::probe(&d3d) {
        Ok(()) => println!("Sonde du lancement : PyroWave décodé ({} ms)", started.elapsed().as_millis()),
        Err(e) => {
            println!("Sonde du lancement : PyroWave indisponible ({e})");
            return 1;
        }
    }
    let device = match Device::new(&d3d) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("PyroWave : {e}");
            return 1;
        }
    };
    if device.fragment {
        println!("Chemin par fragments (recommandé par PyroWave pour cette carte)");
    }
    let mut failed = false;
    for &(width, height) in &sizes {
        for &chroma444 in &chromas {
            for &hdr in &hdrs {
                let case = Case { width, height, chroma444, hdr };
                if let Err(e) = run_case(&d3d, &device, &case, frames, bpp) {
                    println!("{} : ÉCHEC, {e}", label(&case));
                    failed = true;
                }
            }
        }
    }
    if failed { 1 } else { 0 }
}

fn label(c: &Case) -> String {
    format!("{}×{} {} {}", c.width, c.height, if c.chroma444 { "4:4:4" } else { "4:2:0" }, if c.hdr { "HDR10" } else { "SDR " })
}

/// La mire de l'image `t` : rampe qui défile, damier fin (texte), bandes de
/// couleur ; codes 8 bits (SDR) ou 10 bits (HDR), plage réduite.
fn source(c: &Case, t: usize) -> [Vec<u16>; 3] {
    let (w, h) = (c.width as usize, c.height as usize);
    let (cw, ch) = if c.chroma444 { (w, h) } else { (w / 2, h / 2) };
    let shift = if c.hdr { 2 } else { 0 };
    let mut y = vec![0u16; w * h];
    for j in 0..h {
        for i in 0..w {
            let ramp = ((i + 4 * t) % w) as f64 / w as f64;
            let mut v = 16.0 + 219.0 * (0.25 + 0.5 * ramp);
            // Damier de 2 pixels dans un quart de l'image : le détail du texte.
            if i < w / 2 && j < h / 2 && ((i / 2 + j / 2) % 2 == 0) {
                v = 235.0 - v / 4.0;
            }
            y[j * w + i] = (v.round() as u16) << shift;
        }
    }
    let mut cb = vec![0u16; cw * ch];
    let mut cr = vec![0u16; cw * ch];
    for j in 0..ch {
        for i in 0..cw {
            let a = (i as f64 / cw as f64 + t as f64 / 240.0) * std::f64::consts::TAU;
            cb[j * cw + i] = ((128.0 + 96.0 * a.sin()).round() as u16) << shift;
            cr[j * cw + i] = ((128.0 + 96.0 * (a + (j as f64 / ch as f64) * 2.0).cos()).round() as u16) << shift;
        }
    }
    [y, cb, cr]
}

/// Encode une image avec l'encodeur de l'amont : les paquets bout à bout.
fn encode(enc: *mut c_void, c: &Case, planes: &mut [Vec<u16>; 3], max_bytes: usize) -> Result<Vec<u8>, String> {
    let api = pyrowave::api().ok_or("DLL")?;
    let (cw, ch) = if c.chroma444 { (c.width, c.height) } else { (c.width / 2, c.height / 2) };
    // 8 bits : un octet par échantillon ; HDR : 16 bits, le code 10 bits en
    // haut (convention P010 de la Station).
    let mut bytes: Vec<Vec<u8>> = Vec::new();
    let elem = if c.hdr { 2 } else { 1 };
    for p in planes.iter() {
        if c.hdr {
            // Valeur normalisée v = code16 / 65535 : celle que la Station envoie.
            bytes.push(p.iter().flat_map(|v| (v << 6).to_le_bytes()).collect());
        } else {
            bytes.push(p.iter().map(|v| *v as u8).collect());
        }
    }
    let widths = [c.width as usize, cw as usize, cw as usize];
    let heights = [c.height as usize, ch as usize, ch as usize];
    let buf = pw_cpu_buffer {
        data: [bytes[0].as_mut_ptr() as *mut c_void, bytes[1].as_mut_ptr() as *mut c_void, bytes[2].as_mut_ptr() as *mut c_void],
        row_stride_in_bytes: [widths[0] * elem, widths[1] * elem, widths[2] * elem],
        plane_size_in_bytes: [bytes[0].len(), bytes[1].len(), bytes[2].len()],
        width: c.width as i32,
        height: c.height as i32,
        format: match (c.chroma444, c.hdr) {
            (false, false) => pyrowave::PYROWAVE_CPU_BUFFER_FORMAT_YUV420P,
            (true, false) => pyrowave::PYROWAVE_CPU_BUFFER_FORMAT_YUV444P,
            (false, true) => pyrowave::PYROWAVE_CPU_BUFFER_FORMAT_YUV420P16,
            (true, true) => pyrowave::PYROWAVE_CPU_BUFFER_FORMAT_YUV444P16,
        },
    };
    let _ = heights;
    unsafe {
        if (api.encoder_encode_cpu)(enc, &buf, &max_bytes) != PYROWAVE_SUCCESS {
            return Err("encodage".into());
        }
        let mut n = 0usize;
        if (api.encoder_compute_num_packets)(enc, PACKET, &mut n) != PYROWAVE_SUCCESS {
            return Err("nombre de paquets".into());
        }
        let mut packets = vec![pw_packet::default(); n];
        let mut stream = vec![0u8; max_bytes + n * 64 + 4096];
        let mut out = 0usize;
        if (api.encoder_packetize)(enc, packets.as_mut_ptr(), PACKET, &mut out, stream.as_mut_ptr() as *mut c_void, stream.len()) != PYROWAVE_SUCCESS {
            return Err("mise en paquets".into());
        }
        let mut frame = Vec::with_capacity(max_bytes);
        for p in &packets[..out] {
            frame.extend_from_slice(&stream[p.offset..p.offset + p.size]);
        }
        Ok(frame)
    }
}

fn staging(d3d: &ID3D11Device, w: u32, h: u32, format: DXGI_FORMAT) -> Result<ID3D11Texture2D, String> {
    let desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: format,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
    };
    let mut t = None;
    unsafe { d3d.CreateTexture2D(&desc, None, Some(&mut t)) }.map_err(|e| e.to_string())?;
    t.ok_or_else(|| "texture de relecture".into())
}

/// PSNR d'un plan relu (codes 8 ou 10 bits) contre la source.
fn psnr(ctx: &ID3D11DeviceContext, t: &ID3D11Texture2D, src: &[u16], w: usize, h: usize, hdr: bool) -> Result<f64, String> {
    let mut m = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe { ctx.Map(t, 0, D3D11_MAP_READ, 0, Some(&mut m)) }.map_err(|e| e.to_string())?;
    let mut se = 0f64;
    for j in 0..h {
        for i in 0..w {
            let v = unsafe {
                let row = (m.pData as *const u8).add(j * m.RowPitch as usize);
                if hdr {
                    // r16 : convention P010, le code 10 bits en haut.
                    f64::from(*(row as *const u16).add(i)) / 64.0
                } else {
                    f64::from(*row.add(i))
                }
            };
            let d = v - f64::from(src[j * w + i]);
            se += d * d;
        }
    }
    unsafe { ctx.Unmap(t, 0) };
    let peak: f64 = if hdr { 1023.0 } else { 255.0 };
    let mse = se / (w * h) as f64;
    Ok(if mse <= 0.0 { 99.0 } else { 10.0 * (peak * peak / mse).log10() })
}

fn run_case(d3d: &ID3D11Device, device: &std::sync::Arc<Device>, c: &Case, frames: usize, bpp: f64) -> Result<(), String> {
    let api = pyrowave::api().ok_or("DLL")?;
    let ctx = unsafe { d3d.GetImmediateContext() }.map_err(|e| e.to_string())?;
    let mut enc = std::ptr::null_mut();
    let info = pw_encoder_create_info { device: device.handle, width: c.width as i32, height: c.height as i32, chroma: c.chroma444 as u32 };
    if unsafe { (api.encoder_create)(&info, &mut enc) } != PYROWAVE_SUCCESS {
        return Err("encodeur".into());
    }
    struct Enc(*mut c_void, &'static pyrowave::Api);
    impl Drop for Enc {
        fn drop(&mut self) {
            unsafe { (self.1.encoder_destroy)(self.0) };
        }
    }
    let _enc = Enc(enc, api);
    let max_bytes = ((c.width as f64 * c.height as f64 * bpp / 8.0) as usize).max(4096);

    // La mire, encodée d'avance (l'encodage ne compte pas dans le temps de décodage).
    let mut sources = Vec::new();
    let mut encoded = Vec::new();
    for t in 0..frames {
        let mut s = source(c, t);
        encoded.push(encode(enc, c, &mut s, max_bytes)?);
        if t == 0 {
            sources.push(s);
        }
    }
    let bytes: usize = encoded.iter().map(|f| f.len()).sum();
    let mbps = bytes as f64 * 8.0 * 60.0 / frames as f64 / 1e6;

    let mut dec = Decoder::new(device.clone(), d3d)?;
    // La configuration passe par le paquet du contrat, comme d'une Station.
    let config = Config { chroma444: c.chroma444, hdr: c.hdr, full_range: false, width: c.width, height: c.height };
    dec.configure(Config::parse(&config.bytes()).ok_or("paquet de configuration")?)?;
    let format = if c.hdr { DXGI_FORMAT_R16_UNORM } else { DXGI_FORMAT_R8_UNORM };
    let (cw, ch) = config.chroma_size();
    let dst = [staging(d3d, c.width, c.height, format)?, staging(d3d, cw, ch, format)?, staging(d3d, cw, ch, format)?];

    // Décodage cadencé à 60 i/s, comme en session ; chaque image copiée comme
    // par le rendu.
    let mut times = Vec::with_capacity(frames);
    let mut psnrs = [0f64; 3];
    let period = Duration::from_micros(16_667);
    let mut next = Instant::now();
    for (t, frame) in encoded.iter().enumerate() {
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        }
        next += period;
        let at = Instant::now();
        match dec.decode(frame)? {
            Outcome::Ready(_) => times.push(at.elapsed().as_secs_f64() * 1000.0),
            Outcome::Skipped(why) => return Err(format!("image {t} sautée : {why}")),
        }
        dec.copy_into(&ctx, &dst)?;
        if t == 0 {
            let s = &sources[0];
            psnrs[0] = psnr(&ctx, &dst[0], &s[0], c.width as usize, c.height as usize, c.hdr)?;
            psnrs[1] = psnr(&ctx, &dst[1], &s[1], cw as usize, ch as usize, c.hdr)?;
            psnrs[2] = psnr(&ctx, &dst[2], &s[2], cw as usize, ch as usize, c.hdr)?;
        }
    }

    // Image tronquée : sautée, et la suivante se décode seule.
    let half = &encoded[1][..encoded[1].len() / 2];
    let truncated = match dec.decode(half)? {
        Outcome::Skipped(_) => "sautée",
        Outcome::Ready(_) => "décodée quand même (partielle)",
    };
    let after = match dec.decode(&encoded[2])? {
        Outcome::Ready(_) => {
            dec.copy_into(&ctx, &dst)?;
            "suivante décodée"
        }
        Outcome::Skipped(_) => return Err("l'image qui suit une image tronquée n'a pas été décodée".into()),
    };

    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let avg = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "{} : {:.0} Mbit/s à 60 i/s ; décodage {:.2} ms en moyenne (médiane {:.2}, max {:.1}) ; PSNR Y {:.1}, Cb {:.1}, Cr {:.1} dB ; image tronquée {truncated}, {after}",
        label(c),
        mbps,
        avg,
        sorted[sorted.len() / 2],
        sorted[sorted.len() - 1],
        psnrs[0],
        psnrs[1],
        psnrs[2],
    );
    if psnrs[0] < 30.0 {
        return Err(format!("PSNR de Y trop bas ({:.1} dB) : image mal décodée ou mal relue", psnrs[0]));
    }
    Ok(())
}
