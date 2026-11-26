// L'accueil des apps Link et l'incitation au compte KyberNow (docs/link-ui.md,
// « Accueil et compte KyberNow ») : les règles du cœur (`welcome.rs`), par le
// pont `kn_welcome_*` (apps/apple/src/welcome.rs). L'app garde l'état en JSON
// (`WelcomeCore.State`) et le passe à chaque appel ; un état vide vaut une
// app neuve.

import Foundation
@_implementationOnly import CKyberNow

public enum WelcomeCore {
    /// L'état gardé par l'app (le JSON de `WelcomeState`).
    public typealias State = String

    private static func seconds(_ date: Date) -> UInt64 { UInt64(max(0, date.timeIntervalSince1970)) }

    /// Montrer l'accueil : jamais vu dans cette version, et sans compte.
    public static func shouldShow(_ state: State, hasAccount: Bool) -> Bool {
        kn_welcome_should_show(state, hasAccount)
    }

    /// L'accueil est vu (fini, « Passer », « Plus tard ») : le nouvel état.
    public static func done(_ state: State) -> State {
        Core.take(kn_welcome_done(state)) ?? state
    }

    /// La carte du compte à l'accueil.
    public static func cardVisible(_ state: State, hasAccount: Bool, now: Date = Date()) -> Bool {
        kn_welcome_card_visible(state, hasAccount, seconds(now))
    }

    /// La croix de la carte dit « Ne plus afficher » (sinon « Fermer »).
    public static func cardCloseIsFinal(_ state: State) -> Bool {
        kn_welcome_card_close_is_final(state)
    }

    /// La croix de la carte : le nouvel état.
    public static func closeCard(_ state: State, now: Date = Date()) -> State {
        Core.take(kn_welcome_close_card(state, seconds(now))) ?? state
    }

    /// Le bandeau après l'appairage de `computer` : vrai une fois par ordinateur.
    public static func takePairedHint(_ state: inout State, hasAccount: Bool, computer: String) -> Bool {
        var out: UnsafeMutablePointer<CChar>?
        let take = kn_welcome_take_paired_hint(state, hasAccount, computer, &out)
        if let next = Core.take(out) { state = next }
        return take
    }

    /// La fenêtre « Hors de chez vous ? » pour `computer` : une fois par jour au plus.
    public static func takeAwayHint(_ state: inout State, hasAccount: Bool, computer: String, now: Date = Date()) -> Bool {
        var out: UnsafeMutablePointer<CChar>?
        let take = kn_welcome_take_away_hint(state, hasAccount, computer, seconds(now), &out)
        if let next = Core.take(out) { state = next }
        return take
    }

    /// La page d'inscription du relais (vide : le relais public), dans la
    /// langue de l'app : à ouvrir dans le navigateur de l'appareil.
    public static func signUpURL(relay: String = "") -> URL? {
        Core.take(kn_welcome_sign_up_url(relay)).flatMap(URL.init(string:))
    }

    /// La page de téléchargement de KyberNow Station.
    public static var stationDownloadURL: URL? {
        Core.take(kn_welcome_station_download_url()).flatMap(URL.init(string:))
    }
}
