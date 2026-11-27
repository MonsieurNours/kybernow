//! Ce que le décodeur de l'image (`video`) et la sortie du son (`audio`)
//! demandent à ce qu'ils affichent : une session d'ordinateur (Kyber,
//! `session::Shared`) ou une partie sur console PlayStation
//! (`playstation::Host`). Les mêmes MediaCodec et AAudio servent les deux.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use kybernow_core::audio::AudioEngine;
use ndk::native_window::NativeWindow;

use crate::gate::{SurfaceGate, RELEASE_TIMEOUT};
use crate::jvm::Listener;
use crate::video::Msg;

/// La surface d'une session ou d'une partie, et le fil de l'image à prévenir
/// quand elle change.
#[derive(Default)]
pub struct SurfaceSlot {
    window: Mutex<Option<NativeWindow>>,
    gate: SurfaceGate,
    video_tx: Mutex<Option<Sender<Msg>>>,
}

impl SurfaceSlot {
    pub fn new(window: Option<NativeWindow>) -> Self {
        Self { window: Mutex::new(window), ..Default::default() }
    }

    pub fn window(&self) -> Option<NativeWindow> {
        self.window.lock().unwrap().clone()
    }

    pub fn gate(&self) -> &SurfaceGate {
        &self.gate
    }

    pub fn set_video_tx(&self, tx: Sender<Msg>) {
        *self.video_tx.lock().unwrap() = Some(tx);
    }

    /// Nouvelle surface, ou surface retirée (`None`, `surfaceDestroyed`) :
    /// dans ce cas, on attend (250 ms au plus) que le fil de l'image l'ait
    /// lâchée, pour qu'Android la reprenne sans qu'on y écrive encore (AND-04).
    pub fn set(&self, window: Option<NativeWindow>) {
        let gone = window.is_none();
        let generation = {
            let mut w = self.window.lock().unwrap();
            *w = window;
            self.gate.bump()
        };
        let sent = self.video_tx.lock().unwrap().as_ref().is_some_and(|tx| tx.send(Msg::Surface).is_ok());
        if gone && sent && !self.gate.wait_released(generation, RELEASE_TIMEOUT) {
            log::warn!("image : la surface retirée était encore utilisée après {} ms", RELEASE_TIMEOUT.as_millis());
        }
    }
}

pub trait MediaHost: Send + Sync + 'static {
    /// L'objet Kotlin qui reçoit `onDecoder`, `onVideoSize`, `onAudioOutput`.
    fn listener(&self) -> &Listener;
    /// La surface de la `SurfaceView` (nulle en arrière-plan).
    fn window(&self) -> Option<NativeWindow>;
    /// Taille du décodeur au départ.
    fn video_size(&self) -> (u32, u32);
    /// Durée d'une image de l'écran de l'appareil (instant d'affichage estimé).
    fn frame_period_micros(&self) -> i64;
    /// Le fil du décodeur (la surface qui change lui est annoncée).
    fn set_video_tx(&self, tx: Sender<Msg>);
    /// Les changements de surface, numérotés : le fil de l'image dit quand il a lâché l'ancienne.
    fn surface_gate(&self) -> &SurfaceGate;
    /// L'image est en HDR10 (BT.2020, PQ) : dit au décodeur.
    fn hdr(&self) -> bool {
        false
    }
    /// Les valeurs HDR de l'écran de l'appareil (métadonnées HDR10 de
    /// secours, si le flux n'a pas les siennes) ; `None` : rien à donner.
    fn hdr_luminance(&self) -> Option<kybernow_core::profile::HdrLuminance> {
        None
    }
    /// Octets d'une image reçue (mesures).
    fn received_bytes(&self, _n: usize) {}
    /// Trou dans le flux (mesures).
    fn hole(&self) {}
    /// Le décodeur a besoin d'une image clé (surface refaite, décodeur ouvert sans image clé).
    fn request_keyframe(&self);
    /// Le décodeur a perdu sa référence : une image clé est attendue.
    fn keyframe_lost(&self);
    /// Le décodeur a retrouvé une image clé.
    fn keyframe_recovered(&self);
    /// Taille des images décodées.
    fn decoded_size(&self, _w: u32, _h: u32) {}
    /// Image décodée écartée (une plus récente est montrée).
    fn skipped(&self, _pts: u64) {}
    /// Image montrée : reçue à `received`, décodée à `decoded`, à l'écran vers `shown` (µs).
    fn frame_displayed(&self, _pts: u64, _received: i64, _decoded: i64, _shown: i64) {}
    /// Le moteur de son vient d'être créé (son coupé, mesures).
    fn audio_engine(&self, _engine: &Arc<AudioEngine>) {}
}
