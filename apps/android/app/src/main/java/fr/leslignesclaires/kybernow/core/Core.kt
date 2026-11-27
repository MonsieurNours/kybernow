package fr.leslignesclaires.kybernow.core

import android.util.Log
import org.json.JSONObject

/**
 * Les règles et les textes du cœur que l'interface montre ou applique, d'une seule source (audit du 27/11, phase 2b :
 * AND-30, TRANS-34, TRANS-40 à 42 ; guide `docs/audit-2026-11-27/migration-coeur-2b.md`) : l'app n'en garde aucune
 * copie. [source] est la bibliothèque native ; les essais JVM, qui ne l'ont pas, mettent la leur. Sans réponse du cœur,
 * une valeur neutre (texte vide, faux, nul), écrite au journal : jamais une chute de l'app.
 */
object Core {
    /** Ce que le cœur répond (les fonctions de [Native] du même nom). */
    interface Source {
        fun shortFingerprint(fp: String): String
        fun fingerprintSame(a: String, b: String): Boolean
        fun parseAddress(text: String, defaultPort: Int): String?
        fun valuesJson(capabilitiesJson: String): String?
        fun lendAvailabilityLabel(state: String): String?
        fun relayErrorText(code: Int, name: String?): String?
        fun relayPathLabel(transport: Int): String?
        fun relayParts(addr: String): String?
        fun relayLabel(addr: String): String
        fun looksLikeInvite(text: String): Boolean
        fun discoveryFound(json: String): String?
        fun recordingClock(seconds: Long): String
        fun profileSettings(profile: String, hevc: Boolean, hevc444: Boolean, av1: Boolean): String
        fun accountSettingsKnown(syncedJson: String): String
        fun accountSettingsApply(valuesJson: String, baseJson: String, capsJson: String): String
        fun accountSettingsFromStream(settingsJson: String): String
        fun welcome(stateJson: String, op: String, hasAccount: Boolean, computer: String, now: Long): String
        fun welcomeSignUpUrl(relay: String): String
        fun welcomeStationUrl(): String
    }

    /** La bibliothèque native. */
    object NativeSource : Source {
        override fun shortFingerprint(fp: String) = Native.shortFingerprint(fp)
        override fun fingerprintSame(a: String, b: String) = Native.fingerprintSame(a, b)
        override fun parseAddress(text: String, defaultPort: Int) = Native.parseAddress(text, defaultPort)
        override fun valuesJson(capabilitiesJson: String) = Native.valuesJson(capabilitiesJson)
        override fun lendAvailabilityLabel(state: String) = Native.lendAvailabilityLabel(state)
        override fun relayErrorText(code: Int, name: String?) = Native.relayErrorText(code, name)
        override fun relayPathLabel(transport: Int) = Native.relayPathLabel(transport)
        override fun relayParts(addr: String) = Native.relayParts(addr)
        override fun relayLabel(addr: String) = Native.relayLabel(addr)
        override fun looksLikeInvite(text: String) = Native.looksLikeInvite(text)
        override fun discoveryFound(json: String) = Native.discoveryFound(json)
        override fun recordingClock(seconds: Long) = Native.recordingClock(seconds)
        override fun profileSettings(profile: String, hevc: Boolean, hevc444: Boolean, av1: Boolean) = Native.profileSettings(profile, hevc, hevc444, av1)
        override fun accountSettingsKnown(syncedJson: String) = Native.accountSettingsKnown(syncedJson)
        override fun accountSettingsApply(valuesJson: String, baseJson: String, capsJson: String) = Native.accountSettingsApply(valuesJson, baseJson, capsJson)
        // Android n'a pas de mode immersif : rien à joindre.
        override fun accountSettingsFromStream(settingsJson: String) = Native.accountSettingsFromStream(settingsJson, null)
        override fun welcome(stateJson: String, op: String, hasAccount: Boolean, computer: String, now: Long) = Native.welcome(stateJson, op, hasAccount, computer, now)
        override fun welcomeSignUpUrl(relay: String) = Native.welcomeSignUpUrl(relay)
        override fun welcomeStationUrl() = Native.welcomeStationUrl()
    }

    @Volatile
    var source: Source = NativeSource
        set(value) {
            field = value
            values = null
        }

