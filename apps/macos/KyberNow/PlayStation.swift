// Consoles PlayStation (docs/playstation.md ; textes : docs/link-ui.md,
// « Consoles PlayStation ») : les consoles associées, gardées comme les
// ordinateurs (préférences ; l'association, qui porte les clés, et
// l'identifiant du compte PSN au Trousseau), et leur recherche sur le
// réseau tant que l'accueil est à l'écran.

import KyberKit
import SwiftUI

/// Une console associée à ce Mac.
struct PlayStationConsole: Codable, Identifiable, Hashable {
    var id = UUID()
    var name: String
    /// Identifiant de la console (le `mac` de l'association).
    var hostID: String
    /// « PS5 » ou « PS4 ».
    var model: String
    /// Dernière adresse connue (la recherche la tient à jour).
    var address: String
    /// Pour associer de nouveau (`kn_ps_regist`).
    var target: String
    /// La puce « HDR » de l'accueil (allumée la première fois).
    var hdr = true
    var lastSession: Date?
    /// Réglages de la console (JSON du cœur, `PSSettings` ; nil : par défaut).
    var settings: String?
    /// Disposition du clavier et de la souris (JSON du cœur, `PSLayout` ; nil : par défaut).
    var layout: String?

    var hasHDR: Bool { model == "PS5" }
}

// MARK: - Réglages d'une console

/// Réglages d'une console (`apps/core/src/playstation/settings.rs`, JSON
/// stable) : définition, images par seconde, débit, format, plein écran. Le
/// HDR vient de la puce de l'accueil au lancement de la partie.
struct PSSettings: Codable, Equatable {
    var resolution = "auto"
    var fps = 60
    /// kbit/s ; nil : Auto (suit la définition).
    var bitrateKbps: Int?
    var codec = "auto"
    var hdr = false
    var fullscreen = true
    /// Image agrandie aux pixels de l'écran (MetalFX), allumée par défaut.
    var upscale = true
    /// Netteté après l'agrandissement, de 0 (coupée) à 100.
    var sharpness = PSSettings.defaultSharpness

    static let defaultSharpness = 40

    /// Ce que le rendu reçoit (nil : filtre bilinéaire).
    var upscaleSettings: UpscaleSettings? { upscale ? UpscaleSettings(sharpness: Float(sharpness) / 100) : nil }

    private enum CodingKeys: String, CodingKey {
        case resolution, fps, codec, hdr, fullscreen, upscale, sharpness
        case bitrateKbps = "bitrate_kbps"
    }

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        resolution = (try? c.decodeIfPresent(String.self, forKey: .resolution)) ?? "auto"
        fps = (try? c.decodeIfPresent(Int.self, forKey: .fps)) ?? 60
        bitrateKbps = try? c.decodeIfPresent(Int.self, forKey: .bitrateKbps)
        codec = (try? c.decodeIfPresent(String.self, forKey: .codec)) ?? "auto"
        hdr = (try? c.decodeIfPresent(Bool.self, forKey: .hdr)) ?? false
        fullscreen = (try? c.decodeIfPresent(Bool.self, forKey: .fullscreen)) ?? true
        upscale = (try? c.decodeIfPresent(Bool.self, forKey: .upscale)) ?? true
        sharpness = min(100, max(0, (try? c.decodeIfPresent(Int.self, forKey: .sharpness)) ?? Self.defaultSharpness))
    }

    /// Définitions, dans l'ordre des réglages.
    static let resolutions = ["auto", "p1080", "p720", "p540", "p360"]
    static let codecs = ["auto", "h264", "h265"]
    /// Débit choisi à la main, en Mbit/s.
    static let bitrateRange = 2...100

    static func resolutionLabel(_ r: String) -> String {
        switch r {
        case "p1080": return "1080p"
        case "p720": return "720p"
        case "p540": return "540p"
        case "p360": return "360p"
        default: return PSText.auto
        }
    }

    /// Débit d'Auto pour une définition, en kbit/s (celui du cœur).
    static func autoBitrate(_ r: String) -> Int {
        switch r {
        case "p720": return 10_000
        case "p540": return 6_000
        case "p360": return 2_000
        default: return 15_000
        }
    }
}

/// Ce qui déclenche une action : une touche (usage HID) ou un bouton de la
/// souris (0 gauche, 1 droit, 2 milieu, 3 et 4 latéraux).
enum PSInput: Hashable, Codable {
    case key(UInt16)
    case mouse(UInt8)

