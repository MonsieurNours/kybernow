package fr.leslignesclaires.kybernow.session

import android.content.pm.ActivityInfo
import android.graphics.Bitmap
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.view.Gravity
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.PixelCopy
import android.view.PointerIcon
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.WindowManager
import android.widget.FrameLayout
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.platform.ComposeView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.ConsoleClient
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.core.Reply
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareFound
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.model.AudioOutput
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.LastSession
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.model.SessionMode
import fr.leslignesclaires.kybernow.ui.KnTheme
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.io.FileOutputStream
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * La session : l'image plein cadre sur une `SurfaceView` (décodée et rendue
 * par le cœur, MediaCodec), le son par AAudio (le cœur aussi), les entrées
 * captées ici et envoyées au cœur. Rien ne reste au-dessus de l'image quand
 * aucun menu n'est ouvert et que la manette à l'écran est cachée : la vue
 * d'interface est retirée (GONE).
 */
class SessionActivity : AppCompatActivity(), SurfaceHolder.Callback, SessionActions {
    companion object {
        const val EXTRA_HOST = "host"
        /** Session d'invité (KyberNow Share) : la Station et ses jetons sont remis en mémoire par `KyberNowApp.handOverGuest`. */
        const val EXTRA_GUEST = "guest"
    }

    private val app get() = KyberNowApp.instance
    private lateinit var host: Host
    /** KyberNow Share : la Station qui a accepté cet invité ; nul pour la session d'un ordinateur de l'appareil. */
    private var guest: ShareFound? = null
    private lateinit var state: SessionState
    private lateinit var ui: SessionUi
    private lateinit var surfaceView: SurfaceView
    private lateinit var pointerView: PointerView
    private lateinit var overlay: ComposeView
    private lateinit var pads: PadRouter
    private lateinit var remote: RemoteRouter
    private lateinit var touch: TouchInput
    private lateinit var haptics: Haptics
    private val main = Handler(Looper.getMainLooper())

    @Volatile private var handle = 0L
    private var refresh = 60
    private var sessionMode = Triple(1920, 1080, 60)
    private var tv = false
    private var relative = false
    private var mouseSeen = false
    /** Le dernier appui venait d'une souris (et pas d'un doigt ni d'une touche) : voir [share]. */
    private var mouseClick = false
    /**
     * La manette à l'écran vers l'ordinateur, à sa place de joueur (−1 : une cinquième manette,
     * en attente, rien ne part). Barre pilotée à la manette : les appuis ne partent pas (les
     * relâchements, si), comme pour les vraies manettes.
     */
    private val onScreenOut = object : PadOut {
        private fun slot() = app.gamepads.pads.value.firstOrNull { it.onScreen }?.slot ?: -1
        override fun button(button: Int, pressed: Boolean) {
            val s = slot()
            if (s >= 0 && !(pressed && pads.captured)) Native.padButton(handle, s, button, pressed)
        }
        override fun axis(axis: Int, value: Int) {
            val s = slot()
            if (s >= 0 && !(value != 0 && pads.captured)) Native.padAxis(handle, s, axis, value)
        }
        override fun feel(f: Feel) = haptics.feel(f)
    }

