package fr.leslignesclaires.kybernow

import android.app.Application
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.media.AudioDeviceInfo
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.test.core.app.ApplicationProvider
import com.github.takahirom.roborazzi.captureRoboImage
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.HostInfo
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.model.AudioOutput
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepad
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.LastSession
import fr.leslignesclaires.kybernow.model.PadAxes
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.Second
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.gamepads.GamepadsContent
import fr.leslignesclaires.kybernow.ui.gamepads.PadLive
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
import fr.leslignesclaires.kybernow.ui.home.ReadyInfo
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Captures des écrans principaux, sans appareil ni bibliothèque native
 * (données de démonstration, fausse image d'ordinateur) :
 * `./gradlew recordRoborazziDebug`, images dans app/screenshots/.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [35], application = Application::class)
class Screenshots {
    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    /** Une fausse image d'ordinateur : fond, fenêtres, barre des tâches. */
    private fun desktop(w: Int = 1600, h: Int = 900): Bitmap {
        val b = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        val c = Canvas(b)
        val p = Paint(Paint.ANTI_ALIAS_FLAG)
        p.shader = android.graphics.LinearGradient(0f, 0f, w.toFloat(), h.toFloat(), 0xFF1D3B5C.toInt(), 0xFF3B2A5C.toInt(), android.graphics.Shader.TileMode.CLAMP)
        c.drawRect(0f, 0f, w.toFloat(), h.toFloat(), p)
        p.shader = null
        p.color = 0xFF202633.toInt(); c.drawRoundRect(w * 0.12f, h * 0.12f, w * 0.62f, h * 0.7f, 14f, 14f, p)
        p.color = 0xFF2A3142.toInt(); c.drawRoundRect(w * 0.4f, h * 0.25f, w * 0.88f, h * 0.82f, 14f, 14f, p)
        p.color = 0xFF11151D.toInt(); c.drawRect(0f, h - h * 0.06f, w.toFloat(), h.toFloat(), p)
        return b
    }

    private val atelier = Host(
        id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab:cd:ef:01", user = "matthieu",
        os = "Windows 11 Pro 24H2", gpu = "NVIDIA GeForce RTX 3080 Ti", settings = fr.leslignesclaires.kybernow.model.HostSettings(profile = Profile.GAME),
        last = LastSession(8.3, 11.9, 4320, 0),
    )
    private val portable = Host(id = "b", name = "Portable", address = "100.72.4.9", port = 41234, fingerprint = null, user = "m", os = "Windows 11", gpu = "Intel Arc")

    private val pads = listOf(
        Gamepad(1, "d1", "DualSense Wireless Controller", 0x054C, 0x0CE6, 2, 80, 0, false),
        Gamepad(2, "d2", "Xbox Wireless Controller", 0x045E, 0x0B13, 1, 45, 1, false),
        Gamepad(3, "d3", "NVIDIA Controller v01.04", 0x0955, 0x7214, 1, 70, 2, false),
        Gamepad(4, "d4", "Pro Controller", 0x057E, 0x2009, 0, 60, 3, false),
    )

    private fun ready() = ReadyInfo(
        pads.take(2),
        AudioOutput("", AudioDeviceInfo.TYPE_HDMI, 8, false),
        Screen(ScreenMode(1, 3840, 2160, 60), listOf(ScreenMode(1, 3840, 2160, 60), ScreenMode(2, 1920, 1080, 120), ScreenMode(3, 1920, 1080, 60)), true, false),
        Decoders(DecoderInfo("OMX.Nvidia.h264.decode", 4096, 2160, true), DecoderInfo("OMX.Nvidia.h265.decode", 4096, 2160, true), null, false),
    )

    private val noActions = HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {})

    private fun home(form: FormFactor) = HomeState(
        hosts = listOf(atelier, portable),
        selectedId = "a",
        status = mapOf("a" to HostStatus(true, HostInfo("ATELIER", "Windows 11 Pro 24H2", "NVIDIA GeForce RTX 3080 Ti", false, "0.4.4", true, true)), "b" to HostStatus(false)),
        images = mapOf("a" to desktop().asImageBitmap(), "b" to desktop(800, 450).asImageBitmap()),
        announced = listOf(Announced("salon", "Salon-PC", "192.168.1.34", 40911, null)),
        ready = ready().let { if (form == FormFactor.TV) it else it.copy(audio = AudioOutput("", AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, 2, true)) },
    )

    private fun shot(name: String, form: FormFactor, content: @Composable () -> Unit) =
        captureRoboImage("screenshots/$name.png") { KnTheme(form) { content() } }

    @Test
    @Config(qualifiers = "w960dp-h540dp-xhdpi-television")
    fun tvHome() = shot("tv-accueil", FormFactor.TV) { HomeContent(home(FormFactor.TV), noActions) }

    @Test
    @Config(qualifiers = "w411dp-h914dp-port-xxhdpi")
    fun phoneHome() = shot("telephone-accueil", FormFactor.PHONE) { HomeContent(home(FormFactor.PHONE), noActions) }

    @Test
    @Config(qualifiers = "w1280dp-h800dp-land-xhdpi")
    fun tabletHome() = shot("tablette-accueil", FormFactor.TABLET) { HomeContent(home(FormFactor.TABLET), noActions) }

    @Test
    @Config(qualifiers = "w960dp-h540dp-xhdpi-television")
    fun tvGamepads() = shot("tv-manettes", FormFactor.TV) {
        GamepadsContent(pads, PadLive(1, setOf(Pad.A, Pad.L1), PadAxes(18000, -9000, 0, 0, 23000, 0, 0, 0)), { _, _ -> }, {})
    }

    private fun session(tv: Boolean, setup: SessionUi.() -> Unit): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.second.value = Second(8.1, 3.4, 120, 0, 0, 0, 46.0, doubleArrayOf(1.2, 2.1, 0.4, 1.6, 1.9, 0.9))
        state.mode.value = Triple(1920, 1080, 120)
        state.videoSize.value = 1920 to 1080
        state.decoder.value = "OMX.Nvidia.h265.decode"
        state.audioOutput.value = 8 to 10.7
        val actions = object : SessionActions {
            override fun resume() {}
            override fun quit() {}
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
            override fun openKeyboard(open: Boolean) {}
        }
        return SessionUi("Atelier", tv, state, actions).apply { passageDone = true; setup() }
    }

    @Composable
    private fun WithImage(ui: SessionUi) {
        Box(Modifier.fillMaxSize()) {
            Image(desktop().asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            SessionOverlay(ui)
        }
    }

    @Test
    @Config(qualifiers = "w960dp-h540dp-xhdpi-television")
    fun tvMenu() = shot("tv-menu", FormFactor.TV) { WithImage(session(true) { menuOpen = true }) }

    @Test
    @Config(qualifiers = "w960dp-h540dp-xhdpi-television")
    fun tvPassage() = shot("tv-passage", FormFactor.TV) {
        WithImage(session(true) { passageDone = false; state.phase.value = Phase.CONNECTING })
    }

    @Test
    @Config(qualifiers = "w914dp-h411dp-land-xxhdpi")
    fun phoneSession() = shot("telephone-session", FormFactor.PHONE) { WithImage(session(false) { barOpen = true; statsOpen = true }) }

    @Test
    @Config(qualifiers = "w914dp-h411dp-land-xxhdpi")
    fun phoneKeys() = shot("telephone-touches", FormFactor.PHONE) { WithImage(session(false) { keysOpen = true }) }
}
