// Prêter l'ordinateur (KyberNow Share) côté app (docs/pret.md ; textes :
// docs/link-ui.md, « Prêt de l'ordinateur », dans LinkUI.swift) : ce que le
// Mac, l'Apple TV, l'iPhone et l'iPad font de la même façon, une seule fois.
// L'ami : les ordinateurs qu'on lui prête (« Ordinateur de {hôte} », ajoutés
// par un lien d'invitation ou « Avec un compte d'ami », à part des
// ordinateurs appairés, mot de passe au trousseau), leur état, et l'emprunt
// (« Préparation de l'ordinateur », « En attente de {hôte}… », les refus du
// cœur) que chaque session d'emprunt mène avant de se connecter. L'hôte : le
// prêt de son ordinateur depuis sa session (code de séance, demandes,
// reprise) et ce que l'accueil en dit. Chaque app dessine ses écrans ; les
// appels et les règles sont ici.

import Combine
import Foundation
import KyberKit

// MARK: - Les ordinateurs qu'on prête à cet appareil

/// L'ordinateur d'un ami, ajouté à l'accueil : la Station de l'invitation (ou
/// celle choisie sur le réseau), l'identifiant de l'ami, et ce que le dernier
/// statut a dit de l'hôte. Le mot de passe est au trousseau (« Retenir le mot
/// de passe »), sinon en mémoire le temps de l'app.
struct BorrowedComputer: Codable, Identifiable, Equatable {
    var id = UUID()
    var invite: LendInvite
    /// Le prénom de l'hôte (« Ordinateur de {hôte} »), d'après le dernier statut.
    var host: String = ""
    /// Le nom de l'ordinateur, d'après le dernier statut.
    var computer: String = ""
    var rememberPassword = true

    var login: String { invite.login }
    var target: LendTarget? { invite.target }

    /// {hôte} des textes : le prénom de l'hôte (celui du dernier statut, sinon
    /// celui du lien d'invitation), sinon le nom de l'ordinateur, sinon son adresse.
    var hostName: String {
        if !host.isEmpty { return host }
        if let name = invite.name { return name }
        return computer.isEmpty ? invite.host : computer
    }

    /// « Ordinateur de {hôte} ».
    var title: String { LendText.computerOf(hostName) }
}

/// L'état de la carte « Ordinateur de {hôte} ».
enum BorrowedState: Equatable {
    /// Pas encore de réponse.
    case checking
    /// La Station ne répond pas : « Hors ligne ».
    case offline
    case status(LendStatus)
    /// Un refus de la Station à montrer sous le nom (« Identifiant ou mot de passe refusé. »).
    case refused(String)

    var availability: LendAvailability? {
        if case let .status(s) = self { return s.state }
        return nil
    }

    /// « Disponible », « Occupé »…, « Hors ligne », ou rien tant qu'on ne sait
    /// pas ; le prêt en cours est le sien : « Prêté par {hôte} ».
    var label: String? {
        switch self {
        case .checking: return nil
        case .offline: return LinkText.status(false)
        case let .status(s): return s.mine && s.state == .lent ? LendText.lentBy(s.host) : s.state.label
        case .refused: return nil
        }
    }

    /// On peut demander l'ordinateur : libre (« Emprunter »), utilisé
    /// (« Demander à {hôte} »), ou déjà prêté à cet ami (« Revenir »).
    var canBorrow: Bool {
        guard case let .status(s) = self else { return false }
        return s.state == .free || s.state == .busy || (s.state == .lent && s.mine)
    }

    /// Le prêt en cours est le sien : « Revenir ».
    var comesBack: Bool {
        guard case let .status(s) = self else { return false }
        return s.state == .lent && s.mine
    }
}

