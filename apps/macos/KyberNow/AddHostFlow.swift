// Ajouter un ordinateur (écrans « 2 · Ajouter un ordinateur » et
// « 3 · Première connexion » de la maquette) : le trouver (réseau local,
// adresse, QR code ou lien), vérifier que c'est bien lui (empreinte du
// certificat de sa console), puis enregistrer le compte.

import AVFoundation
import KyberKit
import Network
import SwiftUI
import Vision

/// Un ordinateur trouvé, pas encore ajouté.
struct Candidate: Equatable {
    var name: String
    var address: String
    var port: UInt16
    /// Empreinte annoncée (lien, QR code, Bonjour) : à comparer à celle vue.
    var expected: String?
}

/// Lien `kybernow://adresse:port?name=…&fp=sha256:…` (QR code de la console).
enum PairingLink {
    static func parse(_ text: String) -> Candidate? {
        guard let url = URL(string: text.trimmingCharacters(in: .whitespacesAndNewlines)),
              url.scheme == "kybernow", let host = url.host(percentEncoded: false), let port = url.port,
              let port16 = UInt16(exactly: port)
        else { return nil }
        let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let name = items.first { $0.name == "name" }?.value ?? host
        let fp = items.first { $0.name == "fp" }?.value
        return Candidate(name: name, address: host, port: port16, expected: fp)
    }
}

// MARK: - Découverte (Bonjour)

@MainActor
final class Discovery: ObservableObject {
    struct Found: Identifiable, Equatable {
        let id: String
        let name: String
        let fingerprint: String?
        let endpoint: NWEndpoint
    }

    @Published private(set) var found: [Found] = []
    private var browser: NWBrowser?

    func start() {
        guard browser == nil else { return }
        let browser = NWBrowser(for: .bonjourWithTXTRecord(type: "_kybernow._tcp", domain: nil), using: .tcp)
        browser.browseResultsChangedHandler = { [weak self] results, _ in
            let list = results.compactMap { r -> Found? in
                guard case let .service(name, _, _, _) = r.endpoint else { return nil }
                var txt: [String: String] = [:]
                if case let .bonjour(record) = r.metadata { txt = record.dictionary }
                return Found(id: name, name: txt["name"] ?? name, fingerprint: txt["fp"], endpoint: r.endpoint)
            }
            Task { @MainActor in self?.found = list.sorted { $0.name < $1.name } }
        }
        browser.start(queue: .main)
        self.browser = browser
    }

    func stop() {
        browser?.cancel()
        browser = nil
    }

    /// Adresse IPv4 et port d'un service annoncé.
    static func resolve(_ found: Found) async -> Candidate? {
        let params = NWParameters.tcp
        (params.defaultProtocolStack.internetProtocol as? NWProtocolIP.Options)?.version = .v4
        let connection = NWConnection(to: found.endpoint, using: params)
        let once = Once()
        return await withCheckedContinuation { continuation in
            connection.stateUpdateHandler = { state in
                switch state {
                case .ready:
                    guard once.claim() else { return }
                    var result: Candidate?
                    if case let .hostPort(host, port) = connection.currentPath?.remoteEndpoint {
                        var address = "\(host)"
                        if let percent = address.firstIndex(of: "%") { address = String(address[..<percent]) }
                        result = Candidate(name: found.name, address: address, port: port.rawValue, expected: found.fingerprint)
                    }
                    connection.cancel()
                    continuation.resume(returning: result)
                case .failed, .cancelled:
                    guard once.claim() else { return }
                    continuation.resume(returning: nil)
                default:
                    break
                }
            }
            connection.start(queue: .global())
            // Sans réponse en 5 s : l'annulation passe par .cancelled.
            DispatchQueue.global().asyncAfter(deadline: .now() + 5) { connection.cancel() }
        }
    }
}

/// Vrai une seule fois, quel que soit le fil.
private final class Once: @unchecked Sendable {
    private let lock = NSLock()
    private var done = false

    func claim() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        if done { return false }
        done = true
        return true
    }
}

// MARK: - QR code (caméra du Mac)

