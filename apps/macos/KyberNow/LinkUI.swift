// Ce que les apps KyberNow Link d'Apple (Mac, Apple TV, iPhone et iPad)
// disent et font de la même façon (docs/link-ui.md) : les textes de l'accueil,
// des Statistiques et de la Station, leurs chiffres, et le redémarrage ou la
// mise à jour de la Station suivis jusqu'à son retour. Chaque app les dessine
// à sa manière ; un même texte a ainsi la même clé dans les trois catalogues.

import Foundation
import KyberKit

// MARK: - Chiffres

enum LinkFormat {
    private static var french: Bool { LanguageChoice.effective == "fr" }

    /// Une décimale (virgule en français, point en anglais), sans décimale à partir de 100.
    static func decimal(_ value: Double) -> String {
        let text = value >= 100 ? String(Int(value.rounded())) : String(format: "%.1f", value)
        return french ? text.replacingOccurrences(of: ".", with: ",") : text
    }

    /// « 8,9 ms ».
    static func ms(_ value: Double) -> String {
        "\(decimal(value)) ms"
    }

    /// « 42 min », « 1 h 05 ».
    static func duration(_ seconds: TimeInterval) -> String {
        let minutes = Int(seconds / 60)
        if minutes < 60 { return "\(max(1, minutes)) min" }
        return "\(minutes / 60) h \(String(format: "%02d", minutes % 60))"
    }

    /// Le jour d'une session : « aujourd'hui », « hier », « il y a 3 jours », « le 3 octobre ».
    static func day(_ date: Date, now: Date = Date()) -> String {
        let cal = Calendar.current
        if cal.isDateInToday(date) { return String(localized: "aujourd'hui") }
        if cal.isDateInYesterday(date) { return String(localized: "hier") }
        let days = cal.dateComponents([.day], from: cal.startOfDay(for: date), to: cal.startOfDay(for: now)).day ?? 0
        if days < 7 { return String(localized: "il y a \(days) jours") }
        let day = date.formatted(.dateTime.day().month(.wide).locale(Locale(identifier: french ? "fr_FR" : "en_GB")))
        return String(localized: "le \(day)")
    }

    /// « 2560 × 1440 », sans séparateur de milliers.
    static func dims(_ width: Int, _ height: Int) -> String {
        "\(String(width)) × \(String(height))"
    }

    /// Nom d'un format vidéo.
    static func codec(_ codec: VideoCodec) -> String {
        switch codec {
        case .h264: return "H.264"
        case .hevc: return "HEVC"
        case .av1: return "AV1"
        }
    }

    /// Les quatre premiers groupes d'une empreinte : « 7F3A 9C21 B04E 5D88 ».
    static func fingerprint(_ fp: String) -> String {
        let hex = fp.replacingOccurrences(of: "sha256:", with: "").replacingOccurrences(of: ":", with: "").uppercased()
        let groups = stride(from: 0, to: min(hex.count, 16), by: 4).map { i -> String in
            let start = hex.index(hex.startIndex, offsetBy: i)
            return String(hex[start ..< (hex.index(start, offsetBy: 4, limitedBy: hex.endIndex) ?? hex.endIndex)])
        }
        return groups.joined(separator: " ")
    }
}

// MARK: - Accueil

enum LinkText {
    /// État d'un ordinateur : « Recherche… » tant qu'il n'a pas répondu une fois.
    static func status(_ online: Bool?) -> String {
        switch online {
        case nil: return String(localized: "Recherche…")
        case true?: return String(localized: "En ligne")
        case false?: return String(localized: "Hors ligne")
        }
    }

