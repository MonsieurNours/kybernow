// Ordinateurs enregistrés, réglages de connexion et Trousseau.

import Foundation
import KyberKit
import Security

enum Profile: String, Codable, CaseIterable, Identifiable {
    /// Auto (docs/profil-auto.md) : les réglages choisis par le test des
    /// conditions, gardés pour chaque ordinateur (`ConnectionSettings.autoResult`).
    case jeu, bureau, max, auto

    var id: String { rawValue }

    var label: String {
        switch self {
        case .jeu: return String(localized: "Jeu")
        case .bureau: return String(localized: "Bureau")
        case .max: return "Max"
        case .auto: return String(localized: "Auto")
        }
    }

    /// Icône du profil (Phosphor, `Icons/…` des catalogues) ; Auto : la baguette.
    var icon: String {
        switch self {
        case .jeu: return "game-controller"
        case .bureau: return "article"
        case .max: return "lightning"
        case .auto: return "sparkle"
        }
    }

    /// Phrase du profil, sous le sélecteur (accueil et réglages ; docs/link-ui.md).
    var hint: String {
        switch self {
        case .jeu: return String(localized: "La réactivité d'abord, pour jouer.")
        case .bureau: return String(localized: "Un texte net, pour travailler.")
        case .max: return String(localized: "La meilleure image que cet appareil affiche.")
        case .auto: return String(localized: "Les meilleurs réglages pour ce réseau et cet appareil.")
        }
    }

    /// Phrase d'un profil, ou de « Personnalisé » (`nil`).
    static func hint(_ profile: Profile?) -> String {
        profile?.hint ?? String(localized: "Vos propres réglages.")
    }

    /// Nom d'un profil, ou « Personnalisé » (`nil`).
    static func label(_ profile: Profile?) -> String {
        profile?.label ?? String(localized: "Personnalisé")
    }
}

enum DisplayChoice: String, Codable, CaseIterable {
    /// Écran virtuel à la taille et à la fréquence de l'écran de ce Mac.
    case match
    /// Écran virtuel à la taille de la fenêtre KyberNow (session fenêtrée).
    case window
    /// Écran virtuel au mode choisi.
    case custom
    /// Pas d'écran virtuel : les écrans de l'ordinateur tels quels.
    case physical
}

enum Transport: String, Codable, CaseIterable {
    case fast, reliable

    var label: String { self == .fast ? String(localized: "Rapide") : String(localized: "Fiable") }

    /// Aide du réglage « Transport de l'image » (docs/link-ui.md).
    static var help: String {
        String(localized: "Rapide convient au réseau local et au jeu. Fiable renvoie ce qui se perd, au prix d'un peu de délai.")
    }
}

/// Souris pendant la session.
/// Réglage « Écrans » (apps Mac, Windows et Linux, docs/multi-ecrans.md) :
/// Auto (tous les écrans en Bureau et Max, un seul en Jeu et en
/// Personnalisé), Cet écran, Tous les écrans. Le cœur décide avec les mêmes
/// règles partout ; changer ce réglage ne rend pas le profil « Personnalisé ».
enum ScreensChoice: String, Codable, CaseIterable {
    case auto, this, all

    var label: String {
        switch self {
        case .auto: return String(localized: "Auto")
        case .this: return String(localized: "Cet écran")
        case .all: return String(localized: "Tous les écrans")
        }
    }
}

/// Mode immersif (docs/link-ui.md) : tout le clavier, raccourcis du Mac
/// compris, va à l'ordinateur.
enum ImmersiveChoice: String, Codable, CaseIterable {
    case fullScreen
    case always
    case never

    var label: String {
        switch self {
        case .fullScreen: return String(localized: "En plein écran")
        case .always: return String(localized: "Toujours")
        case .never: return String(localized: "Jamais")
        }
    }
}

enum MouseChoice: String, Codable, CaseIterable {
    /// Mode jeu quand un jeu est en plein écran sur l'ordinateur.
    case auto
    /// Pointeur du Mac, positions absolues : le bureau, les vidéos.
    case desktop
    /// Déplacements bruts, pointeur du Mac caché : viser dans un jeu.
    case game

    var label: String {
        switch self {
        case .auto: return String(localized: "Auto")
        case .desktop: return String(localized: "Souris du bureau")
        case .game: return String(localized: "Souris des jeux")
        }
    }

    var hint: String {
        switch self {
        case .auto: return String(localized: "Souris des jeux dès qu'un jeu est en plein écran sur l'ordinateur et cache son pointeur ; souris du bureau le reste du temps. Les navigateurs et lecteurs vidéo en plein écran gardent la souris du bureau.")
        case .desktop: return String(localized: "Le pointeur du Mac, toujours : bureau, vidéos, jeux à la souris (stratégie, cartes).")
        case .game: return String(localized: "Déplacements bruts de la souris, sans l'accélération de macOS, pointeur caché : pour viser.")
        }
    }

    var streamMode: StreamView.MouseMode {
        switch self {
        case .auto: return .auto
        case .desktop: return .desktop
        case .game: return .game
        }
    }
}

enum AudioChoice: String, Codable, CaseIterable {
    case auto, stereo, surround51, surround71, off

    var label: String {
        switch self {
        case .auto: return String(localized: "Auto")
        case .stereo: return String(localized: "Stéréo")
        case .surround51: return "5.1"
        case .surround71: return "7.1"
        case .off: return String(localized: "Sur l'ordinateur")
        }
    }

    var hint: String {
        switch self {
        case .auto: return String(localized: "Auto : stéréo, quelle que soit la sortie de ce Mac.")
        case .stereo: return String(localized: "Deux canaux : idéal au casque ou sur les haut-parleurs du Mac.")
        case .surround51: return String(localized: "Six canaux. Il faut une sortie 5.1 sur ce Mac (ampli HDMI, barre de son).")
        case .surround71: return String(localized: "Huit canaux. Il faut une sortie 7.1 sur ce Mac.")
        case .off: return String(localized: "Le son reste sur l'ordinateur.")
        }
    }

    var channels: Int {
        switch self {
        case .auto: return AudioOutput.channels()
        case .stereo: return 2
        case .surround51: return 6
        case .surround71: return 8
        case .off: return 0
        }
    }
}

/// Manette vue par l'ordinateur (`gamepad_force_xbox360` de la session, qui
/// prime sur le réglage de la console ; celui-ci ne reste qu'un défaut pour
/// les autres clients).
enum GamepadChoice: String, Codable, CaseIterable {
    /// Toujours une manette Xbox 360 : celle que tous les jeux reconnaissent.
    case xbox360
    /// Le modèle réel : une manette Sony devient une DualShock 4 (pavé
    /// tactile, barre lumineuse, pour les jeux qui s'en servent).
    case faithful

    var label: String {
        switch self {
        case .xbox360: return "Xbox 360"
        case .faithful: return String(localized: "Fidèle")
        }
    }

    var hint: String {
        switch self {
        case .xbox360: return String(localized: "Toutes les manettes, PlayStation comprises, deviennent des manettes Xbox 360 sur l'ordinateur : celles que tous les jeux reconnaissent.")
        case .faithful: return String(localized: "Une manette Sony reste une DualShock 4 sur l'ordinateur (pavé tactile, barre lumineuse pour les jeux qui s'en servent) ; les autres restent des Xbox 360. Certains jeux anciens ne la voient pas.")
        }
    }
}

