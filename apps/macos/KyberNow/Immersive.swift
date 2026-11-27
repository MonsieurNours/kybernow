import AppKit
import ApplicationServices
import KyberKit

/// Mode immersif (docs/link-ui.md, « Mode immersif ») : tout le clavier va à
/// l'ordinateur, raccourcis du Mac compris (⌘ ⇥, ⌘ Espace, ⌃ ← →…).
///
/// Le Mac traite ces raccourcis avant l'app : seul un crochet de session
/// (`CGEvent.tapCreate`, au niveau de la session) les voit passer avant le
/// Dock et Spotlight. Il exige l'autorisation Accessibilité. Le crochet est
/// posé tant qu'une session pourrait en avoir besoin ; à chaque touche, il
/// demande aux sessions laquelle a le clavier en mode immersif, et la lui
/// donne directement (l'événement ne va plus au système). Les raccourcis de
/// l'app (⌃⌥⌘ + lettre) passent toujours.
@MainActor
final class ImmersiveKeyboard {
    static let shared = ImmersiveKeyboard()

    /// La vue qui prend le clavier en mode immersif en ce moment (nil : aucune).
    var target: () -> StreamView? = { nil }
    /// Une session pourrait vouloir le mode (réglage autre que « Jamais »).
    var anySessionWants: () -> Bool = { false }

    private var tap: CFMachPort?
    private var source: CFRunLoopSource?
    /// Touches parties en mode immersif : leur relâchement va à la même vue,
    /// même si elle a perdu le clavier entre-temps.
    private var held: [UInt16: WeakView] = [:]
    /// Les vues dont on a posé `immersive` (pour le reposer à faux).
    private var marked: [WeakView] = []
    /// L'autorisation n'est demandée qu'une fois par lancement.
    private var asked = false
    private var observers: [NSObjectProtocol] = []

    private struct WeakView { weak var view: StreamView? }

    private init() {
        // Ce qui peut allumer le mode (premier plan, plein écran) ; au retour
        // des Réglages Système, l'autorisation vient peut-être d'être donnée.
        // Au second plan, le crochet est retiré : les frappes des autres apps
        // ne passent plus par le fil principal de KyberNow.
        let names: [Notification.Name] = [NSApplication.didBecomeActiveNotification, NSApplication.didResignActiveNotification,
                                          NSWindow.didBecomeKeyNotification,
                                          NSWindow.didEnterFullScreenNotification, NSWindow.didExitFullScreenNotification]
        observers = names.map { name in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { ImmersiveKeyboard.shared.check() }
            }
        }
    }

    /// L'app a le droit de prendre les raccourcis du Mac.
    static var trusted: Bool { AXIsProcessTrusted() }

    /// Ouvre Réglages Système › Confidentialité et sécurité › Accessibilité.
    static func openSettings() {
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility") {
            NSWorkspace.shared.open(url)
        }
    }

    /// À chaque changement qui peut allumer le mode : si une session le
    /// voudrait maintenant (fenêtre au premier plan, plein écran selon le
    /// réglage) sans l'autorisation, l'app la demande, une fois par
    /// lancement (fenêtre du système) ; puis le crochet suit.
    func check() {
        if !asked, !Self.trusted, target() != nil {
            asked = true
            let prompt = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
            _ = AXIsProcessTrustedWithOptions([prompt: true] as CFDictionary)
        }
        refresh()
    }

    /// Pose ou retire le crochet : posé tant qu'une session pourrait vouloir
    /// le mode, que l'app est au premier plan et autorisée.
    func refresh() {
        let want = NSApp.isActive && anySessionWants() && Self.trusted
        if want, tap == nil { install() }
        if !want, tap != nil { remove() }
        if tap == nil { unmarkAll() }
    }

    private func install() {
        let mask = (1 << CGEventType.keyDown.rawValue) | (1 << CGEventType.keyUp.rawValue) | (1 << CGEventType.flagsChanged.rawValue)
        guard let tap = CGEvent.tapCreate(tap: .cgSessionEventTap, place: .headInsertEventTap, options: .defaultTap,
                                          eventsOfInterest: CGEventMask(mask), callback: immersiveTapCallback, userInfo: nil) else {
            KyberLog.app.error("mode immersif : crochet clavier refusé")
            return
        }
        let source = CFMachPortCreateRunLoopSource(nil, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        self.tap = tap
        self.source = source
        KyberLog.app.notice("mode immersif : crochet clavier posé")
    }

    private func remove() {
        if let tap { CGEvent.tapEnable(tap: tap, enable: false) }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        tap = nil
        source = nil
        held.removeAll()
        KyberLog.app.notice("mode immersif : crochet clavier retiré")
    }

    private func unmarkAll() {
        for m in marked { m.view?.immersive = false }
        marked.removeAll()
    }

    /// Le système a coupé le crochet (trop lent, ou saisie sécurisée) : on le relance.
    fileprivate func reenable() {
        if let tap { CGEvent.tapEnable(tap: tap, enable: true) }
    }

    /// Vrai : l'événement est pris (il ne va plus au système).
    fileprivate func handle(_ type: CGEventType, _ cg: CGEvent) -> Bool {
        let code = UInt16(cg.getIntegerValueField(.keyboardEventKeycode))
        let target = self.target()
        // Les autres vues ne sont plus en mode immersif.
        for m in marked where m.view !== target { m.view?.immersive = false }
        marked.removeAll { $0.view == nil || $0.view !== target }
        // Un modificateur : appui ou relâchement d'après son bit (gauche et
        // droite à part), jamais d'après l'alternance (un ⌘ appuyé dans une
        // autre app puis relâché ici n'est pas un appui).
        let press: Bool
        switch type {
        case .keyDown: press = true
        case .keyUp: press = false
        default: press = SessionRules.modifierPressed(code: code, flags: cg.flags.rawValue) ?? (held[code] == nil)
        }
        // Un relâchement va à la vue qui a pris l'appui ; un relâchement
        // dont l'appui n'est pas passé ici reste au système.
        if !press {
            guard let view = held.removeValue(forKey: code)?.view else { return false }
            if let event = NSEvent(cgEvent: cg) { view.deliverImmersive(event) }
            return true
        }
        guard let view = target else { return false }
        // ⌃⌥⌘ + lettre : les raccourcis de l'app passent par ses menus.
        let flags = cg.flags
        if type != .flagsChanged, flags.contains(.maskControl), flags.contains(.maskAlternate), flags.contains(.maskCommand) {
            return false
        }
        if !view.immersive {
            view.immersive = true
            marked.append(WeakView(view: view))
        }
        guard let event = NSEvent(cgEvent: cg) else { return false }
        // Verr. Maj et fn basculent : pas de relâchement à attendre.
        if SessionRules.modifierPressed(code: code, flags: cg.flags.rawValue) != nil || type == .keyDown { held[code] = WeakView(view: view) }
        view.deliverImmersive(event)
        return true
    }
}

private func immersiveTapCallback(proxy: CGEventTapProxy, type: CGEventType, event: CGEvent, userInfo: UnsafeMutableRawPointer?) -> Unmanaged<CGEvent>? {
    // Le crochet est sur la boucle principale : on y est déjà.
    MainActor.assumeIsolated {
        switch type {
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            ImmersiveKeyboard.shared.reenable()
            return Unmanaged.passUnretained(event)
        case .keyDown, .keyUp, .flagsChanged:
            return ImmersiveKeyboard.shared.handle(type, event) ? nil : Unmanaged.passUnretained(event)
        default:
            return Unmanaged.passUnretained(event)
        }
    }
}
