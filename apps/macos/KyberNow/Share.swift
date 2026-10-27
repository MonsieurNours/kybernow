// KyberNow Share côté app (docs/kybernow-share.md ; textes : docs/link-ui.md,
// « Partage », dans LinkUI.swift) : ce que le Mac, l'Apple TV, l'iPhone et
// l'iPad font de la même façon, une seule fois. L'invité : son prénom retenu,
// « Rejoindre » (le code part vers les ordinateurs trouvés sur le réseau, ou
// vers celui du lien). L'hôte : le partage de sa session (ouvrir, nouveau
// code, rôles, retirer, arrêter) suivi avec la liste des invités que le cœur
// rend à chaque signe de vie. Chaque app dessine ses écrans et ses panneaux ;
// les appels et les règles sont ici.

import Combine
import Foundation
import KyberKit
#if canImport(UIKit)
import UIKit
#endif

// MARK: - Le prénom de l'invité

enum GuestName {
    private static let key = "kybernow.guestName"

    /// Le prénom retenu la dernière fois, sinon le nom de l'appareil.
    static var proposed: String {
        let saved = UserDefaults.standard.string(forKey: key)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return saved.isEmpty ? deviceName : saved
    }

    static func remember(_ name: String) {
        UserDefaults.standard.set(name.trimmingCharacters(in: .whitespacesAndNewlines), forKey: key)
    }

    /// Le nom de l'appareil, tel que les ordinateurs le voient.
    static var deviceName: String {
        let name = (ConsoleClient.thisDevice["name"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return name.isEmpty ? "KyberNow" : name
    }
}

// MARK: - Liens

/// Ce que lit un lien ou un QR code `kybernow://` : l'ajout d'un ordinateur
/// (adresse, port, empreinte) ou un partage à rejoindre (`kybernow://join?…`).
enum LinkKind {
    case computer(Candidate)
    case join(ShareJoinLink)

    static func parse(_ text: String) -> LinkKind? {
        if let link = Share.parseLink(text) { return .join(link) }
        return PairingLink.parse(text).map(LinkKind.computer)
    }
}

// MARK: - Rejoindre

/// « Rejoindre avec un code » : le code et le prénom, la recherche, les refus
/// du cœur. À la fin, `onJoined` reçoit la Station qui a accepté ; l'app ouvre
/// alors la session d'invité (rien n'est ajouté à ses ordinateurs).
@MainActor
final class ShareJoinModel: ObservableObject {
    enum State: Equatable {
        case idle
        /// « Recherche du partage… ».
        case looking
        /// Le texte du refus, à montrer tel quel.
        case failed(String)
    }

    /// Les six chiffres, tapés comme on veut (« 123 456 »).
    @Published var code = ""
    @Published var name = GuestName.proposed
    @Published private(set) var state: State = .idle
    /// Les ordinateurs annoncés sur le réseau, à qui le code est envoyé.
    let discovery = Discovery()
    /// Un lien a donné la Station : le code lui est envoyé, à elle seule.
    private(set) var link: ShareJoinLink?
    /// Appelé une fois, avec la Station qui a accepté le code.
    var onJoined: ((ShareFound) -> Void)?
    private var task: Task<Void, Never>?
    private var attempt = 0

    init(link: ShareJoinLink? = nil) {
        if let link {
            self.link = link
            code = Share.formatCode(link.code)
        }
    }

    var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    var normalizedCode: String? { Share.normalizeCode(code) }
    var looking: Bool { state == .looking }
    var canJoin: Bool { normalizedCode != nil && !trimmedName.isEmpty && !looking }

    /// À l'ouverture de l'écran : l'annonce Bonjour démarre tout de suite.
    /// Avec un lien, la Station est connue : rien n'est cherché ni envoyé
    /// avant « Rejoindre » (docs/link-ui.md).
    func start() {
        guard link == nil else { return }
        discovery.start()
    }

    /// Un QR code ou un lien lu pendant que l'écran est ouvert (appareil
    /// photo) : le code et la Station sont remplis, comme avec un lien ; rien
    /// ne part avant « Rejoindre ».
    func use(link: ShareJoinLink) {
        cancel()
        discovery.stop()
        self.link = link
        code = Share.formatCode(link.code)
        state = .idle
    }

    func stop() {
        discovery.stop()
    }

    /// Le code tapé : six chiffres au plus, groupés par trois (« 123 456 ») ;
    /// un chiffre de plus ou de moins efface le dernier refus.
    func codeChanged() {
        let digits = String(code.filter(\.isASCII).filter(\.isNumber).prefix(6))
        let grouped = digits.count > 3 ? "\(digits.prefix(3)) \(digits.dropFirst(3))" : digits
        if code != grouped { code = grouped }
        if case .failed = state { state = .idle }
    }

    /// « Rejoindre » : `hosts` sont les ordinateurs de l'appareil, dont les
    /// empreintes retenues servent à vérifier les Stations trouvées.
    func join(hosts: [Host]) {
        guard canJoin, let code = normalizedCode else { return }
        let name = trimmedName
        attempt += 1
        let mine = attempt
        state = .looking
        task?.cancel()
        task = Task { [weak self] in
            guard let self else { return }
            do {
                let targets = await self.targets(hosts: hosts)
                guard !targets.isEmpty else { throw ShareJoinError.noShare(ShareText.noShare) }
                let found = try await Share.join(targets: targets, code: code, name: name)
                // Annulé entre-temps, ou l'écran n'est plus là : l'invité repart tout de suite.
                guard mine == self.attempt, let onJoined = self.onJoined else {
                    Share.leave(found)
                    return
                }
                GuestName.remember(name)
                self.state = .idle
                onJoined(found)
            } catch {
                guard mine == self.attempt else { return }
                self.state = .failed(Self.text(error))
            }
        }
    }

    /// « Annuler » pendant la recherche, ou l'écran qui se ferme.
    func cancel() {
        attempt += 1
        task?.cancel()
        task = nil
        if looking { state = .idle }
    }

    /// Le texte d'un refus : celui du cœur, dans la langue de l'app, en phrase
    /// (les erreurs d'appel, comme un certificat changé, commencent par une minuscule).
    static func text(_ error: Error) -> String {
        let text = (error as? ShareJoinError)?.errorDescription ?? error.localizedDescription
        return text.isEmpty ? ShareText.noShare : ShareText.sentence(text)
    }

    // MARK: Les Stations à qui envoyer le code

    private func targets(hosts: [Host]) async -> [ShareTarget] {
        if let link { return [link.target] }
        // Les annonces arrivent un instant après l'ouverture de l'écran.
        var waited = 0
        while discovery.found.isEmpty, waited < 30, !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(100))
            waited += 1
        }
        let announced = discovery.found
        let candidates = await withTaskGroup(of: Candidate?.self) { group in
            for found in announced {
                group.addTask { await Discovery.resolve(found) }
            }
            var list: [Candidate] = []
            for await candidate in group { if let candidate { list.append(candidate) } }
            return list
        }
        var seen = Set<String>()
        var targets: [ShareTarget] = []
        for c in candidates where seen.insert("\(c.address):\(c.port)").inserted {
            // L'empreinte retenue pour cet ordinateur, sinon celle de l'annonce.
            let known = hosts.first { $0.address == c.address && $0.consolePort == c.port }?.consoleFingerprint
            targets.append(ShareTarget(host: c.address, port: c.port, fingerprint: known ?? c.expected))
        }
        return targets
    }
}

