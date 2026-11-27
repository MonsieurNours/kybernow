//! La caméra de ce PC vers l'ordinateur (docs/webcam.md, règles dans
//! docs/link-ui.md, « Caméra de l'appareil ») : Media Foundation lit la
//! webcam (lecteur de source, conversion en NV12 à la définition voulue) et
//! un encodeur H.264 de Windows l'encode (celui de la carte graphique s'il y
//! en a un, asynchrone ; sinon celui de Microsoft, synchrone), en temps réel :
//! débit constant, sans images B, une image clé toutes les 2 s et à la
//! demande. Chaque image (Annex B, SPS et PPS devant chaque image clé) part
//! au cœur (`CameraLink::frame`), qui l'envoie ou la lâche.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kybernow_core::camera::{CameraCodec, CameraLink, CameraResolution, CameraSettings, FPS, FrameOutcome};
use log::{info, warn};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::Win32::System::Variant::VARIANT;
use windows::core::{GUID, Interface, PWSTR};

/// Une webcam : son lien symbolique (identifiant gardé) et son nom.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CameraChoice {
    pub id: String,
    pub name: String,
}

/// Pourquoi la caméra ne part pas (avis de docs/link-ui.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// Accès refusé par la confidentialité de Windows.
    Denied,
    InUse,
    Unplugged,
    /// Pas d'encodeur, ou la webcam refuse toute définition proposée.
    NoEncoder,
}

/// Ce que dit l'échec de l'ouverture de la webcam (`ActivateObject`, lecteur).
fn open_problem(code: windows::core::HRESULT) -> Problem {
    use windows::Win32::Foundation::{E_ACCESSDENIED, ERROR_SHARING_VIOLATION};
    if code == E_ACCESSDENIED {
        Problem::Denied
    } else if code == MF_E_HW_MFT_FAILED_START_STREAMING
        || code == MF_E_VIDEO_RECORDING_DEVICE_PREEMPTED
        || code == ERROR_SHARING_VIOLATION.to_hresult()
    {
        Problem::InUse
    } else if code == MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED {
        Problem::Unplugged
    } else {
        // Inconnu : comme avant, le cas le plus fréquent.
        Problem::InUse
    }
}

/// Ferme l'objet activé (webcam, encodeur) quand il est lâché : sans
/// `ShutdownObject`, l'encodeur de la carte (sessions NVENC comptées) et la
/// webcam restent pris.
struct Activated(IMFActivate);

impl Drop for Activated {
    fn drop(&mut self) {
        let _ = unsafe { self.0.ShutdownObject() };
    }
}

fn mf_started() -> bool {
    static STARTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *STARTED.get_or_init(|| unsafe { MFStartup(MF_VERSION, MFSTARTUP_LITE).is_ok() })
}

fn pwstr(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

fn devices() -> Vec<IMFActivate> {
    if !mf_started() {
        return Vec::new();
    }
    unsafe {
        let mut attrs: Option<IMFAttributes> = None;
        if MFCreateAttributes(&mut attrs, 1).is_err() {
            return Vec::new();
        }
        let Some(attrs) = attrs else { return Vec::new() };
        if attrs.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID).is_err() {
            return Vec::new();
        }
        let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        if MFEnumDeviceSources(&attrs, &mut list, &mut count).is_err() || list.is_null() {
            return Vec::new();
        }
        let out = (0..count as usize).filter_map(|i| (*list.add(i)).take()).collect();
        CoTaskMemFree(Some(list as *const _));
        out
    }
}

/// Les webcams de ce PC, dans l'ordre de Windows.
pub fn cameras() -> Vec<CameraChoice> {
    devices()
        .iter()
        .filter_map(|d| unsafe {
            let mut name = PWSTR::null();
            let mut link = PWSTR::null();
            let mut n = 0u32;
            d.GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, &mut name, &mut n).ok()?;
            let name = pwstr(name);
            d.GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, &mut link, &mut n).ok()?;
            Some(CameraChoice { id: pwstr(link), name })
        })
        .collect()
}

/// Ce PC a une webcam.
pub fn available() -> bool {
    !devices().is_empty()
}

