// Ajouter un ordinateur (maquette « 2 · Ajouter un ordinateur ») : sans rien
// taper à la télécommande, de préférence. Depuis le Mac (iCloud), trouvé sur
// le réseau (compte à saisir, au clavier de l'iPhone), code à accepter dans
// KyberNow Admin sur l'ordinateur, ou adresse (VPN).
//
// Chaque fois, l'empreinte du certificat d'Admin est retenue (et comparée à
// celle de l'annonce Bonjour quand il y en a une).

import Combine
import CoreImage.CIFilterBuiltins
import KyberKit
import Network
import SwiftUI

struct AddHostView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @ObservedObject private var prefs = Preferences.shared
    #if DEBUG
    /// Vérification sans télécommande : `-addMethod 0…3`.
    @State private var method = UserDefaults.standard.integer(forKey: "addMethod")
    #else
    @State private var method = 0
    #endif
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, method(Int) }

    private let methods: [(icon: String, title: String, sub: String, badge: String?)] = [
        ("cloud", String(localized: "Depuis votre Mac"), String(localized: "Synchronisés par iCloud, avec leurs accès"), String(localized: "Conseillé")),
        ("wifi-high", String(localized: "Sur le réseau"), String(localized: "Compte à saisir, au clavier de l'iPhone"), nil),
        ("number-square-one", String(localized: "Code à l'écran"), String(localized: "À accepter dans KyberNow Admin"), nil),
        ("globe-hemisphere-west", String(localized: "Adresse"), String(localized: "VPN ou réseau sans découverte"), nil),
    ]

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            Button { app.screen = .home } label: {
                HStack(spacing: 10) {
                    Icon("arrow-left", size: 26, bold: true)
                    Text("Retour").font(Theme.sans(22, .medium))
                }
                .padding(.leading, 18).padding(.trailing, 26)
                .frame(height: 60)
            }
            .buttonStyle(GlassButton(radius: 30))
            .focused($focus, equals: .back)
            .padding(.leading, 96).padding(.top, 56)

            VStack(alignment: .leading, spacing: 18) {
                Text("Ajouter un ordinateur").font(Theme.display(64, 660)).tracking(-64 * 0.035).padding(.bottom, 4)
                Text("Sans rien taper à la télécommande, de préférence.")
                    .font(Theme.sans(23)).foregroundStyle(Theme.muted).padding(.bottom, 22)
                ForEach(Array(methods.enumerated()), id: \.offset) { i, m in
                    methodButton(i, m)
                }
            }
            .focusSection()
            .frame(width: 640, alignment: .leading)
            .padding(.leading, 96).padding(.top, 180)

            Group {
                switch method {
                case 0: CloudPane()
                case 1: NetworkPane()
                case 2: CodePane()
                default: AddressPane()
                }
            }
            .id(method)
            .transition(.opacity.combined(with: .offset(x: 28)))
            .padding(.horizontal, 60).padding(.vertical, 56)
            .frame(width: 1920 - 840 - 96, height: 1080 - 180 - 96, alignment: .topLeading)
            .glass(30)
            .focusSection()
            .padding(.leading, 840).padding(.top, 180)
            .animation(Theme.easeOut, value: method)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        // Le volet change au clic seulement : après le clavier (écran ou
        // iPhone), tvOS rend le focus à la colonne, et le formulaire doit rester.
        .defaultFocus($focus, .method(0))
        .onExitCommand { app.screen = .home }
    }

    private func methodButton(_ i: Int, _ m: (icon: String, title: String, sub: String, badge: String?)) -> some View {
        let on = method == i
        return Button { method = i } label: {
            HStack(spacing: 22) {
                Icon(m.icon, size: 30)
                    .foregroundStyle(on ? .white : Theme.muted)
                    .frame(width: 64, height: 64)
                    .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(on ? Theme.accentFill : Color.white.opacity(0.06)))
                VStack(alignment: .leading, spacing: 6) {
                    Text(m.title).font(Theme.sans(27, .semibold))
                    Text(m.sub).font(Theme.sans(19)).foregroundStyle(Theme.muted)
                }
                Spacer(minLength: 0)
                if let badge = m.badge {
                    Text(badge.uppercased())
                        .font(Theme.mono(14, .medium)).tracking(0.8)
                        .foregroundStyle(Theme.accentLight)
                        .padding(.horizontal, 12).padding(.vertical, 6)
                        .background(RoundedRectangle(cornerRadius: 8).fill(Theme.accentFill.opacity(0.2)))
                }
            }
            .padding(.horizontal, 28)
            .frame(width: 640, height: 112)
        }
        .buttonStyle(GlassButton(radius: 22, selected: on, scale: 1.04, fill: on ? Color.white.opacity(0.1) : Color.clear))
        .focused($focus, equals: .method(i))
    }
}

