// Profil « Auto » et débit adaptatif (docs/link-ui.md, « Profil Auto et
// débit adaptatif » ; contrat : docs/profil-auto.md), commun aux apps Apple
// (Mac, Apple TV, iPhone et iPad) : les textes, et le panneau du test vu de
// l'interface (`AutoTestModel`). Chaque app le dessine à sa manière, avec
// les mêmes textes, les mêmes étapes et les mêmes boutons.

import Foundation
import KyberKit
import SwiftUI

// MARK: - Textes

enum AutoText {
    /// Titre du panneau du test.
    static var title: String { String(localized: "Profil Auto") }
    static var subtitle: String { String(localized: "KyberNow mesure votre réseau et cet appareil, puis propose les meilleurs réglages.") }
    /// Titre du résumé.
    static var summary: String { String(localized: "Réglages proposés") }
    static var apply: String { String(localized: "Appliquer") }
    static var cancel: String { String(localized: "Annuler") }
    static var again: String { String(localized: "Refaire le test") }
    static var retry: String { String(localized: "Réessayer") }
    /// Les étapes du test (`autotest::Step` du cœur), dans l'ordre.
    static let stepKeys = ["link", "latency", "bandwidth", "decode"]
    static var stepTitles: [String] {
        [String(localized: "Liaison"), String(localized: "Latence au repos"),
         String(localized: "Débit vers cet appareil"), String(localized: "Décodage de cet appareil")]
    }
    /// Échec sans console (mot de passe absent) : le texte du cœur pour une Station injoignable.
    static func unreachable(_ name: String) -> String { String(localized: "Le test n'a pas pu joindre \(name).") }

    /// Carte « Débit », profil Auto : « Choisi par le test du 26 novembre (Maison). »
    static func chosenBy(_ r: AutoTestResult) -> String {
        let french = LanguageChoice.effective == "fr"
        let date = r.at.formatted(.dateTime.day().month(.wide).locale(Locale(identifier: french ? "fr_FR" : "en_US")))
        return String(localized: "Choisi par le test du \(date) (\(network(r.network))).")
    }

    /// Le réseau du test tel qu'on le dit : son nom, « Par le relais »,
    /// « Données mobiles », ou « réseau inconnu ».
    static func network(_ name: String?) -> String {
        switch name {
        case "relay"?: return String(localized: "Par le relais")
        case "cellular"?: return String(localized: "Données mobiles")
        case let name? where !name.isEmpty: return name
        default: return String(localized: "réseau inconnu")
        }
    }

    /// Bandeau du début de session : Auto, sur un autre réseau que celui du test.
    static var otherNetwork: String { String(localized: "Autre réseau que celui du test") }
    static var otherNetworkText: String { String(localized: "Le débit adaptatif s'en charge.") }
    /// « Refaire le test » (ou Auto sans test) pendant une session : le
    /// panneau s'ouvrira à sa fin (le test mesure le réseau libre).
    static var afterSession: String { String(localized: "Le test se fera à la fin de la session.") }

    /// Interrupteur de la carte « Débit ».
    static var adaptive: String { String(localized: "Débit adaptatif") }
    static var adaptiveHelp: String {
        String(localized: "Baisse le débit quand le réseau sature, puis le remonte. Coupé, le débit ne bouge jamais.")
    }
    /// Son aide en plus, en PyroWave.
    static var adaptivePyroWave: String { String(localized: "En PyroWave, il est en pause : la définition baisse à la place.") }

    /// Aide de « Format vidéo », Auto, par câble, en Jeu et Bureau.
    static var formatJeuBureauWired: String {
        String(localized: "Auto : HEVC. PyroWave est pris par Max et Auto, quand cet appareil est relié par câble.")
    }
}

// MARK: - Le panneau du test

