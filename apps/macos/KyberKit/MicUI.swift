// Le micro d'une session tel que l'interface le montre, tenu par le cœur
// (`micro::MicUi`, pont apps/apple/src/micro.rs ; docs/link-ui.md, « Micro ») :
// puce allumée ou estompée, sous-titre de la tuile (« Envoyé », « Envoi… »
// tant que l'ordinateur n'a pas répondu, « Coupé »), infobulle, raison d'un
// refus, avis « Micro envoyé à {nom}. ». Les textes viennent du catalogue du
// cœur. Mac, iPhone et iPad (l'Apple TV n'a pas de micro).

import Foundation
@_implementationOnly import CKyberNow

/// Ce que montrent la puce et la tuile « Micro ».
public struct MicView: Equatable, Sendable {
    /// Allumé par l'utilisateur (le choix gardé pour l'ordinateur).
    public var on = false
    /// `off`, `waiting`, `on`, `refused`.
    public var state = "off"
    /// La puce est allumée : allumé et pas refusé.
    public var lit = false
    /// Le son part : l'icône de micro près de l'indicateur de la session.
    public var sent = false
    /// La raison quand l'ordinateur ne le prend pas (puce estompée, infobulle, avis).
    public var reason: String?
    /// « Micro » ou « Micro coupé ».
    public var label = ""
    /// « Micro : envoyé à l'ordinateur (⌃⌥⌘V) », « Micro coupé (⌃⌥⌘V) », ou la raison.
    public var tooltip = ""
    /// « Envoyé », « Envoi… », « Coupé ».
    public var tile = ""
    /// La puce a sa place (pas pour un invité que l'hôte ne laisse pas parler).
    public var shown = true
    /// L'app capte (allumé, pris ou en attente de la réponse).
    public var capturing = false

    public init() {}
}

/// Le micro d'une session (une poignée du cœur par session).
public final class MicUI {
    /// Un appui sur la puce ou la tuile.
    public enum Tap: Sendable { case turnOn, turnOff, refused }

    private let handle: OpaquePointer?

    /// `on` : le choix gardé pour cet ordinateur.
    public init(on: Bool) {
        handle = kn_mic_ui_new(on)
    }

    deinit { kn_mic_ui_free(handle) }

    /// L'utilisateur l'allume ou le coupe (à donner aussi à la session).
    public func set(_ on: Bool) { kn_mic_ui_set(handle, on) }

    /// Le système refuse l'accès au micro : coupé (l'app dit sa raison).
    public func denied() { kn_mic_ui_denied(handle) }

    /// Une réponse de l'ordinateur ; l'avis à montrer (« Micro envoyé à
    /// {nom}. », une fois, quand il est pris après avoir été allumé).
    public func changed(_ state: MicState, computer: String) -> String? {
        guard let p = kn_mic_ui_changed(handle, state.json, computer) else { return nil }
        defer { kn_string_free(p) }
        return String(cString: p)
    }

    /// Ce que fait un appui : l'allumer, le couper, ou dire la raison du refus.
    public func tap() -> Tap {
        switch kn_mic_ui_tap(handle) {
        case Int32(KN_MIC_TAP_ON): return .turnOn
        case Int32(KN_MIC_TAP_REFUSED): return .refused
        default: return .turnOff
        }
    }

    /// Ce que montre l'interface, dans la langue de l'app. `shortcut` : celui
    /// de l'app (« ⌃⌥⌘V » sur Mac), vide sans ; `guest` : un invité d'un partage.
    public func view(shortcut: String, guest: Bool) -> MicView {
        guard let p = kn_mic_ui_json(handle, shortcut, guest) else { return MicView() }
        defer { kn_string_free(p) }
        guard let j = (try? JSONSerialization.jsonObject(with: Data(String(cString: p).utf8))) as? [String: Any] else { return MicView() }
        var v = MicView()
        v.on = j["on"] as? Bool ?? false
        v.state = j["state"] as? String ?? "off"
        v.lit = j["lit"] as? Bool ?? false
        v.sent = j["sent"] as? Bool ?? false
        v.reason = j["reason"] as? String
        v.label = j["label"] as? String ?? ""
        v.tooltip = j["tooltip"] as? String ?? ""
        v.tile = j["tile"] as? String ?? ""
        v.shown = j["shown"] as? Bool ?? true
        v.capturing = j["capturing"] as? Bool ?? false
        return v
    }
}

extension MicState {
    /// Le JSON du cœur (`MicState::to_json`), pour `MicUI.changed`.
    var json: String {
        switch self {
        case .off: return #"{"state":"off"}"#
        case .waiting: return #"{"state":"waiting"}"#
        case .on: return #"{"state":"on"}"#
        case let .refused(reason, _):
            let r = (try? JSONSerialization.data(withJSONObject: ["state": "refused", "reason": reason])).flatMap { String(data: $0, encoding: .utf8) }
            return r ?? #"{"state":"refused"}"#
        }
    }
}
