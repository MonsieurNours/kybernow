// KyberNow Share côté app (docs/kybernow-share.md, docs/relais.md ; textes :
// docs/link-ui.md, « Partage », dans LinkUI.swift) : ce que le Mac, l'Apple TV,
// l'iPhone et l'iPad font de la même façon, une seule fois. L'invité : son
// prénom retenu, « Rejoindre » (le code part vers les ordinateurs trouvés sur
// le réseau et vers les relais, ou vers celui du lien ; hors de chez l'hôte,
// « En attente de l'accord de l'hôte… »). L'hôte : le partage de sa session
// (ouvrir, nouveau code, accepter ou refuser une demande, rôles, retirer,
// arrêter) suivi avec la liste des invités que le cœur rend à chaque signe de
// vie. Chaque app dessine ses écrans et ses panneaux ; les appels et les
// règles sont ici.

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

// MARK: - La qualité de l'image de l'invité

enum GuestQuality {
    private static let key = "kybernow.guestQuality"

    /// La qualité retenue la dernière fois (à « Rejoindre » ou dans la barre),
    /// sinon « Auto » (proposé).
    static var proposed: ShareQuality {
        let saved = UserDefaults.standard.object(forKey: key) as? Int
        return saved.flatMap(ShareQuality.init(rawValue:)) ?? .auto
    }

    static func remember(_ quality: ShareQuality) {
        UserDefaults.standard.set(quality.rawValue, forKey: key)
    }

    /// Ce que dit la barre de l'image reçue : « 1080p · 60 Hz » quand le cœur
    /// sait la dire, sinon la qualité choisie (« Auto ») tant que la taille
    /// n'est pas sue.
    static func barLabel(image: ShareImage?, quality: ShareQuality) -> String {
        image?.label ?? quality.label
    }
}

#if DEBUG
extension ShareImage {
    /// Démonstration (Debug, sans Station) : une image d'invité lue par le cœur
    /// comme celle de l'échange avec Admin ; sans taille, elle est « encore
    /// inconnue » (pas de libellé).
    static func demo(_ quality: String, _ width: Int? = nil, _ height: Int? = nil, _ fps: Int? = nil, follows: Bool? = nil) -> ShareImage {
        var json: [String: Any] = ["quality": quality]
        if let width, let height, let fps { (json["width"], json["height"], json["fps"], json["codec"]) = (width, height, fps, "h264") }
        if let follows { json["follows_host"] = follows }
        let text = (try? JSONSerialization.data(withJSONObject: json)).flatMap { String(data: $0, encoding: .utf8) }
        return ShareImage(json: text) ?? ShareImage(quality: .auto, label: nil)
    }
}
#endif

// MARK: - Liens

/// Ce que lit un lien ou un QR code `kybernow://` : l'ajout d'un ordinateur
/// (adresse, port, empreinte), un partage à rejoindre (`kybernow://join?…`)
/// ou l'invitation d'un ami qui prête son ordinateur (`kybernow://borrow?…`).
enum LinkKind {
    case computer(Candidate)
    case join(ShareJoinLink)
    case borrow(LendInvite)
    /// Un lien d'invitation que le cœur ne lit pas (champ manquant, port
    /// faux…) : « Ce lien d'invitation est illisible. » (`LendText.badInvite`).
    case badInvite

    static func parse(_ text: String) -> LinkKind? {
        if let link = Share.parseLink(text) { return .join(link) }
        if let invite = Lend.parseInvite(text) { return .borrow(invite) }
        if LendInvite.looksLikeInvite(text) { return .badInvite }
        return PairingLink.parse(text).map(LinkKind.computer)
    }
}

// MARK: - Rejoindre

/// « Rejoindre avec un code » : le code et le prénom, la recherche, l'attente de
/// l'accord de l'hôte (par le relais), les refus du cœur. À la fin, `onJoined`
/// reçoit la Station qui a accepté ; l'app ouvre alors la session d'invité (rien
/// n'est ajouté à ses ordinateurs).
@MainActor
final class ShareJoinModel: ObservableObject {
    enum State: Equatable {
        case idle
        /// « Recherche du partage… ».
        case looking
        /// Hors de chez l'hôte : « En attente de l'accord de l'hôte… », avec « Annuler ».
        case waiting
        /// Le texte du refus, à montrer tel quel.
        case failed(String)
    }

