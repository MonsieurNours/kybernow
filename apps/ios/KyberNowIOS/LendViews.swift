// Prêter l'ordinateur (KyberNow Share) sur iPhone et iPad (docs/pret.md ;
// textes : docs/link-ui.md, « Prêt de l'ordinateur »). L'hôte : le panneau de
// « Prêter l'ordinateur… » dans la barre de session (durée, compte Steam,
// « Créer un code », code, QR code, « Copier le lien », « Annuler le code »,
// les demandes). L'ami : l'ajout d'un ordinateur prêté (lien d'invitation, ou
// « Avec un compte d'ami » dans l'ajout), sa carte et sa scène sur l'accueil.
// Les appels et les règles sont dans Lend.swift, commun aux apps d'Apple.

import KyberKit
import SwiftUI
import UIKit

// MARK: - Hôte : le panneau

/// Le panneau de l'hôte : les demandes, puis la durée, le compte Steam et
/// « Créer un code » ; le code créé, comme le partage, et « Annuler le code ».
struct LendPanelView: View {
    let computer: String
    @ObservedObject var lend: LendHostModel
    @Environment(\.dismiss) private var dismiss
    @State private var copyResult: String?
    @State private var copied = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack(alignment: .top) {
                    Text(LendText.title(computer))
                        .font(Theme.sans(20, .semibold)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 12)
                    Button { dismiss() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text(LendText.close))
                }
                if let reason = lend.availability.reason {
                    AddHostNotice(kind: .warn, text: reason)
                } else {
                    if lend.showWarning { AddHostNotice(kind: .warn, text: LendText.warning) }
                    if !lend.requests.isEmpty { requests }
                    if let code = lend.code { created(code) } else { choices }
                    if let error = lend.error { AddHostNotice(kind: .danger, text: error) }
                }
            }
            .padding(20)
        }
        .background(Theme.bar)
        .environment(\.colorScheme, .dark)
        .task { await lend.load() }
        // L'avertissement se montre une fois.
        .onDisappear { if lend.availability.isAvailable { lend.warningSeen() } }
    }

    @ViewBuilder private var requests: some View {
        VStack(alignment: .leading, spacing: 14) {
            Theme.eyebrow(ShareText.requests, color: Theme.faint)
            ForEach(lend.requests) { request in
                VStack(alignment: .leading, spacing: 10) {
                    HStack(alignment: .center, spacing: 12) {
                        Icon("key", size: 20).foregroundStyle(Theme.accent).frame(width: 24)
                        VStack(alignment: .leading, spacing: 3) {
                            Text(request.slot ? LendText.slotStarts(request.name) : LendText.wantsToBorrow(request.name))
                                .font(Theme.sans(15, .medium)).foregroundStyle(Theme.text)
                                .fixedSize(horizontal: false, vertical: true)
                            if let detail = LendText.requestDetail(request) {
                                Text(detail).font(Theme.sans(12.5)).foregroundStyle(Theme.faint)
                            }
                        }
                        Spacer(minLength: 0)
                    }
                    if !request.slot {
                        HStack(spacing: 10) {
                            Button(ShareText.accept) { Task { await lend.answer(request, accept: true) } }
                                .buttonStyle(PrimaryButton(height: 46, fontSize: 15))
                            Button(ShareText.decline) { Task { await lend.answer(request, accept: false) } }
                                .buttonStyle(GlassButton(height: 46, fontSize: 15))
                        }
                        .disabled(lend.busy)
                    }
                }
                .accessibilityElement(children: .contain)
            }
            Text(LendText.acceptCloses).font(Theme.sans(13)).foregroundStyle(Theme.faint)
        }
        Rectangle().fill(Theme.line).frame(height: 1)
    }

    @ViewBuilder private var choices: some View {
        LendChoice(title: LendText.duration, options: LendDuration.allCases.map { ($0, $0.label) },
                   selection: Binding(get: { lend.duration }, set: { lend.duration = $0 }))
        LendChoice(title: LendText.steamAccount, options: LendSteam.allCases.map { ($0, $0.label) },
                   selection: Binding(get: { lend.steam }, set: { lend.steam = $0 }))
        Button(LendText.createCode) { Task { await lend.createCode() } }
            .buttonStyle(PrimaryButton(height: 52, fontSize: 16))
            .disabled(lend.busy)
    }

    @ViewBuilder private func created(_ code: LendCodeView) -> some View {
        ViewThatFits(in: .horizontal) {
            HStack(alignment: .top, spacing: 20) {
                LendQR(link: code.link)
                codeAndCopy(code)
            }
            VStack(alignment: .leading, spacing: 16) {
                LendQR(link: code.link)
                codeAndCopy(code)
            }
        }
        VStack(alignment: .leading, spacing: 6) {
            Text(LendText.help).font(Theme.sans(13.5)).foregroundStyle(Theme.muted).fixedSize(horizontal: false, vertical: true)
            Text(LendText.sessionCloses).font(Theme.sans(13.5)).foregroundStyle(Theme.faint).fixedSize(horizontal: false, vertical: true)
        }
        Text(verbatim: [code.duration.flatMap { LendDuration(rawValue: $0)?.label } ?? LendDuration.untilTakenBack.label, code.steam.label].joined(separator: " · "))
            .font(Theme.sans(13)).foregroundStyle(Theme.muted)
        Button(LendText.cancelCode) { Task { await lend.cancelCode() } }
            .buttonStyle(AddHostDangerButton())
            .disabled(lend.busy)
    }

    private func codeAndCopy(_ code: LendCodeView) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Theme.eyebrow(ShareText.code, color: Theme.faint)
            Text(verbatim: Share.formatCode(code.code))
                .font(Theme.mono(36, .medium)).tracking(2)
                .foregroundStyle(Theme.neonCore)
                .neon(.lit)
                .fixedSize()
                .accessibilityLabel(Text(verbatim: "\(ShareText.code) \(code.code.map(String.init).joined(separator: " "))"))
            Text(LendText.oneSession).font(Theme.sans(13)).foregroundStyle(Theme.faint)
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
                if !copied, let link = lend.linkToCopy {
                    Text(verbatim: link).font(Theme.mono(12)).foregroundStyle(Theme.text2)
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    private func copyLink() {
        guard let text = lend.linkToCopy else { return }
        UIPasteboard.general.string = text
        let ok = UIPasteboard.general.hasStrings
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

/// Un choix du panneau (durée, compte Steam) : les options en boutons, deux par ligne si besoin.
private struct LendChoice<Value: Hashable>: View {
    let title: String
    let options: [(Value, String)]
    @Binding var selection: Value

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title).font(Theme.sans(14)).foregroundStyle(Theme.muted)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 140), spacing: 8)], spacing: 8) {
                ForEach(options, id: \.0) { value, label in
                    let on = selection == value
                    Button { selection = value } label: {
                        Text(label)
                            .font(Theme.sans(14.5, .semibold))
                            .foregroundStyle(on ? Theme.text : Theme.muted)
                            .multilineTextAlignment(.center)
                            .padding(.horizontal, 10)
                            .frame(maxWidth: .infinity, minHeight: 46)
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
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text(title))
    }
}

