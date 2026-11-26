use super::*;

const ALL: Capabilities = Capabilities { hevc: true, hevc444: true, av1: true, hdr: true, pyrowave: true };
const UHD120: Mode = Mode { width: 3840, height: 2160, refresh: 120 };

fn device(screen: Mode, wired: bool) -> Device {
    Device { screen, caps: ALL, hdr: false, network: Some("Maison".into()), wired }
}

fn station(pyrowave: bool) -> Station {
    Station { name: "PC".into(), pyrowave, pyrowave_max: Some(5000) }
}

fn fast_decode() -> Vec<DecodeResult> {
    vec![
        DecodeResult { codec: Codec::Hevc, yuv444: false, fps: 400.0 },
        DecodeResult { codec: Codec::Hevc, yuv444: true, fps: 300.0 },
        DecodeResult { codec: Codec::PyroWave, yuv444: false, fps: 300.0 },
        DecodeResult { codec: Codec::PyroWave, yuv444: true, fps: 200.0 },
    ]
}

fn measures(link: Link, mbps: Option<f64>, decode: Vec<DecodeResult>) -> Measures {
    Measures {
        link,
        latency: Some(Latency { min: 1.0, median: 1.5, jitter: 0.5 }),
        bandwidth: mbps.map(|mbps| Bandwidth { mbps, loaded_delta: Some(2.0) }),
        decode,
    }
}

#[test]
fn cable_rapide_prend_pyrowave_en_4k120() {
    let r = decide(&measures(Link::Cable, Some(9400.0), fast_decode()), &device(UHD120, true), &station(true), 1);
    if !PYROWAVE_KYBER {
        assert_eq!(r.codec, Codec::Hevc);
        return;
    }
    assert_eq!((r.codec, r.reasons.codec), (Codec::PyroWave, Reason::PyroWaveCarried));
    assert_eq!((r.width, r.height, r.refresh), (3840, 2160, 120));
    assert!(r.yuv444, "4:4:4 : décodé et porté");
    assert!(!r.reliable);
    // Hors du câble (même mesure) : HEVC, la raison dite.
    let r = decide(&measures(Link::Wifi, Some(9400.0), fast_decode()), &device(UHD120, false), &station(true), 1);
    assert_eq!((r.codec, r.reasons.codec), (Codec::Hevc, Reason::PyroWaveNeedsCable));
}

#[test]
fn gigabit_ne_porte_pas_pyrowave_en_4k120() {
    // 940 Mbit/s × 60 % / 1,3 ≈ 434 Mbit/s : moins que les ≈ 1 200 de PyroWave en 4K120.
    let r = decide(&measures(Link::Cable, Some(940.0), fast_decode()), &device(UHD120, true), &station(true), 1);
    if PYROWAVE_KYBER {
        assert_eq!(r.reasons.codec, Reason::PyroWaveNotCarried);
    }
    assert_eq!(r.codec, Codec::Hevc);
    assert_eq!((r.width, r.refresh), (3840, 120));
    // HEVC : le budget (434 Mbit/s) passe le plafond d'un encodeur matériel,
    // calé sur la 4K à 60 Hz.
    assert_eq!(r.mode_mbps, crate::profile::ENCODER_MAX_MBPS);
    assert_eq!(r.mbps, (f64::from(crate::profile::ENCODER_MAX_MBPS) / scale(UHD120)).round() as u32);
    // 10 Gbit/s : le même plafond (sans PyroWave proposé).
    let r = decide(&measures(Link::Cable, Some(9400.0), fast_decode()), &device(UHD120, true), &station(false), 1);
    assert_eq!(r.mode_mbps, crate::profile::ENCODER_MAX_MBPS);
}

#[test]
fn hevc_plafonne_a_ce_qu_un_encodeur_accepte() {
    // Le cas du 26/11/2026 : câble, 1294 Mbit/s mesurés, écran 4K à 160 Hz,
    // PyroWave plafonné à 600 par la Station (ne porte pas la 4K160) : HEVC
    // demandait 593 Mbit/s, refusé par NVENC à chaque départ.
    let st = Station { name: "PC".into(), pyrowave: true, pyrowave_max: Some(600) };
    let uhd160 = Mode { width: 3840, height: 2160, refresh: 160 };
    let r = decide(&measures(Link::Cable, Some(1294.0), fast_decode()), &device(uhd160, true), &st, 1);
    assert_eq!(r.codec, Codec::Hevc);
    assert_eq!(r.mode_mbps, crate::profile::ENCODER_MAX_MBPS);
    assert!(f64::from(r.mbps) * scale(uhd160) <= f64::from(crate::profile::ENCODER_MAX_MBPS) + 1.0);
    // Sous le plafond, le budget mesuré reste pris tel quel.
    let r = decide(&measures(Link::Wifi, Some(200.0), fast_decode()), &device(uhd160, false), &st, 1);
    assert!(r.mode_mbps < crate::profile::ENCODER_MAX_MBPS);
}

