// Prêter l'ordinateur (KyberNow Share) sur Mac (docs/pret.md ; textes :
// docs/link-ui.md, « Prêt de l'ordinateur ») : côté hôte, « Prêter
// l'ordinateur… » dans la barre de session et son panneau (durée, compte
// Steam, « Créer un code », code, lien, QR code, « Annuler le code », les
// demandes) ; côté ami, l'ajout d'un ordinateur prêté (lien d'invitation ou
// « Avec un compte d'ami »), sa carte et son accueil. Les règles et les
// appels sont dans Lend.swift, communs aux apps d'Apple.

import AppKit
import KyberKit
import SwiftUI

// MARK: - Hôte : la barre et le panneau

/// « Prêter l'ordinateur… » dans la barre : un bouton et son panneau, comme « Partager ».
struct LendBarButton: View {
    @ObservedObject var session: SessionController
    @ObservedObject var lend: LendHostModel
    var compact = false
    @State private var open = false
    @State private var hovering = false

    private var availability: LendHostAvailability { lend.availability }
    private var requests: Int { lend.requests.filter { !$0.slot }.count }

    var body: some View {
        Button {
            guard availability.isAvailable else { return }
            open.toggle()
        } label: {
            HStack(spacing: 7) {
                Icon("key", size: 16, bold: true).foregroundStyle(lend.code != nil ? Theme.accent : Theme.text2)
                if !compact {
                    Text(LendText.lend).font(Theme.sans(13)).foregroundStyle(Theme.text2).lineLimit(1).fixedSize()
                }
                if requests > 0 {
                    Text(compact ? "\(requests)" : ShareText.requestCount(requests))
                        .font(Theme.sans(11.5, .semibold)).foregroundStyle(Theme.text).lineLimit(1).fixedSize()
                        .padding(.horizontal, 8).frame(height: 20)
                        .background(Theme.accentFill, in: Capsule())
                }
            }
            .padding(.horizontal, compact ? 8 : 10)
            .frame(height: 34)
            .background(lend.code != nil ? Theme.accentSoft : Color.white.opacity(hovering || open ? 0.07 : 0), in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.accentLine.opacity(lend.code != nil ? 1 : 0)))
            .opacity(availability.isAvailable ? 1 : 0.45)
            .contentShape(Rectangle())
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text([LendText.lend, requests > 0 ? ShareText.requestCount(requests) : nil].compactMap { $0 }.joined(separator: ", ")))
        }
        .buttonStyle(.plain)
        .disabled(!availability.isAvailable)
        .onHover { hovering = $0 }
        // Inactif : la raison (Station trop ancienne, prêt coupé dans Admin).
        .help(availability.reason ?? "")
        .preference(key: GlassMenuOpenKey.self, value: open)
        .popover(isPresented: $open, arrowEdge: .bottom) {
            LendPanel(computer: session.host.name, lend: lend)
                .preferredColorScheme(.dark)
        }
        .onChange(of: availability) { _, now in
            if !now.isAvailable { open = false }
        }
        // L'avis d'une demande (« Léa veut emprunter l'ordinateur. ») choisi : la barre s'est ouverte, le panneau avec elle.
        .onAppear { openForRequest() }
        .onChange(of: session.lendPanelRequests) { _, _ in openForRequest() }
    }

    private func openForRequest() {
        guard session.takeLendPanelRequest(), availability.isAvailable else { return }
        open = true
    }
}

/// Le panneau de l'hôte : les demandes, puis la durée, le compte Steam et
/// « Créer un code » ; le code créé, comme le partage (code, QR code, « Copier
/// le lien »), et « Annuler le code ».
struct LendPanel: View {
    let computer: String
    @ObservedObject var lend: LendHostModel
    @State private var copyResult: String?
    @State private var copied = false

