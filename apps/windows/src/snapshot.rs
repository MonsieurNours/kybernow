//! Dernière image d'une session, gardée pour l'accueil (comme sur Mac) :
//! encodée en JPEG par Windows (WIC), rangée dans
//! `%LOCALAPPDATA%\\KyberNow\\images\\<ordinateur>-last.jpg`.

use std::path::PathBuf;

use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatJpeg, GUID_WICPixelFormat24bppBGR, IWICImagingFactory, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, STATFLAG_NONAME, STREAM_SEEK_SET,
};
use windows::Win32::UI::Shell::SHCreateMemStream;

/// BGRA → JPEG.
pub fn jpeg(bgra: &[u8], w: u32, h: u32) -> windows::core::Result<Vec<u8>> {
    // JPEG sans alpha : BGR, ligne par ligne.
    let mut bgr = Vec::with_capacity((w * h * 3) as usize);
    for px in bgra.chunks_exact(4) {
        bgr.extend_from_slice(&px[..3]);
    }
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = SHCreateMemStream(None).ok_or_else(windows::core::Error::empty)?;
        let encoder = factory.CreateEncoder(&GUID_ContainerFormatJpeg, std::ptr::null())?;
        encoder.Initialize(&stream, WICBitmapEncoderNoCache)?;
        let mut frame = None;
        let mut props = None;
        encoder.CreateNewFrame(&mut frame, &mut props)?;
        let frame = frame.ok_or_else(windows::core::Error::empty)?;
        frame.Initialize(props.as_ref())?;
        frame.SetSize(w, h)?;
        let mut format = GUID_WICPixelFormat24bppBGR;
        frame.SetPixelFormat(&mut format)?;
        frame.WritePixels(h, w * 3, &bgr)?;
        frame.Commit()?;
        encoder.Commit()?;
        let mut stat = Default::default();
        stream.Stat(&mut stat, STATFLAG_NONAME)?;
        let len = stat.cbSize as usize;
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        let mut out = vec![0u8; len];
        let mut read = 0u32;
        stream.Read(out.as_mut_ptr().cast(), len as u32, Some(&mut read)).ok()?;
        out.truncate(read as usize);
        Ok(out)
    }
}

fn dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("KyberNow").join("images")
}

pub fn path(id: &str) -> PathBuf {
    let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    dir().join(format!("{safe}-last.jpg"))
}

pub fn save(id: &str, jpeg: &[u8]) {
    let p = path(id);
    let _ = std::fs::create_dir_all(dir());
    let tmp = p.with_extension("tmp");
    if std::fs::write(&tmp, jpeg).is_ok() {
        let _ = std::fs::rename(&tmp, &p);
    }
}

pub fn load(id: &str) -> Option<Vec<u8>> {
    std::fs::read(path(id)).ok().filter(|b| !b.is_empty())
}

pub fn forget(id: &str) {
    let _ = std::fs::remove_file(path(id));
}
