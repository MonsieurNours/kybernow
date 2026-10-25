// La manette à l'écran (docs/link-ui.md, « Au toucher ») : une manette Xbox
// fixe par-dessus l'image, pour les deux pouces en paysage, en disposition
// Complète ou Simple. Bouton de la barre de session, retenu pour chaque
// ordinateur ; disposition, taille, opacité et retour haptique dans
// Commandes. L'ordinateur la voit comme une manette Xbox 360, à la première
// place de joueur libre (GamepadBridge) ; tant qu'elle est affichée, le reste
// de l'écran ne sert pas de souris (StreamView.touchAsMouse).
//
// UIKit plutôt que SwiftUI : chaque doigt est suivi (deux sticks et des
// boutons en même temps), un stick qui bouge à 120 Hz ne fait que déplacer un
// calque, et le premier appui part vers l'ordinateur sans attendre un rendu
// de SwiftUI. Pas de flou derrière les commandes : un verre sombre sans effet
// sur l'image en direct, lisible sur n'importe quelle image.
//
// Retour : chaque appui s'allume (signal Spectre) et s'enfonce légèrement ;
// le retour haptique suit PadTick (ScreenPadLogic.swift). Les vibrations du
// jeu font vibrer l'appareil par le pont des manettes.

import CoreHaptics
import KyberKit
import SwiftUI
import UIKit

// MARK: - Libellés

extension ScreenPadLayout {
    var label: String { self == .full ? String(localized: "Complète") : String(localized: "Simple") }
}

extension ScreenPadSize {
    var label: String {
        switch self {
        case .small: return String(localized: "Petite")
        case .medium: return String(localized: "Moyenne")
        case .large: return String(localized: "Grande")
        }
    }
}

extension ScreenPadHaptics {
    var label: String {
        switch self {
        case .off: return String(localized: "Désactivé")
        case .light: return String(localized: "Faible")
        case .medium: return String(localized: "Moyen")
        case .strong: return String(localized: "Fort")
        }
    }
}

enum ScreenPadText {
    static var name: String { String(localized: "Manette à l'écran") }
}

// MARK: - Retour haptique

/// Tics de la manette à l'écran : un moteur Core Haptics démarré à
/// l'affichage, et un lecteur préparé pour chaque tic, joué tout de suite ;
/// à défaut, `UIImpactFeedbackGenerator`. Des événements transitoires
/// seulement, jamais de vibration continue. Core Haptics passe par une file à
/// lui (ses appels sont synchrones vers le service haptique), jamais par le
/// fil principal où arrivent les touchers.
final class PadHaptics {
    /// L'iPad n'a pas de moteur haptique : seul le retour visuel reste.
    static let available: Bool = {
        #if DEBUG && targetEnvironment(simulator)
        // Démonstration : le simulateur n'en a pas, l'iPhone si.
        if UserDefaults.standard.bool(forKey: "demo") { return !DeviceDisplay.isPad }
        #endif
        return CHHapticEngine.capabilitiesForHardware().supportsHaptics
    }()

    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.pad-haptics", qos: .userInteractive)
    // Sur la file seulement.
    private var engine: CHHapticEngine?
    private var running = false
    private var players: [PadTick: CHHapticPatternPlayer] = [:]
    private var gain: Float = 0.72
    // Fil principal.
    private var level: ScreenPadHaptics = .medium
    private var useFallback = false
    private var generators: [UIImpactFeedbackGenerator.FeedbackStyle: UIImpactFeedbackGenerator] = [:]

    /// Intensité et netteté de chaque tic, au réglage Fort.
    private static func base(_ tick: PadTick) -> (intensity: Float, sharpness: Float) {
        switch tick {
        case .press: return (1, 0.7)
        case .triggerStart: return (0.45, 0.45)
        case .triggerEnd: return (1, 0.95)
        case .stickLeave: return (0.55, 0.6)
        case .stickSector: return (0.22, 0.35)
        case .stickEdge: return (0.75, 0.85)
        }
    }

    private static func gain(_ level: ScreenPadHaptics) -> Float {
        switch level {
        case .off: return 0
        case .light: return 0.45
        case .medium: return 0.72
        case .strong: return 1
        }
    }

    func setLevel(_ level: ScreenPadHaptics) {
        guard level != self.level else { return }
        self.level = level
        let gain = Self.gain(level)
        queue.async {
            self.gain = gain
            self.players = [:]
            if self.running { self.prepare() }
        }
    }

    /// Manette affichée : le moteur démarre, les tics sont prêts.
    func start() {
        guard Self.available, level != .off else { return }
        queue.async { self.startEngine() }
        prepareFallback()
    }

    /// Manette retirée : le moteur s'arrête.
    func stop() {
        queue.async {
            self.players = [:]
            if self.running { self.engine?.stop(completionHandler: nil) }
            self.running = false
        }
    }

    func play(_ tick: PadTick) {
        guard level != .off, Self.available else { return }
        if useFallback {
            let style = Self.style(tick)
            let g = generators[style] ?? UIImpactFeedbackGenerator(style: style)
            generators[style] = g
            g.impactOccurred(intensity: CGFloat(Self.base(tick).intensity * Self.gain(level)))
            g.prepare()
            return
        }
        queue.async {
            if !self.running { self.startEngine() }
            if self.players[tick] == nil { self.prepare() }
            try? self.players[tick]?.start(atTime: CHHapticTimeImmediate)
        }
    }

