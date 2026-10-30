// Prêter l'ordinateur (KyberNow Share) sur l'Apple TV (docs/pret.md ; textes :
// docs/link-ui.md, « Prêt de l'ordinateur »). L'hôte : la tuile « Prêter
// l'ordinateur… » du menu de session ouvre ce panneau, en boutons : les
// demandes (« Accepter », « Refuser »), la durée et le compte Steam, « Créer
// un code », puis le code et son QR code (pas de « Copier le lien » : l'Apple
// TV n'a pas de presse-papiers utile) et « Annuler le code ». L'ami : le
// compte d'ami (identifiant et mot de passe au clavier tvOS ou de l'iPhone,
// comme l'appairage), « Rendre l'ordinateur » confirmé, l'accueil de
// l'ordinateur prêté. Les appels et les règles sont dans Lend.swift, commun
// aux apps d'Apple.

import KyberKit
import SwiftUI

// MARK: - Hôte : le panneau du prêt

struct LendPanelTV: View {
    let computer: String
    @ObservedObject var lend: LendHostModel
    let close: () -> Void
    @FocusState private var focus: Target?

    enum Target: Hashable { case duration(Int), steam(Int), create, cancelCode, close, accept(String), decline(String) }

    var body: some View {
        ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 30) {
                Theme.eyebrow(LendText.title(computer))
                if let reason = lend.availability.reason {
                    Text(reason).font(Theme.sans(30)).foregroundStyle(Theme.warnText).frame(width: 1000, alignment: .leading)
                    closeButton
                } else {
                    if lend.showWarning {
                        HStack(alignment: .top, spacing: 16) {
                            Icon("warning", size: 30).foregroundStyle(Theme.warn)
                            Text(LendText.warning).font(Theme.sans(21)).foregroundStyle(Theme.warnText).lineSpacing(4)
                        }
                        .frame(width: 1300, alignment: .leading)
                    }
                    if !lend.requests.isEmpty { requests }
                    if let code = lend.code { created(code) } else { choices }
                    if let error = lend.error { Text(error).font(Theme.sans(22)).foregroundStyle(Theme.dangerText) }
                }
            }
            .padding(56)
            .glass(30, tint: 0.6)
            .focusSection()
        }
        .frame(width: 1920, height: 1080)
        .onExitCommand(perform: close)
        .task { await lend.load() }
        .onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { focus = lend.code == nil ? .create : .close } }
        // L'avertissement se montre une fois.
        .onDisappear { if lend.availability.isAvailable { lend.warningSeen() } }
    }

    private var closeButton: some View {
        Button("Fermer", action: close)
            .buttonStyle(ChoiceButton(selected: false, height: 72))
            .focused($focus, equals: .close)
    }

    /// Les demandes (ordinateur utilisé) : « Léa veut emprunter l'ordinateur. »,
    /// son app, « Accepter » et « Refuser » sur la même ligne ; un créneau qui
    /// commence se dit, sans réponse à donner.
    @ViewBuilder private var requests: some View {
        VStack(alignment: .leading, spacing: 18) {
            Theme.eyebrow(ShareText.requests)
            ForEach(lend.requests) { request in
                HStack(spacing: 18) {
                    Icon(request.viaRelay ? "globe-hemisphere-west" : "user", size: 32).foregroundStyle(Theme.accent).frame(width: 40)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(request.slot ? LendText.slotStarts(request.name) : LendText.wantsToBorrow(request.name))
                            .font(Theme.sans(25, .semibold)).foregroundStyle(Theme.text).lineLimit(2)
                        if let detail = LendText.requestDetail(request) {
                            Text(detail).font(Theme.sans(18)).foregroundStyle(Theme.faint).lineLimit(1)
                        }
                    }
                    Spacer(minLength: 0)
                    if !request.slot {
                        Button { Task { await lend.answer(request, accept: true) } } label: { Text(ShareText.accept).lineLimit(1).fixedSize() }
                            .buttonStyle(ChoiceButton(selected: true, height: 56, light: true))
                            .focused($focus, equals: .accept(request.id))
                        Button { Task { await lend.answer(request, accept: false) } } label: { Text(ShareText.decline).lineLimit(1).fixedSize() }
                            .buttonStyle(ChoiceButton(selected: false, height: 56, light: true))
                            .focused($focus, equals: .decline(request.id))
                    }
                }
                .frame(width: 1300)
                .disabled(lend.busy)
            }
            Text(LendText.acceptCloses).font(Theme.sans(19)).foregroundStyle(Theme.faint)
        }
    }

    /// Avant le code : la durée (« 2 heures » proposé) et le compte Steam, puis « Créer un code ».
    @ViewBuilder private var choices: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(LendText.duration).font(Theme.sans(21)).foregroundStyle(Theme.muted)
            HStack(spacing: 16) {
                ForEach(LendDuration.allCases) { d in
                    Button { lend.duration = d } label: { Text(d.label).lineLimit(1).fixedSize() }
                        .buttonStyle(ChoiceButton(selected: lend.duration == d, height: 64))
                        .focused($focus, equals: .duration(d.rawValue))
                }
            }
        }
        VStack(alignment: .leading, spacing: 14) {
            Text(LendText.steamAccount).font(Theme.sans(21)).foregroundStyle(Theme.muted)
            HStack(spacing: 16) {
                ForEach(LendSteam.allCases, id: \.self) { s in
                    Button { lend.steam = s } label: { Text(s.label).lineLimit(1).fixedSize() }
                        .buttonStyle(ChoiceButton(selected: lend.steam == s, height: 64))
                        .focused($focus, equals: .steam(s.rawValue))
                }
            }
        }
        HStack(spacing: 20) {
            Button(LendText.createCode) { Task { await lend.createCode() } }
                .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                .focused($focus, equals: .create)
                .disabled(lend.busy)
            closeButton
        }
    }

    /// Le code créé : QR code, « 123 456 », « Pour une séance, valable 24 h. »,
    /// l'aide, et « Votre session se fermera quand votre ami arrivera. ».
    @ViewBuilder private func created(_ code: LendCodeView) -> some View {
        HStack(alignment: .top, spacing: 64) {
            LendQRTV(link: code.link)
            VStack(alignment: .leading, spacing: 20) {
                Theme.eyebrow(ShareText.code)
                Text(verbatim: Share.formatCode(code.code))
                    .font(Theme.mono(68, .medium)).tracking(2)
                    .foregroundStyle(Theme.neonCore)
                    .shadow(color: Theme.accent, radius: 14)
                    .lineLimit(1)
                    .fixedSize()
                    .accessibilityLabel(Text(verbatim: "\(ShareText.code) \(code.code.map(String.init).joined(separator: " "))"))
                Text(LendText.oneSession).font(Theme.sans(21)).foregroundStyle(Theme.muted)
                Text((code.duration.flatMap { LendDuration(rawValue: $0) } ?? .untilTakenBack).label + " · " + code.steam.label)
                    .font(Theme.sans(21)).foregroundStyle(Theme.text2)
                Text(LendText.help).font(Theme.sans(20)).foregroundStyle(Theme.muted).lineSpacing(4)
                    .frame(width: 760, alignment: .leading)
                Text(LendText.sessionCloses).font(Theme.sans(20)).foregroundStyle(Theme.faint).lineSpacing(4)
                    .frame(width: 760, alignment: .leading)
            }
        }
        HStack(spacing: 20) {
            Button(LendText.cancelCode) { Task { await lend.cancelCode() } }
                .buttonStyle(ChoiceButton(selected: false, height: 72))
                .focused($focus, equals: .cancelCode)
                .disabled(lend.busy)
            closeButton
        }
    }
}

