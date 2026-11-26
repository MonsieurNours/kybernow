// Enregistrement de la session (docs/enregistrement.md, textes dans
// docs/link-ui.md, « Enregistrement de la session ») : la Station écrit le
// fichier ; l'app le demande (hôte) et le montre (hôte et invités).
//
// L'état et ses textes viennent du cœur (`on_recording`, `kn_console_recording`) ;
// `{time}` se remplit chaque seconde (`clock`), `{shortcut}` par l'app.

import Foundation
@_implementationOnly import CKyberNow

/// L'enregistrement, tel que le dit le dernier signe de vie.
public struct RecordingState: Equatable, Sendable {
    /// La Station enregistre (sinon : ni bouton ni pastille).
    public let available: Bool
    /// Permis dans Admin (sinon : bouton estompé, `texts.disabled` en raison).
    public let allowed: Bool
    public let active: Bool
    /// Secondes écoulées à `received` (temps de la Station).
    let elapsed: Int
    let received: Date
    public let texts: Texts

    public struct Texts: Equatable, Sendable {
        public let record, pill, recordTooltip, stopTooltip, stop, onComputer: String
        public let recordMenu, stopMenu, guestPill, guestTooltip, disabled: String
    }

    public init?(json: String?) {
        guard let json, let data = json.data(using: .utf8),
              let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        let t = o["texts"] as? [String: Any] ?? [:]
        func s(_ k: String) -> String { t[k] as? String ?? "" }
        available = o["available"] as? Bool ?? false
        allowed = o["allowed"] as? Bool ?? false
        active = o["active"] as? Bool ?? false
        elapsed = (o["elapsed"] as? NSNumber)?.intValue ?? 0
        received = Date()
        texts = Texts(record: s("record"), pill: s("pill"), recordTooltip: s("record_tooltip"),
                      stopTooltip: s("stop_tooltip"), stop: s("stop"), onComputer: s("on_computer"),
                      recordMenu: s("record_menu"), stopMenu: s("stop_menu"), guestPill: s("guest_pill"),
                      guestTooltip: s("guest_tooltip"), disabled: s("disabled"))
    }

    /// Le même état (le temps qui passe n'en change pas).
    public static func == (a: Self, b: Self) -> Bool {
        a.available == b.available && a.allowed == b.allowed && a.active == b.active && a.texts == b.texts
    }

    /// Secondes écoulées maintenant.
    public func seconds(at now: Date = Date()) -> Int {
        active ? elapsed + max(0, Int(now.timeIntervalSince(received))) : 0
    }

    /// « 02:14 », « 1:02:14 » (`kn_recording_clock`).
    public func clock(at now: Date = Date()) -> String {
        Core.take(kn_recording_clock(UInt64(seconds(at: now)))) ?? ""
    }

    private func fill(_ text: String, _ now: Date, _ shortcut: String = "") -> String {
        text.replacingOccurrences(of: "{time}", with: clock(at: now)).replacingOccurrences(of: "{shortcut}", with: shortcut)
    }

    /// Le bouton de l'hôte : « Enregistrer », ou « Enregistrement · 02:14 ».
    public func label(at now: Date = Date()) -> String { active ? fill(texts.pill, now) : texts.record }
    /// Son infobulle (`shortcut` : « ⌃⌥⌘R ») ; coupé dans Admin : la raison.
    public func tooltip(shortcut: String) -> String {
        if !allowed && !active { return texts.disabled }
        return fill(active ? texts.stopTooltip : texts.recordTooltip, Date(), shortcut)
    }
    /// Menus à tuiles : titre et sous-titre.
    public func tile(at now: Date = Date()) -> (title: String, subtitle: String) {
        active ? (texts.stop, clock(at: now)) : (texts.record, texts.onComputer)
    }
    /// Menu de l'icône : « Enregistrer la session », « Arrêter l'enregistrement (02:14) ».
    public func menuItem(at now: Date = Date()) -> String { active ? fill(texts.stopMenu, now) : texts.recordMenu }
}

/// Un refus de `kn_console_recording` : l'avis bref à montrer.
public struct RecordingFailure: LocalizedError {
    public let code: Int32
    public let message: String
    public var errorDescription: String? { message }
}

extension ConsoleClient {
    /// Lance (`record`) ou arrête l'enregistrement de la session de cet appareil.
    public func recording(deviceId: String, record: Bool) async throws -> RecordingState? {
        let text = try await call { console -> String? in
            var out: UnsafeMutablePointer<CChar>?
            var error: UnsafeMutablePointer<CChar>?
            let code = deviceId.withCString { kn_console_recording(console, $0, record, &out, &error) }
            if code != KN_OK {
                throw RecordingFailure(code: code, message: Core.take(error) ?? "")
            }
            return Core.take(out)
        }
        return RecordingState(json: text)
    }
}