    /// L'écran que la session demandera : virtuel (à la taille de cet écran
    /// ou sur mesure), ou ceux de l'ordinateur, tels quels.
    static func screen(width: Int, height: Int, refresh: Int, virtual: Bool, sizedForThisScreen: Bool) -> String {
        guard virtual else { return String(localized: "Les écrans de l'ordinateur, tels quels") }
        let dims = LinkFormat.dims(width, height)
        return sizedForThisScreen
            ? String(localized: "Écran de \(dims) à \(refresh) Hz, à la taille de cet écran")
            : String(localized: "Écran de \(dims) à \(refresh) Hz")
    }

    /// Erreur sous le nom de l'ordinateur.
    static func problem(_ p: HostProblem) -> String {
        switch p {
        case .passwordRefused: return String(localized: "Mot de passe refusé : mettez-le à jour dans les réglages.")
        case .certificateChanged: return String(localized: "La Station a été réinstallée : vérifiez son empreinte dans les réglages.")
        }
    }

    /// Bouton principal d'un ordinateur hors ligne dont l'app connaît une carte réseau.
    static var wake: String { String(localized: "Réveiller l'ordinateur") }
    static var waking: String { String(localized: "Réveil de l'ordinateur…") }

    /// Apps de salon : « 2 manettes », « 1 manette », « Aucune manette ».
    static func controllers(_ n: Int) -> String {
        switch n {
        case 0: return String(localized: "Aucune manette")
        case 1: return String(localized: "1 manette")
        default: return String(localized: "\(n) manettes")
        }
    }

    /// Libellé accessible du bouton des réglages d'un ordinateur, et titre de ses réglages.
    static func settingsTitle(_ name: String) -> String {
        French.elides(name) ? String(localized: "Réglages d'\(name)") : String(localized: "Réglages de \(name)")
    }
}

/// Élision du français devant un nom d'ordinateur : « Réglages d'Atelier »,
/// « d'Hugo-PC », mais « Réglages de Bureau-Salon ». Un texte où le nom suit
/// « de » existe sous deux clés (« … de %@ », « … d'%@ »), de même traduction
/// anglaise ; `elides` choisit la bonne.
enum French {
    /// Le nom commence par une voyelle (a, e, i, o, u, y, accentuées comprises) ou un h.
    static func elides(_ name: String) -> Bool {
        guard let first = name.trimmingCharacters(in: .whitespacesAndNewlines).first else { return false }
        let base = String(first).folding(options: [.diacriticInsensitive, .caseInsensitive], locale: Locale(identifier: "fr_FR"))
        return ["a", "e", "i", "o", "u", "y", "h"].contains(base)
    }
}

// MARK: - Réglages de l'ordinateur

/// Les six rubriques des réglages d'un ordinateur, dans cet ordre.
enum ComputerSection: String, CaseIterable, Identifiable {
    case picture, sound, controls, network, statistics, station

    var id: String { rawValue }

    var title: String {
        switch self {
        case .picture: return String(localized: "Image")
        case .sound: return String(localized: "Son")
        case .controls: return String(localized: "Commandes")
        case .network: return String(localized: "Réseau")
        case .statistics: return String(localized: "Statistiques")
        case .station: return String(localized: "Station")
        }
    }

    /// Icône Phosphor de la rubrique.
    var icon: String {
        switch self {
        case .picture: return "image"
        case .sound: return "speaker-high"
        case .controls: return "game-controller"
        case .network: return "wifi-high"
        case .statistics: return "chart-line"
        case .station: return "desktop-tower"
        }
    }
}

enum SettingsText {
    static var virtualDisplayHelp: String {
        String(localized: "L'ordinateur crée un écran à la taille de cet appareil pendant la session ; les siens s'éteignent, puis reviennent à la fin.")
    }

    /// Rubrique Réseau : « Réveil par le réseau ».
    static var wakeTitle: String { String(localized: "Réveil par le réseau") }
    static var wakeReady: String { String(localized: "Prêt") }
    static var wakeNone: String { String(localized: "Pas encore d'adresse : connectez-vous une fois à cet ordinateur.") }
    static var wakeHelp: String {
        String(localized: "Allume l'ordinateur à distance quand il est en veille. Il faut aussi l'autoriser sur l'ordinateur (Admin › Système).")
    }

