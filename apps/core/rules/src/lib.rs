//! Règles pures des apps KyberNow Link (audit du 27/11, vague 2) : ce que
//! chaque app refaisait de son côté, écrit une fois. Rien ici ne dépend
//! d'un système, de Kyber ni d'une autre caisse (sauf `serde`, facultatif,
//! pour le JSON de l'état de l'accueil) : le cœur (`kybernow-core`) les
//! reprend et les expose aux ponts (C, JNI, Tauri), et les interfaces qui se
//! compilent sans le cœur (Linux et LG, pour leurs captures) en dépendent
//! directement. Chaque module ne nomme que `super::` et std.

pub mod address;
pub mod fingerprint;
pub mod format;
pub mod hdr;
pub mod limits;
pub mod links;
pub mod nal;
pub mod relay;
pub mod station;
pub mod welcome;
