package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import fr.leslignesclaires.kybernow.core.HostRelay
import fr.leslignesclaires.kybernow.core.WakeTarget
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.KeyStore
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Profils, dans le vocabulaire du cœur. [reliable] : le transport du profil, comme
 * `StreamSettings::for_profile` du cœur (Bureau fiable, Jeu et Max rapides), pour
 * l'afficher sans la bibliothèque native. [AUTO] : les réglages choisis par le test des
 * conditions (docs/profil-auto.md), gardés avec l'ordinateur ([HostSettings.autoResult]) ;
 * son transport est celui du test ([HostSettings.profileReliable]).
 */
enum class Profile(val id: String, val reliable: Boolean) {
    GAME("jeu", false),
    DESKTOP("bureau", true),
    MAX("max", false),
    AUTO("auto", false),
}

/**
 * Réglages d'un ordinateur (comme Réglages › par ordinateur sur les autres apps). Le profil
 * règle le format, le débit et le transport ; [codec], [mbps] et [transport] s'en écartent
 * (« Personnalisé », [custom]) jusqu'au choix d'un profil, qui les remet à zéro ([apply]).
 */
data class HostSettings(
    val profile: Profile = Profile.GAME,
    /** 0 : comme cet écran ; sinon hauteur (2160, 1440, 1080, 720). */
    val resolution: Int = 0,
    /** 0 : la plus haute que l'écran tient à cette définition. */
    val refresh: Int = 0,
    /** Écran virtuel à la taille de l'appareil (si la Station en crée) ; sinon, ses écrans tels quels. */
    val virtualDisplay: Boolean = true,
    /** `auto`, `hevc`, `h264`, `av1`. */
    val codec: String = "auto",
    /** 0 : celui du profil (Mbit/s, calé sur la 4K à 60 Hz). */
    val mbps: Int = 0,
    /** `auto`, `stereo`, `5.1`, `7.1`, `computer` (le son reste sur l'ordinateur). */
    val audio: String = "auto",
    /** Toutes les manettes vues comme des Xbox 360 par l'ordinateur ; sinon fidèles. */
    val xbox360: Boolean = true,
    /** L1 + R1 + Start + Select ouvrent le menu. */
    val combo: Boolean = true,
    /** Bouton Retour de la télécommande : ouvre le menu (sinon il part à l'ordinateur comme Échap). */
    val backOpensMenu: Boolean = true,
    /** Bouton Guide : `game` (part au jeu), `long` (Start tenu 600 ms), `system`. */
    val guide: String = "game",
    /** Caler l'écran (HDMI) sur la définition et la fréquence de la session. */
    val matchDisplay: Boolean = true,
    /** Mode jeu du téléviseur (ALLM) pendant la session. */
    val gameMode: Boolean = true,
    /** Souris : 0 Auto, 1 Bureau, 2 Jeu. */
    val mouse: Int = 0,
    /** Au doigt : pavé tactile ou toucher direct. */
    val directTouch: Boolean = false,
    val clipboard: Boolean = true,
    /** « Définition de la caméra » (docs/webcam.md) : 1080p, sinon 720p ; hors profil. */
    val camera1080: Boolean = false,
    /** L'ordinateur a déjà proposé la caméra : « Définition de la caméra » apparaît dans ses réglages. */
    val cameraSeen: Boolean = false,
    /** « Ctrl agit comme ⌘ », face à une Station Mac seulement ; nul : jamais changé, allumé (`keymap::control_as_command` du cœur). */
    val ctrlCommand: Boolean? = null,
    val reconnect: Boolean = true,
    /** Transport de l'image : `auto` (celui du profil), `fast`, `reliable`. */
    val transport: String = "auto",
    /** Manette à l'écran (téléphone, tablette) : affichée ou non, retenu à chaque bascule de la barre. */
    val onScreenPad: Boolean = false,
    /** Sa disposition : `full` (manette Xbox entière) ou `simple` (stick gauche, croix, A, B, X, Y, Menu). */
    val padLayout: String = "full",
    /** Sa taille : 0 petite, 1 moyenne, 2 grande. */
    val padSize: Int = 1,
    /** Son opacité, en % ([PAD_OPACITIES]). */
    val padOpacity: Int = 70,
    /** Son retour haptique : 0 désactivé, 1 faible, 2 moyen, 3 fort. */
    val padHaptics: Int = 2,
    /**
     * Le choix « HDR » de cet ordinateur (docs/hdr.md) : allumé la première fois (le HDR est automatique), gardé
     * ensuite. À part du profil : il ne fait jamais passer en « Personnalisé ». Ne sert que sur un écran HDR
     * ([Hdr.state]).
     */
    val hdr: Boolean = true,
    /**
     * « Débit adaptatif » (docs/profil-auto.md) : le débit baisse quand le réseau sature, puis remonte ; allumé par
     * défaut, pour un profil comme pour un débit choisi. À part du profil : il ne fait jamais passer en « Personnalisé ».
     */
    val adaptive: Boolean = true,
    /** Le dernier test du profil Auto appliqué pour cet ordinateur (`AutoResult` du cœur, JSON) ; nul : jamais fait. */
    val autoResult: String? = null,
    /**
     * Le plafond de l'encodeur de cet ordinateur (Mbit/s par écran ; docs/profil-auto.md, « L'encodeur de
     * l'ordinateur ») : trouvé par le test d'Auto (« Appliquer ») ou par une session dont l'image a été refusée
     * (`Notice::EncoderCeiling`) ; nul : aucun. La session ne demande jamais plus en HEVC, AV1 ou H.264. Hors du profil
     * (jamais « Personnalisé ») et propre à cet appareil : jamais synchronisé.
     */
    val encoderMax: Int? = null,
    /**
     * « Micro » (docs/micro.md) : le micro de l'appareil envoyé à cet ordinateur ; coupé la première fois, puis le
     * dernier choix (barre de session, Réglages › Son). Hors du profil, propre à cet appareil : jamais synchronisé.
     */
    val microphone: Boolean = false,
) {
    val padSimple get() = padLayout == "simple"

    /** [ctrlCommand] pour le cœur ([fr.leslignesclaires.kybernow.core.Native.controlAsCommand]) : −1 jamais changé, 0, 1. */
    val ctrlCommandCode get() = when (ctrlCommand) { null -> -1; true -> 1; false -> 0 }

    /** Les réglages s'écartent du profil : « Personnalisé ». */
    val custom get() = codec != "auto" || mbps != 0 || transport != "auto"

    /** Le transport du profil : celui du test pour Auto, sinon celui du profil. */
    val profileReliable: Boolean get() = if (profile == Profile.AUTO) AutoProfile.reliable(autoResult) ?: profile.reliable else profile.reliable

    /** Le transport en vigueur : choisi, sinon celui du profil. */
    val reliable: Boolean get() = when (transport) {
        "reliable" -> true
        "fast" -> false
        else -> profileReliable
    }

    /** Choisir un profil remet le format, le débit et le transport à ceux du profil. */
    fun apply(p: Profile) = copy(profile = p, codec = "auto", mbps = 0, transport = "auto")

    /** Choisir le transport : celui du profil n'est pas un écart. */
    fun withTransport(reliable: Boolean) = copy(transport = if (reliable == profileReliable) "auto" else if (reliable) "reliable" else "fast")

    fun json(): JSONObject = JSONObject()
        .put("profile", profile.id).put("resolution", resolution).put("refresh", refresh).put("virtualDisplay", virtualDisplay)
        .put("codec", codec).put("mbps", mbps).put("audio", audio).put("xbox360", xbox360).put("combo", combo)
        .put("backOpensMenu", backOpensMenu).put("guide", guide).put("matchDisplay", matchDisplay)
        .put("gameMode", gameMode).put("mouse", mouse).put("directTouch", directTouch)
        .put("clipboard", clipboard).put("camera1080", camera1080).put("cameraSeen", cameraSeen).put("ctrlCommand", ctrlCommand ?: JSONObject.NULL).put("reconnect", reconnect).put("transport", transport)
        .put("onScreenPad", onScreenPad).put("padLayout", padLayout).put("padSize", padSize)
        .put("padOpacity", padOpacity).put("padHaptics", padHaptics).put("hdr", hdr)
        .put("adaptive", adaptive).put("autoResult", autoResult ?: JSONObject.NULL)
        .put("encoderMax", encoderMax ?: JSONObject.NULL).put("microphone", microphone)

    companion object {
        /** Les opacités proposées pour la manette à l'écran. */
        val PAD_OPACITIES = listOf(30, 50, 70, 100)

        fun parse(o: JSONObject?): HostSettings {
            if (o == null) return HostSettings()
            val d = HostSettings()
            return HostSettings(
                profile = Profile.entries.firstOrNull { it.id == o.optString("profile") } ?: d.profile,
                resolution = o.optInt("resolution", d.resolution),
                refresh = o.optInt("refresh", d.refresh),
                virtualDisplay = o.optBoolean("virtualDisplay", d.virtualDisplay),
                codec = o.optString("codec", d.codec),
                mbps = o.optInt("mbps", d.mbps),
                audio = o.optString("audio", d.audio),
                xbox360 = o.optBoolean("xbox360", d.xbox360),
                combo = o.optBoolean("combo", d.combo),
                backOpensMenu = o.optBoolean("backOpensMenu", d.backOpensMenu),
                guide = o.optString("guide", d.guide),
                matchDisplay = o.optBoolean("matchDisplay", d.matchDisplay),
                gameMode = o.optBoolean("gameMode", d.gameMode),
                mouse = o.optInt("mouse", d.mouse),
                directTouch = o.optBoolean("directTouch", d.directTouch),
                clipboard = o.optBoolean("clipboard", d.clipboard),
                camera1080 = o.optBoolean("camera1080", d.camera1080),
                cameraSeen = o.optBoolean("cameraSeen", d.cameraSeen),
                ctrlCommand = if (o.has("ctrlCommand") && !o.isNull("ctrlCommand")) o.optBoolean("ctrlCommand") else d.ctrlCommand,
                reconnect = o.optBoolean("reconnect", d.reconnect),
                // Jusqu'à la 0.4.x : `reliable`, le transport fiable forcé.
                transport = o.optString("transport", "").ifEmpty { if (o.optBoolean("reliable")) "reliable" else d.transport },
                // Absents des fichiers d'avant la manette à l'écran : ses valeurs par défaut.
                onScreenPad = o.optBoolean("onScreenPad", d.onScreenPad),
                padLayout = o.optString("padLayout", d.padLayout).takeIf { it == "full" || it == "simple" } ?: d.padLayout,
                padSize = o.optInt("padSize", d.padSize).coerceIn(0, 2),
                padOpacity = o.optInt("padOpacity", d.padOpacity).takeIf { it in PAD_OPACITIES } ?: d.padOpacity,
                padHaptics = o.optInt("padHaptics", d.padHaptics).coerceIn(0, 3),
                hdr = o.optBoolean("hdr", d.hdr),
                adaptive = o.optBoolean("adaptive", d.adaptive),
                autoResult = if (o.has("autoResult") && !o.isNull("autoResult")) o.optString("autoResult").takeIf { it.isNotEmpty() } else null,
                encoderMax = if (o.isNull("encoderMax")) null else o.optInt("encoderMax", 0).takeIf { it > 0 },
                microphone = o.optBoolean("microphone", d.microphone),
            ).let { if (it.transport == "reliable" && it.profileReliable) it.copy(transport = "auto") else it }
        }
    }
}

