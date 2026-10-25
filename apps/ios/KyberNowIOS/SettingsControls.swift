// Briques des réglages d'un ordinateur : lignes à choix, à bascule, à valeur,
// le sélecteur de profil, et le lien réseau de l'appareil. Une
// ligne se pose en largeur (iPad, détail large) ou en hauteur (iPhone).

import KyberKit
import Network
import SwiftUI

// MARK: - Options

/// Un choix d'une ligne : sa clé, son texte, et s'il est impossible ici.
struct SettingsOption: Identifiable {
    let key: String
    let label: String
    var unavailable = false
    var id: String { key }
}

/// Choix d'une ligne : givre plein quand il est retenu, barré et éteint
/// quand l'appareil ne sait pas le faire.
struct SettingsOptionStyle: ButtonStyle {
    var selected: Bool
    var unavailable: Bool
    var large: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(Theme.sans(large ? 14 : 13.5, .semibold))
            .foregroundStyle(selected ? Theme.background : unavailable ? Theme.ghost : Theme.text2)
            .strikethrough(unavailable && !selected, color: Theme.ghost)
            .lineLimit(1)
            .padding(.horizontal, large ? 16 : 14)
            .frame(minWidth: 44, minHeight: 44)
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(selected ? Theme.text : unavailable ? Color.clear : Color.white.opacity(0.08))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(unavailable && !selected ? Color.white.opacity(0.08) : Color.clear)
            )
            .opacity(configuration.isPressed ? 0.75 : 1)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
            .contentShape(Rectangle())
    }
}

/// Interrupteur dessiné à la charte : piste Spectre quand il est actif.
struct SettingsSwitch: View {
    let on: Bool
    var enabled = true

    var body: some View {
        Capsule()
            .fill(on && enabled ? Theme.accentFill : Color.white.opacity(0.18))
            .frame(width: 50, height: 30)
            .overlay(alignment: on ? .trailing : .leading) {
                Circle().fill(Color.white.opacity(enabled ? 1 : 0.5)).frame(width: 24, height: 24).padding(3)
            }
            .animation(.easeOut(duration: 0.18), value: on)
            .accessibilityHidden(true)
    }
}

