// Accueil au premier lancement et incitation au compte KyberNow (docs/link-ui.md,
// « Accueil et compte KyberNow » ; maquettes :
// https://claude.ai/artifact/K6tgZaZC8dqeBxGnTBenru). Commun au Mac, à
// l'iPhone, à l'iPad et à l'Apple TV : l'état, gardé dans les préférences de
// l'appareil, les règles du cœur (`WelcomeCore`, welcome.rs) et les textes ;
// chaque app dessine ses écrans.
//
// - L'accueil en trois écrans : au premier lancement, et une fois après la
//   mise à jour qui l'apporte, sans compte ; « Passer », « Plus tard » ou la
//   fin le marquent vu ; « Revoir l'accueil » (Réglages de l'app) le rejoue.
//   Un compte connecté depuis l'écran 2 passe l'écran 3.
// - Sans compte, ensuite : la carte du compte en fin de rangée (fermée, elle
//   revient 30 jours plus tard, puis « Ne plus afficher ») ; « Hors de chez
//   vous ? » quand un ordinateur appairé ne répond pas sur ce réseau et n'a
//   pas de route par le relais (une fois par jour et par ordinateur) ; le
//   bandeau après un appairage (une fois par ordinateur).
// - « Créer un compte » ouvre la page d'inscription du relais dans le
//   navigateur de l'appareil (jamais dans la fenêtre de connexion : l'app ne
//   crée pas de compte, règle 5.1.1(v) de l'App Store), puis le dit.

import Combine
import Foundation
import KyberKit
#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

// MARK: - Textes

/// Les textes de l'accueil et de l'incitation au compte (docs/link-ui.md).
enum WelcomeText {
    /// « BIENVENUE · 2 / 3 » (en capitales à l'écran).
    static func step(_ n: Int) -> String { String(localized: "Bienvenue · \(n) / 3") }
    static var skip: String { String(localized: "Passer") }
    static var later: String { String(localized: "Plus tard") }
    static var next: String { AccountText.continueLabel }

    static var title1: String { String(localized: "Votre ordinateur, sur tous vos appareils") }
    static var text1: String { String(localized: "Jouez, travaillez et retrouvez votre bureau depuis un autre écran, comme si vous étiez devant.") }

    static var title2: String { String(localized: "Avec un compte, tout vous suit") }
    static var text2: String { String(localized: "Un compte KyberNow, et vos ordinateurs vous attendent partout.") }
    /// Les quatre avantages : étiquette (en capitales à l'écran), texte, icône.
    static var perks: [(label: String, text: String, icon: String)] {
        [(String(localized: "Hors de chez vous"), String(localized: "Rejoignez votre ordinateur de n'importe où, sans rien régler sur votre box."), "globe-hemisphere-west"),
         (String(localized: "Tous vos appareils"), String(localized: "Vos ordinateurs apparaissent d'eux-mêmes partout où vous vous connectez."), "monitor"),
         (String(localized: "Partager et prêter"), String(localized: "Invitez un ami à jouer, prêtez votre PC, ouvrez-le à la famille ou à votre équipe."), "share-network"),
         (String(localized: "Vos réglages"), String(localized: "Profil, format et débit vous suivent d'un appareil à l'autre."), "sliders-horizontal")]
    }
    static var connect: String { AccountText.signIn }
    static var create: String { String(localized: "Créer un compte") }
    static var withoutAccount: String { String(localized: "Continuer sans compte") }
    /// Sous les boutons de l'écran 2 ({relais} : share.kybernow.app).
    static func help(_ relay: String) -> String { String(localized: "La page de \(relay) s'ouvre : votre mot de passe ne passe jamais par l'app.") }
    /// Après « Créer un compte » : l'avis.
    static var createdNotice: String { String(localized: "Créez votre compte dans le navigateur, puis revenez ici : « Connecter un compte ».") }
    /// Téléviseurs : sous les étapes du code.
    static var codeNoAccount: String { String(localized: "Pas encore de compte ? Créez-le sur cette page.") }

    static var title3: String { String(localized: "Ajoutez votre ordinateur") }
    static var text3: String {
        String(localized: "Installez KyberNow Station sur l'ordinateur à rejoindre : sur le même réseau, il apparaît ici tout seul. Un ami vous a donné un code ? Entrez-le.")
    }
    static var inReach: String { String(localized: "Ordinateurs à portée") }
    static var onNetwork: String { String(localized: "Sur le réseau") }
    static var add: String { String(localized: "Ajouter") }
    static var codeLabel: String { String(localized: "Un code") }
    static var codeText: String { String(localized: "Un ami vous ouvre son ordinateur : entrez le code qu'il vous a donné.") }
    static var notInstalled: String { String(localized: "Pas encore installé") }
    static var notInstalledText: String { String(localized: "Téléchargez KyberNow Station sur l'ordinateur à rejoindre.") }
    /// Le lien de téléchargement, tel qu'il s'écrit (pas traduit).
    static var siteLabel: String { "www.kybernow.app" }