    /// Adresse MAC de la première carte apprise (Ethernet d'abord), ou nil.
    static func wakeAddress(_ host: Host) -> String? {
        host.wake?.first?.mac
    }

    static var sharpTextHelp: String {
        String(localized: "Couleurs complètes : le texte reste net, au prix d'un débit plus élevé.")
    }
}

/// Sous-titres des rubriques (colonne des réglages d'un ordinateur) : un
/// résumé simple, dans la langue de l'app, sans chiffre technique.
enum SectionSummary {
    /// « Comme cet écran · 60 Hz » : la définition dite comme dans la rubrique, et la fréquence.
    static func picture(_ definition: String, refresh: Int?) -> String {
        refresh.map { "\(definition) · \($0) Hz" } ?? definition
    }

    /// « Hier », « Il y a 3 jours » ; sans session : « Pas encore de session ».
    static func statistics(_ host: Host) -> String {
        guard host.lastDuration != nil || host.lastLatencyMs != nil, let date = host.lastSession else {
            return String(localized: "Pas encore de session")
        }
        let day = LinkFormat.day(date)
        return day.prefix(1).uppercased() + day.dropFirst()
    }

    /// « À jour », « Version 0.5.1 disponible » ; Station ancienne : sa version.
    static func station(_ info: HostInfo?) -> String {
        guard let info else { return "—" }
        guard let station = info.station else { return info.stationVersion ?? "—" }
        if station.updateAvailable, let v = station.updateVersion { return StationText.available(v) }
        return StationText.upToDate
    }
}

// MARK: - Statistiques

/// La dernière session d'un ordinateur, telle que la montre la rubrique
/// Statistiques (carte « Dernière session », sur le modèle de l'app Mac).
struct LastSessionSummary {
    let median: Double?
    let p95: Double?
    let duration: TimeInterval?
    let date: Date?
    let width: Int?
    let height: Int?
    let refresh: Int?
    let codec: VideoCodec?
    let mbps: Int?

    /// Nil : pas encore de session (ou d'avant les mesures).
    init?(_ host: Host) {
        guard host.lastLatencyMs != nil || host.lastDuration != nil else { return nil }
        median = host.lastLatencyMs
        p95 = host.lastLatencyP95Ms
        duration = host.lastDuration
        date = host.lastSession
        width = host.lastWidth
        height = host.lastHeight
        refresh = host.lastRefresh
        codec = host.lastCodec
        mbps = host.lastMbps
    }

    /// « 42 min · hier ».
    var when: String {
        [duration.map(LinkFormat.duration), date.map { LinkFormat.day($0) }].compactMap { $0 }.joined(separator: " · ")
    }

    /// « 2560 × 1440 · 120 Hz · HEVC · 80 Mbit/s » (ce qui est connu).
    var transmitted: String {
        var parts: [String] = []
        if let width, let height { parts.append(LinkFormat.dims(width, height)) }
        if let refresh { parts.append("\(refresh) Hz") }
        if let codec { parts.append(LinkFormat.codec(codec)) }
        if let mbps { parts.append("\(mbps) Mbit/s") }
        return parts.joined(separator: " · ")
    }
}

enum StatsText {
    static var lastSession: String { String(localized: "Dernière session") }
    static var median: String { String(localized: "médiane") }
    static var thisComputer: String { String(localized: "Cet ordinateur") }
    static var none: String { String(localized: "Pas encore de session sur cet ordinateur.") }

    static func under(_ p95: Double) -> String {
        String(localized: "95 % des images sous \(LinkFormat.ms(p95))")
    }

    static var help: String {
        String(localized: "Le délai entre l'ordinateur et cet écran. Sous un repère, l'image arrive avant l'image suivante à cette fréquence.")
    }

