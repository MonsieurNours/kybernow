// Ordinateurs enregistrés, réglages de connexion et Trousseau.

import Foundation
import KyberKit
import Security

enum Profile: String, Codable, CaseIterable, Identifiable {
    case jeu, bureau, max

    var id: String { rawValue }

    var label: String {
        switch self {
        case .jeu: return String(localized: "Jeu")
        case .bureau: return String(localized: "Bureau")
        case .max: return "Max"
        }
    }

    /// Phrase du profil, sous le sélecteur (accueil et réglages ; docs/link-ui.md).
    var hint: String {
        switch self {
        case .jeu: return String(localized: "La réactivité d'abord, pour jouer.")
        case .bureau: return String(localized: "Un texte net, pour travailler.")
        case .max: return String(localized: "La meilleure image que cet appareil affiche.")
        }
    }

    /// Phrase d'un profil, ou de « Personnalisé » (`nil`).
    static func hint(_ profile: Profile?) -> String {
        profile?.hint ?? String(localized: "Vos propres réglages.")
    }

    /// Nom d'un profil, ou « Personnalisé » (`nil`).
    static func label(_ profile: Profile?) -> String {
        profile?.label ?? String(localized: "Personnalisé")
    }
}

enum DisplayChoice: String, Codable, CaseIterable {
    /// Écran virtuel à la taille et à la fréquence de l'écran de ce Mac.
    case match
    /// Écran virtuel à la taille de la fenêtre KyberNow (session fenêtrée).
    case window
    /// Écran virtuel au mode choisi.
    case custom
    /// Pas d'écran virtuel : les écrans de l'ordinateur tels quels.
    case physical
}

enum Transport: String, Codable, CaseIterable {
    case fast, reliable

    var label: String { self == .fast ? String(localized: "Rapide") : String(localized: "Fiable") }

    /// Aide du réglage « Transport de l'image » (docs/link-ui.md).
    static var help: String {
        String(localized: "Rapide convient au réseau local et au jeu. Fiable renvoie ce qui se perd, au prix d'un peu de délai.")
    }
}

/// Souris pendant la session.
enum MouseChoice: String, Codable, CaseIterable {
    /// Mode jeu quand un jeu est en plein écran sur l'ordinateur.
    case auto
    /// Pointeur du Mac, positions absolues : le bureau, les vidéos.
    case desktop
    /// Déplacements bruts, pointeur du Mac caché : viser dans un jeu.
    case game

    var label: String {
        switch self {
        case .auto: return String(localized: "Auto")
        case .desktop: return String(localized: "Souris du bureau")
        case .game: return String(localized: "Souris des jeux")
        }
    }

    var hint: String {
        switch self {
        case .auto: return String(localized: "Souris des jeux dès qu'un jeu est en plein écran sur l'ordinateur et cache son pointeur ; souris du bureau le reste du temps. Les navigateurs et lecteurs vidéo en plein écran gardent la souris du bureau.")
        case .desktop: return String(localized: "Le pointeur du Mac, toujours : bureau, vidéos, jeux à la souris (stratégie, cartes).")
        case .game: return String(localized: "Déplacements bruts de la souris, sans l'accélération de macOS, pointeur caché : pour viser.")
        }
    }

    var streamMode: StreamView.MouseMode {
        switch self {
        case .auto: return .auto
        case .desktop: return .desktop
        case .game: return .game
        }
    }
}

enum AudioChoice: String, Codable, CaseIterable {
    case auto, stereo, surround51, surround71, off

    var label: String {
        switch self {
        case .auto: return String(localized: "Auto")
        case .stereo: return String(localized: "Stéréo")
        case .surround51: return "5.1"
        case .surround71: return "7.1"
        case .off: return String(localized: "Sur l'ordinateur")
        }
    }

    var hint: String {
        switch self {
        case .auto: return String(localized: "Auto : stéréo, quelle que soit la sortie de ce Mac.")
        case .stereo: return String(localized: "Deux canaux : idéal au casque ou sur les haut-parleurs du Mac.")
        case .surround51: return String(localized: "Six canaux. Il faut une sortie 5.1 sur ce Mac (ampli HDMI, barre de son).")
        case .surround71: return String(localized: "Huit canaux. Il faut une sortie 7.1 sur ce Mac.")
        case .off: return String(localized: "Le son reste sur l'ordinateur.")
        }
    }

