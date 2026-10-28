// KyberNow Share sur iPhone et iPad (docs/link-ui.md, « Partage »).
// L'invité : « Rejoindre avec un code », l'écran du code à six chiffres et du
// prénom ; la session d'invité s'ouvre ensuite, sans que l'ordinateur soit
// ajouté. L'hôte : le panneau de « Partager » dans la barre de session (code,
// QR code, « Copier le lien », invités et ce qu'on peut faire de chacun,
// « Nouveau code », « Arrêter le partage »). Les appels et les règles sont
// dans Share.swift, commun aux apps d'Apple.

import KyberKit
import SwiftUI
import UIKit

// MARK: - Rejoindre

struct JoinView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @Environment(\.dismiss) private var dismiss
    @StateObject private var model: ShareJoinModel
    @StateObject private var camera = AddHostCamera()
    @FocusState private var codeFocused: Bool
    /// « Code » (taper les six chiffres) ou « QR code » (scanner celui de l'hôte).
    @State private var source: Source = .code

    enum Source: Hashable { case code, qr }

    init(link: ShareJoinLink?) {
        // « Auto » : l'écran de l'appareil (pixels réels, fréquence).
        let model = ShareJoinModel(link: link) { DeviceDisplay.shareScreen() }
        #if DEBUG && targetEnvironment(simulator)
        // Démonstration : « En attente de l'accord de l'hôte… » (`-demoJoinWaiting YES`), sans relais.
        if UserDefaults.standard.bool(forKey: "demoJoinWaiting") { model.demoWaiting(code: "482 915") }
        #endif
        _model = StateObject(wrappedValue: model)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top) {
                    Text(ShareText.joinTitle)
                        .font(Theme.display(34, 660)).tracking(-34 * 0.03)
                        .foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 12)
                    Button { close() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text(StationText.cancel))
                }
                Text(ShareText.joinHelp).font(Theme.sans(15)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
                // Un appareil photo : le choix « Code » / « QR code » en haut de l'écran.
                JoinSourcePicker(source: $source)
                if source == .qr { scanner } else { codeField }
                AddHostField(label: ShareText.yourName, text: $model.name, submit: .go) { join() }
                // « Qualité de l'image » : « Auto » proposé, retenu pour la fois suivante.
                QualityPicker(quality: $model.quality).disabled(model.busy)
                status
                Button(ShareText.joinButton) { join() }
                    .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
                    .disabled(!model.canJoin)
                    .opacity(model.canJoin ? 1 : 0.45)
                    .saturation(model.canJoin ? 1 : 0.6)
                if model.busy {
                    Button(StationText.cancel) { model.cancel() }
                        .buttonStyle(GlassButton(height: 48, fontSize: 16))
                        .frame(maxWidth: .infinity)
                }
            }
            .padding(24)
            .frame(maxWidth: 560, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .scrollDismissesKeyboard(.interactively)
        .background(Theme.background)
        .environment(\.addHostWide, DeviceDisplay.isPad)
        .onAppear {
            // Le rappel ne retient que l'app et l'action de fermeture : en
            // capturant la vue, qui tient `model` (StateObject), le modèle se
            // retiendrait lui-même.
            model.onJoined = { [app, dismiss] found in
                dismiss()
                // La feuille se ferme, puis la session s'ouvre à sa place.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { app.openGuestSession(found) }
            }
            model.start()
            codeFocused = source == .code
        }
        .onDisappear {
            model.cancel()
            model.stop()
            model.onJoined = nil
            camera.stop()
            camera.onCode = nil
        }
        .onChange(of: model.code) { _, _ in model.codeChanged() }
        .onChange(of: source) { _, now in
            // La caméra ne s'ouvre (et l'autorisation n'est demandée) que quand la personne choisit « QR code ».
            if now == .qr {
                codeFocused = false
                camera.onCode = { [self] in scanned($0) }
                camera.start()
            } else {
                camera.stop()
                camera.onCode = nil
            }
        }
    }

    /// La caméra : le QR code du partage (celui de l'hôte, ou un lien `kybernow://join`).
    private var scanner: some View {
        AddHostCameraViewer(
            camera: camera, wide: DeviceDisplay.isPad,
            denied: "KyberNow Link n'a pas accès à la caméra. Autorisez-la dans les Réglages, ou tapez le code.",
            unavailable: "Pas de caméra disponible sur cet appareil. Tapez le code."
        )
        .disabled(model.busy)
    }

    /// Un QR code lu : le code et l'ordinateur sont remplis, comme avec un lien ;
    /// rien ne part avant « Rejoindre ». Les autres QR codes croisés sont ignorés.
    private func scanned(_ text: String) {
        guard case let .join(link)? = LinkKind.parse(text) else { return }
        camera.onCode = nil
        camera.stop()
        UIImpactFeedbackGenerator(style: .medium).impactOccurred()
        model.use(link: link)
        source = .code
    }

    /// Les six chiffres, en grand, groupés par trois.
    private var codeField: some View {
        TextField(text: $model.code, prompt: Text(verbatim: "123 456").foregroundStyle(Theme.ghost)) { Text(ShareText.code) }
            .keyboardType(.numberPad)
            .textContentType(.oneTimeCode)
            .autocorrectionDisabled()
            .focused($codeFocused)
            .font(Theme.mono(40, .medium))
            .tracking(4)
            .foregroundStyle(Theme.neonCore)
            .tint(Theme.accent)
            .multilineTextAlignment(.center)
            .padding(.horizontal, 16)
            .frame(height: 76)
            .background(RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Theme.surface))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(codeFocused ? Theme.accent : Theme.line2))
            .disabled(model.busy)
            .accessibilityLabel(Text(ShareText.code))
    }

    @ViewBuilder private var status: some View {
        switch model.state {
        case .idle:
            EmptyView()
        case .looking:
            HStack(spacing: 10) {
                HomeSpinner(size: 13).foregroundStyle(Theme.accentLight).frame(width: 16, height: 16)
                Text(ShareText.looking).font(Theme.sans(14)).foregroundStyle(Theme.muted)
            }
            .accessibilityElement(children: .combine)
        case .waiting:
            // Hors de chez l'hôte : sa réponse se fait attendre (« Annuler » retire la demande).
            HStack(spacing: 10) {
                HomeSpinner(size: 13).foregroundStyle(Theme.accentLight).frame(width: 16, height: 16)
                Text(ShareText.waiting).font(Theme.sans(14)).foregroundStyle(Theme.muted)
            }
            .accessibilityElement(children: .combine)
        case let .failed(text):
            AddHostNotice(kind: .warn, text: text)
        }
    }

    private func join() {
        guard model.canJoin else { return }
        codeFocused = false
        model.join(hosts: store.hosts)
    }

    private func close() {
        model.cancel()
        dismiss()
    }
}