/// Les ordinateurs prêtés à cet appareil, à part des ordinateurs appairés.
@MainActor
final class BorrowedStore: ObservableObject {
    @Published private(set) var computers: [BorrowedComputer] = []
    @Published private(set) var states: [UUID: BorrowedState] = [:]
    /// Démonstration (`-demo YES`, Debug) : préférences à part, rien sur le réseau.
    let demo: Bool
    private let defaults: UserDefaults
    private let key = "borrowed"
    /// Mots de passe non retenus (et ceux de la démonstration) : le temps de l'app.
    private var memory: [UUID: String] = [:]
    private var polling: Task<Void, Never>?
    /// Le statut coûte un mot de passe vérifié chez la Station (dix essais par
    /// minute et par adresse) : un tour toutes les 30 s, et à la demande.
    private static let pollInterval: TimeInterval = 30

    init(demo: Bool = false) {
        self.demo = demo
        defaults = demo ? (UserDefaults(suiteName: "fr.leslignesclaires.kybernow.demo") ?? .standard) : .standard
        if let data = defaults.data(forKey: key), let saved = try? JSONDecoder().decode([BorrowedComputer].self, from: data) {
            computers = saved
        }
        if !demo { startPolling() }
    }

    func computer(_ id: UUID) -> BorrowedComputer? { computers.first { $0.id == id } }

    /// Déjà ajouté (même Station, même identifiant).
    func existing(for invite: LendInvite) -> BorrowedComputer? {
        computers.first { $0.invite.same(as: invite) }
    }

    func password(for computer: BorrowedComputer) -> String? {
        if demo || !computer.rememberPassword { return memory[computer.id] }
        return Keychain.get(computer.id)
    }

    private func setPassword(_ password: String, for computer: BorrowedComputer) {
        if demo || !computer.rememberPassword {
            memory[computer.id] = password
            if !demo { Keychain.delete(computer.id) }
        } else {
            Keychain.set(password, for: computer.id)
            memory[computer.id] = nil
        }
    }

    /// « Ajouter » : l'identifiant et le mot de passe sont vérifiés auprès de la
    /// Station (le statut donne aussi le prénom de l'hôte). Un refus de la
    /// Station est rendu tel quel ; une Station injoignable n'empêche pas
    /// l'ajout (la carte dira « Hors ligne »). Déjà là : mis à jour.
    @discardableResult
    func add(invite: LendInvite, password: String, remember: Bool) async throws -> BorrowedComputer {
        var computer = existing(for: invite) ?? BorrowedComputer(invite: invite)
        computer.invite = invite
        computer.rememberPassword = remember
        var state = BorrowedState.offline
        #if DEBUG
        if demo {
            computer.host = "Matthieu"
            computer.computer = "PC-SALON"
            state = .status(LendStatus(state: .free, host: "Matthieu", computer: "PC-SALON"))
        }
        #endif
        if !demo, let target = invite.target {
            do {
                let status = try await Lend.status(target, login: invite.login, password: password)
                computer.host = status.host
                computer.computer = status.computer
                state = .status(status)
            } catch let error as LendError where !error.unreachable {
                throw error
            } catch {
                state = .offline
            }
        }
        if let i = computers.firstIndex(where: { $0.id == computer.id }) { computers[i] = computer } else { computers.append(computer) }
        setPassword(password, for: computer)
        states[computer.id] = state
        save()
        return computer
    }

    /// Un nouveau mot de passe (refusé, ou pas retenu) : gardé, puis vérifié.
    func updatePassword(_ password: String, remember: Bool, for computer: BorrowedComputer) {
        guard var c = self.computer(computer.id) else { return }
        c.rememberPassword = remember
        update(c)
        setPassword(password, for: c)
        Task { await refresh(c) }
    }

    func remove(_ computer: BorrowedComputer) {
        computers.removeAll { $0.id == computer.id }
        states[computer.id] = nil
        memory[computer.id] = nil
        if !demo { Keychain.delete(computer.id) }
        save()
    }

    private func update(_ computer: BorrowedComputer) {
        guard let i = computers.firstIndex(where: { $0.id == computer.id }) else { return }
        computers[i] = computer
        save()
    }

    private func save() {
        if let data = try? JSONEncoder().encode(computers) { defaults.set(data, forKey: key) }
    }

