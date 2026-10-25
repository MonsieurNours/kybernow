package fr.leslignesclaires.kybernow.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** Ce qu'Admin dit de l'ordinateur (`GET /api/stream/info`). */
data class HostInfo(
    val hostname: String,
    val os: String,
    val gpu: String?,
    val av1: Boolean,
    val adminVersion: String?,
    val virtualDisplay: Boolean,
    val speakers: Boolean,
    /** La Station (0.5.0 et suivantes) ; nul avec une plus ancienne : ni redémarrage ni mise à jour depuis l'app. */
    val station: StationInfo? = null,
    /** Les cartes réseau de l'ordinateur pour le réveiller, Ethernet d'abord ; vide avec une Station d'avant la 0.5.0. */
    val wake: List<WakeTarget> = emptyList(),
    /** Le compte de l'app a été créé pour cet appareil par un code (`account.own_device`, Station 0.5.0 et suivantes). */
    val ownDevice: Boolean = false,
) {
    companion object {
        fun parse(o: JSONObject) = HostInfo(
            hostname = o.optString("hostname"),
            os = o.optString("os"),
            gpu = o.optString("gpu").takeIf { it.isNotEmpty() && it != "null" },
            av1 = o.optBoolean("av1"),
            adminVersion = o.optString("admin_version").takeIf { it.isNotEmpty() && it != "null" },
            virtualDisplay = o.optJSONObject("virtual_display")?.optBoolean("available") ?: false,
            speakers = o.optJSONObject("audio")?.optBoolean("speakers") ?: false,
            station = o.optJSONObject("station")?.let(StationInfo::parse),
            wake = WakeTarget.parseList(o.optJSONArray("wake")),
            ownDevice = o.optJSONObject("account")?.optBoolean("own_device") ?: false,
        )
    }
}

/**
 * Une carte réseau de l'ordinateur pour le réveiller par le réseau (`wake` de `GET /api/stream/info`) :
 * [mac] (`AA:BB:CC:DD:EE:FF`), sa dernière adresse IPv4 [ipv4], la diffusion de son sous-réseau
 * [broadcast], [kind] (`ethernet` ou `wifi`), [armed] (Windows la laisse réveiller l'ordinateur ; nul :
 * inconnu) et son nom [name]. L'app garde la liste de chaque ordinateur quand un `info` en donne une
 * ([toJson], [listFromJson]) et la rend à [Native.wake] ; une adresse saisie à la main tient dans
 * `WakeTarget(mac, ipv4)`.
 */