// MARK: - Éléments communs

private func paneTitle(_ eyebrow: String, _ title: String) -> some View {
    VStack(alignment: .leading, spacing: 28) {
        Theme.eyebrow(eyebrow)
        Text(title).font(Theme.display(48, 640)).tracking(-48 * 0.03).foregroundStyle(Theme.text)
    }
}

private struct WaitingDots: View {
    @State private var on = false

    var body: some View {
        HStack(spacing: -2) {
            ForEach(0..<3) { i in
                Circle().fill(Theme.accent).frame(width: 8, height: 8)
                    .opacity(on ? 1 : 0.25)
                    .animation(.easeInOut(duration: 0.7).repeatForever().delay(Double(i) * 0.2), value: on)
            }
        }
        .onAppear { on = true }
    }
}

// MARK: - Depuis votre Mac (iCloud)

private struct CloudPane: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @ObservedObject private var prefs = Preferences.shared
    @EnvironmentObject private var cloud: CloudSync

    var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            paneTitle(String(localized: "Trousseau iCloud"), String(localized: "Vos ordinateurs suivent votre Mac"))
            Text("Ceux que vous avez ajoutés dans KyberNow Link sur votre Mac arrivent ici avec leurs accès. Rien à taper, et un mot de passe changé sur le Mac suit tout seul.")
                .font(Theme.sans(23)).foregroundStyle(Theme.text2).lineSpacing(8)
                .frame(maxWidth: 860, alignment: .leading)
            VStack(alignment: .leading, spacing: 14) {
                if cloud.hosts.isEmpty {
                    Text("Aucun ordinateur publié par votre Mac pour l'instant. Ouvrez KyberNow Link sur le Mac, avec le même compte iCloud.")
                        .font(Theme.sans(21)).foregroundStyle(Theme.muted)
                        .padding(.horizontal, 28)
                        .frame(height: 84, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
                }
                ForEach(cloud.hosts) { c in
                    if store.host(c.id) != nil {
                        HStack(spacing: 20) {
                            Icon("check", size: 28, bold: true).foregroundStyle(Theme.ok)
                            Text(c.name).font(Theme.sans(25, .semibold))
                            Spacer()
                            Text("Déjà sur cette Apple TV").font(Theme.sans(19)).foregroundStyle(Theme.muted)
                        }
                        .padding(.horizontal, 28)
                        .frame(height: 84)
                        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
                        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
                    } else {
                        Button { _ = app.addFromCloud(c) } label: {
                            HStack(spacing: 20) {
                                Icon("plus", size: 28, bold: true).foregroundStyle(Theme.accent)
                                Text(c.name).font(Theme.sans(25, .semibold))
                                Spacer()
                                Text("Ajouter").font(Theme.sans(19, .semibold)).foregroundStyle(Theme.accentLight)
                            }
                            .padding(.horizontal, 28)
                            .frame(height: 84)
                        }
                        .buttonStyle(GlassButton(radius: 18, scale: 1.04, fill: Theme.accentFill.opacity(0.14)))
                    }
                }
            }
            Spacer(minLength: 0)
            Button { prefs.icloudSync.toggle() } label: {
                HStack(spacing: 16) {
                    Toggle2(on: prefs.icloudSync)
                    Text("Synchroniser avec iCloud · chaque membre du foyer voit ses propres ordinateurs")
                        .font(Theme.sans(20)).foregroundStyle(Theme.muted)
                }
                .padding(10)
            }
            .buttonStyle(GlassButton(radius: 16, scale: 1.03, fill: Color.clear))
        }
    }
}

