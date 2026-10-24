package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
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

/** Profils, dans le vocabulaire du cœur. */
enum class Profile(val id: String) {
    GAME("jeu"),
    DESKTOP("bureau"),
    MAX("max"),
}

/** Réglages d'un ordinateur (comme Réglages › par ordinateur sur les autres apps). */
data class HostSettings(
    val profile: Profile = Profile.GAME,
    /** 0 : comme cet écran ; sinon hauteur (2160, 1440, 1080, 720). */
    val resolution: Int = 0,
    /** 0 : la plus haute que l'écran tient à cette définition. */
    val refresh: Int = 0,
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
    /** Transport fiable même en profil Jeu (Wi-Fi chargé). */
    val reliable: Boolean = false,
) {
    fun json(): JSONObject = JSONObject()
        .put("profile", profile.id).put("resolution", resolution).put("refresh", refresh).put("codec", codec)
        .put("mbps", mbps).put("audio", audio).put("xbox360", xbox360).put("combo", combo)
        .put("backOpensMenu", backOpensMenu).put("guide", guide).put("matchDisplay", matchDisplay)
        .put("gameMode", gameMode).put("mouse", mouse).put("directTouch", directTouch)
        .put("clipboard", clipboard).put("reconnect", reconnect).put("reliable", reliable)

    companion object {
        fun parse(o: JSONObject?): HostSettings {
            if (o == null) return HostSettings()
            val d = HostSettings()
            return HostSettings(
                profile = Profile.entries.firstOrNull { it.id == o.optString("profile") } ?: d.profile,
                resolution = o.optInt("resolution", d.resolution),
                refresh = o.optInt("refresh", d.refresh),
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
                reliable = o.optBoolean("reliable", d.reliable),
            )
        }
    }
}

/** La dernière session : latence médiane et 95e centile (ms), durée (s), quand. */
data class LastSession(val median: Double, val p95: Double, val seconds: Long, val at: Long)

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
    /** Adresse matérielle pour le réveil (Wake-on-LAN), si connue. */
    val mac: String? = null,
    val settings: HostSettings = HostSettings(),
    val last: LastSession? = null,
) {
    fun json(): JSONObject = JSONObject()
        .put("id", id).put("name", name).put("address", address).put("port", port)
        .put("fingerprint", fingerprint ?: JSONObject.NULL).put("user", user)
        .put("os", os ?: JSONObject.NULL).put("gpu", gpu ?: JSONObject.NULL).put("av1", av1)
        .put("virtualDisplay", virtualDisplay).put("mac", mac ?: JSONObject.NULL)
        .put("settings", settings.json())
        .put("last", last?.let { JSONObject().put("median", it.median).put("p95", it.p95).put("seconds", it.seconds).put("at", it.at) } ?: JSONObject.NULL)

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
            settings = HostSettings.parse(o.optJSONObject("settings")),
            last = o.optJSONObject("last")?.let {
                LastSession(it.optDouble("median"), it.optDouble("p95"), it.optLong("seconds"), it.optLong("at"))
            },
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