// MARK: - Hôte

/// Le partage de la session d'hôte : « Partager » l'ouvre, le panneau le
/// montre (code, lien, QR code, invités), ses actions passent par Admin. La
/// liste des invités se met à jour avec le signe de vie du cœur
/// (`SessionEngine.onGuests`).
@MainActor
final class ShareHostModel: ObservableObject {
    /// Le partage ouvert, nil sans partage.
    @Published private(set) var state: ShareState?
    /// Un appel à Admin est en cours.
    @Published private(set) var busy = false
    /// Le texte du dernier refus d'Admin, à montrer dans le panneau.
    @Published private(set) var error: String?
    /// « Partager » est-il disponible avec cet ordinateur ? Relu à chaque fois,
    /// d'après ce que la Station dit d'elle-même : la barre et l'ouverture du
    /// partage n'ont ainsi qu'une seule réponse.
    var availability: ShareAvailability { availabilityOf() }
    private let availabilityOf: @MainActor () -> ShareAvailability
    private var console: ConsoleClient?
    private var openedAt = Date.distantPast
    private var closedAt = Date.distantPast

    init(console: ConsoleClient?, availability: @escaping @MainActor () -> ShareAvailability) {
        self.console = console
        availabilityOf = availability
    }

    /// Ce que la Station d'un ordinateur dit du partage : tant qu'on ne l'a pas
    /// lue (pas encore sondée, injoignable un instant), « Partager » reste
    /// actif ; une Station qui ne connaît pas le partage, ou qui l'a coupé, le
    /// rend inactif avec la raison.
    static func availability(of host: UUID, in store: HostStore) -> ShareAvailability {
        store.status[host]?.info?.share ?? .available
    }
    var isOpen: Bool { state != nil }
    var guests: [ShareGuest] { state?.guests ?? [] }
    /// Quatre invités au plus : le texte se montre quand la liste est pleine.
    var full: Bool { guests.count >= 4 }

    func setConsole(_ console: ConsoleClient?) {
        self.console = console
    }

    /// « Partager » : le partage s'ouvre s'il ne l'est pas déjà.
    func openIfNeeded() async {
        guard state == nil, availability.isAvailable else { return }
        #if DEBUG
        if demo { return demoOpen() }
        #endif
        await run { console in
            if let current = try await console.currentShare() { return current }
            return try await console.shareOpen()
        }
    }