// MARK: - Sur le réseau

private struct NetworkPane: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var discovery: Discovery
    @State private var candidate: Candidate?
    @State private var resolving: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            if let candidate {
                LoginForm(candidate: candidate) { self.candidate = nil }
            } else {
                HStack(spacing: 14) {
                    Theme.eyebrow(String(localized: "Recherche sur le réseau local"))
                    WaitingDots()
                }
                Text("Trouvé autour de vous").font(Theme.display(48, 640)).tracking(-48 * 0.03)
                let list = app.newOnNetwork
                if list.isEmpty {
                    Text("Rien de nouveau pour l'instant.").font(Theme.sans(23)).foregroundStyle(Theme.muted)
                }
                ForEach(list) { found in
                    Button { pick(found) } label: {
                        HStack(spacing: 24) {
                            Icon("monitor", size: 36).foregroundStyle(Theme.accent)
                                .frame(width: 72, height: 72)
                                .background(RoundedRectangle(cornerRadius: 16).fill(Theme.surface))
                            VStack(alignment: .leading, spacing: 8) {
                                Text(found.name).font(Theme.display(34, 640)).tracking(-0.7)
                                Text(resolving == found.id ? "Recherche de son adresse…" : "Annoncé par KyberNow Station")
                                    .font(Theme.mono(17)).foregroundStyle(Theme.muted)
                            }
                            Spacer()
                            Text("Ajouter").font(Theme.sans(22, .semibold)).foregroundStyle(Theme.accentLight)
                        }
                        .padding(.horizontal, 32)
                        .frame(height: 120)
                        .overlay(RoundedRectangle(cornerRadius: 22, style: .continuous).strokeBorder(Theme.accent.opacity(0.6), lineWidth: 2))
                    }
                    .buttonStyle(GlassButton(radius: 22, scale: 1.04, fill: Theme.accentFill.opacity(0.14)))
                }
                Text("Un ordinateur n'apparaît pas ? Il doit être allumé, sur le même réseau, avec KyberNow Station en marche. Par VPN, passez par l'adresse.")
                    .font(Theme.sans(21)).foregroundStyle(Theme.muted).lineSpacing(7)
                    .frame(maxWidth: 860, alignment: .leading)
                Spacer(minLength: 0)
            }
        }
    }

    private func pick(_ found: Discovery.Found) {
        resolving = found.id
        Task {
            let c = await Discovery.resolve(found)
            resolving = nil
            candidate = c
        }
    }
}