    /** Boutons de manette enfoncés depuis l'ouverture d'un menu : le relâché des autres (le combo) est ignoré. */
    private val padDowns = mutableSetOf<Int>()
    private var quitting = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (intent.getBooleanExtra(EXTRA_GUEST, false)) {
            // Un invité n'est jamais ajouté aux ordinateurs de l'appareil : un ordinateur d'attente, en mémoire,
            // pour l'adresse et le nom ; rien ne s'en écrit (`saveLast`, `saveSettings`, `quit` testent `guest`). Remise perdue (le processus a été
            // recréé) : pas de session, les jetons ne se gardent pas.
            val found = app.takeGuest() ?: run { finish(); return }
            guest = found
            host = Host(
                id = "invite", name = found.computer, address = found.target.host, port = found.target.port,
                fingerprint = found.target.fingerprint, user = "", virtualDisplay = false,
            )
        } else {
            host = intent.getStringExtra(EXTRA_HOST)?.let { app.hosts.get(it) } ?: run { finish(); return }
        }
        tv = app.form == FormFactor.TV
        if (app.form == FormFactor.PHONE) requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }

        state = SessionState(this, app.gamepads)
        guest?.let {
            state.shareRole.value = it.role
            // La « Qualité de l'image » de « Rejoindre » (retenue) ; la Station dit l'image reçue (`onShareImage`).
            state.shareQuality.value = app.guestQuality
        }
        pads = PadRouter({ handle }, app.gamepads).apply { guide = host.settings.guide }
        remote = RemoteRouter { handle }
        // Un invité : « Invité sur {nom} » (la connexion, le menu), la barre réduite.
        val title = guest?.let { getString(R.string.share_guest_on, it.computer) } ?: host.name
        ui = SessionUi(title, tv, state, this, OnScreenPad(onScreenOut), guest = guest != null, computerName = host.name).apply {
            profile = host.settings.profile
            mouseMode = host.settings.mouse
            directTouch = host.settings.directTouch
            backOpensMenu = host.settings.backOpensMenu
            combo = host.settings.combo
            padShown = !tv && host.settings.onScreenPad
            padSize = host.settings.padSize
            padOpacity = host.settings.padOpacity
            pad.setLayout(host.settings.padSimple)
            pad.onSwitch = { setPadLayout(!pad.simple) }
            // Un toucher hors des commandes replie le clavier, comme sur l'image sans manette.
            pad.onTapOutside = { if (keyboardOpen) openKeyboard(false) }
        }
        touch = TouchInput(PointerOut.native { handle }, ::viewToHost, ::hostPerViewPixel).apply {
            direct = host.settings.directTouch
            keyboardOpen = { ui.keyboardOpen }
            onFold = { openKeyboard(false) }
        }

        val root = FrameLayout(this)
        surfaceView = SurfaceView(this).apply {
            holder.addCallback(this@SessionActivity)
            isFocusable = true
            isFocusableInTouchMode = true
            // La manette à l'écran prend les doigts au-dessus (vue d'interface) : l'image ne sert plus de souris.
            // Un invité sans le clavier et la souris : l'image ne sert pas de souris (rien ne partirait).
            setOnTouchListener { _, e -> if (!e.isFromSource(InputDevice.SOURCE_TOUCHSCREEN)) false else if (ui.padShown || !ui.keyboardMouseAllowed()) true else touch.onTouch(e) }
            setOnCapturedPointerListener { _, e -> capturedMouse(e) }
        }
        root.addView(surfaceView, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT, Gravity.CENTER))
        pointerView = PointerView(this, if (tv) 2f else 1f)
        root.addView(pointerView, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        overlay = ComposeView(this).apply { setContent { KnTheme(app.form) { SessionOverlay(ui) } } }
        root.addView(overlay, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        setContentView(root)
        // Rotation d'une tablette, fenêtre redimensionnée (ni l'un ni l'autre ne recrée l'activité) : l'image reprend sa place
        // dans le nouveau cadre ; `fitSurface` ne tournait qu'à l'arrivée d'une nouvelle taille d'image.
        root.addOnLayoutChangeListener { _, l, t, r, b, ol, ot, or, ob ->
            if (r - l != or - ol || b - t != ob - ot) root.post { state.videoSize.value?.let { (w, h) -> fitSurface(w, h) } }
        }
        haptics = Haptics(overlay).apply { level = host.settings.padHaptics }
        // Les vibrations du jeu qui visent la manette à l'écran : l'appareil vibre, tant qu'elle est affichée.
        state.onScreenRumble = { large, small, ms -> if (ui.padShown) haptics.rumble(large, small, ms) }
        // Affichée : elle prend sa place de joueur avant la première annonce des manettes.
        if (ui.padShown) app.gamepads.setOnScreen(true)

        state.onCombo = { if (host.settings.combo) toggleMenu(fromPad = true) }
        pads.mouse.onCombo = { if (host.settings.combo) toggleMenu(fromPad = true) }
        state.onInputConnected = { pads.reconnected() }
        start()
        observe()
    }

    // MARK: - Mode de la session et écran

    /** Caler l'écran (HDMI) sur la session, et le mode jeu du téléviseur (ALLM). */
    private fun applyDisplay() {
        val screen = Screen.of(display ?: return)
        val (w, h, r) = sessionMode
        val set = host.settings
        val attrs = window.attributes
        val target: ScreenMode? = when {
            set.matchDisplay && tv -> screen.modeFor(w, h, r) ?: screen.modes.filter { it.refresh == r && it.width >= w && it.height >= h }.minByOrNull { it.width * it.height }
            // Téléphone, tablette : même définition, la fréquence de la session (120 Hz).
            else -> screen.modes.firstOrNull { it.width == screen.current.width && it.height == screen.current.height && it.refresh == r }
        }
        if (target != null) attrs.preferredDisplayModeId = target.id
        attrs.preferredRefreshRate = r.toFloat()
        window.attributes = attrs
        if (set.gameMode && screen.minimalPostProcessing) window.setPreferMinimalPostProcessing(true)
        refresh = target?.refresh ?: r
    }

    private fun settingsJson(profile: Profile): JSONObject {
        val d = app.decoders
        val base = JSONObject(Native.profileSettings(profile.id, d.hevc != null, d.hevc444, d.av1 != null && host.av1))
        val set = host.settings
        when (set.codec) {
            "hevc" -> if (d.hevc != null) base.put("codec", "hevc")
            "h264" -> base.put("codec", "h264").put("yuv444", false)
            "av1" -> if (d.av1 != null && host.av1) base.put("codec", "av1").put("yuv444", false)
        }
        if (set.mbps > 0) base.put("mbps", set.mbps).remove("profile")
        if (set.transport != "auto") base.put("reliable", set.reliable)
        return base
    }

    private fun audioChannels(): Int = when (host.settings.audio) {
        "computer" -> 0
        "stereo" -> 2
        "5.1" -> 6
        "7.1" -> 8
        else -> AudioOutput.current(this).let { if (it.hdmi && it.maxChannels >= 8) 8 else if (it.hdmi && it.maxChannels >= 6) 6 else 2 }
    }

    private fun start() {
        val screen = Screen.of(display ?: return)
        val settings = settingsJson(ui.profile)
        val g = guest
        // Le même calcul que l'accueil (« Écran de … »). Un invité reçoit l'écran du partage, tel que la Station le donne.
        sessionMode = if (g != null && g.width > 0 && g.height > 0) Triple(g.width, g.height, if (g.refresh > 0) g.refresh.coerceIn(24, 240) else 60)
        else SessionMode.pick(screen, host.settings, settings.optString("codec"), app.decoders)
        applyDisplay()
        val (w, h, r) = sessionMode
        val device = JSONObject(app.device.json())
        val params = if (g != null) {
            // Pas de compte, pas de mode ni d'écran virtuel : le cœur ouvre la session avec la réponse de « Rejoindre »
            // (jeton, empreinte de Kyber, écran, rôle, image) et, d'après ce que l'appareil décode, demande à Kyber le
            // format de l'hôte s'il est lu, sinon un format lu (une image à part) : le format ne refuse plus personne.
            val d = app.decoders
            JSONObject()
                .put("console_host", g.target.host)
                .put("console_port", g.target.port)
                .put("console_fingerprint", g.target.fingerprint ?: JSONObject.NULL)
                .put("device", device)
                .put("audio_channels", audioChannels())
                .put("virtual_display", false)
                .put("settings", settings)
                .put("auto_reconnect", host.settings.reconnect)
                .put("gamepad_xbox360", host.settings.xbox360)
                .put("clipboard", false)
                .put("combo", host.settings.combo)
                .put("recovery", JSONObject().put("watch_image", true))
                .put("guest", JSONObject().put("joined", g.joined).put("capabilities", JSONObject().put("hevc", d.hevc != null).put("hevc444", d.hevc444).put("av1", d.av1 != null)))
        } else JSONObject()
            .put("console_host", host.address)
            .put("console_port", host.port)
            .put("user", host.user)
            .put("password", app.hosts.password(host.id) ?: "")
            .put("console_fingerprint", host.fingerprint ?: JSONObject.NULL)
            .put("device", device)
            .put("mode", JSONObject().put("width", w).put("height", h).put("refresh", r))
            .put("audio_channels", audioChannels())
            .put("virtual_display", host.virtualDisplay && host.settings.virtualDisplay)
            .put("settings", settings)
            .put("auto_reconnect", host.settings.reconnect)
            .put("gamepad_xbox360", host.settings.xbox360)
            .put("clipboard", host.settings.clipboard && !tv)
            .put("combo", host.settings.combo)
            .put("recovery", JSONObject().put("watch_image", true).put("fallback_bitrate", 150_000_000))
        handle = Native.sessionStart(params.toString(), null, state, refresh)
        if (handle == 0L) {
            // Un invité que la Station a déjà accepté repart tout de suite (sinon elle l'attend 20 s sans signe de vie).
            g?.let(Share::leave)
            finish()
        }
    }

    private fun observe() {
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.CREATED) {
                launch { state.videoSize.collect { it?.let { (w, h) -> fitSurface(w, h) } } }
                launch {
                    state.ended.collect { e ->
                        if (e == null) return@collect
                        saveLast()
                        if (e.error == null || quitting) finishSession()
                    }
                }
                launch {
                    combine(state.pointer, state.display) { p, _ -> p }.collect { p ->
                        placePointer(p)
                        updateMouseMode()
                    }
                }
                launch { state.pointerShape.collect { it?.let { (b, hot) -> pointerView.setShape(b, hot.first, hot.second) } } }
                launch { state.gameFullscreen.collect { updateMouseMode() } }
                launch { state.display.collect { d -> d?.let { pads.mouse.hostWidth = it.width.toDouble(); remote.hostWidth = it.width.toDouble() } } }
                launch {
                    app.gamepads.pads.collectLatest {
                        // Une manette branchée en cours de session : annoncée à l'ordinateur et dans une capsule
                        // (pas la manette à l'écran : on vient de l'afficher soi-même).
                        val added = pads.sync()
                        // Un invité sans les manettes n'en envoie pas : sa manette n'est annoncée à personne, pas de capsule.
                        if (state.firstImageAt.get() > 0 && padsAllowed()) added.firstOrNull { !it.onScreen }?.let { p -> state.show(Capsule.GamepadConnected(p.name, p.slot, p.motors)) }
                    }
                }
                launch {
                    snapshotFlow { ui.anything }.combine(state.capsule) { a, c -> a || c != null || state.ended.value?.error != null }
                        .collect { visible -> overlay.visibility = if (visible) View.VISIBLE else View.GONE }
                }
                launch { state.ended.collect { if (it?.error != null) overlay.visibility = View.VISIBLE } }
                launch {
                    // Invité : ce que son rôle permet change en cours de partage. Un droit retiré relâche ce qui était tenu
                    // (le cœur le fait de son côté ; ici, ce que l'interface montre).
                    state.shareRole.collect { role -> if (guest != null) applyShareRole(role) }
                }
                launch {
                    // Hôte : ce que « Partager » peut faire avec cet ordinateur, relu à chaque ouverture de la barre ou du menu.
                    snapshotFlow { ui.barOpen || ui.menuOpen }.collect { open -> if (open && guest == null) refreshShareAvailability() }
                }
                launch {
                    // La barre se ferme, quelle que soit la façon (B, la croix, le clavier de l'écran) :
                    // la manette qui la pilotait revient à l'ordinateur.
                    snapshotFlow { ui.barOpen }.collect { open ->
                        if (!open && ui.padBar) {
                            ui.padBar = false
                            if (!quitting) {
                                padDowns.clear()
                                pads.release()
                                // Le clavier de l'écran (ouvert depuis la barre) garde son champ.
                                if (!ui.keyboardOpen) surfaceView.requestFocus()
                            }
                        }
                    }
                }
            }
        }
        app.gamepads.start()
    }

    /** L'image garde son format, centrée (bandes d'encre). */
    private fun fitSurface(vw: Int, vh: Int) {
        val root = surfaceView.parent as View
        val (W, H) = root.width to root.height
        if (W == 0 || H == 0 || vw == 0 || vh == 0) return
        val k = min(W.toDouble() / vw, H.toDouble() / vh)
        val lp = surfaceView.layoutParams as FrameLayout.LayoutParams
        lp.width = (vw * k).roundToInt()
        lp.height = (vh * k).roundToInt()
        lp.gravity = Gravity.CENTER
        surfaceView.layoutParams = lp
    }

    // MARK: - Surface

    override fun surfaceCreated(holder: SurfaceHolder) {
        val s = holder.surface
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            s.setFrameRate(refresh.toFloat(), Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE, Surface.CHANGE_FRAME_RATE_ALWAYS)
        } else {
            s.setFrameRate(refresh.toFloat(), Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE)
        }
        Native.sessionSetSurface(handle, s, refresh)
        surfaceView.requestFocus()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        Native.sessionSetSurface(handle, null, 0)
    }

    // MARK: - Cycle de vie

    override fun onPause() {
        super.onPause()
        // Les relâchements qui suivent ne nous parviendraient pas : tout est relâché.
        ui.pad.releaseAll()
        haptics.stop()
        if (handle != 0L) Native.sessionSetBackground(handle, true, 2_000_000)
        pads.mouse.stop()
    }

    override fun onResume() {
        super.onResume()
        if (handle != 0L) Native.sessionSetBackground(handle, false, 0)
    }

    override fun onDestroy() {
        super.onDestroy()
        // `onCreate` s'arrête avant de construire quoi que ce soit quand la remise d'un invité est perdue (processus ou
        // activité recréés : les jetons ne se gardent pas) ou que l'ordinateur n'existe plus : `onDestroy` suit aussitôt.
        if (!::state.isInitialized) return
        if (::haptics.isInitialized) haptics.stop()
        // La manette à l'écran n'existe qu'en session : sa place se libère.
        app.gamepads.setOnScreen(false)
        app.gamepads.stop()
        state.close()
        val h = handle
        handle = 0L
        if (h != 0L) Thread({ Native.sessionFree(h) }, "kybernow-fin").start()
    }

    // MARK: - Fin

    override fun quit() {
        if (quitting) return
        quitting = true
        ui.menuOpen = false
        ui.barOpen = false
        ui.keysOpen = false
        ui.shareOpen = false
        ui.qualityOpen = false
        ui.typing = false
        ui.statsOpen = false
        ui.pad.releaseAll()
        pads.capture()
        val h = handle
        if (h == 0L || state.ended.value != null) {
            finishSession()
            return
        }
        ui.leaving = true
        // L'ordinateur remet son écran et son son avant de répondre ; au-delà de 6 s, on part
        // quand même (la session finit en arrière-plan, voir onDestroy).
        main.postDelayed({ finishSession() }, 6000)
        // Un invité ne garde pas d'image : elle n'appartient à aucun ordinateur de l'appareil.
        if (guest != null) Native.sessionStop(h) else captureLastImage { Native.sessionStop(h) }
    }

    /** La dernière image de la session, pour l'accueil et la rangée d'Android TV. */
    private fun captureLastImage(next: () -> Unit) {
        // Une seule fois : à la copie ou, si elle traîne, au bout de 800 ms.
        val done = java.util.concurrent.atomic.AtomicBoolean(false)
        val then = { if (!done.getAndSet(true)) next() }
        main.postDelayed({ then() }, 800)
        val w = surfaceView.width
        val h = surfaceView.height
        if (w <= 0 || h <= 0 || state.firstImageAt.get() == 0L) { then(); return }
        val bmp = Bitmap.createBitmap(w / 2, h / 2, Bitmap.Config.ARGB_8888)
        val thread = HandlerThread("kybernow-image").apply { start() }
        runCatching {
            PixelCopy.request(surfaceView, bmp, { result ->
                if (result == PixelCopy.SUCCESS) {
                    runCatching { FileOutputStream(app.hosts.imageFile(host.id)).use { bmp.compress(Bitmap.CompressFormat.JPEG, 85, it) } }
                }
                thread.quitSafely()
                main.post(then)
            }, Handler(thread.looper))
        }.onFailure { thread.quitSafely(); then() }
    }

    /**
     * La dernière session de l'ordinateur (Statistiques), à partir de 3 s d'image : latence
     * médiane et 95e centile, durée, quand, et ce qui a été transmis (mode, format, débit).
     */
    private fun saveLast() {
        // Un invité n'écrit rien dans les ordinateurs de l'appareil.
        if (guest != null) return
        val h = handle
        if (h == 0L) return
        val started = state.firstImageAt.get()
        if (started == 0L) return
        val seconds = (System.currentTimeMillis() - started) / 1000
        if (seconds < 3) return
        val lat = Native.sessionLatency(h) ?: return
        val (w, hh, r) = state.mode.value ?: sessionMode
        val settings = settingsJson(ui.profile)
        val mbps = SessionMode.bitrate(settings.optInt("mbps"), settings.has("profile"), w, hh, r)
        val last = LastSession(lat[0], lat[1], seconds, System.currentTimeMillis(), w, hh, r, settings.optString("codec").ifEmpty { null }, mbps)
        app.hosts.update(host.id) { it.copy(last = last) }
    }

    private fun finishSession() {
        if (isFinishing) return
        finish()
    }

    // MARK: - Menu et barre

    /**
     * Le menu (téléviseur) ou la barre. Hors téléviseur, la combinaison de la manette ouvre la
     * barre pilotée à la manette (consoles portables) : la manette est prise par l'app comme sous
     * le menu du téléviseur, jusqu'à ce que la barre se ferme (voir `observe`).
     */
    private fun toggleMenu(fromPad: Boolean = false) {
        if (tv) {
            if (ui.menuOpen) resume() else {
                padDowns.clear()
                ui.menuOpen = true
                pads.capture()
            }
        } else if (fromPad) {
            if (ui.padBar) ui.barOpen = false else {
                padDowns.clear()
                pads.capture()
                // Hors du mode tactile : sans cela, les éléments de Compose ne prennent pas le focus.
                surfaceView.requestFocusFromTouch()
                ui.padBar = true
                ui.barOpen = true
            }
        } else {
            ui.barOpen = !ui.barOpen
        }
    }

    /** Ferme ce qui est ouvert, du dessus vers le dessous ; `false` s'il n'y avait rien. */
    private fun closeTop(): Boolean = when {
        ui.typing -> { ui.typing = false; true }
        ui.keysOpen -> { ui.keysOpen = false; true }
        ui.qualityOpen -> { ui.qualityOpen = false; true }
        ui.shareOpen -> { ui.shareOpen = false; true }
        ui.statsOpen && !ui.menuOpen -> { ui.statsOpen = false; true }
        ui.keyboardOpen -> { openKeyboard(false); true }
        ui.menuOpen -> { resume(); true }
        ui.barOpen -> { ui.barOpen = false; true }
        else -> false
    }

    override fun resume() {
        ui.menuOpen = false
        ui.keysOpen = false
        ui.shareOpen = false
        ui.qualityOpen = false
        ui.typing = false
        padDowns.clear()
        pads.release()
        surfaceView.requestFocus()
    }

    override fun setProfile(p: Profile) {
        ui.profile = p
        Native.sessionSetSettings(handle, settingsJson(p).toString())
        if (tv) resume()
    }

    override fun setMouseMode(m: Int) {
        ui.mouseMode = m
        updateMouseMode()
    }

    override fun setDirectTouch(on: Boolean) {
        ui.directTouch = on
        touch.direct = on
    }

    override fun setRemoteMouse(on: Boolean) {
        ui.remoteMouse = on
        remote.asMouse = on
        if (on) Native.pointerMoveBy(handle, 0.0, 0.0)
        placePointer(state.pointer.value)
        resume()
    }

    override fun setPadMouse(on: Boolean) {
        ui.padMouse = on
        if (!on) pads.mouse.stop()
        pads.asMouse = on
        if (on) Native.pointerMoveBy(handle, 0.0, 0.0)
        placePointer(state.pointer.value)
        resume()
    }

    override fun shortcut(s: Shortcut) {
        Sc.press(handle, *s.keys)
        if (tv) resume()
    }

    /** Une touche de la rangée, avec les modificateurs collants tenus. */
    override fun key(scancode: Int) {
        val mods = ui.sticky.toIntArray()
        Sc.press(handle, *(mods + scancode))
        ui.sticky = emptySet()
    }

    override fun text(s: String) {
        if (s.isEmpty()) return
        if (ui.sticky.isNotEmpty() && s.length == 1) {
            // Ctrl + lettre : la touche de la lettre (QWERTY de kynput), pas le caractère.
            val code = KeyEvent.keyCodeFromString("KEYCODE_" + s.uppercase())
            val sc = Native.scancode(androidKeyToEvdev(code))
            if (sc >= 0) { key(sc); return }
        }
        Native.inputText(handle, s)
    }

    override fun restoreMode() = Native.sessionRestoreMode(handle)

    override fun pickDisplay(id: Int) = Native.sessionPickDisplay(handle, id)

    override fun openKeyboard(open: Boolean) {
        ui.keyboardOpen = open
        if (open) ui.barOpen = false
        if (!open) {
            val imm = getSystemService(android.view.inputmethod.InputMethodManager::class.java)
            imm?.hideSoftInputFromWindow(surfaceView.windowToken, 0)
            surfaceView.requestFocus()
        }
    }

    override fun setOnScreenPad(on: Boolean) {
        if (tv || ui.padShown == on) return
        if (on) {
            ui.barOpen = false
        } else {
            // Tout relâché tant qu'elle a sa place, puis débranchée (PadRouter.sync).
            ui.pad.releaseAll()
            haptics.stop()
        }
        ui.padShown = on
        app.gamepads.setOnScreen(on)
        saveSettings { it.copy(onScreenPad = on) }
    }

    override fun setPadLayout(simple: Boolean) {
        ui.pad.setLayout(simple)
        saveSettings { it.copy(padLayout = if (simple) "simple" else "full") }
    }

    // MARK: - KyberNow Share

    /**
     * Invité : le rôle a changé (au départ, ou par l'hôte). Ce que l'interface montrait et que le rôle ne
     * permet plus est retiré : la manette à l'écran sans les manettes, le clavier, la souris et les
     * commandes à distance sans le clavier et la souris (chaque droit à part : reprendre l'un ne touche
     * pas à l'autre). Le cœur, lui, applique déjà les droits.
     */
    private fun applyShareRole(role: ShareRole) {
        if (!role.pads && ui.padShown) {
            ui.pad.releaseAll()
            haptics.stop()
            ui.padShown = false
            app.gamepads.setOnScreen(false)
        }
        if (!role.keyboardMouse) {
            // Le clavier de l'écran se replie aussi côté Android (sinon le clavier logiciel reste affiché sans champ).
            if (ui.keyboardOpen) openKeyboard(false)
            ui.keysOpen = false
            ui.typing = false
            ui.sticky = emptySet()
            if (ui.padMouse) { ui.padMouse = false; pads.asMouse = false }
            if (ui.remoteMouse) { ui.remoteMouse = false; remote.asMouse = false }
            pads.mouse.stop()
            // Un geste au doigt en cours (maintien pour glisser, toucher direct) ne doit rien envoyer après coup.
            touch.cancel()
            if (relative) { relative = false; surfaceView.releasePointerCapture() }
            if (handle != 0L) Native.inputReleaseAll(handle)
            pointerView.place(0f, 0f, false)
        }
    }

    /**
     * Invité : la « Qualité de l'image » change en cours de session. Le cœur la dit à la Station (sur un fil, sans rien
     * relancer ici) : elle relance l'image de l'invité seulement, jamais celle de l'hôte, et l'image reçue revient par
     * `onShareImage`. Retenue pour la fois suivante.
     */
    override fun setShareQuality(quality: ShareQuality) {
        if (guest == null || quality == state.shareQuality.value) return
        state.shareQuality.value = quality
        app.guestQuality = quality
        val h = handle
        if (h != 0L && !Native.sessionSetShareQuality(h, quality.code)) android.util.Log.w("KyberNow", "partage : qualité ${quality.id} refusée par le cœur")
    }

    /** Un compte de l'appareil pour les appels de l'hôte au partage (jamais pour un invité). */
    private fun console() = ConsoleClient(host.address, host.port, host.user, app.hosts.password(host.id) ?: "", host.fingerprint)

    /**
     * Hôte : ce que « Partager » peut faire avec cet ordinateur d'après son info (Station 0.8.0 ou plus,
     * partage coupé ou non), les textes étant ceux du cœur.
     */
    private fun refreshShareAvailability() {
        lifecycleScope.launch {
            val reply = console().use { it.info() }
            val info = (reply as? Reply.Ok)?.value ?: return@launch
            val a = Share.availability(info.raw)
            ui.shareAvailability = a
            ui.shareUnavailable = Share.availabilityText(a)
        }
    }

    /**
     * « Partager » : le partage ouvert est montré tel quel, sinon Admin ouvre celui de l'écran principal de la
     * session (code, lien). Un refus d'Admin (409 : pas en session, partage coupé ; 404 : Station trop
     * ancienne) s'affiche dans le panneau avec son texte.
     */
    override fun share() {
        if (guest != null) return
        if (!ui.shareUsable) {
            // « Partager » impossible (Station trop ancienne, partage coupé) : à la souris il est inactif, son infobulle
            // donne la raison ; au toucher et à la télécommande il reste sélectionnable et ouvre le panneau réduit à la raison.
            if (!mouseClick) ui.shareOpen = true
            return
        }
        if (ui.shareBusy) return
        ui.shareOpen = true
        ui.shareError = null
        ui.shareCopy = null
        ui.shareBusy = true
        lifecycleScope.launch {
            val failure = console().use { c ->
                // Le partage ouvert d'abord : « Partager » ne le remplace pas (ses invités resteraient sans lien).
                val current = (c.shareState() as? Reply.Ok)?.value
                if (current != null) {
                    state.share.value = current
                    null
                } else when (val r = c.shareOpen()) {
                    is Reply.Ok -> { state.share.value = r.value; null }
                    is Reply.Failed -> r.message.ifBlank { getString(R.string.add_unreachable) }
                }
            }
            ui.shareError = failure
            ui.shareBusy = false
            // Les invités déjà là (et leurs rôles) : relus pour qu'ils soient à jour dans le panneau.
            if (failure == null) refreshShare()
        }
    }

    /** Relit le partage ouvert (les rôles et les invités arrivent aussi avec le signe de vie de la session). */
    private fun refreshShare() {
        lifecycleScope.launch {
            (console().use { it.shareState() } as? Reply.Ok)?.let { state.share.value = it.value }
        }
    }

    /** Un appel au partage : les boutons attendent sa fin, un refus d'Admin s'affiche, le partage est relu. */
    private fun shareCall(call: suspend (ConsoleClient) -> Reply<*>) {
        if (guest != null || ui.shareBusy) return
        ui.shareBusy = true
        ui.shareError = null
        lifecycleScope.launch {
            val reply = console().use { call(it) }
            ui.shareError = (reply as? Reply.Failed)?.message?.ifBlank { getString(R.string.add_unreachable) }
            ui.shareBusy = false
            refreshShare()
        }
    }

    override fun shareNewCode() = shareCall { it.shareCode() }

    override fun shareSetPads(guest: ShareGuest, on: Boolean) = shareRights(guest) { it.withPads(on) }

    override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) = shareRights(guest) { it.withKeyboardMouse(on) }

    /**
     * Un droit donné ou repris : calculé sur le rôle de l'invité tel que la Station le dit maintenant (pas sur
     * celui que le panneau montrait : l'autre droit a pu changer depuis, par Admin), pour ne pas toucher à l'autre.
     */
    private fun shareRights(guest: ShareGuest, change: (ShareRole) -> ShareRole) = shareCall { c ->
        val now = (c.shareState() as? Reply.Ok)?.value?.guest(guest.id)?.role ?: state.share.value?.guest(guest.id)?.role ?: guest.role
        c.shareGuest(guest.id, change(now))
    }

    override fun shareRemove(guest: ShareGuest) = shareCall { it.shareKick(guest.id) }

    /** « Arrêter le partage » : tout le monde sort ; le panneau se ferme. */
    override fun shareStop() {
        if (guest != null || ui.shareBusy) return
        ui.shareBusy = true
        ui.shareError = null
        lifecycleScope.launch {
            val reply = console().use { it.shareClose() }
            ui.shareBusy = false
            if (reply is Reply.Failed) {
                ui.shareError = reply.message.ifBlank { getString(R.string.add_unreachable) }
                refreshShare()
            } else {
                state.share.value = null
                ui.shareOpen = false
            }
        }
    }

    /**
     * « Copier le lien » : l'adresse du client web du partage (`web`) ; sans elle, le lien des apps. Android
     * peut refuser : le panneau le dit et montre l'adresse à sélectionner.
     */
    override fun shareCopyLink() {
        val s = state.share.value ?: return
        val ok = state.copyText(s.web.ifBlank { s.link })
        ui.shareCopy = if (ok) ShareCopy.COPIED else ShareCopy.FAILED
        val shown = ui.shareCopy
        main.postDelayed({ if (ui.shareCopy == shown) ui.shareCopy = null }, if (ok) 2500 else 8000)
    }

    /** Un réglage de l'ordinateur changé depuis la session, retenu pour la suivante. */
    private fun saveSettings(f: (fr.leslignesclaires.kybernow.model.HostSettings) -> fr.leslignesclaires.kybernow.model.HostSettings) {
        host = host.copy(settings = f(host.settings))
        // Un invité n'écrit rien dans les ordinateurs de l'appareil : ses choix valent pour cette session.
        if (guest == null) app.hosts.update(host.id) { it.copy(settings = f(it.settings)) }
    }

    /** Lettres A à Z et chiffres : code Linux, pour le scancode de kynput. */
    private fun androidKeyToEvdev(code: Int): Int {
        val letters = intArrayOf(30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44)
        return when (code) {
            in KeyEvent.KEYCODE_A..KeyEvent.KEYCODE_Z -> letters[code - KeyEvent.KEYCODE_A]
            KeyEvent.KEYCODE_0 -> 11
            in KeyEvent.KEYCODE_1..KeyEvent.KEYCODE_9 -> 2 + code - KeyEvent.KEYCODE_1
            else -> -1
        }
    }

    // MARK: - Souris

    /** Souris Auto / Bureau / Jeu : capture du pointeur (déplacements bruts) quand un jeu le veut. */
    private fun updateMouseMode() {
        if (!mouseSeen || !ui.keyboardMouseAllowed()) return
        val p = state.pointer.value
        val visibleFor = if (p.shownAt > 0 && !p.remoteHidden) (System.currentTimeMillis() - p.shownAt).toDouble() else 0.0
        val want = Native.mouseWantsRelative(ui.mouseMode, state.gameFullscreen.value, p.remoteHidden, visibleFor, relative)
        if (want == relative) return
        relative = want
        Native.inputSetRelative(handle, want)
        if (want) surfaceView.requestPointerCapture() else surfaceView.releasePointerCapture()
        surfaceView.pointerIcon = PointerIcon.getSystemIcon(this, PointerIcon.TYPE_NULL)
    }

    private fun capturedMouse(e: MotionEvent): Boolean {
        // Un invité à qui l'hôte retire la souris pendant que le pointeur est capturé : rien ne part.
        if (!ui.keyboardMouseAllowed()) return true
        val h = handle
        when (e.actionMasked) {
            MotionEvent.ACTION_MOVE -> Native.inputMouseMove(h, e.x.roundToInt(), e.y.roundToInt())
            MotionEvent.ACTION_BUTTON_PRESS -> Native.inputMouseButton(h, button(e.actionButton), true)
            MotionEvent.ACTION_BUTTON_RELEASE -> Native.inputMouseButton(h, button(e.actionButton), false)
            MotionEvent.ACTION_SCROLL -> Native.inputWheel(h, e.getAxisValue(MotionEvent.AXIS_HSCROLL), -e.getAxisValue(MotionEvent.AXIS_VSCROLL))
        }
        return true
    }

    private fun button(b: Int) = when (b) {
        MotionEvent.BUTTON_SECONDARY -> 1
        MotionEvent.BUTTON_TERTIARY -> 2
        MotionEvent.BUTTON_BACK -> 3
        MotionEvent.BUTTON_FORWARD -> 4
        else -> 0
    }

    /** Coordonnées de la vue → pixels de l'écran de l'ordinateur. */
    private fun viewToHost(x: Float, y: Float): Pair<Double, Double>? {
        val d = state.display.value ?: return null
        val v = surfaceView
        if (v.width == 0 || v.height == 0) return null
        val lx = (x - v.left) / v.width
        val ly = (y - v.top) / v.height
        return (lx.coerceIn(0f, 1f) * d.width).toDouble() to (ly.coerceIn(0f, 1f) * d.height).toDouble()
    }

    /** Pixels de l'ordinateur par pixel de l'image à l'écran (gain du pavé tactile). */
    private fun hostPerViewPixel(): Double {
        val d = state.display.value ?: return 1.0
        return if (surfaceView.width > 0) d.width.toDouble() / surfaceView.width else 1.0
    }

    private fun placePointer(p: PointerState) {
        val d = state.display.value
        // Manette ou télécommande en souris : toujours dessiné (Windows cache le sien sans souris branchée).
        val forced = ui.padMouse || ui.remoteMouse
        if (d == null || !p.visible || (p.remoteHidden && !forced) || relative || d.width == 0) {
            pointerView.place(0f, 0f, false)
            return
        }
        val v = surfaceView
        pointerView.place((v.left + p.x / d.width * v.width).toFloat(), (v.top + p.y / d.height * v.height).toFloat(), true)
    }

    // MARK: - Clavier, télécommande, manettes

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val down = event.action == KeyEvent.ACTION_DOWN
        val code = event.keyCode
        // Une touche n'est pas un clic de souris (voir `share`).
        if (down) mouseClick = false
        // Volume, alimentation : au système.
        if (code == KeyEvent.KEYCODE_VOLUME_UP || code == KeyEvent.KEYCODE_VOLUME_DOWN || code == KeyEvent.KEYCODE_VOLUME_MUTE || code == KeyEvent.KEYCODE_POWER) {
            return super.dispatchKeyEvent(event)
        }
        // Manettes (Retour et Menu d'une manette, comme sa touche Select ou Start, partent au jeu).
        val fromPad = event.device?.let(Gamepads::isGamepad) == true
        if (Gamepads.isGamepadEvent(event) || (fromPad && (code == KeyEvent.KEYCODE_BACK || code == KeyEvent.KEYCODE_MENU))) {
            if (pads.captured || ui.anythingModal()) {
                // Menu ouvert : la manette pilote l'app ; A valide, B ou Retour ferme.
                // Le relâché d'un bouton enfoncé avant (L1 + R1 + Start + Select) ne compte pas,
                // et les autres boutons sont avalés : Android en ferait OK ou Menu.
                if (down) padDowns += code else if (!padDowns.remove(code)) return true
                if (code == KeyEvent.KEYCODE_BUTTON_B || code == KeyEvent.KEYCODE_BACK) { if (!down) closeTop(); return true }
                return when (code) {
                    KeyEvent.KEYCODE_BUTTON_A -> super.dispatchKeyEvent(KeyEvent(event.action, KeyEvent.KEYCODE_DPAD_CENTER))
                    KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN, KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_DPAD_CENTER -> super.dispatchKeyEvent(event)
                    else -> true
                }
            }
            if (!padsRouted()) {
                // Un invité sans les manettes : elles n'envoient rien (le cœur ne lui laisse pas ses touches de manette, même
                // avec le clavier et la souris). Faute de la combinaison, que ces touches forment, Retour, Select et Start
                // ouvrent le menu : une manette seule peut quitter.
                val menuKey = code == KeyEvent.KEYCODE_BACK || code == KeyEvent.KEYCODE_MENU || code == KeyEvent.KEYCODE_BUTTON_SELECT || code == KeyEvent.KEYCODE_BUTTON_START
                if (!down && menuKey && event.repeatCount == 0) toggleMenu(fromPad = true)
                return true
            }
            if (pads.key(event)) return true
            return super.dispatchKeyEvent(event)
        }
        // Retour (télécommande, geste du téléphone) : menu ou barre.
        if (code == KeyEvent.KEYCODE_BACK || code == KeyEvent.KEYCODE_ESCAPE && !isKeyboard(event)) {
            if (!down) {
                if (!closeTop()) {
                    // Retour part à l'ordinateur comme Échap si le réglage le veut ; jamais pour un invité sans le clavier.
                    if (tv && !host.settings.backOpensMenu && ui.keyboardMouseAllowed()) Sc.press(handle, Sc.ESC) else toggleMenu()
                }
            }
            return true
        }
        if (ui.anythingModal()) return super.dispatchKeyEvent(event)
        // Clavier physique : scancode Linux → scancode Windows.
        if (isKeyboard(event)) {
            if (down && event.isCtrlPressed && event.isAltPressed && event.isShiftPressed) {
                when (code) {
                    KeyEvent.KEYCODE_M -> { Native.inputReleaseAll(handle); toggleMenu(); return true }
                    KeyEvent.KEYCODE_Q -> { quit(); return true }
                }
            }
            // Un invité sans le clavier : ses touches ne partent pas.
            if (!ui.keyboardMouseAllowed()) return true
            val sc = Native.scancode(event.scanCode)
            if (sc >= 0) {
                if (event.repeatCount == 0) Native.inputKey(handle, sc, down)
                return true
            }
            // Touche sans code matériel (clavier logiciel, certains claviers Bluetooth) : le texte.
            if (down) event.unicodeChar.takeIf { it > 0 }?.let { Native.inputText(handle, String(Character.toChars(it))); return true }
            return super.dispatchKeyEvent(event)
        }
        // Télécommande (flèches et Entrée vers l'ordinateur : pas pour un invité sans le clavier et la souris).
        if (!ui.keyboardMouseAllowed()) return true
        if (remote.key(event)) return true
        return super.dispatchKeyEvent(event)
    }

    /** Les manettes de l'appareil partent à l'ordinateur : toujours pour l'hôte, pour un invité qui a les manettes. */
    private fun padsAllowed() = guest == null || state.shareRole.value.pads

    /**
     * Les manettes de l'appareil sont lues par l'app : elles partent à l'ordinateur ([padsAllowed]), ou elles
     * pilotent la souris (« Manette en souris », que seul un invité qui a le clavier et la souris peut mettre).
     */
    private fun padsRouted() = padsAllowed() || pads.asMouse

    /** Ce qui prend les touches. La barre pilotée à la manette passe par `pads.captured`. */
    private fun SessionUi.anythingModal() = menuOpen || keysOpen || typing || shareOpen || qualityOpen

    private fun isKeyboard(e: KeyEvent): Boolean {
        val d = e.device ?: return false
        return !d.isVirtual && d.keyboardType == InputDevice.KEYBOARD_TYPE_ALPHABETIC && e.scanCode > 0
    }

    override fun dispatchGenericMotionEvent(event: MotionEvent): Boolean {
        if (event.isFromSource(InputDevice.SOURCE_JOYSTICK) && event.action == MotionEvent.ACTION_MOVE) {
            if (!pads.captured && !ui.anythingModal() && padsRouted() && pads.motion(event)) return true
            return super.dispatchGenericMotionEvent(event)
        }
        if (event.isFromSource(InputDevice.SOURCE_MOUSE) && !ui.anythingModal() && ui.keyboardMouseAllowed()) {
            if (!mouseSeen) {
                mouseSeen = true
                surfaceView.pointerIcon = PointerIcon.getSystemIcon(this, PointerIcon.TYPE_NULL)
                updateMouseMode()
            }
            val h = handle
            when (event.actionMasked) {
                MotionEvent.ACTION_HOVER_MOVE, MotionEvent.ACTION_MOVE -> viewToHost(event.x, event.y)?.let { (x, y) -> Native.pointerMoveTo(h, x, y) }
                MotionEvent.ACTION_BUTTON_PRESS -> Native.inputMouseButton(h, button(event.actionButton), true)
                MotionEvent.ACTION_BUTTON_RELEASE -> Native.inputMouseButton(h, button(event.actionButton), false)
                MotionEvent.ACTION_SCROLL -> Native.inputWheel(h, event.getAxisValue(MotionEvent.AXIS_HSCROLL), -event.getAxisValue(MotionEvent.AXIS_VSCROLL))
                else -> return super.dispatchGenericMotionEvent(event)
            }
            return true
        }
        return super.dispatchGenericMotionEvent(event)
    }

    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        if (ev.actionMasked == MotionEvent.ACTION_DOWN) mouseClick = ev.isFromSource(InputDevice.SOURCE_MOUSE)
        // Une souris (pas un doigt) qui clique hors capture : boutons gérés par dispatchGenericMotionEvent.
        // La manette à l'écran ne prend que les doigts : la souris reste une souris.
        if (ev.isFromSource(InputDevice.SOURCE_MOUSE) && !ui.covering) {
            val h = handle
            when (ev.actionMasked) {
                MotionEvent.ACTION_MOVE -> if (ui.keyboardMouseAllowed()) viewToHost(ev.x, ev.y)?.let { (x, y) -> Native.pointerMoveTo(h, x, y) }
            }
            return true
        }
        return super.dispatchTouchEvent(ev)
    }
}
