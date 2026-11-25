// PyroWave dans les apps (docs/link-ui.md, « PyroWave » ; docs/pyrowave.md) :
// l'annonce de la Station, le câble de cet appareil, et ce que « Format
// vidéo » en montre. Commun aux apps Apple ; les textes sont dans chaque app
// (« cette Apple TV », « cet iPhone », « cet iPad »).

import Foundation
import Network

/// L'annonce de la Station (`pyrowave` de `GET /api/stream/info`) : un objet
/// `{available, reason}` comme le HDR (`reason` : `off`, `windows`, `vulkan`),
/// ou un booléen. Absente : Station d'avant PyroWave (`nil`).
public struct PyroWaveOffer: Sendable, Equatable {
    public let available: Bool
    public let reason: String?

    public init(available: Bool, reason: String? = nil) {
        self.available = available
        self.reason = available ? nil : reason
    }

    public static func parse(_ json: Any?) -> PyroWaveOffer? {
        if let b = json as? Bool { return PyroWaveOffer(available: b) }
        guard let j = json as? [String: Any] else { return nil }
        return PyroWaveOffer(available: j["available"] as? Bool ?? false, reason: j["reason"] as? String)
    }
}

/// Comment cet appareil joint l'ordinateur, pour PyroWave : par câble
/// (Ethernet, sur le même réseau), en Wi-Fi (ou tout lien sans câble), ou
/// par le relais (hors de chez soi).
public enum PyroWaveLink: Sendable, Equatable {
    case cable, wifi, relay

    public init(wired: Bool, relay: Bool) {
        self = relay ? .relay : wired ? .cable : .wifi
    }
}

/// « Format vidéo » face à PyroWave : la valeur montrée ou non, utilisable
/// ou inactive avec sa raison, et le lien du moment.
public struct PyroWaveChoice: Sendable, Equatable {
    public enum Block: Sendable, Equatable {
        /// « {nom} ne propose pas PyroWave. »
        case notOffered
        /// « {cet appareil} ne sait pas décoder PyroWave. »
        case noDecoder
    }

    /// La valeur « PyroWave » est dans la liste (avec « Auto ») : la Station
    /// connaît PyroWave, ou il est déjà choisi.
    public var shown: Bool
    /// Inactive : la première raison qui s'applique ; nil : utilisable.
    public var block: Block?
    public var link: PyroWaveLink

    public var usable: Bool { shown && block == nil }

    public static let hidden = PyroWaveChoice(shown: false, block: .notOffered, link: .wifi)

    public init(shown: Bool, block: Block?, link: PyroWaveLink) {
        self.shown = shown
        self.block = block
        self.link = link
    }

    /// `offer` : l'annonce de la Station (nil : d'avant PyroWave, ou pas
    /// encore lue) ; `decodes` : la sonde du lancement ; `chosen` : PyroWave
    /// est le format choisi pour cet ordinateur.
    public static func of(offer: PyroWaveOffer?, decodes: Bool, chosen: Bool, link: PyroWaveLink) -> PyroWaveChoice {
        guard offer != nil || chosen else { return PyroWaveChoice(shown: false, block: .notOffered, link: link) }
        let block: Block? = offer?.available != true ? .notOffered : !decodes ? .noDecoder : nil
        return PyroWaveChoice(shown: true, block: block, link: link)
    }

    /// « Auto » (un profil) prend PyroWave : utilisable, par câble, hors
    /// relais (règle du cœur, `profile::pyrowave_format`).
    public func auto(profile: Bool) -> Bool { profile && usable && link == .cable }
}

/// Débit de PyroWave, comme le cœur (`PYROWAVE_MBPS`, `PYROWAVE_MBPS_444`,
/// calés sur la 4K à 60 Hz) : pour un PyroWave choisi à la main, celui
/// qu'« Auto » prendrait par câble pour la définition de la session.
public enum PyroWaveRate {
    public static let base = 800
    public static let base444 = 1200
    /// Haut du curseur de débit en PyroWave.
    public static let max = 1500

    public static func mbps(width: Int, height: Int, refresh: Int, yuv444: Bool) -> Int {
        let b = Double(yuv444 ? base444 : base)
        guard width > 0, height > 0, refresh > 0 else { return Int(b) }
        let scale = pow(Double(width * height * refresh) / (3840.0 * 2160 * 60), 0.75)
        return Int((b * Swift.min(Swift.max(scale, 0.4), 3)).rounded())
    }
}

