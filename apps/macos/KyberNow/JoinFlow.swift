// « Rejoindre avec un code » (KyberNow Share ; textes : docs/link-ui.md,
// « Partage ») : l'écran du code à six chiffres et du prénom (celui de la
// dernière fois, sinon le nom de ce Mac). Le code part vers les ordinateurs
// trouvés sur le réseau, celui qui partage répond ; la session d'invité
// s'ouvre alors, sans que l'ordinateur soit ajouté à cette liste. Un code
// de prêt ouvre la session d'emprunt ; un code d'invitation
// (docs/invitations.md) demande à la place son mot de passe, dans la même
// feuille (« Ajouter {ordinateur} »), et ajoute l'ordinateur à l'accueil.

import KyberKit
import SwiftUI

struct JoinSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @StateObject private var model: ShareJoinModel
    @FocusState private var codeFocused: Bool
    /// « Rejoindre » est tombé sur un code d'invitation : la feuille demande son mot de passe.
    @State private var invitation: InviteCodeFound?

    init(initialLink: ShareJoinLink?, demoWaiting: Bool = false) {
        // « Auto » : l'écran de la fenêtre, où s'ouvre la session d'invité.
        let model = ShareJoinModel(link: initialLink) { DisplayInfo.shareScreen(NSApp.keyWindow?.screen) }
        #if DEBUG
        // Démonstration : l'attente de l'accord de l'hôte (hors de chez lui), sans relais.
        if demoWaiting { model.demoWaiting(code: "482 915") }
        #endif
        _model = StateObject(wrappedValue: model)
    }

    /// Les cinq qualités, « Auto » proposé ; le choix est retenu pour la fois suivante.
    private var qualityChoice: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(ShareText.imageQuality).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
            Segmented(options: ShareQuality.allCases.map { ($0, $0.label) },
                      selection: Binding(get: { model.quality }, set: { model.quality = $0 ?? model.quality }),
                      height: 38, fill: true)
                .disabled(model.busy)
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(ShareText.imageQuality)
    }

    var body: some View {
        if let invitation {
            InviteAddPane(found: invitation) { dismiss() }
        } else {
            joinForm
        }
    }

    private var joinForm: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(ShareText.joinTitle).font(Theme.display(30)).foregroundStyle(Theme.text)
                    Text(ShareText.joinHelp).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                }
                Spacer()
                Button { close() } label: { Icon("x", size: 15, bold: true) }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
                    .accessibilityLabel(StationText.cancel)
            }
            codeField
            LabeledField(label: ShareText.yourName, text: $model.name)
                .onSubmit(join)
            qualityChoice
            status
            HStack {
                Spacer()
                if model.busy {
                    Button(StationText.cancel) { model.cancel() }
                        .buttonStyle(GhostButton(height: 44))
                }
                Button(ShareText.joinButton, action: join)
                    .buttonStyle(PrimaryButton(lit: model.busy))
                    .disabled(!model.canJoin)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(.horizontal, 36).padding(.vertical, 32)
        .frame(width: 640)
        .background {
            ZStack {
                Theme.background
                RadialGradient(colors: [Theme.accentFill.opacity(0.22), .clear], center: .topTrailing, startRadius: 0, endRadius: 520)
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            // Le rappel ne retient que l'app : en capturant la vue, qui tient
            // `model` (StateObject), le modèle se retiendrait lui-même.
            // Quatre ordinateurs au plus, invités compris : seul ce qui ouvre une
            // session (un partage, un prêt) est refusé, après coup, avec le texte
            // de la limite (l'invité repart tout de suite) ; une invitation n'ouvre
            // pas de session (docs/plusieurs-ordinateurs.md).
            model.onJoined = { [app, weak model = model] found in
                guard !app.sessions.isFull else {
                    Share.leave(found)
                    model?.refuse(LinkText.computersLimit)
                    return
                }
                app.openGuestSession(found)
            }
            // Un code de prêt : la session d'emprunt prend la suite.
            model.onLend = { [app, weak model = model] source in
                guard !app.sessions.isFull else {
                    model?.refuse(LinkText.computersLimit)
                    return
                }
                app.openBorrowSession(source)
            }
            // Un code d'invitation : son mot de passe, dans cette feuille (le rappel ne retient que l'état).
            model.onInvite = { [invitation = $invitation] found in invitation.wrappedValue = found }
            model.start()
            codeFocused = true
        }
        .onDisappear {
            model.cancel()
            model.stop()
            model.onJoined = nil
            model.onLend = nil
            model.onInvite = nil
        }
        .onChange(of: model.code) { _, _ in model.codeChanged() }
    }

    /// Les six chiffres, en grand, groupés par trois.
    private var codeField: some View {
        TextField(text: $model.code, prompt: Text(verbatim: "123 456").foregroundColor(Theme.ghost)) { EmptyView() }
            .textFieldStyle(.plain)
            .font(Theme.mono(34, .medium))
            .tracking(4)
            .foregroundStyle(Theme.accentLight)
            .multilineTextAlignment(.center)
            .focused($codeFocused)
            .onSubmit(join)
            .padding(.horizontal, 14)
            .frame(height: 78)
            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).stroke(codeFocused ? Theme.accentLine : Theme.line2))
            .disabled(model.busy)
            .accessibilityLabel(ShareText.code)
    }

    @ViewBuilder private var status: some View {
        switch model.state {
        case .idle:
            EmptyView()
        case .looking:
            HStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text(ShareText.looking).font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
        case .waiting:
            // Hors de chez l'hôte : sa réponse se fait attendre (« Annuler » retire la demande).
            HStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text(ShareText.waiting).font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
            .accessibilityElement(children: .combine)
        case let .failed(text):
            Notice(kind: .warn, text: text)
        }
    }

    private func join() {
        guard model.canJoin else { return }
        model.join(hosts: app.store.hosts)
    }

    private func close() {
        model.cancel()
        dismiss()
    }
}

