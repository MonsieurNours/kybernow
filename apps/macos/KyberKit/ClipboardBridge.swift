// Presse-papiers partagé (texte), dans les deux sens.
//
// Protocole de kynput : un côté annonce « du nouveau » (Change), l'autre le
// demande (DataRequest) et reçoit le texte (Data, 60 Ko au plus). Le Mac
// demande tout de suite ce que l'ordinateur annonce, pour que ⌘V marche
// aussitôt ; il annonce ce qui change dans son presse-papiers (macOS ne
// prévient pas : on regarde deux fois par seconde) et répond aux demandes.
// Fonctions C : kyber-patches/0002-kynput.

import AppKit
@_implementationOnly import CKyber

final class ClipboardBridge {
    /// Envoie un paquet vers l'ordinateur.
    var send: ((OpaquePointer?) -> Void)?

    private let board: NSPasteboard
    private var timer: Timer?
    private var seenChangeCount: Int

    /// `board` : le presse-papiers général, ou un presse-papiers privé pour les tests.
    init(board: NSPasteboard = .general) {
        self.board = board
        seenChangeCount = board.changeCount
    }

    func start() {
        // Ce que le Mac a déjà copié est disponible dès le début de la session.
        if board.string(forType: .string) != nil { announce() }
        timer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in self?.poll() }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    /// Paquet de l'ordinateur (fil de kynput).
    func received(_ packet: OpaquePointer) {
        var kind = KYNPUT_CLIPBOARD_EVENT_KIND_CHANGE
        var text = false
        guard kynput_clipboard_event_from_packet(packet, &kind, &text), text else { return }
        switch kind {
        case KYNPUT_CLIPBOARD_EVENT_KIND_CHANGE:
            send?(kynput_clipboard_text_request_to_packet(KYNPUT_INPUT_TARGET_HOST))
        case KYNPUT_CLIPBOARD_EVENT_KIND_DATA:
            var len = 0
            guard let bytes = kynput_clipboard_text_from_packet(packet, &len),
                  let string = String(bytes: UnsafeBufferPointer(start: bytes, count: len), encoding: .utf8)
            else { return }
            DispatchQueue.main.async { [weak self] in self?.write(string) }
        case KYNPUT_CLIPBOARD_EVENT_KIND_DATA_REQUEST:
            DispatchQueue.main.async { [weak self] in self?.answer() }
        default:
            break
        }
    }

    // MARK: - Fil principal

    func poll() {
        let count = board.changeCount
        guard count != seenChangeCount else { return }
        seenChangeCount = count
        if board.string(forType: .string) != nil { announce() }
    }

    private func announce() {
        send?(kynput_clipboard_text_change_to_packet(KYNPUT_INPUT_TARGET_HOST))
    }

    private func answer() {
        guard let string = board.string(forType: .string) else {
            send?(kynput_clipboard_text_unavailable_to_packet(KYNPUT_INPUT_TARGET_HOST))
            return
        }
        let packet = Array(string.utf8).withUnsafeBufferPointer {
            kynput_clipboard_text_data_to_packet(KYNPUT_INPUT_TARGET_HOST, $0.baseAddress, $0.count)
        }
        send?(packet)
    }

    private func write(_ string: String) {
        guard board.string(forType: .string) != string else { return }
        board.clearContents()
        board.setString(string, forType: .string)
        // Notre propre écriture ne doit pas repartir vers l'ordinateur.
        seenChangeCount = board.changeCount
    }
}
