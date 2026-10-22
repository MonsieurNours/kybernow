package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.net.ConnectivityManager
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.net.Inet4Address
import java.net.Inet6Address
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
    private val wifi = context.applicationContext.getSystemService(WifiManager::class.java)
    private var lock: WifiManager.MulticastLock? = null
    private val main = Handler(Looper.getMainLooper())
    private val resolver = Executors.newSingleThreadExecutor()
    private val _found = MutableStateFlow<List<Announced>>(emptyList())
    val found: StateFlow<List<Announced>> = _found.asStateFlow()
    private var listener: NsdManager.DiscoveryListener? = null
    private var jmdns: JmDNS? = null

    fun start() {
        if (listener != null) return
        lock = wifi?.createMulticastLock("kybernow")?.apply { setReferenceCounted(false); acquire() }
        val l = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(serviceType: String) {}
            override fun onDiscoveryStopped(serviceType: String) {}
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {}
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
            override fun onServiceFound(info: NsdServiceInfo) = resolve(info)
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
        lock?.release()
        lock = null
        resolver.execute {
            runCatching { jmdns?.close() }
            jmdns = null
        }
    }

    // Une résolution à la fois (NsdManager refuse les résolutions simultanées avant Android 14).
    @Suppress("DEPRECATION")
    private fun resolve(info: NsdServiceInfo) {
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
            if (txt["kyber"] == "0") return@execute
            val address = address(s) ?: txt["name"]?.let { n -> runCatching { MdnsLookup.ipv4(n, localIpv4()) }.getOrNull() } ?: run {
                Log.w("KyberNow", "découverte : aucune adresse utilisable pour ${s.serviceName}")
                return@execute
            }
            val a = Announced(
                serviceName = s.serviceName,
                name = txt["name"]?.takeIf { it.isNotBlank() } ?: s.serviceName,
                address = address,
                port = s.port,
                fingerprint = txt["fp"]?.takeIf { it.isNotBlank() },
            )
            main.post { _found.value = _found.value.filterNot { it.serviceName == a.serviceName } + a }
        }
    }

    @Suppress("DEPRECATION")
    private fun address(s: NsdServiceInfo): String? {
        val all: List<InetAddress> = if (Build.VERSION.SDK_INT >= 34) s.hostAddresses else listOfNotNull(s.host)
        all.firstOrNull { it is Inet4Address }?.let { return it.hostAddress }
        ipv4ByJmdns(s.serviceName)?.let { return it }
        return all.firstOrNull { it is Inet6Address && !it.isLinkLocalAddress }?.hostAddress
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