final class QRScanner: NSObject, ObservableObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    let session = AVCaptureSession()
    @Published var denied = false
    @Published var noCamera = false
    var onCode: ((String) -> Void)?
    private let queue = DispatchQueue(label: "kybernow.qr")
    private var lastScan = Date.distantPast

    func start() {
        AVCaptureDevice.requestAccess(for: .video) { [weak self] ok in
            DispatchQueue.main.async {
                guard let self else { return }
                if !ok { self.denied = true; return }
                self.configure()
            }
        }
    }

    private func configure() {
        guard session.inputs.isEmpty else { queue.async { self.session.startRunning() }; return }
        guard let device = AVCaptureDevice.default(for: .video), let input = try? AVCaptureDeviceInput(device: device) else {
            noCamera = true
            return
        }
        session.beginConfiguration()
        session.sessionPreset = .high
        if session.canAddInput(input) { session.addInput(input) }
        let output = AVCaptureVideoDataOutput()
        output.setSampleBufferDelegate(self, queue: queue)
        if session.canAddOutput(output) { session.addOutput(output) }
        session.commitConfiguration()
        queue.async { self.session.startRunning() }
    }

    func stop() {
        queue.async { self.session.stopRunning() }
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard Date().timeIntervalSince(lastScan) > 0.25, let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        lastScan = Date()
        let request = VNDetectBarcodesRequest()
        request.symbologies = [.qr]
        try? VNImageRequestHandler(cvPixelBuffer: pixels).perform([request])
        if let code = request.results?.compactMap(\.payloadStringValue).first(where: { $0.hasPrefix("kybernow://") }) {
            DispatchQueue.main.async { self.onCode?(code) }
        }
    }
}

private struct CameraPreview: NSViewRepresentable {
    let session: AVCaptureSession

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        view.wantsLayer = true
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        view.layer = layer
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}
}

// MARK: - Parcours

struct AddHostSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    /// Lien ouvert depuis l'extérieur (kybernow://…), s'il y en a un.
    var initialLink: Candidate?

    enum Tab: Hashable { case network, address, link }
    enum Step { case find, trust, account }

    @State private var tab: Tab? = .network
    @State private var step: Step = .find
    @State private var candidate: Candidate?

    var body: some View {
        Group {
            switch step {
            case .find:
                FindStep(tab: $tab) { found in
                    candidate = found
                    withAnimation(.spring(response: 0.4, dampingFraction: 0.85)) { step = .trust }
                }
            case .trust, .account:
                if let candidate {
                    VerifyAndAccount(candidate: candidate, step: $step) { dismiss() }
                }
            }
        }
        .background(Theme.surface)
        .preferredColorScheme(.dark)
        .onAppear {
            if let initialLink {
                candidate = initialLink
                step = .trust
            }
        }
    }
}

// MARK: Trouver

private struct FindStep: View {
    @Binding var tab: AddHostSheet.Tab?
    let found: (Candidate) -> Void
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Ajouter un ordinateur").font(Theme.display(30)).foregroundStyle(Theme.text)
                    Text("Trois façons de faire, choisissez la plus simple pour vous.").font(Theme.sans(14)).foregroundStyle(Theme.muted)
                }
                Spacer()
                Button { dismiss() } label: { Image(systemName: "xmark") }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
            }
            Segmented(options: [(.network, "Sur ce réseau"), (.address, "Par adresse"), (.link, "QR code ou lien")], selection: $tab, height: 40, fill: true)
            Group {
                switch tab ?? .address {
                case .network: NetworkTab(found: found)
                case .address: AddressTab(found: found)
                case .link: LinkTab(found: found)
                }
            }
            .frame(minHeight: 320, alignment: .top)
        }
        .padding(.horizontal, 36).padding(.vertical, 32)
        .frame(width: 760)
    }
}