    /// Les six chiffres, tapés comme on veut (« 123 456 »).
    @Published var code = ""
    @Published var name = GuestName.proposed
    /// « Qualité de l'image » : « Auto » proposé, retenue pour la fois suivante.
    @Published var quality = GuestQuality.proposed
    @Published private(set) var state: State = .idle
    /// Les ordinateurs annoncés sur le réseau, à qui le code est envoyé.
    let discovery = Discovery()
    /// Un lien a donné la Station : le code lui est envoyé, à elle seule.
    private(set) var link: ShareJoinLink?
    /// Appelé une fois, avec la Station qui a accepté le code.
    var onJoined: ((ShareFound) -> Void)?
    /// Le code est un code de prêt (docs/pret.md) : l'app emprunte
    /// l'ordinateur avec lui (session d'emprunt, « Préparation de
    /// l'ordinateur »). Sans lui : « Ce code sert à emprunter l'ordinateur. »
    var onLend: ((BorrowSource) -> Void)?
    private var task: Task<Void, Never>?
    private var attempt = 0
    /// La demande en attente de l'accord de l'hôte, à annuler si l'écran se ferme.
    private var pending: SharePending?
    /// L'écran de l'appareil (celui de la fenêtre ; la sortie du téléviseur sur
    /// Apple TV), pour « Auto » : sa définition et sa fréquence.
    private let screen: @MainActor () -> ShareScreen

    init(link: ShareJoinLink? = nil, screen: @escaping @MainActor () -> ShareScreen) {
        self.screen = screen
        if let link {
            self.link = link
            code = Share.formatCode(link.code)
        }
    }

