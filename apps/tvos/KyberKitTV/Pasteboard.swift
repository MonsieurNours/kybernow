// Presse-papiers de la plateforme : tvOS n'en a pas, le presse-papiers
// partagé (ClipboardBridge, KyberKit) n'est jamais créé ici.

enum Pasteboard {
    static var general: PlatformPasteboard? { nil }
}