/// L'envoi en cours : un fil qui lit, encode et donne au cœur.
/// Au plus ce temps pour que le fil de la caméra finisse quand on l'arrête :
/// une webcam qui ne rend plus d'image (obturateur, pilote) bloque
/// `ReadSample` ; le fil est alors laissé finir seul (il n'envoie plus rien).
const STOP_WITHIN: Duration = Duration::from_secs(1);

pub struct CameraSender {
    stop: Arc<AtomicBool>,
    key: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl CameraSender {
    /// Ouvre la webcam `id` (nulle : la première) et démarre l'envoi :
    /// `START` au cœur avec la taille obtenue. `problem` : appelé (du fil de
    /// la caméra) si elle s'arrête en route.
    pub fn start(
        link: Arc<CameraLink>,
        id: Option<String>,
        resolution: CameraResolution,
        device: String,
        problem: Arc<dyn Fn(Problem, String) + Send + Sync>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let key = Arc::new(AtomicBool::new(true));
        let (s, k) = (stop.clone(), key.clone());
        let thread = std::thread::Builder::new()
            .name("kybernow-camera".into())
            .spawn(move || {
                let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
                let name = Mutex::new(String::new());
                if let Err(p) = run(&link, id, resolution, &device, &s, &k, &name) {
                    problem(p, name.lock().unwrap().clone());
                }
                unsafe { CoUninitialize() };
            })
            .ok();
        Self { stop, key, thread }
    }

    /// La prochaine image sera une image clé.
    pub fn force_keyframe(&self) {
        self.key.store(true, Ordering::Relaxed);
    }
}

impl Drop for CameraSender {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let started = Instant::now();
            while !t.is_finished() && started.elapsed() < STOP_WITHIN {
                std::thread::sleep(Duration::from_millis(10));
            }
            if t.is_finished() {
                let _ = t.join();
            } else {
                warn!("caméra : la webcam ne répond plus, son fil finira seul");
            }
        }
    }
}

const fn pack(a: u32, b: u32) -> u64 {
    ((a as u64) << 32) | b as u64
}

fn video_type(subtype: &GUID, w: u32, h: u32) -> windows::core::Result<IMFMediaType> {
    unsafe {
        let t = MFCreateMediaType()?;
        t.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        t.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        t.SetUINT64(&MF_MT_FRAME_SIZE, pack(w, h))?;
        t.SetUINT64(&MF_MT_FRAME_RATE, pack(FPS, 1))?;
        t.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack(1, 1))?;
        t.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        Ok(t)
    }
}

/// Le lecteur de la webcam, en NV12 à la définition voulue (Windows convertit
/// et réduit si la webcam ne la sait pas telle quelle) ; refusée en 1080p, en
/// 720p. Rend aussi la webcam activée (fermée quand elle est lâchée) et la
/// définition obtenue.
fn reader(id: Option<&str>, wanted: CameraResolution, name: &Mutex<String>) -> Result<(IMFSourceReader, Activated, CameraResolution), Problem> {
    let list = devices();
    let chosen = list.iter().find(|d| unsafe {
        let mut link = PWSTR::null();
        let mut n = 0;
        id.is_some_and(|id| d.GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, &mut link, &mut n).is_ok() && pwstr(link) == id)
    });
    let device = chosen.or(list.first()).ok_or(Problem::Unplugged)?;
    unsafe {
        let mut n = 0;
        let mut p = PWSTR::null();
        if device.GetAllocatedString(&MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, &mut p, &mut n).is_ok() {
            *name.lock().unwrap() = pwstr(p);
        }
        let fail = |what: &str, e: windows::core::Error| {
            warn!("caméra : {what} ({e})");
            open_problem(e.code())
        };
        let source: IMFMediaSource = device.ActivateObject().map_err(|e| fail("ouverture refusée", e))?;
        let activated = Activated(device.clone());
        let mut attrs: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attrs, 2).map_err(|e| fail("attributs", e))?;
        let attrs = attrs.ok_or(Problem::NoEncoder)?;
        let _ = attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1);
        let _ = attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1);
        let reader = MFCreateSourceReaderFromMediaSource(&source, &attrs).map_err(|e| fail("lecteur refusé", e))?;
        let stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let mut last = None;
        for resolution in resolutions(wanted) {
            let (w, h) = resolution.size();
            let t = video_type(&MFVideoFormat_NV12, w, h).map_err(|e| fail("type", e))?;
            match reader.SetCurrentMediaType(stream, None, &t) {
                Ok(()) => return Ok((reader, activated, resolution)),
                Err(e) => {
                    warn!("caméra : {} refusé ({e})", resolution.label());
                    last = Some(e);
                }
            }
        }
        // Aucune définition : ni occupée, ni refusée, elle ne sait pas faire.
        let _ = last;
        Err(Problem::NoEncoder)
    }
}

