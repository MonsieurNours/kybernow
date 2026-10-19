// Accueil : l'ordinateur principal en grand, les autres à côté, l'ajout.
// Fidèle à l'écran « 1 · Accueil » de la maquette.

import KyberKit
import SwiftUI

struct HomeView: View {
    @EnvironmentObject var app: AppModel
    
    var body: some View {
        ZStack(alignment: .topTrailing) {
            Theme.background.ignoresSafeArea()
            SignalBackground().ignoresSafeArea()
            VStack(alignment: .leading, spacing: 28) {
                header
                if let main = app.store.ordered.first {
                    HStack(alignment: .top, spacing: 20) {
                        FeaturedCard(host: main)
                            .frame(maxWidth: .infinity)
                            .layoutPriority(2)
                        VStack(spacing: 20) {
                            ForEach(app.store.ordered.dropFirst()) { HostTile(host: $0) }
                            AddTile { app.add() }
                        }
                        .frame(width: 340)
                    }
                    .frame(maxHeight: 470)
                } else {
                    EmptyState { app.add() }
                }
                Spacer(minLength: 0)
                footer
            }
            .padding(.horizontal, 56)
            .padding(.top, 36)
            .padding(.bottom, 18)
            if let toast = app.toast {
                Toast(text: toast).padding(24)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .sheet(isPresented: $app.adding) { AddHostSheet(initialLink: app.pendingLink) }
        .preferredColorScheme(.dark)
        .frame(minWidth: 1100, minHeight: 760)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button { app.add() } label: { Label("Ajouter un ordinateur", systemImage: "plus") }
            }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 12) {
                Mark(size: 44)
                Wordmark(size: 26)
            }
            Text("Sur quel ordinateur allez-vous ?")
                .font(Theme.display(44))
                .foregroundStyle(Theme.text)
            Text("Ajoutez un ordinateur équipé de KyberNow avec son adresse et le port de sa console. Par VPN, utilisez son adresse dans le VPN.")
                .font(Theme.sans(15))
                .foregroundStyle(Theme.muted)
                .frame(maxWidth: 620, alignment: .leading)
                .lineSpacing(3)
        }
    }

    private var footer: some View {
        HStack {
            Text("KyberNow · basé sur Kyber 0.29").font(Theme.sans(12)).foregroundStyle(Theme.faint)
            Spacer()
            Link("À propos", destination: URL(string: "https://www.leslignesclaires.fr")!)
                .font(Theme.sans(12))
                .foregroundStyle(Theme.muted)
        }
    }
}

// MARK: - Carte principale

private struct FeaturedCard: View {
    @EnvironmentObject var app: AppModel
    let host: Host

    private var status: HostStatus? { app.store.status[host.id] }

    private var profile: Binding<Profile?> {
        Binding(get: { host.settings.profile }, set: { p in
            guard let p else { return }
            var h = host
            h.settings.apply(p)
            app.store.update(h)
        })
    }

    private var screenLine: String {
        switch host.settings.display {
        case .physical: return "Écran de l'ordinateur tel quel, sans écran virtuel"
        case .window: return "Écran virtuel à la taille de la fenêtre"
        case .custom: return "Écran virtuel \(host.settings.customWidth) × \(host.settings.customHeight) à \(host.settings.customRefresh) Hz"
        case .match:
            let m = DisplayInfo.mode(for: NSScreen.main ?? NSScreen.screens[0])
            return "Écran virtuel \(m.width) × \(m.height) à \(m.refresh) Hz, comme cet écran"
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack {
                StatusPill(online: status?.online, text: status == nil ? "Recherche…" : status!.online ? "En ligne · réseau local" : "Hors ligne")
                Spacer()
                Button { app.route = .settings(host.id) } label: {
                    Label("Réglages", systemImage: "slider.horizontal.3").font(Theme.sans(13))
                }
                .buttonStyle(GhostButton(height: 34, tint: Theme.text2))
                Menu {
                    Button("Oublier cet ordinateur", role: .destructive) { app.store.remove(host) }
                } label: {
                    Image(systemName: "ellipsis").foregroundStyle(Theme.muted)
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .frame(width: 30)
            }

            HStack(spacing: 32) {
                MonitorArt().frame(width: 250)
                VStack(alignment: .leading, spacing: 10) {
                    Text(host.name).font(Theme.display(34)).foregroundStyle(Theme.text).lineLimit(1).minimumScaleFactor(0.6)
                    if let gpu = status?.info?.gpu {
                        Text(gpu).font(Theme.sans(14)).foregroundStyle(Theme.text2)
                    }
                    Text("\(host.address)\(host.streamPort.map { " · flux \($0)" } ?? "")")
                        .font(Theme.mono(13))
                        .foregroundStyle(Theme.muted)
                    Label(screenLine, systemImage: "display")
                        .font(Theme.sans(13))
                        .foregroundStyle(Theme.text2)
                        .labelStyle(AccentIconLabel())
                        .padding(.top, 4)
                }
            }

            VStack(alignment: .leading, spacing: 10) {
                Theme.eyebrow("Profil")
                HStack(spacing: 16) {
                    Segmented(options: Profile.allCases.map { ($0, $0.label) }, selection: profile, height: 36)
                        .fixedSize()
                    Text(host.settings.profile?.hint ?? "Personnalisé : vos propres réglages, dans Réglages.")
                        .font(Theme.sans(13))
                        .foregroundStyle(Theme.muted)
                        .frame(maxWidth: 380, alignment: .leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            Spacer(minLength: 0)

            HStack(spacing: 18) {
                Button { app.connect(host) } label: {
                    HStack(spacing: 10) {
                        Text("Se connecter")
                        Image(systemName: "arrow.right").font(.system(size: 15, weight: .bold))
                    }
                }
                .buttonStyle(PrimaryButton(height: 52))
                .disabled(app.session != nil)
                .keyboardShortcut(.defaultAction)
                if let last = host.lastSession {
                    Text("Dernière session : \(last.formatted(.relative(presentation: .named)))\(host.lastLatencyMs.map { String(format: " · %.0f ms de latence médiane", $0) } ?? "")")
                        .font(Theme.sans(13))
                        .foregroundStyle(Theme.faint)
                }
            }
        }
        .padding(28)
        .frame(height: 470)
        .background(Theme.surface.opacity(0.92), in: RoundedRectangle(cornerRadius: 18))
        .overlay(RoundedRectangle(cornerRadius: 18).stroke(Theme.line2))
    }
}

private struct AccentIconLabel: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 8) {
            configuration.icon.foregroundStyle(Theme.accent)
            configuration.title
        }
    }
}

// MARK: - Autres ordinateurs

private struct HostTile: View {
    @EnvironmentObject var app: AppModel
    let host: Host

