// Ajouter un ordinateur (maquettes iOS, planche 2) : les quatre voies de
// l'Apple TV (depuis le Mac par iCloud, sur le réseau, code à l'écran,
// adresse), plus le QR code de la console lu à la caméra.
//
// Sur iPhone, et sur iPad en Split View étroit, une fiche aux étapes
// empilées : les voies, le volet de la voie, la vérification de l'empreinte,
// le compte. Retour par le bouton du haut ou en glissant depuis le bord
// gauche ; la fiche se ferme en la tirant vers le bas. Sur iPad, deux
// colonnes : les voies à gauche, le détail à droite.
//
// Les volets sont dans AddHostPanes.swift, la caméra dans AddHostScanner.swift.

import KyberKit
import SwiftUI

// MARK: - Parcours

/// Les voies d'ajout, dans l'ordre des maquettes.
enum AddHostMethod: Int, CaseIterable, Identifiable {
    case cloud, network, code, address, qr

    var id: Int { rawValue }
}

/// Où en est l'ajout, une fois la voie choisie.
enum AddHostStage: Equatable {
    /// Le volet de la voie (liste, code, adresse, caméra).
    case pane
    /// Un ordinateur trouvé : l'empreinte de sa console, à vérifier.
    case verify(Candidate)
    /// L'empreinte retenue : le compte à saisir.
    case account(Candidate, fingerprint: String?, verified: Bool)

    /// Identité de l'étape, pour l'animation entre deux étapes.
    var key: String {
        switch self {
        case .pane: return "pane"
        case let .verify(c): return "verify-\(c.address):\(c.port)"
        case let .account(c, _, _): return "account-\(c.address):\(c.port)"
        }
    }
}

@MainActor
final class AddHostFlow: ObservableObject {
    /// La voie choisie ; aucune : la liste des voies (fiche iPhone).
    @Published var method: AddHostMethod?
    @Published var stage: AddHostStage = .pane

    init(link: Candidate?) {
        if let link {
            stage = .verify(link)
        }
        #if DEBUG
        // Vérification au simulateur : `-addMethod 0…4`, `-addStage account`.
        if let raw = UserDefaults.standard.string(forKey: "addMethod"), let n = Int(raw) {
            method = AddHostMethod(rawValue: n)
        }
        if UserDefaults.standard.string(forKey: "addStage") == "account" {
            stage = .account(Candidate(name: "Monstre", address: "192.168.1.57", port: 31877, expected: nil), fingerprint: nil, verified: true)
        }
        #endif
    }

    func pick(_ m: AddHostMethod) {
        method = m
        stage = .pane
    }

    func found(_ c: Candidate) {
        stage = .verify(c)
    }

    func trusted(_ c: Candidate, fingerprint: String?, verified: Bool) {
        stage = .account(c, fingerprint: fingerprint, verified: verified)
    }

    /// Un pas en arrière. Faux quand il n'y en a plus : la fiche se ferme.
    func back() -> Bool {
        switch stage {
        case .verify, .account:
            stage = .pane
            return true
        case .pane:
            guard method != nil else { return false }
            method = nil
            return true
        }
    }

    /// Identité de ce qui est affiché, pour l'animation.
    var key: String { "\(method?.rawValue ?? -1)-\(stage.key)" }

    /// Déjà dans les ordinateurs de cet appareil (même empreinte, ou même adresse et port).
    static func known(_ c: Candidate, in store: HostStore) -> Bool {
        store.hosts.contains { h in
            (h.consoleFingerprint != nil && c.expected != nil && AddHostFingerprint.same(h.consoleFingerprint, c.expected))
                || (h.address == c.address && h.consolePort == c.port)
        }
    }
}

/// Empreinte du certificat de la console (« sha256:ab:cd:… »).
enum AddHostFingerprint {
    static func hex(_ fp: String) -> String {
        fp.replacingOccurrences(of: "sha256:", with: "", options: .caseInsensitive)
            .replacingOccurrences(of: ":", with: "")
            .uppercased()
    }

    static func same(_ a: String?, _ b: String?) -> Bool {
        guard let a, let b else { return false }
        return hex(a) == hex(b)
    }

    /// Groupes de quatre caractères, comme la console les affiche (Accès).
    static func groups(_ fp: String) -> [String] {
        let h = Array(hex(fp))
        return stride(from: 0, to: h.count, by: 4).map { String(h[$0 ..< min($0 + 4, h.count)]) }
    }
}

// MARK: - Mise en page

