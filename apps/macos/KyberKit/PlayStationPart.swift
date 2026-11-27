// Ce qu'une partie sur console fait de la même façon sur Mac, iPhone, iPad
// et Apple TV (audit IOS-25), à côté de `PlayStationPlay` : les mesures de
// chaque seconde et l'arrêt hors du fil principal. Chaque app garde sa
// présentation (fenêtre, barre, menu) et ses entrées.

import Foundation

@MainActor
public enum PlayStationPart {
    /// Une seconde de la partie : le chemin (direct ou par le relais),
    /// l'agrandissement et les mesures (nil tant qu'il n'y en a pas).
    public struct Second {
        public let route: RouteInfo?
        public let upscale: UpscaleReport?
        public let stats: PlayStationPlay.Stats?
    }

    /// Une fois par seconde tant que la partie (`play`) tient : `each` reçoit
    /// la seconde ; la tâche rendue s'annule à la fin.
    public static func statsLoop(play: @escaping @MainActor () -> PlayStationPlay?,
                                 remote: @escaping @MainActor () -> PlayStation.Remote?,
                                 each: @escaping @MainActor (Second) -> Void) -> Task<Void, Never> {
        Task { @MainActor in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard !Task.isCancelled, let p = play() else { return }
                each(Second(route: remote()?.route, upscale: p.upscaleReport, stats: p.stats()))
            }
        }
    }

    /// Arrête la partie hors du fil principal (`kn_ps_stop` attend sa fin) ;
    /// la console distante (relais) vit jusque-là.
    public nonisolated static func stop(_ play: PlayStationPlay, keeping remote: PlayStation.Remote?) {
        DispatchQueue.global(qos: .userInitiated).async {
            play.stop()
            withExtendedLifetime(remote) {}
        }
    }
}
