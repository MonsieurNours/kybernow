// Menu de session (maquette « 5 · En session ») : l'image recule, la santé
// du flux en haut, les actions en bas. Ouvert par le bouton Retour de la
// télécommande ou L1 + R1 + Start + Select ; pendant le menu, les manettes
// pilotent KyberNow et le jeu ne reçoit rien.

import AVKit
import KyberKit
import SwiftUI

struct SessionMenu: View {
    @ObservedObject var session: SessionController
    @EnvironmentObject private var pads: GamepadBridge
    @ObservedObject private var prefs = Preferences.shared
    @State private var showPads = false
    @State private var typing = false
    @State private var sound = false
    @State private var text = ""
    @FocusState private var focus: Item?

    enum Item: Hashable { case resume, pads, sound, mouse, type, quit }

    var body: some View {
        ZStack(alignment: .topLeading) {
            // En haut : où l'on est, et la santé du flux.
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 10) {
                    Theme.eyebrow("En session depuis \(minutesLabel)")
                    Text(session.host.name).font(Theme.display(60, 660)).tracking(-60 * 0.035).foregroundStyle(Theme.text)
                }
                Spacer()
                statsStrip
            }
            .frame(width: 1920 - 192)
            .padding(.leading, 96).padding(.top, 56)

            if prefs.latencyAlert, session.route.isLate, session.host.settings.audio != .off {
                audioWarning
                    .frame(width: 980)
                    .padding(.leading, 96).padding(.top, 210)
            }

            // Le menu.
            VStack(alignment: .leading, spacing: 22) {
                HStack(spacing: 24) {
                    resumeTile
                    tile(.pads, icon: "game-controller", title: "Manettes", detail: padsDetail) { showPads = true }
                    soundTile
                    mouseTile
                    tile(.type, icon: "keyboard", title: "Taper du texte", detail: "Clavier tvOS ou iPhone") { typing = true }
                    tile(.quit, icon: "power", iconColor: Theme.danger, title: "Se déconnecter", detail: "L'ordinateur retrouve ses écrans") { session.end() }
                }
                .focusSection()
                HStack {
                    HStack(spacing: 12) {
                        Glyph(kind: .remote, role: .back, size: 28)
                        Text("Retour ou L1 + R1 + Start + Select : fermer le menu")
                    }
                    Spacer()
                    Text("Pendant le menu, les manettes pilotent KyberNow ; le jeu ne reçoit rien.")
                }
                .font(Theme.sans(19))
                .foregroundStyle(Theme.muted)
            }
            .frame(width: 1920 - 192)
            .padding(.leading, 96).padding(.top, 1080 - 72 - 230 - 22 - 28)