/// Le panneau du test d'un ordinateur : ouvert en choisissant Auto sans test
/// gardé, ou par « Refaire le test ». Le test tourne dès l'ouverture ;
/// « Annuler » l'arrête et garde le profil d'avant ; « Appliquer » garde le
/// résultat pour l'ordinateur et met Auto.
@MainActor
final class AutoTestModel: ObservableObject {
    enum Phase: Equatable {
        case running([AutoTestStep])
        case done(AutoTestResult, [AutoTestChoice])
        case failed(String)
    }

    /// D'où le panneau s'ouvre : chaque app le présente par-dessus l'accueil
    /// ou par-dessus les réglages (iPhone, iPad : une fiche sur une fiche).
    enum Origin: Equatable {
        case home
        case settings
    }

    struct Panel: Identifiable, Equatable {
        let hostID: UUID
        let origin: Origin
        var id: UUID { hostID }
    }

    @Published var panel: Panel?
    @Published private(set) var phase: Phase = .running([])
    /// Auto choisi (ou « Refaire le test ») pendant une session : le panneau
    /// s'ouvre à sa fin (le test mesurerait le réseau occupé par l'image).
    private(set) var afterSession: UUID?

    let store: HostStore
    /// L'écran de cet appareil pour cet ordinateur (pixels et fréquence), donné par l'app.
    var screen: (Host) -> (width: Int, height: Int, refresh: Int) = { _ in (1920, 1080, 60) }
    private var run: AutoTestRun?
    private var demoTask: Task<Void, Never>?