    var channels: Int {
        switch self {
        case .auto: return AudioOutput.channels()
        case .stereo: return 2
        case .surround51: return 6
        case .surround71: return 8
        case .off: return 0
        }
    }
}

/// Manette vue par l'ordinateur (`gamepad_force_xbox360` de la session, qui
/// prime sur le réglage de la console ; celui-ci ne reste qu'un défaut pour
/// les autres clients).
enum GamepadChoice: String, Codable, CaseIterable {
    /// Toujours une manette Xbox 360 : celle que tous les jeux reconnaissent.
    case xbox360
    /// Le modèle réel : une manette Sony devient une DualShock 4 (pavé
    /// tactile, barre lumineuse, pour les jeux qui s'en servent).
    case faithful

    var label: String {
        switch self {
        case .xbox360: return "Xbox 360"
        case .faithful: return String(localized: "Fidèle")
        }
    }

    var hint: String {
        switch self {
        case .xbox360: return String(localized: "Toutes les manettes, PlayStation comprises, deviennent des manettes Xbox 360 sur l'ordinateur : celles que tous les jeux reconnaissent.")
        case .faithful: return String(localized: "Une manette Sony reste une DualShock 4 sur l'ordinateur (pavé tactile, barre lumineuse pour les jeux qui s'en servent) ; les autres restent des Xbox 360. Certains jeux anciens ne la voient pas.")
        }
    }
}

/// Réglages de connexion d'un ordinateur. Un profil remplit l'image et le
/// réseau ; toucher à l'un de ces réglages rend le profil « Personnalisé ».
struct ConnectionSettings: Codable, Hashable {
    /// `nil` : Personnalisé.
    var profile: Profile? = .jeu
    var display: DisplayChoice = .match
    var customWidth = 2560
    var customHeight = 1440
    var customRefresh = 60
    var codec: VideoCodec = .hevc
    var yuv444 = false
    var bitrateMbps = 60
    var transport: Transport = .fast
    var autoReconnect = true
    var commandAsControl = true
    var audio: AudioChoice = .auto
    /// Synchro verticale : sans elle, chaque image s'affiche dès qu'elle est
    /// décodée, quitte à couper l'écran en deux (déchirure).
    var vsync = true
    /// « Comme cet écran » ramené à la 4K au plus (écrans 5K, 6K).
    var limit4K = false
    /// Souris : Auto (jeu en plein écran), Bureau ou Jeu.
    var mouse: MouseChoice = .auto
    /// Presse-papiers partagé avec l'ordinateur (texte).
    var clipboard = true
    /// Garder la dernière image de la session sur ce Mac (fond de l'accueil).
    var keepLastImage = true
    /// Manette vue par l'ordinateur : Xbox 360 (compatible) ou fidèle.
    var gamepad: GamepadChoice = .xbox360

    static let minBitrate = 5
    /// Le plus que Kyber accepte (débit en bits/s sur 32 bits).
    static let maxBitrate = 4294

    /// Applique un profil, ramené à ce que ce Mac décode en matériel : sans
    /// HEVC, du H.264 ; sans 4:4:4 matériel, du 4:2:0 avec un peu plus de débit.
    mutating func apply(_ profile: Profile) {
        self.profile = profile
        let caps = DecoderCapabilities.current
        codec = caps.hevc ? .hevc : .h264
        switch profile {
        case .jeu: yuv444 = false; bitrateMbps = 60; transport = .fast
        case .bureau: yuv444 = true; bitrateMbps = 90; transport = .reliable
        case .max: yuv444 = true; bitrateMbps = 300; transport = .fast
        }
        if yuv444, !Self.decodes444(codec) {
            yuv444 = false
            bitrateMbps += bitrateMbps / 3
        }
        if codec == .h264 { bitrateMbps += bitrateMbps / 2 }
    }

