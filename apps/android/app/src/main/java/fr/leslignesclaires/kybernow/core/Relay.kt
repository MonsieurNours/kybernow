package fr.leslignesclaires.kybernow.core

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/*
 * Le relais de KyberNow vu de l'app (docs/relais.md) : partager et se connecter à son ordinateur hors de chez soi. Le
 * cœur ouvre le tunnel et fait passer Admin et Kyber par lui ; ce fichier range ses réponses pour l'interface (le
 * chemin d'une session, la présence des ordinateurs chez le relais).
 */

/** Par où passe une session (`transport` de `SessionListener.onRoute`, ligne « Chemin » des statistiques). */
enum class RouteTransport(val id: String) {
    /** Sur le réseau, sans relais. */
    DIRECT("direct"),
    /** Par le relais, en QUIC. */
    RELAY("relay"),
    /** Par le relais, repli TCP (l'UDP ne passe pas) : bon pour regarder, moins bon pour jouer. */
    RELAY_TCP("relay_tcp");

    /** Par le relais : la puce « Par le relais ». */
    val viaRelay get() = this != DIRECT

    companion object {
        /** Un chemin inconnu (cœur plus récent) ou absent (session d'avant le relais) est le réseau. */
        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: DIRECT
    }
}

/**
 * Le chemin d'une session (`RouteInfo` du cœur) : [transport], le relais ([relay], `hôte`, ou `hôte:port` s'il n'est pas
 * sur le 443), et les allers-retours en ms entre l'appareil et le relais ([rttClient]) et entre le relais et l'ordinateur ([rttStation]).
 */
data class RouteInfo(val transport: RouteTransport, val relay: String? = null, val rttClient: Double? = null, val rttStation: Double? = null) {
    val viaRelay get() = transport.viaRelay

    companion object {
        fun parse(json: String?): RouteInfo? = runCatching {
            val o = JSONObject(json ?: return null)
            fun number(key: String) = (o.opt(key) as? Number)?.toDouble()
            RouteInfo(
                RouteTransport.parse(o.optString("transport")),
                relay = if (o.isNull("relay")) null else o.optString("relay").takeIf { it.isNotEmpty() },
                rttClient = number("rtt_client"),
                rttStation = number("rtt_station"),
            )
        }.getOrNull()
    }
}

/**
 * Pourquoi le relais n'a pas mené à l'ordinateur : le code que donne le cœur (`RelayError::code`, `Reply.Failed.code`,
 * `SessionListener.onRelayError`). Jamais reconnu au texte du message d'Admin.
 */
enum class RelayFailure(val code: Int) {
    /** 30 : le relais ne répond pas (et le reste : refus du relais, adresse illisible). */
    UNREACHABLE(30),
    /** 31 : l'ordinateur n'est pas connecté au relais (éteint, en veille). */
    STATION_OFFLINE(31),
    /** 32 : « Accès hors de chez vous » est coupé dans Admin (403 `remote_access_off` par le relais). */
    ACCESS_OFF(32);

    companion object {
        /** Nul pour un autre code (réponse d'Admin, certificat, compte refusé…). */
        fun of(code: Int) = entries.firstOrNull { it.code == code }
    }
}

object Relays {
    /**
     * L'adresse d'un relais sous sa forme normale (le cœur : `share.kybernow.app` pour le 443, un autre port gardé :
     * `relais.exemple.org:9443`) ; deux relais sont le même si leurs formes sont égales. Illisible, ou sans
     * bibliothèque native (essais) : l'adresse sans espaces ni majuscules.
     */
    fun normalize(addr: String): String =
        runCatching { Native.relayNormalize(addr) }.getOrNull() ?: addr.trim().lowercase()

    /** `KYBERNOW_RELAY=off` : jamais de relais (essais). `force` ne change rien ici : le cœur y passe `Auto` de lui-même. */
    val off: Boolean get() = Native.relayMode() == 2

    /**
     * Quels ordinateurs ([stations] : leurs identifiants chez le relais, 16 au plus) sont connectés à ce relais :
     * un booléen chacun, dans l'ordre ; `null` si le relais ne répond pas. Une connexion brève à chaque appel : à
     * espacer (toutes les 30 s au plus). Bloquant côté cœur : hors du fil principal.
     */
    suspend fun presence(server: String, stations: List<String>): List<Boolean>? = withContext(Dispatchers.IO) {
        if (stations.isEmpty()) return@withContext emptyList()
        val text = Native.relayPresence(server, JSONArray(stations.take(MAX_STATIONS)).toString())
        runCatching {
            val o = JSONObject(text)
            if (o.optInt("code", 1) != 0) return@runCatching null
            val a = o.getJSONArray("value")
            (0 until a.length()).map { a.getBoolean(it) }
        }.getOrNull()
    }

    /** Le relais ne prend pas plus de seize identifiants par question. */
    const val MAX_STATIONS = 16
}