/// Le QR code du lien du code de prêt (`kybernow://join…`), net, sur fond blanc.
private struct LendQRTV: View {
    let link: String

    var body: some View {
        if let image = Share.qrImage(for: link) {
            Image(decorative: image, scale: 1)
                .interpolation(.none)
                .resizable()
                .aspectRatio(1, contentMode: .fit)
                .frame(width: 280, height: 280)
                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                .accessibilityElement()
                .accessibilityLabel(ShareText.qrLabel)
        } else {
            RoundedRectangle(cornerRadius: 14, style: .continuous).fill(Color.white.opacity(0.08)).frame(width: 280, height: 280)
        }
    }
}

// MARK: - Ami : rendre l'ordinateur

/// « Rendre l'ordinateur à {hôte} ? », « Rendre l'ordinateur » ou « Annuler ».
struct GiveBackPanel: View {
    let host: String
    let confirm: () -> Void
    let cancel: () -> Void
    @FocusState private var focus: Int?

    var body: some View {
        ZStack {
            Color.black.opacity(0.55)
            VStack(alignment: .leading, spacing: 34) {
                Text(LendText.giveBackQuestion(host)).font(Theme.display(52, 640)).tracking(-1.4).foregroundStyle(Theme.text)
                HStack(spacing: 20) {
                    Button(LendText.giveBack, action: confirm)
                        .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                    Button(StationText.cancel, action: cancel)
                        .buttonStyle(ChoiceButton(selected: false, height: 72))
                        .focused($focus, equals: 1)
                }
            }
            .padding(56)
            .glass(30, tint: 0.6)
            .focusSection()
        }
        .frame(width: 1920, height: 1080)
        .onExitCommand(perform: cancel)
        // Le focus va d'abord à « Annuler » : un clic de trop ne rend pas l'ordinateur.
        .onAppear { DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { focus = 1 } }
    }
}

