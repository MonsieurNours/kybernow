// Les images des ordinateurs de l'accueil, communes à l'iPhone, à l'iPad et
// à l'Apple TV (audit IOS-23 ; le Mac a la sienne, en NSImage). Compilé par
// les deux apps (apps/ios/project.yml le reprend).

import Foundation
import KyberKit
import UIKit

/// L'image d'un ordinateur sur l'accueil : la dernière image de sa dernière
/// session, un aperçu demandé à Admin, sinon sa signature (Backdrops). Le
/// dossier et la largeur de l'aperçu sont ceux de l'appareil
/// (`HostImagery.folder`, `previewWidth`, dans son Platform.swift) ; l'Apple
/// TV les garde dans le conteneur partagé, pour le Top Shelf.
@MainActor
final class HostImagery: ObservableObject {
    enum Source: Equatable { case lastSession, preview }

    struct Picture {
        let image: UIImage
        let date: Date
        let source: Source
    }

    @Published private(set) var pictures: [UUID: Picture] = [:]
    private var lastFetch: [UUID: Date] = [:]
    private let folder: URL

    init() {
        folder = Self.folder
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let files = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: [.contentModificationDateKey])) ?? []
        for url in files.sorted(by: { $0.lastPathComponent.hasSuffix("-last.jpg") && !$1.lastPathComponent.hasSuffix("-last.jpg") }) {
            guard url.pathExtension == "jpg" else { continue }
            let name = url.deletingPathExtension().lastPathComponent
            guard let dash = name.lastIndex(of: "-"), let id = UUID(uuidString: String(name[..<dash])),
                  pictures[id] == nil, let image = UIImage(contentsOfFile: url.path) else { continue }
            let date = (try? url.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate ?? Date()
            pictures[id] = Picture(image: image, date: date, source: name.hasSuffix("-last") ? .lastSession : .preview)
        }
    }

    /// Date de l'image de cet ordinateur (Top Shelf : republié quand elle change).
    func date(_ id: UUID) -> Date? { pictures[id]?.date }

    func file(_ id: UUID) -> String? {
        guard let p = pictures[id] else { return nil }
        return "\(id.uuidString)-\(p.source == .lastSession ? "last" : "preview").jpg"
    }

    private func url(_ id: UUID, _ source: Source) -> URL {
        folder.appendingPathComponent("\(id.uuidString)-\(source == .lastSession ? "last" : "preview").jpg")
    }

    func wantsPreview(_ host: Host) -> Bool {
        if pictures[host.id]?.source == .lastSession { return false }
        return Date().timeIntervalSince(lastFetch[host.id] ?? .distantPast) > 60
    }

    func fetchPreview(for host: Host, console: ConsoleClient) async {
        lastFetch[host.id] = Date()
        guard let shot = await console.preview(width: Self.previewWidth), let image = UIImage(data: shot.jpeg),
              pictures[host.id]?.source != .lastSession
        else { return }
        let file = url(host.id, .preview)
        try? shot.jpeg.write(to: file, options: .atomic)
        try? FileManager.default.setAttributes([.modificationDate: shot.capturedAt], ofItemAtPath: file.path)
        pictures[host.id] = Picture(image: image, date: shot.capturedAt, source: .preview)
    }

    func saveLast(_ cg: CGImage, for host: Host) {
        let image = UIImage(cgImage: cg)
        guard host.settings.keepLastImage, let data = image.jpegData(compressionQuality: 0.82) else { return }
        try? data.write(to: url(host.id, .lastSession), options: .atomic)
        try? FileManager.default.removeItem(at: url(host.id, .preview))
        pictures[host.id] = Picture(image: image, date: Date(), source: .lastSession)
    }

    func forgetLast(_ host: Host) {
        try? FileManager.default.removeItem(at: url(host.id, .lastSession))
        if pictures[host.id]?.source == .lastSession { pictures[host.id] = nil }
        lastFetch[host.id] = nil
    }

    func forget(_ host: Host) {
        try? FileManager.default.removeItem(at: url(host.id, .lastSession))
        try? FileManager.default.removeItem(at: url(host.id, .preview))
        pictures[host.id] = nil
    }
}