/// « Code » / « QR code » : le choix en haut de « Rejoindre un partage » (appareil photo).
private struct JoinSourcePicker: View {
    @Binding var source: JoinView.Source

    var body: some View {
        HStack(spacing: 3) {
            option(.code, ShareText.code)
            option(.qr, ShareText.qrCode)
        }
        .padding(4)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.white.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
        .animation(.easeOut(duration: 0.2), value: source)
        .accessibilityElement(children: .contain)
    }

    private func option(_ value: JoinView.Source, _ title: String) -> some View {
        let on = source == value
        return Button { source = value } label: {
            Text(title)
                .font(Theme.sans(14, .semibold))
                .foregroundStyle(on ? Theme.text : Theme.muted)
                .lineLimit(1)
                .frame(maxWidth: .infinity, minHeight: 44)
                .background {
                    if on {
                        RoundedRectangle(cornerRadius: 9, style: .continuous)
                            .fill(Color.white.opacity(0.12))
                            .overlay(alignment: .bottom) { Rectangle().fill(Theme.accent).frame(height: 2) }
                            .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
                    }
                }
                .contentShape(Rectangle())
        }
        .buttonStyle(AddHostPressStyle())
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

/// « Qualité de l'image », sous le prénom : « Auto » (proposé), « Comme
/// l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz ». Les cinq sur
/// une ligne si la largeur le permet, sinon trois puis deux (iPhone et iPad en
/// portrait), et deux par ligne si même cela ne tient pas (grand texte).
private struct QualityPicker: View {
    @Binding var quality: ShareQuality
    @Environment(\.addHostWide) private var wide

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 8 : 6) {
            Text(ShareText.imageQuality).font(Theme.sans(wide ? 14 : 13)).foregroundStyle(Theme.muted)
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 8) { ForEach(ShareQuality.allCases, id: \.self) { option($0) } }
                VStack(spacing: 8) {
                    HStack(spacing: 8) { option(.auto); option(.host); option(.p2160_60) }
                    HStack(spacing: 8) { option(.p1080_60); option(.p720_30) }
                }
                VStack(spacing: 8) {
                    HStack(spacing: 8) { option(.auto); option(.host) }
                    HStack(spacing: 8) { option(.p2160_60); option(.p1080_60) }
                    HStack(spacing: 8) { option(.p720_30) }
                }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text(ShareText.imageQuality))
    }

    private func option(_ value: ShareQuality) -> some View {
        let on = quality == value
        return Button { quality = value } label: {
            Text(value.label)
                .font(Theme.sans(15, .semibold))
                .foregroundStyle(on ? Theme.text : Theme.muted)
                .lineLimit(1)
                .fixedSize(horizontal: true, vertical: false)
                .padding(.horizontal, 12)
                .frame(maxWidth: .infinity, minHeight: 48)
                .background {
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(on ? Color.white.opacity(0.12) : Theme.surface)
                        .overlay(alignment: .bottom) { if on { Rectangle().fill(Theme.accent).frame(height: 2) } }
                        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                }
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(on ? Theme.accent.opacity(0.6) : Theme.line2))
                .contentShape(Rectangle())
        }
        .buttonStyle(AddHostPressStyle())
        .accessibilityAddTraits(on ? .isSelected : [])
    }
}