/// Manette à l'écran (iPhone, iPad, Android ; docs/link-ui.md) : ses deux
/// dispositions fixes. Les libellés sont dans l'app qui l'affiche.
enum ScreenPadLayout: String, Codable, CaseIterable {
    /// Une manette Xbox entière.
    case full
    /// Le stick gauche, la croix, A, B, X, Y et Menu.
    case simple
}

enum ScreenPadSize: String, Codable, CaseIterable {
    case small, medium, large
}

/// Retour haptique de la manette à l'écran.
enum ScreenPadHaptics: String, Codable, CaseIterable {
    case off, light, medium, strong
}

/// Réglages de connexion d'un ordinateur. Un profil remplit l'image et le
/// réseau ; toucher à l'un de ces réglages rend le profil « Personnalisé ».
struct ConnectionSettings: Codable, Hashable {
    /// `nil` : Personnalisé.
    var profile: Profile? = .jeu
    var display: DisplayChoice = .match
    var customWidth = 2560
    var customHeight = 1440
    var customRefresh = 60
    var codec: VideoCodec = .hevc
    var yuv444 = false
    var bitrateMbps = 60
    var transport: Transport = .fast
    var autoReconnect = true
    var commandAsControl = true
    /// Mode immersif : En plein écran, Toujours, Jamais.
    var immersive: ImmersiveChoice = .fullScreen
    var audio: AudioChoice = .auto
    /// Synchro verticale : sans elle, chaque image s'affiche dès qu'elle est
    /// décodée, quitte à couper l'écran en deux (déchirure).
    var vsync = true
    /// « Comme cet écran » ramené à la 4K au plus (écrans 5K, 6K).
    var limit4K = false
    /// Écrans de la session (Mac) : Auto, Cet écran, Tous les écrans.
    var screens: ScreensChoice = .auto
    /// Souris : Auto (jeu en plein écran), Bureau ou Jeu.
    var mouse: MouseChoice = .auto
    /// Presse-papiers partagé avec l'ordinateur (texte).
    var clipboard = true
    /// « Définition de la caméra » (docs/webcam.md) : 1080p, sinon 720p.
    /// Hors profil (comme le HDR) : la changer ne fait pas « Personnalisé ».
    var camera1080 = false
    /// Garder la dernière image de la session sur ce Mac (fond de l'accueil).
    var keepLastImage = true
    /// Manette vue par l'ordinateur : Xbox 360 (compatible) ou fidèle.
    var gamepad: GamepadChoice = .xbox360
    /// Manette à l'écran (iPhone, iPad) : affichée (bouton de la barre de
    /// session), sa disposition, sa taille, son opacité (0,2 à 1) et son
    /// retour haptique. Les autres apps lisent et gardent ces champs sans
    /// s'en servir.
    var screenPad = false
    var screenPadLayout: ScreenPadLayout = .full
    var screenPadSize: ScreenPadSize = .medium
    var screenPadOpacity = 0.6
    var screenPadHaptics: ScreenPadHaptics = .medium
    /// HDR (docs/hdr.md) : le choix de cet ordinateur, allumé la première
    /// fois (le HDR est automatique), gardé ensuite. À part du profil : le
    /// changer ne fait pas passer en « Personnalisé ». Le cœur ne le demande
    /// que sur un écran HDR, à une Station qui le propose (`hdrState`).
    var hdr = true
    /// « Débit adaptatif » (docs/profil-auto.md) : allumé par défaut, gardé
    /// pour cet ordinateur ; le débit baisse quand le réseau sature, puis
    /// remonte. À part du profil : le changer ne fait pas passer en « Personnalisé ».
    var adaptive = true
    /// Le résultat du test du profil Auto pour cet ordinateur (`AutoResult` du
    /// cœur en JSON), gardé à « Appliquer » ; nil : jamais testé.
    var autoResult: String?
    /// Plafond de l'encodeur de cet ordinateur, en Mbit/s par écran
    /// (docs/profil-auto.md, « L'encodeur de l'ordinateur ») : trouvé par le
    /// test Auto ou par une session qui a dû redescendre ; la session ne
    /// demande jamais plus. À part du profil ; nil : aucun connu.
    var encoderMax: Int?
    /// PyroWave choisi à la main : son débit suit l'écran (définition,
    /// fréquence, 4:4:4) tant qu'on n'a pas bougé le curseur du débit
    /// (`PyroWaveRate`) ; le curseur le fige.
    var pyrowaveRateFollows = false

    /// Le résultat gardé du test du profil Auto, lu.
    var auto: AutoTestResult? { autoResult.flatMap(AutoTestResult.init(json:)) }

    /// Le profil prend PyroWave par câble (`Profile::takes_pyrowave` du
    /// cœur) : Max, et Auto quand son test l'a choisi ; Jeu et Bureau restent en HEVC.
    var takesPyroWave: Bool { profile == .max || (profile == .auto && codec == .pyrowave) }

    static let minBitrate = 5
    /// Le plus que Kyber accepte (débit en bits/s sur 32 bits).
    static let maxBitrate = 4294

    /// Applique un profil, ramené à ce que ce Mac décode en matériel : sans
    /// HEVC, du H.264 ; sans 4:4:4 matériel, du 4:2:0 avec un peu plus de débit.
    mutating func apply(_ profile: Profile) {
        // Auto : les réglages du test gardé ; sans test, rien ne change (l'app
        // ouvre alors le panneau du test, `AutoProfile.choose`).
        if profile == .auto {
            if let r = auto { apply(auto: r) }
            return
        }
        self.profile = profile
        let caps = DecoderCapabilities.current
        codec = caps.hevc ? .hevc : .h264
        switch profile {
        case .jeu: yuv444 = false; bitrateMbps = 60; transport = .fast
        case .bureau: yuv444 = true; bitrateMbps = 90; transport = .reliable
        case .max: yuv444 = true; bitrateMbps = 300; transport = .fast
        case .auto: break
        }
        if yuv444, !Self.decodes444(codec) {
            yuv444 = false
            bitrateMbps += bitrateMbps / 3
        }
        if codec == .h264 { bitrateMbps += bitrateMbps / 2 }
    }

    /// Auto avec ce résultat du test : le profil, ses réglages
    /// (`AutoResult::settings` du cœur : format, 4:4:4, débit, transport), le
    /// débit adaptatif allumé, et la définition et la fréquence choisies
    /// (« Comme cet écran » quand le test a gardé celles de l'écran, sinon
    /// une définition sur mesure). Le résultat est gardé pour cet ordinateur.
    mutating func apply(auto r: AutoTestResult) {
        // Un nouveau résultat seulement : le plafond trouvé par une session
        // après le test est plus récent que le résultat gardé.
        if r.json != autoResult { encoderMax = r.nextEncoderMax(previous: encoderMax) }
        autoResult = r.json
        profile = .auto
        let s = r.settings(wired: NetworkLink.shared.wired)
        codec = s?.codec ?? r.codec
        yuv444 = s?.yuv444 ?? r.yuv444
        bitrateMbps = min(max(s?.mbps ?? r.mbps, Self.minBitrate), Self.maxBitrate)
        transport = (s?.reliable ?? r.reliable) ? .reliable : .fast
        adaptive = true
        if r.matchesDisplay {
            if display == .custom { display = .match }
        } else {
            display = .custom
            (customWidth, customHeight, customRefresh) = (r.width, r.height, r.refresh)
        }
    }

