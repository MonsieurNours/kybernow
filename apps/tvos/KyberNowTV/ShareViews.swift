// KyberNow Share sur l'Apple TV (docs/link-ui.md, « Partage »). L'invité :
// « Rejoindre avec un code », l'écran du code à six chiffres, tapés sur un
// pavé à la télécommande (comme les pavés des apps de salon), et du prénom
// (le nom de l'Apple TV, au clavier tvOS ou de l'iPhone) ; la session d'invité
// s'ouvre ensuite, sans que l'ordinateur soit ajouté. L'hôte : la tuile
// « Partager » du menu de session ouvre le partage et ce panneau, en tuiles et
// boutons : code, QR code, invités et ce qu'on peut faire de chacun,
// « Nouveau code », « Arrêter le partage ». Les appels et les règles sont dans
// Share.swift, commun aux apps d'Apple.

import KyberKit
import SwiftUI

// MARK: - Rejoindre

struct JoinView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var store: HostStore
    @StateObject private var model: ShareJoinModel
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, digit(Int), delete, name, quality(ShareQuality), join }

    init(link: ShareJoinLink?) {
        // « Auto » : la sortie du téléviseur (au plus 4K à 60 Hz).
        let model = ShareJoinModel(link: link) {
            let tv = TVDisplay.native
            return ShareScreen(pixelWidth: tv.width, pixelHeight: tv.height, refresh: tv.refresh)
        }
        #if DEBUG
        // Démonstration : « En attente de l'accord de l'hôte… » (`-demoJoinWaiting YES`), sans relais.
        if UserDefaults.standard.bool(forKey: "demoJoinWaiting") { model.demoWaiting(code: "482 915") }
        #endif
        _model = StateObject(wrappedValue: model)
    }

    /// Les chiffres tapés, sans espace.
    private var digits: String { model.code.filter(\.isNumber) }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            Button { leave() } label: {
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

            // À gauche : le titre, le code et son pavé.
            VStack(alignment: .leading, spacing: 18) {
                Text(ShareText.joinTitle).font(Theme.display(64, 660)).tracking(-64 * 0.035).padding(.bottom, 4)
                Text(ShareText.joinHelp).font(Theme.sans(23)).foregroundStyle(Theme.muted).padding(.bottom, 14)
                codeSlots
                pad
            }
            .focusSection()
            .frame(width: 700, alignment: .leading)
            .padding(.leading, 96).padding(.top, 150)

            // À droite : le prénom, la recherche, « Rejoindre ».
            VStack(alignment: .leading, spacing: 30) {
                VStack(alignment: .leading, spacing: 10) {
                    Text(ShareText.yourName).font(Theme.sans(19)).foregroundStyle(Theme.muted)
                    TextField(ShareText.yourName, text: $model.name)
                        .font(Theme.sans(30))
                        .textInputAutocapitalization(.words)
                        .autocorrectionDisabled()
                        .focused($focus, equals: .name)
                        .frame(width: 640)
                }
                qualityChoice
                status
                Spacer(minLength: 0)
                HStack(spacing: 20) {
                    Button(ShareText.joinButton) { join() }
                        .buttonStyle(PrimaryButton(height: 80, fontSize: 28))
                        .disabled(!model.canJoin)
                        .focused($focus, equals: .join)
                    if model.busy {
                        Button(StationText.cancel) { model.cancel() }
                            .buttonStyle(ChoiceButton(selected: false, height: 80, light: true))
                    }
                }
            }
            .padding(.horizontal, 60).padding(.vertical, 56)
            .frame(width: 1920 - 880 - 96, height: 1080 - 180 - 96, alignment: .topLeading)
            .glass(30)
            .focusSection()
            .padding(.leading, 880).padding(.top, 180)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .digit(1))
        .onExitCommand { leave() }
        .onAppear {
            // Le rappel ne retient que l'app : en capturant la vue, qui tient
            // `model` (StateObject), le modèle se retiendrait lui-même.
            model.onJoined = { [app] found in app.openGuestSession(found) }
            // Un code de prêt : la session d'emprunt prend la suite.
            model.onLend = { [app] source in app.openBorrowSession(source) }
            model.start()
        }
        .onDisappear {
            model.cancel()
            model.stop()
            model.onJoined = nil
            model.onLend = nil
        }
        .onChange(of: model.code) { _, _ in
            model.codeChanged()
            // Six chiffres : le focus passe au prénom, puis à « Rejoindre ».
            if digits.count == 6, case .digit? = focus { focus = .join }
        }
    }

    // MARK: Le code

    /// Six cases, groupées par trois, la prochaine allumée.
    private var codeSlots: some View {
        HStack(spacing: 14) {
            ForEach(0..<6, id: \.self) { i in
                if i == 3 { Spacer().frame(width: 22) }
                let char = i < digits.count ? String(Array(digits)[i]) : ""
                Text(verbatim: char)
                    .font(Theme.mono(60, .medium))
                    .foregroundStyle(Theme.neonCore)
                    .frame(width: 84, height: 112)
                    .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(Color.white.opacity(0.06)))
                    .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous)
                        .strokeBorder(i == digits.count ? Theme.accent : Color.white.opacity(0.12), lineWidth: i == digits.count ? 2.5 : 1))
            }
        }
        .padding(.bottom, 12)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: "\(ShareText.code) \(digits.map(String.init).joined(separator: " "))"))
    }

    /// Le pavé : 1 à 9, effacer, 0 ; un chiffre de plus que six est ignoré.
    private var pad: some View {
        VStack(spacing: 14) {
            ForEach(0..<3, id: \.self) { row in
                HStack(spacing: 14) {
                    ForEach(1...3, id: \.self) { col in digitKey(row * 3 + col) }
                }
            }
            HStack(spacing: 14) {
                Button { eraseLast() } label: {
                    Image(systemName: "delete.left").font(.system(size: 34, weight: .medium))
                        .frame(width: 138, height: 84)
                }
                .buttonStyle(GlassButton(radius: 20, scale: 1.08))
                .focused($focus, equals: .delete)
                .accessibilityLabel(Text("Effacer"))
                digitKey(0)
                Spacer().frame(width: 138)
            }
        }
    }

    private func digitKey(_ n: Int) -> some View {
        Button { type(n) } label: {
            Text(verbatim: "\(n)").font(Theme.mono(40, .medium)).frame(width: 138, height: 84)
        }
        .buttonStyle(GlassButton(radius: 20, scale: 1.08))
        .focused($focus, equals: .digit(n))
    }

    private func type(_ n: Int) {
        guard digits.count < 6, !model.busy else { return }
        model.code = digits + String(n)
    }

    private func eraseLast() {
        guard !digits.isEmpty, !model.busy else { return }
        model.code = String(digits.dropLast())
    }

    // MARK: La qualité de l'image

    /// « Qualité de l'image », sous le prénom : « Auto » (proposé), « Comme
    /// l'hôte », « 4K · 60 Hz », « 1080p · 60 Hz », « 720p · 30 Hz » ; retenue
    /// pour la fois suivante. Les cinq sur une ligne si elle tient, sinon trois puis deux.
    private var qualityChoice: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(ShareText.imageQuality).font(Theme.sans(19)).foregroundStyle(Theme.muted)
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 14) { ForEach(ShareQuality.allCases, id: \.self) { choice($0) } }
                VStack(alignment: .leading, spacing: 14) {
                    HStack(spacing: 14) { choice(.auto); choice(.host); choice(.p2160_60) }
                    HStack(spacing: 14) { choice(.p1080_60); choice(.p720_30) }
                }
            }
        }
        .disabled(model.busy)
    }

    private func choice(_ quality: ShareQuality) -> some View {
        Button { model.quality = quality } label: { Text(quality.label).lineLimit(1).fixedSize() }
            .buttonStyle(ChoiceButton(selected: model.quality == quality, height: 64))
            .focused($focus, equals: .quality(quality))
    }

    // MARK: État

    @ViewBuilder private var status: some View {
        switch model.state {
        case .idle:
            EmptyView()
        case .looking:
            HStack(spacing: 14) {
                ProgressView()
                Text(ShareText.looking).font(Theme.sans(24)).foregroundStyle(Theme.muted)
            }
        case .waiting:
            // Hors de chez l'hôte : sa réponse se fait attendre (« Annuler » retire la demande).
            HStack(spacing: 14) {
                ProgressView()
                Text(ShareText.waiting).font(Theme.sans(24)).foregroundStyle(Theme.muted)
            }
        case let .failed(text):
            HStack(alignment: .top, spacing: 16) {
                Icon("warning", size: 30).foregroundStyle(Theme.warn)
                Text(text).font(Theme.sans(24)).foregroundStyle(Theme.warnText).lineSpacing(4)
                    .frame(maxWidth: 700, alignment: .leading)
            }
        }
    }

    private func join() {
        guard model.canJoin else { return }
        model.join(hosts: store.hosts)
    }

    private func leave() {
        model.cancel()
        app.screen = .home
    }
}