    private enum CodingKeys: String, CodingKey { case key, mouse }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        if let k = try c.decodeIfPresent(UInt16.self, forKey: .key) { self = .key(k); return }
        self = .mouse(try c.decode(UInt8.self, forKey: .mouse))
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .key(k): try c.encode(k, forKey: .key)
        case let .mouse(b): try c.encode(b, forKey: .mouse)
        }
    }
}

/// Un lien de la disposition : `[input, action]` en JSON.
struct PSBinding: Codable, Equatable {
    var input: PSInput
    var action: String

    init(input: PSInput, action: String) { (self.input, self.action) = (input, action) }

    init(from decoder: Decoder) throws {
        var c = try decoder.unkeyedContainer()
        input = try c.decode(PSInput.self)
        action = try c.decode(String.self)
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.unkeyedContainer()
        try c.encode(input)
        try c.encode(action)
    }
}

/// Disposition du clavier et de la souris d'une console
/// (`apps/core/src/playstation/keymouse.rs`, `Layout`).
struct PSLayout: Codable, Equatable {
    var bindings: [PSBinding] = []
    var mouseStick = true
    var sensitivity = 1.0
    var curve = 0.8
    var deadzone = 0.2
    var invertY = false

    private enum CodingKeys: String, CodingKey {
        case bindings, sensitivity, curve, deadzone
        case mouseStick = "mouse_stick"
        case invertY = "invert_y"
    }

    init() {}

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        bindings = try c.decode([PSBinding].self, forKey: .bindings)
        mouseStick = (try? c.decodeIfPresent(Bool.self, forKey: .mouseStick)) ?? true
        sensitivity = (try? c.decodeIfPresent(Double.self, forKey: .sensitivity)) ?? 1
        curve = (try? c.decodeIfPresent(Double.self, forKey: .curve)) ?? 0.8
        deadzone = (try? c.decodeIfPresent(Double.self, forKey: .deadzone)) ?? 0.2
        invertY = (try? c.decodeIfPresent(Bool.self, forKey: .invertY)) ?? false
    }

    /// Les actions de la manette, dans l'ordre du cœur (`Action::ALL`).
    static let actions = [
        "left_stick_up", "left_stick_down", "left_stick_left", "left_stick_right",
        "cross", "circle", "square", "triangle", "l1", "r1", "l2", "r2", "l3", "r3",
        "dpad_up", "dpad_down", "dpad_left", "dpad_right",
        "options", "create", "ps", "touchpad",
        "right_stick_up", "right_stick_down", "right_stick_left", "right_stick_right",
    ]

    /// « Réactivité au départ » : exposants de la courbe (Douce, Normale, Vive).
    static let curves = [1.2, 1.0, 0.8]

    func inputs(_ action: String) -> [PSInput] {
        bindings.filter { $0.action == action }.map(\.input)
    }

    /// Lie `input` à `action` (à la place de `replacing`, sinon en plus) ;
    /// une touche déjà prise ailleurs y est retirée.
    mutating func bind(_ input: PSInput, to action: String, replacing: PSInput?) {
        let at = replacing.flatMap { r in bindings.firstIndex { $0.input == r && $0.action == action } }
        var list = bindings
        if let at { list[at].input = input } else { list.append(PSBinding(input: input, action: action)) }
        var kept: [PSBinding] = []
        for (i, b) in list.enumerated() {
            let mine = i == (at ?? list.count - 1)
            if b.input == input, !mine { continue }
            kept.append(b)
        }
        bindings = kept
    }

    mutating func unbind(_ input: PSInput, from action: String) {
        bindings.removeAll { $0.input == input && $0.action == action }
    }
}

/// Textes des consoles (docs/link-ui.md, « Consoles PlayStation »).
enum PSText {
    static var on: String { String(localized: "Allumée") }
    static var standby: String { String(localized: "En veille") }
    static var play: String { String(localized: "Jouer") }
    static var wakeAndPlay: String { String(localized: "Réveiller et jouer") }
    static var waking: String { String(localized: "Réveil de la console…") }
    static var pair: String { String(localized: "Associer") }