    static let width: CGFloat = 470

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(LendText.title(computer))
                .font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text).lineLimit(2)
            if let reason = lend.availability.reason {
                Notice(kind: .info, text: reason)
            } else {
                if lend.showWarning { Notice(kind: .warn, text: LendText.warning) }
                if !lend.requests.isEmpty { requests }
                if let code = lend.code { created(code) } else { choices }
                if let error = lend.error { Notice(kind: .danger, text: error) }
            }
        }
        .padding(20)
        .frame(width: Self.width, alignment: .leading)
        .task { await lend.load() }
        // L'avertissement se montre une fois.
        .onDisappear { if lend.availability.isAvailable { lend.warningSeen() } }
    }

    @ViewBuilder private var requests: some View {
        VStack(alignment: .leading, spacing: 10) {
            Theme.eyebrow(ShareText.requests, color: Theme.faint)
            ForEach(lend.requests) { request in
                VStack(alignment: .leading, spacing: 6) {
                    Text(request.slot ? LendText.slotStarts(request.name) : LendText.wantsToBorrow(request.name))
                        .font(Theme.sans(13.5, .semibold)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                    if let detail = LendText.requestDetail(request) {
                        Text(detail).font(Theme.sans(12)).foregroundStyle(Theme.muted)
                    }
                    if !request.slot {
                        HStack(spacing: 8) {
                            Button(ShareText.accept) { Task { await lend.answer(request, accept: true) } }
                                .buttonStyle(PrimaryButton(height: 32))
                            Button(ShareText.decline) { Task { await lend.answer(request, accept: false) } }
                                .buttonStyle(GhostButton(height: 32))
                        }
                        .disabled(lend.busy)
                    }
                }
            }
            Text(LendText.acceptCloses).font(Theme.sans(12)).foregroundStyle(Theme.faint)
        }
        Rectangle().fill(Theme.line).frame(height: 1)
    }

    @ViewBuilder private var choices: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(LendText.duration).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
            Segmented(options: LendDuration.allCases.map { ($0, $0.label) },
                      selection: Binding(get: { lend.duration }, set: { lend.duration = $0 ?? lend.duration }), height: 32, fill: true)
        }
        VStack(alignment: .leading, spacing: 8) {
            Text(LendText.steamAccount).font(Theme.sans(13, .semibold)).foregroundStyle(Theme.text2)
            Segmented(options: LendSteam.allCases.map { ($0, $0.label) },
                      selection: Binding(get: { lend.steam }, set: { lend.steam = $0 ?? lend.steam }), height: 32, fill: true)
        }
        HStack {
            Spacer()
            Button(LendText.createCode) { Task { await lend.createCode() } }
                .buttonStyle(PrimaryButton(height: 38))
                .disabled(lend.busy)
        }
    }

    @ViewBuilder private func created(_ code: LendCodeView) -> some View {
        HStack(alignment: .top, spacing: 16) {
            LinkQR(link: code.link, waiting: lend.waitingForRelayLink)
            VStack(alignment: .leading, spacing: 8) {
                Theme.eyebrow(ShareText.code, color: Theme.faint)
                Text(Share.formatCode(code.code))
                    .font(Theme.mono(24, .medium)).foregroundStyle(Theme.accentLight)
                    .fixedSize()
                    .textSelection(.enabled)
                Text(LendText.oneSession).font(Theme.sans(12)).foregroundStyle(Theme.faint)
                    .fixedSize(horizontal: false, vertical: true)
                Button { copyLink() } label: {
                    HStack(spacing: 8) {
                        Icon(copied ? "check" : "copy", size: 14, bold: true)
                        Text(ShareText.copyLink)
                    }
                    .font(Theme.sans(13))
                }
                .buttonStyle(GhostButton(height: 32))
                .disabled(lend.waitingForRelayLink)
                if let copyResult {
                    Text(copyResult).font(Theme.sans(12)).foregroundStyle(copied ? Theme.ok : Theme.warnText)
                        .fixedSize(horizontal: false, vertical: true)
                    if !copied, let link = lend.linkToCopy {
                        Text(verbatim: link).font(Theme.mono(11)).foregroundStyle(Theme.text2)
                            .textSelection(.enabled)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
        // Comme le partage : le lien du relais en préparation (10 s au plus), ou pas encore joignable.
        VStack(alignment: .leading, spacing: 6) {
            Text(LendText.help).font(Theme.sans(12.5)).foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
            RelayLinkNote(relay: lend.relay)
            Text(LendText.sessionCloses).font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
                .fixedSize(horizontal: false, vertical: true)
        }
        HStack {
            Text(code.duration.flatMap { LendDuration(rawValue: $0)?.label } ?? LendDuration.untilTakenBack.label)
                .font(Theme.sans(12)).foregroundStyle(Theme.muted)
            Text(verbatim: "·").foregroundStyle(Theme.faint)
            Text(code.steam.label).font(Theme.sans(12)).foregroundStyle(Theme.muted)
            Spacer(minLength: 0)
            Button(LendText.cancelCode) { Task { await lend.cancelCode() } }
                .buttonStyle(GhostButton(height: 34, tint: Theme.dangerText, border: Theme.danger.opacity(0.4)))
                .disabled(lend.busy)
        }
    }

    private func copyLink() {
        guard let text = lend.linkToCopy else { return }
        let board = NSPasteboard.general
        board.clearContents()
        let ok = board.setString(text, forType: .string)
        copied = ok
        copyResult = ok ? ShareText.linkCopied : ShareText.copyFailed
        guard ok else { return }
        Task {
            try? await Task.sleep(for: .seconds(3))
            guard copied else { return }
            copyResult = nil
            copied = false
        }
    }
}

// MARK: - Ami : ajouter un ordinateur prêté

/// « Avec un compte d'ami » : l'ordinateur (celui du lien d'invitation, ou un
/// ordinateur du réseau), l'identifiant, le mot de passe et « Retenir le mot
/// de passe ». « Ajouter » vérifie le compte auprès de la Station.
struct FriendAccountForm: View {
    @EnvironmentObject var app: AppModel
    /// Le lien d'invitation, s'il y en a un ; sinon un ordinateur du réseau à choisir.
    let invite: LendInvite?
    /// Ajouté : l'accueil le met en avant.
    let added: (BorrowedComputer) -> Void
    @State private var login = ""
    @State private var password = ""
    @State private var remember = true
    @State private var picked: Candidate?
    @State private var working = false
    @State private var error: String?
    @StateObject private var discovery = Discovery()
    @State private var resolved: [String: Candidate] = [:]

    private var target: LendInvite? {
        if let invite { return invite.with(login: login.trimmed) }
        guard let picked else { return nil }
        return LendInvite(host: picked.address, port: picked.port, fingerprint: picked.expected, login: login.trimmed)
    }
    private var valid: Bool { target != nil && !login.trimmed.isEmpty && !password.isEmpty && !working }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            if invite == nil { network }
            HStack(alignment: .top, spacing: 14) {
                LabeledField(label: LendText.username, text: $login)
                LabeledField(label: LendText.password, text: $password, secure: true)
                    .onSubmit(add)
            }
            Toggle(isOn: $remember) { Text(LendText.rememberPassword).font(Theme.sans(14)).foregroundStyle(Theme.text2) }
                .toggleStyle(.checkbox)
            if let error { Notice(kind: .warn, text: error) }
            Spacer(minLength: 0)
            HStack {
                Spacer()
                if working { ProgressView().controlSize(.small) }
                Button(LendText.add, action: add)
                    .buttonStyle(PrimaryButton())
                    .disabled(!valid)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .onAppear {
            if let invite { login = invite.login } else { discovery.start() }
        }
        .onDisappear { discovery.stop() }
        .task(id: discovery.found.map(\.id)) {
            for f in discovery.found where resolved[f.id] == nil {
                if let c = await Discovery.resolve(f) { resolved[f.id] = c }
            }
        }
    }

    /// Les ordinateurs annoncés sur le réseau : on choisit celui de l'ami.
    private var network: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Chaque ordinateur équipé de KyberNow Station s'annonce sur le réseau local (Bonjour).")
                    .font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
            ForEach(discovery.found.filter { resolved[$0.id] != nil }) { f in
                let c = resolved[f.id]!
                let on = picked == c
                Button { picked = c } label: {
                    HStack(spacing: 12) {
                        Icon(on ? "check-circle" : "desktop-tower", size: 18).foregroundStyle(on ? Theme.accent : Theme.muted)
                        Text(f.name).font(Theme.sans(15, .semibold)).foregroundStyle(Theme.text)
                        Spacer()
                        Text(verbatim: c.address).font(Theme.mono(12)).foregroundStyle(Theme.faint)
                    }
                    .padding(.horizontal, 16).frame(height: 48)
                    .background(on ? Theme.accentSoft : Theme.well, in: RoundedRectangle(cornerRadius: 12))
                    .overlay(RoundedRectangle(cornerRadius: 12).stroke(on ? Theme.accentLine : Theme.line))
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func add() {
        guard valid, let target else { return }
        working = true
        error = nil
        let password = password, remember = remember
        Task {
            defer { working = false }
            do {
                let computer = try await app.borrowed.add(invite: target, password: password, remember: remember)
                added(computer)
            } catch {
                self.error = BorrowFlow.text(error)
            }
        }
    }
}


/// Le lien d'invitation ouvert (`kybernow://borrow?…`) : « {hôte} vous prête
/// son ordinateur. » quand on sait qui (le prénom du lien, ou l'ordinateur
/// déjà ajouté), sinon « Avec un compte d'ami » ; l'identifiant rempli, le mot
/// de passe à taper, « Ajouter ».
struct InviteSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let invite: LendInvite

    private var known: BorrowedComputer? { app.borrowed.existing(for: invite) }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(LendText.invitationTitle(invite.name ?? known?.hostName))
                        .font(Theme.display(30)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(verbatim: invite.host).font(Theme.mono(13)).foregroundStyle(Theme.faint)
                }
                Spacer()
                Button { dismiss() } label: { Icon("x", size: 15, bold: true) }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
                    .accessibilityLabel(StationText.cancel)
            }
            FriendAccountForm(invite: invite) { computer in
                dismiss()
                app.selectedID = computer.id
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
    }
}

/// Le mot de passe d'un ordinateur prêté, quand il n'est pas retenu (ou refusé) :
/// demandé à « Emprunter ».
struct BorrowPasswordSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let computer: BorrowedComputer
    @State private var password = ""
    @State private var remember: Bool

    init(computer: BorrowedComputer) {
        self.computer = computer
        _remember = State(initialValue: computer.rememberPassword)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            VStack(alignment: .leading, spacing: 6) {
                Text(computer.title).font(Theme.display(26)).foregroundStyle(Theme.text)
                Text(verbatim: computer.login).font(Theme.mono(13)).foregroundStyle(Theme.faint)
            }
            Text(LendText.enterPassword).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                .fixedSize(horizontal: false, vertical: true)
            LabeledField(label: LendText.password, text: $password, secure: true)
                .onSubmit(borrow)
            Toggle(isOn: $remember) { Text(LendText.rememberPassword).font(Theme.sans(14)).foregroundStyle(Theme.text2) }
                .toggleStyle(.checkbox)
            HStack {
                Spacer()
                Button(StationText.cancel) { dismiss() }
                    .buttonStyle(GhostButton(height: 44))
                    .keyboardShortcut(.cancelAction)
                Button(LendText.borrow, action: borrow)
                    .buttonStyle(PrimaryButton())
                    .disabled(password.isEmpty)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(32)
        .frame(width: 480)
        .background(Theme.background)
        .preferredColorScheme(.dark)
    }

    private func borrow() {
        guard !password.isEmpty else { return }
        app.borrowed.updatePassword(password, remember: remember, for: computer)
        dismiss()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { app.borrow(computer) }
    }
}

// MARK: - Ami : l'accueil

/// L'ordinateur d'un ami, mis en avant sur l'accueil (docs/link-ui.md) : son
/// état, « Ordinateur de {hôte} », le créneau, « Emprunter » (« Demander à
/// {hôte} » quand il est utilisé, « Revenir » quand le prêt en cours est le sien).
struct BorrowedHero: View {
    @EnvironmentObject var app: AppModel
    let computer: BorrowedComputer
    let arrived: Bool
    let size: CGSize

    private var state: BorrowedState { app.borrowed.states[computer.id] ?? .checking }
    private var nameSize: CGFloat { min(96, size.width * 0.066, size.height * 0.09) }
    private func rise(_ i: Int) -> some ViewModifier { Rise(arrived: arrived, delay: 0.15 + Double(i) * 0.1) }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            StatusPill(online: state.availability.map { $0 == .free || state.comesBack } ?? (state == .offline ? false : nil),
                       text: state.label ?? LinkText.status(nil))
                .modifier(rise(0))
            VStack(alignment: .leading, spacing: 10) {
                Text(computer.title)
                    .font(Theme.display(nameSize, .bold))
                    .tracking(-nameSize * 0.045)
                    .lineLimit(2)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .frame(maxWidth: size.width * 0.6, alignment: .leading)
                if !computer.computer.isEmpty {
                    Text(verbatim: computer.computer).font(Theme.sans(16)).foregroundStyle(Theme.muted).lineLimit(1)
                }
                if case let .status(s) = state, let slot = s.slot.flatMap(LendText.slot) {
                    HStack(spacing: 8) {
                        Icon("key", size: 15, bold: true).foregroundStyle(Theme.accent)
                        Text(slot).font(Theme.sans(15)).foregroundStyle(Theme.text2)
                    }
                }
                if case let .refused(text) = state {
                    HStack(spacing: 8) {
                        Icon("warning", size: 15, bold: true).foregroundStyle(Theme.warn)
                        Text(text).font(Theme.sans(14)).foregroundStyle(Theme.warnText)
                    }
                }
            }
            .modifier(rise(1))
            actions
                .padding(.top, 10)
                .modifier(rise(2))
        }
        .frame(maxWidth: 820, alignment: .leading)
    }

    @ViewBuilder private var actions: some View {
        HStack(spacing: 14) {
            if let session = app.sessions.all.first(where: { $0.borrowSource?.computer == computer.id }) {
                Button { session.bringToFront() } label: {
                    HStack(spacing: 12) { Text(LinkText.showSession); Icon("arrow-right", size: 18, bold: true) }
                }
                .buttonStyle(PrimaryButton(height: 56, lit: true))
            } else if state == .offline || state == .checking && app.borrowed.password(for: computer) != nil {
                Button { Task { await app.borrowed.refresh(computer) } } label: {
                    HStack(spacing: 10) { Icon("arrows-clockwise", size: 18); Text("Réessayer") }
                }
                .buttonStyle(GhostButton(height: 56))
            } else {
                Button { app.borrow(computer) } label: {
                    HStack(spacing: 12) {
                        Icon("key", size: 16, bold: true)
                        Text(primaryTitle)
                        Icon("arrow-right", size: 18, bold: true)
                    }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
                .disabled(!(state.canBorrow || state == .checking || isRefusedPassword) || app.sessions.isFull)
                .help(app.sessions.isFull ? LinkText.computersLimit : "")
            }
            Button { app.removeBorrowed(computer) } label: {
                HStack(spacing: 8) { Icon("trash", size: 16); Text(LendText.removeComputer) }
            }
            .buttonStyle(GhostButton(height: 48))
        }
    }

    private var isRefusedPassword: Bool {
        if case .refused = state { return true }
        return false
    }

    private var primaryTitle: String {
        if state.comesBack { return LendText.comeBack }
        if state.availability == .busy { return LendText.ask(computer.hostName) }
        return LendText.borrow
    }
}

/// L'accueil d'un ordinateur prêté par cet appareil : « Reprendre
/// l'ordinateur » ; pendant le préavis, « Reprise dans 1:42 » et « Reprendre
/// tout de suite ».
struct LentActions: View {
    @EnvironmentObject var app: AppModel
    let host: Host
    let view: LendView
    @State private var working = false
    @State private var error: String?

    var body: some View {
        HStack(spacing: 14) {
            if view.notice?.reason == .takenBack {
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    Text(LentHome.takingBack(view, now: context.date) ?? "")
                        .font(Theme.mono(17, .medium)).foregroundStyle(Theme.accentLight)
                        .monospacedDigit()
                }
                Button { take(now: true) } label: {
                    HStack(spacing: 10) { Icon("power", size: 18, bold: true); Text(LendText.takeBackNow) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .disabled(working)
            } else {
                Button { take(now: false) } label: {
                    HStack(spacing: 12) { Icon("key", size: 16, bold: true); Text(LendText.takeBack) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
                .disabled(working)
            }
            if let error {
                Text(error).font(Theme.sans(13)).foregroundStyle(Theme.warnText).frame(maxWidth: 320, alignment: .leading)
            }
        }
    }

    private func take(now: Bool) {
        working = true
        Task {
            error = await app.store.takeBack(host, now: now)
            working = false
        }
    }
}

/// L'ordinateur d'un ami, sur la rangée de l'accueil : son nom et son état.
struct BorrowedCard: View {
    @EnvironmentObject var app: AppModel
    let computer: BorrowedComputer
    let select: () -> Void
    @State private var hovering = false

    private var state: BorrowedState { app.borrowed.states[computer.id] ?? .checking }

    var body: some View {
        Button(action: select) {
            VStack(alignment: .leading, spacing: 0) {
                ZStack {
                    Signature(seed: computer.title)
                    Icon("key", size: 22, bold: true).foregroundStyle(Theme.accentLight.opacity(0.8))
                }
                .frame(height: 86)
                .clipped()
                VStack(alignment: .leading, spacing: 4) {
                    Text(computer.title).font(Theme.sans(15, .semibold)).foregroundStyle(state == .offline ? Theme.muted : Theme.text).lineLimit(1)
                    HStack(spacing: 7) {
                        Circle()
                            .fill(state.availability == .free ? Theme.ok : .clear)
                            .overlay(Circle().strokeBorder(state.availability == .free ? .clear : Theme.faint))
                            .frame(width: 6, height: 6)
                        Text(state.label ?? LinkText.status(nil)).font(Theme.sans(12)).foregroundStyle(Theme.muted).lineLimit(1)
                    }
                }
                .padding(.horizontal, 14).padding(.vertical, 11)
            }
            .frame(width: 244, height: 164, alignment: .top)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .glass(14)
            .offset(y: hovering ? -3 : 0)
            .shadow(color: .black.opacity(hovering ? 0.5 : 0), radius: 18, y: 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(Theme.easeOut) { hovering = h } }
        .contextMenu {
            Button(LendText.borrow) { app.borrow(computer) }.disabled(!state.canBorrow || app.sessions.isFull)
            Button(LendText.removeComputer) { app.removeBorrowed(computer) }
        }
    }
}

#if DEBUG
/// Démonstration : le prêt de l'ordinateur dessiné dans un PNG, sans fenêtre
/// (comme `SharePanelDemo`).
@MainActor
enum LendDemo {
    /// La vue d'un ordinateur prêté à Léa, jusqu'à dans 1 h 20 ; `notice` : le préavis de reprise en cours.
    static func lentView(notice: Bool) -> LendView {
        LendView(enabled: true, state: notice ? .ending : .lent,
                 borrower: LendBorrower(name: "Léa", kind: .friend, app: "ios", since: Date().addingTimeInterval(-1800),
                                        until: Calendar.current.date(bySettingHour: 18, minute: 0, second: 0, of: Date())),
                 notice: notice ? LendNotice(reason: .takenBack, at: Date().addingTimeInterval(102)) : nil)
    }

    static func render(_ kind: String, app: AppModel, to path: String) async {
        switch kind {
        case "panel", "panel-code", "panel-off", "panel-impossible", "panel-relay-connecting", "panel-relay-joined", "panel-relay-unreachable":
            let lend = LendHostModel(console: nil) {
                kind == "panel-off" ? .unavailable(LendText.tooOld) : kind == "panel-impossible" ? .unavailable(LendText.notPossible) : .available
            }
            lend.demo = true
            if kind == "panel-code" {
                lend.demoSet(LendView(requests: [LendRequest(id: "1", name: "Léa", app: "ios", viaRelay: true, since: Date())]))
                await lend.createCode()
            } else if kind.hasPrefix("panel-relay-") {
                // Le code et son relais : en préparation (indicateur, copie et QR code retenus), joint, ou pas encore joignable.
                lend.demoRelay = ShareState.Relay(wire: String(kind.dropFirst("panel-relay-".count)))
                await lend.createCode()
            }
            write(LendPanel(computer: "Tour du bureau", lend: lend).background(Theme.surface).environment(\.colorScheme, .dark), width: LendPanel.width, to: path)
        case "home", "lent", "lent-notice":
            if kind == "home" {
                app.borrowed.loadDemo("busy")
                app.borrowed.setDemoState("slot", for: app.borrowed.computers[0].id)
                let view = ZStack(alignment: .topLeading) {
                    Theme.background
                    BorrowedHero(computer: app.borrowed.computers[0], arrived: true, size: CGSize(width: 1100, height: 520)).padding(48)
                }
                .environmentObject(app)
                .frame(width: 1100, height: 520)
                write(view, width: 1100, to: path)
            } else if let tour = app.store.ordered.first {
                let view = lentView(notice: kind == "lent-notice")
                let content = ZStack(alignment: .topLeading) {
                    Theme.background
                    VStack(alignment: .leading, spacing: 18) {
                        StatusPill(online: true, text: LentHome.line(view))
                        Text(tour.name).font(Theme.display(64, .bold)).foregroundStyle(Theme.text)
                        LentActions(host: tour, view: view)
                    }
                    .padding(48)
                }
                .environmentObject(app)
                .frame(width: 1100, height: 360)
                write(content, width: 1100, to: path)
            }
        default:
            // L'ami : la barre (`bar`), l'attente de l'hôte (`waiting`), le préavis, la fin, le réseau perdu.
            let target = LendTarget(host: "192.0.2.1", port: 1, fingerprint: nil)
            let source = BorrowSource(target: target, credentials: .code("482915"), name: "Camille", hostHint: "Matthieu")
            let session = SessionController(borrow: source, store: app.store, opening: .first(home: nil)) { _ in }
            if kind == "waiting" {
                session.demoHeadlessBorrow(waiting: true)
            } else {
                session.demoHeadlessBorrow(waiting: false)
                if kind != "bar" { session.setBar(false) }
                if kind == "notice" || kind == "end" || kind == "lost" {
                    session.demoLend(kind == "end" ? "taken_back" : kind)
                }
            }
            try? await Task.sleep(for: .milliseconds(700))
            let view = ZStack(alignment: .top) {
                Color(white: 0.12)
                SessionChrome(session: session)
            }
            .frame(width: 1400, height: kind == "bar" ? 170 : 420)
            write(view, width: 1400, to: path)
        }
    }

    private static func write<V: View>(_ view: V, width: CGFloat, to path: String) {
        let host = NSHostingView(rootView: view)
        let height = host.fittingSize.height
        host.frame = NSRect(x: 0, y: 0, width: width, height: max(height, 100))
        let window = NSWindow(contentRect: host.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.appearance = NSAppearance(named: .darkAqua)
        window.contentView = host
        host.layoutSubtreeIfNeeded()
        guard let rep = host.bitmapImageRepForCachingDisplay(in: host.bounds) else { return }
        host.cacheDisplay(in: host.bounds, to: rep)
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: path))
    }
}
#endif
