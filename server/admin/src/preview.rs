//! Aperçu de l'écran principal de l'ordinateur, en JPEG, pour le fond de
//! l'accueil des apps (`GET /api/stream/preview`).
//!
//! Le service vit dans la session 0, sans écran : sous Windows, la capture
//! est faite par l'agent (`kyber-admin display-agent preview`) lancé dans la
//! session de l'utilisateur, qui écrit le JPEG dans `preview/` ; la route le
//! lit. La dernière capture est gardée 10 s, et resservie telle quelle
//! pendant une session (pas de capture qui pourrait gêner le flux).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Result, bail};
use axum::body::Bytes;

/// Durée pendant laquelle une capture est resservie sans en refaire une.
const CACHE: Duration = Duration::from_secs(10);
/// Après un échec (écran verrouillé…), délai avant de réessayer.
const RETRY: Duration = Duration::from_secs(5);

pub const MIN_WIDTH: u32 = 320;
pub const MAX_WIDTH: u32 = 1600;
pub const DEFAULT_WIDTH: u32 = 960;
/// Qualité JPEG (0–100).
pub const QUALITY: u8 = 70;

#[derive(Clone)]
pub struct Shot {
    pub jpeg: Bytes,
    pub width: u32,
    pub captured_at: SystemTime,
    taken: Instant,
}

enum Last {
    Shot(Shot),
    Failed { at: Instant, message: String },
}

pub struct Preview {
    data_dir: PathBuf,
    /// Verrou tenu pendant la capture : des appels simultanés n'en lancent qu'une.
    last: tokio::sync::Mutex<Option<Last>>,
    /// Dernière capture réussie, resservie pendant une session.
    last_shot: std::sync::Mutex<Option<Shot>>,
}

impl Preview {
    pub fn new(data_dir: &Path) -> Self {
        Self { data_dir: data_dir.to_path_buf(), last: tokio::sync::Mutex::new(None), last_shot: std::sync::Mutex::new(None) }
    }

    /// Capture de l'écran à `width` pixels de large (en cache si récente).
    /// `streaming` : une session est en cours, on ne capture pas.
    pub async fn get(&self, width: u32, streaming: bool) -> Result<Shot, String> {
        if streaming {
            return self
                .last_shot
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| "une session est en cours sur l'ordinateur : aperçu indisponible".to_string());
        }
        let mut last = self.last.lock().await;
        match &*last {
            Some(Last::Shot(shot)) if shot.width == width && shot.taken.elapsed() < CACHE => return Ok(shot.clone()),
            Some(Last::Failed { at, message }) if at.elapsed() < RETRY => return Err(message.clone()),
            _ => {}
        }
        let data_dir = self.data_dir.clone();
        let result = tokio::task::spawn_blocking(move || capture(&data_dir, width))
            .await
            .map_err(anyhow::Error::from)
            .and_then(|r| r);
        match result {
            Ok(jpeg) => {
                let shot = Shot { jpeg: Bytes::from(jpeg), width, captured_at: SystemTime::now(), taken: Instant::now() };
                *self.last_shot.lock().unwrap() = Some(shot.clone());
                *last = Some(Last::Shot(shot.clone()));
                Ok(shot)
            }
            Err(e) => {
                let message = format!("{e:#}");
                tracing::debug!("aperçu de l'écran : {message}");
                *last = Some(Last::Failed { at: Instant::now(), message: message.clone() });
                Err(message)
            }
        }
    }
}

/// Fait la capture et renvoie le JPEG.
#[cfg(windows)]
fn capture(data_dir: &Path, width: u32) -> Result<Vec<u8>> {
    let dir = data_dir.join("preview");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("capture-{}.jpg", rand::random::<u32>()));
    let args = vec!["preview".to_string(), "--width".into(), width.to_string(), "--jpeg".into(), file.display().to_string()];
    let result = crate::display::agent::run_with_timeout(data_dir, &args, 10_000);
    let jpeg = std::fs::read(&file);
    let _ = std::fs::remove_file(&file);
    result?;
    Ok(jpeg?)
}

#[cfg(not(windows))]
fn capture(data_dir: &Path, width: u32) -> Result<Vec<u8>> {
    let dir = data_dir.join("preview");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join(format!("capture-{}.jpg", rand::random::<u32>()));
    let result = capture_to(&file, width);
    let jpeg = std::fs::read(&file);
    let _ = std::fs::remove_file(&file);
    result?;
    Ok(jpeg?)
}