    /// Le 4:4:4 n'est proposé que si ce Mac le décode en matériel. En AV1,
    /// les décodeurs Apple ne lisent que le profil Main (4:2:0) : un flux
    /// 4:4:4 (profil High) ne s'ouvrirait pas, toutes les images seraient perdues.
    static func decodes444(_ codec: VideoCodec) -> Bool {
        switch codec {
        case .hevc: return DecoderCapabilities.current.hevc444
        // PyroWave : 4:4:4 presque sans coût, décodé par le GPU (docs/pyrowave.md).
        case .pyrowave: return DecoderCapabilities.current.pyrowave
        case .av1, .h264: return false
        }
    }

    /// Débit des profils, calé sur la 4K à 60 Hz.
    static let referencePixelRate = 3840.0 * 2160 * 60

    /// Débit de la session pour `w` × `h` à `hz` : un profil suit le nombre de
    /// pixels par seconde (moins que proportionnellement : l'encodeur profite
    /// des détails plus fins), un débit réglé à la main reste tel quel.
    func bitrate(width w: Int, height h: Int, refresh hz: Int) -> Int {
        guard profile != nil, w > 0, h > 0, hz > 0 else { return bitrateMbps }
        let scale = pow(Double(w * h * hz) / Self.referencePixelRate, 0.75)
        let mbps = (Double(bitrateMbps) * min(max(scale, 0.4), 3)).rounded()
        return min(max(Int(mbps), Self.minBitrate), Self.maxBitrate)
    }

    var options: StreamOptions { options(width: 0, height: 0, refresh: 0) }

    func options(width: Int, height: Int, refresh: Int) -> StreamOptions {
        var o = StreamOptions()
        o.codec = codec
        o.yuv444 = yuv444
        o.bitrate = UInt32(bitrate(width: width, height: height, refresh: refresh)) * 1_000_000
        o.reliable = transport == .reliable
        o.audio = audio != .off
        o.yuv444 = yuv444 && Self.decodes444(codec)
        o.vsync = vsync
        o.clipboard = clipboard
        o.gamepadAsXbox360 = gamepad == .xbox360
        return o
    }

    var profileLabel: String { Profile.label(profile) }

    /// La puce « HDR » de cet ordinateur sur un écran HDR ou non : cachée,
    /// active, ou inactive avec sa raison (le format choisi compte : H.264 n'a pas de HDR).
    func hdrState(_ info: HostInfo?, screenHDR: Bool) -> HDRState {
        HDRSupport.state(offer: info?.hdr, profile: profile?.rawValue, codec: codec, screenHDR: screenHDR)
    }

    /// Le HDR sera demandé : « Texte net (4:4:4) » inactif, le profil Bureau
    /// en 4:2:0 (le HDR n'existe qu'en 4:2:0 ; le cœur l'impose aussi).
    func hdrOn(_ info: HostInfo?, screenHDR: Bool) -> Bool {
        hdrState(info, screenHDR: screenHDR).on(hdr)
    }

    /// Profil dit à la console (activité, fil des événements) : toujours le
    /// libellé français, quelle que soit la langue de l'interface.
    var consoleProfile: String {
        switch profile {
        case .jeu?: return "Jeu"
        case .bureau?: return "Bureau"
        case .max?: return "Max"
        case .auto?: return "Auto"
        case nil: return "Personnalisé"
        }
    }

    /// Si le mode de l'écran de l'ordinateur change en session (un jeu qui
    /// passe en 1080p) : Jeu laisse faire, Bureau le fait remettre ; sinon
    /// (`default`), la console décide (son réglage « Si un jeu change la
    /// définition ») : dit explicitement, pour qu'un profil changé en session
    /// défasse le choix du précédent.
    var modePolicy: String {
        switch profile {
        case .jeu?: return "follow"
        case .bureau?: return "restore"
        default: return "default"
        }
    }
}

extension ConnectionSettings {
    // Lecture tolérante : un réglage ajouté par une version récente garde
    // sa valeur par défaut au lieu de faire perdre les ordinateurs enregistrés.
    private enum Keys: String, CodingKey {
        case profile, display, customWidth, customHeight, customRefresh, codec, yuv444, bitrateMbps, transport
        case autoReconnect, commandAsControl, immersive, audio, vsync, limit4K, screens, mouse, clipboard, keepLastImage, gamepad
        case screenPad, screenPadLayout, screenPadSize, screenPadOpacity, screenPadHaptics, hdr
        case adaptive, autoResult, encoderMax, pyrowaveRateFollows, camera1080
    }

    init(from decoder: Decoder) throws {
        self.init()
        let c = try decoder.container(keyedBy: Keys.self)
        profile = try? c.decodeIfPresent(Profile.self, forKey: .profile)
        display = (try? c.decodeIfPresent(DisplayChoice.self, forKey: .display)) ?? display
        customWidth = (try? c.decodeIfPresent(Int.self, forKey: .customWidth)) ?? customWidth
        customHeight = (try? c.decodeIfPresent(Int.self, forKey: .customHeight)) ?? customHeight
        customRefresh = (try? c.decodeIfPresent(Int.self, forKey: .customRefresh)) ?? customRefresh
        codec = (try? c.decodeIfPresent(VideoCodec.self, forKey: .codec)) ?? codec
        yuv444 = (try? c.decodeIfPresent(Bool.self, forKey: .yuv444)) ?? yuv444
        bitrateMbps = (try? c.decodeIfPresent(Int.self, forKey: .bitrateMbps)) ?? bitrateMbps
        transport = (try? c.decodeIfPresent(Transport.self, forKey: .transport)) ?? transport
        autoReconnect = (try? c.decodeIfPresent(Bool.self, forKey: .autoReconnect)) ?? autoReconnect
        commandAsControl = (try? c.decodeIfPresent(Bool.self, forKey: .commandAsControl)) ?? commandAsControl
        immersive = (try? c.decodeIfPresent(ImmersiveChoice.self, forKey: .immersive)) ?? immersive
        audio = (try? c.decodeIfPresent(AudioChoice.self, forKey: .audio)) ?? audio
        vsync = (try? c.decodeIfPresent(Bool.self, forKey: .vsync)) ?? vsync
        limit4K = (try? c.decodeIfPresent(Bool.self, forKey: .limit4K)) ?? limit4K
        screens = (try? c.decodeIfPresent(ScreensChoice.self, forKey: .screens)) ?? screens
        mouse = (try? c.decodeIfPresent(MouseChoice.self, forKey: .mouse)) ?? mouse
        clipboard = (try? c.decodeIfPresent(Bool.self, forKey: .clipboard)) ?? clipboard
        keepLastImage = (try? c.decodeIfPresent(Bool.self, forKey: .keepLastImage)) ?? keepLastImage
        gamepad = (try? c.decodeIfPresent(GamepadChoice.self, forKey: .gamepad)) ?? gamepad
        screenPad = (try? c.decodeIfPresent(Bool.self, forKey: .screenPad)) ?? screenPad
        screenPadLayout = (try? c.decodeIfPresent(ScreenPadLayout.self, forKey: .screenPadLayout)) ?? screenPadLayout
        screenPadSize = (try? c.decodeIfPresent(ScreenPadSize.self, forKey: .screenPadSize)) ?? screenPadSize
        screenPadOpacity = (try? c.decodeIfPresent(Double.self, forKey: .screenPadOpacity)).map { min(1, max(0.2, $0)) } ?? screenPadOpacity
        screenPadHaptics = (try? c.decodeIfPresent(ScreenPadHaptics.self, forKey: .screenPadHaptics)) ?? screenPadHaptics
        hdr = (try? c.decodeIfPresent(Bool.self, forKey: .hdr)) ?? hdr
        adaptive = (try? c.decodeIfPresent(Bool.self, forKey: .adaptive)) ?? adaptive
        autoResult = try? c.decodeIfPresent(String.self, forKey: .autoResult)
        encoderMax = (try? c.decodeIfPresent(Int.self, forKey: .encoderMax)).flatMap { $0 > 0 ? $0 : nil }
        pyrowaveRateFollows = (try? c.decodeIfPresent(Bool.self, forKey: .pyrowaveRateFollows)) ?? pyrowaveRateFollows
        camera1080 = (try? c.decodeIfPresent(Bool.self, forKey: .camera1080)) ?? camera1080
        // Auto sans test lisible (résultat perdu) : Personnalisé, réglages gardés.
        if profile == .auto, auto == nil { profile = nil }
    }
}