// MARK: - Invitation

/// « Ajouter {ordinateur} » (docs/link-ui.md, « Invitations ») : « Rejoindre »
/// est tombé sur un code d'invitation. Le mot de passe donné avec le code
/// (« xxxx-xxxx-xxxx », tapé comme on veut), « Ajouter l'ordinateur » ;
/// pendant l'appel « Ajout de {ordinateur}… », un refus sous le champ. Ajouté,
/// l'ordinateur est sélectionné sur l'accueil : « {ordinateur} est ajouté. »
struct InviteAddPane: View {
    @EnvironmentObject var app: AppModel
    @StateObject private var model: InviteAddModel
    @FocusState private var focused: Bool
    /// Le mot de passe en clair (« Afficher ») ; masqué par défaut.
    @State private var shown = false
    let close: () -> Void

    init(found: InviteCodeFound, close: @escaping () -> Void) {
        _model = StateObject(wrappedValue: InviteAddModel(found: found))
        self.close = close
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(model.title).font(Theme.display(30)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(InviteText.help).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                Button { cancel() } label: { Icon("x", size: 15, bold: true) }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
                    .accessibilityLabel(StationText.cancel)
            }
            VStack(alignment: .leading, spacing: 8) {
                Text(InviteText.password).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
                // Masqué (des points), « Afficher » à droite ; ni correction ni
                // majuscule automatiques : tirets et espaces facultatifs.
                HStack(spacing: 10) {
                    Group {
                        if shown {
                            TextField(text: $model.password, prompt: Text(verbatim: InviteText.placeholder).foregroundColor(Theme.ghost)) { EmptyView() }
                        } else {
                            SecureField(text: $model.password, prompt: Text(verbatim: InviteText.placeholder).foregroundColor(Theme.ghost)) { EmptyView() }
                        }
                    }
                    .textFieldStyle(.plain)
                    .autocorrectionDisabled()
                    .font(Theme.mono(17))
                    .foregroundStyle(Theme.text)
                    .focused($focused)
                    .onSubmit(add)
                    .accessibilityLabel(InviteText.password)
                    Button(shown ? InviteText.hide : InviteText.show) {
                        shown.toggle()
                        focused = true
                    }
                    .buttonStyle(.plain)
                    .font(Theme.sans(13, .semibold))
                    .foregroundStyle(Theme.accentLight)
                }
                .padding(.leading, 14).padding(.trailing, 12)
                .frame(height: 46)
                .background(Theme.bar, in: RoundedRectangle(cornerRadius: 11))
                .overlay(RoundedRectangle(cornerRadius: 11).stroke(focused ? Theme.accentLine : Theme.line2))
                .disabled(model.adding)
                if let failure = model.failure {
                    Notice(kind: .warn, text: failure)
                }
            }
            if model.adding {
                HStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text(InviteText.adding(model.computer)).font(Theme.sans(13)).foregroundStyle(Theme.muted)
                }
                .accessibilityElement(children: .combine)
            }
            HStack {
                Spacer()
                Button(StationText.cancel) { cancel() }
                    .buttonStyle(GhostButton(height: 44))
                Button(InviteText.add, action: add)
                    .buttonStyle(PrimaryButton(lit: model.adding))
                    .disabled(!model.canAdd)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(.horizontal, 36).padding(.vertical, 32)
        .frame(width: 640)
        .background {
            ZStack {
                Theme.background
                RadialGradient(colors: [Theme.accentFill.opacity(0.22), .clear], center: .topTrailing, startRadius: 0, endRadius: 520)
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            // Le rappel ne retient que l'app (voir `JoinSheet`).
            model.onAdded = { [app] host in
                app.joining = false
                app.route = .home
                app.selectedID = host.id
                app.show(InviteText.added(host.name))
            }
            focused = true
        }
        .onDisappear {
            model.cancel()
            model.onAdded = nil
        }
        .onChange(of: model.password) { _, _ in model.passwordChanged() }
    }

    private func add() {
        guard model.canAdd else { return }
        model.add(to: app.store)
    }

    private func cancel() {
        model.cancel()
        close()
    }
}