/// Vrai dans la colonne de détail de l'iPad : titres plus grands, en-têtes
/// avec leur étiquette (sur iPhone, l'étiquette est dans la barre du haut).
private struct AddHostWideKey: EnvironmentKey {
    static let defaultValue = false
}

extension EnvironmentValues {
    var addHostWide: Bool {
        get { self[AddHostWideKey.self] }
        set { self[AddHostWideKey.self] = newValue }
    }
}

struct AddHostView: View {
    var link: Candidate?
    @StateObject private var flow: AddHostFlow

    init(link: Candidate?) {
        self.link = link
        _flow = StateObject(wrappedValue: AddHostFlow(link: link))
    }

    var body: some View {
        GeometryReader { geo in
            // Deux colonnes dès que la place le permet (iPad, Split View
            // large) ; sinon la fiche de l'iPhone. Le parcours en cours est
            // gardé quand on passe de l'une à l'autre.
            if geo.size.width >= 760 {
                AddHostSplit(width: geo.size.width)
            } else {
                AddHostStack()
            }
        }
        .background(AmbientBackground().ignoresSafeArea())
        .environmentObject(flow)
        .presentationSizing(AddHostSizing())
        .presentationDragIndicator(.hidden)
        .preferredColorScheme(.dark)
    }
}

/// La fiche aussi grande que l'écran le permet (sur iPad, deux colonnes ;
/// sur iPhone, la fiche est de toute façon plein écran).
private struct AddHostSizing: PresentationSizing {
    func proposedSize(for root: PresentationSizingRoot, context: PresentationSizingContext) -> ProposedViewSize {
        ProposedViewSize(width: 1300, height: 1000)
    }
}

// MARK: - iPhone : étapes empilées

private struct AddHostStack: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var flow: AddHostFlow
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 0) {
            navBar
            ScrollView {
                content
                    .id(flow.key)
                    .transition(reduceMotion ? .opacity : .opacity.combined(with: .offset(x: 24)))
                    .padding(.horizontal, 24)
                    .padding(.top, 12)
                    .padding(.bottom, 34)
                    .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .scrollDismissesKeyboard(.interactively)
            .scrollBounceBehavior(.basedOnSize)
        }
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : Theme.easeOut, value: flow.key)
        // Glisser depuis le bord gauche : un pas en arrière.
        .overlay(alignment: .leading) {
            Color.clear
                .frame(width: 18)
                .contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 12)
                        .onEnded { g in
                            if g.translation.width > 70, abs(g.translation.height) < 80 { back() }
                        }
                )
        }
    }

    private var atMenu: Bool { flow.method == nil && flow.stage == .pane }

    private var navBar: some View {
        HStack {
            Button(action: back) {
                HStack(spacing: 4) {
                    Icon("arrow-left", size: 20, bold: true)
                    if atMenu { Text("Vos ordinateurs") } else { Text("Ajouter") }
                }
                .font(Theme.sans(16))
                .foregroundStyle(Theme.accentLight)
                .padding(.leading, 6).padding(.trailing, 12)
                .frame(minHeight: 44)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .keyboardShortcut(.cancelAction)
            Spacer()
            Theme.eyebrow(stepLabel, color: Theme.faint)
        }
        .padding(.leading, 12).padding(.trailing, 20)
        .padding(.top, 12)
    }

    private var stepLabel: String {
        switch flow.stage {
        case .verify: return String(localized: "Vérification")
        case .account: return String(localized: "Dernière étape")
        case .pane:
            switch flow.method {
            case nil: return String(localized: "Quatre voies")
            case .cloud?, .code?: return String(localized: "Étape 1 sur 1")
            case .network?, .qr?: return String(localized: "Étape 1 sur 2")
            case .address?: return String(localized: "Étape 1 sur 3")
            }
        }
    }

    private func back() {
        if !flow.back() { app.sheet = nil }
    }

    @ViewBuilder private var content: some View {
        switch flow.stage {
        case let .verify(c): AddHostVerifyPane(candidate: c)
        case let .account(c, fp, verified): AddHostAccountPane(candidate: c, fingerprint: fp, verified: verified)
        case .pane:
            switch flow.method {
            case nil: AddHostMenu()
            case .cloud?: AddHostCloudPane()
            case .network?: AddHostNetworkPane()
            case .code?: AddHostCodePane()
            case .address?: AddHostAddressPane()
            case .qr?: AddHostQRPane()
            }
        }
    }
}

