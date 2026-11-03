//! Ce que le décodeur de l'image (`video`) et la sortie du son (`audio`)
//! demandent à ce qu'ils affichent : une session d'ordinateur (Kyber,
//! `session::Shared`) ou une partie sur console PlayStation
//! (`playstation::Host`). Les mêmes MediaCodec et AAudio servent les deux.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use kybernow_core::audio::AudioEngine;
use ndk::native_window::NativeWindow;

use crate::jvm::Listener;
use crate::video::Msg;

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
    /// L'image est en HDR10 (BT.2020, PQ) : dit au décodeur.
    fn hdr(&self) -> bool {
        false
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
