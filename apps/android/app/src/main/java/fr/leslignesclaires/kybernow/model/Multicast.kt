package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.net.wifi.WifiManager
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Le verrou de multidiffusion du Wi-Fi (sans lui, le Wi-Fi d'Android jette les paquets de groupe et de diffusion) : un
 * seul pour l'app, tenu tant qu'une recherche au moins en a besoin (ordinateurs sur le réseau, consoles PlayStation,
 * « Recevoir d'un autre appareil »). Chacune prend un [Hold] et le rend ; le dernier rendu lâche le verrou (AND-30).
 */
object Multicast {
    private var lock: WifiManager.MulticastLock? = null
    private var holders = 0

    /** Une prise du verrou ; [release] la rend (une seule fois, même appelée de nouveau). */
    class Hold internal constructor() {
        private val released = AtomicBoolean(false)

        fun release() {
            if (released.compareAndSet(false, true)) Multicast.dropOne()
        }
    }

    @Synchronized
    fun acquire(context: Context): Hold {
        holders++
        if (lock == null) {
            lock = runCatching {
                context.applicationContext.getSystemService(WifiManager::class.java)
                    ?.createMulticastLock("kybernow")?.apply { setReferenceCounted(false); acquire() }
            }.getOrNull()
        }
        return Hold()
    }

    @Synchronized
    private fun dropOne() {
        holders = (holders - 1).coerceAtLeast(0)
        if (holders > 0) return
        runCatching { lock?.release() }
        lock = null
    }

    /** Le verrou est tenu (pour les essais). */
    internal val held: Boolean @Synchronized get() = holders > 0
}