/// Le compte de l'ordinateur : tapé à l'écran ou au clavier de l'iPhone (tvOS
/// propose la saisie sur l'iPhone à proximité, et le Trousseau).
private struct LoginForm: View {
    let candidate: Candidate
    let cancel: () -> Void
    @EnvironmentObject private var app: AppModel
    @State private var username = ""
    @State private var password = ""
    @State private var checking = false
    @State private var error: String?
    @State private var seen: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 26) {
            (French.elides(candidate.name) ? Text("Le compte d'\(candidate.name)") : Text("Le compte de \(candidate.name)"))
                .font(Theme.display(48, 640)).tracking(-48 * 0.03)
            Text("Celui choisi à l'installation de KyberNow Station sur cet ordinateur.")
                .font(Theme.sans(22)).foregroundStyle(Theme.text2)
            field(String(localized: "Identifiant")) {
                TextField("Identifiant", text: $username)
                    .textContentType(.username)
                    .autocorrectionDisabled()
                    .textInputAutocapitalization(.never)
            }
            field(String(localized: "Mot de passe")) {
                SecureField("Mot de passe", text: $password)
                    .textContentType(.password)
            }
            HStack(spacing: 18) {
                Icon("device-mobile", size: 34).foregroundStyle(Theme.accent)
                Text("Une notification s'affiche sur votre iPhone : tapez avec son clavier, ou remplissez d'un geste avec le Trousseau.")
                    .font(Theme.sans(20)).foregroundStyle(Theme.text2).lineSpacing(6)
            }
            .padding(.horizontal, 24).padding(.vertical, 18)
            .frame(width: 760, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 16).fill(Color.white.opacity(0.04)))
            if let error {
                Text(error).font(Theme.sans(21)).foregroundStyle(Theme.dangerText)
            } else if let seen, candidate.expected == nil {
                Text("Empreinte de la Station : \(LinkFormat.fingerprint(seen)). Comparez-la à celle de KyberNow Admin, Appareils Link › « Ajouter un appareil ».")
                    .font(Theme.mono(16)).foregroundStyle(Theme.muted)
            }
            HStack(spacing: 20) {
                Button(action: add) { checking ? Text("Vérification…") : Text("Ajouter \(candidate.name)") }
                    .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                    .disabled(username.isEmpty || password.isEmpty || checking)
                Button("Annuler", action: cancel)
                    .buttonStyle(ChoiceButton(selected: false, height: 72, light: true))
            }
        }
        .task {
            seen = await ConsoleClient(host: candidate.address, port: candidate.port, username: "", password: "", fingerprint: nil).probeFingerprint()
        }
    }

    private func field<F: View>(_ label: String, @ViewBuilder _ content: () -> F) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(label).font(Theme.sans(19)).foregroundStyle(Theme.muted)
            content()
                .font(Theme.sans(27))
                .frame(width: 760)
        }
    }

    private func add() {
        checking = true
        error = nil
        Task {
            defer { checking = false }
            // L'empreinte annoncée (Bonjour, lien) doit être celle vue.
            if let expected = candidate.expected, let seen, expected.lowercased() != seen.lowercased() {
                error = String(localized: "Le certificat de la Station ne correspond pas à l'annonce : ce n'est peut-être pas votre ordinateur.")
                return
            }
            let console = ConsoleClient(host: candidate.address, port: candidate.port, username: username, password: password, fingerprint: seen)
            do {
                let info = try await console.info()
                var host = Host(name: info.hostname ?? candidate.name, address: candidate.address, consolePort: candidate.port, username: username)
                host.consoleFingerprint = console.fingerprint ?? seen
                app.store.add(host, password: password)
                app.selected = host.id
                app.screen = .home
            } catch ConsoleError.http(401, _) {
                error = String(localized: "Identifiant ou mot de passe refusé.")
            } catch {
                self.error = error.localizedDescription
            }
        }
    }
}

// MARK: - Code à l'écran