    /** Une règle de l'accueil (`welcome.rs`) : `{state, value}` en JSON ; nul sans réponse. */
    fun welcome(state: String, op: String, hasAccount: Boolean, computer: String, now: Long): String? =
        ask("accueil", null) { welcome(state, op, hasAccount, computer, now) }

    /** « Créer un compte » : la page d'inscription du relais (vide : le relais public) ; vide sans réponse. */
    fun welcomeSignUpUrl(relay: String): String = ask("accueil", "") { welcomeSignUpUrl(relay) }

    /** La page de téléchargement de KyberNow Station ; vide sans réponse. */
    fun welcomeStationUrl(): String = ask("accueil", "") { welcomeStationUrl() }

    private inline fun <T> ask(what: String, neutral: T, f: Source.() -> T): T =
        runCatching { source.f() }.getOrElse {
            Log.w("KyberNow", "cœur : $what sans réponse ($it)")
            neutral
        }

    /** L'empreinte d'Admin telle qu'on la montre : les quatre premiers groupes, « 7C41 E09B AABB CCDD » ; vide sans chiffre. */
    fun shortFingerprint(fp: String?): String = if (fp.isNullOrBlank()) "" else ask("empreinte", "") { shortFingerprint(fp) }

    /** Deux empreintes sont la même (préfixe, deux-points, espaces et casse ignorés) ; une empreinte absente ne l'est jamais. */
    fun sameFingerprint(a: String?, b: String?): Boolean =
        !a.isNullOrBlank() && !b.isNullOrBlank() && ask("empreinte", false) { fingerprintSame(a, b) }

    /** Une adresse tapée lue par le cœur. */
    data class Address(val host: String, val port: Int)

    /** Une adresse tapée : le port [defaultPort] quand il manque (nul : il est exigé) ; une IPv6 nue est l'hôte entier. */
    fun parseAddress(text: String, defaultPort: Int?): Address? =
        ask("adresse", null) { parseAddress(text, defaultPort ?: 0) }?.let { json ->
            runCatching { JSONObject(json).let { Address(it.getString("host"), it.getInt("port")) } }.getOrNull()
        }

    /**
     * Les valeurs communes des apps (`plan::values_json`), lues une fois : celles qui ne dépendent pas des décodeurs de
     * l'appareil (bornes du débit à la main, délais de la mise à jour des Stations, port d'Admin, relais public).
     */
    data class Values(
        /** Débit réglé à la main, en Mbit/s : au plus, et au plus en PyroWave (0 : inconnu). */
        val manualMbpsMax: Int,
        val manualMbpsMaxPyroWave: Int,
        /** Délai d'attente de la vérification d'une Station avant l'avis de mise à jour (ms). */
        val stationNoticeWaitMs: Long,
        /** La dernière version lancée supposée quand l'app n'en a pas gardé mais connaît des ordinateurs. */
        val stationNoticeBefore: String?,
        /** Le port d'Admin, et l'exemple d'adresse à taper. */
        val adminPort: Int,
        val addressExample: String,
        /** Le relais public. */
        val defaultRelay: String,
    ) {
        companion object {
            fun parse(json: String?): Values {
                val o = runCatching { JSONObject(json ?: "{}") }.getOrNull() ?: JSONObject()
                val manual = o.optJSONObject("manual_mbps") ?: JSONObject()
                return Values(
                    manualMbpsMax = manual.optInt("max"),
                    manualMbpsMaxPyroWave = manual.optInt("max_pyrowave"),
                    stationNoticeWaitMs = o.optLong("station_notice_wait_secs") * 1000,
                    stationNoticeBefore = o.optString("station_notice_before").takeIf { it.isNotEmpty() },
                    adminPort = o.optInt("admin_port"),
                    addressExample = o.optString("address_example"),
                    defaultRelay = o.optString("default_relay"),
                )
            }
        }
    }

    @Volatile private var values: Values? = null

    /** Les valeurs communes (vides sans réponse du cœur, sans être gardées : redemandées à l'appel suivant). */
    fun values(): Values = values ?: ask("valeurs communes", null) { valuesJson("{}") }?.let { Values.parse(it).also { v -> values = v } }
        ?: Values.parse(null)

    /** « Disponible », « Occupé », « Prêté à quelqu'un d'autre », « Verrouillé », « Prêt coupé » (`LendAvailability::label`). */
    fun lendAvailabilityLabel(state: String): String = ask("état du prêt", null) { lendAvailabilityLabel(state) }.orEmpty()

