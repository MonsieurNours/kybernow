// Presse-papiers partagé (texte), dans les deux sens.
//
// Protocole de kynput : un côté annonce « du nouveau » (Change), l'autre le
// demande (DataRequest) et reçoit le texte (Data, 60 Ko au plus). L'appareil
// demande tout de suite ce que l'ordinateur annonce, pour que ⌘V marche
// aussitôt ; il annonce ce qui change dans son presse-papiers (le système ne
// prévient pas : on regarde deux fois par seconde) et répond aux demandes.
// Fonctions C : kyber-patches/0002-kynput.
//
// Le presse-papiers lui-même est celui de la plateforme (PlatformPasteboard :
// NSPasteboard sur Mac, UIPasteboard à brancher sur iOS, rien sur tvOS), lu et
// écrit sur le fil principal. Les paquets de l'ordinateur arrivent par
// `received`, hors du verrou du routeur kynput (InputPipeline les y renvoie
// sur sa file) : les réponses repartent par `send` sans réentrer dans le
// routeur.

import Foundation
@_implementationOnly import CKyber

/// Presse-papiers de la plateforme, texte seulement. Fil principal.
public protocol PlatformPasteboard: AnyObject {
    /// Compteur qui change à chaque écriture, par l'app ou par une autre.
    var changeCount: Int { get }
    /// Texte présent, nil s'il n'y en a pas.
    var text: String? { get }
    /// Remplace le contenu par ce texte.
    func setText(_ text: String)
}

final class ClipboardBridge {
    /// Envoie un paquet vers l'ordinateur (appelable de tout fil).
    var send: ((OpaquePointer?) -> Void)?

    private let board: PlatformPasteboard
    private var timer: Timer?
    private var seenChangeCount: Int

    /// `board` : le presse-papiers général, ou un presse-papiers privé pour les tests.
    init(board: PlatformPasteboard) {
        self.board = board
        seenChangeCount = board.changeCount
    }

    func start() {
        // Ce que l'appareil a déjà copié est disponible dès le début de la session.
        if board.text != nil { announce() }
        timer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in self?.poll() }
    }

    func stop() {
        timer?.invalidate()
        timer = nil
    }

    /// Paquet de l'ordinateur, hors du verrou du routeur kynput (file des entrées).
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
        if board.text != nil { announce() }
    }

    private func announce() {
        send?(kynput_clipboard_text_change_to_packet(KYNPUT_INPUT_TARGET_HOST))
    }

    private func answer() {
        guard let string = board.text else {
            send?(kynput_clipboard_text_unavailable_to_packet(KYNPUT_INPUT_TARGET_HOST))
            return
        }
        let packet = Array(string.utf8).withUnsafeBufferPointer {
            kynput_clipboard_text_data_to_packet(KYNPUT_INPUT_TARGET_HOST, $0.baseAddress, $0.count)
        }
        send?(packet)
    }

    private func write(_ string: String) {
        guard board.text != string else { return }
        board.setText(string)
        // Notre propre écriture ne doit pas repartir vers l'ordinateur.
        seenChangeCount = board.changeCount
    }
}
