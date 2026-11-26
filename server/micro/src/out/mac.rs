//! Mac : l'entrée cachée « KyberNow Micro (entrée) » du pilote KyberNow.driver
//! (server/macos/audio-driver), que les apps entendent dans « Micro KyberNow ».
//! Core Audio tire le son par une procédure d'entrées-sorties (IOProc).

#![allow(non_upper_case_globals)]

use std::ffi::{CString, c_char, c_void};
use std::sync::{Arc, Mutex};

use super::Check;
use crate::jitter::Jitter;

const FEED_UID: &str = "fr.leslignesclaires.kybernow.micro.feed";
const MIC_UID: &str = "fr.leslignesclaires.kybernow.micro";
const NAME: &str = "Micro KyberNow";

type AudioObjectID = u32;
type OSStatus = i32;
type CFStringRef = *const c_void;

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[repr(C)]
struct AudioBuffer {
    channels: u32,
    size: u32,
    data: *mut c_void,
}

#[repr(C)]
struct AudioBufferList {
    count: u32,
    buffers: [AudioBuffer; 1],
}

type IOProc = extern "C" fn(
    AudioObjectID,
    *const c_void,
    *const AudioBufferList,
    *const c_void,
    *mut AudioBufferList,
    *const c_void,
    *mut c_void,
) -> OSStatus;

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyData(
        id: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> OSStatus;
    fn AudioObjectSetPropertyData(
        id: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: u32,
        data: *const c_void,
    ) -> OSStatus;
    fn AudioDeviceCreateIOProcID(device: AudioObjectID, proc_: IOProc, client: *mut c_void, id: *mut *mut c_void) -> OSStatus;
    fn AudioDeviceDestroyIOProcID(device: AudioObjectID, id: *mut c_void) -> OSStatus;
    fn AudioDeviceStart(device: AudioObjectID, id: *mut c_void) -> OSStatus;
    fn AudioDeviceStop(device: AudioObjectID, id: *mut c_void) -> OSStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, encoding: u32) -> CFStringRef;
    fn CFRelease(cf: *const c_void);
}

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}
const kAudioObjectSystemObject: AudioObjectID = 1;
const kAudioHardwarePropertyTranslateUIDToDevice: u32 = fourcc(b"uidd");
const kAudioHardwarePropertyDefaultInputDevice: u32 = fourcc(b"dIn ");
const kAudioObjectPropertyScopeGlobal: u32 = fourcc(b"glob");
const kCFStringEncodingUTF8: u32 = 0x0800_0100;

fn global(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        selector,
        scope: kAudioObjectPropertyScopeGlobal,
        element: 0,
    }
}

/// L'appareil du pilote d'après son identifiant ; `None` s'il n'est pas là.
fn device(uid: &str) -> Option<AudioObjectID> {
    let c = CString::new(uid).ok()?;
    unsafe {
        let s = CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), kCFStringEncodingUTF8);
        if s.is_null() {
            return None;
        }
        let mut id: AudioObjectID = 0;
        let mut size = std::mem::size_of::<AudioObjectID>() as u32;
        let status = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &global(kAudioHardwarePropertyTranslateUIDToDevice),
            std::mem::size_of::<CFStringRef>() as u32,
            &s as *const CFStringRef as *const c_void,
            &mut size,
            &mut id as *mut AudioObjectID as *mut c_void,
        );
        CFRelease(s);
        (status == 0 && id != 0).then_some(id)
    }
}

fn default_input() -> Option<AudioObjectID> {
    let mut id: AudioObjectID = 0;
    let mut size = std::mem::size_of::<AudioObjectID>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &global(kAudioHardwarePropertyDefaultInputDevice),
            0,
            std::ptr::null(),
            &mut size,
            &mut id as *mut AudioObjectID as *mut c_void,
        )
    };
    (status == 0 && id != 0).then_some(id)
}