    var trimmedName: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }
    var normalizedCode: String? { Share.normalizeCode(code) }
    var looking: Bool { state == .looking }
    /// Hors de chez l'hôte : la demande attend son accord.
    var waiting: Bool { state == .waiting }
    /// Une recherche ou une attente est en cours : les champs sont inactifs, « Annuler » est là.
    var busy: Bool { looking || waiting }
    var canJoin: Bool { normalizedCode != nil && !trimmedName.isEmpty && !busy }

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
        let quality = quality
        let screen = screen()
        attempt += 1
        let mine = attempt
        state = .looking
        task?.cancel()
        let link = link
        task = Task { [weak self] in
            guard let self else { return }
            do {
                // Un lien donne la Station (et son relais) ; un code part vers les
                // ordinateurs du réseau et vers les relais, celui des ordinateurs
                // de l'appareil et le relais public.
                let joining: ShareJoining
                if let link {
                    joining = try await Share.joinLink(link, code: code, name: name, quality: quality, screen: screen)
                } else {
                    let targets = await self.targets(hosts: hosts)
                    joining = try await Share.joinAnywhere(targets: targets, relays: Self.relays(hosts: hosts), code: code, name: name, quality: quality, screen: screen)
                }
                switch joining {
                case let .joined(found):
                    self.finish(found, name: name, quality: quality, mine: mine)
                case let .pending(pending):
                    // Annulé entre-temps : la demande part tout de suite.
                    guard mine == self.attempt else {
                        Share.cancel(pending)
                        return
                    }
                    self.pending = pending
                    self.state = .waiting
                    await self.waitForHost(pending, name: name, quality: quality, mine: mine)
                }
            } catch {
                guard mine == self.attempt else { return }
                // Un code de prêt : l'emprunt prend la suite, avec l'écran de l'appareil.
                if case let .lend(found)? = error as? ShareJoinError, let onLend = self.onLend {
                    GuestName.remember(name)
                    self.state = .idle
                    onLend(BorrowSource(target: found.target, credentials: .code(found.code), name: name))
                    return
                }
                self.state = .failed(Self.text(error))
            }
        }
    }

    /// L'invité entre : sa session s'ouvre (annulé entre-temps, ou l'écran n'est
    /// plus là : il repart tout de suite).
    private func finish(_ found: ShareFound, name: String, quality: ShareQuality, mine: Int) {
        guard mine == attempt, let onJoined else {
            Share.leave(found)
            return
        }
        pending = nil
        GuestName.remember(name)
        GuestQuality.remember(quality)
        state = .idle
        onJoined(found)
    }

    /// « En attente de l'accord de l'hôte… » : un appel tenu 25 s, rappelé tant
    /// que l'hôte ne répond pas, jusqu'à l'accord, au refus ou à l'expiration (120 s).
    private func waitForHost(_ pending: SharePending, name: String, quality: ShareQuality, mine: Int) async {
        while mine == attempt, !Task.isCancelled {
            do {
                if let found = try await Share.wait(pending, quality: quality) {
                    finish(found, name: name, quality: quality, mine: mine)
                    return
                }
            } catch {
                guard mine == attempt else { return }
                self.pending = nil
                state = .failed(Self.text(error))
                return
            }
        }
    }

    /// Les relais de chaque ordinateur de l'appareil (`info.relay`), sous leur forme
    /// normale, sans doublon : le code leur est demandé aussi. Le relais public
    /// (`share.kybernow.app`) n'y est pas : le cœur l'interroge toujours.
    static func relays(hosts: [Host]) -> [String] {
        var list: [String] = []
        for server in hosts.compactMap({ $0.relay?.server }) where !server.isEmpty {
            let normal = Relay.normalize(server)
            if !list.contains(normal) { list.append(normal) }
        }
        return list
    }

    #if DEBUG
    /// Démonstration : « En attente de l'accord de l'hôte… », sans relais.
    func demoWaiting(code: String) {
        self.code = code
        state = .waiting
    }
    #endif

    /// « Annuler » pendant la recherche ou l'attente, ou l'écran qui se ferme :
    /// une demande en attente est retirée chez l'hôte.
    func cancel() {
        attempt += 1
        task?.cancel()
        task = nil
        if let pending {
            Share.cancel(pending)
            self.pending = nil
        }
        if busy { state = .idle }
    }

    /// Le texte d'un refus : celui du cœur, dans la langue de l'app, en phrase
    /// (les erreurs d'appel, comme un certificat changé, commencent par une minuscule).
    static func text(_ error: Error) -> String {
        // Trois demandes attendent déjà : le texte de link-ui (celui du cœur le dit un peu autrement).
        if case .pendingFull? = error as? ShareJoinError { return ShareText.pendingFull }
        // L'hôte a arrêté le partage pendant l'attente (code 28 du cœur, jamais le texte).
        if case .stopped? = error as? ShareJoinError { return ShareText.stopped }
        let text = (error as? ShareJoinError)?.errorDescription ?? error.localizedDescription
        return text.isEmpty ? ShareText.noShare : ShareText.sentence(text)
    }

    // MARK: Les Stations à qui envoyer le code

    private func targets(hosts: [Host]) async -> [ShareTarget] {
        if let link { return [link.target] }
        // Les annonces arrivent un instant après l'ouverture de l'écran ; hors de
        // chez soi, il n'y en a pas : le relais n'attend pas plus d'une seconde.
        var waited = 0
        while discovery.found.isEmpty, waited < 10, !Task.isCancelled {
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

/// L'un des deux droits d'un invité, que l'hôte donne ou reprend sans toucher à l'autre.
enum ShareRight: Equatable {
    /// Les manettes.
    case pads
    /// Le clavier et la souris.
    case keyboardAndMouse
}

extension ShareRole {
    /// Le rôle avec ce droit donné ou repris, l'autre tel quel (le calcul est celui du cœur).
    func with(_ right: ShareRight, on: Bool) -> ShareRole {
        switch right {
        case .pads: return with(pads: on)
        case .keyboardAndMouse: return with(keyboardAndMouse: on)
        }
    }
}

/// Le lien du relais en préparation (docs/link-ui.md), pour le panneau du
/// partage comme pour le code de prêt : `connecting` se montre 10 s au plus
/// (« Préparation du lien hors de chez vous… », copie et QR code retenus),
/// comptées depuis que l'app voit l'attente pour ce code, puis comme
/// `unreachable` ; les autres états passent tels quels.
@MainActor
final class RelayLinkWait {
    private var wait: (code: String, since: Date)?
    private var task: Task<Void, Never>?

    /// Ce que le panneau montre du relais de `code`. `recheck` est rappelé une
    /// fois à la fin des 10 s, sans attendre le signe de vie suivant.
    func phase(code: String?, relay: ShareState.Relay?, recheck: @escaping @MainActor () -> Void) -> ShareState.Relay? {
        guard let code, relay == .connecting else {
            wait = nil
            task?.cancel()
            task = nil
            return relay
        }
        if wait?.code != code {
            wait = (code, Date())
            task?.cancel()
            task = Task {
                try? await Task.sleep(for: .seconds(ShareState.relayLinkWait + 0.05))
                guard !Task.isCancelled else { return }
                recheck()
            }
        }
        return ShareState.relayPhase(relay, waitingSince: wait?.since)
    }
}

/// Le partage de la session d'hôte : « Partager » l'ouvre, le panneau le
/// montre (code, lien, QR code, invités), ses actions passent par Admin. La
/// liste des invités se met à jour avec le signe de vie du cœur
/// (`SessionEngine.onGuests`).
@MainActor
final class ShareHostModel: ObservableObject {
    /// Le partage ouvert, nil sans partage.
    @Published private(set) var state: ShareState? {
        didSet {
            announceNewRequest(since: oldValue)
            updateRelay()
        }
    }
    /// Le relais tel que le panneau le montre (docs/link-ui.md) : `connecting`
    /// 10 s au plus depuis que l'app voit l'attente pour ce code, puis
    /// `unreachable` ; sinon `state.relay`. Les vues lisent celui-ci.
    @Published private(set) var relay: ShareState.Relay?
    /// « Préparation du lien hors de chez vous… » : « Copier le lien » et le QR code attendent.
    var waitingForRelayLink: Bool { relay == .connecting }
    private let relayWait = RelayLinkWait()
    /// Une demande vient d'arriver pendant la session : avis bref « {prénom} demande
    /// à rejoindre. » (chaque app le montre à sa façon ; le choisir ouvre le panneau).
    var onRequest: ((ShareGuest) -> Void)?
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
    /// Les invités acceptés (« Invités », pastille « 2 invités »).
    var guests: [ShareGuest] { state?.accepted ?? [] }
    /// Les demandes venues par le relais, qui attendent l'accord de l'hôte
    /// (« Demandes », pastille « 1 demande ») ; elles ne comptent pas dans les invités.
    var requests: [ShareGuest] { state?.requests ?? [] }
    /// Quatre invités au plus : le texte se montre quand la liste est pleine.
    var full: Bool { guests.count >= 4 }

    func setConsole(_ console: ConsoleClient?) {
        self.console = console
    }

    /// Recalcule `relay` (`RelayLinkWait`, relu à la fin des 10 s).
    private func updateRelay() {
        let next = relayWait.phase(code: state?.code, relay: state?.relay) { [weak self] in self?.updateRelay() }
        if relay != next { relay = next }
    }

    /// La première demande que `previous` ne connaissait pas (une seule par
    /// signe de vie) : l'hôte en est averti sans ouvrir le panneau.
    private func announceNewRequest(since previous: ShareState?) {
        let known = Set((previous?.requests ?? []).map(\.id))
        if let request = (state?.requests ?? []).first(where: { !known.contains($0.id) }) { onRequest?(request) }
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

    /// « Donner » ou « Reprendre les manettes », « Donner » ou « Reprendre le
    /// clavier et la souris » : un droit change, l'autre reste tel quel. Une
    /// seule action à la fois, jusqu'à la réponse d'Admin (`busy`, et les
    /// boutons sont inactifs d'ici là) : deux clics rapides sur les deux droits
    /// ne s'annulent pas. Le rôle demandé se calcule d'après le dernier état
    /// connu de l'invité, pas d'après ce que le menu affichait quand il s'est
    /// ouvert : le droit que l'autre bouton vient de changer n'est jamais défait.
    func setRight(_ right: ShareRight, on: Bool, of guest: ShareGuest) async {
        guard !busy else { return }
        let role = (guests.first { $0.id == guest.id } ?? guest).role.with(right, on: on)
        #if DEBUG
        if demo { return demoUpdate { $0.guests = $0.guests.map { $0.id == guest.id ? $0.with(role: role) : $0 } } }
        #endif
        await run { console in
            try await console.shareSetRole(role, of: guest.id)
            return try await console.currentShare()
        }
    }

    /// « Accepter » ou « Refuser » la demande d'un invité venu par le relais :
    /// accepté, il entre ; refusé, son appareil l'est 10 minutes. Quatre invités
    /// au plus : Admin refuse la cinquième acceptation (son texte s'affiche).
    func answer(_ request: ShareGuest, accept: Bool) async {
        #if DEBUG
        if demo {
            return demoUpdate { state in
                if accept, state.accepted.count < 4 {
                    state.guests = state.guests.map { $0.id == request.id ? ShareGuest(id: $0.id, name: $0.name, app: $0.app, role: .spectator, since: Date(), pending: false, viaRelay: $0.viaRelay) : $0 }
                } else {
                    state.guests.removeAll { $0.id == request.id }
                }
            }
        }
        #endif
        await run { console in
            accept ? try await console.shareAccept(request.id) : try await console.shareRefuse(request.id)
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
                           web: "https://192.168.1.20:37462/link/?join=482915", display: "Écran 1", webLocal: "https://192.168.1.20:37462/link/?join=482915",
                           relay: nil, guests: [])
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
    func demoAddGuest(name: String, app: String, role: ShareRole = .spectator, image: ShareImage? = nil, pending: Bool = false, viaRelay: Bool = false) {
        if state == nil { demoOpen() }
        demoUpdate {
            guard pending || $0.accepted.count < 4 else { return }
            $0.guests.append(ShareGuest(id: UUID().uuidString, name: name, app: app, role: role,
                                        since: Date().addingTimeInterval(-Double.random(in: 30 ... 900)), image: image,
                                        pending: pending, viaRelay: viaRelay))
        }
    }

    /// Démonstration : l'état du relais du partage (nil : pas de relais), pour
    /// l'aide du panneau.
    func demoSetRelay(_ relay: ShareState.Relay?) {
        if state == nil { demoOpen() }
        demoUpdate { $0.relay = relay }
    }
    #endif

    // MARK: Ce que le panneau dit

    /// Ce que chaque invité propose à l'hôte, dans cet ordre (« Retirer » est à
    /// part, toujours en dernier) : deux droits indépendants, chacun donné ou
    /// repris sans toucher à l'autre, « Donner » ou « Reprendre les manettes »,
    /// puis « Donner » ou « Reprendre le clavier et la souris »
    /// (docs/link-ui.md). Chaque bouton dit quel droit il change et s'il le
    /// donne (`on`) ou le reprend ; `ShareHostModel.setRight` fait le reste.
    static func roleActions(for role: ShareRole) -> [(title: String, right: ShareRight, on: Bool)] {
        [(role.pads ? ShareText.takeBackPads : ShareText.givePads, .pads, !role.pads),
         (role.keyboardAndMouse ? ShareText.takeBackKeyboardMouse : ShareText.giveKeyboardMouse, .keyboardAndMouse, !role.keyboardAndMouse)]
    }

    /// « Copier le lien » : l'adresse du client web (`web`), à défaut le lien
    /// des apps ; le QR code, lui, porte toujours le lien des apps (`link`).
    /// Rien tant que le lien du relais se prépare (`waitingForRelayLink`).
    var linkToCopy: String? {
        guard let state, !waitingForRelayLink else { return nil }
        return state.web.isEmpty ? state.link : state.web
    }
}
