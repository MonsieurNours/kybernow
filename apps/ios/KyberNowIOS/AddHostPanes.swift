// Les volets de l'ajout (AddHostView) : depuis le Mac (iCloud), sur le
// réseau, code à l'écran, adresse ; puis la vérification de l'empreinte et
// le compte. La logique est celle de l'Apple TV et du Mac ; la mise en page
// suit les maquettes iOS (planche 2).

import KyberKit
import SwiftUI
import UIKit

// MARK: - Éléments communs

/// En-tête d'un volet : sur iPhone le titre seul (l'étiquette est dans la
/// barre du haut), sur iPad l'étiquette puis le titre.
struct AddHostHeader<Trailing: View>: View {
    let eyebrow: String
    let compactTitle: String
    let wideTitle: String
    @ViewBuilder var trailing: () -> Trailing
    @Environment(\.addHostWide) private var wide

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 18 : 14) {
            if wide {
                HStack(spacing: 12) {
                    Theme.eyebrow(eyebrow)
                    trailing()
                }
            }
            Text(wide ? wideTitle : compactTitle)
                .font(Theme.display(wide ? 36 : 34, wide ? 640 : 660))
                .tracking(-(wide ? 36 : 34) * 0.03)
                .foregroundStyle(Theme.text)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
        }
    }
}

extension AddHostHeader where Trailing == EmptyView {
    init(eyebrow: String, title: String) {
        self.init(eyebrow: eyebrow, compactTitle: title, wideTitle: title) { EmptyView() }
    }

    init(eyebrow: String, compactTitle: String, wideTitle: String) {
        self.init(eyebrow: eyebrow, compactTitle: compactTitle, wideTitle: wideTitle) { EmptyView() }
    }
}

/// Paragraphe d'explication.
struct AddHostParagraph: View {
    let text: String
    @Environment(\.addHostWide) private var wide

    init(_ text: String) { self.text = text }

    var body: some View {
        Text(text)
            .font(Theme.sans(wide ? 16 : 14.5))
            .foregroundStyle(Theme.text2)
            .lineSpacing(wide ? 5 : 4)
            .frame(maxWidth: wide ? 600 : .infinity, alignment: .leading)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// Trois points qui patientent (immobiles si « Réduire les animations »).
struct AddHostWaitingDots: View {
    var size: CGFloat = 6
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var on = false

    var body: some View {
        HStack(spacing: -size * 0.8 + size) {
            ForEach(0 ..< 3) { i in
                Circle().fill(Theme.accent).frame(width: size, height: size)
                    .opacity(reduceMotion ? 0.8 : on ? 1 : 0.25)
                    .animation(reduceMotion ? nil : .easeInOut(duration: 0.7).repeatForever().delay(Double(i) * 0.2), value: on)
            }
        }
        .accessibilityHidden(true)
        .onAppear { on = true }
    }
}

/// Encadré d'avertissement (orangé) ou de refus (rouge) : jamais le signal.
struct AddHostNotice: View {
    enum Kind { case warn, danger }
    let kind: Kind
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Icon("warning", size: 20).foregroundStyle(kind == .warn ? Theme.warn : Theme.danger)
            Text(text)
                .font(Theme.sans(14)).foregroundStyle(kind == .warn ? Theme.warnText : Theme.dangerText)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill((kind == .warn ? Theme.warn : Theme.danger).opacity(0.1)))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder((kind == .warn ? Theme.warn : Theme.danger).opacity(0.35)))
    }
}