    /// Le 4:4:4 n'est proposé que si ce Mac le décode en matériel. En AV1,
    /// les décodeurs Apple ne lisent que le profil Main (4:2:0) : un flux
    /// 4:4:4 (profil High) ne s'ouvrirait pas, toutes les images seraient perdues.
    static func decodes444(_ codec: VideoCodec) -> Bool {
        switch codec {
        case .hevc: return DecoderCapabilities.current.hevc444
        case .av1, .h264: return false
        }
    }

    /// Débit des profils, calé sur la 4K à 60 Hz.
    static let referencePixelRate = 3840.0 * 2160 * 60

    /// Débit de la session pour `w` × `h` à `hz` : un profil suit le nombre de
    /// pixels par seconde (moins que proportionnellement : l'encodeur profite
    /// des détails plus fins), un débit réglé à la main reste tel quel.
    func bitrate(width w: Int, height h: Int, refresh hz: Int) -> Int {
        guard profile != nil, w > 0, h > 0, hz > 0 else { return bitrateMbps }
        let scale = pow(Double(w * h * hz) / Self.referencePixelRate, 0.75)
        let mbps = (Double(bitrateMbps) * min(max(scale, 0.4), 3)).rounded()
        return min(max(Int(mbps), Self.minBitrate), Self.maxBitrate)
    }

    var options: StreamOptions { options(width: 0, height: 0, refresh: 0) }

    func options(width: Int, height: Int, refresh: Int) -> StreamOptions {
        var o = StreamOptions()
        o.codec = codec
        o.yuv444 = yuv444
        o.bitrate = UInt32(bitrate(width: width, height: height, refresh: refresh)) * 1_000_000
        o.reliable = transport == .reliable
        o.audio = audio != .off
        o.yuv444 = yuv444 && Self.decodes444(codec)
        o.vsync = vsync
        o.clipboard = clipboard
        o.gamepadAsXbox360 = gamepad == .xbox360
        return o
    }

    var profileLabel: String { Profile.label(profile) }

    /// Profil dit à la console (activité, fil des événements) : toujours le
    /// libellé français, quelle que soit la langue de l'interface.
    var consoleProfile: String {
        switch profile {
        case .jeu?: return "Jeu"
        case .bureau?: return "Bureau"
        case .max?: return "Max"
        case nil: return "Personnalisé"
        }
    }

    /// Si le mode de l'écran de l'ordinateur change en session (un jeu qui
    /// passe en 1080p) : Jeu laisse faire, Bureau le fait remettre ; sinon
    /// (`default`), la console décide (son réglage « Si un jeu change la
    /// définition ») : dit explicitement, pour qu'un profil changé en session
    /// défasse le choix du précédent.
    var modePolicy: String {
        switch profile {
        case .jeu?: return "follow"
        case .bureau?: return "restore"
        default: return "default"
        }
    }
}

extension ConnectionSettings {
    // Lecture tolérante : un réglage ajouté par une version récente garde
    // sa valeur par défaut au lieu de faire perdre les ordinateurs enregistrés.
    private enum Keys: String, CodingKey {
        case profile, display, customWidth, customHeight, customRefresh, codec, yuv444, bitrateMbps, transport
        case autoReconnect, commandAsControl, audio, vsync, limit4K, mouse, clipboard, keepLastImage, gamepad
    }