/// Le QR code du lien du code de prêt (`kybernow://join…`), net, sur fond blanc.
private struct LendQR: View {
    let link: String

    var body: some View {
        if let image = Share.qrImage(for: link) {
            Image(decorative: image, scale: 1)
                .interpolation(.none)
                .resizable()
                .aspectRatio(1, contentMode: .fit)
                .frame(width: 150, height: 150)
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .accessibilityElement()
                .accessibilityLabel(ShareText.qrLabel)
        } else {
            RoundedRectangle(cornerRadius: 10, style: .continuous).fill(Theme.surface).frame(width: 150, height: 150)
        }
    }
}

// MARK: - Ami : ajouter un ordinateur prêté

/// « Avec un compte d'ami » : l'ordinateur (celui du lien d'invitation, ou un
/// ordinateur du réseau), l'identifiant, le mot de passe et « Retenir le mot
/// de passe ». « Ajouter » vérifie le compte auprès de la Station.
struct FriendAccountForm: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var discovery: Discovery
    @Environment(\.addHostWide) private var wide
    /// Le lien d'invitation, s'il y en a un ; sinon un ordinateur du réseau à choisir.
    let invite: LendInvite?
    let added: (BorrowedComputer) -> Void
    @State private var login = ""
    @State private var password = ""
    @State private var remember = true
    @State private var picked: Candidate?
    @State private var working = false
    @State private var error: String?
    @State private var resolved: [String: Candidate] = [:]

    private var target: LendInvite? {
        if let invite { return invite.with(login: login.trimmed) }
        guard let picked else { return nil }
        return LendInvite(host: picked.address, port: picked.port, fingerprint: picked.expected, login: login.trimmed)
    }
    private var valid: Bool { target != nil && !login.trimmed.isEmpty && !password.isEmpty && !working }

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 18 : 14) {
            if invite == nil { network }
            AddHostField(label: LendText.username, text: $login, content: .username)
            AddHostField(label: LendText.password, text: $password, secure: true, content: .password, submit: .go) { add() }
            Toggle(isOn: $remember) { Text(LendText.rememberPassword).font(Theme.sans(15)).foregroundStyle(Theme.text2) }
                .tint(Theme.accent)
            if let error { AddHostNotice(kind: .warn, text: error) }
            Button { add() } label: {
                HStack(spacing: 10) {
                    if working { HomeSpinner(size: 16) }
                    Text(LendText.add)
                }
            }
            .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
            .disabled(!valid)
            .opacity(valid ? 1 : 0.45)
        }
        .onAppear { if let invite { login = invite.login } }
        .task(id: discovery.found.map(\.id)) {
            guard invite == nil else { return }
            for f in discovery.found where resolved[f.id] == nil {
                if let c = await Discovery.resolve(f) { resolved[f.id] = c }
            }
        }
    }

    /// Les ordinateurs annoncés sur le réseau : on choisit celui de l'ami.
    private var network: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 10) {
                Theme.eyebrow(String(localized: "Recherche sur le réseau local"))
                AddHostWaitingDots()
            }
            ForEach(discovery.found.filter { resolved[$0.id] != nil }) { f in
                let c = resolved[f.id]!
                let on = picked == c
                Button { picked = c } label: {
                    HStack(spacing: 12) {
                        Icon(on ? "check" : "monitor", size: 20).foregroundStyle(on ? Theme.accent : Theme.muted)
                        Text(f.name).font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                        Spacer(minLength: 8)
                        Text(verbatim: c.address).font(Theme.mono(12)).foregroundStyle(Theme.faint).lineLimit(1)
                    }
                    .padding(.horizontal, 16).frame(minHeight: 54)
                    .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(on ? Theme.accentFill.opacity(0.22) : Theme.surface))
                    .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(on ? Theme.accent.opacity(0.6) : Theme.line2))
                    .contentShape(Rectangle())
                }
                .buttonStyle(AddHostPressStyle())
                .accessibilityAddTraits(on ? .isSelected : [])
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

