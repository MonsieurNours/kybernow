// KyberNow Share, côté hôte (docs/link-ui.md, « Partage ») : « Partager » dans
// la barre de session (présenté comme les autres menus de la barre) ouvre le
// partage de l'écran principal et son panneau : le code, « Copier le lien »,
// le QR code, les invités et ce qu'on peut faire de chacun, « Nouveau code »,
// « Arrêter le partage ». Un partage ouvert se voit dans la barre (pastille
// « 2 invités », qui rouvre le panneau). Le partage est propre à la session :
// il est dans `SessionController.share`, mis à jour par le signe de vie du cœur.

import AppKit
import KyberKit
import SwiftUI

/// « Partager » dans la barre : un bouton et son panneau, en verre.
struct ShareBarButton: View {
    @ObservedObject var session: SessionController
    @ObservedObject var share: ShareHostModel
    /// Barre étroite : l'icône seule (et le nombre d'invités).
    var compact = false
    @State private var open = false
    @State private var hovering = false

    private var availability: ShareAvailability { session.shareAvailability }
    private var count: Int { share.guests.count }

    var body: some View {
        Button {
            guard availability.isAvailable else { return }
            open.toggle()
            if open { Task { await share.openIfNeeded() } }
        } label: {
            HStack(spacing: 7) {
                Icon("share-network", size: 16, bold: true).foregroundStyle(share.isOpen ? Theme.accent : Theme.text2)
                if !compact {
                    Text(ShareText.share).font(Theme.sans(13)).foregroundStyle(Theme.text2).lineLimit(1).fixedSize()
                }
                if count > 0 {
                    Text(compact ? "\(count)" : ShareText.guestCount(count))
                        .font(Theme.sans(11.5, .semibold)).foregroundStyle(Theme.text).lineLimit(1).fixedSize()
                        .padding(.horizontal, 8).frame(height: 20)
                        .background(Theme.accentSoft, in: Capsule())
                        .overlay(Capsule().strokeBorder(Theme.accentLine))
                }
            }
            .padding(.horizontal, compact ? 8 : 10)
            .frame(height: 34)
            .background(share.isOpen ? Theme.accentSoft : Color.white.opacity(hovering || open ? 0.07 : 0), in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(Theme.accentLine.opacity(share.isOpen ? 1 : 0)))
            .opacity(availability.isAvailable ? 1 : 0.45)
            .contentShape(Rectangle())
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text(count > 0 ? "\(ShareText.share), \(ShareText.guestCount(count))" : ShareText.share))
        }
        .buttonStyle(.plain)
        .disabled(!availability.isAvailable)
        .onHover { hovering = $0 }
        // Inactif : la raison (Station trop ancienne, partage coupé sur l'ordinateur).
        .help(availability.isAvailable ? "" : (unavailableText ?? ""))
        // La barre reste ouverte tant que le panneau l'est.
        .preference(key: GlassMenuOpenKey.self, value: open)
        .popover(isPresented: $open, arrowEdge: .bottom) {
            SharePanel(session: session, share: share)
                .preferredColorScheme(.dark)
        }
        .onChange(of: availability) { _, now in
            if !now.isAvailable { open = false }
        }
    }

    private var unavailableText: String? {
        if case let .unavailable(text) = availability { return text }
        return nil
    }
}

/// Le panneau de l'hôte : code, lien, QR code, invités, actions.
struct SharePanel: View {
    @ObservedObject var session: SessionController
    @ObservedObject var share: ShareHostModel
    /// « Lien copié. » ou « Copie impossible… », quelques secondes.
    @State private var copyResult: String?
    @State private var copied = false
    @State private var qr: (link: String, image: CGImage)?