    /// Réglages de l'app.
    static var replay: String { String(localized: "Revoir l'accueil") }

    // La carte du compte
    static var cardLabel: String { String(localized: "Compte KyberNow") }
    static var cardTitle: String { String(localized: "Retrouvez vos ordinateurs partout") }
    static var cardText: String { String(localized: "Sur tous vos appareils, même hors de chez vous.") }
    static var close: String { String(localized: "Fermer") }
    static var neverAgain: String { String(localized: "Ne plus afficher") }

    // Hors de chez vous
    static var awayTitle: String { String(localized: "Hors de chez vous ?") }
    static func awayText(_ name: String) -> String {
        String(localized: "\(name) n'est pas sur ce réseau. Pour le rejoindre d'où vous êtes, connectez un compte KyberNow : votre ordinateur vous suit partout, sans rien régler sur votre box.")
    }
    static var notNow: String { String(localized: "Pas maintenant") }

    // Après un appairage
    static var pairedText: String { String(localized: "Retrouvez-le aussi hors de chez vous et sur vos autres appareils : connectez un compte KyberNow.") }
}

// MARK: - L'état

@MainActor
final class WelcomeStore: ObservableObject {
    enum Page: Int, CaseIterable {
        case intro = 1, account, computer
    }

    /// L'état du cœur (`WelcomeState`, JSON), gardé dans les préférences.
    @Published private(set) var state: WelcomeCore.State
    /// L'accueil est à l'écran.
    @Published private(set) var showing = false
    @Published var page: Page = .intro
    /// L'ordinateur qui vient d'être appairé : son bandeau, sous ses boutons.
    @Published private(set) var pairedBanner: UUID?
    /// Appairé, son bandeau pas encore montré : il attend que la place sous
    /// les boutons soit libre (la mise à jour de la Station passe avant).
    @Published private(set) var pairedPending: UUID?
    /// « Hors de chez vous ? » pour cet ordinateur.
    @Published var away: Host?
    /// Un avis bref (« Créez votre compte dans le navigateur… ») : l'app le montre.
    var onNotice: ((String) -> Void)?

    private unowned let accounts: AccountStore
    private let defaults: UserDefaults
    /// Démonstration : rien n'est gardé ; l'accueil, la carte et les moments
    /// seulement quand on les demande (`-demoWelcome`, `-demoAccountCard`…).
    private let demo: Bool
    private var demoCard = false
    private var bag: Set<AnyCancellable> = []
    private static let key = "welcomeState"

    init(accounts: AccountStore, defaults: UserDefaults = .standard, demo: Bool = false) {
        self.accounts = accounts
        self.defaults = defaults
        self.demo = demo
        state = demo ? WelcomeCore.done("") : defaults.string(forKey: Self.key) ?? ""
        // Un compte connecté (d'ici ou d'ailleurs) : l'écran 2 mène à l'accueil,
        // le bandeau et la fenêtre « Hors de chez vous ? » n'ont plus lieu d'être.
        accounts.$accounts
            .map(\.isEmpty)
            .removeDuplicates()
            .dropFirst()
            .sink { [weak self] empty in if !empty { self?.accountConnected() } }
            .store(in: &bag)
    }

    var hasAccount: Bool { !accounts.accounts.isEmpty }

    private func save() {
        guard !demo else { return }
        defaults.set(state, forKey: Self.key)
    }

    // MARK: L'accueil

    /// Au lancement : l'accueil s'il n'a pas été vu dans cette version, sans compte.
    func showIfNeeded() {
        guard !showing, WelcomeCore.shouldShow(state, hasAccount: hasAccount) else { return }
        page = .intro
        showing = true
    }

    /// « Revoir l'accueil » (Réglages de l'app).
    func replay() {
        page = .intro
        showing = true
    }

    /// L'écran suivant ; après le dernier, l'accueil est vu.
    func next() {
        if let n = Page(rawValue: page.rawValue + 1) { page = n } else { finish() }
    }

    /// « Passer », « Plus tard », la fin : l'accueil est vu.
    func finish() {
        state = WelcomeCore.done(state)
        save()
        showing = false
    }

    private func accountConnected() {
        if showing, page == .account { finish() }
        pairedBanner = nil
        pairedPending = nil
        away = nil
    }

