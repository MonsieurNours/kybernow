// Réglages de l'app (docs/link-ui.md) : ce qui vaut sur cette Apple TV pour
// tous les ordinateurs : iCloud, langue, test des manettes, et ce que
// l'Apple TV sait faire (écran, décodage, sortie du son). Les réglages de
// connexion, eux, vont par ordinateur (SettingsView).

import AVFoundation
import KyberKit
import SwiftUI

struct AppSettingsView: View {
    @EnvironmentObject private var app: AppModel
    @EnvironmentObject private var cloud: CloudSync
    @EnvironmentObject private var pads: GamepadBridge
    @ObservedObject private var prefs = Preferences.shared
    /// Langue choisie ici ; celle du lancement reste affichée jusqu'à la relance.
    @State private var language = LanguageChoice.current
    private static let launchLanguage = LanguageChoice.current
    @State private var route = AudioRoute.current
    @FocusState private var focus: Target?

    enum Target: Hashable { case back, row(String) }

    var body: some View {
        ZStack(alignment: .topLeading) {
            AmbientBackground()
            Button { app.screen = .home } label: {
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

            VStack(alignment: .leading, spacing: 14) {
                Text("Réglages de l'app").font(Theme.display(64, 660)).tracking(-64 * 0.035)
                Text("Pour tous vos ordinateurs, sur cette Apple TV. L'image, le son et les commandes se règlent par ordinateur.")
                    .font(Theme.sans(22)).foregroundStyle(Theme.muted)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.bottom, 18)
                icloudRow
                languageRow
                padsRow
                versionRow
            }
            .focusSection()
            .frame(width: 1040, alignment: .leading)
            .padding(.leading, 96).padding(.top, 170)

            deviceCard
                .frame(width: 1920 - 1040 - 96 - 96 - 60)
                .padding(.leading, 96 + 1040 + 60).padding(.top, 290)
        }
        .frame(width: 1920, height: 1080, alignment: .topLeading)
        .ignoresSafeArea()
        .defaultFocus($focus, .row("icloud"))
        .onExitCommand { app.screen = .home }
        .onReceive(NotificationCenter.default.publisher(for: AVAudioSession.routeChangeNotification).receive(on: RunLoop.main)) { _ in
            route = AudioRoute.current
        }
    }

    // MARK: Lignes

    private var icloudRow: some View {
        let n = cloud.hosts.count
        let note = !prefs.icloudSync ? String(localized: "Vos ordinateurs et leurs accès suivent votre Mac et votre compte iCloud.")
            : n == 0 ? String(localized: "Aucun ordinateur dans iCloud pour l'instant.")
            : n == 1 ? String(localized: "1 ordinateur dans iCloud.")
            : String(localized: "\(n) ordinateurs dans iCloud.")
        return Button {
            prefs.icloudSync.toggle()
            // Activée : ceux déjà publiés arrivent tout de suite.
            if prefs.icloudSync { for c in cloud.hosts { _ = app.addFromCloud(c) } }
        } label: {
            HStack(spacing: 24) {
                label(String(localized: "Synchroniser avec iCloud"), note)
                Toggle2(on: prefs.icloudSync)
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 88)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row("icloud"))
        .accessibilityValue(prefs.icloudSync ? Text("Activé") : Text("Désactivé"))
    }

    // Langue de l'app (docs/i18n.md) : prise en compte à la relance.
    private var languageRow: some View {
        HStack(spacing: 24) {
            label(String(localized: "Langue"),
                  language == Self.launchLanguage ? String(localized: "Auto : la langue de l'Apple TV ; pour KyberNow Link seulement") : String(localized: "Relancez KyberNow Link pour changer de langue."),
                  noteColor: language == Self.launchLanguage ? Theme.muted : Theme.warnText)
            HStack(spacing: 8) {
                ForEach(LanguageChoice.allCases) { choice in
                    Button(choice.label) {
                        language = choice
                        LanguageChoice.current = choice
                    }
                    .buttonStyle(ChoiceButton(selected: language == choice, height: 60, light: true))
                    .focused($focus, equals: .row("language.\(choice.rawValue)"))
                }
            }
            .fixedSize()
            .focusSection()
        }
        .padding(.horizontal, 24).padding(.vertical, 10)
        .frame(minHeight: 88)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.04)))
    }

    private var padsRow: some View {
        Button { app.showGamepads(from: .appSettings) } label: {
            HStack(spacing: 24) {
                label(String(localized: "Tester les manettes"), padsSummary)
                Icon("arrow-right", size: 22, bold: true).foregroundStyle(Theme.faint)
            }
            .padding(.horizontal, 24).padding(.vertical, 10)
            .frame(minHeight: 88)
        }
        .buttonStyle(GlassButton(radius: 18, scale: 1.03, fill: Color.white.opacity(0.04)))
        .focused($focus, equals: .row("pads"))
    }

    private var padsSummary: String {
        let n = pads.pads.count
        let waiting = pads.pads.filter(\.waiting).count
        var text = LinkText.controllers(n)
        if waiting > 0 { text += " " + String(localized: "(\(waiting) en attente : quatre au plus sur l'ordinateur)") }
        if pads.remoteConnected { text += ", " + String(localized: "1 télécommande") }
        return text
    }

    private var versionRow: some View {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? "—"
        let build = info?["CFBundleVersion"] as? String ?? ""
        return HStack(spacing: 24) {
            label(String(localized: "Version"), String(localized: "KyberNow Link pour l'Apple TV"))
            Text(verbatim: build.isEmpty ? version : "\(version) (\(build))").font(Theme.mono(19)).foregroundStyle(Theme.text2)
        }
        .padding(.horizontal, 24).padding(.vertical, 10)
        .frame(minHeight: 88)
        .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(Color.white.opacity(0.03)))
    }

    // MARK: Cette Apple TV

    /// Ce que l'Apple TV sait faire : les profils s'y adaptent.
    private var deviceCard: some View {
        let n = TVDisplay.native
        let caps = DecoderCapabilities.current
        return VStack(alignment: .leading, spacing: 18) {
            Theme.eyebrow(String(localized: "Cette Apple TV"))
            fact(String(localized: "Écran"), String(localized: "\(LinkFormat.dims(n.width, n.height)) à \(n.refresh) Hz"))
            fact(String(localized: "Décodage matériel"), caps.summary)
            fact(String(localized: "Texte net (4:4:4)"), caps.hevc444 ? String(localized: "décodé") : String(localized: "non décodé"))
            fact(String(localized: "Sortie du son"), "\(route.name) · \(route.channelsLabel)")
            fact(String(localized: "Retard mesuré de la sortie"), "\(route.latencyMs) ms", warn: route.isLate)
        }
        .padding(.horizontal, 32).padding(.vertical, 30)
        .frame(maxWidth: .infinity, alignment: .leading)
        .glass(26)
    }

    private func fact(_ title: String, _ value: String, warn: Bool = false) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(Theme.sans(18)).foregroundStyle(Theme.muted)
            Text(verbatim: value).font(Theme.mono(21)).foregroundStyle(warn ? Theme.warnText : Theme.text)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func label(_ title: String, _ note: String, noteColor: Color = Theme.muted) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(Theme.sans(24, .semibold)).foregroundStyle(Theme.text)
            Text(note).font(Theme.sans(17)).foregroundStyle(noteColor).lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
