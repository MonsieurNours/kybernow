package fr.leslignesclaires.kybernow.session

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.LendEnd
import fr.leslignesclaires.kybernow.core.LendNotice
import fr.leslignesclaires.kybernow.core.LendNoticeReason
import fr.leslignesclaires.kybernow.core.LendView
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.core.RelayFailure
import fr.leslignesclaires.kybernow.core.RouteInfo
import fr.leslignesclaires.kybernow.core.SessionListener
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareImage
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.model.Gamepads
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import java.util.concurrent.atomic.AtomicLong

/** Étapes de la session, dans le vocabulaire du cœur. */
enum class Phase { PREPARING, CONNECTING, FIRST_IMAGES, STREAMING, RECONNECTING, ENDING, ENDED }

data class Ended(val error: String?, val displayRestored: Boolean?, val audioRestored: Boolean?, val reportError: String?)

/** Mesures de la dernière seconde (barre, menu, panneau des mesures). */
data class Second(
    val latencyMs: Double?,
    val rttMs: Double?,
    val fps: Int,
    val lostFrames: Int,
    val holes: Int,
    val lostPackets: Int,
    val mbps: Double,
    /** Capture → encodage, encodage, envoi, réseau, décodage, affichage (ms). */
    val chain: DoubleArray?,
)

/**
 * L'avis d'un changement de droits d'un invité (docs/link-ui.md, « Droits changés »), `R.string.share_notice_…` ;
 * `null` si rien n'a changé. Les deux droits (les manettes, le clavier et la souris) sont indépendants : « Vous
 * pouvez jouer à la manette. », « Les manettes sont reprises. », « Vous avez le clavier et la souris. », « Le
 * clavier et la souris sont repris. » ; plus aucun droit : « Vous regardez de nouveau. » à la place de l'avis de
 * reprise. Une capsule n'a qu'un texte : quand les deux droits changent d'un coup, un droit donné passe avant un
 * droit repris (les manettes remplacées par le clavier et la souris : « Vous avez le clavier et la souris. »), et
 * les manettes avant le clavier et la souris.
 */
@androidx.annotation.StringRes
internal fun shareRightsNotice(before: ShareRole, now: ShareRole): Int? = when {
    now == before -> null
    // Un rôle sans droit n'est que le spectateur : l'invité en avait au moins un.
    !now.plays -> R.string.share_notice_watch
    now.pads && !before.pads -> R.string.share_notice_play
    now.keyboardMouse && !before.keyboardMouse -> R.string.share_notice_keyboard_mouse
    before.pads && !now.pads -> R.string.share_notice_pads_back
    before.keyboardMouse && !now.keyboardMouse -> R.string.share_notice_keyboard_mouse_back
    else -> null
}

/**
 * Le texte de link-ui (« Hors de chez vous ») d'un refus du relais, avec le nom de l'ordinateur : « {nom} n'est pas
 * joignable hors de chez vous (accès coupé dans Admin). » (32), « {nom} n'est pas joignable : l'ordinateur est éteint ou
 * en veille. » (31). Nul pour le relais muet (30) : le texte du cœur (« Le relais ne répond pas. ») est gardé.
 */
internal fun relayText(context: Context, failure: RelayFailure, name: String): String? = when (failure) {
    RelayFailure.ACCESS_OFF -> context.getString(R.string.relay_access_off, name)
    RelayFailure.STATION_OFFLINE -> context.getString(R.string.relay_station_offline, name)
    RelayFailure.UNREACHABLE -> null
}