    static var system: String { String(localized: "Système") }
    static var graphics: String { String(localized: "Carte graphique") }
    static var address: String { String(localized: "Adresse") }
    static var fingerprint: String { String(localized: "Empreinte") }
    /// Station réinstallée : l'empreinte présentée, à comparer avant de la retenir.
    static var presented: String { String(localized: "Empreinte présentée") }
    static var compare: String { String(localized: "Comparez ses quatre premiers groupes à ceux d'Admin (Appareils Link, Ajouter un appareil) avant de lui faire confiance.") }
    static var trust: String { String(localized: "Faire confiance à cette empreinte") }
}

// MARK: - Station

/// Redémarrage ou mise à jour de la Station, suivi jusqu'à son retour.
enum StationTask: Equatable {
    case restarting(since: Date)
    case updating(version: String?, since: Date)
    /// Fini : « La Station a redémarré. », « KyberNow Station 0.5.1 est en place. »
    case done(String)
    case failed(String)

    var working: Bool {
        switch self {
        case .restarting, .updating: return true
        case .done, .failed: return false
        }
    }
}

/// « {appareil} est connecté » : redémarrer ou mettre à jour quand même ?
struct StationConfirm: Identifiable, Equatable {
    enum Kind: Equatable { case restart, update }
    let hostID: UUID
    let kind: Kind
    let device: String

    var id: String { "\(hostID)-\(kind)" }

    var title: String { String(localized: "\(device) est connecté") }

    var message: String {
        kind == .restart
            ? String(localized: "Redémarrer la Station coupera sa session.")
            : String(localized: "La mise à jour coupera sa session.")
    }

    var confirmLabel: String {
        kind == .restart ? String(localized: "Redémarrer quand même") : String(localized: "Mettre à jour quand même")
    }
}

enum StationText {
    static var restart: String { String(localized: "Redémarrer la Station") }
    static var restartHelp: String { String(localized: "Environ 30 secondes. Une session en cours sera coupée.") }
    static var update: String { String(localized: "Mettre à jour la Station") }
    static func updateHelp(_ v: String) -> String { String(localized: "Version \(v). Environ une minute. Une session en cours sera coupée.") }
    static var openAdmin: String { String(localized: "Ouvrir Admin") }
    static var adminHelp: String {
        String(localized: "Admin s'ouvre dans votre navigateur et demande son mot de passe. Le navigateur peut signaler le certificat : la Station signe elle-même sa connexion, c'est normal.")
    }
    static var adminScan: String { String(localized: "Scannez ce code avec votre téléphone pour ouvrir Admin.") }
    static func version(_ v: String) -> String { "KyberNow Station \(v)" }
    static var upToDate: String { String(localized: "À jour") }
    static func available(_ v: String) -> String { String(localized: "Version \(v) disponible") }
    static func requiresSetup(_ v: String) -> String { String(localized: "La version \(v) s'installe sur l'ordinateur, avec KyberNow-Station-Setup.exe.") }
    static var restarting: String { String(localized: "La Station redémarre…") }
    static var updating: String { String(localized: "Mise à jour de la Station…") }
    static var restarted: String { String(localized: "La Station a redémarré.") }
    static func installed(_ v: String) -> String { String(localized: "KyberNow Station \(v) est en place.") }
    static var tooSoon: String { String(localized: "La Station vient de redémarrer. Réessayez dans une minute.") }
    static var tooOld: String { String(localized: "Pour redémarrer ou mettre à jour la Station depuis cet appareil, mettez-la d'abord à jour depuis Admin.") }
    static var notBack: String { String(localized: "La Station tarde à revenir. Ouvrez Admin pour en savoir plus.") }
    static var updateFailed: String { String(localized: "La mise à jour n'a pas abouti. Ouvrez Admin pour en savoir plus.") }
    static var account: String { String(localized: "Compte") }
    static var ownAccess: String { String(localized: "Accès propre à cet appareil") }
    static var password: String { String(localized: "Mot de passe") }
    static var newPassword: String { String(localized: "Nouveau mot de passe") }
    static var passwordHelp: String { String(localized: "Le mot de passe de ce compte, dans Admin, page Utilisateurs.") }
    static var save: String { String(localized: "Enregistrer") }
    static var remove: String { String(localized: "Retirer cet ordinateur") }
    static func removeQuestion(_ name: String) -> String { String(localized: "Retirer \(name) de cet appareil ? Vous pourrez l'ajouter à nouveau.") }
    static var removeConfirm: String { String(localized: "Retirer") }
    static var cancel: String { String(localized: "Annuler") }
}