/// Le lien réseau de cet appareil (`NWPathMonitor`) : par câble quand le
/// chemin par défaut passe par une interface Ethernet (Apple TV en
/// Ethernet ; iPhone ou iPad avec un adaptateur USB-C). Donné au cœur dans
/// les réglages de la session (`StreamSettings.wired`), pour « Auto ».
public final class NetworkLink: @unchecked Sendable {
    public static let shared = NetworkLink()
    /// Posté (fil principal) quand le lien change.
    public static let changed = Notification.Name("fr.leslignesclaires.kybernow.network-link")

    private let monitor = NWPathMonitor()
    private let lock = NSLock()
    private var current = false

    /// Relié par câble (Ethernet).
    public var wired: Bool {
        lock.lock()
        defer { lock.unlock() }
        return current
    }

    private init() {
        monitor.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            let wired = path.status == .satisfied && path.usesInterfaceType(.wiredEthernet)
            self.lock.lock()
            let changed = wired != self.current
            self.current = wired
            self.lock.unlock()
            if changed {
                KyberLog.video.notice("lien réseau : \(wired ? "Ethernet" : "sans câble", privacy: .public)")
                DispatchQueue.main.async { NotificationCenter.default.post(name: Self.changed, object: nil) }
            }
        }
        monitor.start(queue: DispatchQueue(label: "fr.leslignesclaires.kybernow.network-link"))
    }

    /// À appeler au lancement : le premier chemin arrive en quelques millisecondes.
    public static func start() { _ = shared }
}

/// La session en PyroWave, dans toutes les apps Apple (docs/link-ui.md).
public enum PyroWaveSession {
    /// Le bandeau du début de session disparaît après 8 s (ou à la croix).
    public static let bannerSeconds: TimeInterval = 8
    /// « Passer en HEVC » depuis un PyroWave choisi à la main : débit de
    /// l'image en HEVC pour la fin de la session (au plus), un débit qui tient
    /// en Wi-Fi ; le débit de PyroWave des réglages, lui, reste.
    public static let hevcMbps = 80
}

/// PyroWave, définition ramenée par le cœur pour tenir dans le débit
/// (docs/pyrowave.md, « Définition ramenée ») : `width` × `height` à
/// `refresh` Hz, pour tenir dans `mbps` Mbit/s.
public struct PyroWaveLowered: Sendable, Equatable {
    public let width: Int
    public let height: Int
    public let refresh: Int
    public let mbps: Int

    public init(width: Int, height: Int, refresh: Int, mbps: Int) {
        self.width = width
        self.height = height
        self.refresh = refresh
        self.mbps = mbps
    }
}

/// Le débit que ce Mac sait tenir vers l'ordinateur, donné au cœur pour la
/// définition ramenée de PyroWave (`kn_session_set_link_mbps`) : la vitesse
/// de l'interface du chemin par défaut (`if_data.ifi_baudrate`), 80 % en
/// Ethernet et 50 % sans câble, comme le plafond « Auto » de la Station
/// (docs/pyrowave.md, point 6), arrondi aux 50 Mbit/s du dessous ; nil si
/// l'interface ne dit pas sa vitesse.
public enum LinkRate {
    public static func usableMbps(wired: Bool) -> Int? {
        guard let name = defaultInterface(), let bps = baudrate(of: name), bps > 0 else { return nil }
        let mbps = Double(bps) / 1_000_000 * (wired ? 0.8 : 0.5)
        let rounded = Int(mbps / 50) * 50
        return rounded > 0 ? rounded : nil
    }

    /// Le nom de l'interface du chemin par défaut (`en0`…), lu une fois.
    private static func defaultInterface() -> String? {
        let monitor = NWPathMonitor()
        let sem = DispatchSemaphore(value: 0)
        var name: String?
        monitor.pathUpdateHandler = { path in
            name = path.availableInterfaces.first(where: { path.usesInterfaceType($0.type) })?.name
            sem.signal()
        }
        monitor.start(queue: DispatchQueue(label: "fr.leslignesclaires.kybernow.link-rate"))
        _ = sem.wait(timeout: .now() + 0.5)
        monitor.cancel()
        return name
    }

    private static func baudrate(of name: String) -> UInt64? {
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return nil }
        defer { freeifaddrs(list) }
        for p in sequence(first: first, next: { $0.pointee.ifa_next }) {
            let a = p.pointee
            guard String(cString: a.ifa_name) == name, a.ifa_addr?.pointee.sa_family == UInt8(AF_LINK),
                  let data = a.ifa_data?.assumingMemoryBound(to: if_data.self) else { continue }
            return UInt64(data.pointee.ifi_baudrate)
        }
        return nil
    }
}