    private func startEngine() {
        do {
            let engine = try self.engine ?? CHHapticEngine()
            if self.engine == nil {
                engine.playsHapticsOnly = true
                engine.isAutoShutdownEnabled = false
                engine.stoppedHandler = { [weak self] _ in self?.queue.async { self?.running = false } }
                // Serveur haptique relancé : les lecteurs ne valent plus rien.
                engine.resetHandler = { [weak self] in
                    self?.queue.async {
                        guard let self else { return }
                        self.players = [:]
                        self.running = false
                        self.startEngine()
                    }
                }
                self.engine = engine
            }
            try engine.start()
            running = true
            prepare()
        } catch {
            KyberLog.app.error("manette à l'écran : retour haptique indisponible (\(error.localizedDescription, privacy: .public))")
            DispatchQueue.main.async { self.useFallback = true; self.prepareFallback() }
        }
    }

    private func prepare() {
        guard let engine else { return }
        for tick in [PadTick.press, .triggerStart, .triggerEnd, .stickLeave, .stickSector, .stickEdge] {
            let b = Self.base(tick)
            let event = CHHapticEvent(eventType: .hapticTransient, parameters: [
                CHHapticEventParameter(parameterID: .hapticIntensity, value: b.intensity * gain),
                CHHapticEventParameter(parameterID: .hapticSharpness, value: b.sharpness),
            ], relativeTime: 0)
            players[tick] = try? engine.makePlayer(with: CHHapticPattern(events: [event], parameters: []))
        }
    }

    private static func style(_ tick: PadTick) -> UIImpactFeedbackGenerator.FeedbackStyle {
        switch tick {
        case .press, .triggerEnd, .stickEdge: return .rigid
        case .triggerStart, .stickLeave: return .light
        case .stickSector: return .soft
        }
    }

    private func prepareFallback() {
        guard useFallback else { return }
        for style in [UIImpactFeedbackGenerator.FeedbackStyle.rigid, .light, .soft] {
            let g = generators[style] ?? UIImpactFeedbackGenerator(style: style)
            generators[style] = g
            g.prepare()
        }
    }
}

// MARK: - La vue

final class ScreenPadView: UIView {
    var layout: ScreenPadLayout = .full {
        didSet { if layout != oldValue { layoutChanged() } }
    }
    /// Taille choisie (Petite, Moyenne, Grande).
    var size: ScreenPadSize = .medium {
        didSet { if size != oldValue { setNeedsLayout() } }
    }
    /// Opacité du dessin au repos (0,2 à 1) ; un appui s'affiche toujours franc.
    var padOpacity: CGFloat = 0.6 {
        didSet { if padOpacity != oldValue { refreshAll() } }
    }
    var haptics: ScreenPadHaptics = .medium {
        didSet { feedback.setLevel(haptics) }
    }
    /// La barre de session reste ouverte (pupitre) : la manette passe dessous.
    var topReserved: CGFloat = 0 {
        didSet { if topReserved != oldValue { setNeedsLayout() } }
    }
    /// Le petit bouton de la manette : passer à l'autre disposition.
    var onToggleLayout: (() -> Void)?

    private let bridge = GamepadBridge.shared
    private let feedback = PadHaptics()
    private var geometry: PadGeometry?
    private var layers: [PadControl: ControlLayers] = [:]
    private var toggleElement: ToggleElement?

    private var visible: Set<PadControl> {
        layout == .simple ? Set(PadControl.allCases.filter(\.inSimple)) : Set(PadControl.allCases)
    }

    // État des doigts et des commandes (fil principal).
    private struct Track {
        let control: PadControl
        let began: TimeInterval
        /// A, B, X ou Y sous le pouce (il glisse de l'un à l'autre).
        var face: PadControl?
        var maxDeviation = 0.0
        /// L3 ou R3 tenu par ce doigt.
        var clicked = false
        var dpad: Set<PadDirection> = []
        var dpadSector: Int?
    }
    private var tracks: [ObjectIdentifier: (touch: UITouch, track: Track)] = [:]
    /// Appuis par touche : deux doigts sur le même bouton n'en font qu'un.
    private var held: [PadButton: Int] = [:]
    private var stickFeel: [PadControl: StickFeel] = [.leftStick: StickFeel(), .rightStick: StickFeel()]
    private var stickRaw: [PadControl: (x: Double, y: Double)] = [:]
    private var stickTouched: Set<PadControl> = []
    private var lastStickTap: [PadControl: TimeInterval] = [:]
    private var triggerFeel: [PadControl: TriggerFeel] = [.lt: TriggerFeel(), .rt: TriggerFeel()]
    private var triggerValue: [PadControl: Double] = [:]
    private var togglePressed = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        isMultipleTouchEnabled = true
        backgroundColor = .clear
        NotificationCenter.default.addObserver(self, selector: #selector(appLeft), name: UIApplication.willResignActiveNotification, object: nil)
        #if DEBUG
        demoPressed = Set((UserDefaults.standard.string(forKey: "demoPadPress") ?? "").split(separator: ",").map(String.init))
        #endif
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError() }

