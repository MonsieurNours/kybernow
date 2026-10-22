package fr.leslignesclaires.kybernow.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
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
        )
    }
}

/** Réponse d'Admin : valeur, ou code d'erreur (1 injoignable, 3 certificat changé, −401 compte refusé, −404 appel inconnu). */
sealed class Reply<out T> {
    data class Ok<T>(val value: T) : Reply<T>()
    data class Failed(val code: Int, val message: String) : Reply<Nothing>() {
        val unauthorized get() = code == -401
        val unknownCall get() = code == -404
        val certificateChanged get() = code == 3
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
        return if (code == 0) Reply.Ok(value(o.opt("value"))) else Reply.Failed(code, o.optString("error"))
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

    override fun close() {
        if (handle != 0L) {
            Native.consoleFree(handle)
            handle = 0L
        }
    }
}