/// Les définitions à essayer, de la voulue à la plus petite.
fn resolutions(wanted: CameraResolution) -> Vec<CameraResolution> {
    match wanted {
        CameraResolution::P1080 => vec![CameraResolution::P1080, CameraResolution::P720],
        CameraResolution::P720 => vec![CameraResolution::P720],
    }
}

/// Un encodeur H.264 : matériel d'abord (asynchrone), sinon logiciel ; rend
/// aussi son objet activé (fermé quand il est lâché, comme chaque candidat écarté).
fn encoder(w: u32, h: u32, bitrate: u32) -> Result<(IMFTransform, bool, Activated), Problem> {
    let input = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_NV12 };
    let output = MFT_REGISTER_TYPE_INFO { guidMajorType: MFMediaType_Video, guidSubtype: MFVideoFormat_H264 };
    for flags in [MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER, MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_SORTANDFILTER] {
        unsafe {
            let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
            let mut count = 0u32;
            if MFTEnumEx(MFT_CATEGORY_VIDEO_ENCODER, flags, Some(&input), Some(&output), &mut list, &mut count).is_err() || list.is_null() {
                continue;
            }
            let all: Vec<IMFActivate> = (0..count as usize).filter_map(|i| (*list.add(i)).take()).collect();
            CoTaskMemFree(Some(list as *const _));
            for a in all {
                let Ok(t) = a.ActivateObject::<IMFTransform>() else { continue };
                // Écarté (configuration refusée) : fermé en sortant de ce tour.
                let activated = Activated(a);
                let asynchronous = t.GetAttributes().ok().and_then(|at| at.GetUINT32(&MF_TRANSFORM_ASYNC).ok()).unwrap_or(0) != 0;
                if asynchronous && let Ok(at) = t.GetAttributes() {
                    let _ = at.SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, 1);
                }
                if configure(&t, w, h, bitrate).is_ok() {
                    info!("caméra : encodeur H.264 {}", if asynchronous { "matériel" } else { "logiciel" });
                    return Ok((t, asynchronous, activated));
                }
            }
        }
    }
    Err(Problem::NoEncoder)
}

fn codec_value(t: &IMFTransform, api: &GUID, v: u32) {
    if let Ok(c) = t.cast::<ICodecAPI>() {
        let value = VARIANT::from(v);
        let _ = unsafe { c.SetValue(api, &value) };
    }
}

fn configure(t: &IMFTransform, w: u32, h: u32, bitrate: u32) -> windows::core::Result<()> {
    unsafe {
        // Temps réel : peu de délai, débit constant, sans images B, une image clé toutes les 2 s.
        codec_value(t, &CODECAPI_AVLowLatencyMode, 1);
        codec_value(t, &CODECAPI_AVEncCommonRateControlMode, eAVEncCommonRateControlMode_CBR.0 as u32);
        codec_value(t, &CODECAPI_AVEncCommonMeanBitRate, bitrate);
        codec_value(t, &CODECAPI_AVEncMPVDefaultBPictureCount, 0);
        codec_value(t, &CODECAPI_AVEncMPVGOPSize, FPS * 2);
        let out = video_type(&MFVideoFormat_H264, w, h)?;
        out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate)?;
        out.SetUINT32(&MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_Main.0 as u32)?;
        t.SetOutputType(0, &out, 0)?;
        t.SetInputType(0, &video_type(&MFVideoFormat_NV12, w, h)?, 0)?;
        t.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
        t.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        Ok(())
    }
}

