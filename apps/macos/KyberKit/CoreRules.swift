// Les règles communes des apps, tenues par le cœur (phase 2b de l'audit du
// 27/11, docs/audit-2026-11-27/migration-coeur-2b.md) : les apps Apple n'en
// gardent plus de copie en Swift. Empreinte d'Admin (affichée, comparée),
// adresse tapée et adresse d'Admin, valeurs communes (profils, bornes du
// débit, délais, port), débit d'une session, PyroWave (débit d'un profil,
// « Définition ramenée », paquet de configuration), lien d'invitation.
// Mac, iPhone, iPad et Apple TV (pont apps/apple/src/rules.rs).

import Foundation
@_implementationOnly import CKyberNow

// MARK: - Empreinte d'Admin

public enum Fingerprint {
    /// L'empreinte telle que link-ui la montre : les quatre premiers groupes
    /// en capitales, « 7C41 E09B AABB CCDD » ; vide sans chiffre.
    public static func short(_ fp: String) -> String {
        Core.take(kn_short_fingerprint(fp)) ?? ""
    }

    /// Les groupes de `short`, pour les dessiner un à un.
    public static func groups(_ fp: String) -> [String] {
        short(fp).split(separator: " ").map(String.init)
    }

    /// Deux empreintes sont la même (préfixe `sha256:`, deux-points, espaces
    /// et casse ignorés) ; l'une ou l'autre absente ou vide : jamais (MAC-20).
    public static func same(_ a: String?, _ b: String?) -> Bool {
        guard let a, let b else { return false }
        return kn_fingerprint_same(a, b)
    }
}

// MARK: - Adresses

public enum CoreAddress {
    /// Une adresse tapée (« 192.168.1.20:9443 », « [fe80::1]:9443 », un nom) :
    /// hôte et port, nil si elle ne se lit pas. `defaultPort` : le port pris
    /// quand il manque ; nil l'exige. Une IPv6 nue est un hôte entier.
    public static func parse(_ text: String, defaultPort: Int?) -> (host: String, port: Int)? {
        guard let json = Core.take(kn_parse_address(text, UInt16(clamping: defaultPort ?? 0))),
              let v = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any],
              let host = v["host"] as? String, let port = v["port"] as? Int else { return nil }
        return (host, port)
    }

    /// L'adresse d'Admin d'un hôte (crochets d'une IPv6 posés une seule fois).
    public static func adminURL(host: String, port: Int) -> URL? {
        Core.take(kn_admin_url(host, UInt16(clamping: port))).flatMap(URL.init(string:))
    }
}

// MARK: - Valeurs communes

/// Les valeurs que toutes les apps partagent (`plan::values_json` du cœur),
/// pour ce que décode cet appareil.
public struct CoreValues: Sendable {
    /// Réglages d'un profil sur cet appareil (`StreamSettings` du cœur).
    public struct ProfileValues: Sendable, Equatable {
        public let codec: VideoCodec
        public let yuv444: Bool
        public let mbps: Int
        public let reliable: Bool
    }

    public let jeu: ProfileValues
    public let bureau: ProfileValues
    public let max: ProfileValues
    /// Débit de base de PyroWave, en 4:2:0 et en 4:4:4 (Mbit/s, 4K à 60 Hz).
    public let pyrowaveMbps: Int
    public let pyrowaveMbps444: Int
    /// Débit réglé à la main sur cet appareil : le plus bas, le pas, le plus
    /// haut (HEVC, AV1, H.264), le plus haut en PyroWave (Mbit/s).
    public let manualMin: Int
    public let manualStep: Int
    public let manualMax: Int
    public let manualMaxPyroWave: Int
    /// Débit demandé quand l'image n'est plus regardée, sur cet appareil (bit/s).
    public let backgroundBitrate: Int
    public let maxGuests: Int
    public let maxSessions: Int
    /// Attente au plus du lien du relais d'un partage ou d'un code de prêt.
    public let relayLinkWait: TimeInterval
    /// Mise à jour d'une Station : attente avant l'avis, et la version
    /// d'avant laquelle une Station ne sait pas le dire.
    public let stationNoticeWait: TimeInterval
    public let stationNoticeBefore: String
    public let adminPort: Int
    /// « 192.168.1.20:9443 » : l'exemple de l'écran d'ajout.
    public let addressExample: String
    public let defaultRelay: String

    /// Les valeurs de cet appareil, lues une fois.
    public static let current = CoreValues(capabilities: .current)

