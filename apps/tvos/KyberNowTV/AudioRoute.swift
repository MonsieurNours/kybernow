// La sortie du son, commune à l'iPhone, à l'iPad et à l'Apple TV (audit
// IOS-23) : ses voies et son retard. Les noms propres à l'appareil (HDMI,
// haut-parleur ou téléviseur) sont dans son Platform.swift. Compilé par les
// deux apps (apps/ios/project.yml le reprend).

import AVFoundation
import KyberKit

enum AudioOutput {
    /// Voies de la sortie actuelle : ampli HDMI 5.1 ou 7.1, AirPods en audio
    /// spatial (ils acceptent alors le 5.1 et le 7.1), sinon stéréo.
    static func channels() -> Int {
        try? AVAudioSession.sharedInstance().setSupportsMultichannelContent(Preferences.shared.spatialAudio)
        let n = AVAudioSession.sharedInstance().maximumOutputNumberOfChannels
        return n >= 8 ? 8 : n >= 6 ? 6 : 2
    }
}

/// Sortie du son vue par l'appareil, et ce qu'elle ajoute de retard.
struct AudioRoute: Equatable {
    enum Kind { case hdmi, airpods, bluetooth, airplay, builtIn, headphones }
    var kind: Kind
    var name: String
    var channels: Int
    var latencyMs: Int

    static var current: AudioRoute {
        let session = AVAudioSession.sharedInstance()
        let port = session.currentRoute.outputs.first
        let latency = Int(((session.outputLatency + session.ioBufferDuration) * 1000).rounded())
        let channels = AudioOutput.channels()
        switch port?.portType {
        case .HDMI?: return AudioRoute(kind: .hdmi, name: hdmiName(port, channels: channels), channels: channels, latencyMs: latency)
        case .bluetoothA2DP?, .bluetoothLE?:
            // Les voies réelles : des AirPods en audio spatial reçoivent le 5.1 et le 7.1.
            let name = port?.portName ?? "Bluetooth"
            return AudioRoute(kind: name.localizedCaseInsensitiveContains("AirPods") ? .airpods : .bluetooth, name: name, channels: channels, latencyMs: latency)
        case .airPlay?: return AudioRoute(kind: .airplay, name: port?.portName ?? "AirPlay", channels: 2, latencyMs: latency)
        case .headphones?, .usbAudio?: return AudioRoute(kind: .headphones, name: port?.portName ?? String(localized: "casque"), channels: 2, latencyMs: latency)
        default: return AudioRoute(kind: .builtIn, name: builtInName(port), channels: builtInChannels(channels), latencyMs: latency)
        }
    }

    /// AirPods, HomePod, AirPlay, Bluetooth : le son arrive nettement après l'image.
    var isLate: Bool { kind != .hdmi && kind != .builtIn && kind != .headphones && latencyMs > 40 }

    var channelsLabel: String { channels >= 8 ? "7.1" : channels >= 6 ? "5.1" : String(localized: "Stéréo") }
}
