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
import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.core.StationInfo
import fr.leslignesclaires.kybernow.core.StationUpdate
import fr.leslignesclaires.kybernow.core.WakeTarget
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.model.AudioOutput
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepad
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.LastSession
import fr.leslignesclaires.kybernow.model.PadAxes
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.session.Ended
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.Second
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.Shortcut
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.StationAction
import fr.leslignesclaires.kybernow.ui.StationFailure
import fr.leslignesclaires.kybernow.ui.StationTask
import fr.leslignesclaires.kybernow.ui.gamepads.GamepadsContent
import fr.leslignesclaires.kybernow.ui.gamepads.PadLive
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeContent
import fr.leslignesclaires.kybernow.ui.home.HomeState
import fr.leslignesclaires.kybernow.ui.settings.AppSettingsActions
import fr.leslignesclaires.kybernow.ui.settings.AppSettingsContent
import fr.leslignesclaires.kybernow.ui.settings.AppSettingsState
import fr.leslignesclaires.kybernow.ui.settings.FingerprintCheck
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsActions
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsContent
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsState
import fr.leslignesclaires.kybernow.ui.settings.Rubric
import fr.leslignesclaires.kybernow.update.UpdateState
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private const val TV = "w960dp-h540dp-land-television-xhdpi"
private const val PHONE = "w411dp-h914dp-port-xxhdpi"
/** Le téléphone, assez haut pour une rubrique entière (la page défile sur l'appareil). */
private const val PHONE_PAGE = "w411dp-h1400dp-port-xhdpi"
/** Assez haut pour les réglages de l'app entiers. */
private const val PHONE_TALL = "w411dp-h1900dp-port-xhdpi"
private const val PHONE_LAND = "w914dp-h411dp-land-xxhdpi"
private const val TABLET = "w1280dp-h800dp-land-xhdpi"
private const val TABLET_PORTRAIT = "w800dp-h1280dp-port-xhdpi"