    deinit { NotificationCenter.default.removeObserver(self) }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window != nil {
            feedback.start()
            setNeedsLayout()
        } else {
            releaseEverything()
            feedback.stop()
        }
    }

    override func safeAreaInsetsDidChange() {
        super.safeAreaInsetsDidChange()
        setNeedsLayout()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let insets = window?.safeAreaInsets ?? safeAreaInsets
        let factor = PadGeometry.factor(size.rawValue) * (DeviceDisplay.isPad ? 1.1 : 1)
        let next = PadGeometry.make(size: bounds.size, insets: PadInsets(top: insets.top, left: insets.left, bottom: insets.bottom, right: insets.right),
                                    factor: factor, compactHeight: bounds.height < 600, topReserved: topReserved)
        guard next != geometry else { return }
        // La manette change de forme (rotation, taille) : rien ne reste tenu.
        releaseEverything()
        geometry = next
        rebuildLayers()
    }

    // MARK: Toucher

    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        geometry?.control(at: point, among: visible) != nil
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let geometry else { return }
        let now = CACurrentMediaTime()
        for touch in touches {
            let p = touch.location(in: self)
            guard let control = geometry.control(at: p, among: visible) else { continue }
            var track = Track(control: control, began: now)
            switch control {
            case .leftStick, .rightStick:
                stickTouched.insert(control)
                if PadStickClick.isClick(now: now, lastTap: lastStickTap[control]) {
                    track.clicked = true
                    press(control == .leftStick ? .leftStick : .rightStick)
                    feedback.play(.press)
                }
                lastStickTap[control] = nil
                moveStick(control, to: p, track: &track)
            case .dpad:
                moveDpad(to: p, track: &track)
            case .lt, .rt:
                moveTrigger(control, to: p)
            case .a, .b, .x, .y:
                track.face = control
                press(Self.button(control)!)
                feedback.play(.press)
            case .toggle:
                togglePressed = true
                feedback.play(.press)
            default:
                if let b = Self.button(control) { press(b) }
                feedback.play(.press)
            }
            tracks[ObjectIdentifier(touch)] = (touch, track)
            refresh(control)
        }
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) {
        guard let geometry else { return }
        for touch in touches {
            guard var entry = tracks[ObjectIdentifier(touch)] else { continue }
            let p = touch.location(in: self)
            switch entry.track.control {
            case .leftStick, .rightStick:
                moveStick(entry.track.control, to: p, track: &entry.track)
            case .dpad:
                moveDpad(to: p, track: &entry.track)
            case .lt, .rt:
                moveTrigger(entry.track.control, to: p)
            case .a, .b, .x, .y:
                let next = geometry.faceButton(at: p)
                if next != entry.track.face {
                    if let old = entry.track.face { release(Self.button(old)!); refresh(old) }
                    if let next {
                        press(Self.button(next)!)
                        feedback.play(.press)
                        refresh(next)
                    }
                    entry.track.face = next
                }
            default:
                break
            }
            tracks[ObjectIdentifier(touch)] = entry
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { end(touch, cancelled: false) }
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) {
        for touch in touches { end(touch, cancelled: true) }
    }

    private func end(_ touch: UITouch, cancelled: Bool) {
        guard let entry = tracks.removeValue(forKey: ObjectIdentifier(touch)) else { return }
        let track = entry.track
        let control = track.control
        switch control {
        case .leftStick, .rightStick:
            if track.clicked { release(control == .leftStick ? .leftStick : .rightStick) }
            // Un toucher bref et immobile : le suivant, s'il vient vite, enfonce le stick.
            let duration = CACurrentMediaTime() - track.began
            lastStickTap[control] = !cancelled && !track.clicked && PadStickClick.isTap(duration: duration, maxDeviation: track.maxDeviation)
                ? CACurrentMediaTime() : nil
            if !tracks.values.contains(where: { $0.track.control == control }) { restStick(control) }
        case .dpad:
            for d in track.dpad { release(Self.button(d)) }
        case .lt, .rt:
            if !tracks.values.contains(where: { $0.track.control == control }) { setTrigger(control, 0) }
        case .a, .b, .x, .y:
            if let face = track.face { release(Self.button(face)!); refresh(face) }
        case .toggle:
            togglePressed = false
            if !cancelled, let item = geometry?.items[.toggle], item.hit.contains(touch.location(in: self)) {
                onToggleLayout?()
            }
        default:
            if let b = Self.button(control) { release(b) }
        }
        refresh(control)
    }

    // MARK: Commandes

    private func press(_ b: PadButton) {
        let n = held[b, default: 0]
        held[b] = n + 1
        if n == 0 { bridge.screenPadButton(b, pressed: true) }
    }

    private func release(_ b: PadButton) {
        let n = held[b, default: 0]
        guard n > 0 else { return }
        held[b] = n - 1
        if n == 1 {
            held[b] = nil
            bridge.screenPadButton(b, pressed: false)
        }
    }

    private func moveStick(_ control: PadControl, to p: CGPoint, track: inout Track) {
        guard let geometry, let item = geometry.items[control] else { return }
        let raw = PadStick.raw(offset: CGVector(dx: p.x - item.frame.midX, dy: p.y - item.frame.midY), travel: geometry.stickTravel)
        track.maxDeviation = max(track.maxDeviation, hypot(raw.x, raw.y))
        let v = PadStick.value(raw)
        bridge.screenPadStick(right: control == .rightStick, x: Float(v.x), y: Float(v.y))
        stickRaw[control] = raw
        for tick in stickFeel[control, default: StickFeel()].update(raw) { feedback.play(tick) }
        refresh(control)
    }

    private func restStick(_ control: PadControl) {
        bridge.screenPadStick(right: control == .rightStick, x: 0, y: 0)
        stickRaw[control] = nil
        stickFeel[control]?.reset()
        stickTouched.remove(control)
    }

    private func moveDpad(to p: CGPoint, track: inout Track) {
        guard let item = geometry?.items[.dpad] else { return }
        let (dirs, sector) = PadDpad.directions(offset: CGVector(dx: p.x - item.frame.midX, dy: p.y - item.frame.midY),
                                                half: item.frame.width / 2, previous: track.dpadSector)
        track.dpadSector = sector
        guard dirs != track.dpad else { return }
        for d in track.dpad.subtracting(dirs) { release(Self.button(d)) }
        let added = dirs.subtracting(track.dpad)
        for d in added { press(Self.button(d)) }
        if !added.isEmpty { feedback.play(.press) }
        track.dpad = dirs
        refresh(.dpad)
    }

    private func moveTrigger(_ control: PadControl, to p: CGPoint) {
        guard let item = geometry?.items[control] else { return }
        setTrigger(control, PadTrigger.value(along: Double((p.y - item.frame.minY) / max(item.frame.height, 1))))
    }

    private func setTrigger(_ control: PadControl, _ value: Double) {
        guard triggerValue[control, default: 0] != value else { return }
        triggerValue[control] = value
        bridge.screenPadTrigger(right: control == .rt, value: Float(value))
        for tick in triggerFeel[control, default: TriggerFeel()].update(value) { feedback.play(tick) }
        refresh(control)
    }

    /// Fin de l'affichage, arrière-plan, nouvelle forme : chaque doigt est
    /// lâché, chaque touche relâchée, sticks et gâchettes au repos.
    func releaseEverything() {
        for entry in Array(tracks.values) { end(entry.touch, cancelled: true) }
        tracks = [:]
        for (b, _) in held { bridge.screenPadButton(b, pressed: false) }
        held = [:]
        for stick in [PadControl.leftStick, .rightStick] where stickRaw[stick] != nil || stickTouched.contains(stick) { restStick(stick) }
        for trigger in [PadControl.lt, .rt] where triggerValue[trigger, default: 0] > 0 { setTrigger(trigger, 0) }
        lastStickTap = [:]
        togglePressed = false
        refreshAll()
    }

    @objc private func appLeft() { releaseEverything() }

    /// Disposition changée : les commandes qui disparaissent sont relâchées,
    /// les autres restent sous les doigts.
    private func layoutChanged() {
        let shown = visible
        for (key, entry) in tracks where !shown.contains(entry.track.control) {
            end(entry.touch, cancelled: true)
            tracks[key] = nil
        }
        refreshAll()
    }

    private static func button(_ c: PadControl) -> PadButton? {
        switch c {
        case .a: return .a
        case .b: return .b
        case .x: return .x
        case .y: return .y
        case .lb: return .leftShoulder
        case .rb: return .rightShoulder
        case .view: return .back
        case .menu: return .start
        case .guide: return .guide
        default: return nil
        }
    }

    private static func button(_ d: PadDirection) -> PadButton {
        switch d {
        case .up: return .dpadUp
        case .down: return .dpadDown
        case .left: return .dpadLeft
        case .right: return .dpadRight
        }
    }

    // MARK: Dessin

    #if DEBUG
    /// `-demoPadPress a,lt,dpad,leftStick` (captures) : commandes montrées appuyées.
    private var demoPressed: Set<String> = []
    #endif

    private func isPressed(_ c: PadControl) -> Bool {
        #if DEBUG
        if demoPressed.contains(c.rawValue) { return true }
        #endif
        switch c {
        case .toggle: return togglePressed
        case .leftStick, .rightStick: return stickTouched.contains(c)
        case .lt, .rt: return triggerValue[c, default: 0] > 0
        case .dpad: return !dpadPressed.isEmpty
        default: return Self.button(c).map { held[$0, default: 0] > 0 } ?? false
        }
    }

    private var dpadPressed: Set<PadDirection> {
        #if DEBUG
        if demoPressed.contains("dpad") { return [.up] }
        #endif
        return Set(PadDirection.allCases.filter { held[Self.button($0), default: 0] > 0 })
    }

    private var scale: CGFloat { geometry?.scale ?? 1 }

    private func rebuildLayers() {
        layers.values.forEach { $0.root.removeFromSuperlayer() }
        layers = [:]
        guard let geometry else {
            accessibilityElements = nil
            return
        }
        for (control, item) in geometry.items {
            let l = ControlLayers(control: control, frame: item.frame, scale: geometry.scale, screenScale: window?.screen.scale ?? 3)
            layer.addSublayer(l.root)
            layers[control] = l
        }
        let element = ToggleElement(accessibilityContainer: self)
        element.accessibilityTraits = .button
        element.action = { [weak self] in self?.onToggleLayout?() }
        if let item = geometry.items[.toggle] { element.accessibilityFrameInContainerSpace = item.hit }
        toggleElement = element
        accessibilityElements = [element]
        refreshAll()
    }

    private func refreshAll() {
        for c in PadControl.allCases { refresh(c) }
        toggleElement?.accessibilityLabel = layout == .full ? String(localized: "Manette simple") : String(localized: "Manette complète")
    }

    private func refresh(_ c: PadControl) {
        guard let l = layers[c] else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        guard visible.contains(c) else {
            l.root.isHidden = true
            return
        }
        l.root.isHidden = false
        let pressed = isPressed(c)
        // Un appui se voit toujours, même manette très transparente.
        l.root.opacity = Float(pressed ? max(padOpacity, 0.95) : padOpacity)
        switch c {
        case .leftStick, .rightStick:
            var raw = stickRaw[c] ?? (0, 0)
            #if DEBUG
            if demoPressed.contains(c.rawValue), stickRaw[c] == nil { raw = (0.55, 0.45) }
            #endif
            let clicked = tracks.values.contains { $0.track.control == c && $0.track.clicked }
            l.setStick(touched: pressed, clicked: clicked, raw: raw, travel: geometry?.stickTravel ?? 40)
        case .lt, .rt:
            var value = triggerValue[c, default: 0]
            #if DEBUG
            if demoPressed.contains(c.rawValue), value == 0 { value = 0.7 }
            #endif
            l.setTrigger(value)
        case .dpad:
            l.setDpad(dpadPressed)
        case .toggle:
            l.setToggle(simple: layout == .full, pressed: pressed)
        default:
            l.setPressed(pressed)
        }
    }
}

