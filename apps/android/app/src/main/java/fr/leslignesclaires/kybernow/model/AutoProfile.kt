package fr.leslignesclaires.kybernow.model

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiInfo
import fr.leslignesclaires.kybernow.core.Native
import org.json.JSONArray
import org.json.JSONObject
import java.text.DateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.max
import kotlin.math.min

/*
 * Profil « Auto » et débit adaptatif (docs/link-ui.md, « Profil Auto et débit adaptatif » ; contrat :
 * docs/profil-auto.md). Le test des conditions est mené par le cœur (`autotest::run`, apps/android/jni/src/autotest.rs) ;
 * ici, ce que l'app en garde et en fait, sans état (essayé par `AutoProfileTest`).
 */

/** Une étape du test telle que le cœur la donne (`Progress`) : `link`, `latency`, `bandwidth`, `decode`. */
data class AutoStep(val key: String, val title: String, val state: State, val text: String?) {
    enum class State { WAITING, RUNNING, DONE, SKIPPED }

    companion object {
        /** `{"steps": [{step, title, state, text}]}` ; vide s'il est illisible. */
        fun parseProgress(json: String): List<AutoStep> = runCatching {
            val a = JSONObject(json).optJSONArray("steps") ?: JSONArray()
            (0 until a.length()).mapNotNull { i ->
                val o = a.optJSONObject(i) ?: return@mapNotNull null
                val state = when (o.optString("state")) {
                    "running" -> State.RUNNING
                    "done" -> State.DONE
                    "skipped" -> State.SKIPPED
                    else -> State.WAITING
                }
                AutoStep(o.optString("step"), o.optString("title"), state, if (o.isNull("text")) null else o.optString("text").takeIf { it.isNotEmpty() })
            }
        }.getOrDefault(emptyList())
    }
}

/** Une ligne du résumé : « Format vidéo » « PyroWave » — « relié par câble, et le réseau le porte ». */
data class AutoChoice(val key: String, val title: String, val value: String, val reason: String) {
    companion object {
        fun parseList(a: JSONArray?): List<AutoChoice> =
            if (a == null) emptyList() else (0 until a.length()).mapNotNull { i ->
                a.optJSONObject(i)?.let { AutoChoice(it.optString("key"), it.optString("title"), it.optString("value"), it.optString("reason")) }
            }
    }
}

/** Ce qu'un résultat gardé (`AutoResult`) dit à l'app ; le reste se lit par le cœur ([Native.autotestChoices], [Native.autotestSettings]). */
data class AutoSummary(
    /** Date du test, en secondes depuis 1970. */
    val at: Long,
    /** Le réseau du test : « Wi-Fi », « Ethernet », le nom du Wi-Fi, `relay` ; nul : inconnu. */
    val network: String?,
    /** La liaison mesurée : `cable`, `wifi`, `relay`. */
    val link: String,
    val codec: String,
    val width: Int,
    val height: Int,
    val refresh: Int,
    val reliable: Boolean,
) {
    companion object {
        fun parse(json: String?): AutoSummary? = json?.let { s ->
            runCatching {
                val o = JSONObject(s)
                AutoSummary(
                    at = o.optLong("at"),
                    network = if (o.isNull("network")) null else o.optString("network").takeIf { it.isNotEmpty() },
                    link = o.optJSONObject("measures")?.optString("link") ?: "",
                    codec = o.optString("codec"),
                    width = o.optInt("width"),
                    height = o.optInt("height"),
                    refresh = o.optInt("refresh"),
                    reliable = o.optBoolean("reliable"),
                )
            }.getOrNull()
        }
    }
}

object AutoProfile {
    /** Le réseau du relais, comme le cœur l'écrit dans le résultat. */
    const val RELAY = "relay"

    /** Le transport choisi par le test ; nul sans résultat lisible. */
    fun reliable(result: String?): Boolean? = AutoSummary.parse(result)?.reliable

    /**
     * Le nom du réseau du moment, gardé avec le résultat et comparé au début de chaque session : le nom du Wi-Fi
     * seulement si l'app a déjà le droit de le lire (localisation précise : elle ne le demande jamais pour cela), sinon
     * « Wi-Fi » ou « Ethernet » d'après la liaison ; nul : aucun réseau connu.
     */
    fun networkName(context: Context): String? = runCatching {
        val cm = context.getSystemService(ConnectivityManager::class.java)
        val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return null
        when {
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> "Ethernet"
            caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> wifiName(context, caps) ?: "Wi-Fi"
            else -> null
        }
    }.getOrNull()

    private fun wifiName(context: Context, caps: NetworkCapabilities): String? {
        if (context.checkSelfPermission(Manifest.permission.ACCESS_FINE_LOCATION) != PackageManager.PERMISSION_GRANTED) return null
        val ssid = (caps.transportInfo as? WifiInfo)?.ssid ?: return null
        return ssidName(ssid)
    }

    /** Le nom d'un SSID tel qu'Android le donne (entre guillemets) ; nul s'il est caché (`<unknown ssid>`) ou vide. */
    fun ssidName(ssid: String): String? {
        val s = ssid.trim()
        if (s.isEmpty() || s == WifiManagerUnknown) return null
        return (if (s.length >= 2 && s.startsWith("\"") && s.endsWith("\"")) s.substring(1, s.length - 1) else s).takeIf { it.isNotBlank() }
    }