// MARK: - Partager (hôte)

/// Le panneau de l'hôte : code, QR code, « Copier le lien », invités, actions.
struct SharePanelView: View {
    @ObservedObject var session: SessionController
    @ObservedObject var share: ShareHostModel
    @Environment(\.dismiss) private var dismiss
    @State private var copyResult: String?
    @State private var copied = false
    @State private var qr: (link: String, image: CGImage)?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack(alignment: .top) {
                    Text(ShareText.title(session.host.name))
                        .font(Theme.sans(20, .semibold)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 12)
                    Button { dismiss() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text("Fermer"))
                }
                if let reason = session.shareAvailability.reason {
                    AddHostNotice(kind: .warn, text: reason)
                } else if let state = share.state {
                    shared(state)
                } else if let error = share.error {
                    AddHostNotice(kind: .danger, text: error)
                    Button(ShareText.share) { Task { await share.openIfNeeded() } }
                        .buttonStyle(PrimaryButton(height: 48, fontSize: 16))
                        .disabled(share.busy)
                } else {
                    HomeSpinner(size: 16).foregroundStyle(Theme.accentLight)
                }
            }
            .padding(20)
        }
        .background(Theme.bar)
        .environment(\.colorScheme, .dark)
    }

    @ViewBuilder private func shared(_ state: ShareState) -> some View {
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 20) {
                qrImage(for: state.link)
                codeAndCopy(state)
            }
            VStack(alignment: .leading, spacing: 16) {
                qrImage(for: state.link)
                codeAndCopy(state)
            }
        }
        // L'aide suit l'état du relais : relais joint (« Sur le même réseau ou ailleurs… »),
        // ou, relais réglé mais pas joint, le réseau seulement et « Pas encore joignable hors de chez vous. ».
        VStack(alignment: .leading, spacing: 6) {
            Text(ShareText.help(relay: state.relay)).font(Theme.sans(13.5)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            if state.relay == .unreachable {
                Text(ShareText.relayUnreachable).font(Theme.sans(13.5)).foregroundStyle(Theme.faint).fixedSize(horizontal: false, vertical: true)
            }
        }
        Rectangle().fill(Theme.line).frame(height: 1)
        // Les demandes venues par le relais, au-dessus des invités, seulement s'il y en a.
        if !state.requests.isEmpty {
            VStack(alignment: .leading, spacing: 14) {
                Theme.eyebrow(ShareText.requests, color: Theme.faint)
                ForEach(state.requests) { request in RequestRow(share: share, request: request) }
            }
            Rectangle().fill(Theme.line).frame(height: 1)
        }
        VStack(alignment: .leading, spacing: 12) {
            Theme.eyebrow(ShareText.guests, color: Theme.faint)
            if state.accepted.isEmpty {
                Text(ShareText.nobody).font(Theme.sans(15)).foregroundStyle(Theme.muted)
            } else {
                ForEach(state.accepted) { guest in GuestRow(share: share, guest: guest) }
            }
            if share.full {
                Text(ShareText.fourGuests).font(Theme.sans(13)).foregroundStyle(Theme.faint)
            }
        }
        if let error = share.error { AddHostNotice(kind: .danger, text: error) }
        HStack(spacing: 10) {
            Button(ShareText.newCode) { Task { await share.newCode() } }
                .buttonStyle(GlassButton(height: 48, fontSize: 15))
            Button(ShareText.stop) { Task { await share.close() } }
                .buttonStyle(AddHostDangerButton())
        }
        .disabled(share.busy)
    }

    private func codeAndCopy(_ state: ShareState) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Theme.eyebrow(ShareText.code, color: Theme.faint)
            Text(verbatim: Share.formatCode(state.code))
                .font(Theme.mono(36, .medium)).tracking(2)
                .foregroundStyle(Theme.neonCore)
                .neon(.lit)
                .fixedSize()
                .accessibilityLabel(Text(verbatim: "\(ShareText.code) \(state.code.map(String.init).joined(separator: " "))"))
            Button { copyLink() } label: {
                HStack(spacing: 8) {
                    Icon(copied ? "check" : "copy", size: 16, bold: true)
                    Text(ShareText.copyLink)
                }
            }
            .buttonStyle(GlassButton(height: 44, fontSize: 15))
            if let copyResult {
                Text(copyResult).font(Theme.sans(13)).foregroundStyle(copied ? Theme.ok : Theme.warnText)
                    .fixedSize(horizontal: false, vertical: true)
                // Copie impossible : l'adresse à sélectionner à la main.
                if !copied, let link = share.linkToCopy {
                    Text(verbatim: link).font(Theme.mono(12)).foregroundStyle(Theme.text2)
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    /// Le QR code du lien des apps (`kybernow://join…`), net, sur fond blanc.
    @ViewBuilder private func qrImage(for link: String) -> some View {
        let image = qr?.link == link ? qr?.image : Share.qrImage(for: link)
        Group {
            if let image {
                Image(decorative: image, scale: 1)
                    .interpolation(.none)
                    .resizable()
                    .aspectRatio(1, contentMode: .fit)
                    .frame(width: 150, height: 150)
                    .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    .accessibilityElement()
                    .accessibilityLabel(ShareText.qrLabel)
                    .onAppear { if qr?.link != link { qr = (link, image) } }
            } else {
                RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Theme.surface).frame(width: 150, height: 150)
            }
        }
    }

    private func copyLink() {
        guard let text = share.linkToCopy else { return }
        UIPasteboard.general.string = text
        let ok = UIPasteboard.general.hasStrings
        copied = ok
        copyResult = ok ? ShareText.linkCopied : ShareText.copyFailed
        // « Lien copié. » s'efface ; « Copie impossible… » et l'adresse restent à sélectionner.
        guard ok else { return }
        Task {
            try? await Task.sleep(for: .seconds(3))
            guard copied else { return }
            copyResult = nil
            copied = false
        }
    }
}

