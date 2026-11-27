package fr.leslignesclaires.kybernow.session

import android.content.Context
import android.net.wifi.WifiManager
import android.util.Log

/**
 * Le Wi-Fi gardé éveillé et en faible latence pendant une session (AND-07) : sans lui, l'économie d'énergie du Wi-Fi
 * regroupe les réceptions (pics de latence, rafales de pertes, images clés redemandées, puis relances sans image et
 * session coupée). `FLAG_KEEP_SCREEN_ON` garde l'écran, pas la radio. Pris au premier plan (`onResume`), rendu en
 * arrière-plan (`onPause`) et à la fin ; Android ne l'applique qu'à l'app au premier plan, écran allumé.
 */
class LowLatencyWifi(context: Context) {
    private val lock: WifiManager.WifiLock? = runCatching {
        context.applicationContext.getSystemService(WifiManager::class.java)
            ?.createWifiLock(WifiManager.WIFI_MODE_FULL_LOW_LATENCY, "kybernow:session")
            ?.apply { setReferenceCounted(false) }
    }.getOrNull()

    fun acquire() {
        runCatching { lock?.takeIf { !it.isHeld }?.acquire() }.onFailure { Log.w("KyberNow", "Wi-Fi faible latence refusé : ${it.message}") }
    }

    fun release() {
        runCatching { lock?.takeIf { it.isHeld }?.release() }
    }
}