/// L'Apple TV tire un code (valable cinq minutes) et le présente à toutes
/// les Stations trouvées par Bonjour, jamais par VPN ; la première qui
/// l'accepte (Admin, Appareils Link › Accepter) lui donne son accès.
private struct CodePane: View {
    @EnvironmentObject private var app: AppModel
    @State private var code = String(format: "%06d", Int.random(in: 0...999_999))
    @State private var expires = Date().addingTimeInterval(300)
    @State private var now = Date()
    @State private var unsupported = false
    @State private var refused: String?
    @State private var consoleURL: String?
    @EnvironmentObject private var discovery: Discovery
    private let tick = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        HStack(alignment: .top, spacing: 60) {
            VStack(alignment: .leading, spacing: 26) {
                paneTitle(String(localized: "Appairage par code"), String(localized: "Acceptez ce code sur l'ordinateur"))
                // Toujours entier : la chasse fixe de Martian Mono est large.
                Text(verbatim: "\(code.prefix(3)) \(code.suffix(3))")
                    .font(Theme.mono(112, .medium))
                    .tracking(112 * 0.04)
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
                    .fixedSize(horizontal: false, vertical: true)
                    .foregroundStyle(Theme.neonCore)
                    .shadow(color: Theme.neonCore, radius: 1)
                    .shadow(color: Theme.accent, radius: 14)
                    .shadow(color: Theme.accentFill, radius: 40)
                    .padding(.vertical, 10)
                    .accessibilityLabel("Code \(code)")
                VStack(alignment: .leading, spacing: 14) {
                    step(1, String(localized: "Sur l'ordinateur, ouvrez KyberNow Admin (icône KyberNow Station près de l'horloge)."))
                    step(2, String(localized: "Appareils Link › « \(deviceName) » demande l'accès › Accepter."))
                    step(3, String(localized: "C'est tout : l'Apple TV reçoit son propre accès, révocable à tout moment."))
                }
                Spacer(minLength: 0)
                HStack(spacing: 14) {
                    if unsupported {
                        Icon("warning", size: 24).foregroundStyle(Theme.warn)
                        Text("Les ordinateurs trouvés n'ont pas encore la version de KyberNow Station qui accepte les codes : mettez-la à jour sur l'ordinateur.")
                            .font(Theme.sans(20)).foregroundStyle(Theme.warnText)
                            .fixedSize(horizontal: false, vertical: true)
                    } else if let refused {
                        Icon("warning", size: 24).foregroundStyle(Theme.warn)
                        Text(refused).font(Theme.sans(20)).foregroundStyle(Theme.warnText)
                            .fixedSize(horizontal: false, vertical: true)
                    } else {
                        WaitingDots()
                        Text("En attente d'un ordinateur…").font(Theme.sans(21)).foregroundStyle(Theme.muted)
                            .fixedSize()
                        Text("valable \(remaining)").font(Theme.mono(18)).foregroundStyle(Theme.faint)
                            .fixedSize()
                    }
                }
            }
            VStack(spacing: 18) {
                if let qr = qrImage {
                    Image(uiImage: qr)
                        .interpolation(.none)
                        .resizable()
                        .frame(width: 216, height: 216)
                        .padding(22)
                        .background(RoundedRectangle(cornerRadius: 22).fill(Theme.text))
                }
                Text("Ou ouvrez KyberNow Admin depuis votre téléphone")
                    .font(Theme.sans(19)).foregroundStyle(Theme.muted)
                    .multilineTextAlignment(.center)
            }
            .frame(width: 300)
            .frame(maxHeight: .infinity)
        }
        .onReceive(tick) { now = $0; if now > expires { renew() } }
        .task(id: code) { await pairLoop() }
    }

    private var deviceName: String { UIDevice.current.name }

    private var remaining: String {
        let s = max(0, Int(expires.timeIntervalSince(now)))
        return "\(s / 60) min \(String(format: "%02d", s % 60))"
    }

    private func step(_ n: Int, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Text(verbatim: "\(n).").font(Theme.sans(23, .semibold)).foregroundStyle(Theme.accent)
            Text(text).font(Theme.sans(23)).foregroundStyle(Theme.text2).lineSpacing(6)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func renew() {
        code = String(format: "%06d", Int.random(in: 0...999_999))
        expires = Date().addingTimeInterval(300)
        refused = nil
    }

    /// QR code d'Admin sur la première Station trouvée (sa page, sur le téléphone).
    private var qrImage: UIImage? {
        guard let consoleURL else { return nil }
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(consoleURL.utf8)
        filter.correctionLevel = "M"
        guard let out = filter.outputImage, let cg = CIContext().createCGImage(out, from: out.extent) else { return nil }
        return UIImage(cgImage: cg)
    }

    /// Demande envoyée à chaque Station trouvée, puis attente de la réponse.
    private func pairLoop() async {
        var requests: [(Candidate, ConsoleClient, String)] = []
        var anySupported = false
        for found in discovery.found {
            guard let c = await Discovery.resolve(found) else { continue }
            if consoleURL == nil { consoleURL = "https://\(c.address):\(c.port)/" }
            let console = ConsoleClient(host: c.address, port: c.port, username: "", password: "", fingerprint: nil)
            do {
                let id = try await console.pairRequest(code: code)
                requests.append((c, console, id))
                anySupported = true
            } catch ConsoleError.http(404, _) {
                continue
            } catch {
                continue
            }
        }
        unsupported = !anySupported && !discovery.found.isEmpty
        while !Task.isCancelled, !requests.isEmpty {
            try? await Task.sleep(for: .seconds(1.5))
            for (c, console, id) in requests {
                guard let state = try? await console.pairStatus(id) else { continue }
                switch state {
                case let .accepted(username, password):
                    if let expected = c.expected, let seen = console.fingerprint, expected.lowercased() != seen.lowercased() { continue }
                    let check = ConsoleClient(host: c.address, port: c.port, username: username, password: password, fingerprint: console.fingerprint)
                    let info = try? await check.info()
                    var host = Host(name: info?.hostname ?? c.name, address: c.address, consolePort: c.port, username: username)
                    host.consoleFingerprint = console.fingerprint
                    // Compte créé par Admin pour cette Apple TV, à l'accord.
                    host.pairedByCode = true
                    app.store.add(host, password: password)
                    app.selected = host.id
                    app.screen = .home
                    return
                case .refused:
                    refused = String(localized: "\(c.name) a refusé l'accès.")
                    requests.removeAll { $0.2 == id }
                case .expired:
                    renew()
                    return
                case .pending:
                    break
                }
            }
        }
    }
}