    init(capabilities: DecoderCapabilities) {
        let caps = (try? JSONSerialization.data(withJSONObject: SessionEngine.capabilitiesJSON(capabilities))).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
        let v = Core.take(kn_values_json(caps))
            .flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] } ?? [:]
        func int(_ o: Any?) -> Int { (o as? NSNumber)?.intValue ?? 0 }
        func profile(_ key: String) -> ProfileValues {
            let p = (v["profiles"] as? [String: Any])?[key] as? [String: Any] ?? [:]
            return ProfileValues(codec: VideoCodec(rawValue: p["codec"] as? String ?? "") ?? (capabilities.hevc ? .hevc : .h264),
                                 yuv444: p["yuv444"] as? Bool ?? false, mbps: int(p["mbps"]), reliable: p["reliable"] as? Bool ?? false)
        }
        let manual = v["manual_mbps"] as? [String: Any] ?? [:]
        let device = v["device_manual_mbps"] as? [String: Any] ?? [:]
        jeu = profile("jeu")
        bureau = profile("bureau")
        max = profile("max")
        pyrowaveMbps = int(v["pyrowave_mbps"])
        pyrowaveMbps444 = int(v["pyrowave_mbps_444"])
        manualMin = int(manual["min"])
        manualStep = int(manual["step"])
        manualMax = int(device["max"])
        manualMaxPyroWave = int(device["max_pyrowave"])
        backgroundBitrate = int(v["device_background_bitrate"])
        maxGuests = int(v["max_guests"])
        maxSessions = int(v["max_sessions"])
        relayLinkWait = TimeInterval(int(v["relay_link_wait_secs"]))
        stationNoticeWait = TimeInterval(int(v["station_notice_wait_secs"]))
        stationNoticeBefore = v["station_notice_before"] as? String ?? ""
        adminPort = int(v["admin_port"])
        addressExample = v["address_example"] as? String ?? ""
        defaultRelay = v["default_relay"] as? String ?? ""
    }

    /// Le plus haut débit réglé à la main dans ce format.
    public func manualMax(_ codec: VideoCodec) -> Int {
        codec == .pyrowave ? manualMaxPyroWave : manualMax
    }

    /// Un débit réglé à la main (ancien réglage, reçu d'un autre appareil)
    /// ramené dans les bornes de cet appareil et de ce format.
    public static func clampManual(_ mbps: Int, codec: VideoCodec) -> Int {
        Int(kn_clamp_manual_mbps(UInt32(clamping: mbps), codec == .pyrowave))
    }
}

// MARK: - Débit d'une session

public enum CoreBitrate {
    /// Le débit d'une session pour ces réglages (même JSON que `settings` au
    /// départ) à ce mode, en Mbit/s : un profil suit les pixels par seconde,
    /// un débit réglé à la main reste (`StreamSettings::bitrate`).
    public static func session(_ settings: SessionEngine.Settings, width: Int, height: Int, refresh: Int) -> Int {
        guard let data = try? JSONSerialization.data(withJSONObject: settings.json),
              let json = String(data: data, encoding: .utf8) else { return settings.mbps }
        let mbps = Int(kn_settings_bitrate(json, UInt32(clamping: width), UInt32(clamping: height), UInt32(clamping: refresh)))
        return mbps > 0 ? mbps : settings.mbps
    }

    /// Le même débit pour des écrans dont les pixels par seconde font
    /// ensemble `pixelRate` (multi-écrans : la somme des écrans de la session).
    public static func session(_ settings: SessionEngine.Settings, pixelRate: Double) -> Int {
        guard let data = try? JSONSerialization.data(withJSONObject: settings.json),
              let json = String(data: data, encoding: .utf8) else { return settings.mbps }
        let mbps = Int(kn_settings_bitrate_for_pixel_rate(json, pixelRate))
        return mbps > 0 ? mbps : settings.mbps
    }
}

// MARK: - PyroWave

public enum PyroWaveRules {
    /// Le débit de PyroWave d'un profil pour ce mode (Mbit/s), en 4:2:0 ou en 4:4:4.
    public static func mbps(width: Int, height: Int, refresh: Int, yuv444: Bool) -> Int {
        Int(kn_pyrowave_mbps(UInt32(clamping: width), UInt32(clamping: height), UInt32(clamping: refresh), yuv444))
    }

    /// « Définition ramenée à {l} × {h} pour tenir dans {n} Mbit/s. »
    public static func loweredText(width: Int, height: Int, refresh: Int, mbps: Int) -> String {
        Core.take(kn_pyrowave_lowered_text(UInt32(clamping: width), UInt32(clamping: height), UInt32(clamping: refresh), UInt32(clamping: mbps))) ?? ""
    }
}

// MARK: - Lien d'invitation

public enum InviteLink {
    /// Le texte a la forme d'un lien d'invitation, lisible ou non.
    public static func looksLike(_ text: String) -> Bool {
        kn_looks_like_invite(text)
    }
}