// MARK: - Ami : le compte d'ami

/// L'identifiant, le mot de passe et « Retenir le mot de passe », au clavier
/// tvOS ou de l'iPhone, comme l'appairage. `invite` : l'ordinateur du lien
/// d'invitation (ou choisi sur le réseau) ; « Ajouter » vérifie le compte
/// auprès de la Station. `computer` : un ordinateur déjà ajouté dont le mot de
/// passe manque (pas retenu, ou refusé), demandé à « Emprunter ».
struct FriendAccountForm: View {
    @EnvironmentObject private var app: AppModel
    var invite: LendInvite?
    var computer: BorrowedComputer?
    let cancel: () -> Void
    @State private var login = ""
    @State private var password = ""
    @State private var remember = true
    @State private var working = false
    @State private var error: String?

    private var valid: Bool { (computer != nil || !login.trimmed.isEmpty) && !password.isEmpty && !working }

    var body: some View {
        VStack(alignment: .leading, spacing: 26) {
            Text(title).font(Theme.display(48, 640)).tracking(-48 * 0.03).lineLimit(2)
            if let address = invite?.host ?? computer?.invite.host {
                Text(verbatim: address).font(Theme.mono(19)).foregroundStyle(Theme.muted)
            }
            if computer == nil {
                field(LendText.username) {
                    TextField(LendText.username, text: $login)
                        .textContentType(.username)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                }
            } else if let computer {
                Text(verbatim: computer.login).font(Theme.sans(25)).foregroundStyle(Theme.text2)
            }
            field(LendText.password) {
                SecureField(LendText.password, text: $password)
                    .textContentType(.password)
            }
            Button { remember.toggle() } label: {
                HStack(spacing: 16) {
                    Toggle2(on: remember)
                    Text(LendText.rememberPassword).font(Theme.sans(22)).foregroundStyle(Theme.text2)
                }
                .padding(10)
            }
            .buttonStyle(GlassButton(radius: 16, scale: 1.03, fill: Color.clear))
            if let error {
                Text(error).font(Theme.sans(21)).foregroundStyle(Theme.dangerText)
            }
            HStack(spacing: 20) {
                Button(action: submit) { working ? Text("Vérification…") : Text(computer == nil ? LendText.add : LendText.borrow) }
                    .buttonStyle(PrimaryButton(height: 72, fontSize: 25))
                    .disabled(!valid)
                Button(StationText.cancel, action: cancel)
                    .buttonStyle(ChoiceButton(selected: false, height: 72, light: true))
            }
        }
        .onAppear {
            if let invite { login = invite.login }
            if let computer { remember = computer.rememberPassword }
        }
    }

    /// « {hôte} vous prête son ordinateur. » quand on sait déjà qui ; sinon « Avec un compte d'ami ».
    private var title: String {
        if let computer { return computer.title }
        if let invite, let known = app.borrowed.existing(for: invite) { return LendText.invitation(known.hostName) }
        return LendText.withFriendAccount
    }