struct Host: Codable, Identifiable, Hashable {
    var id = UUID()
    var name: String
    var address: String
    var consolePort: UInt16
    var username: String
    /// Port du flux Kyber, appris auprès de la console.
    var streamPort: UInt16?
    /// Empreinte du certificat de la console, retenue au premier contact.
    var consoleFingerprint: String?
    var settings = ConnectionSettings()
    /// Début de la dernière session (l'ordre de l'accueil, « jamais ouvert »).
    var lastSession: Date?
    /// Dernière session d'au moins 3 s (rubrique Statistiques) : latence
    /// médiane et 95e centile en millisecondes, durée.
    var lastLatencyMs: Double?
    var lastLatencyP95Ms: Double?
    var lastDuration: TimeInterval?
    /// Ce qu'elle a transmis : définition, fréquence, format, débit en Mbit/s.
    var lastWidth: Int?
    var lastHeight: Int?
    var lastRefresh: Int?
    var lastCodec: VideoCodec?
    var lastMbps: Int?
    /// Nombre d'écrans de la dernière session (absent : un).
    var lastScreens: Int?
    /// Un écran au moins de la dernière session était en HDR (absent : non).
    var lastHDR: Bool?
    /// L'ordinateur a proposé la caméra à la dernière session (docs/webcam.md) :
    /// « Définition de la caméra » apparaît dans ses réglages.
    var cameraOffered: Bool?
    /// Par où elle est passée : sur le réseau ou par le relais (ligne « Chemin » de
    /// la « Dernière session » ; absent : une session d'avant le relais).
    var lastRoute: RouteInfo.Transport?
    /// Ajouté par code à six chiffres : un compte propre à cet appareil,
    /// créé par Admin à l'accord.
    var pairedByCode: Bool?
    /// Cartes réseau de l'ordinateur, pour le réveiller par le réseau :
    /// apprises d'Admin à chaque info (gardées si la nouvelle liste est vide).
    var wake: [WakeTarget]?
    /// Le relais de l'ordinateur (`relay` de `info`, docs/relais.md) : appris
    /// d'Admin à chaque info, suivi tel qu'il le dit ; hors de chez soi, la
    /// session et la présence passent par lui (« Hors de chez vous »).
    var relay: RelayInfo?
    /// Le système de l'ordinateur (`os` de `info`, « macOS 26.1 ») : appris
    /// d'Admin à chaque info. Face à une Station Mac, ⌘ part comme ⌘ et les
    /// touches proposées sont celles du Mac (docs/link-ui.md).
    var os: String?
    /// L'ordinateur ne crée pas les manettes de l'appareil (`gamepads` de
    /// `info`, Station Mac sans le droit d'Apple) : appris à chaque info.
    var gamepadsUnavailable: Bool?
    /// Un ordinateur d'un compte KyberNow (docs/comptes.md), pas appairé sur
    /// cet appareil : le compte qui le donne et sa Station chez le relais.
    /// Ni mot de passe ni iCloud : le compte ouvre la console et la session
    /// (Accounts.swift) ; il quitte l'accueil avec le compte.
    var account: AccountLink?

    /// Une Station Mac (règle du cœur, `keymap::station_is_mac`).
    var isMac: Bool { Keymap.stationIsMac(os) }
    /// « ⌘ envoyée comme Ctrl » tel qu'il s'applique : jamais face à un Mac ;
    /// le réglage gardé pour les PC ne change pas.
    var commandAsControl: Bool { Keymap.commandAsControl(settings.commandAsControl, os: os) }
    /// « Manettes indisponibles sur cet ordinateur » (docs/link-ui.md).
    var gamepadsAvailable: Bool { gamepadsUnavailable != true }

    /// La route de l'ordinateur par son relais : celle de sa Station, avec
    /// l'empreinte d'Admin retenue ; nil sans relais, ou sans empreinte (jamais
    /// de premier contact par le relais).
    var relayRoute: RelayRoute? {
        relay.flatMap { Relay.route(for: $0, adminFingerprint: consoleFingerprint) }
    }

    /// La route que prend la session quand le réseau de l'ordinateur ne répond
    /// pas. C'est le relais (et son Admin) qui dit si l'accès hors de chez soi est
    /// coupé, par un refus typé (code 32 : « {nom} n'est pas joignable hors de chez
    /// vous (accès coupé dans Admin). ») : jamais déduit ici de ce que `info` disait.
    var sessionRelay: RelayRoute? { relayRoute }

    init(name: String, address: String, consolePort: UInt16, username: String) {
        self.name = name
        self.address = address
        self.consolePort = consolePort
        self.username = username
    }

    /// Fin d'une session commencée à `start` (l'image arrivée) : une session
    /// de moins de 3 s ne remplace pas la précédente dans les Statistiques.
    mutating func recordSession(start: Date, latency: (median: Double, p95: Double)?, width: Int, height: Int, refresh: Int, codec: VideoCodec, mbps: Int, screens: Int = 1, route: RouteInfo.Transport? = nil, hdr: Bool = false) {
        let duration = Date().timeIntervalSince(start)
        guard duration >= 3 else { return }
        lastSession = start
        lastDuration = duration
        lastLatencyMs = latency?.median
        lastLatencyP95Ms = latency?.p95
        lastWidth = width > 0 ? width : nil
        lastHeight = height > 0 ? height : nil
        lastRefresh = refresh > 0 ? refresh : nil
        lastCodec = codec
        lastMbps = mbps > 0 ? mbps : nil
        lastScreens = screens > 1 ? screens : nil
        lastRoute = route
        lastHDR = hdr ? true : nil
    }