/// Champ de saisie : étiquette au-dessus, encre, filet allumé au focus.
struct AddHostField: View {
    let label: String
    @Binding var text: String
    var prompt = ""
    var mono = false
    var secure = false
    var keyboard: UIKeyboardType = .default
    var content: UITextContentType?
    var submit: SubmitLabel = .next
    var onSubmit: () -> Void = {}
    @FocusState private var focused: Bool
    @Environment(\.addHostWide) private var wide

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 8 : 6) {
            Text(label).font(Theme.sans(wide ? 14 : 13)).foregroundStyle(Theme.muted)
            Group {
                if secure {
                    SecureField(text: $text, prompt: Text(prompt).foregroundStyle(Theme.ghost)) { Text(label) }
                } else {
                    TextField(text: $text, prompt: Text(prompt).foregroundStyle(Theme.ghost)) { Text(label) }
                }
            }
            .textContentType(content)
            .keyboardType(keyboard)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(submit)
            .onSubmit(onSubmit)
            .focused($focused)
            .font(mono ? Theme.mono(wide ? 16 : 15) : Theme.sans(wide ? 17 : 16))
            .foregroundStyle(Theme.text)
            .tint(Theme.accent)
            .padding(.horizontal, wide ? 18 : 16)
            .frame(height: wide ? 52 : 50)
            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.surface))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(focused ? Theme.accent : Theme.line2))
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(Theme.accent.opacity(focused ? 0.18 : 0), lineWidth: 3)
                    .padding(-2)
            )
            .contentShape(Rectangle())
            .onTapGesture { focused = true }
            .animation(.easeOut(duration: 0.15), value: focused)
        }
    }
}

/// Bouton de refus : rouge pâle, jamais le signal.
struct AddHostDangerButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Theme.sans(15, .semibold))
            .foregroundStyle(Theme.dangerText)
            .padding(.horizontal, 20)
            .frame(minHeight: 50)
            .frame(maxWidth: .infinity)
            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Theme.danger.opacity(0.12)))
            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Theme.danger.opacity(0.35)))
            .opacity(configuration.isPressed ? 0.75 : 1)
            .contentShape(Rectangle())
    }
}

/// Le bouton principal, pleine largeur sur iPhone, à sa taille sur iPad.
private struct AddHostPrimary: ViewModifier {
    @Environment(\.addHostWide) private var wide
    @Environment(\.isEnabled) private var enabled

    func body(content: Content) -> some View {
        content
            .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
            .frame(maxWidth: wide ? 360 : .infinity)
            .opacity(enabled ? 1 : 0.45)
            .saturation(enabled ? 1 : 0.6)
    }
}

extension View {
    func addHostPrimary() -> some View { modifier(AddHostPrimary()) }
}

/// Ce que l'appareil dit de lui dans les textes.
enum AddHostDevice {
    static var alreadyHere: String {
        DeviceDisplay.isPad ? String(localized: "Déjà sur cet iPad") : String(localized: "Déjà sur cet iPhone")
    }
}

// MARK: - Depuis votre Mac (iCloud)