/** Une capsule en haut de l'image (jamais une alerte). */
sealed class Capsule {
    data class Reconnecting(val attempt: Int, val max: Int) : Capsule()
    data object Relaunched : Capsule()
    data class ModeChanged(val width: Int, val height: Int, val back: Boolean) : Capsule()
    data class Fallback(val level: Int, val mbps: Int) : Capsule()
    data class Adapting(val width: Int, val height: Int) : Capsule()
    data class AdaptFailed(val error: String) : Capsule()
    data class GamepadConnected(val name: String, val slot: Int, val motors: Int) : Capsule()
    /** Invité : ses droits ont changé (« Vous pouvez jouer à la manette. ») ; [text] : le texte, `R.string.share_notice_…`. */
    data class ShareRights(@androidx.annotation.StringRes val text: Int) : Capsule()
    /** Invité : la Station a ajusté son image (« Image allégée pour suivre. ») ; [text] : l'avis tel que le cœur l'écrit. */
    data class ShareNotice(val text: String) : Capsule()
    /** Hôte : un invité demande à rejoindre par le relais (« Léa demande à rejoindre »), à accepter ou à refuser dans le panneau. */
    data class ShareRequest(val name: String) : Capsule()
    /** Emprunt : le préavis de 2 min (« Fin du prêt dans 2 min : sauvegardez votre partie. ») ; [text] : celui du cœur. */
    data class LendNotice(val text: String) : Capsule()
    /**
     * Hôte : un ami veut emprunter l'ordinateur (« Léa veut emprunter l'ordinateur. »), à accepter ou à refuser dans le
     * panneau du prêt ; [slot] : la demande vient de son créneau (« Créneau de Léa : l'ordinateur lui est prêté dans 2 min. »).
     */
    data class LendRequest(val name: String, val slot: Boolean = false) : Capsule()
    /** Hôte : un avis du prêt écrit par le cœur (« Créneau de Léa : l'ordinateur lui est prêté dans 2 min. »). */
    data class LendHostNotice(val text: String) : Capsule()
}

data class DisplayInfo(val id: Int, val width: Int, val height: Int)

data class PointerState(val visible: Boolean, val x: Double, val y: Double, val remoteHidden: Boolean, val shownAt: Long)

/**
 * Ce que le cœur dit de la session, rangé pour l'interface. Les rappels
 * arrivent des fils du cœur : tout passe par des `StateFlow` (lecture sûre
 * de tout fil) ou est remonté sur le fil principal.
 */
class SessionState(private val context: Context, private val gamepads: Gamepads) : SessionListener {
    private val main = Handler(Looper.getMainLooper())

    val phase = MutableStateFlow(Phase.PREPARING)
    /**
     * Dernier recours de la connexion : le cœur redémarre la Station (l'image ne venait pas) ;
     * l'écran de connexion le dit jusqu'au prochain changement d'étape.
     */
    val stationRestarting = MutableStateFlow(false)
    val ended = MutableStateFlow<Ended?>(null)
    val second = MutableStateFlow<Second?>(null)
    val capsule = MutableStateFlow<Capsule?>(null)
    val restoreOffer = MutableStateFlow<Pair<Int, Int>?>(null)
    val display = MutableStateFlow<DisplayInfo?>(null)
    val displays = MutableStateFlow<List<DisplayInfo>>(emptyList())
    val mode = MutableStateFlow<Triple<Int, Int, Int>?>(null)
    /** La session passe par un écran virtuel : pas de choix d'écran (vrai tant que le cœur ne l'a pas dit). */
    val virtualDisplay = MutableStateFlow(true)
    val videoSize = MutableStateFlow<Pair<Int, Int>?>(null)
    val decoder = MutableStateFlow<String?>(null)
    val audioOutput = MutableStateFlow<Pair<Int, Double>?>(null)
    val gameFullscreen = MutableStateFlow(false)
    val pointer = MutableStateFlow(PointerState(false, 0.0, 0.0, false, 0))
    val pointerShape = MutableStateFlow<Pair<Bitmap, Pair<Int, Int>>?>(null)
    /** La combinaison s'est formée : ouvrir le menu. */
    var onCombo: (() -> Unit)? = null
    /** Les entrées ont rejoint l'ordinateur : rebrancher les manettes. */
    var onInputConnected: (() -> Unit)? = null
    /** Vibrations du jeu pour la manette à l'écran (gros moteur, petit, durée) : depuis un fil du cœur. */
    @Volatile var onScreenRumble: ((Int, Int, Int) -> Unit)? = null
    /** Premier instant d'image, pour le passage et la durée de la session. */
    val firstImageAt = AtomicLong(0)

