// Ce que l'app laisse au Top Shelf (rangée du haut de l'accueil de l'Apple
// TV) : ses ordinateurs, leur dernière image et leur état, dans le conteneur
// partagé du groupe d'apps. Compilé dans l'app et dans l'extension.

import Foundation

struct TopShelfEntry: Codable, Identifiable {
    var id: UUID
    var name: String
    var online: Bool
    /// « en ligne », « hors ligne ».
    var reach: String
    var lastSession: Date?
    /// Image du dernier passage (ou aperçu), dans le dossier partagé.
    var imageFile: String?
}

enum TopShelfStore {
    /// `group.<identifiant de l'app>` : l'extension (`….topshelf`) et l'app
    /// partagent le même groupe, y compris quand l'app est installée sous un
    /// autre identifiant (scripts/sideload.sh).
    static var group: String {
        var id = Bundle.main.bundleIdentifier ?? "fr.leslignesclaires.kybernow"
        if id.hasSuffix(".topshelf") { id.removeLast(".topshelf".count) }
        return "group." + id
    }

    static var folder: URL? {
        FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group)?
            .appendingPathComponent("TopShelf", isDirectory: true)
    }

    private static var index: URL? { folder?.appendingPathComponent("hosts.json") }

    static func read() -> [TopShelfEntry] {
        guard let index, let data = try? Data(contentsOf: index) else { return [] }
        return (try? JSONDecoder().decode([TopShelfEntry].self, from: data)) ?? []
    }

    static func write(_ entries: [TopShelfEntry]) {
        guard let folder, let index else { return }
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(entries) { try? data.write(to: index, options: .atomic) }
    }

    static func imageURL(_ name: String) -> URL? { folder?.appendingPathComponent(name) }

    /// Lien qui ouvre directement la session (`kybernow-tv://connect/<id>`).
    static func connectURL(_ id: UUID) -> URL { URL(string: "kybernow-tv://connect/\(id.uuidString)")! }
    static func showURL(_ id: UUID) -> URL { URL(string: "kybernow-tv://show/\(id.uuidString)")! }
}