    init(store: HostStore) {
        self.store = store
        #if DEBUG
        // Démonstration : `-demoAutoTest done|running|failed` ouvre le panneau
        // sur le premier ordinateur (captures des trois apps).
        if store.demo, UserDefaults.standard.string(forKey: "demoAutoTest") != nil {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1) { [weak self] in
                guard let self, let host = self.store.ordered.first else { return }
                self.open(host)
            }
        }
        #endif
    }

    var host: Host? { panel.flatMap { store.host($0.hostID) } }

    /// Choisir un profil (accueil, menus, réglages) : Auto sans test gardé
    /// ouvre le panneau du test ; sinon le profil s'applique (Auto : les
    /// réglages du test gardé). Vrai si le profil a été appliqué.
    @discardableResult
    func choose(_ profile: Profile, for host: Host, origin: Origin = .home) -> Bool {
        guard profile == .auto, host.settings.auto == nil else {
            var h = host
            h.settings.apply(profile)
            store.update(h)
            return true
        }
        open(host, origin: origin)
        return false
    }

    /// Ouvre le panneau et lance le test.
    func open(_ host: Host, origin: Origin = .home) {
        afterSession = nil
        panel = Panel(hostID: host.id, origin: origin)
        start()
    }

    /// Pendant une session : le panneau s'ouvrira à sa fin (`sessionEnded`).
    func openAfterSession(_ host: Host) {
        afterSession = host.id
    }

    /// La session de `hostID` est finie : le panneau demandé pendant elle s'ouvre.
    func sessionEnded(_ hostID: UUID) {
        guard afterSession == hostID, let host = store.host(hostID) else { return }
        open(host)
    }

    /// (Re)lance le test : « Réessayer », « Refaire le test ».
    func start() {
        stop()
        guard let host else { return }
        phase = .running(Self.waiting)
        #if DEBUG
        if store.demo {
            demo(host)
            return
        }
        #endif
        guard let console = store.console(for: host) else {
            phase = .failed(AutoText.unreachable(host.name))
            return
        }
        let mode = screen(host)
        let device = AutoTestDevice(width: mode.width, height: mode.height, refresh: mode.refresh, hdr: host.settings.hdr,
                                    network: CurrentNetwork.name, wired: NetworkLink.shared.wired, name: host.name)
        run = AutoTestRun(console: console, device: device, onProgress: { [weak self] steps in
            guard let self, case .running = self.phase else { return }
            self.phase = .running(steps)
        }, onDone: { [weak self] outcome in
            guard let self else { return }
            self.run = nil
            switch outcome {
            case let .result(r): self.phase = .done(r, r.choices)
            case let .failed(text): self.phase = .failed(text)
            case .cancelled: break
            }
        })
        if run == nil { phase = .failed(AutoText.unreachable(host.name)) }
    }

    /// « Annuler » : le test s'arrête, le panneau se ferme, le profil d'avant reste.
    func cancel() {
        stop()
        panel = nil
    }

    /// « Appliquer » : Auto devient le profil de cet ordinateur, avec ces réglages.
    func apply() {
        guard case let .done(r, _) = phase, let host else { return }
        var h = host
        h.settings.apply(auto: r)
        store.update(h)
        panel = nil
    }

    private func stop() {
        run?.cancel()
        run = nil
        demoTask?.cancel()
        demoTask = nil
    }

    /// Les quatre étapes avant le premier avancement du cœur (mêmes titres
    /// que les siens), la première en cours.
    private static var waiting: [AutoTestStep] {
        zip(AutoText.stepKeys, AutoText.stepTitles).enumerated().map { i, s in
            AutoTestStep(key: s.0, title: s.1, state: i == 0 ? .running : .waiting)
        }
    }

    #if DEBUG
    /// Démonstration : un test qui avance tout seul jusqu'au résumé (captures).
    private func demo(_ host: Host) {
        let fr = LanguageChoice.effective == "fr"
        let titles = AutoText.stepTitles
        let measures = fr
            ? ["Câble", "2,1 ms · gigue 0,4 ms", "940 Mbit/s · latence en charge +3 ms", "HEVC 4K : 240 i/s · PyroWave 4K : 180 i/s"]
            : ["Cable", "2.1 ms · jitter 0.4 ms", "940 Mbit/s · latency under load +3 ms", "HEVC 4K: 240 fps · PyroWave 4K: 180 fps"]
        let keys = ["link", "latency", "bandwidth", "decode"]
        let stop = UserDefaults.standard.string(forKey: "demoAutoTest")
        demoTask = Task { [weak self] in
            for done in 0...4 {
                let steps = (0..<4).map { i in
                    AutoTestStep(key: keys[i], title: titles[i], state: i < done ? .done(measures[i]) : i == done ? .running : .waiting)
                }
                self?.phase = .running(steps)
                if stop == "running", done == 2 { return }
                try? await Task.sleep(for: .milliseconds(700))
                if Task.isCancelled { return }
            }
            if stop == "failed" {
                self?.phase = .failed(AutoText.unreachable(host.name))
                return
            }
            if let r = AutoTestResult(json: Self.demoResult) { self?.phase = .done(r, r.choices) }
        }
    }

    private static let demoResult = """
    {"at":1795680000,"network":"Ethernet","measures":{"link":"cable","latency":{"min":1.8,"median":2.1,"jitter":0.4},\
    "bandwidth":{"mbps":940.0,"loaded_delta":3.0},"decode":[{"codec":"hevc","yuv444":false,"fps":240.0},\
    {"codec":"pyrowave","yuv444":false,"fps":180.0}]},"codec":"hevc","yuv444":true,"width":3840,"height":2160,"refresh":60,\
    "mbps":300,"mode_mbps":300,"reliable":false,"reasons":{"codec":"pyro_wave_not_carried","mode":"matches_display",\
    "yuv444":"yuv444_allowed","bitrate":"measured","transport":"steady","decode_fps":240.0},"station":"PC"}
    """
    #endif
}

extension AutoTestStep {
    /// La mesure d'une étape cochée, ou la raison d'une étape passée.
    var detail: String? {
        switch state {
        case let .done(text), let .skipped(text): return text.isEmpty ? nil : text
        case .waiting, .running: return nil
        }
    }
}

extension HostStore {
    /// Le bandeau « Autre réseau que celui du test » : Auto, sur un réseau
    /// (ou un chemin : le relais) autre que celui du test.
    func autoOtherNetwork(_ host: Host, viaRelay: Bool) -> Bool {
        guard host.settings.profile == .auto, let r = host.settings.auto else { return false }
        return !r.sameNetwork(viaRelay ? "relay" : CurrentNetwork.name)
    }
}