/// Le bouton de disposition pour VoiceOver.
private final class ToggleElement: UIAccessibilityElement {
    var action: (() -> Void)?
    override func accessibilityActivate() -> Bool {
        action?()
        return true
    }
}

// MARK: - Calques d'une commande

/// Couleurs de la charte (brand/v2) : encre, givre, signal Spectre.
private enum PadInk {
    static func rgb(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
        UIColor(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255, blue: CGFloat(hex & 0xFF) / 255, alpha: alpha).cgColor
    }
    /// Verre sombre : l'encre, assez transparente pour laisser l'image.
    static let plate = rgb(0x05070A, 0.58)
    static let knob = rgb(0x0E141A, 0.86)
    static let line = rgb(0xFFFFFF, 0.22)
    static let knobLine = rgb(0xFFFFFF, 0.38)
    static let frost = rgb(0xE3EDF0, 0.94)
    static let spectre = rgb(0x9A8CFF)
    static let spectreFill = rgb(0x6D5BFF, 0.92)
    static let spectreSoft = rgb(0x6D5BFF, 0.34)
    static let white = rgb(0xFFFFFF)
}

private final class ControlLayers {
    let control: PadControl
    let root = CALayer()
    private let plate = CAShapeLayer()
    private let glyph = CAShapeLayer()
    private var label: CATextLayer?
    private var icon: CALayer?
    // Stick.
    private var knob: CAShapeLayer?
    // Gâchette.
    private var gauge: CAShapeLayer?
    private var gaugeMask: CAShapeLayer?
    // Croix : les quatre branches allumées et leurs flèches.
    private var arms: [PadDirection: CAShapeLayer] = [:]
    private var chevrons: [PadDirection: CAShapeLayer] = [:]
    private let s: CGFloat
    private let screenScale: CGFloat
    private var lastToggleSimple: Bool?