    private func startPolling() {
        polling = Task { [weak self] in
            while !Task.isCancelled {
                await self?.refreshAll()
                try? await Task.sleep(for: .seconds(Self.pollInterval))
            }
        }
    }

    func refreshAll() async {
        await withTaskGroup(of: Void.self) { group in
            for c in computers { group.addTask { await self.refresh(c) } }
        }
    }

    /// Le statut de la carte (« Disponible », « Occupé »…). Sans mot de passe
    /// (pas retenu, app relancée), rien n'est demandé : l'ami le tapera à « Emprunter ».
    func refresh(_ computer: BorrowedComputer) async {
        guard !demo, let target = computer.target else { return }
        guard let password = password(for: computer) else {
            if states[computer.id] == nil { states[computer.id] = .checking }
            return
        }
        do {
            let status = try await Lend.status(target, login: computer.login, password: password)
            states[computer.id] = .status(status)
            if var c = self.computer(computer.id), c.host != status.host || c.computer != status.computer {
                c.host = status.host
                c.computer = status.computer
                update(c)
            }
        } catch let error as LendError where !error.unreachable {
            states[computer.id] = .refused(ShareText.sentence(error.localizedDescription))
        } catch {
            states[computer.id] = .offline
        }
    }

    #if DEBUG
    /// Démonstration : « Ordinateur de Matthieu » sur l'accueil, dans l'état demandé
    /// (`-demoBorrowed free|busy|lent|locked|off|offline|slot`).
    func loadDemo(_ state: String) {
        for c in computers { remove(c) }
        guard let invite = LendInvite(json: #"{"host":"192.168.1.30","port":37462,"fingerprint":"sha256:ab12","login":"camille"}"#) else { return }
        let c = BorrowedComputer(invite: invite, host: "Matthieu", computer: "PC-SALON")
        computers = [c]
        memory[c.id] = "demo"
        setDemoState(state, for: c.id)
    }

    func setDemoState(_ state: String, for id: UUID) {
        let now = Date()
        switch state {
        case "offline": states[id] = .offline
        case "refused": states[id] = .refused(String(localized: "Identifiant ou mot de passe refusé."))
        case "mine":
            states[id] = .status(LendStatus(state: .lent, host: "Matthieu", computer: "PC-SALON", mine: true))
        case "slot":
            states[id] = .status(LendStatus(state: .busy, host: "Matthieu", computer: "PC-SALON",
                                            slot: LendSlot(now: false, next: LendPeriod(start: now.addingTimeInterval(86400 * 2), end: now.addingTimeInterval(86400 * 2 + 4 * 3600)))))
        case "slot-now":
            states[id] = .status(LendStatus(state: .free, host: "Matthieu", computer: "PC-SALON",
                                            slot: LendSlot(now: true, next: LendPeriod(start: now.addingTimeInterval(-3600), end: now.addingTimeInterval(2 * 3600)))))
        default:
            states[id] = .status(LendStatus(state: LendAvailability(rawValue: state) ?? .free, host: "Matthieu", computer: "PC-SALON"))
        }
    }
    #endif
}

// MARK: - Emprunter

/// Ce qu'il faut pour emprunter : la Station, le code ou le compte, le prénom
/// de l'ami, et {hôte} quand on le sait déjà (l'ordinateur d'un ami). La
/// session d'emprunt y ajoute l'écran de l'appareil (`BorrowRequest`), et le
/// garde pour « Revenir ».
struct BorrowSource: Equatable {
    let target: LendTarget
    let credentials: LendCredentials
    let name: String
    /// {hôte}, s'il est déjà connu (titre de la session avant l'accord).
    var hostHint: String = ""
    /// L'ordinateur de l'accueil d'où vient l'emprunt (son statut est relu à la fin).
    var computer: UUID?