/**
 * La dernière session (d'au moins 3 s) : latence médiane et 95e centile (ms), durée (s), quand
 * (ms depuis 1970), et ce qui a été transmis : définition, fréquence, format (`hevc`, `h264`,
 * `av1`), débit (Mbit/s). Les quatre derniers manquent dans ce qu'a écrit la 0.4.x.
 */
data class LastSession(
    val median: Double,
    val p95: Double,
    val seconds: Long,
    val at: Long,
    val width: Int = 0,
    val height: Int = 0,
    val refresh: Int = 0,
    val codec: String? = null,
    val mbps: Int = 0,
    /** Le chemin de la session (`route` du cœur : `direct`, `relay`, `relay_tcp`) ; nul avant le relais (0.7.x). */
    val route: String? = null,
    /** L'image a été en HDR (« HDR » après le format) ; absent : non. */
    val hdr: Boolean = false,
) {
    fun json(): JSONObject = JSONObject().put("median", median).put("p95", p95).put("seconds", seconds).put("at", at)
        .put("width", width).put("height", height).put("refresh", refresh).put("codec", codec ?: JSONObject.NULL).put("mbps", mbps)
        .put("route", route ?: JSONObject.NULL).put("hdr", hdr)

    companion object {
        fun parse(o: JSONObject) = LastSession(
            o.optDouble("median"), o.optDouble("p95"), o.optLong("seconds"), o.optLong("at"),
            width = o.optInt("width"), height = o.optInt("height"), refresh = o.optInt("refresh"),
            codec = if (o.isNull("codec")) null else o.optString("codec").takeIf { it.isNotEmpty() },
            mbps = o.optInt("mbps"),
            route = if (o.isNull("route")) null else o.optString("route").takeIf { it.isNotEmpty() },
            hdr = o.optBoolean("hdr"),
        )
    }
}

