package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.appcompat.app.AppCompatDelegate
import fr.leslignesclaires.kybernow.core.BorrowAsk
import fr.leslignesclaires.kybernow.core.DeviceIdentity
import fr.leslignesclaires.kybernow.core.Loan
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.core.ShareFound
import fr.leslignesclaires.kybernow.core.ShareQuality
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

    /**
     * KyberNow Share : la Station qui vient d'accepter l'invité (jetons compris), remise à
     * `SessionActivity` qui ouvre sa session. Seulement en mémoire : un invité n'écrit rien dans
     * l'appareil (ni liste d'ordinateurs, ni Intent, ni disque), et la session ne reprend pas
     * après la mort du processus.
     */
    @Volatile private var pendingGuest: ShareFound? = null

    fun handOverGuest(found: ShareFound) {
        pendingGuest = found
    }

    /** Prend l'invité remis par [handOverGuest] (une seule fois). */
    fun takeGuest(): ShareFound? = pendingGuest.also { pendingGuest = null }

    /**
     * Prêt de l'ordinateur : le prêt accordé (secret et jeton compris) et la demande qui l'a obtenu (pour « Revenir »
     * pendant 15 min), remis à `SessionActivity` qui ouvre la session d'emprunt. Seulement en mémoire, comme un invité.
     */
    @Volatile private var pendingLoan: Pair<Loan, BorrowAsk>? = null

    fun handOverLoan(loan: Loan, ask: BorrowAsk) {
        pendingLoan = loan to ask
    }

    /** Prend le prêt remis par [handOverLoan] (une seule fois). */
    fun takeLoan(): Pair<Loan, BorrowAsk>? = pendingLoan.also { pendingLoan = null }

    /** L'avertissement du prêt (« Votre ami joue dans votre session Windows… ») a été montré une fois dans le panneau. */
    var lendWarned: Boolean
        get() = getSharedPreferences("share", MODE_PRIVATE).getBoolean("lend_warned", false)
        set(v) = getSharedPreferences("share", MODE_PRIVATE).edit().putBoolean("lend_warned", v).apply()

    /** Le prénom d'invité retenu pour la fois suivante ; au début, le nom de l'appareil. */
    var guestName: String
        get() = getSharedPreferences("share", MODE_PRIVATE).getString("name", null)?.takeIf { it.isNotBlank() } ?: device.name
        set(v) = getSharedPreferences("share", MODE_PRIVATE).edit().putString("name", v.trim()).apply()

    /**
     * La « Qualité de l'image » d'invité retenue pour la fois suivante (« Rejoindre » et la barre d'un invité) ;
     * au début, « Auto ».
     */
    var guestQuality: ShareQuality
        get() = ShareQuality.parse(getSharedPreferences("share", MODE_PRIVATE).getString("quality", null))
        set(v) = getSharedPreferences("share", MODE_PRIVATE).edit().putString("quality", v.id).apply()

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