/**
 * Captures des écrans, sans appareil ni bibliothèque native (données de démonstration, fausse
 * image d'ordinateur) : `./gradlew recordRoborazziDebug`, images dans app/screenshots/, nommées
 * `<écran>-<fr|en>-<taille>.png` (taille : telephone, tablette, tv).
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

    private val now = System.currentTimeMillis()
    private val last = LastSession(8.9, 14.2, 42 * 60, now - 24 * 3600 * 1000L, 2560, 1440, 120, "hevc", 80)

    private val ethernet = WakeTarget("3C:7C:3F:12:AB:CD", "192.168.1.20", "192.168.1.255", "ethernet", true, "Intel(R) Ethernet Connection I219-V")
    private val atelier = Host(
        id = "a", name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab:cd:ef:01:23:45:67:89", user = "matthieu",
        os = "Windows 11 Pro 24H2", gpu = "NVIDIA GeForce RTX 3080 Ti", settings = HostSettings(profile = Profile.GAME), last = last, wake = listOf(ethernet),
    )
    /** Hors ligne ; ses cartes réseau, apprises d'Admin, permettent de le réveiller. */
    private val portable = Host(
        id = "b", name = "PC-DE-MATTHIEU", address = "100.72.4.9", port = 41234, fingerprint = "sha256:12:34:56:78", user = "m",
        os = "Windows 11 Famille", gpu = "Intel Arc", wake = listOf(WakeTarget("A1:B2:C3:D4:E5:F6", "192.168.1.31", "192.168.1.255", "wifi", true, "Wi-Fi")),
    )
    private val bureau = Host(id = "c", name = "Bureau", address = "192.168.1.40", port = 41234, fingerprint = null, user = "m", os = "Windows 10 Pro")

    private val pads = listOf(
        Gamepad(1, "d1", "DualSense Wireless Controller", 0x054C, 0x0CE6, 2, 80, 0, false),
        Gamepad(2, "d2", "Xbox Wireless Controller", 0x045E, 0x0B13, 1, 45, 1, false),
        Gamepad(3, "d3", "NVIDIA Controller v01.04", 0x0955, 0x7214, 1, 70, 2, false),
        Gamepad(4, "d4", "Pro Controller", 0x057E, 0x2009, 0, 60, 3, false),
    )

    private fun screen(form: FormFactor) = when (form) {
        FormFactor.TV -> Screen(ScreenMode(1, 3840, 2160, 60), listOf(ScreenMode(1, 3840, 2160, 60), ScreenMode(2, 1920, 1080, 120), ScreenMode(3, 1920, 1080, 60)), true, false)
        FormFactor.PHONE -> Screen(ScreenMode(1, 1080, 2400, 120), listOf(ScreenMode(1, 1080, 2400, 120), ScreenMode(2, 1080, 2400, 60)), false, true)
        FormFactor.TABLET -> Screen(ScreenMode(1, 2560, 1600, 120), listOf(ScreenMode(1, 2560, 1600, 120), ScreenMode(2, 2560, 1600, 60)), false, true)
    }

    private fun decoders(form: FormFactor) = if (form == FormFactor.TV) {
        Decoders(DecoderInfo("OMX.Nvidia.h264.decode", 4096, 2160, true), DecoderInfo("OMX.Nvidia.h265.decode", 4096, 2160, true), null, false)
    } else {
        Decoders(DecoderInfo("c2.qti.avc.decoder", 8192, 4320, true), DecoderInfo("c2.qti.hevc.decoder", 8192, 4320, true), DecoderInfo("c2.qti.av1.decoder", 8192, 4320, true), false)
    }

    private fun station(version: String = "0.5.0", update: StationUpdate = StationUpdate(), restarting: Boolean = false) = StationInfo(version, update, restarting)

    private fun info(station: StationInfo? = station(), name: String = "ATELIER") =
        HostInfo(name, "Windows 11 Pro 24H2", "NVIDIA GeForce RTX 3080 Ti", false, station?.version ?: "0.4.9", true, true, station)

    private val noActions = HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {})

    private fun home(form: FormFactor, selected: String = "a", status: Map<String, HostStatus>? = null, hosts: List<Host> = listOf(atelier, portable, bureau)) = HomeState(
        hosts = hosts,
        selectedId = selected,
        status = status ?: mapOf("a" to HostStatus(true, info()), "b" to HostStatus(false), "c" to HostStatus()),
        images = mapOf("a" to desktop().asImageBitmap(), "b" to desktop(800, 450).asImageBitmap()),
        announced = listOf(Announced("salon", "Salon-PC", "192.168.1.34", 40911, null)),
        screen = screen(form),
        decoders = decoders(form),
        pads = 2,
    )

    /** Une capture par langue : `<nom>-fr-<taille>.png`, `<nom>-en-<taille>.png`. */
    private fun shot(name: String, size: String, form: FormFactor, langs: List<String> = listOf("fr", "en"), content: @Composable () -> Unit) {
        for (lang in langs) {
            RuntimeEnvironment.setQualifiers("+$lang")
            // Sans animations sans fin : Robolectric n'y retrouverait jamais le repos.
            captureRoboImage("screenshots/$name-$lang-$size.png") { KnTheme(form, motion = false) { content() } }
        }
    }

    // MARK: - Accueil

    @Test @Config(qualifiers = TV)
    fun homeTv() {
        shot("accueil", "tv", FormFactor.TV) { HomeContent(home(FormFactor.TV), noActions) }
        shot("accueil-hors-ligne", "tv", FormFactor.TV) { HomeContent(home(FormFactor.TV, selected = "b"), noActions) }
        shot("accueil-reveil", "tv", FormFactor.TV, listOf("fr")) { HomeContent(home(FormFactor.TV, selected = "b").copy(waking = setOf("b")), noActions) }
        shot("accueil-erreurs", "tv", FormFactor.TV) {
            val custom = atelier.copy(settings = atelier.settings.copy(codec = "h264", mbps = 40))
            HomeContent(home(FormFactor.TV, hosts = listOf(custom, portable), status = mapOf("a" to HostStatus(true, null, certificateChanged = true))).copy(update = "0.5.1"), noActions)
        }
        shot("accueil-vide", "tv", FormFactor.TV) { HomeContent(home(FormFactor.TV, hosts = emptyList()), noActions) }
    }

    @Test @Config(qualifiers = PHONE)
    fun homePhone() {
        shot("accueil", "telephone", FormFactor.PHONE) { HomeContent(home(FormFactor.PHONE), noActions) }
        shot("accueil-hors-ligne", "telephone", FormFactor.PHONE) { HomeContent(home(FormFactor.PHONE, selected = "b").copy(update = "0.5.1"), noActions) }
        shot("accueil-reveil", "telephone", FormFactor.PHONE) { HomeContent(home(FormFactor.PHONE, selected = "b").copy(waking = setOf("b")), noActions) }
        shot("accueil-erreurs", "telephone", FormFactor.PHONE) {
            val custom = atelier.copy(settings = atelier.settings.copy(transport = "reliable"))
            HomeContent(home(FormFactor.PHONE, hosts = listOf(custom, portable), status = mapOf("a" to HostStatus(true, null, accountRefused = true))), noActions)
        }
        shot("accueil-recherche", "telephone", FormFactor.PHONE, listOf("fr")) { HomeContent(home(FormFactor.PHONE, selected = "c"), noActions) }
        shot("accueil-vide", "telephone", FormFactor.PHONE) { HomeContent(home(FormFactor.PHONE, hosts = emptyList()), noActions) }
    }

    @Test @Config(qualifiers = TABLET_PORTRAIT)
    fun homeTabletPortrait() {
        shot("accueil-portrait", "tablette", FormFactor.TABLET) { HomeContent(home(FormFactor.TABLET), noActions) }
        shot("accueil-erreurs-portrait", "tablette", FormFactor.TABLET, listOf("fr")) {
            val custom = atelier.copy(settings = atelier.settings.copy(codec = "h264"))
            HomeContent(home(FormFactor.TABLET, hosts = listOf(custom, portable), status = mapOf("a" to HostStatus(true, null, accountRefused = true))), noActions)
        }
    }

    @Test @Config(qualifiers = TABLET)
    fun homeTablet() {
        shot("accueil", "tablette", FormFactor.TABLET) { HomeContent(home(FormFactor.TABLET), noActions) }
        shot("accueil-hors-ligne", "tablette", FormFactor.TABLET) {
            HomeContent(home(FormFactor.TABLET, selected = "b", hosts = listOf(atelier, portable.copy(wake = emptyList()), bureau)), noActions)
        }
        shot("accueil-vide", "tablette", FormFactor.TABLET) { HomeContent(home(FormFactor.TABLET, hosts = emptyList()), noActions) }
    }

    // MARK: - Réglages de l'ordinateur

    private val settingsActions = HostSettingsActions({}, { _, _ -> }, {}, {}, { _, _ -> }, {}, {}, {}, {})

    private fun settings(form: FormFactor, host: Host = atelier, status: HostStatus = HostStatus(true, info()), task: StationTask? = null, check: FingerprintCheck? = null) =
        HostSettingsState(host, status, screen(form), decoders(form), "https://${host.address}:${host.port}/", task, check, now)

    /** Chaque rubrique, et les états de Statistiques et de Station, à une taille. */
    private fun settingsShots(size: String, form: FormFactor) {
        @Composable fun page(state: HostSettingsState, r: Rubric) = HostSettingsContent(state, settingsActions, start = r, only = r)
        val names = mapOf(
            Rubric.PICTURE to "image", Rubric.SOUND to "son", Rubric.CONTROLS to "commandes",
            Rubric.NETWORK to "reseau", Rubric.STATS to "statistiques", Rubric.STATION to "station",
        )
        Rubric.entries.forEach { r -> shot("reglages-${names[r]}", size, form) { page(settings(form), r) } }
        shot("reglages-reseau-sans-adresse", size, form) { page(settings(form, host = bureau, status = HostStatus(true, info(station = null))), Rubric.NETWORK) }
        shot("reglages-reseau-saisie", size, form, listOf("fr")) { page(settings(form, host = bureau.copy(mac = "A1:B2:C3:D4:E5:F6")), Rubric.NETWORK) }
        shot("reglages-statistiques-vide", size, form) { page(settings(form, host = atelier.copy(last = null, fingerprint = null)), Rubric.STATS) }
        shot("reglages-statistiques-empreinte", size, form, listOf("fr")) {
            page(settings(form, status = HostStatus(true, null, certificateChanged = true), check = FingerprintCheck("sha256:9f:3c:71:0a:44:e2:5b:c8")), Rubric.STATS)
        }
        val available = info(station(update = StationUpdate(available = true, version = "0.5.1", installable = true)))
        shot("reglages-station-mise-a-jour", size, form) { page(settings(form, status = HostStatus(true, available)), Rubric.STATION) }
        shot("reglages-station-installateur", size, form, listOf("fr")) {
            page(settings(form, status = HostStatus(true, info(station(update = StationUpdate(available = true, version = "0.6.0", requiresSetup = true))))), Rubric.STATION)
        }
        shot("reglages-station-confirmation", size, form) { page(settings(form, status = HostStatus(true, available), task = StationTask.Confirm(StationAction.UPDATE, "Pixel 8")), Rubric.STATION) }
        shot("reglages-station-redemarrage", size, form) { page(settings(form, task = StationTask.Working(StationAction.RESTART, null)), Rubric.STATION) }
        shot("reglages-station-en-cours", size, form, listOf("fr")) {
            page(settings(form, status = HostStatus(true, info(station(update = StationUpdate(available = true, version = "0.5.1", installable = true, installing = true))))), Rubric.STATION)
        }
        shot("reglages-station-faite", size, form) { page(settings(form, status = HostStatus(true, info(station("0.5.1"))), task = StationTask.Done(StationAction.UPDATE, "0.5.1")), Rubric.STATION) }
        shot("reglages-station-trop-tot", size, form, listOf("fr")) { page(settings(form, task = StationTask.Failed(StationFailure.TOO_SOON)), Rubric.STATION) }
        shot("reglages-station-echec-mise-a-jour", size, form) { page(settings(form, task = StationTask.Failed(StationFailure.UPDATE_FAILED)), Rubric.STATION) }
        shot("reglages-station-tarde", size, form, listOf("fr")) { page(settings(form, task = StationTask.Failed(StationFailure.NOT_BACK)), Rubric.STATION) }
        shot("reglages-station-acces-propre", size, form) { page(settings(form, status = HostStatus(true, info().copy(ownDevice = true))), Rubric.STATION) }
        shot("reglages-station-ancienne", size, form) { page(settings(form, status = HostStatus(true, info(station = null))), Rubric.STATION) }
        shot("reglages-station-hors-ligne", size, form, listOf("fr")) { page(settings(form, host = portable.copy(paired = true), status = HostStatus(false)), Rubric.STATION) }
    }

    @Test @Config(qualifiers = TV)
    fun settingsTv() = settingsShots("tv", FormFactor.TV)

    @Test @Config(qualifiers = PHONE_PAGE)
    fun settingsPhone() = settingsShots("telephone", FormFactor.PHONE)

    /** Le haut de la page, à la taille du téléphone : le titre, le profil, le début de l'image. */
    @Test @Config(qualifiers = PHONE)
    fun settingsPhoneTop() = shot("reglages", "telephone", FormFactor.PHONE) { HostSettingsContent(settings(FormFactor.PHONE), settingsActions) }

    @Test @Config(qualifiers = TABLET)
    fun settingsTablet() = settingsShots("tablette", FormFactor.TABLET)

    // MARK: - Réglages de l'app

    private fun appSettings(form: FormFactor) = AppSettingsState(
        language = "auto",
        deviceName = when (form) { FormFactor.TV -> "SHIELD Android TV"; FormFactor.PHONE -> "Pixel 8"; FormFactor.TABLET -> "Galaxy Tab S9" },
        deviceOs = if (form == FormFactor.TV) "Android 11" else "Android 15",
        decoders = decoders(form),
        screen = screen(form),
        audio = if (form == FormFactor.TV) AudioOutput("", AudioDeviceInfo.TYPE_HDMI, 8, false) else AudioOutput("", AudioDeviceInfo.TYPE_BLUETOOTH_A2DP, 2, true),
        update = UpdateState.Available("0.5.1", "", "", 31_000_000),
        beta = false,
        version = "0.5.0",
    )

    private val appActions = AppSettingsActions({}, {}, {}, {}, {})

    @Test @Config(qualifiers = TV)
    fun appSettingsTv() = shot("reglages-app", "tv", FormFactor.TV) { AppSettingsContent(appSettings(FormFactor.TV), appActions) }

    @Test @Config(qualifiers = PHONE_TALL)
    fun appSettingsPhone() = shot("reglages-app", "telephone", FormFactor.PHONE) { AppSettingsContent(appSettings(FormFactor.PHONE), appActions) }

    @Test @Config(qualifiers = TABLET)
    fun appSettingsTablet() = shot("reglages-app", "tablette", FormFactor.TABLET) { AppSettingsContent(appSettings(FormFactor.TABLET), appActions) }

    @Test @Config(qualifiers = TV)
    fun tvGamepads() = shot("manettes", "tv", FormFactor.TV) {
        GamepadsContent(pads, PadLive(1, setOf(Pad.A, Pad.L1), PadAxes(18000, -9000, 0, 0, 23000, 0, 0, 0)), { _, _ -> }, {})
    }

    // MARK: - Connexion et session

    private fun session(tv: Boolean, setup: SessionUi.() -> Unit): SessionUi {
        val state = SessionState(context, Gamepads(context))
        state.phase.value = Phase.STREAMING
        state.second.value = Second(8.1, 3.4, 120, 0, 0, 0, 46.0, doubleArrayOf(1.2, 2.1, 0.4, 1.6, 1.9, 0.9))
        state.mode.value = Triple(1920, 1080, 120)
        state.videoSize.value = 1920 to 1080
        state.decoder.value = "OMX.Nvidia.h265.decode"
        state.audioOutput.value = 8 to 10.7
        state.firstImageAt.set(1)
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

    /** L'écran de connexion, à la deuxième étape. */
    private fun connecting(tv: Boolean) = session(tv) { passageDone = false; state.phase.value = Phase.CONNECTING; state.firstImageAt.set(0) }

    /** Dernier recours : la connexion n'aboutissait pas, le cœur redémarre la Station. */
    private fun restarting(tv: Boolean) = session(tv) {
        passageDone = false
        state.phase.value = Phase.CONNECTING
        state.firstImageAt.set(0)
        state.stationRestarting.value = true
    }

    /** La connexion a échoué avant la première image (le cœur écrit la raison dans la langue de l'app). */
    @Composable
    private fun failed(tv: Boolean): SessionUi {
        val fr = androidx.compose.ui.platform.LocalConfiguration.current.locales[0].language == "fr"
        return session(tv) {
            passageDone = false
            state.phase.value = Phase.ENDED
            state.firstImageAt.set(0)
            state.ended.value = Ended(if (fr) "l'ordinateur ne répond plus" else "the computer is no longer responding", null, null, null)
        }
    }

    @Composable
    private fun WithImage(ui: SessionUi) {
        Box(Modifier.fillMaxSize()) {
            Image(desktop().asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
            SessionOverlay(ui)
        }
    }

    @Test @Config(qualifiers = TV)
    fun sessionTv() {
        shot("connexion", "tv", FormFactor.TV) { WithImage(connecting(true)) }
        shot("connexion-echec", "tv", FormFactor.TV) { WithImage(failed(true)) }
        shot("connexion-redemarrage", "tv", FormFactor.TV) { WithImage(restarting(true)) }
        shot("session", "tv", FormFactor.TV, listOf("fr")) { WithImage(session(true) {}) }
        shot("session-menu", "tv", FormFactor.TV) { WithImage(session(true) { menuOpen = true }) }
    }

    @Test @Config(qualifiers = PHONE_LAND)
    fun sessionPhone() {
        shot("connexion", "telephone", FormFactor.PHONE) { WithImage(connecting(false)) }
        shot("connexion-echec", "telephone", FormFactor.PHONE) { WithImage(failed(false)) }
        shot("connexion-redemarrage", "telephone", FormFactor.PHONE) { WithImage(restarting(false)) }
        shot("session", "telephone", FormFactor.PHONE, listOf("fr")) { WithImage(session(false) {}) }
        shot("session-barre", "telephone", FormFactor.PHONE) { WithImage(session(false) { barOpen = true; statsOpen = true }) }
        shot("session-touches", "telephone", FormFactor.PHONE, listOf("en")) { WithImage(session(false) { keysOpen = true }) }
    }

    @Test @Config(qualifiers = TABLET)
    fun sessionTablet() {
        shot("connexion", "tablette", FormFactor.TABLET) { WithImage(connecting(false)) }
        shot("session", "tablette", FormFactor.TABLET, listOf("fr")) { WithImage(session(false) {}) }
    }
}