/** Un ordinateur équipé de KyberNow Station, vu de l'appareil. */
data class Host(
    val id: String = UUID.randomUUID().toString(),
    val name: String,
    /** Adresse et port d'Admin. */
    val address: String,
    val port: Int,
    /** Empreinte du certificat d'Admin (`sha256:…`), épinglée. */
    val fingerprint: String?,
    val user: String,
    val os: String? = null,
    val gpu: String? = null,
    val av1: Boolean = false,
    val virtualDisplay: Boolean = true,
    /** Adresse MAC saisie à la main pour le réveil (réglage avancé : Station d'avant la 0.5.0). */
    val mac: String? = null,
    /** Les cartes réseau de l'ordinateur pour le réveil, apprises d'Admin (`wake` de `info`), gardées hors ligne. */
    val wake: List<WakeTarget> = emptyList(),
    val settings: HostSettings = HostSettings(),
    val last: LastSession? = null,
    /** Ajouté par un code accepté dans Admin : un compte propre à cet appareil. */
    val paired: Boolean = false,
    /** Le relais de l'ordinateur, appris d'Admin (`relay` de `info`) et gardé hors ligne : l'accès hors de chez soi. */
    val relay: HostRelay? = null,
    /**
     * L'ordinateur crée les manettes de l'appareil (`gamepads` de `info`, `Info::gamepads_available` du cœur) ; faux pour
     * une Station Mac tant qu'Apple n'a pas accordé son droit : « Manettes indisponibles sur cet ordinateur ».
     */
    val gamepads: Boolean = true,
    /**
     * PyroWave annoncé par la Station (`pyrowave.available` de `info`), gardé avec l'ordinateur : nul avec une Station
     * d'avant PyroWave (la valeur n'apparaît pas dans « Format vidéo »), faux quand elle ne le propose pas.
     */
    val pyrowave: Boolean? = null,
    /**
     * Un compte KyberNow donne cet ordinateur (docs/comptes.md) : la clé du compte et la Station chez le relais. Sans
     * identifiant ([user] vide), l'ordinateur vient du compte seulement ([fromAccount]) : ni mot de passe ni appairage,
     * un jeton d'accès du relais à chaque appel ; avec, c'est un ordinateur appairé qui est aussi sur le compte (sa
     * pastille est celle du compte).
     */
    val account: HostAccount? = null,
) {
    /** L'ordinateur n'est à l'accueil que par un compte : il en part avec le compte (« Se déconnecter », code 70, 71). */
    val fromAccount: Boolean get() = account != null && user.isEmpty()

    /**
     * La route par le relais (`{server, station, admin_fp}`) pour une session « Auto » ou une console hors de chez
     * soi ; nulle sans relais connu ou sans empreinte d'Admin épinglée (par le relais, jamais de premier contact).
     */
    val relayRoute: String? get() = relay?.route(fingerprint)

    /**
     * Où envoyer le paquet de réveil : les cartes apprises d'Admin, et l'adresse MAC saisie à la
     * main si elle n'y est pas (vers la dernière adresse IPv4 connue d'Admin).
     */
    val wakeTargets: List<WakeTarget>
        get() {
            val manual = mac?.takeIf { m -> wake.none { it.mac.equals(m, ignoreCase = true) } }
                ?.let { WakeTarget(it, ipv4 = address.takeIf { a -> a.split('.').size == 4 && a.all { c -> c.isDigit() || c == '.' } }) }
            return wake + listOfNotNull(manual)
        }

    fun json(): JSONObject = JSONObject()
        .put("id", id).put("name", name).put("address", address).put("port", port)
        .put("fingerprint", fingerprint ?: JSONObject.NULL).put("user", user)
        .put("os", os ?: JSONObject.NULL).put("gpu", gpu ?: JSONObject.NULL).put("av1", av1)
        .put("virtualDisplay", virtualDisplay).put("mac", mac ?: JSONObject.NULL)
        .put("wake", JSONArray(WakeTarget.toJson(wake)))
        .put("settings", settings.json())
        .put("last", last?.json() ?: JSONObject.NULL).put("paired", paired)
        .put("relay", relay?.json() ?: JSONObject.NULL).put("gamepads", gamepads)
        .put("pyrowave", pyrowave ?: JSONObject.NULL)
        .put("account", account?.json() ?: JSONObject.NULL)

    companion object {
        private fun JSONObject.str(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

        fun parse(o: JSONObject) = Host(
            id = o.getString("id"),
            name = o.optString("name"),
            address = o.optString("address"),
            port = o.optInt("port"),
            fingerprint = o.str("fingerprint"),
            user = o.optString("user"),
            os = o.str("os"),
            gpu = o.str("gpu"),
            av1 = o.optBoolean("av1"),
            virtualDisplay = o.optBoolean("virtualDisplay", true),
            mac = o.str("mac"),
            wake = WakeTarget.parseList(o.optJSONArray("wake")),
            settings = HostSettings.parse(o.optJSONObject("settings")),
            last = o.optJSONObject("last")?.let(LastSession::parse),
            paired = o.optBoolean("paired"),
            relay = HostRelay.parse(o.optJSONObject("relay")),
            gamepads = o.optBoolean("gamepads", true),
            pyrowave = if (o.has("pyrowave") && !o.isNull("pyrowave")) o.optBoolean("pyrowave") else null,
            account = HostAccount.parse(o.optJSONObject("account")),
        )
    }
}

/** Le compte KyberNow d'un ordinateur de l'accueil : [key] (`relais|compte`, `Account::key` du cœur) et sa [station] chez le relais. */
data class HostAccount(val key: String, val station: String) {
    fun json(): JSONObject = JSONObject().put("key", key).put("station", station)

    companion object {
        fun parse(o: JSONObject?): HostAccount? {
            if (o == null) return null
            val key = o.optString("key").takeIf { it.isNotEmpty() } ?: return null
            val station = o.optString("station").takeIf { it.isNotEmpty() } ?: return null
            return HostAccount(key, station)
        }
    }
}

/**
 * L'ordinateur qu'un ami prête à cet appareil par un compte d'ami (KyberNow Share, docs/pret.md) : la carte
 * « Ordinateur de {hôte} » de l'accueil, à part des ordinateurs appairés. [host] : le prénom de l'hôte ; [computer] : le
 * nom de l'ordinateur ; la Station (adresse et port d'Admin, empreinte), l'identifiant de l'ami et, hors de chez soi, le
 * relais du lien d'invitation ([relay], [sid]). Le mot de passe, s'il est retenu, est chiffré comme ceux des ordinateurs.
 */
data class BorrowedComputer(
    val id: String = "pret-" + UUID.randomUUID().toString(),
    val host: String,
    val computer: String,
    val address: String,
    val port: Int,
    val fingerprint: String?,
    val login: String,
    val relay: String? = null,
    val sid: String? = null,
) {
    /** La route par le relais (`{server, station, admin_fp}`), seulement avec l'empreinte d'Admin : jamais de premier contact. */
    val route: String? get() {
        val server = relay ?: return null
        val station = sid ?: return null
        return HostRelay(server, station).route(fingerprint)
    }

    fun json(): JSONObject = JSONObject()
        .put("id", id).put("host", host).put("computer", computer).put("address", address).put("port", port)
        .put("fingerprint", fingerprint ?: JSONObject.NULL).put("login", login)
        .put("relay", relay ?: JSONObject.NULL).put("sid", sid ?: JSONObject.NULL)

    companion object {
        private fun JSONObject.str(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

        fun parse(o: JSONObject) = BorrowedComputer(
            id = o.getString("id"),
            host = o.optString("host"),
            computer = o.optString("computer"),
            address = o.optString("address"),
            port = o.optInt("port"),
            fingerprint = o.str("fingerprint"),
            login = o.optString("login"),
            relay = o.str("relay"),
            sid = o.str("sid"),
        )
    }
}

/**
 * Les ordinateurs de l'appareil (préférences privées de l'app), leurs mots de
 * passe chiffrés par une clé du Keystore d'Android qui ne quitte jamais
 * l'appareil, et leur dernière image (fichiers de l'app).
 */
class HostStore(private val context: Context) {
    private val prefs = context.getSharedPreferences("hosts", Context.MODE_PRIVATE)
    private val secrets = context.getSharedPreferences("secrets", Context.MODE_PRIVATE)
    private val _hosts = MutableStateFlow(load())
    val hosts: StateFlow<List<Host>> = _hosts.asStateFlow()
    private val _borrowed = MutableStateFlow(loadBorrowed())
    /** Les ordinateurs que des amis prêtent à cet appareil (« Ordinateur de {hôte} »), à part. */
    val borrowed: StateFlow<List<BorrowedComputer>> = _borrowed.asStateFlow()

    /** L'ordinateur choisi sur l'accueil. */
    var selectedId: String?
        get() = prefs.getString("selected", null)
        set(v) = prefs.edit().putString("selected", v).apply()

    private fun load(): List<Host> = parseEach(prefs.getString("list", "[]")) { Host.parse(it) }

    private fun save(list: List<Host>) {
        val a = JSONArray()
        list.forEach { a.put(it.json()) }
        prefs.edit().putString("list", a.toString()).apply()
        _hosts.value = list
    }

    fun get(id: String): Host? = _hosts.value.firstOrNull { it.id == id }

    // MARK: - Ordinateurs prêtés par des amis

    private fun loadBorrowed(): List<BorrowedComputer> = parseEach(prefs.getString("borrowed", "[]")) { BorrowedComputer.parse(it) }

    private fun saveBorrowed(list: List<BorrowedComputer>) {
        val a = JSONArray()
        list.forEach { a.put(it.json()) }
        prefs.edit().putString("borrowed", a.toString()).apply()
        _borrowed.value = list
    }

    fun borrowedComputer(id: String): BorrowedComputer? = _borrowed.value.firstOrNull { it.id == id }

    /**
     * Ajoute ou remplace (même Station et même identifiant : même carte) ; [password] retenu s'il est donné, oublié sinon
     * (« Retenir le mot de passe » décoché). L'accueil le choisit.
     */
    fun putBorrowed(computer: BorrowedComputer, password: String?): BorrowedComputer {
        val list = _borrowed.value.toMutableList()
        val i = list.indexOfFirst { it.id == computer.id || (it.address == computer.address && it.port == computer.port && it.login == computer.login) }
        val c = if (i >= 0) computer.copy(id = list[i].id) else computer
        if (i >= 0) list[i] = c else list.add(c)
        saveBorrowed(list)
        if (password != null) setPassword(c.id, password) else forgetPassword(c.id)
        selectedId = c.id
        return c
    }

    fun updateBorrowed(id: String, f: (BorrowedComputer) -> BorrowedComputer) {
        saveBorrowed(_borrowed.value.map { if (it.id == id) f(it) else it })
    }

    fun removeBorrowed(id: String) {
        saveBorrowed(_borrowed.value.filterNot { it.id == id })
        forgetPassword(id)
        if (selectedId == id) selectedId = _hosts.value.firstOrNull()?.id ?: _borrowed.value.firstOrNull()?.id
    }

    /** Ajoute ou remplace (même adresse et même port : même ordinateur ; un ordinateur d'un compte sans adresse connue n'en a pas). */
    fun put(host: Host, password: String? = null) {
        val list = _hosts.value.toMutableList()
        val i = list.indexOfFirst { it.id == host.id || (host.address.isNotEmpty() && it.address == host.address && it.port == host.port) }
        val h = if (i >= 0) host.copy(id = list[i].id) else host
        if (i >= 0) list[i] = h else list.add(h)
        save(list)
        if (password != null) setPassword(h.id, password)
        if (selectedId == null || i < 0) selectedId = h.id
    }

    /**
     * Les réglages d'un ordinateur changés par la personne (avant, après) : les réglages synchronisés d'un ordinateur
     * du compte partent au relais ([SettingsSync.changed]).
     */
    var onSettingsChanged: ((Host, Host) -> Unit)? = null

    fun update(id: String, f: (Host) -> Host) {
        val before = get(id)
        save(_hosts.value.map { if (it.id == id) f(it) else it })
        val after = get(id)
        if (before != null && after != null && before.settings != after.settings) onSettingsChanged?.invoke(before, after)
    }

    /** Les réglages reçus du compte ([SettingsSync]) : écrits sans repartir au relais. */
    fun applySynced(id: String, f: (Host) -> Host) {
        save(_hosts.value.map { if (it.id == id) f(it) else it })
    }

    fun remove(id: String) {
        save(_hosts.value.filterNot { it.id == id })
        forgetPassword(id)
        imageFile(id).delete()
        forgetStationMemory(id)
        if (selectedId == id) selectedId = _hosts.value.firstOrNull()?.id
    }

    /**
     * Les ordinateurs des comptes à l'accueil ([AccountHosts.sync]) : la liste n'est écrite que si elle change ; ceux qui
     * partent (compte déconnecté, ordinateur retiré du compte) emportent leur image.
     */
    fun syncAccounts(merge: fr.leslignesclaires.kybernow.core.AccountMerge) {
        val before = _hosts.value
        val after = AccountHosts.sync(before, merge)
        if (after == before) return
        save(after)
        val gone = before.filter { b -> after.none { it.id == b.id } }
        gone.forEach { imageFile(it.id).delete(); forgetStationMemory(it.id) }
        if (gone.any { it.id == selectedId }) selectedId = after.firstOrNull()?.id ?: _borrowed.value.firstOrNull()?.id
    }

    /** La dernière image de l'ordinateur (fin de session, aperçu d'Admin). */
    fun imageFile(id: String): File = File(context.filesDir, "images").apply { mkdirs() }.resolve("$id.jpg")

    // MARK: - Mise à jour des Stations (docs/link-ui.md, « Mise à jour des Stations »)

    private fun stationMemories(): JSONObject = runCatching { JSONObject(prefs.getString("station_offer", null) ?: "{}") }.getOrDefault(JSONObject())

    /**
     * Le `Memory` du cœur (`station_offer`) de l'ordinateur [id], en JSON : « Plus tard » (la version et la date) et la
     * dernière vérification demandée (la version de l'app et la date). `{}` sans rien de gardé.
     */
    fun stationMemory(id: String): String = stationMemories().optJSONObject(id)?.toString() ?: "{}"

    fun setStationMemory(id: String, json: String) {
        val o = runCatching { JSONObject(json) }.getOrNull() ?: return
        prefs.edit().putString("station_offer", stationMemories().put(id, o).toString()).apply()
    }

    private fun forgetStationMemory(id: String) {
        val all = stationMemories()
        if (all.has(id)) prefs.edit().putString("station_offer", all.apply { remove(id) }.toString()).apply()
    }

    /** La dernière version de l'app lancée sur cet appareil (nulle au tout premier lancement) : l'avis après une mise à jour de l'app. */
    var lastLaunched: String?
        get() = prefs.getString("last_launched", null)
        set(v) = prefs.edit().putString("last_launched", v).apply()

    // MARK: - Mots de passe

    /**
     * La clé d'AndroidKeyStore et les mots de passe déjà déchiffrés, gardés en mémoire (AND-28) : l'accueil en
     * redemandait un par ordinateur à chaque sondage (toutes les 8 s), sur le fil principal, en rechargeant le magasin
     * de clés à chaque fois. Rien de plus n'est écrit sur l'appareil.
     */
    @Volatile private var cachedKey: SecretKey? = null
    private val plain = java.util.concurrent.ConcurrentHashMap<String, String>()

    private fun forgetPassword(id: String) {
        plain.remove(id)
        secrets.edit().remove(id).apply()
    }

    private fun key(): SecretKey = cachedKey ?: loadKey().also { cachedKey = it }

    private fun loadKey(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build(),
        )
        return gen.generateKey()
    }

    fun setPassword(id: String, password: String) {
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        val data = c.iv + c.doFinal(password.toByteArray())
        secrets.edit().putString(id, Base64.encodeToString(data, Base64.NO_WRAP)).apply()
        plain[id] = password
    }

    fun password(id: String): String? = plain[id] ?: decrypt(id)?.also { plain[id] = it }

    private fun decrypt(id: String): String? = runCatching {
        val data = Base64.decode(secrets.getString(id, null) ?: return null, Base64.NO_WRAP)
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, data, 0, 12)) }
        String(c.doFinal(data, 12, data.size - 12))
    }.getOrNull()

    private companion object {
        const val ALIAS = "kybernow-hosts"
    }
}