    private const val WifiManagerUnknown = "<unknown ssid>"

    /**
     * L'appareil pour le test (le JSON de [Native.autotestStart]) : l'écran de la session (« comme cet écran », à la
     * plus haute fréquence qu'il tient), ce que l'appareil décode ([caps], `{hevc, hevc444, av1, hdr, pyrowave}`), le
     * HDR qui servira, le réseau du moment et le câble.
     */
    fun deviceJson(screen: Screen, caps: JSONObject, hdr: Boolean, network: String?, wired: Boolean): String {
        val (w, h, r) = screenMode(screen)
        return JSONObject()
            .put("screen", JSONObject().put("width", w).put("height", h).put("refresh", r))
            .put("caps", caps)
            .put("hdr", hdr)
            .put("network", network ?: JSONObject.NULL)
            .put("wired", wired)
            .toString()
    }

    /** L'écran de l'appareil en paysage, à sa plus haute fréquence : celui que la session prend « comme cet écran ». */
    fun screenMode(screen: Screen): Triple<Int, Int, Int> {
        val w = max(screen.current.width, screen.current.height)
        val h = min(screen.current.width, screen.current.height)
        return Triple(w, h, screen.bestRefresh(w, h))
    }

    /**
     * « Appliquer » : Auto devient le profil de cet ordinateur avec ce résultat (format, débit et transport : ceux du
     * profil, donc du test), et la définition et la fréquence choisies passent dans « Définition » et « Fréquence » :
     * « Comme cet écran » et « Auto » quand ce sont celles de l'écran, sinon la hauteur et la fréquence du test.
     */
    fun apply(set: HostSettings, result: String, screen: Screen): HostSettings {
        val r = AutoSummary.parse(result) ?: return set.apply(Profile.AUTO)
        val (w, h, _) = screenMode(screen)
        val sameSize = r.width == w && r.height == h
        val resolution = if (sameSize || r.height <= 0) 0 else r.height
        val best = screen.bestRefresh(if (sameSize) w else r.width, if (sameSize) h else r.height)
        val refresh = if (r.refresh <= 0 || r.refresh == best) 0 else r.refresh
        return set.apply(Profile.AUTO).copy(autoResult = result, resolution = resolution, refresh = refresh, adaptive = true)
    }

    /**
     * « Appliquer » à la fin d'un test : [apply], et le plafond de l'encodeur gardé pour cet ordinateur d'après ce
     * nouveau résultat (`AutoResult::next_encoder_max` : la limite trouvée ; un plafond gardé sous le débit accepté du
     * premier coup est retiré ; sinon celui d'avant). Remettre Auto avec le résultat déjà gardé passe par [apply] seul :
     * un plafond trouvé par une session après le test est plus récent.
     */
    fun applyTest(set: HostSettings, result: String, screen: Screen): HostSettings =
        apply(set, result, screen).copy(encoderMax = nextEncoderMax(result, set.encoderMax))

    /** Le plafond à garder (`AutoResult::next_encoder_max`) ; sans bibliothèque native, celui d'avant. */
    fun nextEncoderMax(result: String, previous: Int?): Int? =
        runCatching { Native.autotestEncoderMax(result, previous ?: 0) }.getOrElse { previous ?: 0 }.takeIf { it > 0 }

    /**
     * Les réglages de session d'Auto (le JSON de `sessionStart`) : ceux du test ([Native.autotestSettings]) ; sans
     * résultat lisible (Auto reçu d'un autre appareil), ceux que le cœur donne au profil `auto` ([fallback]).
     */
    fun settings(result: String?, hdr: Boolean, wired: Boolean, fallback: () -> JSONObject): JSONObject =
        result?.let { r -> runCatching { Native.autotestSettings(r, hdr, wired)?.let(::JSONObject) }.getOrNull() } ?: fallback()

    /** Le format qu'Auto demandera (`hevc`, `av1`, `h264`, `pyrowave`) ; nul sans résultat. */
    fun codec(result: String?): String? = AutoSummary.parse(result)?.codec?.takeIf { it.isNotEmpty() }

    /** Le résumé d'un résultat gardé, par le cœur ; vide sans bibliothèque native ou s'il est illisible. */
    fun choices(result: String): List<AutoChoice> =
        runCatching { Native.autotestChoices(result)?.let { AutoChoice.parseList(JSONArray(it)) } }.getOrNull() ?: emptyList()

    /**
     * Le test a-t-il été fait sur ce réseau ? [network] : celui du moment, [RELAY] par le relais. Sans bibliothèque
     * native, la même règle que le cœur (`AutoResult::same_network`).
     */
    fun sameNetwork(result: String, network: String?): Boolean =
        runCatching { Native.autotestSameNetwork(result, network) }.getOrElse { AutoSummary.parse(result)?.network == network }

    /** La date du test pour « Choisi par le test du {date} ({réseau}). » : « 26 nov. 2026 ». */
    fun date(at: Long, locale: Locale): String = DateFormat.getDateInstance(DateFormat.MEDIUM, locale).format(Date(at * 1000))
}