/// Agent : capture l'écran principal, réduit à `width` pixels de large, en
/// JPEG dans `path`.
#[cfg(windows)]
pub fn capture_to(path: &Path, width: u32) -> Result<()> {
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDC, GetDIBits, HALFTONE, ReleaseDC, SRCCOPY, SelectObject, SetBrushOrgEx, SetStretchBltMode,
        StretchBlt,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN, SetProcessDPIAware};

    check_desktop()?;
    let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
    unsafe {
        // Pixels réels de l'écran, pas ceux mis à l'échelle de Windows.
        SetProcessDPIAware();
        let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        if sw <= 0 || sh <= 0 {
            bail!("aucun écran");
        }
        let w = (width as i32).min(sw);
        let h = ((sh as i64 * w as i64 + sw as i64 / 2) / sw as i64).max(1) as i32;

        let screen = GetDC(std::ptr::null_mut());
        if screen.is_null() {
            bail!("écran inaccessible");
        }
        let mem = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let previous = SelectObject(mem, bitmap);
        SetStretchBltMode(mem, HALFTONE);
        SetBrushOrgEx(mem, 0, 0, std::ptr::null_mut());
        // L'écran principal commence en (0, 0) du bureau virtuel.
        let copied = StretchBlt(mem, 0, 0, w, h, screen, 0, 0, sw, sh, SRCCOPY) != 0;

        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // de haut en bas
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        SelectObject(mem, previous);
        let lines = GetDIBits(mem, bitmap, 0, h as u32, pixels.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        DeleteObject(bitmap);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        if !copied || lines != h {
            bail!("capture de l'écran : {}", std::io::Error::last_os_error());
        }

        let encoder = jpeg_encoder::Encoder::new_file(path, QUALITY)?;
        encoder.encode(&pixels, w as u16, h as u16, jpeg_encoder::ColorType::Bgra)?;
        Ok(())
    }
}

/// Refuse la capture quand personne n'est connecté, ou quand l'écran affiché
/// n'est pas le bureau de l'utilisateur (verrouillage, connexion, UAC) : on
/// n'obtiendrait qu'une image noire.
#[cfg(windows)]
fn check_desktop() -> Result<()> {
    use windows_sys::Win32::System::RemoteDesktop::{
        WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTSFreeMemory, WTSQuerySessionInformationW, WTSUserName,
    };
    use windows_sys::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_READOBJECTS, GetUserObjectInformationW, OpenInputDesktop, UOI_NAME,
    };
    unsafe {
        let mut name: windows_sys::core::PWSTR = std::ptr::null_mut();
        let mut bytes = 0u32;
        let mut user = String::new();
        if WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION, WTSUserName, &mut name, &mut bytes) != 0
            && !name.is_null()
        {
            let len = (0..).take_while(|&i| *name.add(i) != 0).count();
            user = String::from_utf16_lossy(std::slice::from_raw_parts(name, len));
            WTSFreeMemory(name.cast());
        }
        if user.is_empty() {
            bail!("personne n'est connecté à l'ordinateur");
        }

        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            bail!("l'écran de l'ordinateur est verrouillé");
        }
        let mut buf = [0u16; 64];
        let mut needed = 0u32;
        let ok = GetUserObjectInformationW(desktop, UOI_NAME, buf.as_mut_ptr().cast(), (buf.len() * 2) as u32, &mut needed);
        CloseDesktop(desktop);
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        if ok == 0 || !String::from_utf16_lossy(&buf[..len]).eq_ignore_ascii_case("Default") {
            bail!("l'écran de l'ordinateur est verrouillé ou affiche une demande de sécurité");
        }
    }
    // L'écran de verrouillage de Windows 10/11 peut s'afficher sur le bureau « Default ».
    if crate::foreground::probe()["exe"].as_str() == Some("lockapp.exe") {
        bail!("l'écran de l'ordinateur est verrouillé");
    }
    Ok(())
}

/// Mac (build de développement) : `screencapture`, réduit par `sips`.
#[cfg(target_os = "macos")]
pub fn capture_to(path: &Path, width: u32) -> Result<()> {
    use std::process::Command;
    let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
    let ok = Command::new("screencapture").args(["-x", "-m", "-t", "jpg"]).arg(path).status()?.success();
    if !ok || !path.exists() {
        bail!("capture de l'écran impossible (session verrouillée, ou autorisation d'enregistrement de l'écran)");
    }
    let quality = QUALITY.to_string();
    let out = Command::new("sips")
        .args(["--resampleWidth", &width.to_string(), "-s", "format", "jpeg", "-s", "formatOptions", &quality])
        .arg(path)
        .arg("--out")
        .arg(path)
        .output()?;
    if !out.status.success() {
        bail!("réduction de la capture : {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn capture_to(_path: &Path, _width: u32) -> Result<()> {
    bail!("aperçu de l'écran indisponible sur ce système")
}