struct AddHostCloudPane: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @EnvironmentObject private var cloud: CloudSync
    @ObservedObject private var prefs = Preferences.shared
    @Environment(\.addHostWide) private var wide
    @State private var missing: String?

    init() {}

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 20 : 14) {
            AddHostHeader(eyebrow: String(localized: "Trousseau iCloud"), compactTitle: String(localized: "Depuis votre Mac"), wideTitle: String(localized: "Vos ordinateurs suivent votre Mac"))
            AddHostParagraph(String(localized: "Les ordinateurs ajoutés dans KyberNow Link sur votre Mac arrivent ici avec leurs accès, par iCloud. Rien à taper, et un mot de passe changé sur le Mac suit tout seul."))
            VStack(spacing: 10) {
                if cloud.hosts.isEmpty {
                    Text("Aucun ordinateur publié par votre Mac pour l'instant. Ouvrez KyberNow Link sur le Mac, avec le même compte iCloud.")
                        .font(Theme.sans(wide ? 15 : 14)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.horizontal, 16).padding(.vertical, 16)
                        .frame(maxWidth: .infinity, minHeight: 64, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color.white.opacity(0.04)))
                }
                ForEach(cloud.hosts) { c in
                    if store.host(c.id) != nil {
                        HStack(spacing: 14) {
                            Icon("check", size: 22, bold: true).foregroundStyle(Theme.ok)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(c.name).font(Theme.sans(wide ? 18 : 16, .semibold)).foregroundStyle(Theme.text)
                                Text(AddHostDevice.alreadyHere).font(Theme.sans(wide ? 13.5 : 12.5)).foregroundStyle(Theme.muted)
                            }
                            Spacer(minLength: 0)
                        }
                        .padding(.horizontal, 16)
                        .frame(minHeight: wide ? 72 : 64)
                        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color.white.opacity(0.04)))
                        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
                        .accessibilityElement(children: .combine)
                    } else {
                        Button { add(c) } label: {
                            HStack(spacing: 14) {
                                Icon("plus", size: 22, bold: true).foregroundStyle(Theme.accent)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(c.name).font(Theme.sans(wide ? 18 : 16, .semibold)).foregroundStyle(Theme.text)
                                    if let from = c.from {
                                        Text("Publié par « \(from) »").font(Theme.sans(wide ? 13.5 : 12.5)).foregroundStyle(Theme.muted)
                                    }
                                }
                                Spacer(minLength: 0)
                                Text("Ajouter").font(Theme.sans(wide ? 15 : 14, .semibold)).foregroundStyle(Theme.accentLight)
                            }
                            .padding(.horizontal, 16)
                            .frame(maxWidth: .infinity, minHeight: wide ? 72 : 64)
                            .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Theme.accentFill.opacity(0.14)))
                            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Theme.accent.opacity(0.5)))
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(AddHostPressStyle())
                    }
                }
            }
            .padding(.top, 6)
            if let missing {
                AddHostNotice(kind: .warn, text: missing)
            }
            Toggle(isOn: $prefs.icloudSync) {
                Text("Synchroniser avec iCloud · les ordinateurs de votre Mac arrivent tout seuls, avec leurs accès.")
                    .font(Theme.sans(wide ? 14 : 13)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .tint(Theme.accentFill)
            .frame(minHeight: 44)
            .padding(.top, 12)
        }
    }

    /// Le mot de passe vient du Trousseau iCloud : il peut arriver après la liste.
    private func add(_ c: CloudHost) {
        if app.addFromCloud(c) {
            app.selected = c.id
            app.sheet = nil
            app.show(String(localized: "\(c.name) est ajouté"))
        } else {
            missing = String(localized: "Le mot de passe de \(c.name) n'est pas encore arrivé par le Trousseau iCloud. Réessayez dans un instant.")
        }
    }
}

// MARK: - Sur le réseau