extension HostStore {
    /// « Accès propre à cet appareil » : Admin le dit (`account.own_device`),
    /// ou l'ordinateur a été ajouté ici par code.
    func ownDevice(_ host: Host) -> Bool {
        status[host.id]?.info?.ownDevice == true || host.pairedByCode == true
    }

    /// « Redémarrer la Station » : `force` après « Redémarrer quand même ».
    func restartStation(_ host: Host, force: Bool = false) {
        guard stationTasks[host.id]?.working != true else { return }
        #if DEBUG
        if demo {
            demoStation(host, task: .restarting(since: Date()), force: force)
            return
        }
        #endif
        guard let console = console(for: host) else { return }
        stationTasks[host.id] = .restarting(since: Date())
        Task {
            do {
                try await console.stationRestart(force: force)
                await follow(host, restart: true, version: nil)
            } catch {
                stationRefused(error, host: host, kind: .restart)
            }
        }
    }

    /// « Mettre à jour la Station » vers la version annoncée par Admin.
    func updateStation(_ host: Host, force: Bool = false) {
        guard stationTasks[host.id]?.working != true else { return }
        let target = status[host.id]?.info?.station?.updateVersion
        #if DEBUG
        if demo {
            demoStation(host, task: .updating(version: target, since: Date()), force: force)
            return
        }
        #endif
        guard let console = console(for: host) else { return }
        stationTasks[host.id] = .updating(version: target, since: Date())
        Task {
            do {
                let version = try await console.stationUpdate(force: force)
                stationTasks[host.id] = .updating(version: version.isEmpty ? target : version, since: Date())
                await follow(host, restart: false, version: version.isEmpty ? target : version)
            } catch {
                stationRefused(error, host: host, kind: .update)
            }
        }
    }

    /// Refus d'Admin : confirmation, message, ou suivi d'une action déjà en cours.
    private func stationRefused(_ error: Error, host: Host, kind: StationConfirm.Kind) {
        guard let e = error as? StationError else {
            stationTasks[host.id] = .failed(error.localizedDescription)
            return
        }
        switch e {
        case let .inSession(device):
            stationTasks[host.id] = nil
            stationConfirm = StationConfirm(hostID: host.id, kind: kind, device: device ?? String(localized: "Un appareil"))
        case .tooSoon:
            stationTasks[host.id] = .failed(StationText.tooSoon)
        case .unsupported:
            stationTasks[host.id] = .failed(StationText.tooOld)
        case let .requiresSetup(message):
            let v = status[host.id]?.info?.station?.updateVersion
            stationTasks[host.id] = .failed(v.map(StationText.requiresSetup) ?? message)
        case .busy:
            // Déjà en cours (depuis Admin ou un autre appareil) : on suit.
            let info = status[host.id]?.info?.station
            let restart = info?.restarting == true || kind == .restart && info?.installing != true
            stationTasks[host.id] = restart ? .restarting(since: Date()) : .updating(version: info?.updateVersion, since: Date())
            Task { await follow(host, restart: restart, version: info?.updateVersion) }
        case let .nothingToInstall(message), let .failed(message):
            stationTasks[host.id] = .failed(message)
        }
    }

