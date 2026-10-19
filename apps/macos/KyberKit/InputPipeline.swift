// Entrées vers l'ordinateur (libkynput) et curseur venant de lui.
//
// Le pipeline client de kynput produit déjà certaines entrées sur macOS
// (manettes via SDL) ; on produit les autres depuis les événements AppKit,
// comme le client Kyber. Les annonces de curseur du PC (forme, position,
// visibilité) arrivent par un gestionnaire dédié.

import AppKit
@_implementationOnly import CKyber

/// Curseur du PC, prêt à être affiché sur le Mac.
public struct RemoteCursor {
    public let visible: Bool
    public let image: NSImage?
    public let hotSpot: CGPoint
}

private let cursorTypes: UnsafeMutablePointer<kynput_InputType> = {
    let p = UnsafeMutablePointer<kynput_InputType>.allocate(capacity: 1)
    p.initialize(to: KYNPUT_INPUT_TYPE_CURSOR)
    return p
}()

private final class CursorSink {
    weak var view: StreamView?
    init(view: StreamView) { self.view = view }
}

final class InputPipeline {
    let pipeline: OpaquePointer
    private let sink: OpaquePointer
    let displayID: UInt32

    init(display: KyberDisplay, view: StreamView) {
        displayID = display.id
        pipeline = kynput_pipeline_new(kynput_get_client_config())
        sink = kynput_pipeline_get_input_sink(pipeline)

        // Curseur du PC.
        let box = Unmanaged.passRetained(CursorSink(view: view)).toOpaque()
        let cursor = kynput_handler_new(kynput_HandlerInterface(
            start: nil,
            stop: nil,
            handled_types: cursorTypes,
            handled_types_count: 1,
            consume: { packet, userdata in
                guard let packet, let userdata, let c = kynput_cursor_from_packet(packet) else { return }
                defer { kynput_cursor_unref(c) }
                let remote = InputPipeline.decode(c)
                let sink = Unmanaged<CursorSink>.fromOpaque(userdata).takeUnretainedValue()
                DispatchQueue.main.async { sink.view?.remoteCursorChanged(remote) }
            },
            produced_types: nil,
            produced_types_count: 0,
            query_type_metadata: { type, _ in
                type == KYNPUT_INPUT_TYPE_CURSOR ? kynput_input_type_metadata_cursor_pack(KYNPUT_CURSOR_FORMAT_RAW) : nil
            },
            destroy: { userdata in
                if let userdata { Unmanaged<CursorSink>.fromOpaque(userdata).release() }
            },
            userdata: box
        ))
        kynput_pipeline_plug_handler(pipeline, cursor)
        kynput_handler_unref(cursor)

        // Entrées que le pipeline ne produit pas lui-même : déclarées ici,
        // envoyées par StreamView.
        let wanted = [KYNPUT_INPUT_TYPE_KEYBOARD, KYNPUT_INPUT_TYPE_MOUSE_POSITION, KYNPUT_INPUT_TYPE_MOUSE_BUTTON, KYNPUT_INPUT_TYPE_MOUSE_WHEEL]
        let missing = wanted.filter { !kynput_pipeline_produces_type(pipeline, $0) }
        KyberLog.app.notice("entrées : \(wanted.count - missing.count) types produits par kynput, \(missing.count) par l'app (écran \(display.id))")
        if !missing.isEmpty {
            // Le tableau doit vivre aussi longtemps que le gestionnaire : on le garde.
            let types = UnsafeMutablePointer<kynput_InputType>.allocate(capacity: missing.count)
            types.initialize(from: missing, count: missing.count)
            let producer = kynput_handler_new(kynput_HandlerInterface(
                start: nil, stop: nil,
                handled_types: nil, handled_types_count: 0, consume: nil,
                produced_types: types, produced_types_count: missing.count,
                query_type_metadata: nil, destroy: nil, userdata: nil
            ))
            kynput_pipeline_plug_handler(pipeline, producer)
            kynput_handler_unref(producer)
        }

        // Souris absolue au départ : le curseur du PC suit celui du Mac.
        kynput_pipeline_set_type_processing_state(pipeline, KYNPUT_INPUT_TYPE_MOUSE_MOVE, kynput_router_processing_state_bool(false))
    }