    init(control: PadControl, frame: CGRect, scale: CGFloat, screenScale: CGFloat) {
        self.control = control
        self.s = scale
        self.screenScale = screenScale
        root.frame = frame
        let bounds = CGRect(origin: .zero, size: frame.size)
        plate.frame = bounds
        plate.lineWidth = 1
        // Une ombre d'encre autour du verre : le bord se lit aussi sur une image claire.
        plate.shadowColor = PadInk.rgb(0x05070A)
        plate.shadowOpacity = 0.45
        plate.shadowRadius = 3
        plate.shadowOffset = .zero
        root.addSublayer(plate)
        glyph.frame = bounds
        glyph.fillColor = nil
        glyph.lineCap = .round
        glyph.lineJoin = .round
        Self.ink(glyph)
        switch control {
        case .leftStick, .rightStick:
            plate.path = UIBezierPath(ovalIn: bounds.insetBy(dx: 0.5, dy: 0.5)).cgPath
            // Repère de la course : un cercle discret.
            glyph.path = UIBezierPath(ovalIn: bounds.insetBy(dx: bounds.width * 0.2, dy: bounds.height * 0.2)).cgPath
            glyph.strokeColor = PadInk.rgb(0xFFFFFF, 0.1)
            glyph.lineWidth = 1
            root.addSublayer(glyph)
            let k = CAShapeLayer()
            let r = bounds.width * 0.23
            k.bounds = CGRect(x: 0, y: 0, width: 2 * r, height: 2 * r)
            k.path = UIBezierPath(ovalIn: k.bounds).cgPath
            k.lineWidth = 1.5
            k.position = CGPoint(x: bounds.midX, y: bounds.midY)
            root.addSublayer(k)
            knob = k
            addLabel(control == .leftStick ? "L" : "R", size: 12, alpha: 0.5, at: CGPoint(x: bounds.midX, y: bounds.midY), in: k)
        case .dpad:
            buildDpad(bounds)
        case .lt, .rt:
            let path = UIBezierPath(roundedRect: bounds.insetBy(dx: 0.5, dy: 0.5), cornerRadius: 12 * scale).cgPath
            plate.path = path
            let g = CAShapeLayer()
            g.frame = bounds
            g.fillColor = PadInk.spectreFill
            let mask = CAShapeLayer()
            mask.frame = bounds
            mask.path = path
            g.mask = mask
            root.addSublayer(g)
            gauge = g
            gaugeMask = mask
            addLabel(control == .lt ? "LT" : "RT", size: 15, at: CGPoint(x: bounds.midX, y: bounds.minY + 17 * scale))
        case .lb, .rb:
            plate.path = UIBezierPath(roundedRect: bounds.insetBy(dx: 0.5, dy: 0.5), cornerRadius: bounds.height / 2).cgPath
            addLabel(control == .lb ? "LB" : "RB", size: 15, at: CGPoint(x: bounds.midX, y: bounds.midY))
        case .a, .b, .x, .y:
            plate.path = UIBezierPath(ovalIn: bounds.insetBy(dx: 0.5, dy: 0.5)).cgPath
            addLabel(control.rawValue.uppercased(), size: 21, at: CGPoint(x: bounds.midX, y: bounds.midY))
        case .view:
            plate.path = UIBezierPath(roundedRect: bounds.insetBy(dx: 0.5, dy: 0.5), cornerRadius: bounds.height / 2).cgPath
            // Deux fenêtres décalées.
            let u = 5 * scale, c = CGPoint(x: bounds.midX, y: bounds.midY)
            let p = UIBezierPath(roundedRect: CGRect(x: c.x - 1.6 * u, y: c.y - 1.4 * u, width: 2.2 * u, height: 1.8 * u), cornerRadius: 0.4 * u)
            p.append(UIBezierPath(roundedRect: CGRect(x: c.x - 0.6 * u, y: c.y - 0.4 * u, width: 2.2 * u, height: 1.8 * u), cornerRadius: 0.4 * u))
            glyph.path = p.cgPath
            glyph.lineWidth = 1.5 * max(scale, 0.9)
            root.addSublayer(glyph)
        case .menu:
            plate.path = UIBezierPath(roundedRect: bounds.insetBy(dx: 0.5, dy: 0.5), cornerRadius: bounds.height / 2).cgPath
            let u = 5 * scale, c = CGPoint(x: bounds.midX, y: bounds.midY)
            let p = UIBezierPath()
            for dy in [-u, 0, u] {
                p.move(to: CGPoint(x: c.x - 1.4 * u, y: c.y + dy))
                p.addLine(to: CGPoint(x: c.x + 1.4 * u, y: c.y + dy))
            }
            glyph.path = p.cgPath
            glyph.lineWidth = 1.6 * max(scale, 0.9)
            root.addSublayer(glyph)
        case .guide:
            plate.path = UIBezierPath(ovalIn: bounds.insetBy(dx: 0.5, dy: 0.5)).cgPath
            let c = CGPoint(x: bounds.midX, y: bounds.midY)
            let p = UIBezierPath(arcCenter: c, radius: bounds.width * 0.24, startAngle: 0, endAngle: 2 * .pi, clockwise: true)
            p.append(UIBezierPath(ovalIn: CGRect(x: c.x - 1.6 * scale, y: c.y - 1.6 * scale, width: 3.2 * scale, height: 3.2 * scale)))
            glyph.path = p.cgPath
            glyph.lineWidth = 1.6 * max(scale, 0.9)
            root.addSublayer(glyph)
        case .toggle:
            plate.path = UIBezierPath(ovalIn: bounds.insetBy(dx: 0.5, dy: 0.5)).cgPath
            let i = CALayer()
            Self.ink(i)
            let side = bounds.width * 0.5
            i.frame = CGRect(x: bounds.midX - side / 2, y: bounds.midY - side / 2, width: side, height: side)
            i.contentsGravity = .resizeAspect
            i.contentsScale = screenScale
            root.addSublayer(i)
            icon = i
        }
        plate.shadowPath = plate.path
    }

