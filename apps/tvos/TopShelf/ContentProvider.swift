// Top Shelf de KyberNow : « Reprendre sur un ordinateur », une carte 16:9 par
// ordinateur avec sa dernière image. Un clic (ou Lecture) ouvre directement
// la session ; un appui long sur la carte ouvre l'app sur cet ordinateur.

import TVServices

final class ContentProvider: TVTopShelfContentProvider {
    override func loadTopShelfContent() async -> TVTopShelfContent? {
        let entries = TopShelfStore.read().sorted { ($0.lastSession ?? .distantPast) > ($1.lastSession ?? .distantPast) }
        guard !entries.isEmpty else { return nil }
        let items = entries.map { entry -> TVTopShelfSectionedItem in
            let item = TVTopShelfSectionedItem(identifier: entry.id.uuidString)
            item.title = entry.name
            item.imageShape = .hdtv
            if let file = entry.imageFile, let url = TopShelfStore.imageURL(file) {
                item.setImageURL(url, for: [.screenScale1x, .screenScale2x])
            }
            item.playAction = TVTopShelfAction(url: TopShelfStore.connectURL(entry.id))
            item.displayAction = TVTopShelfAction(url: entry.online ? TopShelfStore.connectURL(entry.id) : TopShelfStore.showURL(entry.id))
            return item
        }
        let section = TVTopShelfItemCollection(items: items)
        section.title = String(localized: "Reprendre sur un ordinateur")
        return TVTopShelfSectionedContent(sections: [section])
    }
}