    init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: Keys.self)
        profile = try? c.decodeIfPresent(Profile.self, forKey: .profile)
        display = (try? c.decodeIfPresent(DisplayChoice.self, forKey: .display)) ?? display
        customWidth = (try? c.decodeIfPresent(Int.self, forKey: .customWidth)) ?? customWidth
        customHeight = (try? c.decodeIfPresent(Int.self, forKey: .customHeight)) ?? customHeight
        customRefresh = (try? c.decodeIfPresent(Int.self, forKey: .customRefresh)) ?? customRefresh
        codec = (try? c.decodeIfPresent(VideoCodec.self, forKey: .codec)) ?? codec
        yuv444 = (try? c.decodeIfPresent(Bool.self, forKey: .yuv444)) ?? yuv444
        bitrateMbps = (try? c.decodeIfPresent(Int.self, forKey: .bitrateMbps)) ?? bitrateMbps
        transport = (try? c.decodeIfPresent(Transport.self, forKey: .transport)) ?? transport
        autoReconnect = (try? c.decodeIfPresent(Bool.self, forKey: .autoReconnect)) ?? autoReconnect
        commandAsControl = (try? c.decodeIfPresent(Bool.self, forKey: .commandAsControl)) ?? commandAsControl
        audio = (try? c.decodeIfPresent(AudioChoice.self, forKey: .audio)) ?? audio
        vsync = (try? c.decodeIfPresent(Bool.self, forKey: .vsync)) ?? vsync
        limit4K = (try? c.decodeIfPresent(Bool.self, forKey: .limit4K)) ?? limit4K
        mouse = (try? c.decodeIfPresent(MouseChoice.self, forKey: .mouse)) ?? mouse
        clipboard = (try? c.decodeIfPresent(Bool.self, forKey: .clipboard)) ?? clipboard
        keepLastImage = (try? c.decodeIfPresent(Bool.self, forKey: .keepLastImage)) ?? keepLastImage
        gamepad = (try? c.decodeIfPresent(GamepadChoice.self, forKey: .gamepad)) ?? gamepad
    }
}

struct Host: Codable, Identifiable, Hashable {
    var id = UUID()
    var name: String
    var address: String
    var consolePort: UInt16
    var username: String
    /// Port du flux Kyber, appris auprès de la console.
    var streamPort: UInt16?
    /// Empreinte du certificat de la console, retenue au premier contact.
    var consoleFingerprint: String?
    var settings = ConnectionSettings()
    /// Début de la dernière session (l'ordre de l'accueil, « jamais ouvert »).
    var lastSession: Date?
    /// Dernière session d'au moins 3 s (rubrique Statistiques) : latence
    /// médiane et 95e centile en millisecondes, durée.
    var lastLatencyMs: Double?
    var lastLatencyP95Ms: Double?
    var lastDuration: TimeInterval?
    /// Ce qu'elle a transmis : définition, fréquence, format, débit en Mbit/s.
    var lastWidth: Int?
    var lastHeight: Int?
    var lastRefresh: Int?
    var lastCodec: VideoCodec?
    var lastMbps: Int?
    /// Ajouté par code à six chiffres : un compte propre à cet appareil,
    /// créé par Admin à l'accord.
    var pairedByCode: Bool?
    /// Cartes réseau de l'ordinateur, pour le réveiller par le réseau :
    /// apprises d'Admin à chaque info (gardées si la nouvelle liste est vide).
    var wake: [WakeTarget]?

    init(name: String, address: String, consolePort: UInt16, username: String) {
        self.name = name
        self.address = address
        self.consolePort = consolePort
        self.username = username
    }

    /// Fin d'une session commencée à `start` (l'image arrivée) : une session
    /// de moins de 3 s ne remplace pas la précédente dans les Statistiques.
    mutating func recordSession(start: Date, latency: (median: Double, p95: Double)?, width: Int, height: Int, refresh: Int, codec: VideoCodec, mbps: Int) {
        let duration = Date().timeIntervalSince(start)
        guard duration >= 3 else { return }
        lastSession = start
        lastDuration = duration
        lastLatencyMs = latency?.median
        lastLatencyP95Ms = latency?.p95
        lastWidth = width > 0 ? width : nil
        lastHeight = height > 0 ? height : nil
        lastRefresh = refresh > 0 ? refresh : nil
        lastCodec = codec
        lastMbps = mbps > 0 ? mbps : nil
    }