    /// Signe lisible sur une image claire : une ombre d'encre serrée.
    private static func ink(_ layer: CALayer) {
        layer.shadowColor = PadInk.rgb(0x05070A)
        layer.shadowOpacity = 0.7
        layer.shadowRadius = 1.5
        layer.shadowOffset = .zero
    }

    private func addLabel(_ text: String, size: CGFloat, alpha: CGFloat = 1, at center: CGPoint, in parent: CALayer? = nil) {
        let font = Theme.variable("Archivo", size * s, weight: 640, width: 100)
        let t = CATextLayer()
        t.string = text
        t.font = font
        t.fontSize = font.pointSize
        t.alignmentMode = .center
        t.foregroundColor = PadInk.frost
        t.contentsScale = screenScale
        t.opacity = Float(alpha)
        Self.ink(t)
        let h = font.lineHeight
        t.frame = CGRect(x: center.x - 30 * s, y: center.y - h / 2, width: 60 * s, height: h)
        if let parent {
            t.frame = CGRect(x: parent.bounds.midX - 30 * s, y: parent.bounds.midY - h / 2, width: 60 * s, height: h)
            parent.addSublayer(t)
        } else {
            root.addSublayer(t)
            label = t
        }
    }

    private func buildDpad(_ bounds: CGRect) {
        let w = bounds.width, arm = w * 0.34, r = 7 * s
        let c = CGPoint(x: bounds.midX, y: bounds.midY)
        // Le contour de la croix, d'un seul tenant.
        plate.path = Self.crossOutline(c: c, half: w / 2 - 0.5, arm: arm / 2, radius: r).cgPath
        let rects: [PadDirection: CGRect] = [
            .up: CGRect(x: c.x - arm / 2, y: bounds.minY + 0.5, width: arm, height: w / 2 - arm / 2),
            .down: CGRect(x: c.x - arm / 2, y: c.y + arm / 2, width: arm, height: w / 2 - arm / 2 - 0.5),
            .left: CGRect(x: bounds.minX + 0.5, y: c.y - arm / 2, width: w / 2 - arm / 2, height: arm),
            .right: CGRect(x: c.x + arm / 2, y: c.y - arm / 2, width: w / 2 - arm / 2 - 0.5, height: arm),
        ]
        for (d, rect) in rects {
            let l = CAShapeLayer()
            let corners: UIRectCorner = switch d {
            case .up: [.topLeft, .topRight]
            case .down: [.bottomLeft, .bottomRight]
            case .left: [.topLeft, .bottomLeft]
            case .right: [.topRight, .bottomRight]
            }
            l.path = UIBezierPath(roundedRect: rect, byRoundingCorners: corners, cornerRadii: CGSize(width: r, height: r)).cgPath
            l.fillColor = PadInk.spectreFill
            l.isHidden = true
            root.addSublayer(l)
            arms[d] = l
            // Flèche au bout de la branche.
            let chevron = CAShapeLayer()
            let tip = arm * 0.2, mid: CGPoint = switch d {
            case .up: CGPoint(x: c.x, y: bounds.minY + w * 0.17)
            case .down: CGPoint(x: c.x, y: bounds.maxY - w * 0.17)
            case .left: CGPoint(x: bounds.minX + w * 0.17, y: c.y)
            case .right: CGPoint(x: bounds.maxX - w * 0.17, y: c.y)
            }
            let p = UIBezierPath()
            switch d {
            case .up:
                p.move(to: CGPoint(x: mid.x - tip, y: mid.y + tip / 2)); p.addLine(to: CGPoint(x: mid.x, y: mid.y - tip / 2)); p.addLine(to: CGPoint(x: mid.x + tip, y: mid.y + tip / 2))
            case .down:
                p.move(to: CGPoint(x: mid.x - tip, y: mid.y - tip / 2)); p.addLine(to: CGPoint(x: mid.x, y: mid.y + tip / 2)); p.addLine(to: CGPoint(x: mid.x + tip, y: mid.y - tip / 2))
            case .left:
                p.move(to: CGPoint(x: mid.x + tip / 2, y: mid.y - tip)); p.addLine(to: CGPoint(x: mid.x - tip / 2, y: mid.y)); p.addLine(to: CGPoint(x: mid.x + tip / 2, y: mid.y + tip))
            case .right:
                p.move(to: CGPoint(x: mid.x - tip / 2, y: mid.y - tip)); p.addLine(to: CGPoint(x: mid.x + tip / 2, y: mid.y)); p.addLine(to: CGPoint(x: mid.x - tip / 2, y: mid.y + tip))
            }
            chevron.path = p.cgPath
            chevron.fillColor = nil
            chevron.strokeColor = PadInk.frost
            chevron.lineWidth = 1.8 * max(s, 0.9)
            chevron.lineCap = .round
            chevron.lineJoin = .round
            root.addSublayer(chevron)
            chevrons[d] = chevron
        }
    }