    func request(screen: LendScreen) -> BorrowRequest {
        BorrowRequest(target: target, credentials: credentials, name: name, screen: screen)
    }
}

/// L'emprunt, avant la session : « Préparation de l'ordinateur » (le mode
/// console se met en place, 30 s au plus), ou « En attente de {hôte}… » avec
/// « Annuler » (ordinateur utilisé : un appel tenu 30 s, rappelé jusqu'à
/// l'accord, au refus ou à l'expiration), puis le prêt (`onReady`) ou le
/// texte du refus. Chaque session d'emprunt (Mac, Apple TV, iPhone, iPad)
/// en a un.
@MainActor
final class BorrowFlow: ObservableObject {
    enum State: Equatable {
        case idle
        /// « Préparation de l'ordinateur ».
        case preparing
        /// « En attente de {hôte}… », avec « Annuler ».
        case waiting(host: String)
        /// Le prêt est là : la session se connecte.
        case ready
        /// Le texte du refus, à montrer tel quel.
        case failed(String)
    }

    @Published private(set) var state: State = .idle
    let source: BorrowSource
    private(set) var request: BorrowRequest?
    /// Le prêt, une fois obtenu.
    private(set) var loan: LendLoan?
    var onReady: ((LendLoan) -> Void)?
    var onFailed: ((String) -> Void)?
    private var task: Task<Void, Never>?
    private var pending: LendPending?
    private var attempt = 0

    init(source: BorrowSource) {
        self.source = source
    }

    /// {hôte} : celui de la réponse, sinon celui déjà connu.
    var host: String {
        if let loan { return loan.host }
        if case let .waiting(host) = state { return host }
        return source.hostHint
    }

    /// Emprunte avec l'écran de l'appareil (« Revenir » : le même, une fois de plus).
    func start(screen: LendScreen) {
        let request = source.request(screen: screen)
        self.request = request
        attempt += 1
        let mine = attempt
        task?.cancel()
        state = .preparing
        task = Task { [weak self] in
            do {
                switch try await Lend.borrow(request) {
                case let .ready(loan):
                    self?.finish(loan, mine: mine)
                case let .pending(pending):
                    guard let self, mine == self.attempt else {
                        Lend.cancel(pending)
                        return
                    }
                    self.pending = pending
                    self.state = .waiting(host: pending.host.isEmpty ? self.source.hostHint : pending.host)
                    await self.waitForHost(pending, mine: mine)
                }
            } catch {
                self?.fail(error, mine: mine)
            }
        }
    }

    #if DEBUG
    /// Démonstration : l'emprunt sans Station (`waiting` : « En attente de {hôte}… »).
    func demo(_ loan: LendLoan?, waiting: String? = nil) {
        if let waiting {
            state = .waiting(host: waiting)
            return
        }
        guard let loan else { return }
        self.loan = loan
        state = .ready
    }
    #endif

    private func waitForHost(_ pending: LendPending, mine: Int) async {
        while mine == attempt, !Task.isCancelled {
            do {
                if let loan = try await Lend.wait(pending) {
                    finish(loan, mine: mine)
                    return
                }
            } catch {
                fail(error, mine: mine)
                return
            }
        }
    }

    private func finish(_ loan: LendLoan, mine: Int) {
        guard mine == attempt, let onReady else {
            Lend.leave(loan)
            return
        }
        pending = nil
        self.loan = loan
        state = .ready
        onReady(loan)
    }

    private func fail(_ error: Error, mine: Int) {
        guard mine == attempt else { return }
        pending = nil
        let text = Self.text(error)
        state = .failed(text)
        onFailed?(text)
    }

    /// « Annuler » pendant la préparation ou l'attente, ou la session qui se
    /// ferme avant le prêt : la demande en attente est retirée chez l'hôte.
    func cancel() {
        attempt += 1
        task?.cancel()
        task = nil
        if let pending {
            Lend.cancel(pending)
            self.pending = nil
        }
        if state == .preparing || state.isWaiting { state = .idle }
    }

