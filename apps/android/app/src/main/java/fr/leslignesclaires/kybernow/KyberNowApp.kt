package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.appcompat.app.AppCompatDelegate
import fr.leslignesclaires.kybernow.core.DeviceIdentity
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Discovery
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.HostStore
import java.util.Locale

/** Ce que toute l'app partage : ordinateurs, manettes, découverte, décodeurs. */
class KyberNowApp : Application() {
    lateinit var hosts: HostStore
        private set
    lateinit var gamepads: Gamepads
        private set
    lateinit var discovery: Discovery
        private set
    lateinit var device: DeviceIdentity
        private set
    lateinit var form: FormFactor
        private set
    lateinit var updates: fr.leslignesclaires.kybernow.update.Updates
        private set

    /** Décodeurs matériels, vérifiés une fois au lancement. */
    val decoders: Decoders by lazy { Decoders.detect() }

    override fun onCreate() {
        super.onCreate()
        instance = this
        hosts = HostStore(this)
        gamepads = Gamepads(this)
        discovery = Discovery(this)
        device = DeviceIdentity.of(this)
        form = DeviceIdentity.formFactor(this)
        Native.init(language())
        updates = fr.leslignesclaires.kybernow.update.Updates(this).also { it.start() }
    }

    /** `fr` ou `en` : réglage de l'app (Auto / Français / English), sinon le système. */
    fun language(): String {
        val app = AppCompatDelegate.getApplicationLocales()
        val tag = if (!app.isEmpty) app[0]!!.language else Locale.getDefault().language
        return if (tag.startsWith("fr")) "fr" else "en"
    }

    companion object {
        lateinit var instance: KyberNowApp
            private set
    }
}