    // Lecture tolérante : les ordinateurs enregistrés par les versions
    // précédentes n'avaient qu'un profil, ni le détail de la dernière session.
    private enum CodingKeys: String, CodingKey {
        case id, name, address, consolePort, username, streamPort, consoleFingerprint, settings, lastSession, lastLatencyMs, lastLatencyP95Ms, lastDuration, profile
        case lastWidth, lastHeight, lastRefresh, lastCodec, lastMbps, lastScreens, lastRoute, lastHDR, pairedByCode, wake, relay, cameraOffered
        case os, gamepadsUnavailable, account
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(UUID.self, forKey: .id)
        name = try c.decode(String.self, forKey: .name)
        address = try c.decode(String.self, forKey: .address)
        consolePort = try c.decode(UInt16.self, forKey: .consolePort)
        username = try c.decode(String.self, forKey: .username)
        streamPort = try c.decodeIfPresent(UInt16.self, forKey: .streamPort)
        consoleFingerprint = try c.decodeIfPresent(String.self, forKey: .consoleFingerprint)
        lastSession = try c.decodeIfPresent(Date.self, forKey: .lastSession)
        lastLatencyMs = try c.decodeIfPresent(Double.self, forKey: .lastLatencyMs)
        lastLatencyP95Ms = try c.decodeIfPresent(Double.self, forKey: .lastLatencyP95Ms)
        lastDuration = try c.decodeIfPresent(TimeInterval.self, forKey: .lastDuration)
        lastWidth = try? c.decodeIfPresent(Int.self, forKey: .lastWidth)
        lastHeight = try? c.decodeIfPresent(Int.self, forKey: .lastHeight)
        lastRefresh = try? c.decodeIfPresent(Int.self, forKey: .lastRefresh)
        lastCodec = try? c.decodeIfPresent(VideoCodec.self, forKey: .lastCodec)
        lastMbps = try? c.decodeIfPresent(Int.self, forKey: .lastMbps)
        lastScreens = try? c.decodeIfPresent(Int.self, forKey: .lastScreens)
        lastRoute = try? c.decodeIfPresent(RouteInfo.Transport.self, forKey: .lastRoute)
        lastHDR = try? c.decodeIfPresent(Bool.self, forKey: .lastHDR)
        cameraOffered = try? c.decodeIfPresent(Bool.self, forKey: .cameraOffered)
        pairedByCode = try? c.decodeIfPresent(Bool.self, forKey: .pairedByCode)
        wake = try? c.decodeIfPresent([WakeTarget].self, forKey: .wake)
        relay = try? c.decodeIfPresent(RelayInfo.self, forKey: .relay)
        os = try? c.decodeIfPresent(String.self, forKey: .os)
        gamepadsUnavailable = try? c.decodeIfPresent(Bool.self, forKey: .gamepadsUnavailable)
        account = try? c.decodeIfPresent(AccountLink.self, forKey: .account)
        if let s = try c.decodeIfPresent(ConnectionSettings.self, forKey: .settings) {
            settings = s
        } else if let p = try c.decodeIfPresent(Profile.self, forKey: .profile) {
            settings.apply(p)
        }
    }

    func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id, forKey: .id)
        try c.encode(name, forKey: .name)
        try c.encode(address, forKey: .address)
        try c.encode(consolePort, forKey: .consolePort)
        try c.encode(username, forKey: .username)
        try c.encodeIfPresent(streamPort, forKey: .streamPort)
        try c.encodeIfPresent(consoleFingerprint, forKey: .consoleFingerprint)
        try c.encode(settings, forKey: .settings)
        try c.encodeIfPresent(lastSession, forKey: .lastSession)
        try c.encodeIfPresent(lastLatencyMs, forKey: .lastLatencyMs)
        try c.encodeIfPresent(lastLatencyP95Ms, forKey: .lastLatencyP95Ms)
        try c.encodeIfPresent(lastDuration, forKey: .lastDuration)
        try c.encodeIfPresent(lastWidth, forKey: .lastWidth)
        try c.encodeIfPresent(lastHeight, forKey: .lastHeight)
        try c.encodeIfPresent(lastRefresh, forKey: .lastRefresh)
        try c.encodeIfPresent(lastCodec, forKey: .lastCodec)
        try c.encodeIfPresent(lastMbps, forKey: .lastMbps)
        try c.encodeIfPresent(lastScreens, forKey: .lastScreens)
        try c.encodeIfPresent(lastRoute, forKey: .lastRoute)
        try c.encodeIfPresent(lastHDR, forKey: .lastHDR)
        try c.encodeIfPresent(cameraOffered, forKey: .cameraOffered)
        try c.encodeIfPresent(pairedByCode, forKey: .pairedByCode)
        try c.encodeIfPresent(wake, forKey: .wake)
        try c.encodeIfPresent(relay, forKey: .relay)
        try c.encodeIfPresent(os, forKey: .os)
        try c.encodeIfPresent(gamepadsUnavailable, forKey: .gamepadsUnavailable)
        try c.encodeIfPresent(account, forKey: .account)
    }
}

/// Ce qui empêche d'entrer dans un ordinateur qui répond pourtant.
enum HostProblem: Equatable {
    /// Admin refuse le compte (mot de passe changé ou compte retiré).
    case passwordRefused
    /// Le certificat d'Admin n'est plus celui retenu (Station réinstallée).
    case certificateChanged
    // Hors de chez soi, l'accueil ne dit que l'état (« Hors de chez vous », « Hors
    // ligne ») : les refus du relais (accès coupé dans Admin, ordinateur éteint ou
    // en veille) se disent à l'ouverture de l'ordinateur, à la place de la
    // connexion, avec son nom (`SessionEngine.Params.computerName`).
}

/// État d'un ordinateur vu depuis cet appareil, rafraîchi en continu.
struct HostStatus: Equatable {
    var online: Bool
    var info: HostInfo?
    var problem: HostProblem? = nil
    /// Joignable seulement par le relais (hors de chez soi) : l'accueil dit
    /// « Hors de chez vous » à la place de « En ligne » (`online` est alors vrai :
    /// on peut se connecter, par le relais).
    var away = false
}

/// Par où une console parle à un ordinateur.
enum ConsoleRoute {
    /// Par le relais si l'ordinateur n'est joignable que par lui (`HostStatus.away`), sinon sur le réseau.
    case auto
    /// Sur le réseau seulement (le sondage de l'accueil).
    case direct
    /// Par le relais (la session passe par lui).
    case relay
}