    // KyberNow Share (docs/kybernow-share.md).
    /** Hôte : le partage ouvert et ses invités (le signe de vie de la session le tient à jour) ; nul sans partage. */
    val share = MutableStateFlow<ShareState?>(null)
    /** Invité : ce que son rôle lui permet, tel que la Station le dit (au départ, puis à chaque changement). */
    val shareRole = MutableStateFlow(ShareRole.SPECTATOR)
    /**
     * Par où passe la session (docs/relais.md) : au début, puis quand le chemin change ; nul avant la première annonce
     * du cœur. [RouteInfo.viaRelay] : la puce « Par le relais » ; [RouteInfo.transport] : la ligne « Chemin » des
     * statistiques et celle de la « Dernière session ».
     */
    val route = MutableStateFlow<RouteInfo?>(null)
    /**
     * Invité : la « Qualité de l'image » qu'il a choisie (celle de « Rejoindre », puis celle de la barre). L'image
     * qu'il reçoit est [shareImage] : la Station tranche, « Auto » aussi.
     */
    val shareQuality = MutableStateFlow(ShareQuality.AUTO)
    /**
     * Invité : l'image reçue (au départ, puis à chaque changement du signe de vie ou de la réponse de qualité) ;
     * nulle tant que la Station n'en a rien dit. Son libellé (« 1080p · 60 Hz ») n'existe que si la taille est
     * connue : sans lui, l'interface ne montre aucune qualité reçue.
     */
    val shareImage = MutableStateFlow<ShareImage?>(null)
    /** Le libellé du cœur d'une image reçue ; les essais sans bibliothèque native en donnent un. */
    internal var imageLabel: (String) -> String? = Share::imageLabel
    /** Le code du partage tel qu'on l'affiche (« 123 456 », le cœur) ; les essais sans bibliothèque native en donnent un. */
    internal var codeFormat: (String) -> String = Share::formatCode
    /**
     * Invité : le texte de la fin du partage (« Léa a arrêté le partage. »), tel que le cœur l'écrit ; il est aussi
     * `Ended.error`, que l'écran de fin montre sans y toucher (le nom de l'hôte n'a pas à changer de casse).
     */
    @Volatile var shareEndText: String? = null

    /**
     * Le nom de l'ordinateur (pour un invité, celui de l'ordinateur du partage), pour les refus du relais : « {nom} n'est
     * pas joignable… » (docs/link-ui.md, « Hors de chez vous »).
     */
    @Volatile var computerName: String = ""
    /**
     * Par le relais, la session a fini faute de joindre l'ordinateur (`onRelayError`, juste avant la fin) : le code du
     * cœur, et son texte. La fin le dit avec le nom de l'ordinateur ([relayEndText]), choisi par le code.
     */
    @Volatile private var relayEnd: Pair<RelayFailure, String>? = null

    // Prêt de l'ordinateur (docs/pret.md).
    /** Hôte : la vue du prêt (le signe de vie de la session la tient à jour) ; nulle tant qu'il n'y a rien à dire. */
    val lend = MutableStateFlow<LendView?>(null)
    /** Emprunt : le préavis de 2 min en cours (« Encore 2 min » dans la barre) ; nul sans préavis. */
    val lendNotice = MutableStateFlow<LendNotice?>(null)
    /**
     * Emprunt : la fin du prêt (`onLendEnd`, juste avant la fin) ; nulle si la session a fini autrement (réseau perdu,
     * Kyber coupé) : l'ami peut alors revenir pendant 15 min (« Revenir »).
     */
    @Volatile var lendEnd: LendEnd? = null
    @Volatile private var lendEndText: String? = null
    /** Le texte d'un préavis chez l'hôte (le cœur) ; les essais sans bibliothèque native en donnent un. */
    internal var lendNoticeText: (LendNotice, String) -> String? = { n, name -> Native.lendNoticeText(n.json().toString(), name) }