#[test]
fn wifi_lent_baisse_le_debit_et_garde_420() {
    let m = measures(Link::Wifi, Some(60.0), fast_decode());
    let r = decide(&m, &device(Mode { width: 2560, height: 1440, refresh: 60 }, false), &station(true), 1);
    assert_eq!(r.codec, Codec::Hevc);
    // 60 × 50 % / 1,3 ≈ 23 Mbit/s.
    assert_eq!(r.mode_mbps, 23);
    assert_eq!(r.reasons.bitrate, Reason::Measured);
    assert!(!r.yuv444);
    assert_eq!(r.reasons.yuv444, Reason::Yuv444TooTight);
    let fr = r.choices(Lang::Fr);
    assert_eq!(fr[3].value, "23 Mbit/s");
    assert_eq!(fr[3].reason, "60 Mbit/s mesurés, avec une marge");
}

#[test]
fn decodage_lent_baisse_la_frequence_puis_la_definition() {
    // 150 i/s en 4K : 4K120 demande 144 → tenu ; 100 i/s : 4K60 seulement.
    let slow = |fps: f64| vec![DecodeResult { codec: Codec::Hevc, yuv444: false, fps }];
    let r = decide(&measures(Link::Cable, Some(940.0), slow(150.0)), &device(UHD120, true), &station(false), 1);
    assert_eq!((r.width, r.refresh, r.reasons.mode), (3840, 120, Reason::MatchesDisplay));
    let r = decide(&measures(Link::Cable, Some(940.0), slow(100.0)), &device(UHD120, true), &station(false), 1);
    assert_eq!((r.width, r.refresh, r.reasons.mode), (3840, 60, Reason::DecodeLimited));
    assert_eq!(r.choices(Lang::Fr)[1].reason, "décodage limité à 100 i/s en 4K");
    // 40 i/s en 4K : 1440p60 demande 72 × (4K / 1440p) ≈ 32 → tenu.
    let r = decide(&measures(Link::Cable, Some(940.0), slow(40.0)), &device(UHD120, true), &station(false), 1);
    assert_eq!((r.width, r.height, r.refresh), (2560, 1440, 60));
}

#[test]
fn station_d_avant_sans_rafale() {
    let r = decide(&measures(Link::Wifi, None, fast_decode()), &device(Mode { width: 3840, height: 2160, refresh: 60 }, false), &station(false), 1);
    assert_eq!(r.reasons.bitrate, Reason::AssumedFromLink);
    // Wi-Fi supposé : 80 Mbit/s en 4K60.
    assert_eq!(r.mode_mbps, 80);
    assert_eq!(r.codec, Codec::Hevc);
    assert_eq!(r.choices(Lang::En)[3].reason, "based on the connection (Station to update)");
}

#[test]
fn hdr_et_reseau_irregulier() {
    let mut d = device(UHD120, true);
    d.hdr = true;
    let mut m = measures(Link::Wifi, Some(400.0), fast_decode());
    m.latency = Some(Latency { min: 3.0, median: 8.0, jitter: 14.0 });
    let r = decide(&m, &d, &station(false), 1);
    assert_eq!((r.yuv444, r.reasons.yuv444), (false, Reason::Yuv444Hdr));
    assert_eq!((r.reliable, r.reasons.transport), (true, Reason::Unsteady));
    let s = r.settings(true, false);
    assert_eq!((s.profile, s.adaptive, s.reliable, s.hdr), (Some(Profile::Auto), true, true, true));
}