struct AddHostNetworkPane: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var discovery: Discovery
    @EnvironmentObject private var flow: AddHostFlow
    @Environment(\.addHostWide) private var wide
    /// Adresse et port de chaque annonce, résolus en avance pour les montrer.
    @State private var resolved: [String: Candidate] = [:]
    @State private var resolving: String?

    init() {}

    var body: some View {
        let list = app.newOnNetwork
        VStack(alignment: .leading, spacing: wide ? 20 : 14) {
            AddHostHeader(eyebrow: String(localized: "Recherche sur le réseau local"), compactTitle: String(localized: "Sur le réseau"), wideTitle: String(localized: "Trouvé autour de vous")) {
                AddHostWaitingDots(size: 7)
            }
            if !wide {
                HStack(spacing: 10) {
                    Theme.eyebrow(String(localized: "Recherche sur le réseau local"))
                    AddHostWaitingDots()
                }
            }
            if list.isEmpty {
                Text("Rien de nouveau pour l'instant.")
                    .font(Theme.sans(wide ? 16 : 14.5)).foregroundStyle(Theme.muted)
            }
            ForEach(list) { found in
                Button { pick(found) } label: { row(found) }
                    .buttonStyle(AddHostPressStyle())
            }
            Text("Un ordinateur n'apparaît pas ? Il doit être allumé, sur le même réseau, avec KyberNow Station installée. Par VPN, passez par l'adresse ou le QR code.")
                .font(Theme.sans(wide ? 15 : 13.5)).foregroundStyle(Theme.muted)
                .lineSpacing(3)
                .frame(maxWidth: wide ? 600 : .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, 6)
        }
        .task(id: discovery.found.map(\.id)) {
            for f in discovery.found where resolved[f.id] == nil {
                if let c = await Discovery.resolve(f) { resolved[f.id] = c }
            }
        }
    }

    private func row(_ found: Discovery.Found) -> some View {
        HStack(spacing: 14) {
            Icon("monitor", size: wide ? 28 : 24).foregroundStyle(Theme.accent)
                .frame(width: wide ? 56 : 48, height: wide ? 56 : 48)
                .background(RoundedRectangle(cornerRadius: wide ? 14 : 12, style: .continuous).fill(Theme.surface))
            VStack(alignment: .leading, spacing: wide ? 5 : 3) {
                Text(found.name)
                    .font(Theme.display(wide ? 26 : 22, 640)).tracking(-0.4)
                    .foregroundStyle(Theme.text)
                    .lineLimit(1).minimumScaleFactor(0.7)
                Text(detail(found))
                    .font(Theme.mono(wide ? 12.5 : 11)).foregroundStyle(Theme.muted)
                    .lineLimit(1).minimumScaleFactor(0.8)
                if found.fingerprint != nil, resolved[found.id] != nil {
                    Text("empreinte annoncée")
                        .font(Theme.mono(wide ? 12.5 : 11)).foregroundStyle(Theme.faint)
                }
            }
            Spacer(minLength: 4)
            Text("Ajouter").font(Theme.sans(wide ? 16 : 14, .semibold)).foregroundStyle(Theme.accentLight)
        }
        .padding(.leading, 12).padding(.trailing, 16)
        .frame(maxWidth: .infinity, minHeight: wide ? 96 : 84)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Theme.accentFill.opacity(0.14)))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Theme.accent.opacity(0.6)))
        // Trouvé : le néon en veille.
        .shadow(color: Theme.accentFill.opacity(0.28), radius: wide ? 14 : 11)
        .contentShape(Rectangle())
    }

    private func detail(_ found: Discovery.Found) -> String {
        if resolving == found.id { return String(localized: "Recherche de son adresse…") }
        guard let c = resolved[found.id] else { return String(localized: "Annoncé par KyberNow Station") }
        return String(localized: "\(c.address) · port \(String(c.port))")
    }

    private func pick(_ found: Discovery.Found) {
        if let c = resolved[found.id] {
            flow.found(c)
            return
        }
        resolving = found.id
        Task {
            let c = await Discovery.resolve(found)
            resolving = nil
            if let c {
                resolved[found.id] = c
                flow.found(c)
            } else {
                app.show(String(localized: "\(found.name) ne répond pas. Il est peut-être en veille."))
            }
        }
    }
}

// MARK: - Code à l'écran