/// « Avec un compte d'ami », dans l'ajout d'un ordinateur.
struct AddHostFriendPane: View {
    @EnvironmentObject private var app: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            AddHostHeader(eyebrow: String(localized: "Ajouter un ordinateur"), title: LendText.withFriendAccount)
            FriendAccountForm(invite: nil) { computer in
                app.sheet = nil
                app.selected = computer.id
            }
        }
    }
}

/// Le lien d'invitation ouvert (`kybernow://borrow?…`) : « {hôte} vous prête
/// son ordinateur. » quand on sait déjà qui, l'identifiant rempli, le mot de
/// passe à taper, « Ajouter ».
struct InviteView: View {
    @EnvironmentObject private var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let invite: LendInvite

    private var known: BorrowedComputer? { app.borrowed.existing(for: invite) }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                HStack(alignment: .top) {
                    Text(known.map { LendText.invitation($0.hostName) } ?? LendText.withFriendAccount)
                        .font(Theme.display(34, 660)).tracking(-34 * 0.03)
                        .foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityAddTraits(.isHeader)
                    Spacer(minLength: 12)
                    Button { dismiss() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text(StationText.cancel))
                }
                Text(verbatim: invite.host).font(Theme.mono(13)).foregroundStyle(Theme.faint)
                FriendAccountForm(invite: invite) { computer in
                    dismiss()
                    app.selected = computer.id
                }
            }
            .padding(24)
            .frame(maxWidth: 560, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .scrollDismissesKeyboard(.interactively)
        .background(Theme.background)
        .environment(\.addHostWide, DeviceDisplay.isPad)
    }
}

/// Le mot de passe d'un ordinateur prêté, quand il n'est pas retenu (ou
/// refusé) : demandé à « Emprunter ».
struct BorrowPasswordView: View {
    @EnvironmentObject private var app: AppModel
    @Environment(\.dismiss) private var dismiss
    let computer: BorrowedComputer
    @State private var password = ""
    @State private var remember: Bool