/// SPS et PPS de l'encodeur (en tête du type de sortie), pour les remettre
/// devant une image clé qui ne les porte pas.
fn sequence_header(t: &IMFTransform) -> Vec<u8> {
    unsafe {
        let Ok(out) = t.GetOutputCurrentType(0) else { return Vec::new() };
        let Ok(n) = out.GetBlobSize(&MF_MT_MPEG_SEQUENCE_HEADER) else { return Vec::new() };
        let mut v = vec![0u8; n as usize];
        if out.GetBlob(&MF_MT_MPEG_SEQUENCE_HEADER, &mut v, None).is_err() {
            return Vec::new();
        }
        v
    }
}

fn sample_bytes(s: &IMFSample) -> Option<Vec<u8>> {
    unsafe {
        let b = s.ConvertToContiguousBuffer().ok()?;
        let mut p = std::ptr::null_mut();
        let mut n = 0u32;
        b.Lock(&mut p, None, Some(&mut n)).ok()?;
        let v = std::slice::from_raw_parts(p, n as usize).to_vec();
        let _ = b.Unlock();
        Some(v)
    }
}

/// Le premier NAL est-il un SPS (type 7) ?
fn starts_with_sps(data: &[u8]) -> bool {
    let at = if data.starts_with(&[0, 0, 0, 1]) { 4 } else if data.starts_with(&[0, 0, 1]) { 3 } else { return false };
    data.get(at).is_some_and(|b| b & 0x1f == 7)
}

struct Encoded<'a> {
    link: &'a CameraLink,
    header: Vec<u8>,
    key: &'a AtomicBool,
    stop: &'a AtomicBool,
    started: Instant,
}

impl Encoded<'_> {
    /// Une sortie de l'encodeur vers le cœur (rien après l'arrêt : un fil
    /// laissé finir seul n'envoie plus d'image).
    fn deliver(&mut self, t: &IMFTransform, sample: &IMFSample) {
        if self.stop.load(Ordering::Relaxed) {
            return;
        }
        let Some(mut data) = sample_bytes(sample) else { return };
        let key = unsafe { sample.GetUINT32(&MFSampleExtension_CleanPoint) }.unwrap_or(0) != 0;
        if key && !starts_with_sps(&data) {
            if self.header.is_empty() {
                self.header = sequence_header(t);
            }
            let mut v = self.header.clone();
            v.append(&mut data);
            data = v;
        }
        let pts = self.started.elapsed().as_micros() as u64;
        if self.link.frame(&data, pts, key) == FrameOutcome::WantKeyframe {
            self.key.store(true, Ordering::Relaxed);
        }
    }
}

