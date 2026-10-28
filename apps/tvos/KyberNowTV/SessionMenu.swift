// Menu de session (maquette « 5 · En session ») : l'image recule, les
// statistiques du flux en haut (les seules mesures de la session, à la
// demande), les actions en bas. Ouvert par le bouton Retour de la
// télécommande ou L1 + R1 + Start + Select ; pendant le menu, les manettes
// pilotent KyberNow et le jeu ne reçoit rien.

import AVKit
import KyberKit
import SwiftUI

struct SessionMenu: View {
    @ObservedObject var session: SessionController
    /// Le partage est observé à part : la liste des invités change avec le signe de vie, pas avec la session.
    @ObservedObject private var share: ShareHostModel
    @EnvironmentObject private var pads: GamepadBridge
    @ObservedObject private var prefs = Preferences.shared
    @State private var showPads = false
    @State private var typing = false
    @State private var sound = false
    @State private var text = ""
    @FocusState private var focus: Item?

    enum Item: Hashable { case resume, pads, sound, quality, qualityChoice(ShareQuality), mouse, padMouse, type, share, quit, restoreMode }

    init(session: SessionController) {
        self.session = session
        _share = ObservedObject(wrappedValue: session.share)
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            // En haut : où l'on est, et la santé du flux.
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 10) {
                    // Un invité : son rôle au-dessus de « Invité sur {nom} » ; « Par le relais » à côté quand la session passe par lui.
                    HStack(spacing: 18) {
                        if session.isGuest {
                            Theme.eyebrow(ShareText.role(session.shareRole))
                        } else {
                            Theme.eyebrow(String(localized: "En session depuis \(minutesLabel)"))
                        }
                        if session.path.viaRelay { relayChip }
                    }
                    Text(session.title).font(Theme.display(60, 660)).tracking(-60 * 0.035).foregroundStyle(Theme.text)
                        .lineLimit(1).minimumScaleFactor(0.5)
                }
                Spacer()
                VStack(alignment: .trailing, spacing: 10) {
                    Theme.eyebrow(String(localized: "Statistiques"))
                    statsStrip
                }
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
                    // Un invité : ses manettes s'il joue, le clavier et la souris
                    // seulement si l'hôte les lui a donnés ; pas de profil ni de réglages.
                    if !session.isGuest || session.shareRole.pads {
                        tile(.pads, icon: "game-controller", title: String(localized: "Manettes"), detail: padsDetail) { showPads = true }
                    }
                    soundTile
                    // Un invité : la qualité de l'image reçue, et le choix des quatre.
                    if session.isGuest { qualityTile }
                    if !session.isGuest || session.shareRole.keyboardAndMouse {
                        mouseTile
                        padMouseTile
                        tile(.type, icon: "keyboard", title: String(localized: "Taper du texte"), detail: String(localized: "Clavier tvOS ou iPhone")) { typing = true }
                    }
                    if session.isGuest {
                        tile(.quit, icon: "power", iconColor: Theme.danger, title: ShareText.leave, detail: ShareText.guestOn(session.guest?.computer ?? "")) { session.end() }
                    } else {
                        shareTile
                        tile(.quit, icon: "power", iconColor: Theme.danger, title: String(localized: "Se déconnecter"), detail: String(localized: "L'ordinateur retrouve ses écrans")) { session.end() }
                    }
                }
                .focusSection()
                HStack {
                    HStack(spacing: 12) {
                        Glyph(kind: .remote, role: .back, size: 28)
                        Text("Retour ou L1 + R1 + Start + Select : fermer le menu")
                    }
                    Spacer()
                    if let offer = session.modeRestoreOffer {
                        // L'ordinateur a changé de définition (profil Bureau).
                        Button { session.restoreMode() } label: {
                            HStack(spacing: 12) {
                                Icon("monitor", size: 26).foregroundStyle(Theme.accent)
                                Text(offer).foregroundStyle(Theme.text)
                            }
                            .padding(.horizontal, 24)
                            .frame(height: 56)
                        }
                        .buttonStyle(GlassButton(radius: 18, scale: 1.05))
                        .focused($focus, equals: .restoreMode)
                    } else {
                        Text("Pendant le menu, les manettes pilotent KyberNow Link ; le jeu ne reçoit rien.")
                    }
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
            if session.qualityPanel {
                qualityPanelView
            }
            if session.sharePanel {
                SharePanelTV(session: session, share: session.share) { session.sharePanel = false }
            }
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .defaultFocus($focus, .resume)
        .onAppear {
            // Le menu s'ouvre sur « Reprendre », allumé.
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { focus = .resume }
        }
        // Un droit repris pendant que sa fenêtre est ouverte : elle se ferme (le texte tapé ne partirait plus).
        .onChange(of: session.shareRole) { _, role in
            guard session.isGuest else { return }
            if !role.keyboardAndMouse, typing { typing = false; text = "" }
            if !role.pads { showPads = false }
        }
        .fullScreenCover(isPresented: $showPads) {
            GamepadsView(inSession: true) { showPads = false }
                .environmentObject(pads)
        }
    }

    // MARK: Santé du flux

    private var minutesLabel: String {
        let m = session.sessionMinutes
        let mm = String(format: "%02d", m % 60)
        return m < 60 ? String(localized: "\(m) min") : String(localized: "\(m / 60) h \(mm)")
    }

    private var statsStrip: some View {
        let s = session.stats
        let lost = (s?.lostFrames ?? 0) + (s?.lostPackets ?? 0)
        let codec = session.imageCodec == .h264 ? "H.264" : "HEVC"
        let format = s?.format.components(separatedBy: " ").first ?? "4:2:0"
        return HStack(spacing: 28) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                Text(s?.latencyMs.map(HomeView.ms) ?? "—").font(Theme.mono(30, .medium)).foregroundStyle(Theme.text)
                Text(verbatim: "ms")
            }
            Rectangle().fill(Color.white.opacity(0.14)).frame(width: 1, height: 32)
            Text("\(Int((s?.fps ?? 0).rounded())) i/s")
            Text("\(Int((s?.mbps ?? 0).rounded())) Mbit/s")
            Text(verbatim: "\(codec) \(format)")
            // « Chemin » : Direct, Par le relais, Par le relais (TCP).
            Text(verbatim: session.path.label)
            HStack(spacing: 8) {
                Circle().fill(lost == 0 ? Theme.ok : Theme.warn).frame(width: 9, height: 9)
                lost == 0 ? Text("0 perte") : Text("\(lost) pertes")
            }
        }
        .font(Theme.mono(18))
        .foregroundStyle(Theme.text2)
        .padding(.horizontal, 30)
        .frame(height: 72)
        .glass(20, tint: 0.55)
    }

    /// « Par le relais » : la session passe par le relais, hors de chez soi.
    private var relayChip: some View {
        HStack(spacing: 10) {
            Icon("globe-hemisphere-west", size: 22, bold: true).foregroundStyle(Theme.accentLight)
            Text(LinkText.viaRelay).font(Theme.sans(20, .semibold)).foregroundStyle(Theme.text)
        }
        .padding(.horizontal, 18)
        .frame(height: 40)
        .glass(20, tint: 0.55)
        .accessibilityElement(children: .combine)
    }

    private var audioWarning: some View {
        let r = session.route
        let title = String(localized: "\(r.name) : le son arrive \(r.latencyMs) ms après l'image")
        let channels = session.route.channelsLabel
        let body: String = switch r.kind {
        case .airpods: !prefs.spatialAudio
            ? String(localized: "Pour jouer en rythme, préférez la sortie HDMI du téléviseur ou de l'ampli.")
            : session.route.channels > 2
            ? String(localized: "Le \(channels) du jeu est spatialisé dans les AirPods, sans suivi de la tête. Pour jouer en rythme, préférez la sortie HDMI du téléviseur ou de l'ampli.")
            : String(localized: "Le son du jeu est spatialisé dans les AirPods, sans suivi de la tête. Pour jouer en rythme, préférez la sortie HDMI du téléviseur ou de l'ampli.")
        case .airplay: String(localized: "Par AirPlay, l'Apple TV ne peut pas retarder un jeu en direct pour l'aligner. Gardez le son sur l'ordinateur ou passez par la sortie HDMI.")
        default: String(localized: "Une sortie Bluetooth ajoute du retard. Pour jouer en rythme, préférez la sortie HDMI.")
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

    /// Les tuiles se partagent la largeur : sept d'ordinaire, huit avec « Partager » ; moins pour un invité.
    private var tileCount: Int {
        if session.isGuest { return 4 + (session.shareRole.pads ? 1 : 0) + (session.shareRole.keyboardAndMouse ? 3 : 0) }
        return 8
    }

    private var tileWidth: CGFloat { (1920 - 192 - CGFloat(tileCount - 1) * 24) / (CGFloat(tileCount) + 0.3) }

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
            .frame(width: tileWidth * 1.3, height: 230, alignment: .leading)
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
        .frame(width: tileWidth, height: 230, alignment: .leading)
    }

    /// « Partager » (KyberNow Share) : ouvre le partage de l'écran principal et
    /// son panneau ; sous-titre : les demandes d'abord, puis les invités (« 1 demande ·
    /// 2 invités »), sinon le code du partage ouvert, sinon « Code et QR code »
    /// (link-ui) ; estompé quand l'ordinateur ne le permet pas : la tuile reste
    /// sélectionnable, le panneau dit pourquoi.
    private var shareTile: some View {
        let availability = session.shareAvailability
        let detail = ShareText.tileDetail(requests: share.requests.count, guests: share.guests.count)
            ?? (share.isOpen ? Share.formatCode(share.state?.code ?? "") : ShareText.tileDetail)
        return Button {
            session.sharePanel = true
            if availability.isAvailable { Task { await share.openIfNeeded() } }
        } label: {
            tileLabel(icon: "share-network", iconColor: share.isOpen ? Theme.accentLight : Theme.accent,
                      title: ShareText.share, detail: detail,
                      trailing: share.isOpen ? AnyView(Toggle2(on: true)) : nil)
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: .share)
        .opacity(availability.isAvailable ? 1 : 0.5)
    }

    private var padsDetail: String {
        guard !pads.pads.isEmpty else { return String(localized: "Aucune manette") }
        return pads.pads.prefix(2).map { p in
            let kind = p.kind == .xbox ? "Xbox" : p.kind == .playstation ? HomeView.short(p.name) : String(localized: "Manette")
            return String(localized: "J\(p.player) \(kind)")
        }.joined(separator: " · ")
    }

    /// Le son : sa sortie ; un clic ouvre le détail et le choix de la sortie de l'Apple TV.
    private var soundTile: some View {
        let r = session.route
        let detail = session.host.settings.audio == .off ? String(localized: "Sur l'ordinateur")
            : r.kind == .hdmi ? String(localized: "\(r.channelsLabel) · \(r.name) · \(r.latencyMs) ms")
            : r.kind == .airpods && prefs.spatialAudio ? String(localized: "\(r.name) · audio spatial") : r.name
        return Button { sound = true } label: {
            tileLabel(icon: "speaker-high", iconColor: r.isLate ? Theme.warn : Theme.accent, title: String(localized: "Son"), detail: detail)
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: .sound)
    }

    /// « Qualité de l'image » (invité) : la qualité reçue (« 1080p · 60 Hz »,
    /// ou la qualité choisie tant que la taille n'est pas sue) ; un clic ouvre le choix des quatre.
    private var qualityTile: some View {
        Button { session.qualityPanel = true } label: {
            tileLabel(icon: "monitor", title: ShareText.imageQuality, detail: session.shareImageLabel)
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: .quality)
    }

    /// Le choix de la qualité : « Auto », « Comme l'hôte », « 4K · 60 Hz »,
    /// « 1080p · 60 Hz », « 720p · 30 Hz » ; le cœur relance l'image de l'invité seulement.
    private var qualityPanelView: some View {
        ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 24) {
                Theme.eyebrow(ShareText.imageQuality)
                Text(verbatim: session.shareImageLabel)
                    .font(Theme.display(44, 640)).tracking(-1.2)
                HStack(spacing: 16) {
                    ForEach(ShareQuality.allCases, id: \.self) { quality in
                        Button { session.setShareQuality(quality) } label: { Text(quality.label).lineLimit(1).fixedSize() }
                            .buttonStyle(ChoiceButton(selected: session.shareQuality == quality, height: 72))
                            .focused($focus, equals: .qualityChoice(quality))
                    }
                }
                Button("Fermer") { session.qualityPanel = false }
                    .buttonStyle(ChoiceButton(selected: false, height: 72))
            }
            .padding(56)
            .glass(30, tint: 0.6)
            .focusSection()
        }
        .frame(width: 1920, height: 1080)
        .onExitCommand { session.qualityPanel = false }
        .onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { focus = .qualityChoice(session.shareQuality) } }
    }

    private var soundPanel: some View {
        let r = session.route
        return ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 24) {
                Theme.eyebrow(String(localized: "Son"))
                (session.host.settings.audio == .off ? Text("Le son reste sur l'ordinateur") : Text("Son \(r.channelsLabel) · \(r.name)"))
                    .font(Theme.display(44, 640)).tracking(-1.2)
                (session.host.settings.audio == .off
                    ? (French.elides(session.host.name)
                        ? Text("Ses haut-parleurs ou son casque. Pour l'entendre ici, choisissez Auto dans Réglages d'\(session.host.name), Son.")
                        : Text("Ses haut-parleurs ou son casque. Pour l'entendre ici, choisissez Auto dans Réglages de \(session.host.name), Son."))
                    : r.isLate
                    ? Text("Sortie mesurée : \(r.latencyMs) ms. Une sortie sans fil ajoute du retard : la sortie HDMI reste la plus rapide.")
                    : Text("Sortie mesurée : \(r.latencyMs) ms. C'est la sortie la plus rapide de l'Apple TV."))
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
            tileLabel(icon: "cursor", title: String(localized: "Pavé en souris"),
                      detail: session.remoteMouse ? String(localized: "Télécommande : pavé = pointeur, clic = clic gauche") : String(localized: "Désactivée · le pavé navigue"),
                      trailing: AnyView(Toggle2(on: session.remoteMouse)))
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: .mouse)
        .accessibilityValue(session.remoteMouse ? Text("Activée") : Text("Désactivée"))
    }

    /// La manette en souris : stick gauche, A / ✕, X / □…
    private var padMouseTile: some View {
        Button { session.togglePadMouse() } label: {
            tileLabel(icon: "game-controller", title: String(localized: "Manette en souris"),
                      detail: session.padMouse ? String(localized: "Stick : pointeur · A clic · X clic droit") : String(localized: "Désactivée · la manette joue"),
                      trailing: AnyView(Toggle2(on: session.padMouse)))
        }
        .buttonStyle(GlassButton(radius: 22, scale: 1.07))
        .focused($focus, equals: .padMouse)
        .accessibilityValue(session.padMouse ? Text("Activée") : Text("Désactivée"))
    }

    // MARK: Texte

    private var typingPanel: some View {
        ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 26) {
                Theme.eyebrow(String(localized: "Taper du texte sur \(session.host.name)"))
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
