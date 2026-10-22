package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.Looper
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.util.concurrent.Executors

/** Un ordinateur annoncé sur le réseau par Admin (`_kybernow._tcp`, TXT `v`, `name`, `fp`, `kyber`). */
data class Announced(val serviceName: String, val name: String, val address: String, val port: Int, val fingerprint: String?)

/**
 * Découverte des ordinateurs du réseau local (mDNS, comme Bonjour sur les apps
 * Apple). Un Admin sans serveur Kyber derrière (`kyber=0`) est écarté.
 */
class Discovery(context: Context) {
    private val nsd = context.getSystemService(NsdManager::class.java)
    private val wifi = context.applicationContext.getSystemService(WifiManager::class.java)
    private var lock: WifiManager.MulticastLock? = null
    private val main = Handler(Looper.getMainLooper())
    private val resolver = Executors.newSingleThreadExecutor()
    private val _found = MutableStateFlow<List<Announced>>(emptyList())
    val found: StateFlow<List<Announced>> = _found.asStateFlow()
    private var listener: NsdManager.DiscoveryListener? = null

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
    }

    // Une résolution à la fois (NsdManager refuse les résolutions simultanées avant Android 14).
    @Suppress("DEPRECATION")
    private fun resolve(info: NsdServiceInfo) {
        resolver.execute {
            val done = java.util.concurrent.CountDownLatch(1)
            nsd.resolveService(info, object : NsdManager.ResolveListener {
                override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) = done.countDown()
                override fun onServiceResolved(s: NsdServiceInfo) {
                    val txt = s.attributes.mapValues { (_, v) -> v?.toString(Charsets.UTF_8) }
                    if (txt["kyber"] != "0") {
                        val a = Announced(
                            serviceName = s.serviceName,
                            name = txt["name"]?.takeIf { it.isNotBlank() } ?: s.serviceName,
                            address = s.host?.hostAddress ?: "",
                            port = s.port,
                            fingerprint = txt["fp"]?.takeIf { it.isNotBlank() },
                        )
                        main.post { _found.value = _found.value.filterNot { it.serviceName == a.serviceName } + a }
                    }
                    done.countDown()
                }
            })
            done.await(5, java.util.concurrent.TimeUnit.SECONDS)
        }
    }

    private companion object {
        const val TYPE = "_kybernow._tcp"
    }
}