/// Une sortie de l'encodeur, si elle est prête ; `Ok(false)` : il lui faut une entrée.
fn pull(t: &IMFTransform, enc: &mut Encoded) -> windows::core::Result<bool> {
    unsafe {
        let info = t.GetOutputStreamInfo(0)?;
        let provides = info.dwFlags & MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 as u32 != 0;
        let mut buffer = MFT_OUTPUT_DATA_BUFFER { dwStreamID: 0, ..Default::default() };
        if !provides {
            let s = MFCreateSample()?;
            s.AddBuffer(&MFCreateMemoryBuffer(info.cbSize.max(1 << 20))?)?;
            buffer.pSample = std::mem::ManuallyDrop::new(Some(s));
        }
        let mut status = 0u32;
        let r = t.ProcessOutput(0, std::slice::from_mut(&mut buffer), &mut status);
        let sample = std::mem::ManuallyDrop::take(&mut buffer.pSample);
        let _ = std::mem::ManuallyDrop::take(&mut buffer.pEvents);
        match r {
            Ok(()) => {
                if let Some(s) = sample {
                    enc.deliver(t, &s);
                }
                Ok(true)
            }
            Err(e) if e.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(false),
            Err(e) if e.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                // Le format de sortie change : on le reprend tel que l'encodeur le propose.
                if let Ok(ty) = t.GetOutputAvailableType(0, 0) {
                    let _ = t.SetOutputType(0, &ty, 0);
                }
                enc.header.clear();
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }
}

fn read(reader: &IMFSourceReader) -> windows::core::Result<Option<IMFSample>> {
    let mut flags = 0u32;
    let mut sample: Option<IMFSample> = None;
    unsafe {
        reader.ReadSample(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0, None, Some(&mut flags), None, Some(&mut sample))?;
    }
    if flags & (MF_SOURCE_READERF_ERROR.0 as u32 | MF_SOURCE_READERF_ENDOFSTREAM.0 as u32) != 0 {
        return Err(windows::core::Error::from(MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED));
    }
    Ok(sample)
}

fn run(
    link: &CameraLink,
    id: Option<String>,
    resolution: CameraResolution,
    device: &str,
    stop: &AtomicBool,
    key: &AtomicBool,
    name: &Mutex<String>,
) -> Result<(), Problem> {
    if !mf_started() {
        return Err(Problem::NoEncoder);
    }
    // La webcam et l'encodeur sont fermés (`ShutdownObject`) à toute sortie.
    let (reader, _webcam, resolution) = reader(id.as_deref(), resolution, name)?;
    let (w, h) = resolution.size();
    let mut bitrate = link.target_bitrate();
    let (t, asynchronous, _encoder) = encoder(w, h, bitrate)?;
    if !link.start(CameraSettings { codec: CameraCodec::H264, width: w, height: h, fps: FPS, resolution, device: device.to_owned() }) {
        return Ok(());
    }
    let mut enc = Encoded { link, header: Vec::new(), key, stop, started: Instant::now() };
    let events = if asynchronous { t.cast::<IMFMediaEventGenerator>().ok() } else { None };
    let mut last_rate = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if last_rate.elapsed().as_secs() >= 1 {
            last_rate = Instant::now();
            let target = link.target_bitrate();
            if target != bitrate {
                bitrate = target;
                codec_value(&t, &CODECAPI_AVEncCommonMeanBitRate, bitrate);
            }
        }
        let feed = |t: &IMFTransform| -> Result<(), Problem> {
            let sample = match read(&reader) {
                Ok(Some(s)) => s,
                Ok(None) => return Ok(()),
                Err(e) => {
                    warn!("caméra : lecture arrêtée ({e})");
                    return Err(Problem::Unplugged);
                }
            };
            if key.swap(false, Ordering::Relaxed) {
                codec_value(t, &CODECAPI_AVEncVideoForceKeyFrame, 1);
            }
            unsafe { t.ProcessInput(0, &sample, 0) }.map_err(|_| Problem::NoEncoder)
        };
        match &events {
            Some(generator) => {
                // Encodeur matériel : il dit quand il veut une image, quand il en a une.
                let Ok(event) = (unsafe { generator.GetEvent(MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS(0)) }) else { break };
                match unsafe { event.GetType() }.unwrap_or(0) as i32 {
                    x if x == METransformNeedInput.0 => feed(&t)?,
                    x if x == METransformHaveOutput.0 => {
                        let _ = pull(&t, &mut enc);
                    }
                    _ => {}
                }
            }
            None => {
                feed(&t)?;
                while let Ok(true) = pull(&t, &mut enc) {}
            }
        }
    }
    unsafe {
        let _ = t.ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0);
        let _ = t.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echecs_d_ouverture() {
        use windows::Win32::Foundation::E_ACCESSDENIED;
        assert_eq!(open_problem(E_ACCESSDENIED), Problem::Denied);
        assert_eq!(open_problem(MF_E_HW_MFT_FAILED_START_STREAMING), Problem::InUse);
        assert_eq!(open_problem(MF_E_VIDEO_RECORDING_DEVICE_INVALIDATED), Problem::Unplugged);
        assert_eq!(resolutions(CameraResolution::P1080), vec![CameraResolution::P1080, CameraResolution::P720]);
    }

    #[test]
    fn sps_first() {
        assert!(starts_with_sps(&[0, 0, 0, 1, 0x67, 1]));
        assert!(starts_with_sps(&[0, 0, 1, 0x27]));
        assert!(!starts_with_sps(&[0, 0, 0, 1, 0x65]));
        assert!(!starts_with_sps(&[1, 2]));
    }
}