    /// Contour d'une croix aux bouts arrondis.
    private static func crossOutline(c: CGPoint, half: CGFloat, arm: CGFloat, radius r: CGFloat) -> UIBezierPath {
        // Sens horaire, depuis le haut de la branche du haut ; coins arrondis
        // au bout des branches, presque vifs au centre.
        let pts: [(CGPoint, Bool)] = [
            (CGPoint(x: c.x - arm, y: c.y - half), true), (CGPoint(x: c.x + arm, y: c.y - half), true),
            (CGPoint(x: c.x + arm, y: c.y - arm), false),
            (CGPoint(x: c.x + half, y: c.y - arm), true), (CGPoint(x: c.x + half, y: c.y + arm), true),
            (CGPoint(x: c.x + arm, y: c.y + arm), false),
            (CGPoint(x: c.x + arm, y: c.y + half), true), (CGPoint(x: c.x - arm, y: c.y + half), true),
            (CGPoint(x: c.x - arm, y: c.y + arm), false),
            (CGPoint(x: c.x - half, y: c.y + arm), true), (CGPoint(x: c.x - half, y: c.y - arm), true),
            (CGPoint(x: c.x - arm, y: c.y - arm), false),
        ]
        let n = pts.count
        let path = CGMutablePath()
        path.move(to: CGPoint(x: (pts[n - 1].0.x + pts[0].0.x) / 2, y: (pts[n - 1].0.y + pts[0].0.y) / 2))
        for i in 0 ..< n {
            let (corner, rounded) = pts[i]
            let next = pts[(i + 1) % n].0
            path.addArc(tangent1End: corner, tangent2End: next, radius: rounded ? r : 2)
        }
        path.closeSubpath()
        return UIBezierPath(cgPath: path)
    }