private struct NetworkTab: View {
    let found: (Candidate) -> Void
    @EnvironmentObject var app: AppModel
    @StateObject private var discovery = Discovery()
    @State private var resolving: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("La console KyberNow de chaque ordinateur s'annonce sur le réseau local (Bonjour).")
                    .font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
            ForEach(discovery.found) { f in
                let known = app.store.hosts.contains { $0.consoleFingerprint != nil && $0.consoleFingerprint == f.fingerprint }
                HStack(spacing: 16) {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(f.name).font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text)
                        Text(f.fingerprint == nil ? "annonce sans empreinte" : "empreinte annoncée").font(Theme.mono(12)).foregroundStyle(Theme.faint)
                    }
                    Spacer()
                    if known {
                        Text("Déjà ajouté").font(Theme.sans(13)).foregroundStyle(Theme.accent)
                    } else {
                        Button(resolving == f.id ? "…" : "Ajouter") {
                            resolving = f.id
                            Task {
                                if let c = await Discovery.resolve(f) { found(c) }
                                resolving = nil
                            }
                        }
                        .buttonStyle(PrimaryButton(height: 38))
                        .disabled(resolving != nil)
                    }
                }
                .padding(.horizontal, 18).padding(.vertical, 16)
                .background(known ? Theme.well : Theme.accentSoft.opacity(0.35), in: RoundedRectangle(cornerRadius: 14))
                .overlay(RoundedRectangle(cornerRadius: 14).stroke(known ? Theme.line : Theme.accentLine))
            }
            if discovery.found.isEmpty {
                Text("Aucun ordinateur trouvé pour l'instant. Il est peut-être sur un autre réseau (VPN), ou sa console n'est pas encore à jour : passez par l'onglet Par adresse.")
                    .font(Theme.sans(13)).foregroundStyle(Theme.faint).padding(.top, 6)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onAppear { discovery.start() }
        .onDisappear { discovery.stop() }
    }
}

private struct AddressTab: View {
    let found: (Candidate) -> Void
    @State private var address = ""
    @State private var port = ""

    private var valid: Bool { !address.trimmingCharacters(in: .whitespaces).isEmpty && UInt16(port.trimmingCharacters(in: .whitespaces)) != nil }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(alignment: .top, spacing: 14) {
                LabeledField(label: "Adresse de l'ordinateur", text: $address, prompt: "ex. 192.168.1.100", mono: true)
                LabeledField(label: "Port de la console", text: $port, prompt: "ex. 37462", mono: true).frame(width: 170)
            }
            Notice(kind: .info, text: "Le port de la console s'affiche à la fin de l'installation sur l'ordinateur, et dans la console elle-même. L'app y lit tout le reste : port du flux, nom et carte graphique. Par VPN (Tailscale, WireGuard…), utilisez l'adresse de l'ordinateur dans le VPN.")
            Spacer(minLength: 0)
            HStack {
                Spacer()
                Button("Continuer") {
                    let a = address.trimmingCharacters(in: .whitespaces)
                    found(Candidate(name: a, address: a, port: UInt16(port.trimmingCharacters(in: .whitespaces))!, expected: nil))
                }
                .buttonStyle(PrimaryButton())
                .disabled(!valid)
                .keyboardShortcut(.defaultAction)
            }
        }
    }
}

private struct LinkTab: View {
    let found: (Candidate) -> Void
    @StateObject private var scanner = QRScanner()
    @State private var link = ""
    @State private var invalid = false

    var body: some View {
        HStack(alignment: .top, spacing: 26) {
            ZStack {
                if scanner.denied || scanner.noCamera {
                    Text(scanner.denied ? "Accès à la caméra refusé (Réglages Système › Confidentialité)." : "Aucune caméra sur ce Mac.")
                        .font(Theme.sans(12)).foregroundStyle(Theme.muted).multilineTextAlignment(.center).padding()
                } else {
                    CameraPreview(session: scanner.session)
                }
                ScanFrame()
            }
            .frame(width: 260, height: 260)
            .background(Theme.rgb(0x05080B))
            .clipShape(RoundedRectangle(cornerRadius: 16))
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(Theme.line))
            VStack(alignment: .leading, spacing: 14) {
                (Text("Dans la console de l'ordinateur, ouvrez ") + Text("Appareils").fontWeight(.semibold).foregroundColor(Theme.text) + Text(", puis présentez le QR code devant la caméra."))
                    .font(Theme.sans(15)).foregroundStyle(Theme.text2)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 12) {
                    Rectangle().fill(Theme.line).frame(height: 1)
                    Text("OU").font(Theme.sans(12)).tracking(1).foregroundStyle(Theme.faint)
                    Rectangle().fill(Theme.line).frame(height: 1)
                }
                LabeledField(label: "Collez le lien copié depuis la console", text: $link, prompt: "kybernow://192.168.1.100:37462?fp=sha256:…", mono: true)
                    .onSubmit(useLink)
                if invalid {
                    Text("Ce n'est pas un lien KyberNow.").font(Theme.sans(12)).foregroundStyle(Theme.warn)
                }
                HStack(alignment: .top, spacing: 10) {
                    Image(systemName: "checkmark.shield").foregroundStyle(Theme.accent)
                    Text("Le lien contient l'adresse, le port et l'empreinte du certificat de l'ordinateur : la connexion est sûre dès la première fois, sans question.")
                        .font(Theme.sans(13)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                HStack {
                    Spacer()
                    Button("Continuer", action: useLink)
                        .buttonStyle(PrimaryButton())
                        .disabled(link.isEmpty)
                }
            }
        }
        .onAppear {
            scanner.onCode = { code in
                if let c = PairingLink.parse(code) {
                    scanner.stop()
                    found(c)
                }
            }
            scanner.start()
        }
        .onDisappear { scanner.stop() }
    }