    // Réglages de la console
    static var auto: String { String(localized: "Auto") }
    static var custom: String { String(localized: "Personnalisé") }
    static var resolution: String { String(localized: "Définition") }
    static var resolutionAutoHelp: String { String(localized: "La plus haute que la console accepte : 1080p.") }
    static var frameRate: String { String(localized: "Images par seconde") }
    static func fps(_ n: Int) -> String { String(localized: "\(n) i/s") }
    static var bitrate: String { String(localized: "Débit") }
    static var bitrateAutoHelp: String { String(localized: "Suit la définition.") }
    static func mbps(_ n: Int) -> String { String(localized: "\(n) Mbit/s") }
    static var format: String { String(localized: "Format") }
    static var formatAutoHelp: String { String(localized: "H.265 si cet appareil le lit.") }
    static var formatH264Help: String { String(localized: "Sans HDR.") }
    static var formatPS4Help: String { String(localized: "La PS4 n'envoie que du H.264.") }
    static var fullScreen: String { String(localized: "Ouvrir la partie en plein écran") }
    static var upscale: String { String(localized: "Agrandir l'image à la taille de l'écran") }
    static var upscaleHelp: String { String(localized: "La console envoie du 1080p au plus : l'image est agrandie aux pixels de votre écran par la puce graphique du Mac. Coupé de lui-même si ce Mac est trop lent.") }
    static var sharpness: String { String(localized: "Netteté") }
    static func percent(_ n: Int) -> String { String(localized: "\(n) %") }

    // Barre de la partie
    static var mute: String { String(localized: "Couper le son") }
    static var unmute: String { String(localized: "Rétablir le son") }
    static var exitFullScreen: String { String(localized: "Quitter le plein écran") }
    static var takeMouse: String { String(localized: "Prendre la souris") }
    static var giveMouse: String { String(localized: "Rendre la souris") }
    static var padAway: String { String(localized: "La console ne la reçoit plus") }
    static var padBack: String { String(localized: "revenir à la console") }

    /// « 1920 × 1080 · 60 i/s · H.265 · HDR · 15 Mbit/s demandés ».
    static func statsAsked(_ s: PlayStationPlay.Stats, fps: Int) -> String {
        var parts = ["\(s.width) × \(s.height)", Self.fps(fps), s.hevc ? "H.265" : "H.264"]
        if s.hdr { parts.append("HDR") }
        parts.append(String(localized: "\(Int((Double(s.targetKbps) / 1000).rounded())) Mbit/s demandés"))
        return parts.joined(separator: " · ")
    }

    /// « Reçu : 59 i/s · 12,4 Mbit/s ».
    static func statsReceived(_ s: PlayStationPlay.Stats) -> String {
        let mbps = String(format: "%.1f", s.mbps).replacingOccurrences(of: ".", with: LanguageChoice.effective == "fr" ? "," : ".")
        return String(localized: "Reçu : \(Int(s.fps.rounded())) i/s · \(mbps) Mbit/s")
    }

    /// « Images perdues : 3 · réparées : 2 » (depuis le début de la partie).
    static func statsLosses(lost: Int, repaired: Int) -> String {
        String(localized: "Images perdues : \(lost) · réparées : \(repaired)")
    }
    static var controlsHelp: String { String(localized: "Le clavier et la souris tiennent lieu de manette. Une manette branchée les remplace.") }
    static var none: String { String(localized: "Aucune") }
    static var pressKey: String { String(localized: "Appuyez sur une touche ou cliquez…") }
    static var mouseStick: String { String(localized: "La souris tient le stick droit") }
    static var sensitivity: String { String(localized: "Sensibilité") }
    static var startingResponse: String { String(localized: "Réactivité au départ") }
    static var curves: [String] { [String(localized: "Douce"), String(localized: "Normale"), String(localized: "Vive")] }
    static var minimumTilt: String { String(localized: "Inclinaison minimale") }
    static var minimumTiltHelp: String { String(localized: "Pour passer la zone morte des jeux.") }
    static var invertY: String { String(localized: "Inverser l'axe vertical") }
    static var defaultLayout: String { String(localized: "Disposition par défaut") }

