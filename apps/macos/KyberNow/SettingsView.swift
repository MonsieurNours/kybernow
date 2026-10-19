// Réglages de connexion d'un ordinateur, fidèles à l'écran « 4 · Réglages
// de connexion » de la maquette. Chaque changement est enregistré tout de
// suite ; toucher à l'image ou au réseau rend le profil « Personnalisé ».

import GameController
import KyberKit
import SwiftUI

struct SettingsView: View {
    @EnvironmentObject var app: AppModel
    let hostID: UUID

    var body: some View {
        if let host = app.store.host(hostID) {
            SettingsContent(host: host, info: app.store.status[hostID]?.info)
        } else {
            Color.clear.onAppear { app.route = .home }
        }
    }
}

private struct SettingsContent: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let info: HostInfo?

    private var s: ConnectionSettings { host.settings }

    private func update(custom: Bool = false, _ change: (inout ConnectionSettings) -> Void) {
        var h = host
        change(&h.settings)
        if custom { h.settings.profile = nil }
        app.store.update(h)
    }

    private var profile: Binding<Profile?> {
        Binding(get: { s.profile }, set: { p in if let p { update { $0.apply(p) } } })
    }

    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            VStack(spacing: 0) {
                topBar
                ScrollView {
                    VStack(alignment: .leading, spacing: 22) {
                        header
                        HStack(alignment: .top, spacing: 18) {
                            displayCard.frame(maxWidth: .infinity)
                            imageCard.frame(maxWidth: .infinity)
                            VStack(spacing: 18) {
                                networkCard
                                controlsCard
                                machineCard
                            }
                            .frame(maxWidth: .infinity)
                        }
                    }
                    .padding(.horizontal, 48)
                    .padding(.top, 28)
                    .padding(.bottom, 32)
                }
            }
        }
        .preferredColorScheme(.dark)
        .frame(minWidth: 1100, minHeight: 760)
    }

    // MARK: - En-tête

    private var topBar: some View {
        ZStack {
            Text(host.name).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.muted)
            HStack {
                Button { app.route = .home } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "chevron.left").font(.system(size: 12, weight: .bold))
                        Text("Vos ordinateurs")
                    }
                    .font(Theme.sans(13))
                    .foregroundStyle(Theme.text2)
                }
                .buttonStyle(.plain)
                .keyboardShortcut(.cancelAction)
                Spacer()
            }
            .padding(.leading, 84)
        }
        .frame(height: 52)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
    }

    private var header: some View {
        HStack(alignment: .bottom) {
            VStack(alignment: .leading, spacing: 6) {
                Text("Réglages de connexion").font(Theme.display(36)).foregroundStyle(Theme.text)
                Text("Pour \(host.name), sur ce Mac. Partez d'un profil, ajustez ensuite si besoin : il devient « Personnalisé ».")
                    .font(Theme.sans(14)).foregroundStyle(Theme.muted)
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 8) {
                Segmented(options: Profile.allCases.map { ($0, $0.label) }, selection: profile)
                    .fixedSize()
                Text(s.profile?.summary ?? "Personnalisé : vous avez ajusté un profil.")
                    .font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
        }
    }

    // MARK: - Écran

    private var displayCard: some View {
        SettingsCard(symbol: "display", title: "Écran de l'ordinateur") {
            Text("L'ordinateur peut créer un écran virtuel à la taille du vôtre et mettre ses vrais écrans en veille le temps de la session. L'image remplit votre écran, sans bandes noires.")
                .font(Theme.sans(13)).foregroundStyle(Theme.muted).lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
            let m = DisplayInfo.mode(for: NSScreen.main ?? NSScreen.screens[0])
            VStack(spacing: 8) {
                option(.match, "Comme cet écran", "\(m.width) × \(m.height) · \(m.refresh) Hz")
                option(.window, "Taille de la fenêtre", "session fenêtrée, l'écran suit la fenêtre")
                option(.custom, "Sur mesure", "\(s.customWidth) × \(s.customHeight) · \(s.customRefresh) Hz")
                if s.display == .custom { customMode }
                option(.physical, "Un écran de l'ordinateur", "sans écran virtuel")
            }
            if info != nil, info?.virtualDisplay == false, s.display != .physical {
                Notice(kind: .warn, text: "Le pilote d'écran virtuel ne répond pas sur l'ordinateur : vous verrez ses écrans tels quels.")
            }
            Spacer(minLength: 0)
            Notice(kind: .info, text: "À la fin de la session, l'écran virtuel disparaît et les écrans de l'ordinateur se rallument comme avant.")
        }
        .frame(minHeight: 590, alignment: .top)
    }

    private func option(_ choice: DisplayChoice, _ title: String, _ sub: String) -> some View {
        let on = s.display == choice
        return Button { update { $0.display = choice } } label: {
            HStack(spacing: 12) {
                Circle()
                    .strokeBorder(on ? Theme.accent : Theme.line3, lineWidth: on ? 5 : 2)
                    .frame(width: 16, height: 16)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                    Text(sub).font(Theme.mono(12)).foregroundStyle(Theme.muted)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 14).padding(.vertical, 12)
            .background(on ? Theme.accentSoft.opacity(0.45) : Theme.well, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(on ? Theme.accent : Theme.line))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(on ? .isSelected : [])
    }

    private var customMode: some View {
        HStack(spacing: 8) {
            number(s.customWidth, range: 640...7680) { v in update { $0.customWidth = v } }
            Text("×").foregroundStyle(Theme.muted)
            number(s.customHeight, range: 480...4320) { v in update { $0.customHeight = v } }
            Text("à").foregroundStyle(Theme.muted)
            number(s.customRefresh, range: 24...500) { v in update { $0.customRefresh = v } }
            Text("Hz").foregroundStyle(Theme.muted)
        }
        .font(Theme.sans(13))
        .padding(.leading, 28)
    }

    private func number(_ value: Int, range: ClosedRange<Int>, set: @escaping (Int) -> Void) -> some View {
        NumberField(value: value, range: range, set: set).frame(width: 72)
    }

    // MARK: - Image

    private var imageCard: some View {
        SettingsCard(symbol: "photo", title: "Image") {
            VStack(alignment: .leading, spacing: 8) {
                Text("Format de compression").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                Segmented(
                    options: [(VideoCodec.h264, "H.264"), (.hevc, "HEVC"), (.av1, "AV1")],
                    selection: Binding(get: { s.codec }, set: { c in if let c { update(custom: true) { $0.codec = c } } }),
                    disabled: disabledCodecs,
                    fill: true
                )
                Text(codecHint).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }

            HStack(spacing: 8) {
                Text("Texte net (4:4:4)").font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                InfoTip(label: "Qu'est-ce que le 4:4:4 ?") {
                    Text("Toutes les couleurs, pour chaque pixel").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                    Text("D'habitude, la couleur est envoyée pour un pixel sur quatre : invisible dans un jeu, mais le texte fin bave un peu. Le 4:4:4 garde tout.")
                    Text("+ Texte, code et tableurs parfaitement nets").foregroundStyle(Theme.accent)
                    Text("− Débit plus élevé, encodage un peu plus long sur l'ordinateur").foregroundStyle(Theme.warn)
                }
                Spacer()
                Switch(isOn: Binding(get: { s.yuv444 && can444 }, set: { v in update(custom: true) { $0.yuv444 = v } }), label: "Texte net (4:4:4)")
                    .disabled(!can444)
                    .opacity(can444 ? 1 : 0.4)
            }
            .padding(.top, 4)
            if !can444 {
                Text(s.codec == .h264 ? "Le 4:4:4 demande le HEVC." : "Ce Mac ne décode pas le 4:4:4 en matériel : l'image reste en 4:2:0.")
                    .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }

            HStack(spacing: 8) {
                Text("Synchro verticale").font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                InfoTip(label: "Qu'est-ce que la synchro verticale ?") {
                    Text("Afficher l'image au bon moment").font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text)
                    Text("Avec la synchro, chaque image attend le prochain rafraîchissement de l'écran : jamais coupée, au plus un rafraîchissement d'attente (8 ms à 120 Hz, 16 ms à 60 Hz).")
                    Text("Sans elle, l'image s'affiche dès qu'elle est décodée : la latence la plus basse, mais l'écran peut montrer deux images coupées par une ligne (déchirure).")
                    Text("Sur un écran à fréquence variable (ProMotion, Adaptive-Sync), c'est l'écran qui attend l'image : gardez la synchro, elle ne coûte rien.").foregroundStyle(Theme.accent)
                }
                Spacer()
                Switch(isOn: Binding(get: { s.vsync }, set: { v in update { $0.vsync = v } }), label: "Synchro verticale")
            }
            if !s.vsync {
                Text(variableRefresh ? "Votre écran suit déjà l'image (fréquence variable) : sans synchro, vous ne gagnez presque rien." : "Latence la plus basse, déchirure possible dans les mouvements rapides.")
                    .font(Theme.sans(12)).foregroundStyle(variableRefresh ? Theme.warnText : Theme.muted).fixedSize(horizontal: false, vertical: true)
            }

            Spacer(minLength: 0)
            BitrateControl(value: Binding(get: { s.bitrateMbps }, set: { v in update(custom: true) { $0.bitrateMbps = v } }))
        }
        .frame(minHeight: 590, alignment: .top)
    }

    private var can444: Bool { ConnectionSettings.decodes444(s.codec) }

    private var variableRefresh: Bool { DisplayInfo.variableRefresh(NSScreen.main ?? NSScreen.screens[0]) }

    /// Formats que l'ordinateur encode ET que ce Mac décode en matériel.
    private var disabledCodecs: Set<VideoCodec> {
        let caps = DecoderCapabilities.current
        var off: Set<VideoCodec> = []
        if !caps.hevc { off.insert(.hevc) }
        if !(caps.av1 && info?.av1 == true) { off.insert(.av1) }
        return off
    }

    private var codecHint: String {
        let base: String
        switch s.codec {
        case .h264: base = "Le plus compatible. Demande plus de débit pour la même netteté."
        case .hevc: base = "Recommandé : plus net à débit égal, décodé par la puce de ce Mac."
        case .av1: base = "Le plus efficace, décodé par les puces M3 et suivantes."
        }
        if !DecoderCapabilities.current.av1 { return base + " Ce Mac ne décode pas l'AV1 en matériel." }
        return info?.av1 == true ? base : base + " AV1 demande une carte RTX 40 ou plus récente sur l'ordinateur."
    }

    // MARK: - Ce Mac

    /// Ce que KyberNow a mesuré de ce Mac : les profils s'y adaptent.
    private var machineCard: some View {
        let screen = NSScreen.main ?? NSScreen.screens[0]
        let size = screen.frame.size
        let scale = screen.backingScaleFactor
        return SettingsCard(symbol: "laptopcomputer", title: "Ce Mac") {
            VStack(alignment: .leading, spacing: 8) {
                fact("Écran", "\(Int(size.width * scale)) × \(Int(size.height * scale)) · \(DisplayInfo.refreshLabel(screen))")
                fact("Décodage matériel", DecoderCapabilities.current.summary)
            }
            Text(variableRefresh
                 ? "L'écran se cale sur chaque image : latence minimale sans déchirure."
                 : "Les profils demandent l'image à la fréquence de cet écran.")
                .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
        }
    }

    private func fact(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(label).font(Theme.sans(13)).foregroundStyle(Theme.muted)
            Spacer(minLength: 12)
            Text(value).font(Theme.mono(12)).foregroundStyle(Theme.text).multilineTextAlignment(.trailing)
        }
    }

    // MARK: - Réseau

    private var networkCard: some View {
        SettingsCard(symbol: "waveform.path.ecg", title: "Réseau") {
            Segmented(
                options: Transport.allCases.map { ($0, $0.label) },
                selection: Binding(get: { s.transport }, set: { t in if let t { update(custom: true) { $0.transport = t } } }),
                fill: true
            )
            Text(s.transport.hint).font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            HStack {
                Text("Se reconnecter tout seul").font(Theme.sans(14)).foregroundStyle(Theme.text)
                Spacer()
                Switch(isOn: Binding(get: { s.autoReconnect }, set: { v in update { $0.autoReconnect = v } }), label: "Se reconnecter tout seul")
            }
        }
    }

    // MARK: - Contrôles et son

    private var controlsCard: some View {
        SettingsCard(symbol: "gamecontroller", title: "Contrôles et son") {
            HStack {
                Text("Touche ⌘ envoyée comme Ctrl").font(Theme.sans(14)).foregroundStyle(Theme.text)
                Spacer()
                Switch(isOn: Binding(get: { s.commandAsControl }, set: { v in update { $0.commandAsControl = v } }), label: "Touche ⌘ envoyée comme Ctrl")
            }
            VStack(alignment: .leading, spacing: 8) {
                Text("Son").font(Theme.sans(14)).foregroundStyle(Theme.text)
                Segmented(
                    options: AudioChoice.allCases.map { ($0, $0.label) },
                    selection: Binding(get: { s.audio }, set: { a in if let a { update { $0.audio = a } } }),
                    height: 30,
                    fill: true
                )
                Text(audioHint)
                    .font(Theme.sans(12))
                    .foregroundStyle(s.audio == .surround51 || s.audio == .surround71 ? Theme.warnText : Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            GamepadStatus()
        }
    }

    private var audioHint: String {
        if info != nil, info?.speakers == false, s.audio != .off {
            return s.audio.hint + " Sans Steam sur l'ordinateur, le son reste aussi sur ses haut-parleurs."
        }
        return s.audio.hint
    }
}

// MARK: - Débit

/// Débit de 5 à 4 294 Mbit/s, curseur logarithmique, saisie libre et
/// avertissements au-delà de 500 et 900 Mbit/s.
private struct BitrateControl: View {
    @Binding var value: Int

    private let lo = Double(ConnectionSettings.minBitrate)
    private let hi = Double(ConnectionSettings.maxBitrate)

    private var position: Binding<Double> {
        Binding(
            get: { log(Double(value) / lo) / log(hi / lo) },
            set: { p in
                let v = lo * pow(hi / lo, p)
                value = v < 100 ? Int(v.rounded()) : v < 1000 ? Int((v / 5).rounded()) * 5 : Int((v / 50).rounded()) * 50
            }
        )
    }

    private var level: Int { value > 900 ? 2 : value > 500 ? 1 : 0 }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("Débit").font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                Spacer()
                NumberField(value: value, range: ConnectionSettings.minBitrate...ConnectionSettings.maxBitrate) { value = $0 }
                    .frame(width: 76)
                Text("Mbit/s").font(Theme.sans(12)).foregroundStyle(Theme.muted)
            }
            Slider(value: position, in: 0...1)
                .tint(level == 2 ? Theme.danger : level == 1 ? Theme.warn : Theme.accent)
                .accessibilityLabel("Débit")
                .accessibilityValue("\(value) mégabits par seconde")
            HStack {
                ForEach(["5", "50", "300", "1 000", "4 294"], id: \.self) { t in
                    Text(t).font(Theme.mono(11)).foregroundStyle(Theme.faint)
                    if t != "4 294" { Spacer() }
                }
            }
            switch level {
            case 2:
                Notice(kind: .danger, text: "Au-delà de ce qu'un réseau Gigabit transporte (~940 Mbit/s). Réservé au 2,5 ou 10 Gbit/s, câblé de bout en bout. Sinon, l'image saccadera.")
            case 1:
                Notice(kind: .warn, text: "Très haut débit. En Wi-Fi ou sur un réseau chargé, des images risquent de se perdre et la latence de grimper. Préférez un câble Ethernet.")
            default:
                Text("Plus de débit, plus de détails dans les scènes chargées. En réseau local, 80 à 150 Mbit/s suffisent en 4K.")
                    .font(Theme.sans(12)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// Champ numérique compact, validé à la frappe d'Entrée ou en quittant le champ.
private struct NumberField: View {
    let value: Int
    let range: ClosedRange<Int>
    let set: (Int) -> Void
    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        TextField("", text: $text)
            .textFieldStyle(.plain)
            .font(Theme.mono(13))
            .multilineTextAlignment(.trailing)
            .foregroundStyle(Theme.text)
            .padding(.horizontal, 8)
            .frame(height: 30)
            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(focused ? Theme.accent : Theme.line2))
            .focused($focused)
            .onAppear { text = String(value) }
            .onChange(of: value) { _, v in if !focused { text = String(v) } }
            .onSubmit(commit)
            .onChange(of: focused) { _, f in if !f { commit() } }
    }

    private func commit() {
        let v = Int(text.filter(\.isNumber)) ?? value
        let clamped = min(max(v, range.lowerBound), range.upperBound)
        text = String(clamped)
        if clamped != value { set(clamped) }
    }
}

/// Manette branchée sur ce Mac.
private struct GamepadStatus: View {
    @State private var name: String?

    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(name != nil ? Theme.accent : Theme.rgb(0x5B6778)).frame(width: 8, height: 8)
            Text(name.map { "Manette prête : \($0)" } ?? "Aucune manette branchée")
                .font(Theme.sans(13)).foregroundStyle(Theme.text2)
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.well, in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.line))
        .onAppear(perform: refresh)
        .onReceive(NotificationCenter.default.publisher(for: .GCControllerDidConnect)) { _ in refresh() }
        .onReceive(NotificationCenter.default.publisher(for: .GCControllerDidDisconnect)) { _ in refresh() }
    }

    private func refresh() {
        name = GCController.controllers().first?.vendorName
    }
}
