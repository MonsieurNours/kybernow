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
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.Native
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
 * aucun menu n'est ouvert : la vue d'interface est retirée (GONE).
 */
class SessionActivity : AppCompatActivity(), SurfaceHolder.Callback, SessionActions {
    companion object {
        const val EXTRA_HOST = "host"
    }

    private val app get() = KyberNowApp.instance
    private lateinit var host: Host
    private lateinit var state: SessionState
    private lateinit var ui: SessionUi
    private lateinit var surfaceView: SurfaceView
    private lateinit var pointerView: PointerView
    private lateinit var overlay: ComposeView
    private lateinit var pads: PadRouter
    private lateinit var remote: RemoteRouter
    private lateinit var touch: TouchInput
    private val main = Handler(Looper.getMainLooper())

    @Volatile private var handle = 0L
    private var refresh = 60
    private var sessionMode = Triple(1920, 1080, 60)
    private var tv = false
    private var relative = false
    private var mouseSeen = false
    /** Boutons de manette enfoncés depuis l'ouverture d'un menu : le relâché des autres (le combo) est ignoré. */
    private val padDowns = mutableSetOf<Int>()
    private var quitting = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        host = intent.getStringExtra(EXTRA_HOST)?.let { app.hosts.get(it) } ?: run { finish(); return }
        tv = app.form == FormFactor.TV
        if (app.form == FormFactor.PHONE) requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }

        state = SessionState(this, app.gamepads)
        pads = PadRouter({ handle }, app.gamepads).apply { guide = host.settings.guide }
        remote = RemoteRouter { handle }
        ui = SessionUi(host.name, tv, state, this).apply {
            profile = host.settings.profile
            mouseMode = host.settings.mouse
            directTouch = host.settings.directTouch
            backOpensMenu = host.settings.backOpensMenu
            combo = host.settings.combo
        }
        touch = TouchInput({ handle }, ::viewToHost, ::hostPerViewPixel).apply { direct = host.settings.directTouch }

        val root = FrameLayout(this)
        surfaceView = SurfaceView(this).apply {
            holder.addCallback(this@SessionActivity)
            isFocusable = true
            isFocusableInTouchMode = true
            setOnTouchListener { _, e -> if (e.isFromSource(InputDevice.SOURCE_TOUCHSCREEN)) touch.onTouch(e) else false }
            setOnCapturedPointerListener { _, e -> capturedMouse(e) }
        }
        root.addView(surfaceView, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT, Gravity.CENTER))
        pointerView = PointerView(this, if (tv) 2f else 1f)
        root.addView(pointerView, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        overlay = ComposeView(this).apply { setContent { KnTheme(app.form) { SessionOverlay(ui) } } }
        root.addView(overlay, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        setContentView(root)

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
        // Le même calcul que l'accueil (« Écran de … »).
        sessionMode = SessionMode.pick(screen, host.settings, settings.optString("codec"), app.decoders)
        applyDisplay()
        val (w, h, r) = sessionMode
        val device = JSONObject(app.device.json())
        val params = JSONObject()
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
        if (handle == 0L) finish()
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
                        // Une manette branchée en cours de session : annoncée à l'ordinateur et dans une capsule.
                        val added = pads.sync()
                        if (state.firstImageAt.get() > 0) added.firstOrNull()?.let { p -> state.show(Capsule.GamepadConnected(p.name, p.slot, p.motors)) }
                    }
                }
                launch {
                    snapshotFlow { ui.anything }.combine(state.capsule) { a, c -> a || c != null || state.ended.value?.error != null }
                        .collect { visible -> overlay.visibility = if (visible) View.VISIBLE else View.GONE }
                }
                launch { state.ended.collect { if (it?.error != null) overlay.visibility = View.VISIBLE } }
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
        if (handle != 0L) Native.sessionSetBackground(handle, true, 2_000_000)
        pads.mouse.stop()
    }

    override fun onResume() {
        super.onResume()
        if (handle != 0L) Native.sessionSetBackground(handle, false, 0)
    }

    override fun onDestroy() {
        super.onDestroy()
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
        ui.typing = false
        ui.statsOpen = false
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
        captureLastImage { Native.sessionStop(h) }
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
        ui.statsOpen && !ui.menuOpen -> { ui.statsOpen = false; true }
        ui.keyboardOpen -> { openKeyboard(false); true }
        ui.menuOpen -> { resume(); true }
        ui.barOpen -> { ui.barOpen = false; true }
        else -> false
    }

    override fun resume() {
        ui.menuOpen = false
        ui.keysOpen = false
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
        if (!mouseSeen) return
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
            if (pads.key(event)) return true
            return super.dispatchKeyEvent(event)
        }
        // Retour (télécommande, geste du téléphone) : menu ou barre.
        if (code == KeyEvent.KEYCODE_BACK || code == KeyEvent.KEYCODE_ESCAPE && !isKeyboard(event)) {
            if (!down) {
                if (!closeTop()) {
                    if (tv && !host.settings.backOpensMenu) Sc.press(handle, Sc.ESC) else toggleMenu()
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
            val sc = Native.scancode(event.scanCode)
            if (sc >= 0) {
                if (event.repeatCount == 0) Native.inputKey(handle, sc, down)
                return true
            }
            // Touche sans code matériel (clavier logiciel, certains claviers Bluetooth) : le texte.
            if (down) event.unicodeChar.takeIf { it > 0 }?.let { Native.inputText(handle, String(Character.toChars(it))); return true }
            return super.dispatchKeyEvent(event)
        }
        // Télécommande.
        if (remote.key(event)) return true
        return super.dispatchKeyEvent(event)
    }

    /** Ce qui prend les touches. La barre pilotée à la manette passe par `pads.captured`. */
    private fun SessionUi.anythingModal() = menuOpen || keysOpen || typing

    private fun isKeyboard(e: KeyEvent): Boolean {
        val d = e.device ?: return false
        return !d.isVirtual && d.keyboardType == InputDevice.KEYBOARD_TYPE_ALPHABETIC && e.scanCode > 0
    }

    override fun dispatchGenericMotionEvent(event: MotionEvent): Boolean {
        if (event.isFromSource(InputDevice.SOURCE_JOYSTICK) && event.action == MotionEvent.ACTION_MOVE) {
            if (!pads.captured && !ui.anythingModal() && pads.motion(event)) return true
            return super.dispatchGenericMotionEvent(event)
        }
        if (event.isFromSource(InputDevice.SOURCE_MOUSE) && !ui.anythingModal()) {
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
        // Une souris (pas un doigt) qui clique hors capture : boutons gérés par dispatchGenericMotionEvent.
        if (ev.isFromSource(InputDevice.SOURCE_MOUSE) && !ui.anything) {
            val h = handle
            when (ev.actionMasked) {
                MotionEvent.ACTION_MOVE -> viewToHost(ev.x, ev.y)?.let { (x, y) -> Native.pointerMoveTo(h, x, y) }
            }
            return true
        }
        return super.dispatchTouchEvent(ev)
    }
}