    /** Le texte de la fin par le relais (avec le nom de l'ordinateur) ; nul si la session n'a pas fini ainsi. */
    internal fun relayEndText(): String? = relayEnd?.let { (failure, text) -> relayText(context, failure, computerName) ?: text }

    private var capsuleClear: Runnable? = null

    fun show(c: Capsule, ms: Long = 4000) = main.post {
        capsule.value = c
        capsuleClear?.let(main::removeCallbacks)
        if (ms > 0) {
            val r = Runnable { if (capsule.value == c) capsule.value = null }
            capsuleClear = r
            main.postDelayed(r, ms)
        }
    }

    override fun onStatus(kind: Int, attempt: Int, max: Int, error: String?, hasReport: Boolean, displayRestored: Boolean, audioRestored: Boolean, reportError: String?) {
        val p = Phase.entries[kind.coerceIn(0, 6)]
        // L'avis du redémarrage suit un état « connexion » : seul l'état suivant le clôt.
        stationRestarting.value = false
        phase.value = p
        when (p) {
            Phase.RECONNECTING -> show(Capsule.Reconnecting(attempt, max), 0)
            Phase.STREAMING -> {
                firstImageAt.compareAndSet(0, System.currentTimeMillis())
                if (capsule.value is Capsule.Reconnecting) main.post { capsule.value = null }
            }
            // Fin par le relais (accès coupé dans Admin, ordinateur éteint ou en veille) : ce texte, avec le nom de l'ordinateur,
            // à la place de celui du cœur (qui n'a pas le nom d'une session de l'app).
            // Fin d'un prêt : le texte du cœur (« {hôte} a repris l'ordinateur. ») ; l'ordinateur rendu (depuis l'app ou le
            // lanceur) : « Vous avez rendu l'ordinateur. » sur la carte de fin, sans l'erreur de la session.
            Phase.ENDED -> {
                val core = lendEndText?.takeIf { it.isNotBlank() }
                val text = when (lendEnd) {
                    LendEnd.LEFT -> core ?: context.getString(R.string.lend_end_left)
                    LendEnd.STOPPED -> core ?: relayEndText() ?: error ?: context.getString(R.string.lend_end_stopped)
                    else -> core ?: relayEndText() ?: error
                }
                ended.value = Ended(text, if (hasReport) displayRestored else null, if (hasReport) audioRestored else null, reportError)
            }
            else -> {}
        }
    }

    override fun onDisplay(id: Int, width: Int, height: Int) {
        display.value = DisplayInfo(id, width, height)
    }

    override fun onDisplays(json: String) {
        displays.value = runCatching {
            val a = JSONArray(json)
            (0 until a.length()).map { a.getJSONObject(it).let { o -> DisplayInfo(o.getInt("id"), o.getInt("width"), o.getInt("height")) } }
        }.getOrDefault(emptyList())
    }

    override fun onMode(width: Int, height: Int, refresh: Int, virtualDisplay: Boolean) {
        mode.value = Triple(width, height, refresh)
        this.virtualDisplay.value = virtualDisplay
    }

    override fun onSecond(values: DoubleArray) {
        val chain = if (values[10] > 0.5) values.copyOfRange(11, 17) else null
        second.value = Second(
            latencyMs = values[0].takeIf { it >= 0 },
            rttMs = values[1].takeIf { it >= 0 },
            fps = values[2].toInt(),
            lostFrames = values[5].toInt(),
            holes = values[6].toInt(),
            lostPackets = values[8].toInt(),
            mbps = values[9] * 8 / 1_000_000,
            chain = chain,
        )
    }