    private func field<F: View>(_ label: String, @ViewBuilder _ content: () -> F) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(label).font(Theme.sans(19)).foregroundStyle(Theme.muted)
            content()
                .font(Theme.sans(27))
                .frame(width: 760)
        }
    }

    private func submit() {
        guard valid else { return }
        if let computer {
            app.borrowed.updatePassword(password, remember: remember, for: computer)
            app.screen = .home
            app.borrow(computer)
            return
        }
        guard let invite else { return }
        working = true
        error = nil
        let target = invite.with(login: login.trimmed), password = password, remember = remember
        Task {
            defer { working = false }
            do {
                let added = try await app.borrowed.add(invite: target, password: password, remember: remember)
                app.selected = added.id
                app.screen = .home
            } catch {
                self.error = BorrowFlow.text(error)
            }
        }
    }
}

/// Un écran à part pour le compte d'ami : le lien d'invitation
/// (`kybernow-tv://borrow?…`), ou le mot de passe d'un ordinateur prêté.
struct FriendAccountView: View {
    @EnvironmentObject private var app: AppModel
    var invite: LendInvite?
    var computer: BorrowedComputer?

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            FriendAccountForm(invite: invite, computer: computer) { app.screen = .home }
                .padding(.horizontal, 60).padding(.vertical, 56)
                .frame(width: 1100, alignment: .topLeading)
                .glass(30)
                .focusSection()
                .padding(.leading, 96).padding(.top, 150)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .onExitCommand { app.screen = .home }
    }
}

// MARK: - Ami : l'accueil

/// Ce que l'accueil dit d'un ordinateur prêté par un ami : l'état (pastille),
/// « Ordinateur de {hôte} », le nom de l'ordinateur, le créneau, un refus.
struct BorrowedHeroText: View {
    let computer: BorrowedComputer
    let state: BorrowedState

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(spacing: 12) {
                Circle().fill(state.availability == .free || state.comesBack ? Theme.ok : Color.clear)
                    .overlay(Circle().strokeBorder(state.availability == .free || state.comesBack ? Color.clear : Theme.faint, lineWidth: 1.5))
                    .frame(width: 10, height: 10)
                Text(state.label ?? LinkText.status(nil)).font(Theme.sans(21))
            }
            .padding(.leading, 16).padding(.trailing, 18)
            .frame(height: 44)
            .glass(22)
            Text(computer.title)
                .font(Theme.display(120, 680))
                .tracking(-120 * 0.045)
                .lineLimit(1)
                .minimumScaleFactor(0.5)
                .foregroundStyle(Theme.text)
                .padding(.vertical, -16)
                .frame(maxWidth: 1080, alignment: .leading)
            if !computer.computer.isEmpty {
                Text(verbatim: computer.computer).font(Theme.sans(24)).foregroundStyle(Theme.muted)
            }
            if case let .status(s) = state, let slot = s.slot.flatMap(LendText.slot) {
                HStack(spacing: 14) {
                    Icon("key", size: 26).foregroundStyle(Theme.accent)
                    Text(slot).font(Theme.sans(24)).foregroundStyle(Theme.text2)
                }
            }
            if case let .refused(text) = state {
                HStack(spacing: 12) {
                    Icon("warning", size: 26).foregroundStyle(Theme.warn)
                    Text(text).font(Theme.sans(22)).foregroundStyle(Theme.warnText)
                }
            }
        }
    }
}

extension BorrowedState {
    /// Le bouton principal : « Emprunter », « Demander à {hôte} » (ordinateur
    /// utilisé), « Revenir » (le prêt en cours est le sien), « Réessayer » (hors ligne).
    func primaryLabel(host: String) -> String {
        if self == .offline { return String(localized: "Réessayer") }
        if comesBack { return LendText.comeBack }
        if availability == .busy { return LendText.ask(host) }
        return LendText.borrow
    }

    /// Le bouton principal peut servir (pas pour « Verrouillé », « Prêt coupé »…).
    var primaryEnabled: Bool {
        switch self {
        case .offline, .checking, .refused: return true
        case .status: return canBorrow
        }
    }
}