data class WakeTarget(
    val mac: String,
    val ipv4: String? = null,
    val broadcast: String? = null,
    val kind: String = "",
    val armed: Boolean? = null,
    val name: String = "",
) {
    fun json(): JSONObject = JSONObject()
        .put("mac", mac)
        .put("ipv4", ipv4 ?: JSONObject.NULL)
        .put("broadcast", broadcast ?: JSONObject.NULL)
        .put("kind", kind)
        .put("armed", armed ?: JSONObject.NULL)
        .put("name", name)

    companion object {
        private fun JSONObject.text(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

        /** Une carte ; nulle sans adresse MAC. */
        fun parse(o: JSONObject): WakeTarget? = o.text("mac")?.let { mac ->
            WakeTarget(
                mac = mac,
                ipv4 = o.text("ipv4"),
                broadcast = o.text("broadcast"),
                kind = o.text("kind") ?: "",
                armed = if (o.isNull("armed")) null else o.optBoolean("armed"),
                name = o.text("name") ?: "",
            )
        }

        /** La liste `wake` d'Admin ; les cartes illisibles sont ignorées. */
        fun parseList(a: JSONArray?): List<WakeTarget> =
            if (a == null) emptyList() else (0 until a.length()).mapNotNull { i -> a.optJSONObject(i)?.let(::parse) }

        /** La liste en JSON, pour la garder avec l'ordinateur et pour [Native.wake]. */
        fun toJson(list: List<WakeTarget>): String = JSONArray().apply { list.forEach { put(it.json()) } }.toString()

        /** La liste gardée par [toJson] ; vide si le texte est absent ou illisible. */
        fun listFromJson(text: String?): List<WakeTarget> =
            if (text.isNullOrEmpty()) emptyList() else runCatching { parseList(JSONArray(text)) }.getOrDefault(emptyList())
    }
}

/** La Station (`station` de `GET /api/stream/info`) : sa version, sa mise à jour, un redémarrage en cours. */
data class StationInfo(val version: String, val update: StationUpdate, val restarting: Boolean) {
    companion object {
        fun parse(o: JSONObject) = StationInfo(
            version = o.optString("version"),
            update = StationUpdate.parse(o.optJSONObject("update")),
            restarting = o.optBoolean("restarting"),
        )
    }
}

/**
 * La mise à jour de la Station, telle qu'Admin la connaît : une version plus récente et signée
 * ([available], [version]), qui s'installe depuis l'app ([installable]) ou avec l'installateur, sur
 * l'ordinateur ([requiresSetup]), et une installation en cours ([installing]).
 */
data class StationUpdate(
    val available: Boolean = false,
    val version: String? = null,
    val installable: Boolean = false,
    val requiresSetup: Boolean = false,
    val installing: Boolean = false,
) {
    companion object {
        fun parse(o: JSONObject?) = if (o == null) StationUpdate() else StationUpdate(
            available = o.optBoolean("available"),
            version = o.optString("version").takeIf { it.isNotEmpty() && it != "null" },
            installable = o.optBoolean("installable"),
            requiresSetup = o.optBoolean("requires_setup"),
            installing = o.optBoolean("installing"),
        )
    }
}

/**
 * Réponse d'Admin : valeur, ou code d'erreur (1 injoignable, 3 certificat changé, −401 compte refusé,
 * −404 appel inconnu ; refus de la Station : 10 à 15, avec [Failed.device] et [Failed.seconds]).
 */
sealed class Reply<out T> {
    data class Ok<T>(val value: T) : Reply<T>()
    data class Failed(val code: Int, val message: String, val device: String? = null, val seconds: Long? = null) : Reply<Nothing>() {
        val unauthorized get() = code == -401
        val unknownCall get() = code == -404
        val certificateChanged get() = code == 3

        /** Un appareil est en session ([device]) : refaire l'appel avec `force` coupe sa session. */
        val inSession get() = code == 10
        /** Trop tôt (un redémarrage par minute, ou trop d'essais ratés) : attendre [seconds]. */
        val tooSoon get() = code == 11
        /** Rien à installer depuis l'app : Station à jour, ou version refusée. */
        val nothingToInstall get() = code == 12
        /** La version s'installe avec l'installateur, sur l'ordinateur. */
        val requiresSetup get() = code == 13
        /** Une installation ou un redémarrage est déjà en cours. */
        val busy get() = code == 14
        /** Station 0.4.9 ou plus ancienne : elle ne connaît pas l'appel. */
        val stationTooOld get() = code == 15
    }

    fun valueOrNull(): T? = (this as? Ok<T>)?.value
}

enum class PairState { PENDING, ACCEPTED, REFUSED, EXPIRED }

data class PairStatus(val state: PairState, val username: String? = null, val password: String? = null)

/**
 * Client d'Admin hors session (le cœur : certificat épinglé, `Accept-Language`).
 * `pinned` vide : le premier certificat vu est accepté et lisible par
 * [fingerprintSeen] (premier contact par adresse). Appels sur `Dispatchers.IO`.
 */
class ConsoleClient(host: String, port: Int, user: String = "", password: String = "", pinned: String? = null) : AutoCloseable {
    private var handle = Native.consoleNew(host, port, user, password, pinned)

    val fingerprintSeen: String? get() = if (handle != 0L) Native.consoleFingerprintSeen(handle) else null

    private fun <T> parse(json: String?, value: (Any?) -> T): Reply<T> {
        if (json == null) return Reply.Failed(1, "")
        val o = JSONObject(json)
        val code = o.optInt("code", 1)
        if (code == 0) return Reply.Ok(value(o.opt("value")))
        val device = o.optString("device").takeIf { it.isNotEmpty() && it != "null" }
        val seconds = if (o.has("seconds") && !o.isNull("seconds")) o.optLong("seconds") else null
        return Reply.Failed(code, o.optString("error"), device, seconds)
    }

    suspend fun info(): Reply<HostInfo> = withContext(Dispatchers.IO) {
        parse(Native.consoleInfo(handle)) { HostInfo.parse(it as JSONObject) }
    }

    /** Premier contact : l'info si le compte est accepté, nul sinon (le certificat est vu). */
    suspend fun probe(): Reply<HostInfo?> = withContext(Dispatchers.IO) {
        parse(Native.consoleProbe(handle)) { (it as? JSONObject)?.let(HostInfo::parse) }
    }

    /** Aperçu JPEG de l'écran de l'ordinateur ; vide si Admin ne peut pas capturer. */
    suspend fun preview(width: Int): ByteArray? = withContext(Dispatchers.IO) { Native.consolePreview(handle, width) }

    suspend fun pairRequest(code: String, device: DeviceIdentity): Reply<String> = withContext(Dispatchers.IO) {
        parse(Native.consolePairRequest(handle, code, device.json())) { it as String }
    }

    suspend fun pairStatus(id: String): Reply<PairStatus> = withContext(Dispatchers.IO) {
        parse(Native.consolePairStatus(handle, id)) {
            val o = it as JSONObject
            when (o.optString("status")) {
                "accepted" -> PairStatus(PairState.ACCEPTED, o.optString("username"), o.optString("password"))
                "refused" -> PairStatus(PairState.REFUSED)
                "expired" -> PairStatus(PairState.EXPIRED)
                else -> PairStatus(PairState.PENDING)
            }
        }
    }

    /** Adresse d'Admin à ouvrir dans le navigateur (`https://adresse:port/`). */
    val adminUrl: String? get() = if (handle != 0L) Native.consoleAdminUrl(handle) else null

    /**
     * Redémarre la Station entière (le serveur et Admin, environ une minute). [deviceId] : cet
     * appareil, nommé dans le fil d'Admin ; [force] : même si un appareil est en session (coupée).
     */
    suspend fun stationRestart(deviceId: String?, force: Boolean = false): Reply<Unit> = withContext(Dispatchers.IO) {
        parse(Native.consoleStationRestart(handle, deviceId, force)) { }
    }

    /**
     * Lance la mise à jour de la Station vers [StationUpdate.version] ; rend cette version. Bloque le
     * temps qu'Admin télécharge et vérifie le paquet (quelques minutes au plus).
     */
    suspend fun stationUpdate(deviceId: String?, force: Boolean = false): Reply<String> = withContext(Dispatchers.IO) {
        parse(Native.consoleStationUpdate(handle, deviceId, force)) { (it as JSONObject).optString("version") }
    }

    override fun close() {
        if (handle != 0L) {
            Native.consoleFree(handle)
            handle = 0L
        }
    }
}
