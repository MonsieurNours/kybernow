package fr.leslignesclaires.kybernow.session

import android.content.Context
import android.content.pm.ActivityInfo
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
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
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.core.PsListener
import fr.leslignesclaires.kybernow.core.PsNative
import fr.leslignesclaires.kybernow.core.RouteInfo
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.PadAxes
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.playstation.PsConsole
import fr.leslignesclaires.kybernow.playstation.PsKeys
import fr.leslignesclaires.kybernow.playstation.PsRemote
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.playstation.PsHdr
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import org.json.JSONObject
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * Une partie sur une console PlayStation (docs/playstation.md) : l'image plein cadre sur une `SurfaceView` (décodée
 * par MediaCodec dans le cœur, comme un ordinateur), le son par AAudio, la manette de l'appareil (ou la manette à
 * l'écran, ou le clavier et la souris) vue par la console comme une manette PlayStation. L'écran de connexion, la
 * barre, le menu du téléviseur, les statistiques et la carte de fin sont ceux d'une session d'ordinateur
 * (`SessionOverlay`, avec [ConsoleUi]).
 */
class PsSessionActivity : AppCompatActivity(), SurfaceHolder.Callback, SessionActions {
    companion object {
        const val EXTRA_CONSOLE = "console"
        const val EXTRA_ADDRESS = "address"
        /** Hors de chez soi : la console jointe par une Station est remise en mémoire par `KyberNowApp.handOverRemote`. */
        const val EXTRA_REMOTE = "remote"
        private const val PAD_PREFS = "playstation_session"
    }

    private val app get() = KyberNowApp.instance
    private lateinit var console: PsConsole
    private var address = ""
    private var remote: PsRemote? = null
    private lateinit var state: SessionState
    private lateinit var ui: SessionUi
    private lateinit var cui: ConsoleUi
    private lateinit var surfaceView: SurfaceView
    private lateinit var overlay: ComposeView
    private lateinit var haptics: Haptics
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var handle = 0L
    private var refresh = 60
    private var tv = false
    private var quitting = false
    /** La barre (ou le menu) a la manette : seuls les relâchements partent à la console. */
    private var captured = false
    private val padDowns = mutableSetOf<Int>()
    private val lastAxes = HashMap<Int, PadAxes>()
    /** Places branchées dans le lecteur du cœur : place → manette (descripteur). */
    private val plugged = HashMap<Int, String>()
    private var mouseCaptured = false

    /** La manette à l'écran vers la console, à sa place de joueur. */
    private val onScreenOut = object : PadOut {
        private fun slot() = app.gamepads.pads.value.firstOrNull { it.onScreen }?.slot ?: -1
        override fun button(button: Int, pressed: Boolean) {
            val s = slot()
            if (s >= 0 && !(pressed && captured)) PsNative.padButton(s, button, pressed)
        }
        override fun axis(axis: Int, value: Int) {
            val s = slot()
            if (s >= 0 && !(value != 0 && captured)) PsNative.padAxis(s, axis, value)
        }
        override fun feel(f: Feel) = haptics.feel(f)
    }