    override fun onNotice(kind: Int, width: Int, height: Int, back: Boolean, level: Int, mbps: Int, hasMode: Boolean, error: String?) {
        when (kind) {
            SessionListener.NOTICE_STATION_RESTARTING -> stationRestarting.value = true
            0 -> show(Capsule.Relaunched)
            1 -> show(Capsule.ModeChanged(width, height, back))
            2 -> show(Capsule.Fallback(level, mbps), 6000)
            3 -> show(Capsule.Adapting(width, height))
            4 -> main.post { if (capsule.value is Capsule.Adapting) capsule.value = null }
            5 -> show(Capsule.AdaptFailed(error ?: ""), 6000)
            6 -> restoreOffer.value = if (hasMode) width to height else null
        }
    }

    override fun onGameFullscreen(game: Boolean) {
        gameFullscreen.value = game
    }

    override fun onInputConnected() {
        main.post { onInputConnected?.invoke() }
    }

    override fun onVideoSize(width: Int, height: Int) {
        videoSize.value = width to height
    }

    override fun onDecoder(name: String) {
        decoder.value = name
    }

    override fun onAudioOutput(channels: Int, bufferMs: Double) {
        audioOutput.value = channels to bufferMs
    }

    @Volatile private var rumbleSeen = false

    override fun onRumble(pad: Int, large: Int, small: Int, durationMs: Int) {
        if (!rumbleSeen) {
            rumbleSeen = true
            android.util.Log.i("KyberNow", "vibrations : première reçue de l'ordinateur (rang $pad, $large/$small, $durationMs ms)")
        }
        val p = gamepads.pads.value.firstOrNull { it.slot == pad } ?: run {
            android.util.Log.w("KyberNow", "vibrations : aucune manette au rang $pad")
            return
        }
        // La manette à l'écran : c'est l'appareil qui vibre.
        if (p.onScreen) onScreenRumble?.invoke(large, small, durationMs)
        else gamepads.rumble(p.deviceId, large, small, durationMs)
    }

    override fun onCombo(pad: Int) {
        main.post { onCombo?.invoke() }
    }

    // MARK: - KyberNow Share

    override fun onGuests(json: String?) {
        val now = ShareState.parse(json, codeFormat, imageLabel)
        val known = share.value?.requests.orEmpty().map { it.id }
        share.value = now
        // Une nouvelle demande venue par le relais : son prénom dans une capsule (« Léa demande à rejoindre »), le temps de
        // rouvrir la barre et d'y répondre ; le panneau et la pastille « 1 demande » la montrent ensuite.
        now?.requests?.firstOrNull { it.id !in known }?.let { show(Capsule.ShareRequest(it.name), 6000) }
    }

    override fun onRoute(json: String) {
        RouteInfo.parse(json)?.let { route.value = it }
    }

    override fun onRelayError(code: Int, text: String) {
        RelayFailure.of(code)?.let { relayEnd = it to text }
    }

    override fun onShareRole(role: Int, previous: Int) {
        val now = ShareRole.of(role)
        shareRole.value = now
        if (previous < 0) return
        shareRightsNotice(ShareRole.of(previous), now)?.let { show(Capsule.ShareRights(it)) }
    }

    override fun onShareImage(imageJson: String, previousJson: String?, notice: String?) {
        ShareImage.parse(imageJson, imageLabel)?.let { shareImage.value = it }
        // L'avis (« Image allégée pour suivre. ») se montre comme celui d'un changement de droits ; le cœur le tait quand il n'y a rien à dire.
        if (!notice.isNullOrBlank()) show(Capsule.ShareNotice(notice))
    }

    override fun onShareEnd(end: Int, text: String) {
        // Le texte arrive aussi avec la fin de la session (`Ended.error`) : gardé pour que l'écran de fin le montre tel quel.
        shareEndText = text
        android.util.Log.i("KyberNow", "partage : fin ${listOf("arrêté", "retiré", "format illisible").getOrElse(end) { end.toString() }}")
    }