// MARK: - Partager (hôte)

/// Le panneau de l'hôte, sur le menu de session : code, QR code, invités et
/// leurs actions, « Nouveau code », « Arrêter le partage ». Retour le ferme.
struct SharePanelTV: View {
    @ObservedObject var session: SessionController
    @ObservedObject var share: ShareHostModel
    let close: () -> Void
    @FocusState private var focus: Target?

    enum Target: Hashable { case newCode, stop, close, role(String, Int), kick(String), accept(String), decline(String) }

    var body: some View {
        ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 30) {
                Theme.eyebrow(ShareText.title(session.host.name))
                if let reason = session.shareAvailability.reason {
                    Text(reason).font(Theme.sans(30)).foregroundStyle(Theme.warnText).frame(width: 1000, alignment: .leading)
                    closeButton
                } else if let state = share.state {
                    shared(state)
                } else if let error = share.error {
                    Text(error).font(Theme.sans(28)).foregroundStyle(Theme.dangerText).frame(width: 1000, alignment: .leading)
                    HStack(spacing: 20) {
                        Button(ShareText.share) { Task { await share.openIfNeeded() } }
                            .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                        closeButton
                    }
                } else {
                    ProgressView()
                    closeButton
                }
            }
            .padding(56)
            .glass(30, tint: 0.6)
            .focusSection()
        }
        .frame(width: 1920, height: 1080)
        .onExitCommand(perform: close)
        .onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { focus = .close } }
    }

    private var closeButton: some View {
        Button("Fermer", action: close)
            .buttonStyle(ChoiceButton(selected: false, height: 72))
            .focused($focus, equals: .close)
    }

    @ViewBuilder private func shared(_ state: ShareState) -> some View {
        HStack(alignment: .top, spacing: 64) {
            VStack(alignment: .leading, spacing: 20) {
                LinkQRTV(link: state.link, waiting: share.waitingForRelayLink)
                Theme.eyebrow(ShareText.code)
                Text(verbatim: Share.formatCode(state.code))
                    .font(Theme.mono(68, .medium)).tracking(2)
                    .foregroundStyle(Theme.neonCore)
                    .shadow(color: Theme.accent, radius: 14)
                    .lineLimit(1)
                    .fixedSize()
                    .accessibilityLabel(Text(verbatim: "\(ShareText.code) \(state.code.map(String.init).joined(separator: " "))"))
                // L'aide suit l'état du relais (docs/link-ui.md) : relais joint, en préparation
                // (10 s au plus), ou réglé mais pas joint.
                Text(ShareText.help(relay: share.relay)).font(Theme.sans(20)).foregroundStyle(Theme.muted).lineSpacing(4)
                    .frame(width: 440, alignment: .leading)
                RelayLinkNoteTV(relay: share.relay, width: 440)
            }
            VStack(alignment: .leading, spacing: 18) {
                // Les demandes venues par le relais, au-dessus des invités, seulement s'il y en a.
                if !state.requests.isEmpty {
                    Theme.eyebrow(ShareText.requests)
                    ForEach(state.requests) { request in requestRow(request) }
                }
                Theme.eyebrow(ShareText.guests)
                if state.accepted.isEmpty {
                    Text(ShareText.nobody).font(Theme.sans(26)).foregroundStyle(Theme.muted)
                } else {
                    ForEach(state.accepted) { guest in guestRow(guest) }
                }
                if share.full {
                    Text(ShareText.fourGuests).font(Theme.sans(20)).foregroundStyle(Theme.faint)
                }
                if let error = share.error { Text(error).font(Theme.sans(22)).foregroundStyle(Theme.dangerText) }
            }
            .frame(width: 880, alignment: .topLeading)
        }
        HStack(spacing: 20) {
            Button(ShareText.newCode) { Task { await share.newCode() } }
                .buttonStyle(ChoiceButton(selected: false, height: 72))
                .focused($focus, equals: .newCode)
            Button(ShareText.stop) {
                Task {
                    await share.close()
                    close()
                }
            }
            .buttonStyle(ChoiceButton(selected: false, height: 72))
            .focused($focus, equals: .stop)
            closeButton
        }
        .disabled(share.busy)
    }

    /// Une demande venue par le relais : « {prénom} demande à rejoindre », son app
    /// et « hors de chez vous », « Accepter » et « Refuser » sur la même ligne.
    private func requestRow(_ request: ShareGuest) -> some View {
        HStack(spacing: 18) {
            Icon("globe-hemisphere-west", size: 32)
                .foregroundStyle(Theme.accent)
                .frame(width: 40)
            VStack(alignment: .leading, spacing: 4) {
                Text(ShareText.wantsToJoin(request.name)).font(Theme.sans(25, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                Text(ShareText.requestDetail(request)).font(Theme.sans(18)).foregroundStyle(Theme.faint).lineLimit(1)
            }
            Spacer(minLength: 0)
            // Quatre invités au plus : « Accepter » est inactif quand les quatre places sont prises.
            Button { Task { await share.answer(request, accept: true) } } label: { Text(ShareText.accept).lineLimit(1).fixedSize() }
                .buttonStyle(ChoiceButton(selected: true, height: 56, light: true))
                .focused($focus, equals: .accept(request.id))
                .disabled(share.full)
            Button { Task { await share.answer(request, accept: false) } } label: { Text(ShareText.decline).lineLimit(1).fixedSize() }
                .buttonStyle(ChoiceButton(selected: false, height: 56, light: true))
                .focused($focus, equals: .decline(request.id))
        }
        .disabled(share.busy)
    }

    /// Un invité : son prénom, son rôle, son app et depuis quand ; ses actions.
    /// Le prénom, le rôle, l'app et la durée sur une ligne, les actions dessous :
    /// « Donner le clavier et la souris » tient en entier, sans être coupé.
    private func guestRow(_ guest: ShareGuest) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 18) {
                Icon(ShareText.icon(guest.role), size: 32)
                    .foregroundStyle(guest.role.plays ? Theme.accent : Theme.faint)
                    .frame(width: 40)
                VStack(alignment: .leading, spacing: 4) {
                    Text(ShareText.guest(guest.name, guest.role)).font(Theme.sans(25, .semibold)).foregroundStyle(Theme.text).lineLimit(2)
                    Text(detail(guest)).font(Theme.sans(18)).foregroundStyle(Theme.faint).lineLimit(1)
                }
                Spacer(minLength: 0)
            }
            HStack(spacing: 14) {
                ForEach(Array(ShareHostModel.roleActions(for: guest.role).enumerated()), id: \.offset) { i, action in
                    Button { Task { await share.setRight(action.right, on: action.on, of: guest) } } label: { Text(action.title).lineLimit(1).fixedSize() }
                        .buttonStyle(ChoiceButton(selected: false, height: 56, light: true))
                        .focused($focus, equals: .role(guest.id, i))
                }
                Button { Task { await share.kick(guest) } } label: { Text(ShareText.remove).lineLimit(1).fixedSize() }
                    .buttonStyle(ChoiceButton(selected: false, height: 56, light: true))
                    .focused($focus, equals: .kick(guest.id))
            }
            .padding(.leading, 58)
        }
        .disabled(share.busy)
    }

    /// L'app, la qualité d'image si ce n'est pas celle de l'hôte, depuis quand.
    private func detail(_ guest: ShareGuest) -> String { ShareText.guestDetail(guest) }

}