@MainActor
final class HostStore: ObservableObject {
    @Published private(set) var hosts: [Host] = []
    @Published private(set) var status: [UUID: HostStatus] = [:]
    /// Redémarrage ou mise à jour de la Station en cours, par ordinateur
    /// (rubrique « Station », LinkUI.swift).
    @Published var stationTasks: [UUID: StationTask] = [:]
    /// Ordinateurs dont la session passe par le relais (la session le dit avec son
    /// chemin, `setSessionViaRelay`) : ni redémarrage ni mise à jour de la Station
    /// depuis l'app, Admin les refuse par le relais (`viaRelay`).
    @Published var relaySessions: Set<UUID> = []
    /// Un appareil est en session : « Redémarrer quand même » à confirmer.
    @Published var stationConfirm: StationConfirm?
    /// Mise à jour des Stations proposée par l'app (LinkUI.swift) : « Plus
    /// tard » et la dernière vérification, par ordinateur, gardés dans les
    /// préférences de l'app (`stationOffers`).
    @Published var stationMemory: [UUID: StationOffer.Memory] = [:]
    /// Mises à jour lancées depuis le bandeau de l'accueil : le bandeau en
    /// montre la suite (pendant, en place, échec).
    @Published var stationBannerFollow: Set<UUID> = []
    /// L'avis après une mise à jour de l'app (« {nom} a aussi une mise à jour »).
    @Published var stationNotice: StationNotice?
    /// Une session de cet appareil est ouverte (l'app le dit) : ni bandeau, ni
    /// vérification, ni avis ; l'avis attend le retour à l'accueil.
    @Published var sessionOpen = false {
        didSet { if oldValue, !sessionOpen { showPendingStationNotice() } }
    }
    /// Vérifications des Stations en cours (`stationCheck`).
    var stationChecking: Set<UUID> = []
    /// L'avis est prêt mais une session est ouverte.
    var pendingStationNotice: StationNotice?
    /// Réveil par le réseau en cours (« Réveil de l'ordinateur… »), par ordinateur.
    @Published private(set) var waking: Set<UUID> = []
    /// Prêter l'ordinateur : la vue du prêt de chaque ordinateur dont la
    /// Station le connaît (« Prêté à {prénom} » sur l'accueil), relue avec le
    /// sondage ; absente sans prêt possible.
    @Published private(set) var lends: [UUID: LendView] = [:]
    /// Image de chaque ordinateur sur l'accueil.
    let imagery = HostImagery()
    private let key = "hosts"
    private var polling: Task<Void, Never>?
    /// Intervalle du sondage des ordinateurs : 8 s ; l'app Mac le passe à 30 s
    /// quand sa fenêtre est fermée (elle ne vit plus que dans la barre des menus).
    var pollInterval: TimeInterval = 8
    /// Fin du dernier tour de sondage.
    private(set) var lastPoll = Date.distantPast
    /// Ordinateurs de démonstration (`-demo YES`, en Debug) : leurs propres
    /// préférences, mots de passe en mémoire, états posés par l'app, aucun
    /// appel réseau.
    let demo: Bool
    let defaults: UserDefaults
    private var demoPasswords: [UUID: String] = [:]
    /// Appelé après chaque ajout, modification ou retrait : l'app Mac publie
    /// ses ordinateurs dans iCloud pour l'Apple TV (CloudPublisher).
    var didSave: (([Host]) -> Void)?
    /// Présence des ordinateurs chez leur relais (accueil : « Hors de chez
    /// vous »), par relais : une connexion brève à chaque interrogation, donc
    /// une fois toutes les 30 s au plus, pour tous les ordinateurs d'un relais.
    private var presenceResults: [String: (at: Date, present: [String: Bool])] = [:]
    private var presenceTasks: [String: Task<[String: Bool]?, Never>] = [:]
    private static let presenceInterval: TimeInterval = 30
    /// Les consoles PlayStation que voit la Station d'un ordinateur sur le
    /// réseau (`GET /api/consoles`, docs/relais.md) : données aux consoles
    /// (PlayStationStore), qui retiennent par quelle Station passer hors de
    /// chez soi. Demandé une fois par minute au plus par ordinateur.
    var onConsoles: ((UUID, [PlayStation.StationConsole]) -> Void)?
    private var consolesAsked: [UUID: Date] = [:]
    /// Les comptes KyberNow de l'appareil (Accounts.swift) : la console et la
    /// session de leurs ordinateurs, leur état hors du réseau.
    weak var accounts: AccountStore?

    init(demo: Bool = false) {
        self.demo = demo
        defaults = demo ? (UserDefaults(suiteName: "fr.leslignesclaires.kybernow.demo") ?? .standard) : .standard
        if let data = defaults.data(forKey: key),
           let saved = try? JSONDecoder().decode([Host].self, from: data) {
            hosts = saved
        }
        stationMemory = Self.loadStationMemory(defaults)
        if !demo { startPolling() }
    }

    /// Le plus récemment utilisé d'abord.
    var ordered: [Host] {
        hosts.sorted { ($0.lastSession ?? .distantPast) > ($1.lastSession ?? .distantPast) }
    }

    private func save() {
        if let data = try? JSONEncoder().encode(hosts) {
            defaults.set(data, forKey: key)
        }
        // Les ordinateurs d'un compte restent sur cet appareil : ni iCloud, ni Apple TV par le Mac.
        didSave?(hosts.filter { $0.account == nil })
    }

    /// Les ordinateurs d'un compte à l'accueil (`AccountStore.sync`) : ajoutés,
    /// mis à jour ou retirés d'un coup, réglages et statistiques gardés.
    func setAccountHosts(_ list: [Host]) {
        let ids = Set(list.map(\.id))
        let gone = hosts.filter { $0.account != nil && !ids.contains($0.id) }
        let added = list.filter { h in !hosts.contains { $0.id == h.id } }
        // L'ordre de la liste gardé : chacun à sa place, les nouveaux à la fin.
        let next = hosts.compactMap { h -> Host? in
            guard h.account != nil else { return h }
            return list.first { $0.id == h.id }
        } + added
        guard next != hosts else { return }
        for h in gone {
            imagery.forget(h)
            status[h.id] = nil
            stationTasks[h.id] = nil
            lends[h.id] = nil
        }
        hosts = next
        save()
        for h in added { Task { await refresh(h) } }
    }

    func add(_ host: Host, password: String) {
        hosts.append(host)
        setPassword(password, for: host.id)
        save()
        Task { await refresh(host) }
    }

    /// Un ordinateur ajouté par une invitation (docs/invitations.md) : gardé
    /// comme un ordinateur appairé par code (compte propre à cet appareil au
    /// trousseau, empreinte d'Admin épinglée, relais pour l'ouvrir hors de chez
    /// soi). Le même ordinateur déjà là (même empreinte d'Admin) n'est pas
    /// ajouté deux fois : il prend le nouvel accès (compte, adresse, relais),
    /// en gardant ses réglages et ses statistiques. `computer` : le nom donné
    /// par « Rejoindre », quand la Station ne dit pas le sien. Rend l'ordinateur,
    /// à sélectionner sur l'accueil.
    @discardableResult
    func addInvited(_ invited: Invited, computer: String) -> Host {
        let name = !invited.hostname.isEmpty ? invited.hostname : (computer.isEmpty ? invited.address : computer)
        if let fingerprint = invited.fingerprint,
           var host = hosts.first(where: { $0.consoleFingerprint.map { Self.sameFingerprint($0, fingerprint) } ?? false }) {
            host.username = invited.username
            if !invited.address.isEmpty {
                host.address = invited.address
                if invited.port != 0 { host.consolePort = invited.port }
            }
            if let relay = invited.relay { host.relay = relay }
            host.pairedByCode = true
            update(host)
            setPassword(invited.password, for: host.id)
            Task { await refresh(host) }
            return host
        }
        // Sans port dit par Admin : celui d'Admin par défaut.
        var host = Host(name: name, address: invited.address, consolePort: invited.port != 0 ? invited.port : 9443, username: invited.username)
        host.consoleFingerprint = invited.fingerprint
        host.relay = invited.relay
        // Compte créé par Admin pour cet appareil, comme à l'accord d'un appairage par code.
        host.pairedByCode = true
        add(host, password: invited.password)
        return host
    }