    init(computer: BorrowedComputer) {
        self.computer = computer
        _remember = State(initialValue: computer.rememberPassword)
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack(alignment: .top) {
                    Text(computer.title)
                        .font(Theme.display(30, 660)).foregroundStyle(Theme.text)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 12)
                    Button { dismiss() } label: {
                        Icon("x", size: 16).foregroundStyle(Theme.muted).frame(width: 44, height: 44)
                    }
                    .accessibilityLabel(Text(StationText.cancel))
                }
                Text(verbatim: computer.login).font(Theme.mono(13)).foregroundStyle(Theme.faint)
                AddHostField(label: LendText.password, text: $password, secure: true, content: .password, submit: .go) { borrow() }
                Toggle(isOn: $remember) { Text(LendText.rememberPassword).font(Theme.sans(15)).foregroundStyle(Theme.text2) }
                    .tint(Theme.accent)
                Button(LendText.borrow) { borrow() }
                    .buttonStyle(PrimaryButton(height: 54, fontSize: 17))
                    .disabled(password.isEmpty)
                    .opacity(password.isEmpty ? 0.45 : 1)
            }
            .padding(24)
            .frame(maxWidth: 560, alignment: .leading)
            .frame(maxWidth: .infinity)
        }
        .background(Theme.background)
    }

    private func borrow() {
        guard !password.isEmpty else { return }
        app.borrowed.updatePassword(password, remember: remember, for: computer)
        dismiss()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { app.borrow(computer) }
    }
}

// MARK: - Ami : l'accueil

/// L'ordinateur d'un ami en scène (docs/link-ui.md) : son état, « Ordinateur
/// de {hôte} », le nom de l'ordinateur, le créneau, un refus s'il y en a un.
struct BorrowedHero: View {
    @ObservedObject var borrowed: BorrowedStore
    let computer: BorrowedComputer
    let wide: Bool

    private var state: BorrowedState { borrowed.states[computer.id] ?? .checking }

