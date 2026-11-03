// KyberNow Manettes (kybernow-gamepads) : manettes virtuelles de la Station
// Mac, par CoreHID (`HIDVirtualDevice`, macOS 15).
//
// Lancée par kynputserver (correctif Kyber Mac 0011, `macos/host_gamepad.rs`)
// au premier branchement d'une manette, et pilotée par un tube :
//   entrée  : [op u8][id u32 LE][longueur u32 LE][données]
//     1 créer    : vid u16, pid u16, bluetooth u8, produit, fabricant, numéro
//                  de série (longueur u8 + UTF-8), descripteur (u16 + octets),
//                  rapports de fonction (nombre u8, puis numéro u8, u16, octets)
//     2 rapport  : rapport d'entrée complet (numéro en tête)
//     3 retirer
//   sortie : même trame
//     0x81 créée ; 0x82 refusée (1 : macOS refuse, droit d'Apple absent) ;
//     0x83 rapport reçu d'un jeu : type (0 entrée, 1 sortie, 2 fonction),
//          numéro (0 sans), octets.
// Fin du tube : les manettes disparaissent et l'aide s'arrête.
//
// `kybernow-gamepads --check` : 0 si macOS laisse créer une manette virtuelle
// (droit `com.apple.developer.hid.virtual.device` présent), 2 sinon. Admin
// s'en sert pour `gamepads` de `GET /api/stream/info`. L'appareil d'essai est
// propre au fabricant (page 0xFF00) : aucun jeu ne le prend pour une manette.
//
// Ce droit est accordé par Apple sur demande, pour l'App ID
// fr.leslignesclaires.kybernow.station.gamepads (docs/station-macos.md).

import CoreHID
import Foundation

let opCreate: UInt8 = 1, opInput: UInt8 = 2, opDestroy: UInt8 = 3
let opCreated: UInt8 = 0x81, opFailed: UInt8 = 0x82, opOutput: UInt8 = 0x83
let failedRefused: UInt8 = 1, failedInvalid: UInt8 = 2

func log(_ s: String) {
    FileHandle.standardError.write(("kybernow-gamepads: " + s + "\n").data(using: .utf8)!)
}

// MARK: Sortie (plusieurs tâches écrivent : un verrou)

let outputLock = NSLock()

func send(_ op: UInt8, _ id: UInt32, _ payload: Data = Data()) {
    var frame = Data([op])
    withUnsafeBytes(of: id.littleEndian) { frame.append(contentsOf: $0) }
    withUnsafeBytes(of: UInt32(payload.count).littleEndian) { frame.append(contentsOf: $0) }
    frame.append(payload)
    outputLock.lock()
    defer { outputLock.unlock() }
    frame.withUnsafeBytes { buf in
        var off = 0
        while off < buf.count {
            let n = write(1, buf.baseAddress! + off, buf.count - off)
            if n <= 0 { return }
            off += n
        }
    }
}

/// Rapports déjà notés au journal : (manette, numéro du rapport).
nonisolated(unsafe) var loggedReports = Set<UInt64>()

func firstReport(_ id: UInt32, _ report: UInt8) -> Bool {
    outputLock.lock()
    defer { outputLock.unlock() }
    return loggedReports.insert(UInt64(id) << 8 | UInt64(report)).inserted
}

// MARK: Entrée

struct Frame {
    let op: UInt8
    let id: UInt32
    let data: Data
}

func readExact(_ count: Int) -> Data? {
    var out = Data(count: count)
    var off = 0
    let ok = out.withUnsafeMutableBytes { buf -> Bool in
        while off < count {
            let n = read(0, buf.baseAddress! + off, count - off)
            if n <= 0 { return false }
            off += n
        }
        return true
    }
    return ok ? out : nil
}

func readFrame() -> Frame? {
    guard let head = readExact(9) else { return nil }
    let id = head.subdata(in: 1..<5).withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) }.littleEndian
    let len = head.subdata(in: 5..<9).withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) }.littleEndian
    guard len <= 1 << 20, let data = readExact(Int(len)) else { return nil }
    return Frame(op: head[0], id: id, data: data)
}

/// Lecture des champs de `créer`.
struct Cursor {
    let data: Data
    var at: Int

    init(_ data: Data) {
        self.data = data
        at = data.startIndex
    }

    mutating func bytes(_ n: Int) -> Data? {
        guard at + n <= data.endIndex else { return nil }
        defer { at += n }
        return data.subdata(in: at..<at + n)
    }

    mutating func u8() -> UInt8? { bytes(1)?.first }

    mutating func u16() -> UInt16? {
        guard let b = bytes(2) else { return nil }
        return UInt16(b[b.startIndex]) | UInt16(b[b.startIndex + 1]) << 8
    }

    mutating func string() -> String? {
        guard let n = u8(), let b = bytes(Int(n)) else { return nil }
        return String(data: b, encoding: .utf8)
    }
}