    private val prefs get() = getSharedPreferences(PAD_PREFS, Context.MODE_PRIVATE)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        console = intent.getStringExtra(EXTRA_CONSOLE)?.let { app.ps.store.console(it) } ?: run { finish(); return }
        address = intent.getStringExtra(EXTRA_ADDRESS) ?: console.address
        if (intent.getBooleanExtra(EXTRA_REMOTE, false)) remote = app.takeRemote() ?: run { finish(); return }
        tv = app.form == FormFactor.TV
        if (app.form == FormFactor.PHONE) requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
        state = SessionState(this, app.gamepads)
        state.computerName = console.name
        cui = ConsoleUi(
            onMute = { on -> cui.muted = on; PsNative.setMuted(handle, on) },
            onRestMode = { PsNative.gotoBed(handle) },
            onKeyboardText = { t -> PsNative.keyboardText(handle, t) },
            onKeyboardDone = { ok -> keyboardDone(ok) },
        )
        ui = SessionUi(console.name, tv, state, this, OnScreenPad(onScreenOut), console = cui).apply {
            combo = true
            backOpensMenu = true
            // Téléphone et tablette : la manette à l'écran, la première fois (la console n'a qu'une manette).
            padShown = !tv && prefs.getBoolean("pad_shown", true)
            pad.onSwitch = { setPadLayout(!pad.simple) }
            pad.setLayout(prefs.getBoolean("pad_simple", false))
        }
        val root = FrameLayout(this)
        surfaceView = SurfaceView(this).apply {
            holder.addCallback(this@PsSessionActivity)
            isFocusable = true
            isFocusableInTouchMode = true
            setOnCapturedPointerListener { _, e -> capturedMouse(e) }
        }
        root.addView(surfaceView, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT, Gravity.CENTER))
        overlay = ComposeView(this).apply { setContent { KnTheme(app.form) { SessionOverlay(ui) } } }
        root.addView(overlay, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
        setContentView(root)
        root.addOnLayoutChangeListener { _, l, t, r, b, ol, ot, or, ob ->
            if (r - l != or - ol || b - t != ob - ot) root.post { state.videoSize.value?.let { (w, h) -> fitSurface(w, h) } }
        }
        haptics = Haptics(overlay)
        if (ui.padShown) app.gamepads.setOnScreen(true)
        remote?.let { r -> PsNative.remoteRoute(r.handle)?.let { state.route.value = RouteInfo.parse(it) } }
        start()
        observe()
    }

    private fun settingsJson(): String {
        val o = JSONObject(console.settings ?: PsNative.defaultSettings())
        // La puce HDR de l'accueil, seulement sur un écran et un décodeur HDR.
        o.put("hdr", console.hdr && console.ps5 && PsHdr.screen(this) && PsHdr.decoder)
        cui.fps = o.optInt("fps", 60)
        return o.toString()
    }

    private fun start() {
        val reg = app.ps.store.registered(console) ?: run { finish(); return }
        refresh = display?.refreshRate?.roundToInt()?.coerceAtLeast(30) ?: 60
        state.phase.value = Phase.CONNECTING
        handle = PsNative.start(address, reg, settingsJson(), app.decoders.hevc != null, null, listener, refresh)
        if (handle == 0L) {
            finish()
            return
        }
        console.layout?.let { PsNative.setLayout(handle, it) }
    }

    private fun observe() {
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.CREATED) {
                launch { state.videoSize.collect { it?.let { (w, h) -> fitSurface(w, h) } } }
                launch {
                    state.ended.collect { e ->
                        if (e == null) return@collect
                        if (e.error == null || quitting) finishSession()
                    }
                }
                launch { app.gamepads.pads.collectLatest { syncPads() } }
                launch {
                    snapshotFlow { ui.anything || cui.keyboard != null || cui.cantDisplay }.combine(state.capsule) { a, c -> a || c != null || state.ended.value?.error != null }
                        .collect { visible -> overlay.visibility = if (visible) View.VISIBLE else View.GONE }
                }
                launch { state.ended.collect { if (it?.error != null) overlay.visibility = View.VISIBLE } }
                launch {
                    // La barre ou le menu s'ouvre : la console reçoit une manette au repos ; ils se ferment : la manette lui revient.
                    snapshotFlow { ui.barOpen || ui.menuOpen }.collect { open -> setCaptured(open) }
                }
                launch {
                    // Les intensités de la DualSense : tout de suite, même pendant la partie.
                    app.ps.store.consoles.collect { list ->
                        val c = list.firstOrNull { it.id == console.id } ?: return@collect
                        val o = JSONObject(c.settings ?: PsNative.defaultSettings())
                        PsNative.setIntensities(handle, o.optInt("rumble", 100), o.optInt("triggers", 100))
                        if (c.layout != console.layout) PsNative.setLayout(handle, c.layout ?: PsNative.defaultLayout())
                        console = c
                    }
                }
                launch {
                    // Les mesures, une fois par seconde (puce de la barre, panneau des statistiques).
                    while (isActive) {
                        delay(1000)
                        val st = PsStats.parse(PsNative.stats(handle)) ?: continue
                        cui.stats = st
                        cui.lostTotal += st.lost
                        cui.recoveredTotal += st.recovered
                        state.second.value = Second(null, null, st.fps.roundToInt(), st.lost.toInt(), 0, 0, st.mbps, null)
                        remote?.let { r -> PsNative.remoteRoute(r.handle)?.let { state.route.value = RouteInfo.parse(it) } }
                    }
                }
            }
        }
        app.gamepads.start()
    }

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

    // MARK: - Rappels de la partie (fils du cœur)

    private val listener = object : PsListener {
        override fun onPsState(state: Int, width: Int, height: Int, hdr: Boolean, detail: String) {
            val st = this@PsSessionActivity.state
            when (state) {
                PsNative.CONNECTING -> st.phase.value = Phase.CONNECTING
                PsNative.STREAMING -> if (st.phase.value != Phase.STREAMING) st.phase.value = Phase.FIRST_IMAGES
                PsNative.CANT_DISPLAY -> main.post { cui.cantDisplay = true }
                PsNative.CAN_DISPLAY -> main.post { cui.cantDisplay = false }
                else -> {
                    if (detail.isNotEmpty()) android.util.Log.i("KyberNow", "PlayStation : fin de la partie ($state, $detail)")
                    val text = when (state) {
                        PsNative.END_IN_USE -> getString(R.string.ps_end_in_use)
                        PsNative.END_NOT_PAIRED -> getString(R.string.ps_end_not_paired)
                        PsNative.END_UNREACHABLE -> getString(R.string.ps_end_unreachable)
                        PsNative.END_REMOTE -> if (quitting) null else getString(R.string.ps_end_remote)
                        else -> null
                    }
                    // La console ne reconnaît plus ce compte : cette association est effacée (« Associer » revient si c'était la dernière).
                    if (state == PsNative.END_NOT_PAIRED) main.post { app.ps.store.chosenAccount(console)?.let { app.ps.store.unpair(console, it.id) } }
                    main.post { cui.keyboard = null }
                    st.phase.value = Phase.ENDED
                    st.ended.value = Ended(text, null, null, null)
                }
            }
        }

        override fun onPsMenu() {
            main.post { toggleMenu(fromPad = true) }
        }

        // La barre se pilote par les touches d'Android (Compose), pas par le cœur.
        override fun onPsNavigate(key: Int) {}

        override fun onPsKeyboard(event: Int, text: String) {
            main.post {
                cui.keyboard = when (event) {
                    PsNative.KEYBOARD_OPEN, PsNative.KEYBOARD_TEXT -> text
                    else -> null
                }
                if (cui.keyboard == null) surfaceView.requestFocus()
            }
        }

        override fun onPsRumble(pad: Int, large: Int, small: Int, durationMs: Int) {
            val g = app.gamepads.pads.value.firstOrNull { it.slot == pad } ?: return
            if (g.onScreen) {
                if (ui.padShown) haptics.rumble(large, small, durationMs)
            } else {
                app.gamepads.rumble(g.deviceId, large, small, durationMs)
            }
        }

        override fun onDecoder(name: String) = state.onDecoder(name)

        override fun onVideoSize(width: Int, height: Int) {
            state.onVideoSize(width, height)
            if (state.phase.value != Phase.STREAMING && state.ended.value == null) {
                state.firstImageAt.compareAndSet(0, System.currentTimeMillis())
                state.phase.value = Phase.STREAMING
            }
        }

        override fun onAudioOutput(channels: Int, bufferMs: Double) = state.onAudioOutput(channels, bufferMs)
    }

    // MARK: - Surface

    override fun surfaceCreated(holder: SurfaceHolder) {
        val s = holder.surface
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            s.setFrameRate(refresh.toFloat(), Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE, Surface.CHANGE_FRAME_RATE_ALWAYS)
        } else {
            s.setFrameRate(refresh.toFloat(), Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE)
        }
        PsNative.setSurface(handle, s, refresh)
        surfaceView.requestFocus()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        PsNative.setSurface(handle, null, 0)
    }

    // MARK: - Cycle de vie

    override fun onPause() {
        super.onPause()
        ui.pad.releaseAll()
        haptics.stop()
        PsNative.releaseAll(handle)
        releasePads()
    }

    override fun onDestroy() {
        super.onDestroy()
        if (!::state.isInitialized) return
        if (::haptics.isInitialized) haptics.stop()
        app.gamepads.setOnScreen(false)
        app.gamepads.stop()
        state.close()
        val h = handle
        handle = 0L
        val r = remote
        remote = null
        Thread({
            if (h != 0L) PsNative.stop(h)
            r?.close()
        }, "kybernow-fin").start()
    }

    override fun quit() {
        if (quitting) {
            finishSession()
            return
        }
        quitting = true
        ui.menuOpen = false
        ui.barOpen = false
        ui.statsOpen = false
        ui.pad.releaseAll()
        if (handle == 0L || state.ended.value != null) {
            finishSession()
            return
        }
        ui.leaving = true
        val h = handle
        // La partie s'arrête sur un autre fil (elle attend la fin de son fil) ; la fin arrive par `onPsState`.
        Thread({ PsNative.stop(h); main.post { finishSession() } }, "kybernow-fin").start()
        handle = 0L
    }

    private fun finishSession() {
        if (isFinishing) return
        finish()
    }

    // MARK: - Barre et menu

    private fun toggleMenu(fromPad: Boolean = false) {
        if (tv) {
            if (ui.menuOpen) resume() else {
                padDowns.clear()
                ui.menuOpen = true
            }
        } else if (fromPad) {
            if (ui.padBar) ui.barOpen = false else {
                padDowns.clear()
                surfaceView.requestFocusFromTouch()
                ui.padBar = true
                ui.barOpen = true
            }
        } else {
            ui.barOpen = !ui.barOpen
        }
    }

    /** La barre ou le menu prend la manette : tout est relâché chez la console, puis seuls les relâchements partent. */
    private fun setCaptured(on: Boolean) {
        if (captured == on) return
        captured = on
        if (on) releasePads()
        if (!on) {
            ui.padBar = false
            lastAxes.clear()
            if (cui.keyboard == null) surfaceView.requestFocus()
        }
        PsNative.setCaptured(handle, on)
    }

    private fun releasePads() {
        for (slot in plugged.keys) {
            for (b in Pad.A..Pad.RIGHT) PsNative.padButton(slot, b, false)
            for (a in Pad.LEFT_X..Pad.R2) PsNative.padAxis(slot, a, 0)
        }
    }

    private fun closeTop(): Boolean = when {
        ui.statsOpen && !ui.menuOpen -> { ui.statsOpen = false; true }
        ui.menuOpen -> { resume(); true }
        ui.barOpen -> { ui.barOpen = false; true }
        else -> false
    }

    override fun resume() {
        ui.menuOpen = false
        padDowns.clear()
        surfaceView.requestFocus()
    }

    /** Clavier de la console : Entrée valide, Échap annule ; le bandeau se retire tout de suite. */
    private fun keyboardDone(accept: Boolean) {
        if (accept) PsNative.keyboardAccept(handle) else PsNative.keyboardReject(handle)
        cui.keyboard = null
        val imm = getSystemService(android.view.inputmethod.InputMethodManager::class.java)
        imm?.hideSoftInputFromWindow(surfaceView.windowToken, 0)
        surfaceView.requestFocus()
    }

    // MARK: - Manettes

    /** Les manettes de l'appareil, données au lecteur du cœur à leur place (la manette à l'écran comprise). */
    private fun syncPads() {
        val now = app.gamepads.pads.value.filter { it.slot >= 0 }.associateBy { it.slot }
        for ((slot, desc) in plugged.toMap()) {
            if (now[slot]?.descriptor != desc) {
                PsNative.padUnplug(slot)
                plugged.remove(slot)
            }
        }
        now.values.filter { plugged[it.slot] == null }.forEach {
            PsNative.padPlug(it.slot, it.vendorId, it.productId)
            plugged[it.slot] = it.descriptor
        }
    }

    private fun slot(deviceId: Int): Int = app.gamepads.byDevice(deviceId)?.slot ?: run { app.gamepads.refresh(); app.gamepads.byDevice(deviceId)?.slot ?: -1 }

    private fun padKey(e: KeyEvent): Boolean {
        if (e.repeatCount > 0) return true
        app.gamepads.noteUsed(e.deviceId)
        val down = e.action == KeyEvent.ACTION_DOWN
        val s = slot(e.deviceId)
        if (s < 0) return true
        val trigger = Gamepads.digitalTrigger(e.keyCode)
        if (trigger >= 0) {
            if (!(down && captured)) PsNative.padAxis(s, trigger, if (down) 32767 else 0)
            return true
        }
        val b = Gamepads.button(e.keyCode)
        if (b < 0) return false
        if (!(down && captured)) PsNative.padButton(s, b, down)
        return true
    }

    private fun padMotion(e: MotionEvent): Boolean {
        app.gamepads.noteUsed(e.deviceId)
        if (captured) return false
        val s = slot(e.deviceId)
        if (s < 0) return true
        val now = PadAxes.read(e)
        val before = lastAxes[e.deviceId] ?: PadAxes.REST
        lastAxes[e.deviceId] = now
        for (axis in Pad.LEFT_X..Pad.R2) {
            val v = now.value(axis)
            if (v != before.value(axis)) PsNative.padAxis(s, axis, v)
        }
        if (now.hatX != before.hatX) {
            if (before.hatX < 0) PsNative.padButton(s, Pad.LEFT, false)
            if (before.hatX > 0) PsNative.padButton(s, Pad.RIGHT, false)
            if (now.hatX < 0) PsNative.padButton(s, Pad.LEFT, true)
            if (now.hatX > 0) PsNative.padButton(s, Pad.RIGHT, true)
        }
        if (now.hatY != before.hatY) {
            if (before.hatY < 0) PsNative.padButton(s, Pad.UP, false)
            if (before.hatY > 0) PsNative.padButton(s, Pad.DOWN, false)
            if (now.hatY < 0) PsNative.padButton(s, Pad.UP, true)
            if (now.hatY > 0) PsNative.padButton(s, Pad.DOWN, true)
        }
        return true
    }

    // MARK: - Clavier et souris

    private fun isKeyboard(e: KeyEvent): Boolean {
        val d = e.device ?: return false
        return !d.isVirtual && d.keyboardType == InputDevice.KEYBOARD_TYPE_ALPHABETIC && e.scanCode > 0
    }

    private fun capturedMouse(e: MotionEvent): Boolean {
        when (e.actionMasked) {
            MotionEvent.ACTION_MOVE, MotionEvent.ACTION_HOVER_MOVE -> {
                var dx = 0f
                var dy = 0f
                for (i in 0 until e.historySize) {
                    dx += e.getHistoricalAxisValue(MotionEvent.AXIS_RELATIVE_X, i)
                    dy += e.getHistoricalAxisValue(MotionEvent.AXIS_RELATIVE_Y, i)
                }
                dx += e.getAxisValue(MotionEvent.AXIS_RELATIVE_X)
                dy += e.getAxisValue(MotionEvent.AXIS_RELATIVE_Y)
                PsNative.mouseMove(handle, dx, dy)
            }
            MotionEvent.ACTION_BUTTON_PRESS -> PsNative.mouseButton(handle, mouseButton(e.actionButton), true)
            MotionEvent.ACTION_BUTTON_RELEASE -> PsNative.mouseButton(handle, mouseButton(e.actionButton), false)
        }
        return true
    }

    private fun mouseButton(b: Int) = when (b) {
        MotionEvent.BUTTON_SECONDARY -> 1
        MotionEvent.BUTTON_TERTIARY -> 2
        MotionEvent.BUTTON_BACK -> 3
        MotionEvent.BUTTON_FORWARD -> 4
        else -> 0
    }

    private fun setMouseCaptured(on: Boolean) {
        if (mouseCaptured == on) return
        mouseCaptured = on
        if (on) surfaceView.requestPointerCapture() else {
            surfaceView.releasePointerCapture()
            PsNative.releaseAll(handle)
        }
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        val down = event.action == KeyEvent.ACTION_DOWN
        val code = event.keyCode
        if (code == KeyEvent.KEYCODE_VOLUME_UP || code == KeyEvent.KEYCODE_VOLUME_DOWN || code == KeyEvent.KEYCODE_VOLUME_MUTE || code == KeyEvent.KEYCODE_POWER) {
            return super.dispatchKeyEvent(event)
        }
        // Le clavier de la console est ouvert : les touches écrivent dans son champ (Échap annule).
        if (cui.keyboard != null && !Gamepads.isGamepadEvent(event)) {
            if (code == KeyEvent.KEYCODE_ESCAPE) {
                if (!down) keyboardDone(false)
                return true
            }
            if (code == KeyEvent.KEYCODE_BACK && !down) {
                keyboardDone(false)
                return true
            }
            return super.dispatchKeyEvent(event)
        }
        val fromPad = event.device?.let(Gamepads::isGamepad) == true
        if (Gamepads.isGamepadEvent(event) || (fromPad && (code == KeyEvent.KEYCODE_BACK || code == KeyEvent.KEYCODE_MENU))) {
            if (captured || ui.menuOpen) {
                // La barre ou le menu : la manette pilote l'app ; Croix (A) valide, Rond (B) ou Retour ferme. Les relâchements
                // partent quand même à la console (rien n'y reste tenu).
                padKey(event)
                if (down) padDowns += code else if (!padDowns.remove(code)) return true
                if (code == KeyEvent.KEYCODE_BUTTON_B || code == KeyEvent.KEYCODE_BACK) { if (!down) closeTop(); return true }
                return when (code) {
                    KeyEvent.KEYCODE_BUTTON_A -> super.dispatchKeyEvent(KeyEvent(event.action, KeyEvent.KEYCODE_DPAD_CENTER))
                    KeyEvent.KEYCODE_DPAD_UP, KeyEvent.KEYCODE_DPAD_DOWN, KeyEvent.KEYCODE_DPAD_LEFT, KeyEvent.KEYCODE_DPAD_RIGHT, KeyEvent.KEYCODE_DPAD_CENTER -> super.dispatchKeyEvent(event)
                    else -> true
                }
            }
            if (padKey(event)) return true
            return super.dispatchKeyEvent(event)
        }
        // Retour (télécommande, geste du téléphone) : la barre ou le menu.
        if (code == KeyEvent.KEYCODE_BACK || (code == KeyEvent.KEYCODE_ESCAPE && !isKeyboard(event))) {
            if (!down && !closeTop()) toggleMenu()
            return true
        }
        if (ui.menuOpen || ui.barOpen) return super.dispatchKeyEvent(event)
        if (isKeyboard(event)) {
            // Ctrl + Alt + Maj + M : rendre la souris (ou ouvrir la barre) ; + D : se déconnecter.
            if (down && event.isCtrlPressed && event.isAltPressed && event.isShiftPressed) {
                when (code) {
                    KeyEvent.KEYCODE_M -> { PsNative.releaseAll(handle); if (mouseCaptured) setMouseCaptured(false) else toggleMenu(); return true }
                    KeyEvent.KEYCODE_D, KeyEvent.KEYCODE_Q -> { quit(); return true }
                }
            }
            val usage = PsKeys.usage(event)
            if (usage >= 0) {
                if (event.repeatCount == 0) PsNative.key(handle, usage, down)
                return true
            }
        }
        return super.dispatchKeyEvent(event)
    }

    override fun dispatchGenericMotionEvent(event: MotionEvent): Boolean {
        if (event.isFromSource(InputDevice.SOURCE_JOYSTICK) && event.action == MotionEvent.ACTION_MOVE) {
            if (!ui.menuOpen && padMotion(event)) return true
            return super.dispatchGenericMotionEvent(event)
        }
        return super.dispatchGenericMotionEvent(event)
    }

    override fun dispatchTouchEvent(ev: MotionEvent): Boolean {
        // Un clic de souris dans l'image : la souris est prise (clavier et souris en manette).
        if (ev.isFromSource(InputDevice.SOURCE_MOUSE) && !ui.covering && ev.actionMasked == MotionEvent.ACTION_DOWN) {
            setMouseCaptured(true)
            return true
        }
        return super.dispatchTouchEvent(ev)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus) {
            mouseCaptured = false
            PsNative.releaseAll(handle)
        }
    }

    // MARK: - SessionActions (ce qui a du sens pour une console)

    override fun setOnScreenPad(on: Boolean) {
        if (tv || ui.padShown == on) return
        if (on) ui.barOpen = false else {
            ui.pad.releaseAll()
            haptics.stop()
        }
        ui.padShown = on
        app.gamepads.setOnScreen(on)
        prefs.edit().putBoolean("pad_shown", on).apply()
    }

    override fun setPadLayout(simple: Boolean) {
        ui.pad.setLayout(simple)
        prefs.edit().putBoolean("pad_simple", simple).apply()
    }

    override fun openKeyboard(open: Boolean) {}
    override fun setProfile(p: Profile) {}
    override fun setMouseMode(m: Int) {}
    override fun setDirectTouch(on: Boolean) {}
    override fun setRemoteMouse(on: Boolean) {}
    override fun setPadMouse(on: Boolean) {}
    override fun shortcut(s: Shortcut) {}
    override fun key(scancode: Int) {}
    override fun text(s: String) {}
    override fun restoreMode() {}
    override fun pickDisplay(id: Int) {}
    override fun setShareQuality(quality: ShareQuality) {}
    override fun share() {}
    override fun shareNewCode() {}
    override fun shareSetPads(guest: ShareGuest, on: Boolean) {}
    override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) {}
    override fun shareAccept(guest: ShareGuest) {}
    override fun shareRefuse(guest: ShareGuest) {}
    override fun shareRemove(guest: ShareGuest) {}
    override fun shareStop() {}
    override fun shareCopyLink() {}
}
