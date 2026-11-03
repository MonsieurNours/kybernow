// Consoles PlayStation à l'accueil et leur association (textes :
// docs/link-ui.md, « Consoles PlayStation ») : la console choisie (état,
// nom, modèle, jeu en cours, bouton principal, puce du compte, puce
// « HDR »), sa carte dans la rangée des ordinateurs, la carte « Sur le
// réseau » d'une console pas encore associée, l'onglet « PlayStation » de
// la fenêtre d'ajout (comptes PSN, consoles trouvées), et la feuille de
// l'association (choix du compte ; compte PSN dans une fenêtre web, une
// fois par compte ; puis le code à 8 chiffres).

import KyberKit
import SwiftUI
import WebKit

// MARK: - La console choisie

struct ConsoleHero: View {
    @EnvironmentObject var app: AppModel
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    let arrived: Bool
    let size: CGSize
    @StateObject private var screen = HDRScreenWatch()

    private var nameSize: CGFloat { min(104, size.width * 0.072, size.height * 0.095) }
    private var status: PlayStationStore.Status { store.status(console) }
    private var paired: Bool { store.paired.contains(console.id) }

    private func rise(_ i: Int) -> some ViewModifier { Rise(arrived: arrived, delay: 0.15 + Double(i) * 0.1) }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            StatusPill(online: status.online, text: status.text)
                .id(status)
                .modifier(rise(0))
            VStack(alignment: .leading, spacing: 10) {
                Text(verbatim: console.name)
                    .font(Theme.display(nameSize, .bold))
                    .tracking(-nameSize * 0.045)
                    .lineLimit(2)
                    .minimumScaleFactor(0.5)
                    .foregroundStyle(Theme.text)
                    .frame(maxWidth: size.width * 0.56, alignment: .leading)
                Text(verbatim: console.model).font(Theme.sans(16)).foregroundStyle(Theme.muted).lineLimit(1)
            }
            .modifier(rise(1))
            if let game = store.live(console)?.runningApp {
                HStack(spacing: 10) {
                    Icon("game-controller", size: 18).foregroundStyle(Theme.accent)
                    Text(verbatim: game).font(Theme.sans(15)).foregroundStyle(Theme.text2)
                }
                .modifier(rise(2))
            }
            actions
                .padding(.top, 10)
                .modifier(rise(3))
        }
        .frame(maxWidth: 820, alignment: .leading)
    }

    @ViewBuilder private var actions: some View {
        HStack(spacing: 14) {
            if !paired {
                Button { store.pairing = PairTarget(console: console, live: store.live(console)) } label: {
                    HStack(spacing: 10) { Icon("link-simple", size: 18, bold: true); Text(PSText.pair) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if let session = store.sessions[console.id] {
                Button { session.bringToFront() } label: {
                    HStack(spacing: 12) { Text(LinkText.showSession); Icon("arrow-right", size: 18, bold: true) }
                }
                .buttonStyle(PrimaryButton(height: 56, lit: true))
                .keyboardShortcut(.defaultAction)
            } else if store.waking.contains(console.id) {
                Button {} label: {
                    HStack(spacing: 10) { ProgressView().controlSize(.small); Text(PSText.waking) }
                }
                .buttonStyle(GhostButton(height: 56))
                .disabled(true)
            } else if store.reaching.contains(console.id) {
                // Hors de chez soi : la console est jointe par une Station.
                Button {} label: {
                    HStack(spacing: 10) { ProgressView().controlSize(.small); Text(PSText.connecting) }
                }
                .buttonStyle(GhostButton(height: 56))
                .disabled(true)
            } else if status == .away {
                // « Hors de chez vous » : par une Station de son réseau et le relais
                // (réveillée d'abord si elle est en mode repos).
                Button { store.playAway(console, app: app) } label: {
                    HStack(spacing: 12) { Icon("game-controller", size: 18, bold: true); Text(PSText.play) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if status == .on {
                Button { store.play(console, app: app) } label: {
                    HStack(spacing: 12) { Icon("game-controller", size: 18, bold: true); Text(PSText.play) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if status == .standby {
                Button { store.wakeAndPlay(console, app: app) } label: {
                    HStack(spacing: 10) { Icon("power", size: 18, bold: true); Text(PSText.wakeAndPlay) }
                }
                .buttonStyle(PrimaryButton(height: 56))
                .keyboardShortcut(.defaultAction)
            } else if status == .offline {
                Button { Task { await store.searchOnce() } } label: {
                    HStack(spacing: 10) { Icon("arrows-clockwise", size: 18); Text("Réessayer") }
                }
                .buttonStyle(GhostButton(height: 56))
            } else {
                Button {} label: { Text(PSText.play) }
                    .buttonStyle(PrimaryButton(height: 56))
                    .disabled(true)
            }
            // Associée avec plusieurs comptes : la puce « {pseudo} » ouvre le choix
            // du compte (le profil avec lequel la console ouvre la partie).
            if paired, store.pairedAccounts(console).count > 1, let chosen = store.chosenAccount(console) {
                AccountChip(console: console, chosen: chosen, store: store)
            }
            // La puce « HDR » : mêmes règles que pour un ordinateur (écran HDR ; PS5).
            if paired, console.hasHDR, screen.hdr {
                HDRChipButton(on: console.hdr, available: true, help: HDRText.help) {
                    store.setHDR(!console.hdr, for: console)
                }
            }
            // « Auto » ou « Personnalisé » (pas de profils pour une console) : ouvre les réglages.
            let custom = store.isCustom(console)
            Button { app.openSettings(console, section: .picture) } label: {
                HStack(spacing: 8) {
                    Icon("sliders-horizontal", size: 16, bold: true)
                    Text(custom ? PSText.custom : PSText.auto)
                }
                .font(Theme.sans(14, .semibold))
                .foregroundStyle(Theme.text2)
                .padding(.horizontal, 16)
                .frame(height: 40)
                .background(Color.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Theme.line2))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(LinkText.settingsTitle(console.name))
            Button { app.openSettings(console) } label: {
                HStack(spacing: 8) { Icon("gear-six", size: 17); Text("Réglages") }
            }
            .buttonStyle(GhostButton(height: 48))
            .help(LinkText.settingsTitle(console.name))
            .accessibilityLabel(LinkText.settingsTitle(console.name))
        }
    }
}

/// La puce du compte, à côté du bouton principal : le compte choisi, un
/// clic ouvre la liste des comptes associés à la console (menu du Mac, sous
/// le pointeur ; un `Menu` de SwiftUI perdrait l'apparence des puces).
private struct AccountChip: View {
    let console: PlayStationConsole
    let chosen: PSNAccount
    @ObservedObject var store: PlayStationStore

    var body: some View {
        Button(action: open) {
            HStack(spacing: 8) {
                Icon("user", size: 16, bold: true)
                Text(verbatim: store.name(chosen)).lineLimit(1)
                Icon("caret-down", size: 12, bold: true)
            }
            .font(Theme.sans(14, .semibold))
            .foregroundStyle(Theme.text2)
            .padding(.horizontal, 16)
            .frame(height: 40)
            .background(Color.white.opacity(0.04), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).strokeBorder(Theme.line2))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(store.name(chosen))
    }

    private func open() {
        let picker = Picker(store: store, console: console)
        let menu = NSMenu()
        for a in store.pairedAccounts(console) {
            let item = NSMenuItem(title: store.name(a), action: #selector(Picker.pick(_:)), keyEquivalent: "")
            item.target = picker
            item.representedObject = a
            item.state = a.id == chosen.id ? .on : .off
            menu.addItem(item)
        }
        // Le menu tient la main jusqu'au choix : `picker` vit jusque-là.
        menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
        withExtendedLifetime(picker) {}
    }

    @MainActor
    private final class Picker: NSObject {
        let store: PlayStationStore
        let console: PlayStationConsole
        init(store: PlayStationStore, console: PlayStationConsole) { (self.store, self.console) = (store, console) }
        @objc func pick(_ item: NSMenuItem) {
            if let a = item.representedObject as? PSNAccount { store.choose(a, for: console) }
        }
    }
}

// MARK: - Dans la rangée

/// Une console associée : son nom et son état.
struct ConsoleCard: View {
    @EnvironmentObject var app: AppModel
    let console: PlayStationConsole
    @ObservedObject var store: PlayStationStore
    let select: () -> Void
    @State private var hovering = false

    var body: some View {
        let status = store.status(console)
        Button(action: select) {
            VStack(alignment: .leading, spacing: 0) {
                Signature(seed: console.name)
                    .frame(height: 86)
                    .clipped()
                VStack(alignment: .leading, spacing: 4) {
                    Text(verbatim: console.name).font(Theme.sans(15, .semibold)).foregroundStyle(status.online == false ? Theme.muted : Theme.text).lineLimit(1)
                    HStack(spacing: 7) {
                        Circle()
                            .fill(status.online == true ? Theme.ok : .clear)
                            .overlay(Circle().strokeBorder(status.online == true ? .clear : Theme.faint))
                            .frame(width: 6, height: 6)
                        Text(status.text).font(Theme.sans(12)).foregroundStyle(Theme.muted).lineLimit(1)
                    }
                }
                .padding(.horizontal, 14).padding(.vertical, 11)
            }
            .frame(width: cardSize.width, height: cardSize.height, alignment: .top)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .glass(14)
            .offset(y: hovering ? -3 : 0)
            .shadow(color: .black.opacity(hovering ? 0.5 : 0), radius: 18, y: 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { h in withAnimation(Theme.easeOut) { hovering = h } }
        .contextMenu {
            Button(LinkText.settingsTitle(console.name)) { app.openSettings(console) }
            Button(String(localized: "Retirer")) { store.remove(console) }
        }
    }
}

/// Console trouvée sur le réseau, pas encore associée.
struct ConsoleFoundCard: View {
    let found: PlayStation.Found
    let pair: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 7) {
                    Icon("wifi-high", size: 14)
                    Theme.eyebrow(String(localized: "Sur le réseau"), color: Theme.accentLight)
                }
                .foregroundStyle(Theme.accentLight)
                Text(verbatim: found.name).font(Theme.display(22, .semibold)).tracking(-0.4).foregroundStyle(Theme.text).lineLimit(1)
                Text(verbatim: found.model).font(Theme.sans(12)).foregroundStyle(Theme.muted)
            }
            Spacer(minLength: 0)
            Button(PSText.pair, action: pair)
                .buttonStyle(SignalButton(height: 32))
        }
        .padding(16)
        .frame(width: cardSize.width, height: cardSize.height, alignment: .topLeading)
        .background(Theme.accentFill.opacity(0.12), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(Theme.accent.opacity(0.6)))
    }
}

// MARK: - Association

struct PairConsoleSheet: View {
    @EnvironmentObject var app: AppModel
    @Environment(\.dismiss) private var dismiss
    @ObservedObject private var store = AppModel.shared.playstation
    let target: PairTarget
    /// « Ajouter un compte » (sans console) : le compte est gardé, `true` s'il était déjà là.
    var accountAdded: ((Bool) -> Void)?

    private enum Step { case choose, account, code }
    @State private var step: Step = .account
    @State private var web = false
    @State private var byHand = false
    @State private var manualID = ""
    @State private var manualName = ""
    /// Le compte avec lequel associer (étape du code).
    @State private var account: PSNAccount?
    /// Le compte lu était déjà gardé : « Ce compte est déjà là. »
    @State private var already = false
    @State private var code = ""
    @State private var busy = false
    @State private var error: String?
    @FocusState private var codeFocused: Bool

    /// Identifiant saisi à la main : 8 octets en base64 (12 caractères).
    private var manualValid: Bool {
        Data(base64Encoded: manualID.trimmingCharacters(in: .whitespaces))?.count == 8
    }
    private var digits: String { code.filter(\.isNumber) }
    /// Comptes qu'on peut encore associer à cette console.
    private var free: [PSNAccount] { target.found.map { store.unpairedAccounts(hostID: $0.hostID) } ?? [] }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    if let found = target.found {
                        Text(verbatim: found.name).font(Theme.display(30)).foregroundStyle(Theme.text)
                        Text(verbatim: found.model).font(Theme.sans(14)).foregroundStyle(Theme.muted)
                    } else {
                        Text(PSText.addAccount).font(Theme.display(30)).foregroundStyle(Theme.text)
                    }
                }
                Spacer()
                Button { dismiss() } label: { Icon("x", size: 15, bold: true) }
                    .buttonStyle(GhostButton(height: 34))
                    .keyboardShortcut(.cancelAction)
                    .accessibilityLabel(StationText.cancel)
            }
            switch step {
            case .choose: choose
            case .account: accountStep
            case .code: codeStep
            }
            if let error { Notice(kind: .warn, text: error) }
        }
        .padding(.horizontal, 36).padding(.vertical, 32)
        .frame(width: web ? 720 : 640)
        .background {
            ZStack {
                Theme.background
                RadialGradient(colors: [Theme.accentFill.opacity(0.22), .clear], center: .topTrailing, startRadius: 0, endRadius: 520)
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            // Deux comptes ou plus pas encore associés : le choix ; un seul : le code ; aucun : l'étape Compte.
            switch target.found == nil ? 0 : free.count {
            case 0: step = .account
            case 1: account = free[0]; step = .code
            default: step = .choose
            }
            codeFocused = step == .code
        }
    }

    // « Avec quel compte ? »
    @ViewBuilder private var choose: some View {
        Text(PSText.whichAccount).font(Theme.display(22)).foregroundStyle(Theme.text)
        VStack(spacing: 8) {
            ForEach(free) { a in
                Button {
                    account = a
                    error = nil
                    step = .code
                    codeFocused = true
                } label: {
                    PSListRow(title: store.name(a), detail: PSText.consoleCount(store.consoles(of: a).count)) {
                        Icon("arrow-right", size: 16, bold: true).foregroundStyle(Theme.text2)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        Button { error = nil; step = .account } label: {
            HStack(spacing: 8) { Icon("plus", size: 15, bold: true); Text(PSText.addAccount) }
        }
        .buttonStyle(GhostButton(height: 40))
    }

    @ViewBuilder private var accountStep: some View {
        Text(PSText.signInHelp).font(Theme.sans(14)).foregroundStyle(Theme.text2)
            .fixedSize(horizontal: false, vertical: true)
        if web {
            PSNWebView { url in signedIn(url) }
                .frame(height: 520)
                .clipShape(RoundedRectangle(cornerRadius: 12))
                .overlay { if busy { ProgressView() } }
        } else {
            Button { error = nil; web = true } label: {
                HStack(spacing: 10) { Icon("user", size: 18, bold: true); Text(PSText.signIn) }
            }
            .buttonStyle(PrimaryButton(height: 48))
        }
        Button(PSText.byHand) { byHand.toggle() }
            .buttonStyle(.plain)
            .font(Theme.sans(13))
            .foregroundStyle(Theme.accentLight)
        if byHand {
            HStack(alignment: .top, spacing: 12) {
                LabeledField(label: PSText.accountField, text: $manualID, prompt: "AbCdEfGhIjk=", mono: true)
                    .onSubmit(useManualID)
                VStack(alignment: .leading, spacing: 6) {
                    LabeledField(label: PSText.nameField, text: $manualName)
                        .onSubmit(useManualID)
                    Text(PSText.nameHelp).font(Theme.sans(12)).foregroundStyle(Theme.faint)
                }
            }
            HStack {
                Spacer()
                Button(String(localized: "Continuer"), action: useManualID)
                    .buttonStyle(PrimaryButton(height: 44))
                    .disabled(!manualValid)
            }
        }
    }

    @ViewBuilder private var codeStep: some View {
        if let account {
            HStack(spacing: 8) {
                Icon("user", size: 15, bold: true).foregroundStyle(Theme.accent)
                Text(verbatim: PSText.accountLine(store.name(account))).font(Theme.sans(14, .semibold)).foregroundStyle(Theme.text)
            }
        }
        if already { Notice(kind: .info, text: PSText.alreadyHere) }
        Text(PSText.codeHelp).font(Theme.sans(14)).foregroundStyle(Theme.text2)
            .fixedSize(horizontal: false, vertical: true)
        TextField(text: $code, prompt: Text(verbatim: "1234 5678").foregroundColor(Theme.ghost)) { EmptyView() }
            .textFieldStyle(.plain)
            .font(Theme.mono(34, .medium))
            .tracking(4)
            .foregroundStyle(Theme.accentLight)
            .multilineTextAlignment(.center)
            .focused($codeFocused)
            .onSubmit(pair)
            .padding(.horizontal, 14)
            .frame(height: 78)
            .background(Theme.bar, in: RoundedRectangle(cornerRadius: 14))
            .overlay(RoundedRectangle(cornerRadius: 14).stroke(codeFocused ? Theme.accentLine : Theme.line2))
            .disabled(busy)
            .accessibilityLabel(PSText.codeField)
            .help(PSText.codeField)
        HStack {
            Spacer()
            Button(action: pair) {
                HStack(spacing: 10) {
                    if busy { ProgressView().controlSize(.small) }
                    Text(PSText.pair)
                }
            }
            .buttonStyle(PrimaryButton(lit: busy))
            .disabled(digits.count != 8 || busy)
            .keyboardShortcut(.defaultAction)
        }
    }

    private func useManualID() {
        guard manualValid else { return }
        kept(manualID.trimmingCharacters(in: .whitespaces), name: manualName)
    }

    /// Un compte lu ou saisi : gardé tout de suite (même si l'association
    /// échoue ensuite), puis le code ; sans console, la feuille se ferme.
    private func kept(_ id: String, name: String?) {
        let (a, existed) = store.keep(accountID: id, name: name)
        guard target.found != nil else {
            accountAdded?(existed)
            dismiss()
            return
        }
        account = a
        already = existed
        error = nil
        step = .code
        codeFocused = true
    }

    /// Sony a rendu l'adresse de retour : l'identifiant et le pseudo en sont tirés (deux requêtes).
    private func signedIn(_ url: URL) {
        guard !busy else { return }
        busy = true
        Task {
            let result = await Task.detached { PlayStation.account(redirect: url) }.value
            busy = false
            web = false
            switch result {
            case let .success(read):
                kept(read.id, name: read.onlineID)
            case let .failure(e):
                KyberLog.app.notice("PlayStation : identifiant du compte non lu (\(e.detail, privacy: .public))")
                error = PSText.accountUnread
                byHand = true
            }
        }
    }

    private func pair() {
        guard digits.count == 8, !busy, let found = target.found, let account, let id = store.accountID(account), let pin = UInt32(digits) else { return }
        busy = true
        error = nil
        Task {
            let result = await Task.detached { PlayStation.regist(address: found.address, target: found.target, account: id, pin: pin) }.value
            busy = false
            switch result {
            case let .success(registered):
                let console = store.paired(found, registered: registered, account: account.id)
                app.selectedID = console.id
                dismiss()
            case let .failure(e):
                error = PSText.regist(e)
            }
        }
    }
}

/// Une ligne des listes de consoles et de comptes (même présentation que
/// les ordinateurs de « Sur ce réseau »).
struct PSListRow<Trailing: View>: View {
    let title: String
    var detail: String?
    var lit = false
    @ViewBuilder let trailing: () -> Trailing

    var body: some View {
        HStack(spacing: 16) {
            VStack(alignment: .leading, spacing: 3) {
                Text(verbatim: title).font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text).lineLimit(1)
                if let detail {
                    Text(verbatim: detail).font(Theme.sans(13)).foregroundStyle(Theme.muted).lineLimit(2)
                }
            }
            Spacer(minLength: 12)
            trailing()
        }
        .padding(.horizontal, 18).padding(.vertical, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(lit ? Theme.accentSoft.opacity(0.35) : Theme.well, in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(lit ? Theme.accentLine : Theme.line))
    }
}

/// Une console trouvée (onglets « PlayStation » et « Sur ce réseau ») :
/// « {nom} · PS5 », son état ; pas associée : « Associer » ; associée :
/// « Associée : {pseudos} » et, s'il reste un compte, « Associer avec un autre compte ».
struct ConsoleFoundRow: View {
    let found: PlayStation.Found
    @ObservedObject var store: PlayStationStore
    let pair: (PairTarget) -> Void

    var body: some View {
        let kept = store.console(hostID: found.hostID)
        let accounts = kept.map { store.pairedAccounts($0) } ?? []
        let state = found.ready ? PSText.on : PSText.standby
        PSListRow(title: "\(found.name) · \(found.model)",
                  detail: accounts.isEmpty ? "\(state) · \(found.address)" : "\(state) · \(PSText.pairedWith(accounts.map { store.name($0) }))",
                  lit: accounts.isEmpty) {
            if accounts.isEmpty {
                Button(PSText.pair) { pair(target(kept)) }
                    .buttonStyle(PrimaryButton(height: 38))
            } else if !store.unpairedAccounts(hostID: found.hostID).isEmpty {
                Button(PSText.pairAnother) { pair(target(kept)) }
                    .buttonStyle(GhostButton(height: 38))
            }
        }
    }

    private func target(_ kept: PlayStationConsole?) -> PairTarget {
        kept.map { PairTarget(console: $0, live: found) } ?? PairTarget(found: found)
    }
}

/// Onglet « PlayStation » de la fenêtre d'ajout : les comptes PlayStation
/// Network, puis les consoles sur ce réseau.
struct PlayStationTab: View {
    @EnvironmentObject var app: AppModel
    @ObservedObject private var store = AppModel.shared.playstation
    @State private var adding = false
    @State private var already = false
    @State private var renaming: PSNAccount?
    @State private var newName = ""
    @State private var removing: PSNAccount?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                Text(PSText.accountsTitle).font(Theme.display(17, .semibold)).foregroundStyle(Theme.text)
                if store.accounts.isEmpty {
                    Text(PSText.noAccount).font(Theme.sans(13)).foregroundStyle(Theme.faint)
                        .fixedSize(horizontal: false, vertical: true)
                }
                ForEach(store.accounts) { a in
                    PSListRow(title: store.name(a), detail: PSText.consoleCount(store.consoles(of: a).count)) {
                        Button(PSText.rename) { newName = store.name(a); renaming = a }
                            .buttonStyle(GhostButton(height: 34))
                        Button(PSText.remove) { removing = a }
                            .buttonStyle(GhostButton(height: 34))
                    }
                }
                if already { Notice(kind: .info, text: PSText.alreadyHere) }
                Button { already = false; adding = true } label: {
                    HStack(spacing: 8) { Icon("plus", size: 15, bold: true); Text(PSText.addAccount) }
                }
                .buttonStyle(GhostButton(height: 40))

                Text(PSText.consolesTitle).font(Theme.display(17, .semibold)).foregroundStyle(Theme.text).padding(.top, 14)
                let list = store.onNetwork
                if list.isEmpty {
                    Text(PSText.noConsole).font(Theme.sans(13)).foregroundStyle(Theme.faint)
                        .fixedSize(horizontal: false, vertical: true)
                }
                ForEach(list, id: \.hostID) { f in
                    ConsoleFoundRow(found: f, store: store) { app.pair(fromAddSheet: $0) }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxHeight: 460)
        .sheet(isPresented: $adding) {
            PairConsoleSheet(target: .newAccount) { existed in already = existed }
                .environmentObject(app)
        }
        .alert(PSText.rename, isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } }), presenting: renaming) { a in
            TextField(PSText.nameField, text: $newName)
            Button(PSText.rename) { store.rename(a, to: newName); renaming = nil }
            Button(PSText.cancel, role: .cancel) { renaming = nil }
        }
        .alert(removing.map { PSText.removeQuestion(store.name($0), consoles: store.consoles(of: $0).count) } ?? "",
               isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }), presenting: removing) { a in
            Button(PSText.remove, role: .destructive) { store.removeAccount(a); removing = nil }
            Button(PSText.cancel, role: .cancel) { removing = nil }
        } message: { _ in
            Text(PSText.removeHelp)
        }
    }
}

/// La page de connexion de Sony. Son adresse de retour n'est jamais
/// chargée : elle est rendue à `onRedirect`. Rien n'est gardé (ni cookies
/// ni cache) une fois la feuille fermée.
struct PSNWebView: NSViewRepresentable {
    let onRedirect: (URL) -> Void

    /// Safari de bureau : la page de Sony refuse certains navigateurs intégrés.
    private static let userAgent = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15"

    func makeCoordinator() -> Coordinator { Coordinator(onRedirect: onRedirect) }

    func makeNSView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        let view = WKWebView(frame: .zero, configuration: config)
        view.customUserAgent = Self.userAgent
        view.navigationDelegate = context.coordinator
        if let url = PlayStation.loginURL { view.load(URLRequest(url: url)) }
        return view
    }

    func updateNSView(_ view: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKNavigationDelegate {
        let onRedirect: (URL) -> Void
        private var done = false

        init(onRedirect: @escaping (URL) -> Void) { self.onRedirect = onRedirect }

        private func caught(_ url: URL?) -> Bool {
            guard let url, PlayStation.isRedirect(url) else { return false }
            if !done {
                done = true
                onRedirect(url)
            }
            return true
        }

        func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void) {
            decisionHandler(caught(action.request.url) ? .cancel : .allow)
        }

        func webView(_ webView: WKWebView, didReceiveServerRedirectForProvisionalNavigation navigation: WKNavigation!) {
            if caught(webView.url) { webView.stopLoading() }
        }

        func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
            let url = (error as NSError).userInfo[NSURLErrorFailingURLErrorKey] as? URL
            if !caught(url) {
                KyberLog.app.notice("PlayStation : page de Sony pas chargée (\(error.localizedDescription, privacy: .public))")
            }
        }
    }
}
