// L'app d'essai : lance le banc au démarrage, montre chaque ligne à l'écran
// et l'écrit sur la sortie standard (lue par `devicectl … --console`), puis
// quitte avec le code 0 (tout bon) ou 1.
import os
import SwiftUI

enum KyberLog { static let video = Logger(subsystem: "fr.leslignesclaires.kybernow.pyrowavebench", category: "video") }

@MainActor
final class BenchLog: ObservableObject {
    @Published var lines: [String] = []
}

@main
struct PyroWaveBenchApp: App {
    @StateObject private var log = BenchLog()

    var body: some Scene {
        WindowGroup {
            ScrollView {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(Array(log.lines.enumerated()), id: \.offset) { _, line in
                        Text(line).font(.system(.footnote, design: .monospaced))
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(40)
            }
            .task {
                let log = self.log
                let ok = await Task.detached(priority: .userInitiated) {
                    Bench.run { line in
                        print(line)
                        fflush(stdout)
                        Task { @MainActor in log.lines.append(line) }
                    }
                }.value
                print(ok ? "OK" : "ÉCHEC")
                fflush(stdout)
                try? await Task.sleep(nanoseconds: 500_000_000)
                exit(ok ? 0 : 1)
            }
        }
    }
}