fn set_default_input(id: AudioObjectID) -> OSStatus {
    unsafe {
        AudioObjectSetPropertyData(
            kAudioObjectSystemObject,
            &global(kAudioHardwarePropertyDefaultInputDevice),
            0,
            std::ptr::null(),
            std::mem::size_of::<AudioObjectID>() as u32,
            &id as *const AudioObjectID as *const c_void,
        )
    }
}

pub fn check() -> Check {
    match (device(FEED_UID), device(MIC_UID)) {
        (Some(_), Some(_)) => Check::Ready(NAME.into()),
        _ => Check::Missing("KyberNow.driver sans « Micro KyberNow » (à mettre à jour avec la Station)".into()),
    }
}

extern "C" fn io_proc(
    _device: AudioObjectID,
    _now: *const c_void,
    _input: *const AudioBufferList,
    _input_time: *const c_void,
    output: *mut AudioBufferList,
    _output_time: *const c_void,
    client: *mut c_void,
) -> OSStatus {
    if output.is_null() || client.is_null() {
        return 0;
    }
    // Pas d'allocation ni d'attente ici : fil temps réel de Core Audio.
    let jitter = unsafe { &*(client as *const Mutex<Jitter>) };
    let list = unsafe { &mut *output };
    if list.count == 0 {
        return 0;
    }
    let buffer = &mut list.buffers[0];
    if buffer.data.is_null() || buffer.channels == 0 {
        return 0;
    }
    let total = buffer.size as usize / std::mem::size_of::<f32>();
    let samples = unsafe { std::slice::from_raw_parts_mut(buffer.data as *mut f32, total) };
    let channels = buffer.channels as usize;
    if channels == 1 {
        super::pull(jitter, samples);
    } else {
        // Le pilote est mono ; au cas où, la même voix partout.
        let mut mono = [0f32; 512];
        for chunk in samples.chunks_mut(channels * mono.len()) {
            let frames = chunk.len() / channels;
            super::pull(jitter, &mut mono[..frames]);
            for (f, s) in chunk.chunks_mut(channels).zip(mono.iter()) {
                f.fill(*s);
            }
        }
    }
    0
}

pub struct Player {
    device: AudioObjectID,
    proc_id: *mut c_void,
    /// Gardé en vie tant que la procédure tourne.
    jitter: Arc<Mutex<Jitter>>,
    previous: Option<AudioObjectID>,
}

impl Player {
    pub fn start(jitter: Arc<Mutex<Jitter>>) -> Result<Self, String> {
        let device = device(FEED_UID).ok_or("KyberNow.driver sans « Micro KyberNow »")?;
        let mut proc_id = std::ptr::null_mut();
        let client = Arc::as_ptr(&jitter) as *mut c_void;
        let status = unsafe { AudioDeviceCreateIOProcID(device, io_proc, client, &mut proc_id) };
        if status != 0 {
            return Err(format!("Core Audio refuse la procédure ({status})"));
        }
        let status = unsafe { AudioDeviceStart(device, proc_id) };
        if status != 0 {
            unsafe { AudioDeviceDestroyIOProcID(device, proc_id) };
            return Err(format!("Core Audio ne démarre pas l'entrée ({status})"));
        }
        Ok(Self {
            device,
            proc_id,
            jitter,
            previous: None,
        })
    }

    pub fn name(&self) -> &str {
        NAME
    }

    pub fn make_default(&mut self) {
        let Some(mic) = device(MIC_UID) else {
            return;
        };
        match default_input() {
            Some(current) if current == mic => {}
            current => {
                let status = set_default_input(mic);
                if status == 0 {
                    eprintln!("micro par défaut : {NAME}");
                    self.previous = current;
                } else {
                    eprintln!("micro par défaut inchangé ({status})");
                }
            }
        }
    }

    pub fn stop(mut self) {
        unsafe {
            AudioDeviceStop(self.device, self.proc_id);
            AudioDeviceDestroyIOProcID(self.device, self.proc_id);
        }
        let _ = &self.jitter;
        if let (Some(previous), Some(mic)) = (self.previous.take(), device(MIC_UID))
            && default_input() == Some(mic)
        {
            let status = set_default_input(previous);
            eprintln!("micro par défaut rendu ({status})");
        }
    }
}