/// Rangée de boutons qui passe à la ligne quand la place manque.
struct SettingsFlow: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let maxWidth = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, rowHeight: CGFloat = 0, widest: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > 0, x + size.width > maxWidth {
                y += rowHeight + spacing
                x = 0
                rowHeight = 0
            }
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
            widest = max(widest, x - spacing)
        }
        return CGSize(width: proposal.width ?? widest, height: y + rowHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x: CGFloat = 0, y: CGFloat = 0, rowHeight: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > 0, x + size.width > bounds.width {
                y += rowHeight + spacing
                x = 0
                rowHeight = 0
            }
            view.place(at: CGPoint(x: bounds.minX + x, y: bounds.minY + y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
    }
}

// MARK: - Lignes

/// Fabrique des lignes, en largeur (`wide`) ou en hauteur.
struct SettingsRows {
    let wide: Bool

    private var radius: CGFloat { 14 }

    func container<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        content()
            .padding(.horizontal, wide ? 18 : 14)
            .padding(.vertical, wide ? 10 : 10)
            .frame(maxWidth: .infinity, minHeight: wide ? 68 : 60, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(Color.white.opacity(wide ? 0.04 : 0.045)))
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(Color.white.opacity(wide ? 0 : 0.07)))
    }

    func label(_ title: String, _ note: String, color: Color = Theme.text, noteColor: Color = Theme.muted) -> some View {
        VStack(alignment: .leading, spacing: wide ? 3 : 2) {
            Text(title).font(Theme.sans(wide ? 16 : 15, .semibold)).foregroundStyle(color)
            if !note.isEmpty {
                Text(note)
                    .font(Theme.sans(wide ? 12.5 : 12))
                    .foregroundStyle(noteColor)
                    .lineSpacing(1.5)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Une ligne à choix ; un choix impossible reste visible, grisé.
    func choice(_ title: String, _ note: String, _ options: [SettingsOption], _ current: String,
                noteColor: Color = Theme.muted, enabled: Bool = true, pick: @escaping (String) -> Void) -> some View {
        let buttons = ForEach(options) { o in
            Button { pick(o.key) } label: { Text(o.label) }
                .buttonStyle(SettingsOptionStyle(selected: current == o.key, unavailable: o.unavailable || !enabled, large: wide))
                .disabled(o.unavailable || !enabled)
                .accessibilityAddTraits(current == o.key ? .isSelected : [])
        }
        return container {
            if wide {
                HStack(spacing: 20) {
                    label(title, note, noteColor: noteColor)
                    HStack(spacing: 6) { buttons }.fixedSize()
                }
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    label(title, note, noteColor: noteColor)
                    SettingsFlow(spacing: 6) { buttons }
                }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text(title))
    }

    /// Une ligne à bascule : toute la ligne se touche.
    func toggle(_ title: String, _ note: String, _ on: Bool, enabled: Bool = true, noteColor: Color = Theme.muted, flip: @escaping () -> Void) -> some View {
        Button(action: flip) {
            container {
                HStack(spacing: wide ? 20 : 12) {
                    label(title, note, color: enabled ? Theme.text : Theme.faint, noteColor: noteColor)
                    SettingsSwitch(on: on, enabled: enabled)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(SettingsPressStyle())
        .disabled(!enabled)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(title))
        .accessibilityHint(Text(note))
        .accessibilityValue(on ? Text("Activé") : Text("Désactivé"))
        .accessibilityAddTraits(.isToggle)
    }

    /// Une ligne qui montre une valeur ; touchable si elle mène quelque part.
    @ViewBuilder
    func value(_ title: String, _ note: String, _ value: String, color: Color = Theme.text, noteColor: Color = Theme.muted, action: (() -> Void)? = nil) -> some View {
        let row = container {
            HStack(spacing: wide ? 20 : 12) {
                label(title, note, color: color, noteColor: noteColor)
                if !value.isEmpty {
                    Text(verbatim: value)
                        .font(Theme.mono(wide ? 13.5 : 13))
                        .foregroundStyle(Theme.text2)
                        .lineLimit(1)
                        .layoutPriority(1)
                }
                if action != nil {
                    Icon("arrow-right", size: 16, bold: true).foregroundStyle(Theme.ghost)
                }
            }
        }
        if let action {
            Button(action: action) { row.contentShape(Rectangle()) }
                .buttonStyle(SettingsPressStyle())
        } else {
            row.accessibilityElement(children: .combine)
        }
    }
}

/// Appui sur une ligne entière : léger enfoncement.
struct SettingsPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.985 : 1)
            .opacity(configuration.isPressed ? 0.85 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

// MARK: - Profil

/// Jeu, Bureau, Max, Personnalisé : un réglage ajusté passe en Personnalisé,
/// qu'on ne choisit pas soi-même.
struct SettingsProfilePicker: View {
    let current: Profile?
    /// Largeur répartie entre les quatre (iPhone) ou au plus juste (iPad).
    let fill: Bool
    let pick: (Profile) -> Void

    var body: some View {
        HStack(spacing: 3) {
            ForEach(Array((Profile.allCases.map(Optional.some) + [nil]).enumerated()), id: \.offset) { _, p in
                let on = current == p
                Button { if let p { pick(p) } } label: {
                    Text(Profile.label(p))
                        .font(Theme.sans(fill ? 13 : 14, .semibold))
                        .foregroundStyle(on ? Theme.text : p == nil ? Theme.faint : Theme.muted)
                        .lineLimit(1)
                        .minimumScaleFactor(0.75)
                        .padding(.horizontal, fill ? 4 : 14)
                        .frame(maxWidth: fill ? .infinity : nil, minHeight: 44)
                        .background {
                            if on {
                                RoundedRectangle(cornerRadius: 9, style: .continuous)
                                    .fill(Color.white.opacity(0.12))
                                    .overlay(alignment: .bottom) {
                                        Rectangle().fill(Theme.accent).frame(height: 2)
                                    }
                                    .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
                            }
                        }
                        .contentShape(Rectangle())
                }
                .buttonStyle(SettingsPressStyle())
                .disabled(p == nil || on)
                .accessibilityAddTraits(on ? .isSelected : [])
                .accessibilityLabel(Text(Profile.label(p)))
            }
        }
        .padding(4)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.white.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Color.white.opacity(0.08)))
        .accessibilityElement(children: .contain)
        .accessibilityLabel(Text("Profil"))
    }
}

// MARK: - Lien réseau

/// Le lien de l'appareil en ce moment : Ethernet, Wi-Fi ou données mobiles.
@MainActor
final class SettingsLink: ObservableObject {
    enum Kind { case unknown, ethernet, wifi, cellular, offline }

    @Published private(set) var kind: Kind = .unknown
    private let monitor = NWPathMonitor()

    init() {
        monitor.pathUpdateHandler = { [weak self] path in
            let kind: Kind = path.status != .satisfied ? .offline
                : path.usesInterfaceType(.wiredEthernet) ? .ethernet
                : path.usesInterfaceType(.wifi) ? .wifi
                : path.usesInterfaceType(.cellular) ? .cellular : .unknown
            Task { @MainActor in self?.kind = kind }
        }
        monitor.start(queue: DispatchQueue(label: "fr.leslignesclaires.kybernow.settings-link"))
    }

    deinit { monitor.cancel() }

    var label: String {
        switch kind {
        case .unknown: return String(localized: "Réseau")
        case .ethernet: return "Ethernet"
        case .wifi: return "Wi-Fi"
        case .cellular: return String(localized: "Données mobiles")
        case .offline: return String(localized: "Hors réseau")
        }
    }
}