// MARK: - Adresse

private struct AddressPane: View {
    @State private var address = ""
    @State private var port = ""
    @State private var candidate: Candidate?

    var body: some View {
        VStack(alignment: .leading, spacing: 26) {
            if let candidate {
                LoginForm(candidate: candidate) { self.candidate = nil }
            } else {
                paneTitle(String(localized: "VPN, autre réseau"), String(localized: "Son adresse et son port"))
                Text("Affichés dans KyberNow Admin, Appareils Link › « Ajouter un appareil ». Le port a été tiré au hasard à l'installation.")
                    .font(Theme.sans(22)).foregroundStyle(Theme.text2).lineSpacing(7)
                    .frame(maxWidth: 860, alignment: .leading)
                HStack(spacing: 20) {
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Adresse ou nom").font(Theme.sans(19)).foregroundStyle(Theme.muted)
                        TextField(String("10.8.0.12"), text: $address)
                            .font(Theme.mono(26))
                            .keyboardType(.URL)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                            .frame(width: 560)
                    }
                    VStack(alignment: .leading, spacing: 10) {
                        Text("Port d'Admin").font(Theme.sans(19)).foregroundStyle(Theme.muted)
                        TextField(String("31877"), text: $port)
                            .font(Theme.mono(26))
                            .keyboardType(.numberPad)
                            .frame(width: 220)
                    }
                }
                Spacer(minLength: 0)
                HStack(spacing: 20) {
                    Button("Continuer") {
                        let host = address.trimmingCharacters(in: .whitespaces)
                        if let p = UInt16(port), !host.isEmpty {
                            candidate = Candidate(name: host, address: host, port: p, expected: nil)
                        }
                    }
                    .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                    .disabled(address.isEmpty || UInt16(port) == nil)
                    Text("Ensuite : le compte de l'ordinateur.").font(Theme.sans(20)).foregroundStyle(Theme.muted)
                }
            }
        }
    }
}