    private static let width: CGFloat = 372

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(ShareText.title(session.host.name))
                .font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text).lineLimit(2)
            if let state = share.state {
                shared(state)
            } else if let error = share.error {
                Notice(kind: .danger, text: error)
                retry
            } else {
                ProgressView().controlSize(.small)
            }
        }
        .padding(20)
        .frame(width: Self.width, alignment: .leading)
    }

    private var retry: some View {
        Button(ShareText.share) { Task { await share.openIfNeeded() } }
            .buttonStyle(PrimaryButton(height: 36))
            .disabled(share.busy)
    }

    @ViewBuilder private func shared(_ state: ShareState) -> some View {
        HStack(alignment: .top, spacing: 16) {
            qrImage(for: state.link)
            VStack(alignment: .leading, spacing: 8) {
                Theme.eyebrow(ShareText.code, color: Theme.faint)
                Text(Share.formatCode(state.code))
                    .font(Theme.mono(24, .medium)).foregroundStyle(Theme.accentLight)
                    .fixedSize()
                    .textSelection(.enabled)
                Button { copyLink() } label: {
                    HStack(spacing: 8) {
                        Icon(copied ? "check" : "copy", size: 14, bold: true)
                        Text(ShareText.copyLink)
                    }
                    .font(Theme.sans(13))
                }
                .buttonStyle(GhostButton(height: 32))
                if let copyResult {
                    Text(copyResult).font(Theme.sans(12)).foregroundStyle(copied ? Theme.ok : Theme.warnText)
                        .fixedSize(horizontal: false, vertical: true)
                    // Copie impossible : l'adresse à sélectionner à la main.
                    if !copied, let link = share.linkToCopy {
                        Text(verbatim: link).font(Theme.mono(11)).foregroundStyle(Theme.text2)
                            .textSelection(.enabled)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
        Text(ShareText.help).font(Theme.sans(12.5)).foregroundStyle(Theme.muted)
            .fixedSize(horizontal: false, vertical: true)
        Rectangle().fill(Theme.line).frame(height: 1)
        VStack(alignment: .leading, spacing: 10) {
            Theme.eyebrow(ShareText.guests, color: Theme.faint)
            if state.guests.isEmpty {
                Text(ShareText.nobody).font(Theme.sans(13)).foregroundStyle(Theme.muted)
            } else {
                ForEach(state.guests) { guest in GuestRow(share: share, guest: guest) }
            }
            if share.full {
                Text(ShareText.fourGuests).font(Theme.sans(12)).foregroundStyle(Theme.faint)
            }
        }
        if let error = share.error { Notice(kind: .danger, text: error) }
        HStack(spacing: 10) {
            Button(ShareText.newCode) { Task { await share.newCode() } }
                .buttonStyle(GhostButton(height: 34))
            Spacer(minLength: 0)
            Button(ShareText.stop) { Task { await share.close() } }
                .buttonStyle(GhostButton(height: 34, tint: Theme.dangerText, border: Theme.danger.opacity(0.4)))
        }
        .disabled(share.busy)
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
                    .frame(width: 128, height: 128)
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .accessibilityElement()
                    .accessibilityLabel(ShareText.qrLabel)
                    .onAppear { if qr?.link != link { qr = (link, image) } }
            } else {
                RoundedRectangle(cornerRadius: 8, style: .continuous).fill(Theme.well).frame(width: 128, height: 128)
            }
        }
    }

    private func copyLink() {
        guard let text = share.linkToCopy else { return }
        let board = NSPasteboard.general
        board.clearContents()
        let ok = board.setString(text, forType: .string)
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

#if DEBUG
/// Démonstration : le panneau de l'hôte et l'écran « Rejoindre » dessinés dans
/// un PNG, sans fenêtre à l'écran (comme `-demoSnapshot`).
@MainActor
enum SharePanelDemo {
    static func render(session: SessionController, to path: String) {
        let view = SharePanel(session: session, share: session.share)
            .background(Theme.surface)
            .environment(\.colorScheme, .dark)
        write(view, width: 372, to: path)
    }

    /// Les textes des deux droits en clair, pour les relire sans fenêtre : pour
    /// chacun des quatre rôles, le libellé de l'invité, ce que dit la barre,
    /// son icône et les boutons de l'hôte (avec le rôle que chacun demande) ;
    /// puis l'avis de chacun des douze changements de droits.
    static func texts() -> String {
        let roles: [ShareRole] = [.spectator, .player, .kbm, .playerKbm]
        var lines: [String] = []
        for role in roles {
            lines.append("\(ShareText.guest("Léa", role)) | barre : \(ShareText.role(role)) | icône : \(ShareText.icon(role))")
            for action in ShareHostModel.roleActions(for: role) { lines.append("    \(action.title) → \(role.with(action.right, on: action.on))") }
        }
        lines.append("")
        for from in roles {
            for to in roles where from != to {
                lines.append("\(from) → \(to) : \(ShareText.roleNotice(to, previous: from) ?? "(aucun avis)")")
            }
        }
        return lines.joined(separator: "\n")
    }

    static func renderJoin(to path: String, app: AppModel) {
        write(JoinSheet(initialLink: nil).environmentObject(app), width: 560, to: path)
    }

    /// Le panneau « Partager » d'une session d'hôte sans fenêtre (le premier
    /// ordinateur de l'accueil), avec ou sans invités.
    static func renderPanel(app: AppModel, guests: Bool, to path: String) async {
        guard let host = app.store.ordered.first else { return }
        let session = SessionController(host: host, store: app.store, opening: .first(home: nil)) { _ in }
        session.share.demo = true
        await session.share.openIfNeeded()
        if guests {
            // Léa et Sam reçoivent une image à part (leur qualité se voit), Hugo suit
            // l'hôte, Zoé n'a rien de su encore : seules les images à part se montrent.
            session.share.demoAddGuest(name: "Léa", app: "ios", role: .player, image: ShareImage.demo("1080p60", 1920, 1080, 60, follows: false))
            session.share.demoAddGuest(name: "Hugo", app: "tvos", image: ShareImage.demo("host", 2560, 1440, 120, follows: true))
            session.share.demoAddGuest(name: "Sam", app: "web", role: .playerKbm, image: ShareImage.demo("720p30", 1280, 720, 30, follows: false))
            session.share.demoAddGuest(name: "Zoé", app: "mac", role: .kbm, image: ShareImage.demo("auto"))
        }
        render(session: session, to: path)
    }

    /// La session d'un invité sans fenêtre à un moment donné : `end` (retiré du
    /// partage : la raison reste dans la capsule), `lag` ou `limit` (l'avis de
    /// la Station sur l'image : plus légère, ou plus d'image à part possible),
    /// puis l'avis bref d'un changement de droits : `play` (spectateur à qui
    /// l'hôte donne les
    /// manettes), `kbm` (joueur à qui il donne le clavier et la souris),
    /// `watch` (joueur ramené en spectateur), `unpads` (joueur avec clavier et
    /// souris, manettes reprises), `unkbm` (le clavier et la souris repris).
    static func renderGuestMoment(_ moment: String, app: AppModel, to path: String) async {
        let (start, next): (ShareRole, ShareRole) = switch moment {
        case "lag", "limit": (.player, .player)
        case "play": (.spectator, .player)
        case "kbm": (.player, .playerKbm)
        case "watch": (.player, .spectator)
        case "unpads": (.playerKbm, .kbm)
        case "unkbm": (.playerKbm, .player)
        default: (.player, .player)
        }
        let session = SessionController(guest: ShareFound(demoComputer: "PC-DE-MATTHIEU", host: "Matthieu", role: start), store: app.store,
                                        opening: .first(home: nil)) { _ in }
        session.demoHeadless()
        if moment == "end" {
            session.demoShareEnded(removed: true)
        } else if moment == "lag" || moment == "limit" {
            // L'avis de la Station sur l'image : en retard (plus légère), ou plus d'image à part possible.
            session.setBar(false)
            let shown = moment == "lag" ? ShareImage.demo("1080p60", 1920, 1080, 30, follows: false) : ShareImage.demo("host", 1920, 1080, 60, follows: false)
            let notice = moment == "lag" ? "Image allégée pour suivre." : "L'ordinateur ne peut pas faire une image de plus : vous recevez 1080p · 60 Hz."
            session.demoShareImage(shown, notice: notice)
        } else {
            session.setBar(false)
            session.demoRoleChanged(next)
        }
        // L'avis s'affiche au tour suivant de la boucle.
        try? await Task.sleep(for: .milliseconds(700))
        let view = ZStack(alignment: .top) {
            Color(white: 0.12)
            SessionChrome(session: session)
        }
        .frame(width: 1200, height: 420)
        write(view, width: 1200, to: path)
    }

    /// La barre de session, ouverte, d'une session sans fenêtre : l'hôte
    /// (`host`, deux invités), ou un invité (`spectator`, `player`, `kbm`, `player_kbm`).
    static func renderBar(kind: String, app: AppModel, to path: String) async {
        let session: SessionController
        if kind == "host" {
            guard let host = app.store.ordered.first else { return }
            session = SessionController(host: host, store: app.store, opening: .first(home: nil)) { _ in }
            session.share.demo = true
            await session.share.openIfNeeded()
            session.share.demoAddGuest(name: "Léa", app: "ios", role: .player)
            session.share.demoAddGuest(name: "Hugo", app: "tvos")
        } else {
            let role: ShareRole = switch kind {
            case "player": .player
            case "kbm": .kbm
            case "player_kbm": .playerKbm
            default: .spectator
            }
            session = SessionController(guest: ShareFound(demoComputer: "PC-DE-MATTHIEU", host: "Matthieu", role: role), store: app.store,
                                        opening: .first(home: nil)) { _ in }
        }
        session.demoHeadless()
        // Un invité : la qualité reçue dans la barre (« 1080p · 60 Hz »).
        if kind != "host" { session.demoShareImage(ShareImage.demo("1080p60", 1920, 1080, 60, follows: false)) }
        let view = ZStack(alignment: .top) {
            Color(white: 0.12)
            SessionChrome(session: session)
        }
        .frame(width: 1500, height: 170)
        write(view, width: 1500, to: path)
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

/// Un invité : son prénom et son rôle, son app et depuis quand, et ce que
/// l'hôte peut en faire (rôle, « Retirer »).
private struct GuestRow: View {
    @ObservedObject var share: ShareHostModel
    let guest: ShareGuest

    var body: some View {
        HStack(alignment: .center, spacing: 10) {
            Icon(ShareText.icon(guest.role), size: 16, bold: true)
                .foregroundStyle(guest.role.plays ? Theme.accent : Theme.faint)
                .frame(width: 20)
            VStack(alignment: .leading, spacing: 2) {
                Text(ShareText.guest(guest.name, guest.role)).font(Theme.sans(13.5, .medium)).foregroundStyle(Theme.text)
                    .lineLimit(2).fixedSize(horizontal: false, vertical: true)
                TimelineView(.periodic(from: .now, by: 20)) { _ in
                    Text(detail).font(Theme.sans(12)).foregroundStyle(Theme.faint)
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
                Icon("dots-three", size: 18, bold: true).foregroundStyle(Theme.text2).frame(width: 30, height: 28)
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
            .disabled(share.busy)
            .accessibilityLabel(guest.name)
        }
    }

    /// « Mac · 1080p · 60 Hz · depuis 3 min » : sa qualité d'image seulement si
    /// ce n'est pas celle de l'hôte.
    private var detail: String { ShareText.guestDetail(guest) }
}