    // MARK: - Prêt de l'ordinateur

    override fun onLendNotice(json: String, text: String) {
        LendNotice.parse(json)?.let { lendNotice.value = it }
        // Le préavis reste un moment à l'écran ; la barre dit ensuite le temps qui reste.
        if (text.isNotBlank()) show(Capsule.LendNotice(text), 12_000)
    }

    override fun onLendEnd(end: Int, text: String) {
        lendEnd = LendEnd.of(end)
        lendEndText = text
        android.util.Log.i("KyberNow", "prêt : fin ${lendEnd}")
    }

    override fun onLend(json: String?) {
        val now = LendView.parse(json, codeFormat)
        val before = lend.value
        lend.value = now
        // Une nouvelle demande : son prénom dans une capsule (« Léa veut emprunter l'ordinateur. ») ; la choisir ouvre le panneau.
        val known = before?.requests.orEmpty().map { it.id }
        now?.requests?.firstOrNull { it.id !in known }?.let { show(Capsule.LendRequest(it.name, it.slot), 8000); return }
        // Le créneau d'un ami commence : l'avis du cœur, une fois.
        val notice = now?.notice
        if (notice != null && notice.reason == LendNoticeReason.SLOT_START && notice != before?.notice) {
            lendNoticeText(notice, now.borrower?.name ?: "")?.takeIf { it.isNotBlank() }?.let { show(Capsule.LendHostNotice(it), 8000) }
        }
    }

    override fun onPointer(hasPosition: Boolean, x: Double, y: Double, remoteHidden: Boolean, width: Int, height: Int, hotX: Int, hotY: Int, pixels: IntArray?) {
        if (pixels != null && width > 0 && height > 0) {
            val bmp = Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888).apply { isPremultiplied = true }
            pointerShape.value = bmp to (hotX to hotY)
        }
        val prev = pointer.value
        val shownAt = if (!remoteHidden && prev.remoteHidden) System.currentTimeMillis() else if (remoteHidden) 0 else prev.shownAt.takeIf { it > 0 } ?: System.currentTimeMillis()
        pointer.value = PointerState(hasPosition, x, y, remoteHidden, shownAt)
    }

    // MARK: - Presse-papiers (lu seulement quand l'ordinateur le demande)

    private val clipboard = context.getSystemService(ClipboardManager::class.java)
    private val clipChanges = AtomicLong(0)
    /** Notre propre écriture : son avis de changement (qui arrive après) ne compte pas. */
    private val ownWrite = java.util.concurrent.atomic.AtomicBoolean(false)
    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        if (!ownWrite.getAndSet(false)) clipChanges.incrementAndGet()
    }

    init {
        main.post { clipboard?.addPrimaryClipChangedListener(clipListener) }
    }

    fun close() {
        main.post { clipboard?.removePrimaryClipChangedListener(clipListener) }
    }

    override fun clipChangeCount(): Long = clipChanges.get()

    override fun clipHasText(): Boolean = clipboard?.primaryClipDescription?.hasMimeType("text/*") == true

    override fun clipText(): String? = runCatching { clipboard?.primaryClip?.getItemAt(0)?.coerceToText(context)?.toString() }.getOrNull()

    override fun clipSetText(text: String) {
        copyText(text)
    }

    /**
     * Écrit [text] dans le presse-papiers de l'appareil sans que l'ordinateur le prenne pour une copie
     * de l'utilisateur (« Copier le lien » : le lien du partage ne va pas coller sur l'ordinateur) ;
     * `false` si Android refuse.
     */
    fun copyText(text: String): Boolean {
        val manager = clipboard ?: return false
        ownWrite.set(true)
        val ok = runCatching { manager.setPrimaryClip(ClipData.newPlainText("KyberNow", text)) }.isSuccess
        if (!ok) ownWrite.set(false)
        return ok
    }
}