    /// Interroge Admin toutes les 2 s jusqu'au retour de la Station (et à
    /// sa nouvelle version pour une mise à jour) : 3 min au plus pour un
    /// redémarrage, 10 min pour une mise à jour.
    private func follow(_ host: Host, restart: Bool, version: String?) async {
        let start = Date()
        let limit: TimeInterval = restart ? 180 : 600
        var wentAway = false
        var installing = false
        while Date().timeIntervalSince(start) < limit {
            try? await Task.sleep(for: .seconds(2))
            guard self.host(host.id) != nil, stationTasks[host.id]?.working == true else { return }
            await refresh(host)
            guard let s = status[host.id] else { continue }
            if !s.online { wentAway = true; continue }
            let station = s.info?.station
            if restart {
                // Admin répond encore un instant avant de s'arrêter.
                if station?.restarting == false, wentAway || Date().timeIntervalSince(start) > 20 {
                    stationTasks[host.id] = .done(StationText.restarted)
                    return
                }
            } else if let v = s.info?.stationVersion, let version, v == version, station?.updateAvailable != true || station?.updateVersion != version {
                stationTasks[host.id] = .done(StationText.installed(v))
                return
            } else if station?.installing == true {
                installing = true
            } else if wentAway || installing, station?.restarting != true {
                // Revenue, mais pas dans la nouvelle version : l'installation a échoué
                // (Admin revient à la précédente).
                stationTasks[host.id] = .failed(StationText.updateFailed)
                return
            }
        }
        stationTasks[host.id] = .failed(StationText.notBack)
    }

    #if DEBUG
    /// Démonstration : une session sur l'ordinateur au premier essai
    /// (`-stationInSession YES`), puis quelques secondes de travail.
    private func demoStation(_ host: Host, task: StationTask, force: Bool) {
        if !force, UserDefaults.standard.bool(forKey: "stationInSession") {
            var kind = StationConfirm.Kind.update
            if case .restarting = task { kind = .restart }
            stationConfirm = StationConfirm(hostID: host.id, kind: kind, device: "iPhone de Camille")
            return
        }
        stationTasks[host.id] = task
        Task {
            try? await Task.sleep(for: .seconds(6))
            switch task {
            case .restarting:
                stationTasks[host.id] = .done(StationText.restarted)
            case let .updating(version, _):
                let v = version ?? "0.5.1"
                if let info = status[host.id]?.info {
                    setDemoStatus(HostStatus(online: true, info: HostInfo(hostname: info.hostname, gpu: info.gpu, av1: info.av1, virtualDisplay: info.virtualDisplay,
                                                                           speakers: info.speakers, os: info.os, adminVersion: v,
                                                                           station: StationInfo(version: v))), for: host.id)
                }
                stationTasks[host.id] = .done(StationText.installed(v))
            default: break
            }
        }
    }
    #endif
}

#if DEBUG
// MARK: - Démonstration