    // Lecture tolérante : les ordinateurs enregistrés par les versions
    // précédentes n'avaient qu'un profil, ni le détail de la dernière session.
    private enum CodingKeys: String, CodingKey {
        case id, name, address, consolePort, username, streamPort, consoleFingerprint, settings, lastSession, lastLatencyMs, lastLatencyP95Ms, lastDuration, profile
        case lastWidth, lastHeight, lastRefresh, lastCodec, lastMbps, pairedByCode, wake
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UUID.self, forKey: .id)
        name = try c.decode(String.self, forKey: .name)
        address = try c.decode(String.self, forKey: .address)
        consolePort = try c.decode(UInt16.self, forKey: .consolePort)
        username = try c.decode(String.self, forKey: .username)
        streamPort = try c.decodeIfPresent(UInt16.self, forKey: .streamPort)
        consoleFingerprint = try c.decodeIfPresent(String.self, forKey: .consoleFingerprint)
        lastSession = try c.decodeIfPresent(Date.self, forKey: .lastSession)
        lastLatencyMs = try c.decodeIfPresent(Double.self, forKey: .lastLatencyMs)
        lastLatencyP95Ms = try c.decodeIfPresent(Double.self, forKey: .lastLatencyP95Ms)
        lastDuration = try c.decodeIfPresent(TimeInterval.self, forKey: .lastDuration)
        lastWidth = try? c.decodeIfPresent(Int.self, forKey: .lastWidth)
        lastHeight = try? c.decodeIfPresent(Int.self, forKey: .lastHeight)
        lastRefresh = try? c.decodeIfPresent(Int.self, forKey: .lastRefresh)
        lastCodec = try? c.decodeIfPresent(VideoCodec.self, forKey: .lastCodec)
        lastMbps = try? c.decodeIfPresent(Int.self, forKey: .lastMbps)
        pairedByCode = try? c.decodeIfPresent(Bool.self, forKey: .pairedByCode)
        wake = try? c.decodeIfPresent([WakeTarget].self, forKey: .wake)
        if let s = try c.decodeIfPresent(ConnectionSettings.self, forKey: .settings) {
            settings = s
        } else if let p = try c.decodeIfPresent(Profile.self, forKey: .profile) {
            settings.apply(p)
        }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(name, forKey: .name)
        try c.encode(address, forKey: .address)
        try c.encode(consolePort, forKey: .consolePort)
        try c.encode(username, forKey: .username)
        try c.encodeIfPresent(streamPort, forKey: .streamPort)
        try c.encodeIfPresent(consoleFingerprint, forKey: .consoleFingerprint)
        try c.encode(settings, forKey: .settings)
        try c.encodeIfPresent(lastSession, forKey: .lastSession)
        try c.encodeIfPresent(lastLatencyMs, forKey: .lastLatencyMs)
        try c.encodeIfPresent(lastLatencyP95Ms, forKey: .lastLatencyP95Ms)
        try c.encodeIfPresent(lastDuration, forKey: .lastDuration)
        try c.encodeIfPresent(lastWidth, forKey: .lastWidth)
        try c.encodeIfPresent(lastHeight, forKey: .lastHeight)
        try c.encodeIfPresent(lastRefresh, forKey: .lastRefresh)
        try c.encodeIfPresent(lastCodec, forKey: .lastCodec)
        try c.encodeIfPresent(lastMbps, forKey: .lastMbps)
        try c.encodeIfPresent(pairedByCode, forKey: .pairedByCode)
        try c.encodeIfPresent(wake, forKey: .wake)
    }
}

/// Ce qui empêche d'entrer dans un ordinateur qui répond pourtant.
enum HostProblem: Equatable {
    /// Admin refuse le compte (mot de passe changé ou compte retiré).
    case passwordRefused
    /// Le certificat d'Admin n'est plus celui retenu (Station réinstallée).
    case certificateChanged
}

/// État d'un ordinateur vu depuis cet appareil, rafraîchi en continu.
struct HostStatus: Equatable {
    var online: Bool
    var info: HostInfo?
    var problem: HostProblem? = nil
}

