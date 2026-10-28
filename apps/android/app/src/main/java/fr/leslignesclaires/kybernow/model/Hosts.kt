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
 * l'afficher sans la bibliothèque native.
 */
enum class Profile(val id: String, val reliable: Boolean) {
    GAME("jeu", false),
    DESKTOP("bureau", true),
    MAX("max", false),
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
) {
    val padSimple get() = padLayout == "simple"

    /** Les réglages s'écartent du profil : « Personnalisé ». */
    val custom get() = codec != "auto" || mbps != 0 || transport != "auto"

    /** Le transport en vigueur : choisi, sinon celui du profil. */
    val reliable get() = when (transport) {
        "reliable" -> true
        "fast" -> false
        else -> profile.reliable
    }

    /** Choisir un profil remet le format, le débit et le transport à ceux du profil. */
    fun apply(p: Profile) = copy(profile = p, codec = "auto", mbps = 0, transport = "auto")

    /** Choisir le transport : celui du profil n'est pas un écart. */
    fun withTransport(reliable: Boolean) = copy(transport = if (reliable == profile.reliable) "auto" else if (reliable) "reliable" else "fast")

    fun json(): JSONObject = JSONObject()
        .put("profile", profile.id).put("resolution", resolution).put("refresh", refresh).put("virtualDisplay", virtualDisplay)
        .put("codec", codec).put("mbps", mbps).put("audio", audio).put("xbox360", xbox360).put("combo", combo)
        .put("backOpensMenu", backOpensMenu).put("guide", guide).put("matchDisplay", matchDisplay)
        .put("gameMode", gameMode).put("mouse", mouse).put("directTouch", directTouch)
        .put("clipboard", clipboard).put("reconnect", reconnect).put("transport", transport)
        .put("onScreenPad", onScreenPad).put("padLayout", padLayout).put("padSize", padSize)
        .put("padOpacity", padOpacity).put("padHaptics", padHaptics)

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
                reconnect = o.optBoolean("reconnect", d.reconnect),
                // Jusqu'à la 0.4.x : `reliable`, le transport fiable forcé.
                transport = o.optString("transport", "").ifEmpty { if (o.optBoolean("reliable")) "reliable" else d.transport },
                // Absents des fichiers d'avant la manette à l'écran : ses valeurs par défaut.
                onScreenPad = o.optBoolean("onScreenPad", d.onScreenPad),
                padLayout = o.optString("padLayout", d.padLayout).takeIf { it == "full" || it == "simple" } ?: d.padLayout,
                padSize = o.optInt("padSize", d.padSize).coerceIn(0, 2),
                padOpacity = o.optInt("padOpacity", d.padOpacity).takeIf { it in PAD_OPACITIES } ?: d.padOpacity,
                padHaptics = o.optInt("padHaptics", d.padHaptics).coerceIn(0, 3),
            ).let { if (it.transport == "reliable" && it.profile.reliable) it.copy(transport = "auto") else it }
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
) {
    fun json(): JSONObject = JSONObject().put("median", median).put("p95", p95).put("seconds", seconds).put("at", at)
        .put("width", width).put("height", height).put("refresh", refresh).put("codec", codec ?: JSONObject.NULL).put("mbps", mbps)
        .put("route", route ?: JSONObject.NULL)

    companion object {
        fun parse(o: JSONObject) = LastSession(
            o.optDouble("median"), o.optDouble("p95"), o.optLong("seconds"), o.optLong("at"),
            width = o.optInt("width"), height = o.optInt("height"), refresh = o.optInt("refresh"),
            codec = if (o.isNull("codec")) null else o.optString("codec").takeIf { it.isNotEmpty() },
            mbps = o.optInt("mbps"),
            route = if (o.isNull("route")) null else o.optString("route").takeIf { it.isNotEmpty() },
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
) {
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
        .put("relay", relay?.json() ?: JSONObject.NULL)

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

    /** L'ordinateur choisi sur l'accueil. */
    var selectedId: String?
        get() = prefs.getString("selected", null)
        set(v) = prefs.edit().putString("selected", v).apply()

    private fun load(): List<Host> = runCatching {
        val a = JSONArray(prefs.getString("list", "[]"))
        (0 until a.length()).map { Host.parse(a.getJSONObject(it)) }
    }.getOrDefault(emptyList())

    private fun save(list: List<Host>) {
        val a = JSONArray()
        list.forEach { a.put(it.json()) }
        prefs.edit().putString("list", a.toString()).apply()
        _hosts.value = list
    }

    fun get(id: String): Host? = _hosts.value.firstOrNull { it.id == id }

    /** Ajoute ou remplace (même adresse et même port : même ordinateur). */
    fun put(host: Host, password: String? = null) {
        val list = _hosts.value.toMutableList()
        val i = list.indexOfFirst { it.id == host.id || (it.address == host.address && it.port == host.port) }
        val h = if (i >= 0) host.copy(id = list[i].id) else host
        if (i >= 0) list[i] = h else list.add(h)
        save(list)
        if (password != null) setPassword(h.id, password)
        if (selectedId == null || i < 0) selectedId = h.id
    }

    fun update(id: String, f: (Host) -> Host) {
        save(_hosts.value.map { if (it.id == id) f(it) else it })
    }

    fun remove(id: String) {
        save(_hosts.value.filterNot { it.id == id })
        secrets.edit().remove(id).apply()
        imageFile(id).delete()
        if (selectedId == id) selectedId = _hosts.value.firstOrNull()?.id
    }

    /** La dernière image de l'ordinateur (fin de session, aperçu d'Admin). */
    fun imageFile(id: String): File = File(context.filesDir, "images").apply { mkdirs() }.resolve("$id.jpg")

    // MARK: - Mots de passe

    private fun key(): SecretKey {
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
    }

    fun password(id: String): String? = runCatching {
        val data = Base64.decode(secrets.getString(id, null) ?: return null, Base64.NO_WRAP)
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, data, 0, 12)) }
        String(c.doFinal(data, 12, data.size - 12))
    }.getOrNull()

    private companion object {
        const val ALIAS = "kybernow-hosts"
    }
}
