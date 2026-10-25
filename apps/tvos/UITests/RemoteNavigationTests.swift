// Parcours à la télécommande : ce qu'on fait sur l'Apple TV, appui par appui.

import XCTest

final class RemoteNavigationTests: XCTestCase {
    private let remote = XCUIRemote.shared

    private func launch(_ args: [String] = []) -> XCUIApplication {
        let app = XCUIApplication()
        // Libellés cherchés en français, quelle que soit la langue du simulateur.
        app.launchArguments = ["-demo", "YES", "-AppleLanguages", "(fr)"] + args
        app.launch()
        return app
    }

    /// Le bouton qui a le focus.
    private func focused(_ app: XCUIApplication) -> XCUIElement {
        app.descendants(matching: .any).element(matching: NSPredicate(format: "hasFocus == true"))
    }

    /// De « Se connecter », à droite après les profils : les réglages de l'ordinateur.
    func testSettingsReachableFromHome() {
        let app = launch()
        let label = "Réglages de Tour du bureau"
        XCTAssertTrue(app.buttons[label].waitForExistence(timeout: 5))
        sleep(1)
        for _ in 0..<4 { remote.press(.right) }
        XCTAssertEqual(focused(app).label, label)
        remote.press(.select)
        XCTAssertTrue(app.staticTexts["Statistiques"].waitForExistence(timeout: 3))
    }

    /// Réglages de l'ordinateur : de chaque rubrique, du bas de la liste
    /// (Station, Statistiques) comme du haut, la droite entre dans le panneau
    /// par sa première ligne, même si ses lignes sont plus haut ; la gauche,
    /// puis Retour, ramènent à la même rubrique.
    func testBottomSectionsReachTheirPanel() {
        let cases = [
            ("station", "Station", "Redémarrer la Station"), ("statistics", "Statistiques", "Système"),
            ("network", "Réseau", "Rapide"), ("controls", "Commandes", "Télécommande en souris"),
            ("sound", "Son", "Auto"), ("picture", "Image", "Comme cet écran"),
        ]
        for (section, title, row) in cases {
            let app = launch(["-screen", "settings", "-section", section])
            XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", title)).firstMatch.waitForExistence(timeout: 5))
            sleep(1)
            XCTAssertTrue(focused(app).label.hasPrefix(title), "focus de départ sur la rubrique \(title) : \(focused(app).label)")
            remote.press(.right)
            sleep(1)
            XCTAssertTrue(focused(app).label.hasPrefix(row), "\(title), droite : le focus doit entrer dans le panneau (\(focused(app).label))")
            remote.press(.left)
            sleep(1)
            XCTAssertTrue(focused(app).label.hasPrefix(title), "\(title), gauche : retour à la rubrique (\(focused(app).label))")
            remote.press(.right)
            sleep(1)
            remote.press(.menu)
            sleep(1)
            XCTAssertTrue(focused(app).label.hasPrefix(title), "\(title), Retour depuis le panneau : retour à la rubrique (\(focused(app).label))")
            app.terminate()
        }
    }

    /// Réglages de l'app : la droite ne quitte pas la liste (la carte « Cette
    /// Apple TV » n'a rien à choisir), et Retour ramène à l'accueil.
    func testAppSettingsKeepFocusInTheList() {
        let app = launch(["-screen", "app"])
        XCTAssertTrue(app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "Synchroniser avec iCloud")).firstMatch.waitForExistence(timeout: 5))
        sleep(1)
        XCTAssertTrue(focused(app).label.hasPrefix("Synchroniser avec iCloud"), focused(app).label)
        for _ in 0..<2 { remote.press(.down) }
        sleep(1)
        XCTAssertTrue(focused(app).label.hasPrefix("Tester les manettes"), focused(app).label)
        remote.press(.right)
        sleep(1)
        XCTAssertTrue(focused(app).label.hasPrefix("Tester les manettes"), focused(app).label)
        remote.press(.menu)
        XCTAssertTrue(app.buttons["Réglages de l'app"].waitForExistence(timeout: 3))
    }

    /// Volet « Adresse » : taper l'adresse au clavier de l'écran ne ramène pas au premier volet.
    func testAddressFormSurvivesKeyboard() {
        let app = launch(["-screen", "add"])
        XCTAssertTrue(app.buttons["Adresse"].waitForExistence(timeout: 5) || app.staticTexts["Adresse"].waitForExistence(timeout: 1))
        sleep(1)
        for _ in 0..<3 { remote.press(.down) }
        remote.press(.select)
        XCTAssertTrue(app.staticTexts["Son adresse et son port"].waitForExistence(timeout: 3))
        remote.press(.right)
        remote.press(.select)
        sleep(1)
        app.typeText("10.8.0.12")
        remote.press(.menu)
        sleep(1)
        XCTAssertTrue(app.staticTexts["Son adresse et son port"].exists, "le volet Adresse doit rester affiché après le clavier")
    }
}
