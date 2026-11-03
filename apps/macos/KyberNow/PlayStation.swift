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

    var hasHDR: Bool { model == "PS5" }
}

/// Textes des consoles (docs/link-ui.md, « Consoles PlayStation »).
enum PSText {
    static var on: String { String(localized: "Allumée") }
    static var standby: String { String(localized: "En veille") }
    static var play: String { String(localized: "Jouer") }
    static var wakeAndPlay: String { String(localized: "Réveiller et jouer") }
    static var waking: String { String(localized: "Réveil de la console…") }
    static var pair: String { String(localized: "Associer") }

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
                if visible() || !self.waking.isEmpty { await self.searchOnce() }
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
        let hdr = c.hasHDR && c.hdr && HDRDisplay.isHDR((NSApp.keyWindow ?? NSApp.mainWindow)?.screen ?? NSScreen.main)
        let session = PlayStationSession(console: console, address: address, registered: registered, hdr: hdr) { [weak self] s, end in
            guard let self else { return }
            self.sessions[s.console.id] = nil
            // La fin se dit dans la fenêtre ; « Associer » revient si la console ne reconnaît plus ce Mac.
            if end == .notPaired, let c = self.console(s.console.id) { self.unpair(c) }
        }
        sessions[c.id] = session
        session.open()
    }

    func setHDR(_ on: Bool, for c: PlayStationConsole) {
        var c = c
        c.hdr = on
        update(c)
    }
}