    func newCode() async {
        #if DEBUG
        if demo { return demoNewCode() }
        #endif
        await run { try await $0.shareNewCode() }
    }

    func setRole(_ role: ShareRole, of guest: ShareGuest) async {
        #if DEBUG
        if demo { return demoUpdate { $0.guests = $0.guests.map { $0.id == guest.id ? $0.with(role: role) : $0 } } }
        #endif
        await run { console in
            try await console.shareSetRole(role, of: guest.id)
            return try await console.currentShare()
        }
    }

    func kick(_ guest: ShareGuest) async {
        #if DEBUG
        if demo { return demoUpdate { $0.guests.removeAll { $0.id == guest.id } } }
        #endif
        await run { console in
            try await console.shareKick(guest.id)
            return try await console.currentShare()
        }
    }

    /// « Arrêter le partage » : tout le monde sort.
    func close() async {
        #if DEBUG
        if demo {
            state = nil
            return
        }
        #endif
        guard let console else { return }
        busy = true
        defer { busy = false }
        do {
            try await console.shareClose()
            error = nil
            closedAt = Date()
            state = nil
        } catch {
            self.error = ShareText.sentence(error.localizedDescription)
        }
    }

    /// Ce que le signe de vie du cœur dit du partage (liste des invités, code).
    func heartbeat(_ share: ShareState?) {
        if let share {
            // Une réponse partie avant « Arrêter le partage » n'ouvre rien.
            guard Date().timeIntervalSince(closedAt) > 3 else { return }
            if state != share { state = share }
        } else if state != nil, Date().timeIntervalSince(openedAt) > 8 {
            state = nil
        }
    }

    private func run(_ call: @escaping @Sendable (ConsoleClient) async throws -> ShareState?) async {
        guard let console, !busy else { return }
        busy = true
        defer { busy = false }
        do {
            let next = try await call(console)
            error = nil
            if let next {
                if state == nil { openedAt = Date() }
                state = next
            } else {
                state = nil
            }
        } catch {
            self.error = ShareText.sentence(error.localizedDescription)
        }
    }

    #if DEBUG
    // MARK: Démonstration

    /// Démonstration (`-demo YES`, Debug) : sans Station, le partage se joue
    /// ici (code, invités, rôles), pour dessiner et essayer le panneau.
    var demo = false

    private func demoOpen() {
        state = ShareState(code: "482915", link: "kybernow://join?host=192.168.1.20&port=37462&fp=sha256:c41be0f2&code=482915",
                           web: "https://192.168.1.20:37462/link/?join=482915", display: "Écran 1", guests: [])
        openedAt = Date()
    }

    private func demoNewCode() {
        demoUpdate {
            $0.code = String(format: "%06d", Int.random(in: 100_000 ... 999_999))
            $0.link = "kybernow://join?host=192.168.1.20&port=37462&fp=sha256:c41be0f2&code=\($0.code)"
            $0.web = "https://192.168.1.20:37462/link/?join=\($0.code)"
        }
    }

    private func demoUpdate(_ change: (inout ShareState) -> Void) {
        guard var next = state else { return }
        change(&next)
        state = next
    }

    /// Un invité qui arrive (commande de démonstration).
    func demoAddGuest(name: String, app: String, role: ShareRole = .spectator) {
        if state == nil { demoOpen() }
        demoUpdate {
            guard $0.guests.count < 4 else { return }
            $0.guests.append(ShareGuest(id: UUID().uuidString, name: name, app: app, role: role, since: Date().addingTimeInterval(-Double.random(in: 30 ... 900))))
        }
    }
    #endif

    // MARK: Ce que le panneau dit

    /// Ce que chaque rôle propose à l'hôte, dans cet ordre (« Retirer » est à
    /// part, toujours en dernier) : un spectateur, « Faire jouer » ; un
    /// joueur, « Faire regarder » (retour en spectateur) à la place, puis
    /// « Donner » ou « Reprendre le clavier et la souris » (docs/link-ui.md).
    static func roleActions(for role: ShareRole) -> [(title: String, role: ShareRole)] {
        switch role {
        case .spectator: return [(ShareText.letPlay, .player)]
        case .player: return [(ShareText.watchOnly, .spectator), (ShareText.giveKeyboardMouse, .playerKbm)]
        case .playerKbm: return [(ShareText.watchOnly, .spectator), (ShareText.takeBackKeyboardMouse, .player)]
        }
    }

    /// « Copier le lien » : l'adresse du client web (`web`), à défaut le lien
    /// des apps ; le QR code, lui, porte toujours le lien des apps (`link`).
    var linkToCopy: String? {
        guard let state else { return nil }
        return state.web.isEmpty ? state.link : state.web
    }
}