    /// Une action de la manette.
    static func action(_ a: String) -> String {
        switch a {
        case "left_stick_up": return String(localized: "Stick gauche vers le haut")
        case "left_stick_down": return String(localized: "Stick gauche vers le bas")
        case "left_stick_left": return String(localized: "Stick gauche vers la gauche")
        case "left_stick_right": return String(localized: "Stick gauche vers la droite")
        case "cross": return String(localized: "Croix")
        case "circle": return String(localized: "Rond")
        case "square": return String(localized: "Carré")
        case "triangle": return String(localized: "Triangle")
        case "dpad_up": return String(localized: "Croix directionnelle vers le haut")
        case "dpad_down": return String(localized: "Croix directionnelle vers le bas")
        case "dpad_left": return String(localized: "Croix directionnelle vers la gauche")
        case "dpad_right": return String(localized: "Croix directionnelle vers la droite")
        case "options": return String(localized: "Options")
        case "create": return String(localized: "Create")
        case "ps": return String(localized: "Bouton PS")
        case "touchpad": return String(localized: "Pavé tactile")
        case "right_stick_up": return String(localized: "Stick droit vers le haut")
        case "right_stick_down": return String(localized: "Stick droit vers le bas")
        case "right_stick_left": return String(localized: "Stick droit vers la gauche")
        case "right_stick_right": return String(localized: "Stick droit vers la droite")
        default: return a.uppercased()  // L1, R1, L2, R2, L3, R3
        }
    }

    /// Une touche ou un bouton de la souris : la lettre du clavier du Mac,
    /// le symbole d'une touche spéciale, ou le nom du bouton.
    static func input(_ i: PSInput) -> String {
        switch i {
        case let .mouse(b):
            switch b {
            case 0: return String(localized: "Clic gauche")
            case 1: return String(localized: "Clic droit")
            case 2: return String(localized: "Clic du milieu")
            default: return String(localized: "Bouton \(Int(b) + 1)")
            }
        case let .key(k):
            return key(k)
        }
    }

    private static func key(_ k: UInt16) -> String {
        switch k {
        case 0x28: return "↩"
        case 0x29: return "⎋"
        case 0x2A: return "⌫"
        case 0x2B: return "⇥"
        case 0x2C: return String(localized: "Espace")
        case 0x39: return "⇪"
        case 0x3A...0x45: return "F\(k - 0x3A + 1)"
        case 0x68...0x6F: return "F\(k - 0x68 + 13)"
        case 0x4A: return "↖"
        case 0x4B: return "⇞"
        case 0x4C: return "⌦"
        case 0x4D: return "↘"
        case 0x4E: return "⇟"
        case 0x4F: return "→"
        case 0x50: return "←"
        case 0x51: return "↓"
        case 0x52: return "↑"
        case 0x53: return "⌧"
        case 0x58: return "⌤"
        case 0xE0: return String(localized: "\("⌃") gauche")
        case 0xE1: return String(localized: "\("⇧") gauche")
        case 0xE2: return String(localized: "\("⌥") gauche")
        case 0xE3: return String(localized: "\("⌘") gauche")
        case 0xE4: return String(localized: "\("⌃") droite")
        case 0xE5: return String(localized: "\("⇧") droite")
        case 0xE6: return String(localized: "\("⌥") droite")
        case 0xE7: return String(localized: "\("⌘") droite")
        default:
            let c = Keymap.character(hid: k)
            // Pavé numérique : 0x54 à 0x63 et 0x67.
            if let c, (0x54...0x63).contains(k) || k == 0x67 { return String(localized: "Pavé \(c)") }
            return c ?? String(localized: "Touche \(Int(k))")
        }
    }

    // Clavier et souris
    static var clickToPlay: String { String(localized: "Cliquez dans l'image pour jouer au clavier et à la souris") }
    static var releaseMouse: String { String(localized: "⌃⌥⌘M pour rendre la souris") }

    // Association
    static var signInHelp: String { String(localized: "Connectez-vous à votre compte PlayStation Network. KyberNow lit seulement l'identifiant de votre compte, une seule fois.") }
    static var signIn: String { String(localized: "Se connecter à PlayStation Network") }
    static var byHand: String { String(localized: "Saisir l'identifiant à la main") }
    static var accountField: String { String(localized: "Identifiant du compte") }
    static var accountUnread: String { String(localized: "Identifiant du compte non lu : réessayez, ou saisissez-le à la main.") }
    static var codeHelp: String { String(localized: "Sur la console : Réglages › Système › Lecture à distance › Associer un appareil. Saisissez le code affiché.") }
    static var codeField: String { String(localized: "Code à 8 chiffres") }
    static var noAnswer: String { String(localized: "La console ne répond pas : gardez l'écran « Associer un appareil » ouvert.") }
    static var wrongCode: String { String(localized: "Code refusé : il change toutes les 5 minutes.") }
    static var accountRefused: String { String(localized: "La console refuse ce compte.") }
    static var unreachable: String { String(localized: "Console injoignable.") }