    /**
     * Le texte de link-ui d'une erreur du relais avec le nom de l'ordinateur : 30 « Le relais ne répond pas. », 31 « {nom}
     * n'est pas joignable : l'ordinateur est éteint ou en veille. », 32 « {nom} n'est pas joignable hors de chez vous
     * (accès coupé dans Admin). » ; nul pour un autre code.
     */
    fun relayErrorText(code: Int, name: String?): String? = ask("erreur du relais", null) { relayErrorText(code, name) }

    /** « Direct », « Par le relais », « Par le relais (TCP) » : la ligne « Chemin », et la puce « Par le relais ». */
    fun routeLabel(transport: RouteTransport): String = ask("chemin", null) { relayPathLabel(
        when (transport) { RouteTransport.DIRECT -> 0; RouteTransport.RELAY -> 1; RouteTransport.RELAY_TCP -> 2 },
    ) }.orEmpty()

    /** Le relais tel qu'on le montre (`rules::relay::label`) : `share.kybernow.app`, `relais.exemple.org:9443`, sans empreinte. */
    fun relayLabel(addr: String): String = ask("relais", "") { relayLabel(addr) }

    /** Les parties d'une adresse de relais (`rules::relay::parse`) : `{authority, pin}` ; nul si elle ne se lit pas. */
    fun relayParts(addr: String): JSONObject? = ask("relais", null) { relayParts(addr) }?.let { runCatching { JSONObject(it) }.getOrNull() }

    /**
     * Une Station trouvée sur le réseau, lue par le cœur (`discovery::found`) : l'annonce résolue par l'app ([fullname],
     * [port], ses [addresses], son TXT) ; rend `{id, name, host, port, fingerprint}`, ou nul pour une annonce à ignorer.
     */
    fun discoveryFound(fullname: String, port: Int, addresses: List<String>, txt: Map<String, String?>): JSONObject? {
        val o = JSONObject().put("fullname", fullname).put("port", port).put("addresses", org.json.JSONArray(addresses))
            .put("txt", JSONObject().apply { txt.forEach { (k, v) -> if (v != null) put(k, v) } })
        return ask("découverte", null) { discoveryFound(o.toString()) }?.let { runCatching { JSONObject(it) }.getOrNull() }
    }

    /** Le texte ressemble à un lien d'invitation, lisible ou non. */
    fun looksLikeInvite(text: String): Boolean = ask("lien d'invitation", false) { looksLikeInvite(text) }

    /** Les réglages d'un profil (`jeu`, `bureau`, `max`, `auto`) ramenés aux décodeurs de l'appareil (`StreamSettings::for_profile`). */
    fun profileSettings(profile: String, hevc: Boolean, hevc444: Boolean, av1: Boolean): JSONObject? =
        ask("profil", null) { profileSettings(profile, hevc, hevc444, av1) }?.let { runCatching { JSONObject(it) }.getOrNull() }

    /** Les clés connues et vérifiées de réglages synchronisés (`SyncedSettings::known`) ; nul sans réponse. */
    fun settingsKnown(synced: JSONObject): JSONObject? =
        ask("réglages synchronisés", null) { accountSettingsKnown(synced.toString()) }?.let(::accountValue)

    /** Des réglages reçus appliqués aux réglages de session (`SyncedValues::apply_to`, bornes du débit comprises) ; nul sans réponse. */
    fun settingsApply(values: JSONObject, base: JSONObject, caps: JSONObject): JSONObject? =
        ask("réglages synchronisés", null) { accountSettingsApply(values.toString(), base.toString(), caps.toString()) }?.let(::accountValue)

    /** Toutes les clés à envoyer d'après des réglages de session (`SyncedValues::from_stream`) ; nul sans réponse. */
    fun settingsFromStream(stream: JSONObject): JSONObject? =
        ask("réglages synchronisés", null) { accountSettingsFromStream(stream.toString()) }?.let(::accountValue)

    private fun accountValue(text: String): JSONObject? = (Accounts.read(text) { it as? JSONObject } as? AccountReply.Ok)?.value

    /** Le temps d'un enregistrement : « 02:14 », « 59:59 », « 1:02:14 ». */
    fun recordingClock(seconds: Long): String = ask("temps de l'enregistrement", "") { recordingClock(seconds.coerceAtLeast(0)) }
}
