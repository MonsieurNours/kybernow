// Lancement animé (maquettes iOS, planche 10 : « Lancement » et « Images
// clés »). iOS montre d'abord une image fixe (UILaunchScreen : encre unie, le
// symbole éteint au centre) ; cette vue démarre sur exactement la même image,
// puis :
//
//   0 → 0,2 s    rien ne bouge (le temps que la vue soit prête) ;
//   0,2 → 0,6 s  allumage de la charte : le symbole grésille (trois éclairs)
//                puis reste allumé ;
//   0,8 → 1,1 s  le nom KyberNow apparaît, centré sous le symbole ;
//   → 1,8 s      la scène tient, allumée ;
//   1,8 → 2,4 s  l'accueil prend la main, sans fondu : son propre symbole
//                et son nom partent d'ici et filent à leur place en haut à
//                gauche (HomeView), pendant que son décor arrive en fondu et
//                que ses blocs montent.
//
// « Réduire les animations » : le symbole s'allume sans grésiller, puis un
// simple fondu vers l'accueil, le symbole déjà à sa place.

import SwiftUI
import UIKit

struct LaunchView: View {
    var done: () -> Void

    /// Taille du symbole au centre : celle de l'image fixe du système
    /// (LaunchMark, UILaunchScreen), lue dans le catalogue pour qu'aucun saut
    /// ne trahisse le passage de l'image du système à l'app. Les maquettes
    /// le veulent à 96 pt.
    static let markSize: CGFloat = UIImage(named: "LaunchMark")?.size.width ?? 96
    /// Le nom, sous le symbole, et l'écart entre les deux.
    static let nameSize: CGFloat = 28
    static let nameGap: CGFloat = 22

    @EnvironmentObject private var app: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var level: NeonLevel = .off
    @State private var named = false
    /// L'accueil a pris le relais : ses symbole et nom sont à la même place,
    /// ceux-ci s'effacent d'un coup pour ne pas laisser de double en fondu.
    @State private var handedOver = false

    var body: some View {
        ZStack {
            Theme.background.ignoresSafeArea()
            // Centré dans la zone sûre, comme l'image du système
            // (UIImageRespectsSafeAreaInsets) : aucun saut au premier instant.
            Mark(size: Self.markSize, level: level)
                .modifier(LaunchGlow(level: level))
                .overlay(alignment: .top) {
                    Wordmark(size: Self.nameSize)
                        .fixedSize()
                        .offset(y: Self.markSize + Self.nameGap)
                        .opacity(named ? 1 : 0)
                }
                .opacity(handedOver ? 0 : 1)
        }
        .accessibilityElement()
        .accessibilityLabel(Text(verbatim: "KyberNow"))
        .task { await run() }
    }

    private func run() async {
        let start = ContinuousClock.now
        func at(_ seconds: Double) async {
            try? await Task.sleep(until: start + .milliseconds(Int(seconds * 1000)), clock: .continuous)
        }

        if reduceMotion {
            await at(0.2)
            withAnimation(.easeOut(duration: 0.25)) { level = .lit; named = true }
            await at(0.7)
            done()
            return
        }

        // Le grésillement : chaque palier rejoint le suivant en ligne droite,
        // comme les images clés de la maquette (3 éclairs en 0,45 s).
        let steps: [(time: Double, level: NeonLevel)] = [
            (0.25, .flash), (0.29, .off), (0.34, .lit), (0.38, .off), (0.44, .flash), (0.48, .hover), (0.57, .lit),
        ]
        var previous = 0.2
        await at(previous)
        for step in steps {
            withAnimation(.linear(duration: step.time - previous)) { level = step.level }
            previous = step.time
            await at(step.time)
        }

        await at(0.78)
        withAnimation(.linear(duration: 0.3)) { named = true }

        await at(1.8)
        // Passage de relais sans fondu : l'accueil apparaît d'un coup, son
        // symbole et son nom exactement là où sont ceux-ci, puis il mène
        // lui-même son arrivée (vol, décor en fondu, blocs en cascade). Un
        // fondu croisé ferait clignoter le symbole pendant une image.
        var handover = Transaction()
        handover.disablesAnimations = true
        withTransaction(handover) {
            handedOver = true
            app.launched = true
        }
        // `done` tient le contrat de la vue ; l'accueil est déjà là, il ne change plus rien.
        done()
    }
}

/// Le halo du symbole, aux niveaux des images clés du lancement : éclair
/// (blanc, Spectre, halo large), allumé stable, veille. Sans animation
/// propre : le grésillement en règle le rythme lui-même.
struct LaunchGlow: ViewModifier {
    var level: NeonLevel
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency

    func body(content: Content) -> some View {
        let g: (core: Color, a: Double, ra: CGFloat, mid: Color, b: Double, rb: CGFloat, halo: Double, rc: CGFloat) = switch level {
        case .flash: (.white, 1, 2, Theme.accent, 1, 9, 1, 28)
        case .lit: (Theme.neonCore, 1, 1, Theme.accent, 1, 5, 0.8, 16)
        case .hover: (Theme.accent, 0.8, 2, Theme.accentFill, 0.45, 8, 0, 8)
        case .off: (Theme.neonCore, 0, 1, Theme.accent, 0, 5, 0, 16)
        }
        let k = reduceTransparency ? 0.0 : 1.0
        return content
            .shadow(color: g.core.opacity(g.a * k), radius: g.ra / 2)
            .shadow(color: g.mid.opacity(g.b * k), radius: g.rb / 2)
            .shadow(color: Theme.accentFill.opacity(g.halo * k), radius: g.rc / 2)
    }
}