    private func useLink() {
        if let c = PairingLink.parse(link) { found(c) } else { invalid = true }
    }
}

/// Coins du viseur et ligne de balayage.
private struct ScanFrame: View {
    @State private var sweep = false

    var body: some View {
        GeometryReader { geo in
            let inset: CGFloat = 34
            ZStack {
                ForEach(0..<4) { corner in
                    Path { p in
                        let x = corner % 2 == 0 ? inset : geo.size.width - inset
                        let y = corner < 2 ? inset : geo.size.height - inset
                        let dx: CGFloat = corner % 2 == 0 ? 36 : -36
                        let dy: CGFloat = corner < 2 ? 36 : -36
                        p.move(to: CGPoint(x: x, y: y + dy))
                        p.addLine(to: CGPoint(x: x, y: y))
                        p.addLine(to: CGPoint(x: x + dx, y: y))
                    }
                    .stroke(Theme.accent, style: StrokeStyle(lineWidth: 3, lineCap: .round))
                }
                Rectangle()
                    .fill(Theme.accent)
                    .frame(width: geo.size.width - 80, height: 2)
                    .shadow(color: Theme.accent, radius: 7)
                    .position(x: geo.size.width / 2, y: sweep ? geo.size.height * 0.82 : geo.size.height * 0.14)
            }
        }
        .allowsHitTesting(false)
        .onAppear {
            withAnimation(.easeInOut(duration: 1.6).repeatForever(autoreverses: true)) { sweep = true }
        }
    }
}

struct LabeledField: View {
    let label: String
    @Binding var text: String
    var prompt = ""
    var mono = false
    var secure = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(label).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
            Group {
                if secure {
                    SecureField("", text: $text, prompt: Text(prompt))
                } else {
                    TextField("", text: $text, prompt: Text(prompt).foregroundColor(Theme.rgb(0x6B7789)))
                }
            }
            .textFieldStyle(.plain)
            .font(mono ? Theme.mono(14) : Theme.sans(15))
            .foregroundStyle(Theme.text)
            .padding(.horizontal, 14)
            .frame(height: 46)
            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 11))
            .overlay(RoundedRectangle(cornerRadius: 11).stroke(Theme.line2))
        }
    }
}

// MARK: - Vérifier puis compte

private struct VerifyAndAccount: View {
    @EnvironmentObject var app: AppModel
    let candidate: Candidate
    @Binding var step: AddHostSheet.Step
    let done: () -> Void

    @State private var seen: String?
    @State private var probed = false
    @State private var username = ""
    @State private var password = ""
    @State private var checking = false
    @State private var error: String?

    private var verified: Bool { candidate.expected != nil && seen != nil && candidate.expected == seen }
    private var mismatch: Bool { candidate.expected != nil && seen != nil && candidate.expected != seen }

    var body: some View {
        HStack(spacing: 0) {
            rail.frame(width: 300)
            VStack(alignment: .leading, spacing: 20) {
                if step == .trust { trust } else { account }
            }
            .padding(.horizontal, 40).padding(.vertical, 36)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        }
        .frame(width: 880, height: 600)
        .task {
            let console = ConsoleClient(host: candidate.address, port: candidate.port, username: "", password: "", fingerprint: nil)
            seen = await console.probeFingerprint()
            probed = true
            if verified {
                withAnimation(.spring(response: 0.4, dampingFraction: 0.85)) { step = .account }
            }
        }
    }