/// L'appareil tire un code et le présente à toutes les Stations trouvées sur
/// le réseau (Bonjour) ; la première qui l'accepte (Appareils Link) lui donne
/// son propre accès. Valable cinq minutes, puis un autre code est tiré.
struct AddHostCodePane: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var discovery: Discovery
    @Environment(\.addHostWide) private var wide
    @State private var code = AddHostCodePane.draw()
    @State private var expires = Date().addingTimeInterval(300)
    @State private var unsupported = false
    @State private var refused: String?

    init() {}

    private static func draw() -> String { String(format: "%06d", Int.random(in: 0 ... 999_999)) }

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 20 : 16) {
            AddHostHeader(eyebrow: String(localized: "Appairage par code"), title: String(localized: "Acceptez ce code sur l'ordinateur"))
            // Le moment actif de ce volet : le code allumé en néon.
            Text(verbatim: "\(code.prefix(3)) \(code.suffix(3))")
                .font(Theme.mono(wide ? 92 : 64, .medium))
                .tracking((wide ? 92 : 64) * 0.06)
                .foregroundStyle(Theme.neonCore)
                .lineLimit(1)
                .minimumScaleFactor(0.5)
                .neon(.lit)
                .frame(maxWidth: .infinity, alignment: wide ? .leading : .center)
                .padding(.vertical, wide ? 6 : 10)
                .accessibilityLabel(Text("Code \(code.map(String.init).joined(separator: " "))"))
            VStack(alignment: .leading, spacing: 10) {
                step(1, String(localized: "Sur l'ordinateur, ouvrez KyberNow Admin (icône KyberNow Station près de l'horloge)."))
                step(2, String(localized: "Appareils Link › « \(deviceName) » demande l'accès › Accepter."))
                step(3, DeviceDisplay.isPad
                    ? String(localized: "L'iPad reçoit son propre accès, révocable à tout moment depuis Admin.")
                    : String(localized: "L'iPhone reçoit son propre accès, révocable à tout moment depuis Admin."))
            }
            .frame(maxWidth: wide ? 600 : .infinity, alignment: .leading)
            status.padding(.top, 10)
        }
        .task(id: code) { await pairLoop() }
    }

    private var deviceName: String {
        ConsoleClient.thisDevice["name"] as? String ?? (DeviceDisplay.isPad ? "iPad" : "iPhone")
    }

    private func step(_ n: Int, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(verbatim: "\(n).").font(Theme.sans(wide ? 16 : 14.5, .semibold)).foregroundStyle(Theme.accent)
            Text(text).font(Theme.sans(wide ? 16 : 14.5)).foregroundStyle(Theme.text2)
                .lineSpacing(3)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder private var status: some View {
        if unsupported {
            AddHostNotice(kind: .warn, text: String(localized: "Les ordinateurs trouvés n'ont pas encore la version de KyberNow Station qui accepte les codes : mettez-la à jour sur l'ordinateur, ou passez par l'adresse."))
        } else if let refused {
            AddHostNotice(kind: .warn, text: refused)
        } else {
            TimelineView(.periodic(from: .now, by: 1)) { context in
                let waiting = HStack(spacing: 10) {
                    AddHostWaitingDots(size: 7)
                    Text(discovery.found.isEmpty ? String(localized: "Aucune Station sur ce réseau pour l'instant…") : String(localized: "En attente d'un ordinateur…"))
                        .font(Theme.sans(wide ? 15 : 13.5)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                let validity = Text("valable \(remaining(at: context.date))")
                    .font(Theme.mono(wide ? 13 : 11.5)).foregroundStyle(Theme.faint)
                // Sur une ligne si elle suffit, sinon la validité passe dessous.
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: 10) {
                        waiting.fixedSize()
                        Spacer(minLength: 8)
                        validity.fixedSize()
                    }
                    VStack(alignment: .leading, spacing: 6) {
                        waiting
                        validity.padding(.leading, 31)
                    }
                }
                .onChange(of: context.date) { _, now in
                    if now > expires { renew() }
                }
            }
        }
    }

    private func remaining(at now: Date) -> String {
        let s = max(0, Int(expires.timeIntervalSince(now)))
        return "\(s / 60) min \(String(format: "%02d", s % 60))"
    }

    private func renew() {
        code = Self.draw()
        expires = Date().addingTimeInterval(300)
        refused = nil
    }

    private struct Request {
        let candidate: Candidate
        let console: ConsoleClient
        let id: String
    }

    /// Demande envoyée à chaque Station trouvée (et à celles qui arrivent
    /// pendant l'attente), puis attente de la réponse.
    private func pairLoop() async {
        var asked: Set<String> = []
        var requests: [Request] = []
        var outdated = 0
        while !Task.isCancelled {
            for found in discovery.found where !asked.contains(found.id) {
                asked.insert(found.id)
                guard let c = await Discovery.resolve(found) else { continue }
                let console = ConsoleClient(host: c.address, port: c.port, username: "", password: "", fingerprint: nil)
                do {
                    let id = try await console.pairRequest(code: code)
                    requests.append(Request(candidate: c, console: console, id: id))
                } catch ConsoleError.http(404, _) {
                    outdated += 1
                } catch {
                    continue
                }
            }
            if Task.isCancelled { return }
            unsupported = requests.isEmpty && outdated > 0 && outdated == asked.count
            for r in requests {
                guard let state = try? await r.console.pairStatus(r.id) else { continue }
                switch state {
                case let .accepted(username, password):
                    // L'empreinte annoncée (Bonjour) doit être celle vue.
                    if let expected = r.candidate.expected, let seen = r.console.fingerprint, !AddHostFingerprint.same(expected, seen) { continue }
                    let check = ConsoleClient(host: r.candidate.address, port: r.candidate.port, username: username, password: password, fingerprint: r.console.fingerprint)
                    let info = try? await check.info()
                    var host = Host(name: info?.hostname ?? r.candidate.name, address: r.candidate.address, consolePort: r.candidate.port, username: username)
                    host.consoleFingerprint = r.console.fingerprint
                    app.store.add(host, password: password)
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    app.selected = host.id
                    app.sheet = nil
                    app.show(String(localized: "\(host.name) est ajouté"))
                    return
                case .refused:
                    refused = String(localized: "\(r.candidate.name) a refusé l'accès.")
                    requests.removeAll { $0.id == r.id }
                case .expired:
                    renew()
                    return
                case .pending:
                    break
                }
            }
            try? await Task.sleep(for: .seconds(1.5))
        }
    }
}

