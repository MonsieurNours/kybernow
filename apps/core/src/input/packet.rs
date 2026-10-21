//! Paquets kynput dont nous tenons une référence, et l'envoi vers la chaîne.
//!
//! `kynput_rs::Packet` est un pointeur nu : il ne libère rien quand il est
//! jeté, et `kynput_consumer_consume` prend la référence qu'on lui passe,
//! accepté ou non. [`OwnedPacket`] rend cela explicite : une référence
//! libérée à la fin (`kynput_packet_unref`), clonée pour chaque essai
//! d'envoi ([`OwnedPacket::clone_ref`]), ou cédée à kynput
//! ([`OwnedPacket::into_raw`]).

use kynput_rs::Packet;
use kynput_sys as sys;

/// Cible de tous les paquets produits par l'appareil : l'ordinateur.
pub(crate) const HOST: sys::kynput_InputTarget = sys::kynput_InputTarget_KYNPUT_INPUT_TARGET_HOST;

/// Une référence sur un paquet kynput, libérée à la fin.
pub struct OwnedPacket(*mut sys::kynput_Packet);

unsafe impl Send for OwnedPacket {}
unsafe impl Sync for OwnedPacket {}

impl OwnedPacket {
    /// Prend la référence d'un paquet construit par `kynput_rs` (`to_packet`).
    /// `None` : pointeur nul (kynput a refusé de construire le paquet).
    pub fn from_packet(packet: Packet) -> Option<Self> {
        let ptr = packet.to_ptr_mut();
        (!ptr.is_null()).then_some(Self(ptr))
    }

    /// Prend une référence déjà détenue (résultat d'une fonction `*_to_packet`
    /// de kynput). `None` : pointeur nul.
    ///
    /// # Safety
    /// `ptr` doit être nul, ou une référence que l'appelant nous cède.
    pub unsafe fn from_raw(ptr: *mut sys::kynput_Packet) -> Option<Self> {
        (!ptr.is_null()).then_some(Self(ptr))
    }

    /// Une référence de plus sur le même paquet (un essai d'envoi la consomme).
    pub fn clone_ref(&self) -> Self {
        Self(unsafe { sys::kynput_packet_clone(self.0) })
    }

    /// Cède la référence (à `kynput_consumer_consume`, qui la garde).
    pub fn into_raw(self) -> *mut sys::kynput_Packet {
        let ptr = self.0;
        std::mem::forget(self);
        ptr
    }

    /// Lecture du paquet par les fonctions de `kynput_rs` (`from_packet`).
    pub fn borrow(&self) -> Packet {
        Packet::from_ptr(self.0)
    }
}

impl Drop for OwnedPacket {
    fn drop(&mut self) {
        unsafe { sys::kynput_packet_unref(self.0) }
    }
}

/// L'envoi d'un paquet vers la chaîne kynput : prend la référence, dit s'il
/// est parti. kynput refuse (`false`) quand sa file d'envoi (64 paquets) est
/// pleine ou tant que le réseau n'est pas relié. Les tests y substituent un
/// envoi qui refuse à volonté.
pub trait PacketSink: Send + Sync {
    fn consume(&self, packet: OwnedPacket) -> bool;
}

/// Le puits d'entrée d'une chaîne kynput (`kynput_pipeline_get_input_sink`).
pub struct KynputSink(*mut sys::kynput_Consumer);

unsafe impl Send for KynputSink {}
unsafe impl Sync for KynputSink {}

impl KynputSink {
    pub fn of(pipeline: &kynput_rs::Pipeline) -> Self {
        Self(unsafe { sys::kynput_pipeline_get_input_sink(pipeline.to_ptr()) })
    }
}

impl PacketSink for KynputSink {
    fn consume(&self, packet: OwnedPacket) -> bool {
        unsafe { sys::kynput_consumer_consume(self.0, packet.into_raw()) == 0 }
    }
}

impl Drop for KynputSink {
    fn drop(&mut self) {
        unsafe { sys::kynput_consumer_unref(self.0) }
    }
}