@MainActor
final class HostStore: ObservableObject {
    @Published private(set) var hosts: [Host] = []
    @Published private(set) var status: [UUID: HostStatus] = [:]
    /// Redémarrage ou mise à jour de la Station en cours, par ordinateur
    /// (rubrique « Station », LinkUI.swift).
    @Published var stationTasks: [UUID: StationTask] = [:]
    /// Un appareil est en session : « Redémarrer quand même » à confirmer.
    @Published var stationConfirm: StationConfirm?
    /// Réveil par le réseau en cours (« Réveil de l'ordinateur… »), par ordinateur.
    @Published private(set) var waking: Set<UUID> = []
    /// Image de chaque ordinateur sur l'accueil.
    let imagery = HostImagery()
    private let key = "hosts"
    private var polling: Task<Void, Never>?
    /// Ordinateurs de démonstration (`-demo YES`, en Debug) : leurs propres
    /// préférences, mots de passe en mémoire, états posés par l'app, aucun
    /// appel réseau.
    let demo: Bool
    private let defaults: UserDefaults
    private var demoPasswords: [UUID: String] = [:]
    /// Appelé après chaque ajout, modification ou retrait : l'app Mac publie
    /// ses ordinateurs dans iCloud pour l'Apple TV (CloudPublisher).
    var didSave: (([Host]) -> Void)?

    init(demo: Bool = false) {
        self.demo = demo
        defaults = demo ? (UserDefaults(suiteName: "fr.leslignesclaires.kybernow.demo") ?? .standard) : .standard
        if let data = defaults.data(forKey: key),
           let saved = try? JSONDecoder().decode([Host].self, from: data) {
            hosts = saved
        }
        if !demo { startPolling() }
    }

    /// Le plus récemment utilisé d'abord.
    var ordered: [Host] {
        hosts.sorted { ($0.lastSession ?? .distantPast) > ($1.lastSession ?? .distantPast) }
    }

    private func save() {
        if let data = try? JSONEncoder().encode(hosts) {
            defaults.set(data, forKey: key)
        }
        didSave?(hosts)
    }

    func add(_ host: Host, password: String) {
        hosts.append(host)
        setPassword(password, for: host.id)
        save()
        Task { await refresh(host) }
    }

    func update(_ host: Host) {
        guard let i = hosts.firstIndex(where: { $0.id == host.id }) else { return }
        hosts[i] = host
        save()
    }

    func host(_ id: UUID) -> Host? {
        hosts.first { $0.id == id }
    }

    func remove(_ host: Host) {
        imagery.forget(host)
        hosts.removeAll { $0.id == host.id }
        status[host.id] = nil
        stationTasks[host.id] = nil
        if demo { demoPasswords[host.id] = nil } else { Keychain.delete(host.id) }
        save()
    }

    func password(for host: Host) -> String? {
        demo ? demoPasswords[host.id] : Keychain.get(host.id)
    }

    private func setPassword(_ password: String, for id: UUID) {
        if demo { demoPasswords[id] = password } else { Keychain.set(password, for: id) }
    }

    /// Nouveau mot de passe (rubrique Station, « Mot de passe refusé ») :
    /// gardé, puis vérifié tout de suite auprès d'Admin.
    func updatePassword(_ password: String, for host: Host) {
        setPassword(password, for: host.id)
        Task { await refresh(host) }
    }

    /// Nouvelle empreinte acceptée (Station réinstallée, empreinte comparée
    /// à celle d'Admin) : retenue à la place de l'ancienne.
    func trust(fingerprint: String, for host: Host) {
        guard var h = self.host(host.id) else { return }
        h.consoleFingerprint = fingerprint
        update(h)
        Task { await refresh(h) }
    }