    /// Deux empreintes d'Admin sont la même : sans `sha256:`, sans deux-points, sans casse.
    static func sameFingerprint(_ a: String, _ b: String) -> Bool {
        func normal(_ s: String) -> String {
            var t = s.lowercased()
            if t.hasPrefix("sha256:") { t.removeFirst(7) }
            return t.replacingOccurrences(of: ":", with: "").trimmingCharacters(in: .whitespaces)
        }
        return !normal(a).isEmpty && normal(a) == normal(b)
    }

    func update(_ host: Host) {
        guard let i = hosts.firstIndex(where: { $0.id == host.id }) else { return }
        let old = hosts[i]
        hosts[i] = host
        save()
        // Réglages synchronisés d'un ordinateur du compte (Accounts.swift) : les clés changées partent au relais.
        if old.settings != host.settings { accounts?.settingsChanged(from: old, to: host) }
    }

    func host(_ id: UUID) -> Host? {
        hosts.first { $0.id == id }
    }

    func remove(_ host: Host) {
        imagery.forget(host)
        hosts.removeAll { $0.id == host.id }
        status[host.id] = nil
        stationTasks[host.id] = nil
        lends[host.id] = nil
        if demo { demoPasswords[host.id] = nil } else { Keychain.delete(host.id) }
        save()
    }

    func password(for host: Host) -> String? {
        // Un ordinateur d'un compte n'a pas de compte Kyber : le jeton d'accès suffit.
        if host.account != nil { return "" }
        return demo ? demoPasswords[host.id] : Keychain.get(host.id)
    }

    private func setPassword(_ password: String, for id: UUID) {
        if demo { demoPasswords[id] = password } else { Keychain.set(password, for: id) }
    }

    /// Nouveau mot de passe (rubrique Station, « Mot de passe refusé ») :
    /// gardé, puis vérifié tout de suite auprès d'Admin.
    func updatePassword(_ password: String, for host: Host) {
        setPassword(password, for: host.id)
        Task { await refresh(host) }
    }

    /// Nouvelle empreinte acceptée (Station réinstallée, empreinte comparée
    /// à celle d'Admin) : retenue à la place de l'ancienne.
    func trust(fingerprint: String, for host: Host) {
        guard var h = self.host(host.id) else { return }
        h.consoleFingerprint = fingerprint
        update(h)
        Task { await refresh(h) }
    }

    /// Empreinte que présente Admin en ce moment, sans la comparer à celle retenue.
    func presentedFingerprint(of host: Host) async -> String? {
        #if DEBUG
        // Démonstration : une Station « réinstallée », pour l'écran de l'empreinte.
        if demo { return "sha256:c41be0f2d79a3e58b6014c7dd2e95a1f03b8c6e4927d5a0f1e3b6c8d9a2f4e71" }
        #endif
        guard !demo else { return nil }
        let probe = ConsoleClient(host: host.address, port: host.consolePort, username: host.username, password: password(for: host) ?? "", fingerprint: nil)
        return await probe.probeFingerprint()
    }

    // MARK: - En ligne ou pas

    /// Un tour de sondage toutes les `pollInterval` secondes ; l'attente se
    /// compte par seconde, pour qu'un intervalle raccourci s'applique tout de suite.
    private func startPolling() {
        polling = Task { [weak self] in
            while !Task.isCancelled {
                guard let self else { return }
                await self.refreshAll()
                while !Task.isCancelled, Date().timeIntervalSince(self.lastPoll) < self.pollInterval {
                    try? await Task.sleep(for: .seconds(1))
                }
            }
        }
    }

    /// Tous les ordinateurs sondés en même temps (aussi à l'ouverture du menu
    /// de la barre des menus, sur Mac).
    func refreshAll() async {
        await withTaskGroup(of: Void.self) { group in
            for host in hosts {
                group.addTask { await self.refresh(host) }
            }
        }
        lastPoll = Date()
    }

    /// La console de l'ordinateur : sur le réseau, ou par son relais quand il
    /// n'est joignable que par lui (`route`).
    func console(for host: Host, route: ConsoleRoute = .auto) -> ConsoleClient? {
        let current = self.host(host.id) ?? host
        // Un ordinateur d'un compte : la console du compte (réseau ou relais).
        if current.account != nil {
            guard !demo else { return nil }
            let viaRelay = route == .relay || (route == .auto && status[host.id]?.away == true)
            return accounts?.console(for: current, viaRelay: viaRelay)
        }
        guard !demo, let password = password(for: host) else { return nil }
        let viaRelay = route == .relay || (route == .auto && status[host.id]?.away == true)
        return ConsoleClient(host: current.address, port: current.consolePort, username: current.username, password: password,
                             fingerprint: current.consoleFingerprint, relay: viaRelay ? current.relayRoute : nil)
    }

    /// « Réessayer » : le sondage tout de suite, y compris chez le relais (le
    /// cache de présence est passé).
    func retry(_ host: Host) async {
        await refresh(host, explicit: true)
    }

    func refresh(_ host: Host, explicit: Bool = false) async {
        // Un ordinateur d'un compte pas vu sur le réseau : son état chez le relais.
        if host.account != nil, let accounts, !demo, !accounts.isOnNetwork(host) {
            let s = await accounts.awayStatus(host, previous: status[host.id])
            if self.host(host.id) != nil { status[host.id] = s }
            return
        }
        guard let console = console(for: host, route: .direct) else { return }
        do {
            let info = try await console.info()
            status[host.id] = HostStatus(online: true, info: info)
            // Mise à jour des Stations : demander à la Station de vérifier, quand c'est le moment (LinkUI.swift).
            stationCheckIfDue(host)
            // Le prêt de l'ordinateur (Station 0.9.0 et suivantes) : « Prêté à {prénom} » sur l'accueil.
            if info.lend?.available == true {
                if let view = try? await console.lendState() { setLend(view, for: host.id) }
            } else {
                setLend(nil, for: host.id)
            }
            // Le relais de l'ordinateur, tel que sa Station le dit (absent : plus de relais).
            if var h = self.host(host.id), h.relay != info.relay {
                h.relay = info.relay
                update(h)
            }
            // Le système et les manettes de l'ordinateur, gardés pour lui (une
            // Station sans `os` garde le précédent).
            let unavailable: Bool? = info.gamepadsAvailable ? nil : true
            if var h = self.host(host.id), (info.os != nil && h.os != info.os) || h.gamepadsUnavailable != unavailable {
                h.os = info.os ?? h.os
                h.gamepadsUnavailable = unavailable
                update(h)
            }
            // Les cartes réseau pour le réveil : une liste vide (Station
            // plus ancienne, carte absente) garde la précédente.
            if !info.wake.isEmpty, var h = self.host(host.id), h.wake != info.wake {
                h.wake = info.wake
                update(h)
            }
            // Jamais ouvert depuis cet appareil (ou image non gardée) : un aperçu à jour.
            if imagery.wantsPreview(host) { await imagery.fetchPreview(for: host, console: console) }
            // Les consoles que voit sa Station (une Station d'avant les consoles répond 404).
            if let onConsoles, Date().timeIntervalSince(consolesAsked[host.id] ?? .distantPast) > 60 {
                consolesAsked[host.id] = Date()
                if let list = try? await console.consoles(), !list.isEmpty { onConsoles(host.id, list) }
            }
            if host.consoleFingerprint == nil, let fp = console.fingerprint, var h = self.host(host.id) {
                h.consoleFingerprint = fp
                update(h)
            }
        } catch ConsoleError.http(404, _) {
            // Console d'une version précédente, sans /api/stream/info : elle répond, donc en ligne.
            status[host.id] = HostStatus(online: true, info: nil)
        } catch ConsoleError.http(401, _) {
            // Elle répond, mais refuse le compte.
            status[host.id] = HostStatus(online: true, info: status[host.id]?.info, problem: .passwordRefused)
        } catch ConsoleError.certificateChanged {
            status[host.id] = HostStatus(online: true, info: status[host.id]?.info, problem: .certificateChanged)
        } catch let ConsoleError.account(failure, message) {
            // Un ordinateur d'un compte : session de l'appareil fermée, ou
            // ordinateur qui n'est plus au compte (le compte le retire de l'accueil).
            accounts?.refused(host, failure: failure, message: message)
            if self.host(host.id) != nil { status[host.id] = HostStatus(online: false, info: status[host.id]?.info) }
        } catch {
            status[host.id] = await unreachableOnNetwork(host, explicit: explicit)
        }
    }