/// Première étape de la fiche : la caméra, puis les quatre voies.
private struct AddHostMenu: View {
    @EnvironmentObject private var flow: AddHostFlow

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Ajouter un ordinateur")
                .font(Theme.display(34, 660)).tracking(-34 * 0.03)
                .foregroundStyle(Theme.text)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
            cameraCard
            HStack(spacing: 12) {
                Rectangle().fill(Color.white.opacity(0.1)).frame(height: 1)
                Text("ou").font(Theme.sans(12)).foregroundStyle(Theme.ghost)
                Rectangle().fill(Color.white.opacity(0.1)).frame(height: 1)
            }
            VStack(spacing: 10) {
                ForEach([AddHostMethod.cloud, .network, .code, .address]) { m in
                    Button { flow.pick(m) } label: {
                        AddHostMethodLabel(method: m, selected: false, compact: true)
                    }
                    .buttonStyle(AddHostRowStyle())
                }
            }
        }
    }

    /// Le viseur : toucher ouvre la caméra (l'autorisation n'est demandée qu'à ce moment).
    private var cameraCard: some View {
        Button { flow.pick(.qr) } label: {
            VStack(spacing: 0) {
                Spacer(minLength: 14)
                AddHostViewfinder(size: 104, corner: 24)
                Spacer(minLength: 12)
                HStack(spacing: 10) {
                    AddHostQRGlyph().stroke(Theme.accent, style: StrokeStyle(lineWidth: 1.8, lineCap: .round, lineJoin: .round))
                        .frame(width: 18, height: 18)
                    Text("Visez le QR code d'appairage, dans KyberNow Admin (espace Accès). L'empreinte est vérifiée toute seule.")
                        .font(Theme.sans(13.5)).foregroundStyle(Theme.text2)
                        .multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.horizontal, 14).padding(.bottom, 12)
            }
            .frame(maxWidth: .infinity)
            .frame(height: 196)
            .background(LinearGradient(colors: [Theme.rgb(0x1A2232), Theme.rgb(0x0A0F17)], startPoint: .topLeading, endPoint: .bottomTrailing))
            .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Color.white.opacity(0.1)))
            .contentShape(Rectangle())
        }
        .buttonStyle(AddHostPressStyle())
        .accessibilityLabel(Text("Lire le QR code d'appairage avec la caméra"))
    }
}

// MARK: - iPad : deux colonnes

private struct AddHostSplit: View {
    let width: CGFloat
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var flow: AddHostFlow
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// La voie allumée à gauche : la voie choisie, iCloud par défaut. Un
    /// lien reçu n'en allume aucune.
    private var shown: AddHostMethod? {
        if let m = flow.method { return m }
        return flow.stage == .pane ? .cloud : nil
    }

    var body: some View {
        let side: CGFloat = width > 1100 ? 64 : 40
        let column: CGFloat = min(460, max(340, width * 0.36))
        VStack(alignment: .leading, spacing: 30) {
            Button { app.sheet = nil } label: {
                HStack(spacing: 8) {
                    Icon("arrow-left", size: 18, bold: true)
                    Text("Vos ordinateurs")
                }
                .font(Theme.sans(15, .medium))
                .foregroundStyle(Theme.text)
                .padding(.leading, 12).padding(.trailing, 18)
                .frame(height: 44)
                .glass(22)
                .contentShape(Capsule())
            }
            .buttonStyle(AddHostPressStyle())
            .keyboardShortcut(.cancelAction)
            HStack(alignment: .top, spacing: width > 1100 ? 64 : 36) {
                methods.frame(width: column)
                detail
            }
        }
        .padding(.horizontal, side)
        .padding(.top, 28)
        .padding(.bottom, 36)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var methods: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Ajouter un ordinateur")
                .font(Theme.display(40, 660)).tracking(-40 * 0.035)
                .foregroundStyle(Theme.text)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
            Text("Sans rien taper, de préférence.")
                .font(Theme.sans(16)).foregroundStyle(Theme.muted)
                .padding(.bottom, 12)
            ForEach(AddHostMethod.allCases) { m in
                let on = shown == m
                Button { flow.pick(m) } label: {
                    AddHostMethodLabel(method: m, selected: on, compact: false)
                }
                .buttonStyle(AddHostMethodStyle(selected: on))
                .accessibilityAddTraits(on ? .isSelected : [])
            }
            Spacer(minLength: 16)
            Text("Un lien kybernow:// reçu par AirDrop ou Messages ouvre directement cette fiche, pré-remplie.")
                .font(Theme.sans(13)).foregroundStyle(Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxHeight: .infinity, alignment: .top)
    }