    static func regist(_ e: PlayStation.RegistError) -> String {
        switch e {
        case .noAnswer: return noAnswer
        case .wrongCode: return wrongCode
        case .account: return accountRefused
        case .network, .other: return unreachable
        }
    }

    // Session
    static var connecting: String { String(localized: "Connexion à la console…") }
    static var cantDisplay: String { String(localized: "La console ne peut pas diffuser ce qu'elle affiche.") }
    static var rest: String { String(localized: "Mettre la console en veille") }

    /// Fin d'une partie qui n'a pas été voulue par ce Mac (nil : fermée ici).
    static func end(_ end: PlayStationPlay.End) -> String? {
        switch end {
        case .stopped: return nil
        case .inUse: return String(localized: "La console est déjà jouée à distance par un autre appareil.")
        case .notPaired: return String(localized: "La console ne reconnaît plus cet appareil : associez-la de nouveau.")
        case .unreachable: return String(localized: "La console ne répond pas.")
        case .remote: return String(localized: "La console a terminé la partie.")
        }
    }
}

/// La console à associer (feuille de l'association).
struct PairTarget: Identifiable {
    var found: PlayStation.Found
    var id: String { found.hostID }

    /// Une console gardée qu'il faut associer de nouveau.
    init(console c: PlayStationConsole, live: PlayStation.Found?) {
        found = live ?? PlayStation.Found(address: c.address, hostID: c.hostID, name: c.name, model: c.model, state: "unknown", target: c.target, runningApp: nil)
    }

    init(found: PlayStation.Found) { self.found = found }
}

@MainActor
final class PlayStationStore: ObservableObject {
    /// État d'une console vue d'ici.
    enum Status: Equatable {
        case searching, on, standby, offline

        var online: Bool? {
            switch self {
            case .searching: return nil
            case .on: return true
            case .standby, .offline: return false
            }
        }

        var text: String {
            switch self {
            case .searching: return LinkText.status(nil)
            case .on: return PSText.on
            case .standby: return PSText.standby
            case .offline: return LinkText.status(false)
            }
        }
    }

    @Published private(set) var consoles: [PlayStationConsole] = []
    /// Consoles associées (association au Trousseau), lues une fois.
    @Published private(set) var paired: Set<UUID> = []
    /// Dernière réponse de chaque console, par identifiant.
    @Published private(set) var found: [String: (at: Date, console: PlayStation.Found)] = [:]
    /// Une recherche au moins a abouti.
    @Published private(set) var searched = false
    /// Réveil en cours (« Réveil de la console… »).
    @Published private(set) var waking: Set<UUID> = []
    /// Feuille de l'association ouverte.
    @Published var pairing: PairTarget?
    /// Parties en cours, une fenêtre chacune.
    @Published private(set) var sessions: [UUID: PlayStationSession] = [:]
    /// La partie dont la fenêtre a le clavier (menu Session).
    var keySession: PlayStationSession? { sessions.values.first { $0.isKey } }

    private let key = "playstationConsoles"
    private static let accountKey = "playstation.psn-account"
    private let defaults: UserDefaults
    private let demo: Bool
    private var searching: Task<Void, Never>?
    /// Sans réponse depuis ce délai : hors ligne.
    private static let offlineAfter: TimeInterval = 12

    init(demo: Bool) {
        self.demo = demo
        defaults = demo ? (UserDefaults(suiteName: "fr.leslignesclaires.kybernow.demo") ?? .standard) : .standard
        if let data = defaults.data(forKey: key), let saved = try? JSONDecoder().decode([PlayStationConsole].self, from: data) {
            consoles = saved
        }
        if !demo { paired = Set(consoles.filter { Keychain.get(account: Self.secretKey($0.id)) != nil }.map(\.id)) }
    }

    private static func secretKey(_ id: UUID) -> String { "playstation.\(id.uuidString)" }

    private func save() {
        if let data = try? JSONEncoder().encode(consoles) { defaults.set(data, forKey: key) }
    }

    // MARK: - Compte PSN et associations

    /// Identifiant du compte PSN (base64), lu une fois pour toutes.
    var accountID: String? {
        get { demo ? nil : Keychain.get(account: Self.accountKey) }
        set {
            guard !demo else { return }
            if let newValue { Keychain.set(newValue, account: Self.accountKey) } else { Keychain.delete(account: Self.accountKey) }
        }
    }

