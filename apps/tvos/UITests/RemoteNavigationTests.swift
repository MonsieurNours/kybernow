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

    /// De « Se connecter », on monte jusqu'à la barre puis à droite : les réglages.
    func testSettingsReachableFromHome() {
        let app = launch()
        XCTAssertTrue(app.buttons["Réglages"].waitForExistence(timeout: 5))
        sleep(1)
        remote.press(.up)
        for _ in 0..<4 { remote.press(.right) }
        XCTAssertEqual(focused(app).label, "Réglages")
        remote.press(.select)
        XCTAssertTrue(app.staticTexts["Image"].waitForExistence(timeout: 3))
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