/// Une demande venue par le relais (docs/link-ui.md, « Demandes ») : « {prénom}
/// demande à rejoindre », son app et « hors de chez vous », « Accepter » et
/// « Refuser ». Elle ne prend pas de place d'invité tant qu'elle n'est pas acceptée.
private struct RequestRow: View {
    @ObservedObject var share: ShareHostModel
    let request: ShareGuest

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .center, spacing: 12) {
                Icon("globe-hemisphere-west", size: 20)
                    .foregroundStyle(Theme.accent)
                    .frame(width: 24)
                VStack(alignment: .leading, spacing: 3) {
                    Text(ShareText.wantsToJoin(request.name)).font(Theme.sans(15, .medium)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(ShareText.requestDetail(request)).font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
                }
                Spacer(minLength: 0)
            }
            HStack(spacing: 10) {
                Button(ShareText.accept) { Task { await share.answer(request, accept: true) } }
                    .buttonStyle(PrimaryButton(height: 46, fontSize: 15))
                Button(ShareText.decline) { Task { await share.answer(request, accept: false) } }
                    .buttonStyle(GlassButton(height: 46, fontSize: 15))
            }
            .disabled(share.busy)
        }
        .accessibilityElement(children: .contain)
    }
}

/// Un invité : son prénom et son rôle, son app et depuis quand, et ce que
/// l'hôte peut en faire (rôle, « Retirer »).
private struct GuestRow: View {
    @ObservedObject var share: ShareHostModel
    let guest: ShareGuest

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            Icon(ShareText.icon(guest.role), size: 20)
                .foregroundStyle(guest.role.plays ? Theme.accent : Theme.faint)
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 3) {
                Text(ShareText.guest(guest.name, guest.role)).font(Theme.sans(15, .medium)).foregroundStyle(Theme.text)
                    .fixedSize(horizontal: false, vertical: true)
                TimelineView(.periodic(from: .now, by: 20)) { _ in
                    Text(detail).font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
                }
            }
            Spacer(minLength: 8)
            Menu {
                ForEach(Array(ShareHostModel.roleActions(for: guest.role).enumerated()), id: \.offset) { _, action in
                    Button(action.title) { Task { await share.setRight(action.right, on: action.on, of: guest) } }
                }
                Divider()
                Button(ShareText.remove, role: .destructive) { Task { await share.kick(guest) } }
            } label: {
                Icon("sliders-horizontal", size: 20).foregroundStyle(Theme.text2).frame(width: 44, height: 44)
                    .glass(12)
            }
            .disabled(share.busy)
            .accessibilityLabel(Text(guest.name))
        }
    }

    /// « iPhone ou iPad · 1080p · 60 Hz · depuis 3 min » : sa qualité d'image
    /// seulement si ce n'est pas celle de l'hôte.
    private var detail: String { ShareText.guestDetail(guest) }
}