/// Sous l'aide du partage ou du code de prêt, l'état du relais
/// (docs/link-ui.md) : « Préparation du lien hors de chez vous… » avec
/// l'indicateur d'attente (10 s au plus), ou, relais réglé mais pas joint,
/// « Pas encore joignable hors de chez vous. » ; rien sans relais ou relais joint.
struct RelayLinkNoteTV: View {
    let relay: ShareState.Relay?
    let width: CGFloat

    var body: some View {
        if relay == .connecting {
            HStack(spacing: 14) {
                ProgressView()
                Text(ShareText.relayConnecting).font(Theme.sans(20)).foregroundStyle(Theme.faint).lineSpacing(4)
            }
            .frame(width: width, alignment: .leading)
        } else if relay == .unreachable {
            Text(ShareText.relayUnreachable).font(Theme.sans(20)).foregroundStyle(Theme.faint).lineSpacing(4)
                .frame(width: width, alignment: .leading)
        }
    }
}

/// Le QR code du lien des apps (`kybernow://join…`) d'un partage ou d'un code
/// de prêt, net, sur fond blanc ; `waiting` : le lien du relais se prépare, le
/// QR code l'attend.
struct LinkQRTV: View {
    let link: String
    var waiting = false
    var size: CGFloat = 240
    @State private var cache: (link: String, image: CGImage)?

    var body: some View {
        let image = waiting ? nil : cache?.link == link ? cache?.image : Share.qrImage(for: link)
        Group {
            if let image {
                Image(decorative: image, scale: 1)
                    .interpolation(.none)
                    .resizable()
                    .aspectRatio(1, contentMode: .fit)
                    .frame(width: size, height: size)
                    .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                    .accessibilityElement()
                    .accessibilityLabel(ShareText.qrLabel)
                    .onAppear { if cache?.link != link { cache = (link, image) } }
            } else {
                RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color.white.opacity(0.08)).frame(width: size, height: size)
                    .overlay { if waiting { ProgressView() } }
            }
        }
    }
}