// MARK: - Adresse

struct AddHostAddressPane: View {
    @EnvironmentObject private var flow: AddHostFlow
    @EnvironmentObject private var store: HostStore
    @Environment(\.addHostWide) private var wide
    @State private var address = ""
    @State private var port = ""
    @State private var error: String?

    init() {}

    private var host: String { address.trimmingCharacters(in: .whitespacesAndNewlines) }
    private var portNumber: UInt16? { UInt16(port.trimmingCharacters(in: .whitespaces)) }

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 20 : 14) {
            AddHostHeader(eyebrow: String(localized: "VPN, autre réseau"), title: String(localized: "Son adresse et son port"))
            AddHostParagraph(String(localized: "Affichés dans KyberNow Admin, espace Appareils Link. Le port a été tiré au hasard à l'installation."))
            if wide {
                HStack(alignment: .top, spacing: 16) {
                    addressField.frame(maxWidth: 400)
                    portField.frame(width: 180)
                }
            } else {
                addressField
                portField.frame(width: 160)
            }
            if let error {
                AddHostNotice(kind: .warn, text: error)
            }
            HStack(spacing: 16) {
                Button("Continuer", action: next)
                    .addHostPrimary()
                    .disabled(host.isEmpty || portNumber == nil)
                if wide { caption }
            }
            .padding(.top, 8)
            if !wide { caption }
        }
    }

    private var addressField: some View {
        AddHostField(label: String(localized: "Adresse ou nom"), text: $address, prompt: "10.8.0.12", mono: true, keyboard: .URL, content: .URL)
    }

    private var portField: some View {
        AddHostField(label: String(localized: "Port d'Admin"), text: $port, prompt: "31877", mono: true, keyboard: .numberPad, submit: .continue, onSubmit: next)
    }

    private var caption: some View {
        Text("Ensuite : l'empreinte à comparer, puis le compte.")
            .font(Theme.sans(wide ? 14 : 13)).foregroundStyle(Theme.faint)
    }

    private func next() {
        guard !host.isEmpty, let p = portNumber, p > 0 else { return }
        let c = Candidate(name: host, address: host, port: p, expected: nil)
        if AddHostFlow.known(c, in: store) {
            error = String(localized: "Cet ordinateur est déjà dans vos ordinateurs.")
            return
        }
        error = nil
        flow.found(c)
    }
}

// MARK: - Vérifier l'empreinte

/// Admin présente son certificat : son empreinte est comparée à celle du
/// lien ou de l'annonce (vérifiée toute seule), sinon montrée pour être
/// comparée à l'œil avec celle qu'affiche Admin.
struct AddHostVerifyPane: View {
    let candidate: Candidate
    @EnvironmentObject private var flow: AddHostFlow
    @Environment(\.addHostWide) private var wide
    @State private var seen: String?
    @State private var probed = false
    @State private var attempt = 0

    init(candidate: Candidate) {
        self.candidate = candidate
    }