#[test]
fn appareil_sans_hevc() {
    let mut d = device(Mode { width: 1920, height: 1080, refresh: 60 }, false);
    d.caps = Capabilities { av1: true, ..Capabilities::default() };
    let r = decide(&measures(Link::Wifi, Some(200.0), vec![]), &d, &station(true), 1);
    assert_eq!(r.codec, Codec::Av1);
    d.caps = Capabilities::default();
    let r = decide(&measures(Link::Wifi, Some(200.0), vec![]), &d, &station(true), 1);
    assert_eq!(r.codec, Codec::H264);
    assert_eq!(r.reasons.codec, Reason::BestFormat);
}

#[test]
fn resultat_en_json_et_reseau_du_test() {
    let r = decide(&measures(Link::Cable, Some(940.0), fast_decode()), &device(UHD120, true), &station(true), 42);
    let back: AutoResult = serde_json::from_value(serde_json::to_value(&r).unwrap()).unwrap();
    assert_eq!(back, r);
    assert!(r.same_network(Some("Maison")));
    assert!(!r.same_network(Some("relay")));
    let fr = r.choices(Lang::Fr);
    assert_eq!(fr.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["codec", "mode", "yuv444", "bitrate", "transport", "adaptive"]);
    assert_eq!(fr[1].value, "3840 × 2160 à 120 Hz");
    assert_eq!(fr[5].reason, "suit le réseau pendant la session");
}

#[test]
fn mesures_de_latence_et_de_debit() {
    let l = latency_of(&[3.0, 1.0, 2.0, 1.5, 9.0, 1.2, 1.1, 1.3, 1.4, 1.6]).unwrap();
    assert_eq!(l.min, 1.0);
    assert!(l.jitter > 1.0);
    assert_eq!(latency_of(&[]), None);
    // 16 quarts de seconde : la montée du début ne compte pas.
    let mut w = vec![1_000_000u64; 4];
    w.extend(vec![29_375_000u64; 12]);
    let mbps = bandwidth_of(&w, Duration::from_millis(250)).unwrap();
    assert_eq!(mbps.round(), 940.0);
    assert_eq!(bandwidth_of(&[5], Duration::from_millis(250)), None);
}

#[test]
fn extraits_embarques() {
    let all = samples();
    assert_eq!(all.len(), 6, "six extraits lisibles");
    for s in &all {
        assert_eq!((s.width, s.height), (3840, 2160));
        assert!(!s.packets.is_empty());
        assert!(s.packets[0].is_key, "{:?} commence par une image clé", s.codec);
    }
    let hevc = all.iter().find(|s| s.codec == Codec::Hevc && !s.yuv444).unwrap();
    assert_eq!(hevc.packets.len(), 20);
    assert_eq!(hevc.packets.iter().filter(|p| p.is_key).count(), 1);
    // PyroWave : une image qui commence par son en-tête de séquence (bit
    // `extended`, 3840 × 2160, chroma), pas par la fin du paquet de
    // configuration `KNPW` (extraits illisibles de la 0.11.2).
    for s in all.iter().filter(|s| s.codec == Codec::PyroWave) {
        let p = &s.packets[0].data;
        let w0 = u32::from_le_bytes(p[0..4].try_into().unwrap());
        let w1 = u32::from_le_bytes(p[4..8].try_into().unwrap());
        assert_eq!(w0 >> 31, 1, "en-tête de séquence PyroWave");
        assert_eq!(((w0 & 0x3FFF) + 1, ((w0 >> 14) & 0x3FFF) + 1), (3840, 2160));
        assert_eq!((w1 >> 26) & 1 == 1, s.yuv444);
    }
    // Un appareil sans HEVC 4:4:4 ni PyroWave n'essaie pas ces extraits.
    let caps = Capabilities { hevc: true, ..Capabilities::default() };
    assert!(samples_for(caps).iter().all(|s| !s.yuv444 && s.codec != Codec::PyroWave && s.codec != Codec::Av1));
    assert!(parse_sample(b"KNS0").is_none());
}

#[test]
fn avancement_en_json() {
    let mut p = Progress::new(Lang::Fr);
    p.set(Step::Link, StepState::Done("Câble".into()));
    p.set(Step::Latency, StepState::Running);
    p.set(Step::Bandwidth, StepState::Skipped("Mettez à jour PC pour mesurer le débit.".into()));
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["steps"][0], serde_json::json!({ "step": "link", "title": "Liaison", "state": "done", "text": "Câble" }));
    assert_eq!(v["steps"][1]["state"], "running");
    assert_eq!(v["steps"][2]["state"], "skipped");
    assert_eq!(v["steps"][3]["state"], "waiting");
}
