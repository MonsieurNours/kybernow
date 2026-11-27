package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.net.ConnectivityManager
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import fr.leslignesclaires.kybernow.core.Core
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.net.Inet4Address
import java.net.InetAddress
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import javax.jmdns.JmDNS

/** Un ordinateur annoncé sur le réseau par Admin (`_kybernow._tcp`, TXT `v`, `name`, `fp`, `kyber`). */
data class Announced(val serviceName: String, val name: String, val address: String, val port: Int, val fingerprint: String?)

/**
 * Découverte des ordinateurs du réseau local (mDNS, comme Bonjour sur les apps
 * Apple). Un Admin sans serveur Kyber derrière (`kyber=0`) est écarté.
 *
 * Adresse : l'IPv4 d'abord. Avant Android 14, NsdManager ne rend qu'une
 * adresse, parfois l'IPv6 locale au lien (`fe80::…`), inutilisable sans le
 * nom de l'interface : l'IPv4 est alors redemandée par JmDNS, puis
 * directement pour le nom de l'ordinateur (`MdnsLookup`). Faute d'IPv4, une
 * IPv6 routable convient ; une IPv6 locale au lien, jamais.
 */
class Discovery(private val context: Context) {
    private val nsd = context.getSystemService(NsdManager::class.java)
    private var lock: Multicast.Hold? = null
    private val main = Handler(Looper.getMainLooper())
    private val resolver = Executors.newSingleThreadExecutor()
    private val _found = MutableStateFlow<List<Announced>>(emptyList())
    val found: StateFlow<List<Announced>> = _found.asStateFlow()
    private var listener: NsdManager.DiscoveryListener? = null
    private var jmdns: JmDNS? = null

    /** Chaque recherche a son numéro : une résolution finie après l'arrêt n'ajoute plus rien (AND-20). */
    @Volatile private var generation = 0

    fun start() {
        if (listener != null) return
        generation++
        lock = Multicast.acquire(context)
        val l = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(serviceType: String) {}
            override fun onDiscoveryStopped(serviceType: String) {}
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {}
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
            override fun onServiceFound(info: NsdServiceInfo) = resolve(info, generation)
            override fun onServiceLost(info: NsdServiceInfo) {
                main.post { _found.value = _found.value.filterNot { it.serviceName == info.serviceName } }
            }
        }
        listener = l
        runCatching { nsd.discoverServices(TYPE, NsdManager.PROTOCOL_DNS_SD, l) }
    }

    fun stop() {
        listener?.let { runCatching { nsd.stopServiceDiscovery(it) } }
        listener = null
        // De retour sur un autre réseau, « Sur le réseau » ne montre plus les ordinateurs de l'ancien (AND-20).
        generation++
        _found.value = emptyList()
        lock?.release()
        lock = null
        resolver.execute {
            runCatching { jmdns?.close() }
            jmdns = null
        }
    }

    // Une résolution à la fois (NsdManager refuse les résolutions simultanées avant Android 14).
    @Suppress("DEPRECATION")
    private fun resolve(info: NsdServiceInfo, gen: Int) {
        resolver.execute {
            val done = CountDownLatch(1)
            var resolved: NsdServiceInfo? = null
            nsd.resolveService(info, object : NsdManager.ResolveListener {
                override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) = done.countDown()
                override fun onServiceResolved(s: NsdServiceInfo) {
                    resolved = s
                    done.countDown()
                }
            })
            done.await(5, TimeUnit.SECONDS)
            val s = resolved ?: return@execute
            val txt = s.attributes.mapValues { (_, v) -> v?.toString(Charsets.UTF_8) }
            // Les adresses que donne Android, puis celles qu'on va chercher (JmDNS, le nom) ; le cœur choisit
            // (`discovery::found` : une IPv4 d'abord), lit le nom et met l'empreinte sous sa forme normale.
            val addresses = addresses(s).ifEmpty {
                listOfNotNull(txt["name"]?.let { n -> runCatching { MdnsLookup.ipv4(n, localIpv4()) }.getOrNull() })
            }
            val found = Core.discoveryFound("${s.serviceName}.$TYPE.local.", s.port, addresses, txt) ?: run {
                if (txt["kyber"] != "0") Log.w("KyberNow", "découverte : annonce de ${s.serviceName} sans adresse utilisable")
                return@execute
            }
            val a = Announced(
                serviceName = s.serviceName,
                name = found.optString("name").ifBlank { s.serviceName },
                address = found.optString("host"),
                port = found.optInt("port", s.port),
                fingerprint = if (found.isNull("fingerprint")) null else found.optString("fingerprint").takeIf { it.isNotBlank() },
            )
            main.post { if (gen == generation) _found.value = _found.value.filterNot { it.serviceName == a.serviceName } + a }
        }
    }

    /** Les adresses de l'annonce, plus l'IPv4 que donne JmDNS quand Android n'en donne aucune. */
    @Suppress("DEPRECATION")
    private fun addresses(s: NsdServiceInfo): List<String> {
        val all: List<InetAddress> = if (Build.VERSION.SDK_INT >= 34) s.hostAddresses else listOfNotNull(s.host)
        val jmdns = if (all.none { it is Inet4Address }) ipv4ByJmdns(s.serviceName) else null
        return all.mapNotNull { it.hostAddress?.substringBefore('%') } + listOfNotNull(jmdns)
    }

    /** L'IPv4 du service, demandée par JmDNS sur l'interface du réseau actif (fil du résolveur). */
    private fun ipv4ByJmdns(serviceName: String): String? = runCatching {
        val dns = jmdns ?: JmDNS.create(localIpv4()).also { jmdns = it }
        dns.getServiceInfo("$TYPE.local.", serviceName, 3000)?.inet4Addresses?.firstOrNull()?.hostAddress
    }.onFailure { Log.w("KyberNow", "découverte : JmDNS ($it)") }.getOrNull()

    private fun localIpv4(): InetAddress? {
        val cm = context.getSystemService(ConnectivityManager::class.java) ?: return null
        val lp = cm.getLinkProperties(cm.activeNetwork) ?: return null
        return lp.linkAddresses.map { it.address }.firstOrNull { it is Inet4Address }
    }

    private companion object {
        const val TYPE = "_kybernow._tcp"
    }
}