/// Ordinateurs de démonstration, pour vérifier les écrans sans Station
/// (`-demo YES`, en Debug ; préférences à part, rien sur le réseau) :
///
/// - « Tour du bureau », en ligne, avec une dernière session complète ;
/// - « Atelier », hors ligne ;
/// - « Mac mini du studio », en ligne, jamais ouvert, sans dernière session.
///
/// Arguments : `-demoEmpty YES` (aucun ordinateur), `-demoState online|offline|searching`
/// (état de tous), `-demoStation current|update|setup|old|installing` (Station de la Tour),
/// `-demoProblem password|certificate`, `-stationInSession YES` (confirmation au premier essai).
extension HostStore {
    func loadDemo() {
        for h in hosts { remove(h) }
        let args = UserDefaults.standard
        guard !args.bool(forKey: "demoEmpty") else { return }
        var tour = Host(name: "Tour du bureau", address: "192.168.1.42", consolePort: 9443, username: "camille")
        tour.consoleFingerprint = "sha256:7f3a9c21b04e5d88a1c2e3f405162738495a6b7c8d9e0f1a2b3c4d5e6f708192"
        let yesterday = Calendar.current.date(byAdding: .day, value: -1, to: Date())!.addingTimeInterval(-3600)
        tour.lastSession = yesterday
        tour.lastLatencyMs = 8.9
        tour.lastLatencyP95Ms = 14.2
        tour.lastDuration = 42 * 60
        (tour.lastWidth, tour.lastHeight, tour.lastRefresh, tour.lastCodec, tour.lastMbps) = (2560, 1440, 120, .hevc, 80)
        tour.wake = [WakeTarget(mac: "3C:7C:3F:12:AB:CD", ipv4: "192.168.1.42", broadcast: "192.168.1.255", kind: "ethernet", armed: true)]
        var atelier = Host(name: "Atelier", address: "10.8.0.12", consolePort: 9443, username: "camille")
        atelier.wake = [WakeTarget(mac: "D8:BB:C1:04:5E:72", ipv4: "10.8.0.12", broadcast: "10.8.0.255", kind: "ethernet")]
        atelier.lastSession = Date().addingTimeInterval(-3 * 86400)
        atelier.lastLatencyMs = 11.4
        atelier.lastLatencyP95Ms = 19.8
        atelier.lastDuration = 75 * 60
        (atelier.lastWidth, atelier.lastHeight, atelier.lastRefresh, atelier.lastCodec, atelier.lastMbps) = (1920, 1080, 60, .h264, 45)
        var studio = Host(name: "Mac mini du studio", address: "192.168.1.20", consolePort: 9443, username: "Apple-TV-1234")
        studio.pairedByCode = true
        studio.settings.apply(.bureau)
        for h in [tour, atelier, studio] { add(h, password: "demo") }

        let station: StationInfo? = switch args.string(forKey: "demoStation") ?? "current" {
        case "update", "updating": StationInfo(version: "0.5.0", updateAvailable: true, updateVersion: "0.5.1", installable: true)
        case "setup": StationInfo(version: "0.5.0", updateAvailable: true, updateVersion: "0.6.0", requiresSetup: true)
        case "installing": StationInfo(version: "0.5.0", updateAvailable: true, updateVersion: "0.5.1", installable: true, installing: true)
        case "old": nil
        default: StationInfo(version: "0.5.0")
        }
        let state = args.string(forKey: "demoState") ?? "online"
        let problem: HostProblem? = switch args.string(forKey: "demoProblem") {
        case "password": .passwordRefused
        case "certificate": .certificateChanged
        default: nil
        }
        func put(_ h: Host, online: Bool, _ info: HostInfo, problem: HostProblem? = nil) {
            switch state {
            case "searching": setDemoStatus(nil, for: h.id)
            case "offline": setDemoStatus(HostStatus(online: false, info: info), for: h.id)
            default: setDemoStatus(HostStatus(online: online, info: info, problem: problem), for: h.id)
            }
        }
        put(tour, online: true, HostInfo(hostname: tour.name, gpu: "NVIDIA GeForce RTX 4080", av1: true, os: "Windows 11 Pro 24H2",
                                          adminVersion: station?.version ?? "0.4.9", station: station), problem: problem)
        put(atelier, online: false, HostInfo(hostname: atelier.name, gpu: "AMD Radeon RX 7800 XT", os: "Windows 10 Pro 22H2",
                                              adminVersion: "0.5.0", station: StationInfo(version: "0.5.0")))
        put(studio, online: true, HostInfo(hostname: studio.name, gpu: "Apple M4", virtualDisplay: false, os: "macOS 26.1",
                                            adminVersion: "0.5.0", station: StationInfo(version: "0.5.0")))
        if args.string(forKey: "demoStation") == "restarting" { stationTasks[tour.id] = .restarting(since: Date()) }
        if args.string(forKey: "demoStation") == "updating" { stationTasks[tour.id] = .updating(version: "0.5.1", since: Date()) }
    }
}
#endif