struct CreateRequest {
    let properties: HIDVirtualDevice.Properties
    let features: [UInt8: Data]

    init?(id: UInt32, _ data: Data) {
        var c = Cursor(data)
        guard let vid = c.u16(), let pid = c.u16(), let bluetooth = c.u8(),
              let product = c.string(), let maker = c.string(), let serial = c.string(),
              let dlen = c.u16(), let descriptor = c.bytes(Int(dlen)), let count = c.u8()
        else { return nil }
        var features: [UInt8: Data] = [:]
        for _ in 0..<count {
            guard let rid = c.u8(), let len = c.u16(), let bytes = c.bytes(Int(len)) else { return nil }
            features[rid] = bytes
        }
        self.features = features
        properties = HIDVirtualDevice.Properties(
            descriptor: descriptor,
            vendorID: UInt32(vid),
            productID: UInt32(pid),
            transport: bluetooth != 0 ? .bluetooth : .usb,
            product: product,
            manufacturer: maker,
            serialNumber: serial,
            uniqueID: serial,
            // Un emplacement par manette, stable le temps de la session.
            locationID: 0x4B4E_0000 | UInt64(id & 0xFFFF)
        )
    }
}

// MARK: Manettes

/// Réponses aux jeux : rapports de fonction connus, rapports reçus renvoyés
/// à kynput (vibrations).
struct Delegate: HIDVirtualDeviceDelegate {
    let id: UInt32
    let features: [UInt8: Data]

    func hidVirtualDevice(_ device: HIDVirtualDevice, receivedSetReportRequestOfType type: HIDReportType,
                          id reportID: HIDReportID?, data: Data) async throws {
        var payload = Data([kind(type), reportID?.rawValue ?? 0])
        payload.append(data)
        // Le premier rapport de chaque numéro, par manette, au journal de Kyber
        // (un jeu peut en envoyer des dizaines par seconde).
        if firstReport(id, reportID?.rawValue ?? 0) {
            log("pad \(id): first \(type) report \(reportID?.rawValue ?? 0), \(data.count) bytes: "
                + data.prefix(12).map { String(format: "%02x", $0) }.joined(separator: " "))
        }
        send(opOutput, id, payload)
    }

    func hidVirtualDevice(_ device: HIDVirtualDevice, receivedGetReportRequestOfType type: HIDReportType,
                          id reportID: HIDReportID?, maxSize: Int) async throws -> Data {
        guard type == .feature, let rid = reportID?.rawValue, let data = features[rid] else {
            throw CocoaError(.featureUnsupported)
        }
        return data.prefix(maxSize)
    }

    func kind(_ type: HIDReportType) -> UInt8 {
        switch type {
        case .input: return 0
        case .output: return 1
        default: return 2
        }
    }
}

func check() async -> Int32 {
    // Collection propre au fabricant, un octet d'entrée.
    let descriptor = Data([0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x09, 0x01, 0x15, 0x00, 0x26, 0xFF, 0x00,
                           0x75, 0x08, 0x95, 0x01, 0x81, 0x02, 0xC0])
    let props = HIDVirtualDevice.Properties(descriptor: descriptor, vendorID: 0, product: "KyberNow check")
    if HIDVirtualDevice(properties: props) != nil {
        print("ok")
        return 0
    }
    print("refused")
    return 2
}

func serve() async {
    let frames = AsyncStream<Frame> { continuation in
        Thread.detachNewThread {
            while let f = readFrame() { continuation.yield(f) }
            continuation.finish()
        }
    }
    var devices: [UInt32: HIDVirtualDevice] = [:]
    for await f in frames {
        switch f.op {
        case opCreate:
            devices[f.id] = nil
            guard let request = CreateRequest(id: f.id, f.data) else {
                log("pad \(f.id): invalid create request")
                send(opFailed, f.id, Data([failedInvalid]))
                continue
            }
            guard let device = HIDVirtualDevice(properties: request.properties) else {
                log("pad \(f.id): macOS refused the virtual device (com.apple.developer.hid.virtual.device missing?)")
                send(opFailed, f.id, Data([failedRefused]))
                continue
            }
            await device.activate(delegate: Delegate(id: f.id, features: request.features))
            devices[f.id] = device
            log("pad \(f.id): \(request.properties.product ?? "?") created")
            send(opCreated, f.id)
        case opInput:
            guard let device = devices[f.id] else { continue }
            do {
                try await device.dispatchInputReport(data: f.data, timestamp: SuspendingClock.now)
            } catch {
                log("pad \(f.id): input report refused (\(error))")
            }
        case opDestroy:
            devices[f.id] = nil
            log("pad \(f.id): removed")
        default:
            log("unknown op \(f.op)")
        }
    }
    devices.removeAll()
    log("pipe closed, \(CommandLine.arguments[0]) exits")
}

signal(SIGPIPE, SIG_IGN)
if CommandLine.arguments.contains("--check") {
    exit(await check())
}
await serve()
