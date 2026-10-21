//! Lie FFmpeg (libavcodec, libswresample, libavutil) et libopus en statique depuis le rootfs
//! Windows de Kyber (`KYBERNOW_NATIVE_PREFIX`, posé par scripts/build.sh), et
//! génère les liaisons Rust de FFmpeg avec bindgen.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=KYBERNOW_NATIVE_PREFIX");
    println!("cargo:rerun-if-env-changed=KYBERNOW_NATIVE_LIB");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let prefix = env::var("KYBERNOW_NATIVE_PREFIX")
        .expect("KYBERNOW_NATIVE_PREFIX : lancer apps/windows/scripts/build.sh");

    // Dossier qui ne contient que les bibliothèques ci-dessous : celui du
    // rootfs contient aussi libvlc.dll.a, qui prendrait la place des
    // fonctions vides de VLC.
    let libdir = env::var("KYBERNOW_NATIVE_LIB").expect("KYBERNOW_NATIVE_LIB : lancer apps/windows/scripts/build.sh");
    println!("cargo:rustc-link-search=native={libdir}");
    // Statiques, dans l'ordre des dépendances (libavcodec.pc, libavutil.pc) ;
    // x264, VPL et zlib n'existent que dans la libavcodec du serveur (x64).
    let present = |lib: &str| std::path::Path::new(&libdir).join(format!("lib{lib}.a")).exists();
    for lib in ["avcodec", "swresample", "avutil", "opus", "x264", "vpl", "z"] {
        if present(lib) {
            println!("cargo:rustc-link-lib=static={lib}");
        }
    }
    for lib in ["mfuuid", "ole32", "strmiids", "user32", "gdi32", "uuid", "bcrypt"] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
    if present("vpl") {
        // libvpl est en C++ : runtime C++ de MinGW lié en statique.
        println!("cargo:rustc-link-lib=static=stdc++");
    }

    let bindings = bindgen::Builder::default()
        .header_contents(
            "ffmpeg.h",
            "#include <libavcodec/avcodec.h>\n#include <libavutil/hwcontext.h>\n#include <libavutil/pixdesc.h>\n#include <libavutil/error.h>\n",
        )
        .clang_arg(format!("-I{prefix}/include"))
        .allowlist_function("avcodec_.*|av_new_packet|av_packet_.*|av_frame_.*|av_hwdevice_.*|av_buffer_.*|av_log_set_level|av_strerror|av_get_pix_fmt_name")
        .allowlist_type("AVCodecContext|AVPacket|AVFrame|AVHWDeviceContext|AVPixelFormat|AVHWDeviceType|AVColorSpace|AVColorRange|AVCodecID")
        .allowlist_var("AV_CODEC_FLAG.*|AV_CODEC_FLAG2.*|AV_PKT_FLAG_.*|AV_FRAME_FLAG_.*|FF_THREAD_.*|AV_LOG_.*|AV_NUM_DATA_POINTERS")
        .default_enum_style(bindgen::EnumVariation::Consts)
        .prepend_enum_name(false)
        .layout_tests(false)
        .derive_default(true)
        .generate()
        .expect("liaisons FFmpeg");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out.join("ffmpeg.rs"))
        .expect("écriture des liaisons FFmpeg");
}
