package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.util.Log
import fr.leslignesclaires.kybernow.core.Native
import org.json.JSONObject

/**
 * PyroWave (docs/pyrowave.md, docs/pyrowave-client-android.md ; règles et textes : docs/link-ui.md, « PyroWave ») :
 * ce que l'appareil en sait.
 *
 * - **Décodeur** : la sonde du lancement ([probe], hors du fil principal, comme le Mac) crée le device Vulkan, un
 *   décodeur et décode une image de test ; `pyrowave` part au cœur dans les capacités ([Hdr.capabilities]).
 * - **Câble** : [wired], le réseau actif est en Ethernet (`NetworkCapabilities.TRANSPORT_ETHERNET`) ; `wired` part au cœur
 *   dans les réglages de la session, pour la règle d'« Auto ».
 */
object PyroWave {
    /** La sonde a décodé l'image de test (faux avant la fin de la sonde, et sans bibliothèque native). */
    @Volatile var decodes: Boolean = false
        private set

    /** Ce que la sonde a dit : le GPU et le chemin (« Mali-G57, fragment »), ou pourquoi PyroWave est indisponible. */
    @Volatile var report: String = ""
        private set

    /** La sonde du lancement, une fois, sur un fil d'arrière-plan. */
    fun probe() {
        val json = runCatching { JSONObject(Native.pyrowaveProbe()) }.getOrNull() ?: return
        decodes = json.optBoolean("available")
        report = json.optString("report")
        Log.i("KyberNow", "PyroWave : ${if (decodes) "disponible" else "indisponible"} ($report)")
    }

    /** L'appareil est relié par câble (Ethernet) au réseau, en ce moment. */
    fun wired(context: Context): Boolean = runCatching {
        val cm = context.getSystemService(ConnectivityManager::class.java)
        cm.getNetworkCapabilities(cm.activeNetwork)?.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) == true
    }.getOrDefault(false)

    /**
     * Le débit du lien de l'appareil vers le réseau, en Mbit/s, pour la définition ramenée du cœur
     * (`Session::set_link_mbps`, docs/pyrowave.md) ; 0 : inconnu. En Wi-Fi, le débit de réception négocié
     * (`WifiInfo.getRxLinkSpeedMbps`) ; en Ethernet, Android ne donne qu'une estimation fixe
     * (`getLinkDownstreamBandwidthKbps`), pas la vitesse de la prise : inconnu.
     */
    fun linkMbps(context: Context): Int = runCatching {
        val cm = context.getSystemService(ConnectivityManager::class.java)
        val caps = cm.getNetworkCapabilities(cm.activeNetwork) ?: return 0
        if (!caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) return 0
        val info = caps.transportInfo as? android.net.wifi.WifiInfo
            ?: @Suppress("DEPRECATION") context.getSystemService(android.net.wifi.WifiManager::class.java).connectionInfo
        val rx = info.rxLinkSpeedMbps
        (if (rx > 0) rx else info.linkSpeed).coerceAtLeast(0)
    }.getOrDefault(0)

    /**
     * Ce que la Station annonce (`pyrowave` de `GET /api/stream/info`) : vrai ou faux selon `available`, nul quand le
     * champ manque (Station d'avant PyroWave : la valeur n'apparaît jamais).
     */
    fun offered(info: JSONObject): Boolean? = info.optJSONObject("pyrowave")?.optBoolean("available")
}

/** Pourquoi la valeur PyroWave de « Format vidéo » est inactive. */
enum class PyroWaveBlock { NOT_OFFERED, NO_DECODER }

/** Par où la session passerait : ce qui décide de l'avertissement sous le choix et du bandeau. */
enum class PyroWaveLink { CABLE, WIFI, RELAY }

/**
 * « PyroWave » dans « Format vidéo » pour un ordinateur (docs/link-ui.md, « PyroWave ») : montré seulement quand la
 * Station l'annonce ([Host.pyrowave] non nul) ; inactif avec sa raison quand elle ne le propose pas ou que l'appareil
 * ne le décode pas ; [link] : le câble, le Wi-Fi, ou le relais (ordinateur joint hors de chez soi).
 */
data class PyroWaveChoice(val shown: Boolean, val block: PyroWaveBlock?, val link: PyroWaveLink) {
    /** La valeur peut se choisir, et « Auto » peut la prendre. */
    val usable: Boolean get() = shown && block == null

    companion object {
        val HIDDEN = PyroWaveChoice(false, null, PyroWaveLink.WIFI)

        fun of(host: Host, decodes: Boolean, wired: Boolean, relayed: Boolean): PyroWaveChoice {
            val offered = host.pyrowave ?: return HIDDEN
            val block = when {
                !offered -> PyroWaveBlock.NOT_OFFERED
                !decodes -> PyroWaveBlock.NO_DECODER
                else -> null
            }
            val link = when {
                relayed -> PyroWaveLink.RELAY
                wired -> PyroWaveLink.CABLE
                else -> PyroWaveLink.WIFI
            }
            return PyroWaveChoice(true, block, link)
        }
    }
}
