//! Pont JNI du cœur KyberNow pour l'app Android (`libkybernow.so`), voir
//! apps/android/README.md. L'équivalent du pont C des apps Apple
//! (`apps/apple`), pour Kotlin : la console (Admin), la session, les
//! entrées. Le décodage de l'image (MediaCodec) et la sortie du son (AAudio)
//! sont ici aussi, en Rust : le chemin chaud ne passe jamais par Java.
//!
//! Hors d'Android la crate se réduit aux règles pures (`hdr`), essayées par
//! `cargo test` : le reste de l'espace de travail se compile sans le NDK.

#[cfg(target_os = "android")]
mod audio;
#[cfg(target_os = "android")]
mod exports;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod hdr;
#[cfg(target_os = "android")]
mod jvm;
#[cfg(target_os = "android")]
mod lend;
#[cfg(target_os = "android")]
mod media;
#[cfg(target_os = "android")]
mod playstation;
#[cfg(target_os = "android")]
mod relay;
#[cfg(target_os = "android")]
mod session;
#[cfg(target_os = "android")]
mod share;
#[cfg(target_os = "android")]
mod update;
#[cfg(target_os = "android")]
mod video;
