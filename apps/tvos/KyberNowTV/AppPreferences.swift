// Les préférences de l'app communes à l'iPhone, à l'iPad et à l'Apple TV
// (audit IOS-23), gardées sur cet appareil : chaque app y ajoute les siennes
// (`Preferences`, dans son Platform.swift). Compilé par les deux apps
// (apps/ios/project.yml le reprend).

import Foundation

class AppPreferences: ObservableObject {
    let d = UserDefaults.standard

    /// L1 + R1 + Start + Select ouvre la barre (le menu sur l'Apple TV).
    @Published var menuCombo: Bool { didSet { d.set(menuCombo, forKey: "menuCombo") } }
    /// Les vibrations renvoyées par le jeu, sur chaque manette.
    @Published var rumble: Bool { didSet { d.set(rumble, forKey: "rumble") } }
    /// Audio spatial : des AirPods reçoivent le 5.1 et le 7.1.
    @Published var spatialAudio: Bool { didSet { d.set(spatialAudio, forKey: "spatialAudio") } }
    @Published var latencyAlert: Bool { didSet { d.set(latencyAlert, forKey: "latencyAlert") } }
    @Published var icloudSync: Bool { didSet { d.set(icloudSync, forKey: "icloudSync") } }
    /// Clavier Apple ISO (européen) branché : touches `@#` et `<>` échangées
    /// (Réglages de l'app › Clavier : il décrit le clavier, pas l'ordinateur).
    @Published var appleISOKeyboard: Bool { didSet { d.set(appleISOKeyboard, forKey: "appleISOKeyboard") } }

    /// Une préférence gardée, ou sa valeur par défaut (allumée sauf mention contraire).
    static func flag(_ key: String, _ fallback: Bool = true) -> Bool {
        UserDefaults.standard.object(forKey: key) as? Bool ?? fallback
    }

    init() {
        menuCombo = Self.flag("menuCombo")
        rumble = Self.flag("rumble")
        spatialAudio = Self.flag("spatialAudio")
        latencyAlert = Self.flag("latencyAlert")
        icloudSync = Self.flag("icloudSync")
        appleISOKeyboard = Self.flag("appleISOKeyboard", false)
    }
}