    func registered(_ c: PlayStationConsole) -> String? {
        demo ? nil : Keychain.get(account: Self.secretKey(c.id))
    }

    /// Une console vient d'être associée : gardée (ou mise à jour).
    @discardableResult
    func paired(_ f: PlayStation.Found, registered: String) -> PlayStationConsole {
        let hostID = PlayStation.hostID(ofRegistered: registered) ?? f.hostID
        var c = consoles.first { $0.hostID == hostID } ?? PlayStationConsole(name: f.name, hostID: hostID, model: f.model, address: f.address, target: f.target)
        c.name = f.name
        c.address = f.address
        c.target = f.target
        if !demo { Keychain.set(registered, account: Self.secretKey(c.id)) }
        if let i = consoles.firstIndex(where: { $0.id == c.id }) { consoles[i] = c } else { consoles.append(c) }
        paired.insert(c.id)
        save()
        return c
    }

    /// La console ne reconnaît plus ce Mac : « Associer » revient.
    func unpair(_ c: PlayStationConsole) {
        if !demo { Keychain.delete(account: Self.secretKey(c.id)) }
        paired.remove(c.id)
    }

    func remove(_ c: PlayStationConsole) {
        unpair(c)
        consoles.removeAll { $0.id == c.id }
        save()
    }

    func update(_ c: PlayStationConsole) {
        guard let i = consoles.firstIndex(where: { $0.id == c.id }) else { return }
        consoles[i] = c
        save()
    }

    func console(_ id: UUID) -> PlayStationConsole? { consoles.first { $0.id == id } }

    // MARK: - Recherche

    func live(_ c: PlayStationConsole) -> PlayStation.Found? {
        guard let f = found[c.hostID], Date().timeIntervalSince(f.at) < Self.offlineAfter else { return nil }
        return f.console
    }

    func status(_ c: PlayStationConsole) -> Status {
        if let f = live(c) { return f.ready ? .on : .standby }
        return searched ? .offline : .searching
    }

    /// Consoles qui répondent sur le réseau et pas encore gardées.
    var newcomers: [PlayStation.Found] {
        let now = Date()
        return found.values
            .filter { f in now.timeIntervalSince(f.at) < Self.offlineAfter && !consoles.contains { $0.hostID == f.console.hostID } }
            .map(\.console)
            .sorted { $0.name < $1.name }
    }

