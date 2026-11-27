// « Envoyer le journal » sur l'Apple TV (docs/link-ui.md, « Réglages de
// l'app » › À propos, étapes par QR code) : le journal est préparé, chiffré
// sur l'Apple TV et déposé sur le relais du compte KyberNow (sinon le relais
// public), qui ne peut pas le lire ; le téléphone qui scanne le QR code
// l'ouvre, puis l'enregistre ou le partage (`AppJournal`, docs/relais.md,
// « Dépôt d'un journal »). Le panneau relit l'état du dépôt toutes les 2 s ;
// « Terminé » retire un dépôt pas encore ouvert.

import KyberKit
import SwiftUI

@MainActor
final class JournalSend: ObservableObject {
    enum Step: Equatable {
        case preparing, uploading
        case code(AppJournal.Deposit)
        case opened, expired
        case failed(String)
    }

    @Published private(set) var step: Step = .preparing
    private var qr: (url: String, image: CGImage)?
    private var polling: Task<Void, Never>?
    private let relay: String?

    init(relay: String?) {
        self.relay = relay
    }

    /// Le QR code du dépôt, fait une fois.
    func image(for deposit: AppJournal.Deposit) -> CGImage? {
        if let qr, qr.url == deposit.url { return qr.image }
        guard let image = Share.qrImage(for: deposit.url) else { return nil }
        qr = (deposit.url, image)
        return image
    }

    /// Prépare, dépose, puis suit le dépôt jusqu'à son ouverture ou son expiration.
    func start() {
        polling?.cancel()
        step = .preparing
        polling = Task { [weak self, relay] in
            // « Préparation du journal… » se voit le temps de lire le journal.
            try? await Task.sleep(for: .milliseconds(300))
            self?.step = .uploading
            let result = await AppJournal.upload(relay: relay)
            guard let self, !Task.isCancelled else { return }
            switch result {
            case let .failure(f):
                self.step = .failed(JournalText.failure(f))
            case let .success(deposit):
                self.step = .code(deposit)
                await self.follow(deposit)
            }
        }
    }

    private func follow(_ deposit: AppJournal.Deposit) async {
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(2))
            guard !Task.isCancelled, case .code = step else { return }
            if Date() >= deposit.expires {
                step = .expired
                return
            }
            switch await AppJournal.state(deposit) {
            case .opened:
                step = .opened
                return
            case .gone:
                step = .expired
                return
            case .waiting, .unreachable:
                // Un relais muet un instant : le QR code reste, l'état se relit.
                break
            }
        }
    }

    /// « Terminé » : le dépôt pas encore ouvert est retiré.
    func finish() {
        polling?.cancel()
        polling = nil
        if case let .code(deposit) = step { Task { await AppJournal.remove(deposit) } }
    }
}

/// Le panneau du QR code, par-dessus les Réglages de l'app.
struct JournalPanel: View {
    @ObservedObject var send: JournalSend
    let close: () -> Void
    @FocusState private var focused: Bool

    var body: some View {
        ZStack {
            Color.black.opacity(0.6).ignoresSafeArea()
            VStack(spacing: 26) {
                Text(JournalText.send).font(Theme.display(44, 640)).tracking(-44 * 0.02)
                content
                    .frame(minHeight: 420)
                buttons
            }
            .padding(.horizontal, 70).padding(.vertical, 56)
            .frame(width: 900)
            .glass(30)
        }
        .onExitCommand { finish() }
    }

    @ViewBuilder private var content: some View {
        switch send.step {
        case .preparing, .uploading:
            VStack(spacing: 24) {
                ProgressView().scaleEffect(1.6)
                Text(send.step == .preparing ? JournalText.preparing : JournalText.uploading)
                    .font(Theme.sans(24)).foregroundStyle(Theme.text2)
            }
        case let .code(deposit):
            VStack(spacing: 22) {
                if let qr = send.image(for: deposit) {
                    Image(decorative: qr, scale: 1)
                        .interpolation(.none)
                        .resizable()
                        .frame(width: 300, height: 300)
                        .padding(18)
                        .background(RoundedRectangle(cornerRadius: 22, style: .continuous).fill(Theme.text))
                        .accessibilityLabel(Text(JournalText.scan))
                }
                Text(JournalText.scan)
                    .font(Theme.sans(23)).foregroundStyle(Theme.text)
                    .multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
                Text(JournalText.encrypted)
                    .font(Theme.sans(18)).foregroundStyle(Theme.muted)
                    .multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
            }
        case .opened:
            message(JournalText.opened, icon: "check-circle", color: Theme.accentLight)
        case .expired:
            message(JournalText.expired, icon: "warning", color: Theme.warnText)
        case let .failed(text):
            message(text, icon: "warning", color: Theme.warnText)
        }
    }

    private func message(_ text: String, icon: String, color: Color) -> some View {
        VStack(spacing: 22) {
            Icon(icon, size: 64).foregroundStyle(color)
            Text(text).font(Theme.sans(24)).foregroundStyle(Theme.text)
                .multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
        }
    }

    @ViewBuilder private var buttons: some View {
        HStack(spacing: 18) {
            switch send.step {
            case .expired:
                Button(JournalText.newCode) { send.start() }
                    .buttonStyle(ChoiceButton(selected: false, height: 72, light: true))
                    .focused($focused)
            case .failed:
                Button(JournalText.retry) { send.start() }
                    .buttonStyle(ChoiceButton(selected: false, height: 72, light: true))
                    .focused($focused)
            default:
                EmptyView()
            }
            Button(JournalText.done) { finish() }
                .buttonStyle(ChoiceButton(selected: false, height: 72))
        }
        .focusSection()
        .onAppear { focused = true }
    }

    private func finish() {
        send.finish()
        close()
    }
}