    private var detail: some View {
        ScrollView {
            Group {
                switch flow.stage {
                case let .verify(c): AddHostVerifyPane(candidate: c)
                case let .account(c, fp, verified): AddHostAccountPane(candidate: c, fingerprint: fp, verified: verified)
                case .pane:
                    switch shown ?? .cloud {
                    case .cloud: AddHostCloudPane()
                    case .network: AddHostNetworkPane()
                    case .code: AddHostCodePane()
                    case .address: AddHostAddressPane()
                    case .qr: AddHostQRPane()
                    }
                }
            }
            .id(flow.key)
            .transition(reduceMotion ? .opacity : .opacity.combined(with: .offset(x: 24)))
            .padding(.horizontal, 44).padding(.vertical, 40)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .scrollDismissesKeyboard(.interactively)
        .scrollBounceBehavior(.basedOnSize)
        .environment(\.addHostWide, true)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .glass(26)
        .clipShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
        .animation(reduceMotion ? .easeInOut(duration: 0.2) : Theme.easeOut, value: flow.key)
    }
}

// MARK: - Les voies

/// Une voie : icône, titre, sous-titre, badge.
private struct AddHostMethodLabel: View {
    let method: AddHostMethod
    let selected: Bool
    let compact: Bool
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var discovery: Discovery

    var body: some View {
        HStack(spacing: compact ? 14 : 16) {
            icon
                .foregroundStyle(compact ? Theme.accentLight : selected ? Color.white : Theme.muted)
                .frame(width: compact ? 44 : 48, height: compact ? 44 : 48)
                .background(
                    RoundedRectangle(cornerRadius: compact ? 12 : 13, style: .continuous)
                        .fill(compact ? Theme.accentFill.opacity(0.18) : selected ? Theme.accentFill : Color.white.opacity(0.06))
                )
            VStack(alignment: .leading, spacing: compact ? 3 : 4) {
                if method == .cloud {
                    // Le badge à côté du titre s'il y tient, sinon dessous.
                    ViewThatFits(in: .horizontal) {
                        HStack(spacing: 8) { titleText; badge }
                        VStack(alignment: .leading, spacing: 4) { titleText; badge }
                    }
                } else {
                    titleText
                }
                Text(subtitle).font(Theme.sans(compact ? 12.5 : 13.5)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .multilineTextAlignment(.leading)
            Spacer(minLength: 4)
            if compact {
                Icon("arrow-right", size: 16, bold: true).foregroundStyle(Theme.ghost)
            }
        }
    }

    private var titleText: some View {
        Text(title).font(Theme.sans(compact ? 16 : 18, .semibold)).foregroundStyle(Theme.text)
            .fixedSize(horizontal: false, vertical: true)
    }

    private var badge: some View {
        Text(String(localized: "Conseillé").uppercased())
            .font(Theme.mono(compact ? 9.5 : 10, .medium)).tracking(0.6)
            .foregroundStyle(Theme.accentLight)
            .lineLimit(1)
            .fixedSize()
            .padding(.horizontal, 8).padding(.vertical, 4)
            .background(RoundedRectangle(cornerRadius: 6).fill(Theme.accentFill.opacity(0.2)))
    }

    @ViewBuilder private var icon: some View {
        switch method {
        case .cloud: Icon("cloud", size: 22)
        case .network: Icon("wifi-high", size: 22)
        case .code: Icon("number-square-one", size: 22)
        case .address: Icon("globe-hemisphere-west", size: 22)
        case .qr: AddHostQRGlyph().stroke(style: StrokeStyle(lineWidth: 1.7, lineCap: .round, lineJoin: .round)).frame(width: 22, height: 22)
        }
    }

    private var title: String {
        switch method {
        case .cloud: return String(localized: "Depuis votre Mac")
        case .network: return String(localized: "Sur le réseau")
        case .code: return String(localized: "Code à l'écran")
        case .address: return String(localized: "Adresse")
        case .qr: return String(localized: "QR code d'appairage")
        }
    }

    private var subtitle: String {
        switch method {
        case .cloud: return String(localized: "Synchronisés par iCloud, avec leurs accès")
        case .network:
            let n = app.newOnNetwork.count
            if n == 0 { return String(localized: "Trouvé par Bonjour · compte à saisir") }
            if n == 1 { return String(localized: "1 ordinateur trouvé · compte à saisir") }
            return String(localized: "\(n) ordinateurs trouvés · compte à saisir")
        case .code: return String(localized: "À accepter dans KyberNow Admin")
        case .address: return String(localized: "VPN ou réseau sans découverte")
        case .qr: return DeviceDisplay.isPad ? String(localized: "Avec la caméra de l'iPad") : String(localized: "Avec la caméra de l'iPhone")
        }
    }
}

/// Rangée de la fiche iPhone : verre léger, filet clair.
struct AddHostRowStyle: ButtonStyle {
    var minHeight: CGFloat = 72

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .padding(.leading, 12).padding(.trailing, 14)
            .frame(maxWidth: .infinity, minHeight: minHeight, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color.white.opacity(0.05)))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
            .contentShape(Rectangle())
    }
}

/// Voie de la colonne de l'iPad : allumée quand elle est choisie, veille au survol.
private struct AddHostMethodStyle: ButtonStyle {
    let selected: Bool