            if typing {
                typingPanel
            }
            if sound {
                soundPanel
            }
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .defaultFocus($focus, .resume)
        .onAppear {
            // Le menu s'ouvre sur « Reprendre », allumé.
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { focus = .resume }
        }
        .fullScreenCover(isPresented: $showPads) {
            GamepadsView(inSession: true) { showPads = false }
                .environmentObject(pads)
        }
    }

    // MARK: Santé du flux

    private var minutesLabel: String {
        let m = session.sessionMinutes
        return m < 60 ? "\(m) min" : "\(m / 60) h \(String(format: "%02d", m % 60))"
    }

    private var statsStrip: some View {
        let s = session.stats
        let lost = (s?.lostFrames ?? 0) + (s?.lostPackets ?? 0)
        return HStack(spacing: 28) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(s?.latencyMs.map(HomeView.ms) ?? "—").font(Theme.mono(30, .medium)).foregroundStyle(Theme.text)
                Text("ms")
            }
            Rectangle().fill(Color.white.opacity(0.14)).frame(width: 1, height: 32)
            Text("\(Int((s?.fps ?? 0).rounded())) i/s")
            Text("\(Int((s?.mbps ?? 0).rounded())) Mbit/s")
            Text("\(session.host.settings.codec == .h264 ? "H.264" : "HEVC") \(s?.format.components(separatedBy: " ").first ?? "4:2:0")")
            HStack(spacing: 8) {
                Circle().fill(lost == 0 ? Theme.ok : Theme.warn).frame(width: 9, height: 9)
                Text(lost == 0 ? "0 perte" : "\(lost) pertes")
            }
        }
        .font(Theme.mono(18))
        .foregroundStyle(Theme.text2)
        .padding(.horizontal, 30)
        .frame(height: 72)
        .glass(20, tint: 0.55)
    }

    private var audioWarning: some View {
        let r = session.route
        let title = "\(r.name) : le son arrive \(r.latencyMs) ms après l'image"
        let body: String = switch r.kind {
        case .airpods: prefs.spatialAudio
            ? "Le \(session.route.channels > 2 ? session.route.channelsLabel : "son") du jeu est spatialisé dans les AirPods, sans suivi de la tête. Pour jouer en rythme, préférez la sortie HDMI du téléviseur ou de l'ampli."
            : "Pour jouer en rythme, préférez la sortie HDMI du téléviseur ou de l'ampli."
        case .airplay: "Par AirPlay, l'Apple TV ne peut pas retarder un jeu en direct pour l'aligner. Gardez le son sur l'ordinateur ou passez par la sortie HDMI."
        default: "Une sortie Bluetooth ajoute du retard. Pour jouer en rythme, préférez la sortie HDMI."
        }
        return HStack(alignment: .top, spacing: 20) {
            Icon("warning", size: 34).foregroundStyle(Theme.warn)
            VStack(alignment: .leading, spacing: 8) {
                Text(title).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.warnText)
                Text(body).font(Theme.sans(20)).foregroundStyle(Theme.rgb(0xD8CDB4)).lineSpacing(5)
            }
        }
        .padding(.horizontal, 28).padding(.vertical, 24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 22, style: .continuous).fill(Theme.rgb(0x1E1A12, 0.88)))
        .overlay(RoundedRectangle(cornerRadius: 22, style: .continuous).strokeBorder(Theme.rgb(0x4A3A1E)))
    }

    // MARK: Tuiles

    private static let tileWidth: CGFloat = (1920 - 192 - 5 * 24) / 6.3

    private var resumeTile: some View {
        Button { session.closeMenu() } label: {
            VStack(alignment: .leading) {
                Icon("play", size: 44, bold: true)
                Spacer()
                VStack(alignment: .leading, spacing: 6) {
                    Text("Reprendre").font(Theme.sans(32, .bold))
                    Text("La manette repart vers le jeu").font(Theme.sans(19)).foregroundStyle(.white.opacity(0.8))
                }
            }
            .padding(28)
            .frame(width: Self.tileWidth * 1.3, height: 230, alignment: .leading)
        }
        .buttonStyle(ResumeTileStyle())
        .focused($focus, equals: .resume)
    }

    private func tile(_ item: Item, icon: String, iconColor: Color = Theme.accent, title: String, detail: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            tileLabel(icon: icon, iconColor: iconColor, title: title, detail: detail)
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: item)
    }

    private func tileLabel(icon: String, iconColor: Color = Theme.accent, title: String, detail: String, trailing: AnyView? = nil) -> some View {
        VStack(alignment: .leading) {
            HStack(alignment: .top) {
                Icon(icon, size: 40).foregroundStyle(iconColor)
                Spacer()
                trailing
            }
            Spacer()
            VStack(alignment: .leading, spacing: 6) {
                Text(title).font(Theme.sans(26, .semibold)).foregroundStyle(Theme.text).lineLimit(2)
                Text(detail).font(Theme.sans(18)).foregroundStyle(Theme.muted).lineLimit(2)
            }
        }
        .padding(26)
        .frame(width: Self.tileWidth, height: 230, alignment: .leading)
    }

    private var padsDetail: String {
        pads.pads.isEmpty ? "Aucune manette" : pads.pads.prefix(2).map { "J\($0.player) \($0.kind == .xbox ? "Xbox" : $0.kind == .playstation ? HomeView.short($0.name) : "Manette")" }.joined(separator: " · ")
    }

    /// Le son : sa sortie ; un clic ouvre le détail et le choix de la sortie de l'Apple TV.
    private var soundTile: some View {
        let r = session.route
        let detail = session.host.settings.audio == .off ? "Sur l'ordinateur"
            : r.kind == .hdmi ? "\(r.channelsLabel) · \(r.name) · \(r.latencyMs) ms"
            : r.kind == .airpods && prefs.spatialAudio ? "\(r.name) · audio spatial" : r.name
        return Button { sound = true } label: {
            tileLabel(icon: "speaker-high", iconColor: r.isLate ? Theme.warn : Theme.accent, title: "Son", detail: detail)
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: .sound)
    }

    private var soundPanel: some View {
        let r = session.route
        return ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 24) {
                Theme.eyebrow("Son")
                Text(session.host.settings.audio == .off ? "Le son reste sur l'ordinateur" : "Son \(r.channelsLabel) · \(r.name)")
                    .font(Theme.display(44, 640)).tracking(-1.2)
                Text(session.host.settings.audio == .off
                     ? "Ses haut-parleurs ou son casque. Pour l'entendre ici, changez « Le son joue » dans les réglages de l'ordinateur."
                     : "Sortie mesurée : \(r.latencyMs) ms. \(r.isLate ? "Une sortie sans fil ajoute du retard : la sortie HDMI reste la plus rapide." : "C'est la sortie la plus rapide de l'Apple TV.")")
                    .font(Theme.sans(23)).foregroundStyle(Theme.text2).frame(width: 900, alignment: .leading)
                HStack(spacing: 20) {
                    RoutePicker()
                        .frame(width: 80, height: 80)
                    Text("Changer de sortie : AirPods, HomePod, téléviseur").font(Theme.sans(22)).foregroundStyle(Theme.muted)
                }
                Button("Fermer") { sound = false }
                    .buttonStyle(ChoiceButton(selected: false, height: 72))
            }
            .padding(56)
            .glass(30, tint: 0.6)
        }
        .frame(width: 1920, height: 1080)
        .onExitCommand { sound = false }
    }

    private var mouseTile: some View {
        Button { session.toggleRemoteMouse() } label: {
            tileLabel(icon: "cursor", title: "Télécommande en souris",
                      detail: session.remoteMouse ? "Pavé = pointeur, clic = clic gauche" : "Désactivée · le pavé navigue",
                      trailing: AnyView(Toggle2(on: session.remoteMouse)))
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: .mouse)
        .accessibilityValue(session.remoteMouse ? "Activée" : "Désactivée")
    }

    // MARK: Texte

    private var typingPanel: some View {
        ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 26) {
                Theme.eyebrow("Taper du texte sur \(session.host.name)")
                Text("Le texte part tel quel, à l'endroit où se trouve le curseur sur l'ordinateur.")
                    .font(Theme.sans(23)).foregroundStyle(Theme.text2)
                TextField("Texte à envoyer", text: $text)
                    .font(Theme.sans(27))
                    .frame(width: 900)
                    .onSubmit(send)
                HStack(spacing: 20) {
                    Button("Envoyer", action: send).buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                    Button("Annuler") { typing = false; text = "" }
                        .buttonStyle(ChoiceButton(selected: false, height: 72))
                }
            }
            .padding(56)
            .glass(30, tint: 0.6)
        }
        .frame(width: 1920, height: 1080)
        .onExitCommand { typing = false }
    }

    private func send() {
        guard !text.isEmpty else { return }
        session.type(text)
        text = ""
        typing = false
        session.closeMenu()
    }
}

/// « Reprendre » : l'aplat Spectre et son pan coupé, allumé quand il a le focus.
private struct ResumeTileStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        Styled(configuration: configuration)
    }

    private struct Styled: View {
        let configuration: Configuration
        @Environment(\.isFocused) private var focused

        var body: some View {
            configuration.label
                .foregroundStyle(.white)
                .background(SignalCorner(radius: 22, chamfer: 18).fill(Theme.accentFill))
                .neon(configuration.isPressed ? .flash : focused ? .lit : .off)
                .scaleEffect(focused ? 1.07 : 1)
                .animation(Theme.focus, value: focused)
        }
    }
}

/// Choix de la sortie du son de l'Apple TV (AirPods, HomePod, téléviseur).
struct RoutePicker: UIViewRepresentable {
    func makeUIView(context: Context) -> AVRoutePickerView {
        let view = AVRoutePickerView()
        view.tintColor = UIColor(Theme.text)
        view.activeTintColor = UIColor(Theme.accent)
        return view
    }

    func updateUIView(_ uiView: AVRoutePickerView, context: Context) {}
}