    /// Cherche toutes les 3 s environ tant que l'accueil est à l'écran (ou
    /// qu'une console se réveille).
    func startSearching(visible: @escaping @MainActor () -> Bool) {
        guard !demo, searching == nil else { return }
        searching = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                // Pas pendant une partie : rien à chercher, et la console répond
                // à la découverte sur le réseau du flux (essai du 03/11/2026 :
                // une image perdue toutes les 15 s pendant la recherche).
                if (visible() && self.sessions.isEmpty) || !self.waking.isEmpty { await self.searchOnce() }
                try? await Task.sleep(for: .seconds(3))
            }
        }
    }

    func stopSearching() {
        searching?.cancel()
        searching = nil
    }

    func searchOnce(wait: UInt32 = 2000) async {
        let direct = consoles.map(\.address)
        let list = await Task.detached(priority: .utility) { PlayStation.search(direct: direct, wait: wait) }.value
        let now = Date()
        for f in list {
            found[f.hostID] = (now, f)
            // Adresse ou nom changés : retenus.
            if var c = consoles.first(where: { $0.hostID == f.hostID }), c.address != f.address || c.name != f.name {
                c.address = f.address
                c.name = f.name
                update(c)
            }
        }
        searched = true
    }

    // MARK: - Réveil et partie

    /// « Réveiller et jouer » : la console est réveillée, puis la partie
    /// s'ouvre dès qu'elle se dit allumée (une minute au plus).
    func wakeAndPlay(_ c: PlayStationConsole, app: AppModel) {
        guard let registered = registered(c), !waking.contains(c.id) else { return }
        waking.insert(c.id)
        Task {
            let address = live(c)?.address ?? c.address
            let sent = await Task.detached { PlayStation.wake(address: address, registered: registered) }.value
            KyberLog.app.notice("PlayStation : réveil \(sent ? "envoyé" : "pas envoyé", privacy: .public)")
            let deadline = Date().addingTimeInterval(60)
            while sent, Date() < deadline {
                await searchOnce(wait: 1500)
                if live(c)?.ready == true { break }
            }
            waking.remove(c.id)
            if live(c)?.ready == true { play(c, app: app) }
        }
    }

    /// « Jouer » : la partie dans sa fenêtre (déjà ouverte : elle passe devant).
    func play(_ c: PlayStationConsole, app: AppModel) {
        if let s = sessions[c.id] {
            s.bringToFront()
            return
        }
        guard let registered = registered(c) else { return }
        let address = live(c)?.address ?? c.address
        var console = c
        console.lastSession = Date()
        update(console)
        // La puce « HDR » allumée et l'écran de la fenêtre HDR : demandé dans les réglages.
        var settings = self.settings(c)
        settings.hdr = c.hasHDR && c.hdr && HDRDisplay.isHDR((NSApp.keyWindow ?? NSApp.mainWindow)?.screen ?? NSScreen.main)
        let settingsJSON = (try? JSONEncoder().encode(settings)).flatMap { String(data: $0, encoding: .utf8) } ?? Self.defaultSettings
        let id = c.id
        let session = PlayStationSession(console: console, address: address, registered: registered, settings: settingsJSON,
                                         layout: layoutJSON(c), fullScreen: settings.fullscreen,
                                         onFullScreen: { [weak self] on in self?.rememberFullScreen(on, id: id) }) { [weak self] s, end in
            guard let self else { return }
            self.sessions[s.console.id] = nil
            // La fin se dit dans la fenêtre ; « Associer » revient si la console ne reconnaît plus ce Mac.
            if end == .notPaired, let c = self.console(s.console.id) { self.unpair(c) }
        }
        sessions[c.id] = session
        session.open()
    }

    // MARK: - Réglages

    private static let defaultSettings = PlayStation.defaultSettings
    private static let defaultLayout = PlayStation.defaultLayout

    func settings(_ c: PlayStationConsole) -> PSSettings {
        let json = console(c.id)?.settings ?? c.settings ?? Self.defaultSettings
        return (try? JSONDecoder().decode(PSSettings.self, from: Data(json.utf8))) ?? PSSettings()
    }

    func setSettings(_ s: PSSettings, for c: PlayStationConsole) {
        guard var c = console(c.id), let data = try? JSONEncoder().encode(s) else { return }
        c.settings = String(data: data, encoding: .utf8)
        update(c)
        // L'agrandissement suit tout de suite dans une partie en cours.
        sessions[c.id]?.setUpscale(s.upscaleSettings)
    }

    /// « Personnalisé » à l'accueil (la règle du cœur).
    func isCustom(_ c: PlayStationConsole) -> Bool {
        PlayStation.settingsCustom(console(c.id)?.settings ?? Self.defaultSettings)
    }

    func layout(_ c: PlayStationConsole) -> PSLayout {
        let json = console(c.id)?.layout ?? c.layout ?? Self.defaultLayout
        return (try? JSONDecoder().decode(PSLayout.self, from: Data(json.utf8)))
            ?? (try? JSONDecoder().decode(PSLayout.self, from: Data(Self.defaultLayout.utf8))) ?? PSLayout()
    }

    private func layoutJSON(_ c: PlayStationConsole) -> String {
        console(c.id)?.layout ?? Self.defaultLayout
    }

    /// Nouvelle disposition : gardée, et tout de suite dans la partie de cette console.
    func setLayout(_ l: PSLayout, for c: PlayStationConsole) {
        guard var c = console(c.id), let data = try? JSONEncoder().encode(l), let json = String(data: data, encoding: .utf8) else { return }
        c.layout = json
        update(c)
        sessions[c.id]?.setLayout(json)
    }

    func resetLayout(for c: PlayStationConsole) {
        guard var c = console(c.id) else { return }
        c.layout = nil
        update(c)
        sessions[c.id]?.setLayout(Self.defaultLayout)
    }

    /// Plein écran rejoint ou quitté pendant la partie : retenu pour la suivante.
    private func rememberFullScreen(_ on: Bool, id: UUID) {
        guard let c = console(id) else { return }
        var s = settings(c)
        guard s.fullscreen != on else { return }
        s.fullscreen = on
        setSettings(s, for: c)
    }

    func setHDR(_ on: Bool, for c: PlayStationConsole) {
        var c = c
        c.hdr = on
        update(c)
    }
}