    var body: some View {
        VStack(alignment: .leading, spacing: wide ? 16 : 11) {
            HomeChip(height: wide ? 32 : 30, fontSize: wide ? 14 : 13) {
                HomeStatusDot(online: BorrowedLook.online(state), size: wide ? 8 : 7)
                Text(state.label ?? LinkText.status(nil)).lineLimit(1)
            }
            Text(computer.title)
                .font(Theme.display(wide ? 84 : 48, 680))
                .tracking(wide ? -84 * 0.045 : -48 * 0.04)
                .lineSpacing(wide ? -12 : -6)
                .lineLimit(2)
                .minimumScaleFactor(0.5)
                .foregroundStyle(Theme.text)
                .frame(minHeight: wide ? 160 : 96, alignment: .bottomLeading)
                .accessibilityAddTraits(.isHeader)
            if !computer.computer.isEmpty {
                Text(verbatim: computer.computer).font(Theme.sans(wide ? 17 : 14.5)).foregroundStyle(Theme.text2).lineLimit(1)
            }
            if case let .status(s) = state, let slot = s.slot.flatMap(LendText.slot) {
                HStack(alignment: .firstTextBaseline, spacing: 9) {
                    Icon("key", size: wide ? 18 : 16).foregroundStyle(Theme.accent)
                    Text(slot).font(Theme.sans(wide ? 16 : 13.5)).foregroundStyle(Theme.text2)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            if case let .refused(text) = state {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Icon("warning", size: wide ? 17 : 15).foregroundStyle(Theme.warn)
                    Text(text).font(Theme.sans(wide ? 15 : 13.5)).foregroundStyle(Theme.warnText)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .animation(Theme.easeOut, value: state)
    }
}

/// Le bouton principal de l'ordinateur d'un ami : « Emprunter », « Demander à
/// {hôte} » (utilisé), « Revenir » (le prêt en cours est le sien),
/// « Réessayer » (hors ligne) ; puis « Retirer ».
struct BorrowedActions: View {
    @EnvironmentObject private var app: AppModel
    @ObservedObject var borrowed: BorrowedStore
    let computer: BorrowedComputer
    let wide: Bool
    let remove: () -> Void
    @State private var retrying = false

    private var state: BorrowedState { borrowed.states[computer.id] ?? .checking }
    private var offline: Bool { state == .offline || (state == .checking && borrowed.password(for: computer) != nil) }
    private var refused: Bool { if case .refused = state { return true } else { return false } }
    private var enabled: Bool { offline || state.canBorrow || state == .checking || refused }

    private var title: String {
        if offline { return String(localized: "Réessayer") }
        if state.comesBack { return LendText.comeBack }
        if state.availability == .busy { return LendText.ask(computer.hostName) }
        return LendText.borrow
    }

    var body: some View {
        let cta = Button {
            if offline { retry() } else { app.borrow(computer) }
        } label: {
            HStack(spacing: 12) {
                Text(title)
                if !wide { Spacer(minLength: 0) }
                if retrying { HomeSpinner(size: 20) } else { Icon("arrow-right", size: 20, bold: true) }
            }
            .padding(.leading, wide ? 22 : 20)
            .padding(.trailing, wide ? 26 : 18)
            .frame(maxWidth: wide ? nil : .infinity)
        }
        .buttonStyle(HomeCTAStyle(online: enabled && !offline, height: wide ? 58 : 56, fontSize: wide ? 18 : 17))
        .disabled(!enabled || retrying)
        let removeButton = Button(role: .destructive, action: remove) {
            HStack(spacing: 9) {
                Icon("trash", size: 20)
                if wide { Text(ShareText.remove) }
            }
            .font(Theme.sans(15, .medium))
            .foregroundStyle(Theme.text)
            .padding(.horizontal, wide ? 18 : 0)
            .frame(minWidth: 52, minHeight: 52)
            .glass(12)
            .contentShape(Rectangle())
        }
        .buttonStyle(HomePressStyle())
        .accessibilityLabel(Text(ShareText.remove))
        HStack(spacing: wide ? 16 : 8) {
            cta
            removeButton
        }
    }

    private func retry() {
        retrying = true
        Task {
            await borrowed.refresh(computer)
            retrying = false
        }
    }
}

/// L'ordinateur d'un ami dans la rangée : son nom et son état.
struct BorrowedCard: View {
    let computer: BorrowedComputer
    let state: BorrowedState
    let selected: Bool
    let m: HomeCardMetrics

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ZStack {
                LinearGradient(colors: [Theme.rgb(0x1A2232), Theme.rgb(0x0A0F17)], startPoint: .topLeading, endPoint: .bottomTrailing)
                Icon("key", size: 26).foregroundStyle(Theme.accentLight.opacity(0.8))
            }
            .frame(width: m.width, height: m.image)
            .clipped()
            VStack(alignment: .leading, spacing: 4) {
                Text(computer.title)
                    .font(Theme.sans(m.name, .semibold))
                    .foregroundStyle(state == .offline ? Theme.muted : Theme.text)
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
                HStack(spacing: 6) {
                    HomeStatusDot(online: BorrowedLook.online(state), size: 6, pulse: false)
                    Text(state.label ?? LinkText.status(nil)).lineLimit(1)
                }
                .font(Theme.sans(m.detail))
                .foregroundStyle(Theme.muted)
            }
            .padding(.horizontal, m.padding)
            .padding(.vertical, m.padding - 3)
            Spacer(minLength: 0)
        }
        .frame(width: m.width, height: m.height, alignment: .topLeading)
        .overlay(alignment: .bottom) {
            if selected {
                Capsule()
                    .fill(Theme.accent)
                    .frame(height: 3)
                    .padding(.horizontal, m.padding)
                    .padding(.bottom, 6)
                    .shadow(color: Theme.accentFill, radius: 5)
            }
        }
        .glass(m.radius)
        .clipShape(RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .contentShape(.contextMenuPreview, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
        .contentShape(.hoverEffect, RoundedRectangle(cornerRadius: m.radius, style: .continuous))
    }
}

enum BorrowedLook {
    /// Le point de l'état : allumé quand on peut emprunter tout de suite, éteint hors ligne, à l'attente sinon.
    static func online(_ state: BorrowedState) -> Bool? {
        switch state {
        case .offline: return false
        case .checking: return nil
        default: return state.availability == .free || state.comesBack
        }
    }
}

// MARK: - Hôte : l'accueil d'un ordinateur prêté

/// L'accueil d'un ordinateur prêté par cet appareil : « Reprendre
/// l'ordinateur » ; pendant le préavis, « Reprise dans 1:42 » et « Reprendre
/// tout de suite ».
struct LentActions: View {
    @EnvironmentObject private var app: AppModel
    let host: Host
    let view: LendView
    let wide: Bool
    @State private var working = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if view.notice?.reason == .takenBack {
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    Text(LentHome.takingBack(view, now: context.date) ?? "")
                        .font(Theme.mono(wide ? 20 : 17, .medium)).foregroundStyle(Theme.accentLight)
                        .monospacedDigit()
                }
                cta(LendText.takeBackNow) { take(now: true) }
            } else {
                cta(LendText.takeBack) { take(now: false) }
            }
            if let error {
                Text(error).font(Theme.sans(13.5)).foregroundStyle(Theme.warnText).fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private func cta(_ title: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Text(title)
                if !wide { Spacer(minLength: 0) }
                if working { HomeSpinner(size: 20) } else { Icon("key", size: 20, bold: true) }
            }
            .padding(.leading, wide ? 22 : 20)
            .padding(.trailing, wide ? 26 : 18)
            .frame(maxWidth: wide ? nil : .infinity)
        }
        .buttonStyle(HomeCTAStyle(online: true, height: wide ? 58 : 56, fontSize: wide ? 18 : 17))
        .disabled(working)
    }

    private func take(now: Bool) {
        working = true
        Task {
            error = await app.store.takeBack(host, now: now)
            working = false
        }
    }
}