    private var status: HostStatus? { app.store.status[host.id] }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Circle().fill(status?.online == true ? Theme.accent : Theme.rgb(0x5B6778)).frame(width: 8, height: 8)
                Text(status?.online == true ? "En ligne" : "Hors ligne").font(Theme.sans(13)).foregroundStyle(Theme.muted)
                Spacer()
                Button { app.route = .settings(host.id) } label: { Image(systemName: "slider.horizontal.3") }
                    .buttonStyle(.plain)
                    .foregroundStyle(Theme.muted)
                    .help("Réglages de connexion")
            }
            Text(host.name).font(Theme.display(22)).foregroundStyle(status?.online == true ? Theme.text : Theme.text2).lineLimit(1)
            Text(status?.info?.gpu ?? host.address).font(Theme.sans(13)).foregroundStyle(Theme.faint).lineLimit(1)
            Button(status?.online == true ? "Se connecter" : "Réessayer") {
                if status?.online == true { app.connect(host) } else { Task { await app.store.refresh(host) } }
            }
            .buttonStyle(GhostButton(height: 36, tint: status?.online == true ? Theme.accentLight : Theme.text2, border: status?.online == true ? Theme.accentLine : Theme.line2))
            .padding(.top, 6)
            .disabled(app.session != nil)
        }
        .padding(20)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.rail.opacity(0.9), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(Theme.line))
    }
}

private struct AddTile: View {
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 10) {
                    Image(systemName: "plus")
                        .font(.system(size: 14, weight: .bold))
                        .foregroundStyle(Theme.accent)
                        .frame(width: 30, height: 30)
                        .background(Theme.surface2, in: RoundedRectangle(cornerRadius: 9))
                    Text("Ajouter un ordinateur").font(Theme.sans(16, .semibold)).foregroundStyle(Theme.text)
                }
                Text("Par adresse et port de la console").font(Theme.sans(13)).foregroundStyle(Theme.muted)
            }
            .padding(20)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .overlay(RoundedRectangle(cornerRadius: 16).stroke(Theme.line2, style: StrokeStyle(lineWidth: 1, dash: [5, 4])))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .frame(minHeight: 110)
    }
}

private struct EmptyState: View {
    let action: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Aucun ordinateur pour l'instant.").font(Theme.sans(15)).foregroundStyle(Theme.muted)
            Button("Ajouter un ordinateur", action: action).buttonStyle(PrimaryButton())
        }
        .padding(28)
        .frame(maxWidth: 520, alignment: .leading)
        .background(Theme.surface.opacity(0.92), in: RoundedRectangle(cornerRadius: 18))
        .overlay(RoundedRectangle(cornerRadius: 18).stroke(Theme.line2))
    }
}

struct Toast: View {
    let text: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "checkmark")
                .font(.system(size: 12, weight: .bold))
                .foregroundStyle(Theme.accentInk)
                .frame(width: 24, height: 24)
                .background(Theme.accent, in: Circle())
            Text(text).font(Theme.sans(13)).foregroundStyle(Theme.text)
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .background(Theme.surface.opacity(0.95), in: RoundedRectangle(cornerRadius: 14))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.line2))
        .shadow(color: .black.opacity(0.45), radius: 20, y: 10)
    }
}
