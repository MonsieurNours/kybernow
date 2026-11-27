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

    /// Appuie sur `move` jusqu'à ce que le focus soit sur `label` (au plus `limit` fois) :
    /// la rangée de l'accueil gagne des puces avec les versions (« Auto », HDR…).
    @discardableResult
    private func reach(_ label: String, by move: XCUIRemote.Button, limit: Int = 8, in app: XCUIApplication) -> Bool {
        for _ in 0..<limit {
            if focused(app).label == label { return true }
            remote.press(move)
            usleep(400_000)
        }
        return focused(app).label == label
    }

    /// De « Se connecter », à droite après les profils et les puces : les réglages de l'ordinateur.
    func testSettingsReachableFromHome() {
        let app = launch()
        let label = "Réglages de Tour du bureau"
        XCTAssertTrue(app.buttons[label].waitForExistence(timeout: 5))
        sleep(1)
        reach(label, by: .right, in: app)
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

    /// « Rejoindre avec un code » sur un code d'invitation (`-demoInvite YES`,
    /// sans Station) : « Ajouter PC du salon », le mot de passe (masqué) au
    /// clavier de l'écran, « Afficher » à sa droite, le bas jusqu'à « Ajouter
    /// l'ordinateur » ; l'ordinateur arrive
    /// sur l'accueil avec l'avis « PC du salon est ajouté. ».
    func testInvitationCodeAddsTheComputer() {
        let app = launch(["-screen", "join", "-demoInvite", "YES"])
        XCTAssertTrue(app.buttons["1"].waitForExistence(timeout: 5))
        sleep(1)
        // Six fois « 1 » sur le pavé : le focus passe ensuite à « Rejoindre ».
        for _ in 0..<6 { remote.press(.select) }
        sleep(1)
        XCTAssertEqual(focused(app).label, "Rejoindre")
        remote.press(.select)
        XCTAssertTrue(app.staticTexts["Ajouter PC du salon"].waitForExistence(timeout: 5))
        sleep(1)
        remote.press(.select)
        sleep(1)
        app.typeText("K7MQ P2XA 9FZ3")
        remote.press(.menu)
        sleep(1)
        XCTAssertTrue(app.staticTexts["Ajouter PC du salon"].exists, "le clavier fermé, l'écran de l'invitation doit rester")
        // Le mot de passe est masqué : « Afficher », à droite du champ, le montre (puis « Masquer »).
        remote.press(.right)
        sleep(1)
        XCTAssertEqual(focused(app).label, "Afficher")
        remote.press(.select)
        sleep(1)
        XCTAssertEqual(focused(app).label, "Masquer")
        remote.press(.left)
        sleep(1)
        remote.press(.down)
        sleep(1)
        XCTAssertEqual(focused(app).label, "Ajouter l'ordinateur")
        remote.press(.select)
        XCTAssertTrue(app.staticTexts["PC du salon est ajouté."].waitForExistence(timeout: 5))
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

    /// Menu de session (`-screen menu`) : le panneau « Son » garde le focus,
    /// et Retour ne ferme que lui ; le menu reste ouvert, le focus revient à
    /// la tuile « Son » (avant, Retour fermait tout le menu).
    func testSoundPanelBackClosesOnlyThePanel() {
        let app = launch(["-screen", "menu"])
        XCTAssertTrue(app.staticTexts["Reprendre"].waitForExistence(timeout: 8))
        sleep(1)
        // De « Reprendre », la tuile « Son » : à droite ou en dessous, selon la rangée.
        var reached = false
        for move in [XCUIRemote.Button.right, .right, .right, .right, .down, .left, .left, .left, .left] {
            if focused(app).label.hasPrefix("Son") { reached = true; break }
            remote.press(move)
            usleep(400_000)
        }
        reached = reached || focused(app).label.hasPrefix("Son")
        XCTAssertTrue(reached, "tuile « Son » introuvable à la télécommande (\(focused(app).label))")
        remote.press(.select)
        XCTAssertTrue(app.staticTexts["Changer de sortie : AirPods, HomePod, téléviseur"].waitForExistence(timeout: 3))
        sleep(1)
        XCTAssertEqual(focused(app).label, "Fermer", "le focus doit entrer dans le panneau")
        remote.press(.menu)
        sleep(1)
        XCTAssertFalse(app.staticTexts["Changer de sortie : AirPods, HomePod, téléviseur"].exists, "Retour ferme le panneau")
        XCTAssertTrue(app.staticTexts["Reprendre"].exists, "le menu reste ouvert")
        XCTAssertTrue(focused(app).label.hasPrefix("Son"), "le focus revient à la tuile « Son » (\(focused(app).label))")
    }

    /// Menu de session : le panneau « Taper du texte » s'ouvre sur son champ,
    /// et Retour ne ferme que lui (focus rendu à la tuile).
    func testTypingPanelBackClosesOnlyThePanel() {
        let app = launch(["-screen", "menu"])
        XCTAssertTrue(app.staticTexts["Reprendre"].waitForExistence(timeout: 8))
        sleep(1)
        var reached = false
        // Une seule rangée de tuiles : à droite de « Reprendre ».
        for move in Array(repeating: XCUIRemote.Button.right, count: 8) {
            if focused(app).label.hasPrefix("Taper du texte") { reached = true; break }
            remote.press(move)
            usleep(400_000)
        }
        reached = reached || focused(app).label.hasPrefix("Taper du texte")
        XCTAssertTrue(reached, "tuile « Taper du texte » introuvable à la télécommande (\(focused(app).label))")
        remote.press(.select)
        XCTAssertTrue(app.staticTexts["Le texte part tel quel, à l'endroit où se trouve le curseur sur l'ordinateur."].waitForExistence(timeout: 3))
        sleep(1)
        remote.press(.menu)
        sleep(1)
        XCTAssertFalse(app.staticTexts["Le texte part tel quel, à l'endroit où se trouve le curseur sur l'ordinateur."].exists, "Retour ferme le panneau")
        XCTAssertTrue(app.staticTexts["Reprendre"].exists, "le menu reste ouvert")
        XCTAssertTrue(focused(app).label.hasPrefix("Taper du texte"), "le focus revient à la tuile (\(focused(app).label))")
    }
}
