//! Règles pures des apps KyberNow Link (audit du 27/11, vague 2) : ce que
//! chaque app refaisait de son côté, écrit une fois. Rien ici ne dépend
//! d'un système, de Kyber ni d'une autre caisse : le cœur (`kybernow-core`)
//! les reprend et les expose aux ponts (C, JNI, Tauri), et les interfaces
//! qui se compilent sans le cœur (Linux et LG, pour leurs captures) peuvent
//! en dépendre directement.
//!
//! Chaque module ne nomme que `super::` et std : un fichier peut aussi être
//! repris tel quel par `#[path]` (c'est le cas de `relay.rs` et
//! `fingerprint.rs` dans `welcome.rs` du cœur, tant que les interfaces
//! Linux et LG l'incluent ainsi).

pub mod address;
pub mod fingerprint;
pub mod format;
pub mod hdr;
pub mod limits;
pub mod links;
pub mod relay;