    // MARK: - Hors de chez soi (docs/relais.md)

    /// Le réseau ne répond pas : l'ordinateur est peut-être joignable par son
    /// relais. Présent chez lui : « Hors de chez vous » (on peut se connecter) ;
    /// sinon « Hors ligne ». L'accueil ne dit que cet état : les refus du relais
    /// (accès coupé dans Admin, éteint ou en veille) se disent à l'ouverture de
    /// l'ordinateur, avec son nom, d'après le code du cœur (jamais le texte d'Admin).
    /// `explicit` (« Réessayer ») passe le cache de présence.
    private func unreachableOnNetwork(_ host: Host, explicit: Bool) async -> HostStatus {
        let info = status[host.id]?.info
        let offline = HostStatus(online: false, info: info)
        // Un ordinateur d'un compte : ce que le relais en dit (liste du compte).
        if host.account != nil, let accounts, !demo { return await accounts.awayStatus(host, previous: status[host.id]) }
        guard !demo, Relay.mode != .off, let known = self.host(host.id), let relay = known.relay else { return offline }
        // Jamais de premier contact par le relais : il faut l'empreinte d'Admin.
        guard known.relayRoute != nil, let present = await presence(of: relay, force: explicit) else { return offline }
        return present ? HostStatus(online: true, info: info, away: true) : offline
    }

    /// L'ordinateur est-il chez son relais ? nil si le relais ne répond pas.
    /// Tous les ordinateurs d'un même relais (le même si les formes normales de
    /// leurs adresses sont égales) sont demandés d'un coup, une fois toutes les
    /// 30 s (tout de suite avec `force`).
    private func presence(of relay: RelayInfo, force: Bool) async -> Bool? {
        let server = Relay.normalize(relay.server)
        if !force, let r = presenceResults[server], Date().timeIntervalSince(r.at) < Self.presenceInterval, let known = r.present[relay.id] {
            return known
        }
        if let running = presenceTasks[server] { return await running.value?[relay.id] }
        let ids = Array(Set(hosts.compactMap { $0.relay.flatMap { Relay.normalize($0.server) == server ? $0.id : nil } }).prefix(16))
        let task = Task { () -> [String: Bool]? in
            guard let list = try? await Relay.presence(server: server, stations: ids), list.count == ids.count else { return nil }
            return Dictionary(uniqueKeysWithValues: zip(ids, list))
        }
        presenceTasks[server] = task
        let result = await task.value
        presenceTasks[server] = nil
        if let result { presenceResults[server] = (Date(), result) }
        return result?[relay.id]
    }

    // MARK: - Réveil par le réseau

    /// L'app connaît une carte réseau de l'ordinateur : « Réveiller l'ordinateur ».
    func canWake(_ host: Host) -> Bool {
        !(self.host(host.id)?.wake ?? host.wake ?? []).isEmpty
    }

    /// « Réveiller l'ordinateur » : le paquet part, puis l'ordinateur est
    /// sondé toutes les 2 s, 2 min au plus, jusqu'à ce qu'il soit en ligne.
    func wake(_ host: Host) {
        guard !waking.contains(host.id), let targets = self.host(host.id)?.wake, !targets.isEmpty else { return }
        waking.insert(host.id)
        Task {
            if !demo {
                // Un envoi parti suffit (la diffusion peut être refusée sur iPhone, iPad, Apple TV).
                _ = await Wake.send(targets)
            }
            let start = Date()
            while Date().timeIntervalSince(start) < 120, self.host(host.id) != nil {
                try? await Task.sleep(for: .seconds(2))
                #if DEBUG
                if demo, Date().timeIntervalSince(start) > 8, let info = status[host.id]?.info {
                    setDemoStatus(HostStatus(online: true, info: info), for: host.id)
                }
                #endif
                await refresh(host)
                if status[host.id]?.online == true { break }
            }
            waking.remove(host.id)
        }
    }

    /// La vue du prêt de cet ordinateur (sondage, réponse d'Admin, signe de vie de sa session).
    /// Ce que la Station annonce après `stationCheck` : remplace `station` de
    /// son dernier état (le sondage suivant le relira).
    func setStation(_ station: StationInfo, for id: UUID) {
        guard var s = status[id], var info = s.info else { return }
        info.station = station
        s.info = info
        if status[id] != s { status[id] = s }
    }

    func setLend(_ view: LendView?, for id: UUID) {
        if lends[id] != view { lends[id] = view }
    }

    #if DEBUG
    /// Démonstration : l'état d'un ordinateur, posé par l'app.
    func setDemoStatus(_ s: HostStatus?, for id: UUID) {
        status[id] = s
    }

    /// Démonstration : la vue du prêt d'un ordinateur.
    func setDemoLend(_ view: LendView?, for id: UUID) {
        lends[id] = view
    }
    #endif
}

enum Keychain {
    private static let service = "fr.leslignesclaires.kybernow"

    private static func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
    }

    static func set(_ password: String, for id: UUID) { set(password, account: id.uuidString) }
    static func get(_ id: UUID) -> String? { get(account: id.uuidString) }
    static func delete(_ id: UUID) { delete(account: id.uuidString) }

    /// Secret rangé sous un nom (consoles PlayStation : association, compte PSN).
    static func set(_ password: String, account: String) {
        SecItemDelete(query(account) as CFDictionary)
        var item = query(account)
        item[kSecValueData as String] = Data(password.utf8)
        item[kSecAttrLabel as String] = "KyberNow"
        SecItemAdd(item as CFDictionary, nil)
    }

    static func get(account: String) -> String? {
        var q = query(account)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(account: String) {
        SecItemDelete(query(account) as CFDictionary)
    }
}