    /// Empreinte que présente Admin en ce moment, sans la comparer à celle retenue.
    func presentedFingerprint(of host: Host) async -> String? {
        #if DEBUG
        // Démonstration : une Station « réinstallée », pour l'écran de l'empreinte.
        if demo { return "sha256:c41be0f2d79a3e58b6014c7dd2e95a1f03b8c6e4927d5a0f1e3b6c8d9a2f4e71" }
        #endif
        guard !demo else { return nil }
        let probe = ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: password(for: host) ?? "", fingerprint: nil)
        return await probe.probeFingerprint()
    }

    // MARK: - En ligne ou pas

    private func startPolling() {
        polling = Task { [weak self] in
            while !Task.isCancelled {
                if let self {
                    await withTaskGroup(of: Void.self) { group in
                        for host in await self.hosts {
                            group.addTask { await self.refresh(host) }
                        }
                    }
                }
                try? await Task.sleep(for: .seconds(8))
            }
        }
    }

    func console(for host: Host) -> ConsoleClient? {
        guard !demo, let password = password(for: host) else { return nil }
        return ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: password, fingerprint: host.consoleFingerprint)
    }

    func refresh(_ host: Host) async {
        guard let console = console(for: host) else { return }
        do {
            let info = try await console.info()
            status[host.id] = HostStatus(online: true, info: info)
            // Les cartes réseau pour le réveil : une liste vide (Station
            // plus ancienne, carte absente) garde la précédente.
            if !info.wake.isEmpty, var h = self.host(host.id), h.wake != info.wake {
                h.wake = info.wake
                update(h)
            }
            // Jamais ouvert depuis cet appareil (ou image non gardée) : un aperçu à jour.
            if imagery.wantsPreview(host) { await imagery.fetchPreview(for: host, console: console) }
            if host.consoleFingerprint == nil, let fp = console.fingerprint, var h = self.host(host.id) {
                h.consoleFingerprint = fp
                update(h)
            }
        } catch ConsoleError.http(404, _) {
            // Console d'une version précédente, sans /api/stream/info : elle répond, donc en ligne.
            status[host.id] = HostStatus(online: true, info: nil)
        } catch ConsoleError.http(401, _) {
            // Elle répond, mais refuse le compte.
            status[host.id] = HostStatus(online: true, info: status[host.id]?.info, problem: .passwordRefused)
        } catch ConsoleError.certificateChanged {
            status[host.id] = HostStatus(online: true, info: status[host.id]?.info, problem: .certificateChanged)
        } catch {
            status[host.id] = HostStatus(online: false, info: status[host.id]?.info)
        }
    }

    // MARK: - Réveil par le réseau

    /// L'app connaît une carte réseau de l'ordinateur : « Réveiller l'ordinateur ».
    func canWake(_ host: Host) -> Bool {
        !(self.host(host.id)?.wake ?? host.wake ?? []).isEmpty
    }

    /// « Réveiller l'ordinateur » : le paquet part, puis l'ordinateur est
    /// sondé toutes les 2 s, 2 min au plus, jusqu'à ce qu'il soit en ligne.
    func wake(_ host: Host) {
        guard !waking.contains(host.id), let targets = self.host(host.id)?.wake, !targets.isEmpty else { return }
        waking.insert(host.id)
        Task {
            if !demo {
                // Un envoi parti suffit (la diffusion peut être refusée sur iPhone, iPad, Apple TV).
                _ = await Wake.send(targets)
            }
            let start = Date()
            while Date().timeIntervalSince(start) < 120, self.host(host.id) != nil {
                try? await Task.sleep(for: .seconds(2))
                #if DEBUG
                if demo, Date().timeIntervalSince(start) > 8, let info = status[host.id]?.info {
                    setDemoStatus(HostStatus(online: true, info: info), for: host.id)
                }
                #endif
                await refresh(host)
                if status[host.id]?.online == true { break }
            }
            waking.remove(host.id)
        }
    }

    #if DEBUG
    /// Démonstration : l'état d'un ordinateur, posé par l'app.
    func setDemoStatus(_ s: HostStatus?, for id: UUID) {
        status[id] = s
    }
    #endif
}

enum Keychain {
    private static let service = "fr.leslignesclaires.kybernow"

    private static func query(_ id: UUID) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: id.uuidString]
    }

    static func set(_ password: String, for id: UUID) {
        SecItemDelete(query(id) as CFDictionary)
        var item = query(id)
        item[kSecValueData as String] = Data(password.utf8)
        item[kSecAttrLabel as String] = "KyberNow"
        SecItemAdd(item as CFDictionary, nil)
    }

    static func get(_ id: UUID) -> String? {
        var q = query(id)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(_ id: UUID) {
        SecItemDelete(query(id) as CFDictionary)
    }
}
