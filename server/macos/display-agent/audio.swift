// Sortie son de la Station Mac (docs/station-macos.md, jalon 4), pour Admin
// (`audio/mac.rs`) : commandes ponctuelles, une réponse JSON sur la sortie.
//
//   kybernow-display audio status
//       → {"speakers": {"id", "name", "channels"} | null, "default": {"id", "name", "channels"} | null}
//   kybernow-display audio route --channels 2|6|8
//       → {"previous": "<uid>", "previous_system": "<uid>", "speakers": "<uid>", "channels": n}
//   kybernow-display audio restore --previous <uid> [--previous-system <uid>]
//
// `route` règle la sortie KyberNow (server/macos/audio-driver) au nombre de
// canaux demandé, puis en fait la sortie par défaut et celle des alertes ;
// `restore` rend celles d'avant (si elles existent encore).

import CoreAudio
import Foundation

let kyberNowUID = "fr.leslignesclaires.kybernow.audio.device"

private func address(_ selector: AudioObjectPropertySelector, _ scope: AudioObjectPropertyScope = kAudioObjectPropertyScopeGlobal) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress(mSelector: selector, mScope: scope, mElement: kAudioObjectPropertyElementMain)
}

private func check(_ status: OSStatus, _ what: String) throws {
    if status != noErr { throw Failure(message: "\(what) : erreur Core Audio \(status)") }
}

private func deviceFor(uid: String) -> AudioDeviceID? {
    var cf = uid as CFString
    var id = AudioDeviceID(kAudioObjectUnknown)
    var size = UInt32(MemoryLayout<AudioDeviceID>.size)
    var a = address(kAudioHardwarePropertyTranslateUIDToDevice)
    let status = withUnsafeMutablePointer(to: &cf) { p in
        AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &a, UInt32(MemoryLayout<CFString>.size), p, &size, &id)
    }
    return status == noErr && id != kAudioObjectUnknown ? id : nil
}

private func stringProperty(_ id: AudioObjectID, _ selector: AudioObjectPropertySelector) -> String? {
    var a = address(selector)
    var value: Unmanaged<CFString>?
    var size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
    guard AudioObjectGetPropertyData(id, &a, 0, nil, &size, &value) == noErr, let v = value else { return nil }
    return v.takeRetainedValue() as String
}

private func defaultDevice(_ selector: AudioObjectPropertySelector) -> AudioDeviceID? {
    var a = address(selector)
    var id = AudioDeviceID(kAudioObjectUnknown)
    var size = UInt32(MemoryLayout<AudioDeviceID>.size)
    guard AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &a, 0, nil, &size, &id) == noErr, id != kAudioObjectUnknown else { return nil }
    return id
}

private func setDefault(_ selector: AudioObjectPropertySelector, _ device: AudioDeviceID) throws {
    var a = address(selector)
    var id = device
    try check(AudioObjectSetPropertyData(AudioObjectID(kAudioObjectSystemObject), &a, 0, nil, UInt32(MemoryLayout<AudioDeviceID>.size), &id), "sortie par défaut")
}

private func outputStream(_ device: AudioDeviceID) -> AudioStreamID? {
    var a = address(kAudioDevicePropertyStreams, kAudioObjectPropertyScopeOutput)
    var size: UInt32 = 0
    guard AudioObjectGetPropertyDataSize(device, &a, 0, nil, &size) == noErr, size >= UInt32(MemoryLayout<AudioStreamID>.size) else { return nil }
    var ids = [AudioStreamID](repeating: 0, count: Int(size) / MemoryLayout<AudioStreamID>.size)
    guard AudioObjectGetPropertyData(device, &a, 0, nil, &size, &ids) == noErr else { return nil }
    return ids.first
}

private func channels(of stream: AudioStreamID) -> UInt32 {
    var a = address(kAudioStreamPropertyPhysicalFormat)
    var f = AudioStreamBasicDescription()
    var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
    return AudioObjectGetPropertyData(stream, &a, 0, nil, &size, &f) == noErr ? f.mChannelsPerFrame : 0
}

private func setChannels(_ stream: AudioStreamID, _ count: UInt32) throws {
    var a = address(kAudioStreamPropertyPhysicalFormat)
    var f = AudioStreamBasicDescription()
    var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
    try check(AudioObjectGetPropertyData(stream, &a, 0, nil, &size, &f), "format de KyberNow")
    if f.mChannelsPerFrame == count { return }
    f.mChannelsPerFrame = count
    f.mBytesPerFrame = 4 * count
    f.mBytesPerPacket = 4 * count
    try check(AudioObjectSetPropertyData(stream, &a, 0, nil, size, &f), "canaux de KyberNow")
    // Le module applique le changement un peu plus tard (configuration de l'appareil).
    for _ in 0..<40 where channels(of: stream) != count {
        Thread.sleep(forTimeInterval: 0.05)
    }
}

private func option(_ args: [String], _ name: String) -> String? {
    guard let i = args.firstIndex(of: name), i + 1 < args.count else { return nil }
    return args[i + 1]
}

func audioCommand(_ args: [String]) throws -> [String: Any] {
    switch args.first {
    case "status":
        // Même forme que sous Windows (`audio/endpoint.rs`).
        func describe(_ d: AudioDeviceID?) -> Any {
            guard let d else { return NSNull() }
            return [
                "id": stringProperty(d, kAudioDevicePropertyDeviceUID) ?? "",
                "name": stringProperty(d, kAudioObjectPropertyName) ?? "",
                "channels": outputStream(d).map { channels(of: $0) } ?? 0,
            ] as [String: Any]
        }
        return [
            "speakers": describe(deviceFor(uid: kyberNowUID)),
            "default": describe(defaultDevice(kAudioHardwarePropertyDefaultOutputDevice)),
        ]
    case "route":
        guard let device = deviceFor(uid: kyberNowUID), let stream = outputStream(device) else {
            throw Failure(message: "sortie KyberNow absente (pilote non installé)")
        }
        let wanted = UInt32(option(args, "--channels") ?? "2") ?? 2
        let previous = defaultDevice(kAudioHardwarePropertyDefaultOutputDevice).flatMap { stringProperty($0, kAudioDevicePropertyDeviceUID) } ?? ""
        let previousSystem = defaultDevice(kAudioHardwarePropertyDefaultSystemOutputDevice).flatMap { stringProperty($0, kAudioDevicePropertyDeviceUID) } ?? ""
        try setChannels(stream, [2, 6, 8].contains(wanted) ? wanted : 2)
        try setDefault(kAudioHardwarePropertyDefaultOutputDevice, device)
        try setDefault(kAudioHardwarePropertyDefaultSystemOutputDevice, device)
        return ["previous": previous, "previous_system": previousSystem, "speakers": kyberNowUID, "channels": channels(of: stream)]
    case "restore":
        // Sortie d'origine disparue (casque débranché) : macOS en a choisi une autre, on la laisse.
        if let uid = option(args, "--previous"), let d = deviceFor(uid: uid) {
            try setDefault(kAudioHardwarePropertyDefaultOutputDevice, d)
        }
        if let uid = option(args, "--previous-system"), let d = deviceFor(uid: uid) {
            try setDefault(kAudioHardwarePropertyDefaultSystemOutputDevice, d)
        }
        if let s = deviceFor(uid: kyberNowUID), let stream = outputStream(s) {
            try? setChannels(stream, 2)
        }
        return [:]
    default:
        throw Failure(message: "audio status|route|restore")
    }
}