    private var mismatch: Bool { candidate.expected != nil && seen != nil && !AddHostFingerprint.same(candidate.expected, seen) }

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 20 : 14) {
            AddHostHeader(eyebrow: String(localized: "Vérification"), title: String(localized: "C'est bien cet ordinateur ?"))
            if !probed {
                HStack(spacing: 12) {
                    ProgressView().tint(Theme.accent)
                    Text("Connexion à la Station de \(candidate.name)…")
                        .font(Theme.sans(wide ? 16 : 14.5)).foregroundStyle(Theme.muted)
                }
                .padding(.top, 6)
            } else if seen == nil {
                AddHostNotice(kind: .warn, text: String(localized: "La Station ne répond pas à \(candidate.address):\(String(candidate.port)). Vérifiez l'adresse, le port, et que l'ordinateur est allumé."))
                actions {
                    Button("Réessayer") { attempt += 1 }.addHostPrimary()
                    Button("Retour") { _ = flow.back() }.buttonStyle(GlassButton(height: 50, fontSize: 15))
                }
            } else if mismatch {
                AddHostNotice(kind: .danger, text: String(localized: "Le certificat de la Station ne correspond pas à celui annoncé : ce n'est peut-être pas votre ordinateur. Ne l'ajoutez pas."))
                actions {
                    Button("Retour") { _ = flow.back() }.buttonStyle(AddHostDangerButton()).frame(maxWidth: wide ? 260 : .infinity)
                }
            } else if let seen {
                manual(seen)
            }
        }
        .task(id: attempt) { await probe() }
    }

    private func probe() async {
        probed = false
        let fp = await ConsoleClient(host: candidate.address, port: candidate.port, username: "", password: "", fingerprint: nil).probeFingerprint()
        // Une autre voie choisie entre-temps : le parcours a changé.
        guard !Task.isCancelled else { return }
        seen = fp
        probed = true
        // Le lien ou l'annonce donnait l'empreinte, et c'est la même : rien à comparer.
        if let seen, candidate.expected != nil, !mismatch {
            UIImpactFeedbackGenerator(style: .light).impactOccurred()
            flow.trusted(candidate, fingerprint: seen, verified: true)
        }
    }

    @ViewBuilder private func manual(_ fp: String) -> some View {
        let groups = AddHostFingerprint.groups(fp)
        AddHostParagraph(String(localized: "Comparez ces quatre groupes avec ceux de KyberNow Admin (Appareils Link › Ajouter un appareil). Ils sont l'empreinte du certificat de l'ordinateur."))
        HStack(spacing: wide ? 12 : 8) {
            ForEach(Array(groups.prefix(4).enumerated()), id: \.offset) { _, g in
                Text(verbatim: g)
                    .font(Theme.mono(wide ? 28 : 19, .medium))
                    .foregroundStyle(Theme.text)
                    .lineLimit(1).minimumScaleFactor(0.6)
                    .frame(maxWidth: wide ? 130 : .infinity)
                    .frame(height: wide ? 84 : 64)
                    .background(RoundedRectangle(cornerRadius: wide ? 14 : 12, style: .continuous).fill(Theme.surface))
                    .overlay(RoundedRectangle(cornerRadius: wide ? 14 : 12, style: .continuous).strokeBorder(Theme.line2))
            }
        }
        .padding(.top, 6)
        .accessibilityElement(children: .combine)
        Text(verbatim: "sha256 · " + groups.prefix(6).joined(separator: " ") + " …")
            .font(Theme.mono(wide ? 12 : 11)).foregroundStyle(Theme.ghost)
            .frame(maxWidth: wide ? nil : .infinity, alignment: wide ? .leading : .center)
            .lineLimit(1).minimumScaleFactor(0.7)
        HStack(spacing: 12) {
            Icon("lock-simple", size: 20).foregroundStyle(Theme.accent)
            Text("Avec le QR code ou une annonce sur le réseau, cette étape se fait toute seule.")
                .font(Theme.sans(13)).foregroundStyle(Theme.text2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.horizontal, 14)
        .frame(maxWidth: wide ? 600 : .infinity, minHeight: 60, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color.white.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
        actions {
            Button("C'est la même empreinte") { flow.trusted(candidate, fingerprint: fp, verified: false) }.addHostPrimary()
            Button("Ce n'est pas la même") { _ = flow.back() }.buttonStyle(AddHostDangerButton()).frame(maxWidth: wide ? 240 : .infinity)
        }
    }

    /// Boutons : empilés sur iPhone, côte à côte sur iPad.
    @ViewBuilder private func actions<C: View>(@ViewBuilder _ content: () -> C) -> some View {
        if wide {
            HStack(spacing: 14) { content() }.padding(.top, 12)
        } else {
            VStack(spacing: 10) { content() }.padding(.top, 12)
        }
    }
}

// MARK: - Le compte

struct AddHostAccountPane: View {
    let candidate: Candidate
    let fingerprint: String?
    let verified: Bool
    @EnvironmentObject private var app: AppModel
    @Environment(\.addHostWide) private var wide
    @State private var username = ""
    @State private var password = ""
    @State private var checking = false
    @State private var error: String?

    init(candidate: Candidate, fingerprint: String?, verified: Bool) {
        self.candidate = candidate
        self.fingerprint = fingerprint
        self.verified = verified
    }

    /// Par adresse, le nom de l'ordinateur n'est connu qu'après coup (Admin le donne).
    private var named: Bool { candidate.name != candidate.address }

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 20 : 14) {
            AddHostHeader(eyebrow: String(localized: "Dernière étape"), title: named ? String(localized: "Le compte de \(candidate.name)") : String(localized: "Le compte de l'ordinateur"))
            AddHostParagraph(DeviceDisplay.isPad
                ? String(localized: "Celui choisi à l'installation de KyberNow Station sur cet ordinateur. Il est gardé dans le Trousseau de cet iPad.")
                : String(localized: "Celui choisi à l'installation de KyberNow Station sur cet ordinateur. Il est gardé dans le Trousseau de cet iPhone."))
            if verified {
                HStack(spacing: 10) {
                    Icon("check", size: 18, bold: true).foregroundStyle(Theme.ok)
                    Text("Empreinte vérifiée : c'est bien l'ordinateur annoncé.")
                        .font(Theme.sans(13.5)).foregroundStyle(Theme.text2)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            VStack(alignment: .leading, spacing: 14) {
                AddHostField(label: String(localized: "Identifiant"), text: $username, content: .username)
                AddHostField(label: String(localized: "Mot de passe"), text: $password, secure: true, content: .password, submit: .go, onSubmit: add)
            }
            .frame(maxWidth: wide ? 480 : .infinity)
            .padding(.top, 4)
            if let error {
                AddHostNotice(kind: .warn, text: error)
            }
            VStack(alignment: wide ? .leading : .center, spacing: 10) {
                HStack(spacing: 16) {
                    Button(action: add) {
                        if checking {
                            Text("Vérification…")
                        } else if named {
                            Text("Ajouter \(candidate.name)").lineLimit(1)
                        } else {
                            Text("Ajouter l'ordinateur")
                        }
                    }
                    .addHostPrimary()
                    .disabled(username.isEmpty || password.isEmpty || checking)
                    if wide { caption }
                }
                if !wide { caption }
            }
            .frame(maxWidth: .infinity, alignment: wide ? .leading : .center)
            .padding(.top, 8)
        }
    }

    private var caption: some View {
        Text("Vérifié auprès de la Station avant d'être gardé.")
            .font(Theme.sans(wide ? 14 : 12.5)).foregroundStyle(Theme.faint)
    }

    /// Vérifie le compte auprès d'Admin (certificat épinglé), puis enregistre.
    private func add() {
        guard !username.isEmpty, !password.isEmpty, !checking else { return }
        checking = true
        error = nil
        Task {
            defer { checking = false }
            let console = ConsoleClient(host: candidate.address, port: candidate.port, username: username, password: password, fingerprint: fingerprint)
            do {
                let info = try await console.info()
                save(name: info.hostname ?? candidate.name, fingerprint: console.fingerprint ?? fingerprint)
            } catch ConsoleError.http(401, _) {
                error = String(localized: "Identifiant ou mot de passe refusé.")
                UINotificationFeedbackGenerator().notificationOccurred(.error)
            } catch ConsoleError.http(404, _) {
                // Admin d'une version qui ne connaît pas cet appel : le compte
                // n'a pas pu être vérifié, il est gardé tel quel.
                save(name: candidate.name, fingerprint: fingerprint)
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func save(name: String, fingerprint: String?) {
        var host = Host(name: name, address: candidate.address, consolePort: candidate.port, username: username)
        host.consoleFingerprint = fingerprint
        app.store.add(host, password: password)
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        app.selected = host.id
        app.sheet = nil
        app.show(String(localized: "\(name) est ajouté"))
    }
}