    // MARK: Le compte

    /// Le relais montré dans l'aide de l'écran 2.
    var relayLabel: String { Accounts.defaultRelay }

    #if os(iOS) || os(macOS)
    /// « Connecter un compte » : la connexion de toujours (fenêtre du système).
    func connect() {
        accounts.signIn()
    }

    /// « Créer un compte » : la page d'inscription du relais dans le navigateur, puis l'avis.
    func createAccount() {
        guard let url = WelcomeCore.signUpURL() else { return }
        #if os(macOS)
        NSWorkspace.shared.open(url)
        #else
        UIApplication.shared.open(url)
        #endif
        onNotice?(WelcomeText.createdNotice)
    }

    /// Le lien « www.kybernow.app » de l'écran 3.
    func openStationDownload() {
        guard let url = WelcomeCore.stationDownloadURL else { return }
        #if os(macOS)
        NSWorkspace.shared.open(url)
        #else
        UIApplication.shared.open(url)
        #endif
    }
    #endif

    // MARK: La carte du compte

    var cardVisible: Bool {
        (!demo || demoCard) && WelcomeCore.cardVisible(state, hasAccount: hasAccount)
    }

    /// La croix : « Ne plus afficher » (vrai) ou « Fermer ».
    var cardCloseIsFinal: Bool { WelcomeCore.cardCloseIsFinal(state) }

    func closeCard() {
        state = WelcomeCore.closeCard(state)
        save()
    }

    // MARK: Les moments utiles

    /// Un ordinateur vient d'être appairé sur cet appareil (sans compte) :
    /// son bandeau attend d'être montré (`revealPaired`).
    func paired(_ host: Host) {
        guard !hasAccount else { return }
        pairedPending = host.id
    }

    /// Un seul bandeau à la fois sous la rangée « Se connecter » : la mise à
    /// jour de la Station (`stationBanner`) passe avant. Le bandeau après un
    /// appairage n'est compté comme vu (`take_paired_hint`) que quand il
    /// s'affiche vraiment. Appelé par l'accueil de l'ordinateur en scène.
    func revealPaired(_ host: Host, stationBanner: Bool) {
        guard pairedPending == host.id, !stationBanner else { return }
        pairedPending = nil
        var s = state
        let take = WelcomeCore.takePairedHint(&s, hasAccount: hasAccount, computer: host.id.uuidString)
        state = s
        save()
        if take { pairedBanner = host.id }
    }

    /// Le bandeau de cet ordinateur est à montrer (jamais avec celui de la Station).
    func showsPairedBanner(_ host: Host, stationBanner: Bool) -> Bool {
        !stationBanner && pairedBanner == host.id && !hasAccount
    }

    /// « Plus tard » ou « Connecter un compte » sur le bandeau.
    func dismissPairedBanner() {
        pairedBanner = nil
    }

    /// Un ordinateur appairé ne répond pas sur ce réseau et n'a pas de route
    /// par le relais : « Hors de chez vous ? », une fois par jour au plus.
    /// Vrai si la fenêtre s'ouvre (l'appelant ne fait rien d'autre).
    @discardableResult
    func offerAway(_ host: Host, status: HostStatus?) -> Bool {
        guard host.account == nil, status?.online == false, Relay.mode == .off || host.relayRoute == nil else { return false }
        var s = state
        let take = WelcomeCore.takeAwayHint(&s, hasAccount: hasAccount, computer: host.id.uuidString)
        state = s
        save()
        if take { away = host }
        return take
    }

    // MARK: Démonstration

    /// `-demoWelcome 1|2|3` (l'accueil à cet écran), `-demoAccountCard YES`
    /// (la carte du compte ; `final` : revenue, « Ne plus afficher »),
    /// `-demoPairedHint YES` (le bandeau sous l'ordinateur choisi),
    /// `-demoAway YES` (« Hors de chez vous ? » pour l'ordinateur choisi).
    func applyDemo(selected: Host?) {
        guard demo else { return }
        let args = UserDefaults.standard
        if let card = args.string(forKey: "demoAccountCard") {
            demoCard = true
            state = ""
            if card == "final" {
                state = WelcomeCore.closeCard(state, now: Date().addingTimeInterval(-31 * 24 * 3600))
            }
            state = WelcomeCore.done(state)
        }
        if let host = selected, args.bool(forKey: "demoPairedHint") { pairedBanner = host.id }
        if let host = selected, args.bool(forKey: "demoAway") { away = host }
        let n = args.integer(forKey: "demoWelcome")
        if let p = Page(rawValue: n) {
            page = p
            showing = true
        }
    }
}