    // Colonne de gauche : l'ordinateur et les étapes.
    private var rail: some View {
        VStack(alignment: .leading, spacing: 26) {
            ZStack {
                Circle().fill(Theme.accentSoft)
                Image(systemName: "display").font(.system(size: 26)).foregroundStyle(Theme.accent)
            }
            .frame(width: 64, height: 64)
            VStack(alignment: .leading, spacing: 4) {
                Text(candidate.name).font(Theme.display(22)).foregroundStyle(Theme.text).lineLimit(2)
                Text("\(candidate.address):\(candidate.port)").font(Theme.mono(12)).foregroundStyle(Theme.faint)
            }
            VStack(alignment: .leading, spacing: 18) {
                stepRow(1, "Vérifier l'ordinateur", verified ? "Empreinte vérifiée par le lien" : "Son certificat, une seule fois", done: step == .account, now: step == .trust)
                stepRow(2, "Votre compte", "Gardé dans le Trousseau", done: false, now: step == .account)
            }
            Spacer()
        }
        .padding(.horizontal, 28).padding(.vertical, 32)
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Theme.well)
        .overlay(alignment: .trailing) { Rectangle().fill(Theme.line).frame(width: 1) }
    }

    private func stepRow(_ n: Int, _ title: String, _ sub: String, done: Bool, now: Bool) -> some View {
        HStack(alignment: .top, spacing: 12) {
            ZStack {
                Circle().fill(done ? Theme.accent : now ? Theme.accentSoft : Theme.surface2)
                if now { Circle().stroke(Theme.accent, lineWidth: 1.5) }
                Text(done ? "✓" : "\(n)").font(Theme.sans(13, .semibold)).foregroundStyle(done ? Theme.accentInk : now ? Theme.accentLight : Theme.faint)
            }
            .frame(width: 26, height: 26)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(Theme.sans(15, .semibold)).foregroundStyle(done || now ? Theme.text : Theme.faint)
                Text(sub).font(Theme.sans(13)).foregroundStyle(Theme.faint)
            }
        }
    }

    // Étape 1 : c'est bien votre ordinateur ?
    @ViewBuilder private var trust: some View {
        Text("C'est bien votre ordinateur ?").font(Theme.display(28)).foregroundStyle(Theme.text)
        if !probed {
            HStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text("Connexion à la console…").font(Theme.sans(14)).foregroundStyle(Theme.muted)
            }
        } else if seen == nil {
            Notice(kind: .danger, text: "La console ne répond pas à \(candidate.address):\(candidate.port). Vérifiez l'adresse, le port, et que l'ordinateur est allumé.")
            Spacer()
            HStack { Spacer(); Button("Retour") { step = .find }.buttonStyle(GhostButton(height: 44)) }
        } else if verified {
            HStack(spacing: 10) {
                Image(systemName: "checkmark.seal.fill").font(.system(size: 22)).foregroundStyle(Theme.accent)
                Text("Empreinte vérifiée : c'est bien l'ordinateur annoncé.").font(Theme.sans(15)).foregroundStyle(Theme.text)
            }
            Text("Le lien ou l'annonce de l'ordinateur contenait l'empreinte de son certificat, et elle correspond à celui qu'il présente.")
                .font(Theme.sans(14)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            Spacer()
            actions(primary: "Continuer") { step = .account }
        } else {
            Text("Votre ordinateur prouve son identité avec un certificat que KyberNow a créé pendant l'installation. Comparez son empreinte avec celle qu'affiche la console de l'ordinateur, dans **Appareils**.")
                .font(Theme.sans(15)).foregroundStyle(Theme.text2).fixedSize(horizontal: false, vertical: true)
            FingerprintBox(fingerprint: seen ?? "")
            if mismatch {
                Notice(kind: .danger, text: "Cette empreinte ne correspond pas à celle du lien ou de l'annonce. Quelqu'un se fait peut-être passer pour votre ordinateur : refusez.")
            } else {
                Notice(kind: .warn, text: "Si elle diffère, quelqu'un se fait peut-être passer pour votre ordinateur. Refusez, et vérifiez l'adresse.")
            }
            Spacer()
            actions(primary: "C'est bien mon ordinateur", secondary: "Refuser") { step = .account }
        }
    }

    // Étape 2 : le compte.
    @ViewBuilder private var account: some View {
        Text("Votre compte KyberNow").font(Theme.display(28)).foregroundStyle(Theme.text)
        Text("C'est le compte créé pendant l'installation sur l'ordinateur. D'autres comptes s'ajoutent dans la console, rubrique **Utilisateurs**.")
            .font(Theme.sans(15)).foregroundStyle(Theme.text2).fixedSize(horizontal: false, vertical: true)
        LabeledField(label: "Identifiant", text: $username)
        LabeledField(label: "Mot de passe", text: $password, secure: true)
        HStack(spacing: 12) {
            Image(systemName: "key.fill").foregroundStyle(Theme.accent)
            VStack(alignment: .leading, spacing: 2) {
                Text("Gardé dans le Trousseau").font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
                Text("La prochaine fois, un clic sur Se connecter suffit.").font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.well, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Theme.line))
        if let error {
            Text(error).font(Theme.sans(13)).foregroundStyle(Theme.warn)
        }
        Spacer()
        HStack {
            Button("Retour") { step = .trust }.buttonStyle(GhostButton(height: 44))
            Spacer()
            Button(checking ? "Vérification…" : "Ajouter l'ordinateur", action: add)
                .buttonStyle(PrimaryButton())
                .disabled(username.isEmpty || password.isEmpty || checking)
                .keyboardShortcut(.defaultAction)
        }
    }

    private func actions(primary: String, secondary: String? = nil, action: @escaping () -> Void) -> some View {
        HStack {
            Spacer()
            if let secondary {
                Button(secondary) { done() }.buttonStyle(GhostButton(height: 44))
            }
            Button(primary, action: action)
                .buttonStyle(PrimaryButton())
                .disabled(mismatch)
                .keyboardShortcut(.defaultAction)
        }
    }

    /// Vérifie le compte auprès de la console (certificat épinglé), puis enregistre.
    private func add() {
        checking = true
        error = nil
        Task {
            let console = ConsoleClient(host: candidate.address, port: candidate.port, username: username, password: password, fingerprint: seen)
            do {
                let info = try await console.info()
                var host = Host(name: info.hostname ?? candidate.name, address: candidate.address, consolePort: candidate.port, username: username)
                host.consoleFingerprint = seen
                app.store.add(host, password: password)
                done()
            } catch ConsoleError.http(401, _) {
                error = "Identifiant ou mot de passe refusé."
            } catch ConsoleError.http(404, _) {
                // Console d'une version précédente : le compte n'a pas pu être vérifié.
                var host = Host(name: candidate.name, address: candidate.address, consolePort: candidate.port, username: username)
                host.consoleFingerprint = seen
                app.store.add(host, password: password)
                done()
            } catch {
                self.error = error.localizedDescription
            }
            checking = false
        }
    }
}

/// Empreinte en groupes de quatre, les quatre premiers en évidence.
private struct FingerprintBox: View {
    let fingerprint: String

    private var groups: [String] {
        let hex = fingerprint.replacingOccurrences(of: "sha256:", with: "").replacingOccurrences(of: ":", with: "").uppercased()
        return stride(from: 0, to: hex.count, by: 4).map { i in
            let start = hex.index(hex.startIndex, offsetBy: i)
            return String(hex[start..<(hex.index(start, offsetBy: 4, limitedBy: hex.endIndex) ?? hex.endIndex)])
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Theme.eyebrow("Empreinte SHA-256 reçue")
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 8), alignment: .leading, spacing: 6) {
                ForEach(Array(groups.enumerated()), id: \.offset) { i, g in
                    Text(g).font(Theme.mono(15, i < 4 ? .medium : .regular)).foregroundStyle(i < 4 ? Theme.accent : Theme.muted)
                }
            }
            Text("Les quatre premiers groupes suffisent : s'ils sont identiques, c'est bien votre ordinateur.")
                .font(Theme.sans(13)).foregroundStyle(Theme.muted)
        }
        .padding(.horizontal, 20).padding(.vertical, 18)
        .background(Theme.bar, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.line))
    }
}
