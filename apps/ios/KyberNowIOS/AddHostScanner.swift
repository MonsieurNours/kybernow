// QR code de la console, lu à la caméra (AVCaptureMetadataOutput) : le lien
// `kybernow://adresse:port?name=…&fp=…` donne l'adresse, le port et
// l'empreinte, vérifiée ensuite toute seule. Caméra arrière sur iPhone,
// avant sur iPad (l'iPad posé face à l'écran de l'ordinateur). Un lien copié
// depuis la console se colle aussi.

import AVFoundation
import KyberKit
import SwiftUI
import UIKit

// MARK: - Caméra

final class AddHostCamera: NSObject, ObservableObject, AVCaptureMetadataOutputObjectsDelegate {
    enum State { case idle, running, denied, unavailable }

    let session = AVCaptureSession()
    @Published private(set) var state: State = .idle
    /// La caméra retenue (pour tenir l'image droite quand l'appareil tourne).
    @Published private(set) var device: AVCaptureDevice?
    /// Appelé sur le fil principal à chaque code lu.
    var onCode: ((String) -> Void)?
    private let queue = DispatchQueue(label: "fr.leslignesclaires.kybernow.qr")

    /// L'autorisation n'est demandée qu'ici, quand la personne a choisi la caméra.
    func start() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configure()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { ok in
                DispatchQueue.main.async {
                    if ok { self.configure() } else { self.state = .denied }
                }
            }
        default:
            state = .denied
        }
    }

    private func configure() {
        if device != nil {
            state = .running
            queue.async { self.session.startRunning() }
            return
        }
        let position: AVCaptureDevice.Position = UIDevice.current.userInterfaceIdiom == .pad ? .front : .back
        guard let device = AVCaptureDevice.default(.builtInWideAngleCamera, for: .video, position: position) ?? AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device)
        else {
            state = .unavailable
            return
        }
        session.beginConfiguration()
        if session.canAddInput(input) { session.addInput(input) }
        let output = AVCaptureMetadataOutput()
        if session.canAddOutput(output) {
            session.addOutput(output)
            output.setMetadataObjectsDelegate(self, queue: .main)
            if output.availableMetadataObjectTypes.contains(.qr) { output.metadataObjectTypes = [.qr] }
        }
        session.commitConfiguration()
        self.device = device
        state = .running
        queue.async { self.session.startRunning() }
    }

    func stop() {
        queue.async { self.session.stopRunning() }
    }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
        for case let object as AVMetadataMachineReadableCodeObject in metadataObjects {
            if let value = object.stringValue { onCode?(value) }
        }
    }
}

/// L'image de la caméra, tenue droite quelle que soit l'orientation.
private final class AddHostPreviewView: UIView {
    override class var layerClass: AnyClass { AVCaptureVideoPreviewLayer.self }
    var preview: AVCaptureVideoPreviewLayer { layer as! AVCaptureVideoPreviewLayer }
    private var coordinator: AVCaptureDevice.RotationCoordinator?
    private var observation: NSKeyValueObservation?

    func attach(_ device: AVCaptureDevice?) {
        guard coordinator == nil, let device else { return }
        let c = AVCaptureDevice.RotationCoordinator(device: device, previewLayer: preview)
        coordinator = c
        observation = c.observe(\.videoRotationAngleForHorizonLevelPreview, options: [.new]) { [weak self] _, _ in
            DispatchQueue.main.async { self?.level() }
        }
        level()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        level()
    }

    private func level() {
        guard let coordinator, let connection = preview.connection else { return }
        let angle = coordinator.videoRotationAngleForHorizonLevelPreview
        if connection.isVideoRotationAngleSupported(angle) { connection.videoRotationAngle = angle }
    }
}

private struct AddHostCameraPreview: UIViewRepresentable {
    @ObservedObject var camera: AddHostCamera

    func makeUIView(context: Context) -> AddHostPreviewView {
        let view = AddHostPreviewView()
        view.preview.session = camera.session
        view.preview.videoGravity = .resizeAspectFill
        return view
    }

    func updateUIView(_ view: AddHostPreviewView, context: Context) {
        view.attach(camera.device)
    }
}

// MARK: - Volet

struct AddHostQRPane: View {
    @EnvironmentObject private var flow: AddHostFlow
    @EnvironmentObject private var store: HostStore
    @Environment(\.addHostWide) private var wide
    @StateObject private var camera = AddHostCamera()
    @State private var notice: String?

    init() {}

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 20 : 14) {
            AddHostHeader(eyebrow: String(localized: "Caméra"), title: String(localized: "Visez le QR code d'appairage"))
            AddHostParagraph(String(localized: "Il est dans l'espace Appareils Link de KyberNow Admin. Le lien qu'il porte donne l'adresse, le port et l'empreinte : elle est vérifiée toute seule."))
            viewer
            if let notice {
                AddHostNotice(kind: .warn, text: notice)
            }
            HStack(spacing: 14) {
                PasteButton(payloadType: String.self) { strings in
                    let text = strings.first ?? ""
                    DispatchQueue.main.async { use(text, pasted: true) }
                }
                .tint(Theme.accentFill)
                .frame(minHeight: 44)
                Text("Ou collez le lien kybernow:// copié depuis KyberNow Admin.")
                    .font(Theme.sans(13)).foregroundStyle(Theme.faint)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onAppear {
            camera.onCode = { use($0, pasted: false) }
            camera.start()
        }
        .onDisappear { camera.stop() }
    }

    private var viewer: some View {
        ZStack {
            LinearGradient(colors: [Theme.rgb(0x1A2232), Theme.rgb(0x0A0F17)], startPoint: .topLeading, endPoint: .bottomTrailing)
            switch camera.state {
            case .running:
                AddHostCameraPreview(camera: camera)
                AddHostViewfinder(size: wide ? 220 : 180, corner: wide ? 40 : 30)
            case .idle:
                AddHostViewfinder(size: wide ? 220 : 180, corner: wide ? 40 : 30)
            case .denied:
                VStack(spacing: 14) {
                    Text("KyberNow Link n'a pas accès à la caméra. Autorisez-la dans les Réglages, ou collez le lien copié depuis KyberNow Admin.")
                        .font(Theme.sans(14)).foregroundStyle(Theme.text2)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: false, vertical: true)
                    Button("Ouvrir les Réglages") {
                        if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                    }
                    .buttonStyle(GlassButton())
                }
                .padding(24)
            case .unavailable:
                Text("Pas de caméra disponible sur cet appareil. Collez le lien copié depuis KyberNow Admin.")
                    .font(Theme.sans(14)).foregroundStyle(Theme.text2)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(24)
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: wide ? 420 : 320)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(Color.white.opacity(0.1)))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Image de la caméra"))
    }

    /// Un code lu ou un lien collé : l'ordinateur qu'il désigne passe à la vérification.
    private func use(_ text: String, pasted: Bool) {
        guard let c = PairingLink.parse(text) else {
            // La caméra peut croiser d'autres QR codes : seul un lien collé est signalé.
            if pasted { notice = String(localized: "Ce n'est pas un lien KyberNow.") }
            return
        }
        if AddHostFlow.known(c, in: store) {
            notice = String(localized: "\(c.name) est déjà dans vos ordinateurs.")
            return
        }
        camera.onCode = nil
        camera.stop()
        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
        flow.found(c)
    }
}