    // MARK: États

    private func colors(pressed: Bool) {
        plate.fillColor = pressed ? PadInk.spectreFill : PadInk.plate
        plate.strokeColor = pressed ? PadInk.spectre : PadInk.line
        glyph.strokeColor = pressed ? PadInk.white : PadInk.frost
        label?.foregroundColor = pressed ? PadInk.white : PadInk.frost
        // Le bouton s'enfonce légèrement.
        root.setAffineTransform(pressed ? CGAffineTransform(scaleX: 0.93, y: 0.93) : .identity)
    }

    func setPressed(_ pressed: Bool) { colors(pressed: pressed) }

    func setStick(touched: Bool, clicked: Bool, raw: (x: Double, y: Double), travel: CGFloat) {
        plate.fillColor = clicked ? PadInk.spectreSoft : PadInk.plate
        plate.strokeColor = clicked || touched ? PadInk.spectre : PadInk.line
        plate.lineWidth = clicked ? 2.5 : 1
        root.setAffineTransform(clicked ? CGAffineTransform(scaleX: 0.95, y: 0.95) : .identity)
        guard let knob else { return }
        knob.fillColor = touched ? PadInk.spectreFill : PadInk.knob
        knob.strokeColor = touched ? PadInk.spectre : PadInk.knobLine
        let b = root.bounds
        knob.position = CGPoint(x: b.midX + CGFloat(raw.x) * travel, y: b.midY - CGFloat(raw.y) * travel)
    }

    func setTrigger(_ value: Double) {
        let pressed = value > 0
        plate.fillColor = PadInk.plate
        plate.strokeColor = pressed ? PadInk.spectre : PadInk.line
        label?.foregroundColor = pressed ? PadInk.white : PadInk.frost
        root.setAffineTransform(pressed ? CGAffineTransform(scaleX: 0.96, y: 0.96) : .identity)
        // La jauge descend avec la course.
        let b = root.bounds
        gauge?.path = UIBezierPath(rect: CGRect(x: 0, y: 0, width: b.width, height: b.height * CGFloat(value))).cgPath
    }

    func setDpad(_ pressed: Set<PadDirection>) {
        plate.fillColor = PadInk.plate
        plate.strokeColor = pressed.isEmpty ? PadInk.line : PadInk.spectre
        for (d, l) in arms { l.isHidden = !pressed.contains(d) }
        for (d, l) in chevrons { l.strokeColor = pressed.contains(d) ? PadInk.white : PadInk.frost }
        root.setAffineTransform(pressed.isEmpty ? .identity : CGAffineTransform(scaleX: 0.97, y: 0.97))
    }

    /// `simple` : le bouton mène à la disposition Simple (icône qui resserre).
    func setToggle(simple: Bool, pressed: Bool) {
        colors(pressed: pressed)
        guard let icon, lastToggleSimple != simple || icon.contents == nil else { return }
        lastToggleSimple = simple
        let name = simple ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right"
        let config = UIImage.SymbolConfiguration(pointSize: icon.bounds.width * 0.8, weight: .semibold)
        guard let symbol = UIImage(systemName: name, withConfiguration: config)?
            .withTintColor(UIColor(cgColor: PadInk.frost), renderingMode: .alwaysOriginal) else { return }
        // Le symbole dessiné dans une image à lui : sa teinte y reste.
        let format = UIGraphicsImageRendererFormat()
        format.scale = screenScale
        icon.contents = UIGraphicsImageRenderer(size: symbol.size, format: format).image { _ in symbol.draw(at: .zero) }.cgImage
    }
}

// MARK: - Dans SwiftUI

/// La manette par-dessus l'image de la session.
struct ScreenPadOverlay: UIViewRepresentable {
    let layout: ScreenPadLayout
    let size: ScreenPadSize
    let opacity: Double
    let haptics: ScreenPadHaptics
    var topReserved: CGFloat = 0
    let toggle: () -> Void

    func makeUIView(context: Context) -> ScreenPadView {
        let view = ScreenPadView(frame: .zero)
        update(view)
        return view
    }

    func updateUIView(_ view: ScreenPadView, context: Context) { update(view) }

    private func update(_ view: ScreenPadView) {
        view.onToggleLayout = toggle
        view.haptics = haptics
        view.size = size
        view.padOpacity = CGFloat(opacity)
        view.topReserved = topReserved
        view.layout = layout
    }

    static func dismantleUIView(_ view: ScreenPadView, coordinator: ()) {
        view.releaseEverything()
    }
}