    func makeBody(configuration: Configuration) -> some View {
        AddHostHoverRow(selected: selected, pressed: configuration.isPressed) { configuration.label }
    }
}

private struct AddHostHoverRow<Label: View>: View {
    let selected: Bool
    let pressed: Bool
    @ViewBuilder let label: () -> Label
    @State private var hover = false

    var body: some View {
        label()
            .padding(.horizontal, 18)
            .frame(maxWidth: .infinity, minHeight: 80, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(selected ? Color.white.opacity(0.1) : Color.clear))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .strokeBorder(Color.white.opacity(selected ? 0.16 : hover ? 0.22 : 0))
            )
            .scaleEffect(pressed ? 0.985 : 1)
            .animation(Theme.focus, value: pressed)
            .animation(Theme.focus, value: hover)
            .contentShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
            .onHover { hover = $0 }
    }
}

/// Simple enfoncement au toucher.
struct AddHostPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

// MARK: - Viseur

/// Les quatre coins d'un viseur (petite icône du QR code).
struct AddHostQRGlyph: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        let k = r.width / 24
        func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: r.minX + x * k, y: r.minY + y * k) }
        p.move(to: pt(4, 8)); p.addLine(to: pt(4, 4)); p.addLine(to: pt(8, 4))
        p.move(to: pt(16, 4)); p.addLine(to: pt(20, 4)); p.addLine(to: pt(20, 8))
        p.move(to: pt(20, 16)); p.addLine(to: pt(20, 20)); p.addLine(to: pt(16, 20))
        p.move(to: pt(8, 20)); p.addLine(to: pt(4, 20)); p.addLine(to: pt(4, 16))
        p.addEllipse(in: CGRect(origin: pt(9, 9), size: CGSize(width: 6 * k, height: 6 * k)))
        return p
    }
}

/// Coins du viseur et ligne de balayage (immobile si « Réduire les animations »).
struct AddHostViewfinder: View {
    var size: CGFloat
    var corner: CGFloat
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var sweep = false

    init(size: CGFloat, corner: CGFloat) {
        self.size = size
        self.corner = corner
    }

    var body: some View {
        ZStack {
            ForEach(0 ..< 4) { i in
                AddHostCorner(length: corner)
                    .stroke(Theme.text, style: StrokeStyle(lineWidth: 3, lineCap: .round, lineJoin: .round))
                    .rotationEffect(.degrees(Double(i) * 90))
            }
            if !reduceMotion {
                Rectangle()
                    .fill(Theme.accent)
                    .frame(width: size * 0.76, height: 2)
                    .shadow(color: Theme.accent, radius: 6)
                    .offset(y: sweep ? size * 0.32 : -size * 0.32)
            }
        }
        .frame(width: size, height: size)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
        .onAppear {
            guard !reduceMotion else { return }
            withAnimation(.easeInOut(duration: 2.4).repeatForever(autoreverses: true)) { sweep = true }
        }
    }
}

/// Le coin haut gauche du viseur ; les trois autres en sont des rotations.
private struct AddHostCorner: Shape {
    let length: CGFloat

    func path(in r: CGRect) -> Path {
        var p = Path()
        let radius: CGFloat = 6
        p.move(to: CGPoint(x: r.minX, y: r.minY + length))
        p.addLine(to: CGPoint(x: r.minX, y: r.minY + radius))
        p.addQuadCurve(to: CGPoint(x: r.minX + radius, y: r.minY), control: CGPoint(x: r.minX, y: r.minY))
        p.addLine(to: CGPoint(x: r.minX + length, y: r.minY))
        return p
    }
}