    /// Le texte d'un refus : celui du cœur, en phrase.
    static func text(_ error: Error) -> String {
        let text = (error as? LendError)?.errorDescription ?? error.localizedDescription
        return ShareText.sentence(text)
    }
}

extension BorrowFlow.State {
    var isWaiting: Bool {
        if case .waiting = self { return true }
        return false
    }
}

/// Le temps qui reste à un emprunt, pour la barre et le lanceur : jusqu'au
/// préavis s'il y en a un, sinon jusqu'à la fin prévue ; nil sans fin
/// (« Jusqu'à ce que {hôte} reprenne »).
enum BorrowClock {
    static func remaining(until: Date?, notice: LendNotice?, now: Date = Date()) -> TimeInterval? {
        let end = notice?.at ?? until
        return end.map { max(0, $0.timeIntervalSince(now)) }
    }

    /// « Encore 1 h 20 » ou « Jusqu'à ce que {hôte} reprenne ».
    static func label(until: Date?, notice: LendNotice?, host: String, now: Date = Date()) -> String {
        guard let left = remaining(until: until, notice: notice, now: now) else { return LendText.untilTakesBack(host) }
        return LendText.timeLeft(left)
    }
}

// MARK: - Hôte

/// « Prêter l'ordinateur… » : possible, ou inactif avec la raison (Station trop
/// ancienne, prêt coupé dans Admin).
enum LendHostAvailability: Equatable {
    case available
    case unavailable(String)

    var isAvailable: Bool { self == .available }
    var reason: String? {
        if case let .unavailable(text) = self { return text }
        return nil
    }

    /// D'après ce que la Station dit d'elle-même (`info.lend`). Tant qu'on ne
    /// l'a pas lue, possible ; absente : Station trop ancienne ; `available`
    /// faux : « Le prêt n'est pas possible sur cet ordinateur. ».
    static func of(_ info: HostInfo?) -> LendHostAvailability {
        guard let info else { return .available }
        guard let lend = info.lend else { return .unavailable(LendText.tooOld) }
        guard lend.available else { return .unavailable(LendText.notPossible) }
        return lend.enabled ? .available : .unavailable(LendText.offInAdmin)
    }
}

/// La durée d'un code de séance : « 1 heure », « 2 heures » (proposé),
/// « 4 heures », « Jusqu'à ce que je reprenne ».
enum LendDuration: Int, CaseIterable, Identifiable {
    case oneHour = 3600
    case twoHours = 7200
    case fourHours = 14400
    case untilTakenBack = 0

    var id: Int { rawValue }
    /// En secondes ; nil : sans fin.
    var seconds: Int? { self == .untilTakenBack ? nil : rawValue }

    var label: String {
        switch self {
        case .oneHour: return String(localized: "1 heure")
        case .twoHours: return String(localized: "2 heures")
        case .fourHours: return String(localized: "4 heures")
        case .untilTakenBack: return String(localized: "Jusqu'à ce que je reprenne")
        }
    }
}

extension LendSteam {
    var label: String {
        switch self {
        case .friend: return String(localized: "Son compte Steam")
        case .host: return String(localized: "Votre compte Steam")
        }
    }
}

/// Le prêt de l'ordinateur depuis la session de l'hôte : « Prêter
/// l'ordinateur… » ouvre son panneau (durée, compte Steam, « Créer un code »,
/// code, lien, QR code, « Annuler le code »), les demandes arrivent avec
/// « Accepter » et « Refuser ». La vue se met à jour avec le signe de vie du
/// cœur (`SessionEngine.onLend`).
@MainActor
final class LendHostModel: ObservableObject {
    @Published private(set) var view: LendView? {
        didSet {
            announceNewRequest(since: oldValue)
            updateRelay()
        }
    }
    /// Le relais du code tel que le panneau le montre, comme celui du partage
    /// (`ShareHostModel.relay`, docs/link-ui.md) : `connecting` 10 s au plus
    /// depuis que l'app voit l'attente pour ce code, puis `unreachable`.
    @Published private(set) var relay: ShareState.Relay?
    /// « Préparation du lien hors de chez vous… » : « Copier le lien » et le QR code attendent.
    var waitingForRelayLink: Bool { relay == .connecting }
    private let relayWait = RelayLinkWait()
    @Published var duration: LendDuration = .twoHours
    @Published var steam: LendSteam = .friend
    @Published private(set) var busy = false
    @Published private(set) var error: String?
    /// Une demande vient d'arriver : « {prénom} veut emprunter l'ordinateur. »
    /// (chaque app le montre à sa façon ; le choisir ouvre le panneau).
    var onRequest: ((LendRequest) -> Void)?
    var availability: LendHostAvailability { availabilityOf() }
    private let availabilityOf: @MainActor () -> LendHostAvailability
    private var console: ConsoleClient?
    private static let warningKey = "kybernow.lendWarningSeen"

