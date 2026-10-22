//! Sortie du son par AAudio, en mode faible latence : le moteur de son du
//! cœur (Opus, tampon adaptatif, dérive, masquage, mixage descendant) est
//! tiré directement par le rappel temps réel d'AAudio, sans passer par Java.
//!
//! Le flux s'ouvre au premier paquet de configuration (le nombre de voies
//! est alors connu), sur un fil à part : le fil réseau de Kyber ne bloque
//! pas. Une sortie qui change (casque branché, HDMI) ferme le flux : il est
//! rouvert sur la nouvelle sortie. Une sortie qui refuse 5.1 ou 7.1 reçoit
//! la stéréo, mixée par le moteur (le nombre de voies vient de la taille
//! du tampon demandé).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use jni::objects::JValue;
use ndk::audio::{
    AudioCallbackResult, AudioContentType, AudioDirection, AudioFormat, AudioPerformanceMode, AudioSharingMode, AudioStream, AudioStreamBuilder,
    AudioUsage,
};

use kybernow_core::audio::{AudioEngine, MonotonicClock, OutputOrder};
use kybernow_core::media::{Packet, PacketHandler};

use crate::session::Shared;

const RATE: i32 = 48_000;

struct Stream(AudioStream);
unsafe impl Send for Stream {}

pub struct AudioSink {
    engine: Arc<AudioEngine>,
    stream: Arc<Mutex<Option<Stream>>>,
    closed: Arc<AtomicBool>,
    shared: Arc<Shared>,
}

impl AudioSink {
    pub fn new(shared: Arc<Shared>) -> Arc<Self> {
        // Ordre des voies d'Android : celui de Windows (G, D, C, LFE, arrière G, D, côtés G, D).
        let engine = Arc::new(AudioEngine::with_order(Arc::new(MonotonicClock::new()), OutputOrder::Windows));
        Arc::new(Self { engine, stream: Arc::new(Mutex::new(None)), closed: Arc::new(AtomicBool::new(false)), shared })
    }

    fn open_later(&self) {
        let (engine, stream, closed, shared) = (self.engine.clone(), self.stream.clone(), self.closed.clone(), self.shared.clone());
        let _ = std::thread::Builder::new().name("kybernow-son".into()).spawn(move || open(engine, stream, closed, shared));
    }
}

impl PacketHandler for AudioSink {
    fn packet(&self, packet: Packet<'_>) {
        match packet {
            Packet::Codec { .. } => {}
            Packet::Media { is_config: true, data, .. } => {
                self.engine.configure(data);
                self.open_later();
            }
            Packet::Media { data, .. } => {
                if !data.is_empty() {
                    self.engine.decode(data);
                }
            }
            Packet::Hole { missing_samples } => self.engine.conceal(missing_samples as usize),
        }
    }
}

impl Drop for AudioSink {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.engine.stop();
        if let Some(s) = self.stream.lock().unwrap().take() {
            let _ = s.0.request_stop();
        }
    }
}

fn open(engine: Arc<AudioEngine>, slot: Arc<Mutex<Option<Stream>>>, closed: Arc<AtomicBool>, shared: Arc<Shared>) {
    if closed.load(Ordering::Acquire) {
        return;
    }
    if let Some(old) = slot.lock().unwrap().take() {
        let _ = old.0.request_stop();
    }
    let wanted = engine.channels().max(2) as i32;
    let attempts = [(wanted, AudioSharingMode::Exclusive), (wanted, AudioSharingMode::Shared), (2, AudioSharingMode::Shared)];
    let mut opened = None;
    for (channels, sharing) in attempts {
        if channels > wanted {
            continue;
        }
        match build(engine.clone(), slot.clone(), closed.clone(), shared.clone(), channels, sharing) {
            Ok(s) => {
                opened = Some(s);
                break;
            }
            Err(e) => log::warn!("son : sortie {channels} voies ({sharing:?}) refusée ({e:?})"),
        }
    }
    let Some(stream) = opened else {
        log::error!("son : aucune sortie AAudio");
        return;
    };
    if let Err(e) = stream.request_start() {
        log::error!("son : démarrage refusé ({e:?})");
        return;
    }
    let rate = stream.sample_rate().max(1) as f64;
    let buffer_ms = stream.buffer_size_in_frames() as f64 * 1000.0 / rate;
    engine.set_output_latency_ms(buffer_ms);
    let channels = stream.channel_count();
    log::info!(
        "son : AAudio {channels} voies, {} Hz, {:?}, {:?}, tampon {buffer_ms:.1} ms",
        stream.sample_rate(),
        stream.sharing_mode(),
        stream.performance_mode()
    );
    shared.listener.void("onAudioOutput", "(ID)V", &[JValue::Int(channels), JValue::Double(buffer_ms)]);
    *slot.lock().unwrap() = Some(Stream(stream));
}

fn build(
    engine: Arc<AudioEngine>,
    slot: Arc<Mutex<Option<Stream>>>,
    closed: Arc<AtomicBool>,
    shared: Arc<Shared>,
    channels: i32,
    sharing: AudioSharingMode,
) -> ndk::audio::Result<AudioStream> {
    let fill = engine.clone();
    AudioStreamBuilder::new()?
        .direction(AudioDirection::Output)
        .format(AudioFormat::PCM_Float)
        .sample_rate(RATE)
        .channel_count(channels)
        .performance_mode(AudioPerformanceMode::LowLatency)
        .sharing_mode(sharing)
        .usage(AudioUsage::Game)
        .content_type(AudioContentType::Movie)
        .data_callback(Box::new(move |stream, data, frames| {
            let frames = frames.max(0) as usize;
            let n = frames * stream.channel_count().max(1) as usize;
            let out = unsafe { std::slice::from_raw_parts_mut(data as *mut f32, n) };
            fill.fill(out, frames);
            AudioCallbackResult::Continue
        }))
        .error_callback(Box::new(move |_, e| {
            // Sortie débranchée ou changée : AAudio demande de rouvrir ailleurs
            // que dans ce rappel.
            log::info!("son : sortie perdue ({e:?}), réouverture");
            let (engine, slot, closed, shared) = (engine.clone(), slot.clone(), closed.clone(), shared.clone());
            let _ = std::thread::Builder::new().name("kybernow-son".into()).spawn(move || open(engine, slot, closed, shared));
        }))
        .open_stream()
}