    deinit {
        kynput_consumer_unref(sink)
        kynput_pipeline_unref(pipeline)
    }

    func stop() {
        kynput_pipeline_flush_keys(pipeline)
    }

    private var logged = Set<String>()

    private func send(_ packet: OpaquePointer?, _ kind: String) {
        guard let packet else { return }
        let rc = kynput_consumer_consume(sink, packet)
        if logged.insert(kind).inserted {
            KyberLog.app.notice("entrées : premier envoi \(kind, privacy: .public) (code \(rc))")
        }
    }

    func key(scancode: UInt16, pressed: Bool) {
        send(kynput_keyboard_key_to_packet(KYNPUT_INPUT_TARGET_HOST, scancode, pressed), "touche")
    }

    func mouse(x: Int16, y: Int16) {
        send(kynput_mouse_position_to_packet(KYNPUT_INPUT_TARGET_HOST, displayID, x, y), "position")
    }

    func mouseButton(_ button: MouseButton, pressed: Bool) {
        send(kynput_mouse_button_to_packet(KYNPUT_INPUT_TARGET_HOST, button.sdk, pressed), "clic")
    }

    func wheel(dx: Float, dy: Float) {
        send(kynput_mouse_wheel_to_packet(KYNPUT_INPUT_TARGET_HOST, dx, dy), "molette")
    }

    func flushKeys() {
        kynput_pipeline_flush_keys(pipeline)
    }

    enum MouseButton {
        case left, right, middle, side, extra

        var sdk: kynput_MouseButtonType {
            switch self {
            case .left: return KYNPUT_MOUSE_BUTTON_TYPE_LEFT
            case .right: return KYNPUT_MOUSE_BUTTON_TYPE_RIGHT
            case .middle: return KYNPUT_MOUSE_BUTTON_TYPE_MIDDLE
            case .side: return KYNPUT_MOUSE_BUTTON_TYPE_SIDE
            case .extra: return KYNPUT_MOUSE_BUTTON_TYPE_EXTRA
            }
        }
    }

    /// Forme ARGB (32 bits, non prémultipliée) → NSImage.
    private static func decode(_ c: OpaquePointer) -> RemoteCursor {
        let visible = kynput_cursor_visible(c)
        let width = Int(kynput_cursor_width(c))
        let height = Int(kynput_cursor_height(c))
        let hot = CGPoint(x: Int(kynput_cursor_xhot(c)), y: Int(kynput_cursor_yhot(c)))
        var count: UInt32 = 0
        guard kynput_cursor_format(c) == KYNPUT_CURSOR_FORMAT_RAW,
              let raw = kynput_cursor_raw_data(c, &count),
              width > 0, height > 0, Int(count) >= width * height
        else { return RemoteCursor(visible: visible, image: nil, hotSpot: hot) }

        var pixels = [UInt32](repeating: 0, count: width * height)
        for i in 0..<(width * height) {
            let p = raw[i]
            let a = (p >> 24) & 0xFF
            let r = ((p >> 16) & 0xFF) * a / 255
            let g = ((p >> 8) & 0xFF) * a / 255
            let b = (p & 0xFF) * a / 255
            pixels[i] = (a << 24) | (r << 16) | (g << 8) | b
        }
        let image: NSImage? = pixels.withUnsafeMutableBytes { buffer in
            guard let ctx = CGContext(
                data: buffer.baseAddress, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                space: CGColorSpace(name: CGColorSpace.sRGB)!,
                bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue
            ), let cg = ctx.makeImage() else { return nil }
            return NSImage(cgImage: cg, size: NSSize(width: width, height: height))
        }
        return RemoteCursor(visible: visible, image: image, hotSpot: hot)
    }
}