    init(console: ConsoleClient?, availability: @escaping @MainActor () -> LendHostAvailability) {
        self.console = console
        availabilityOf = availability
    }

    func setConsole(_ console: ConsoleClient?) { self.console = console }

    var code: LendCodeView? { view?.code }
    var requests: [LendRequest] { view?.requests ?? [] }
    /// « Copier le lien » : l'adresse du client web, à défaut le lien des apps.
    /// Rien tant que le lien du relais se prépare (`waitingForRelayLink`).
    var linkToCopy: String? {
        guard let code, !waitingForRelayLink else { return nil }
        return code.web.isEmpty ? code.link : code.web
    }

    /// L'avertissement se montre une fois (puis dans Admin).
    var showWarning: Bool { !UserDefaults.standard.bool(forKey: Self.warningKey) }
    func warningSeen() {
        #if DEBUG
        // La démonstration ne touche pas aux préférences de l'app.
        if demo { return }
        #endif
        UserDefaults.standard.set(true, forKey: Self.warningKey)
    }

    /// À l'ouverture du panneau : la vue du moment (le code en cours, s'il y en a un).
    func load() async {
        #if DEBUG
        if demo { return }
        #endif
        await run { try await $0.lendState() }
    }

    func createCode() async {
        #if DEBUG
        if demo { return demoCode() }
        #endif
        let duration = duration.seconds, steam = steam
        await run { try await $0.lendCode(duration: duration, steam: steam) }
    }

    func cancelCode() async {
        #if DEBUG
        if demo { return demoUpdate { $0.code = nil } }
        #endif
        await run { try await $0.lendCancelCode() }
    }

    /// « Accepter » (avec le compte Steam choisi) ou « Refuser ». Accepter
    /// ferme la session de l'hôte (« Votre session va se fermer. »).
    func answer(_ request: LendRequest, accept: Bool) async {
        #if DEBUG
        if demo { return demoUpdate { $0.requests.removeAll { $0.id == request.id } } }
        #endif
        // Le compte Steam proposé par la demande (celui du compte d'ami, ou du code).
        let steam = request.steam
        await run { try await $0.lendAnswer(request.id, accept: accept, steam: steam) }
    }

    /// Ce que le signe de vie du cœur dit du prêt.
    func heartbeat(_ view: LendView?) {
        if self.view != view { self.view = view }
    }

    /// Recalcule `relay` (`RelayLinkWait`, comme le partage).
    private func updateRelay() {
        let next = relayWait.phase(code: code?.code, relay: code?.relay) { [weak self] in self?.updateRelay() }
        if relay != next { relay = next }
    }

    private func announceNewRequest(since previous: LendView?) {
        let known = Set((previous?.requests ?? []).map(\.id))
        if let request = requests.first(where: { !known.contains($0.id) }) { onRequest?(request) }
    }

    private func run(_ call: @escaping @Sendable (ConsoleClient) async throws -> LendView) async {
        guard let console, !busy else { return }
        busy = true
        defer { busy = false }
        do {
            view = try await call(console)
            error = nil
        } catch {
            self.error = ShareText.sentence(error.localizedDescription)
        }
    }

    #if DEBUG
    // MARK: Démonstration

    /// Démonstration (`-demo YES`, Debug) : sans Station, le prêt se joue ici.
    var demo = false
    /// Démonstration : le relais des codes créés (nil : pas de relais réglé).
    var demoRelay: ShareState.Relay?

    private func demoCode() {
        let code = String(format: "%06d", Int.random(in: 100_000 ... 999_999))
        let joined = demoRelay == .joined
        demoUpdate {
            $0.code = LendCodeView(code: code, link: "kybernow://join?host=192.168.1.20&port=37462&fp=sha256:c41be0f2&code=\(code)"
                                       + (joined ? "&relay=share.kybernow.app&sid=k7f3q9" : ""),
                                   web: joined ? "https://share.kybernow.app/s/k7f3q9/link/?join=\(code)" : "https://192.168.1.20:37462/link/?join=\(code)",
                                   duration: self.duration.seconds, steam: self.steam,
                                   expires: Date().addingTimeInterval(86400), relay: self.demoRelay)
        }
    }

    private func demoUpdate(_ change: (inout LendView) -> Void) {
        var next = view ?? LendView()
        change(&next)
        view = next
    }

    func demoSet(_ view: LendView?) { self.view = view }
    #endif
}

extension HostStore {
    /// « Prêter l'ordinateur… » avec cet ordinateur.
    func lendAvailability(of id: UUID) -> LendHostAvailability {
        LendHostAvailability.of(status[id]?.info)
    }

    /// « Reprendre l'ordinateur » (préavis de 2 min) ou « Reprendre tout de
    /// suite », depuis l'accueil de cet ordinateur. Rend le texte d'un refus.
    @discardableResult
    func takeBack(_ host: Host, now: Bool) async -> String? {
        #if DEBUG
        if demo {
            guard var view = lends[host.id], let borrower = view.borrower else { return nil }
            if now {
                setDemoLend(nil, for: host.id)
            } else {
                view.state = .ending
                view.notice = LendNotice(reason: .takenBack, at: Date().addingTimeInterval(120))
                view.borrower = borrower
                setDemoLend(view, for: host.id)
            }
            return nil
        }
        #endif
        guard let console = console(for: host) else { return nil }
        do {
            let view = try await console.lendTakeBack(now: now)
            setLend(view, for: host.id)
            return nil
        } catch {
            return ShareText.sentence(error.localizedDescription)
        }
    }
}

/// Ce que dit l'accueil d'un ordinateur prêté par cet appareil : « Prêté à
/// {prénom} » (« … jusqu'à 18 h »), « Reprendre l'ordinateur », et pendant le
/// préavis « Reprise dans 1:42 » et « Reprendre tout de suite ».
@MainActor
enum LentHome {
    /// La vue, seulement si l'ordinateur est prêté.
    static func view(of id: UUID, in store: HostStore) -> LendView? {
        guard let view = store.lends[id], view.isLent else { return nil }
        return view
    }

    static func line(_ view: LendView) -> String {
        guard let borrower = view.borrower else { return "" }
        if let until = borrower.until { return LendText.lentTo(borrower.name, until: until) }
        return LendText.lentTo(borrower.name)
    }

    /// Le préavis de reprise en cours (« Reprise dans 1:42 »).
    static func takingBack(_ view: LendView, now: Date = Date()) -> String? {
        guard let notice = view.notice, notice.reason == .takenBack else { return nil }
        return LendText.takingBackIn(max(0, notice.at.timeIntervalSince(now)))
    }
}

extension LendInvite {
    /// L'invitation avec l'identifiant tapé par l'ami, la Station et la route du lien gardées.
    func with(login: String) -> LendInvite {
        guard !login.isEmpty, login != self.login, var j = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any] else { return self }
        j["login"] = login
        return (try? JSONSerialization.data(withJSONObject: j)).flatMap { String(data: $0, encoding: .utf8) }.flatMap(LendInvite.init(json:)) ?? self
    }
}

extension String {
    var trimmed: String { trimmingCharacters(in: .whitespacesAndNewlines) }

    /// Le texte, ou nil s'il est vide (après les espaces).
    var nonEmpty: String? {
        let t = trimmingCharacters(in: .whitespacesAndNewlines)
        return t.isEmpty ? nil : t
    }
}
