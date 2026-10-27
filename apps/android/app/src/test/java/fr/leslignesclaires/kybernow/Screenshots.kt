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
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.test.core.app.ApplicationProvider
import com.github.takahirom.roborazzi.captureRoboImage
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.HostInfo
import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.core.ShareAvailability
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareImage
import fr.leslignesclaires.kybernow.core.ShareLink
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.core.ShareTarget
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
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.Ended
import fr.leslignesclaires.kybernow.session.Phase
import fr.leslignesclaires.kybernow.session.Second
import fr.leslignesclaires.kybernow.session.SessionActions
import fr.leslignesclaires.kybernow.session.SessionOverlay
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.session.SessionUi
import fr.leslignesclaires.kybernow.session.ShareCopy
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
import fr.leslignesclaires.kybernow.ui.share.JoinContent
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
private const val PHONE_PAGE = "w411dp-h1600dp-port-xhdpi"
/** Assez haut pour les réglages de l'app entiers. */
private const val PHONE_TALL = "w411dp-h1900dp-port-xhdpi"
private const val PHONE_LAND = "w914dp-h411dp-land-xxhdpi"
private const val TABLET = "w1280dp-h800dp-land-xhdpi"
/** Un petit téléphone en paysage (moins de 700 dp de large). */
private const val PHONE_SMALL_LAND = "w640dp-h360dp-land-xxhdpi"
private const val TABLET_PORTRAIT = "w800dp-h1280dp-port-xhdpi"
/** La tablette, assez haute pour la rubrique Commandes entière (elle défile sur l'appareil). */
private const val TABLET_TALL = "w1280dp-h1500dp-xhdpi"

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

    private val noActions = HomeActions({}, { _, _ -> }, {}, {}, {}, {}, {}, {}, {}, {})

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

    private fun session(tv: Boolean, title: String = "Atelier", guest: Boolean = false, setup: SessionUi.() -> Unit): SessionUi {
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
            override fun setOnScreenPad(on: Boolean) {}
            override fun setPadLayout(simple: Boolean) {}
            override fun share() {}
            override fun shareNewCode() {}
            override fun shareSetPads(guest: ShareGuest, on: Boolean) {}
            override fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean) {}
            override fun shareRemove(guest: ShareGuest) {}
            override fun shareStop() {}
            override fun shareCopyLink() {}
            override fun setShareQuality(quality: ShareQuality) {}
        }
        return SessionUi(title, tv, state, actions, guest = guest, computerName = "Atelier").apply { passageDone = true; setup() }
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

    /** Une image d'ordinateur claire (fenêtres blanches) : la manette doit y rester lisible. */
    private fun bright(w: Int = 1600, h: Int = 900): Bitmap {
        val b = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
        val c = Canvas(b)
        val p = Paint(Paint.ANTI_ALIAS_FLAG)
        p.shader = android.graphics.LinearGradient(0f, 0f, w.toFloat(), h.toFloat(), 0xFFBFD9F2.toInt(), 0xFFF2E6D9.toInt(), android.graphics.Shader.TileMode.CLAMP)
        c.drawRect(0f, 0f, w.toFloat(), h.toFloat(), p)
        p.shader = null
        p.color = 0xFFFFFFFF.toInt(); c.drawRoundRect(w * 0.0f, h * 0.2f, w * 0.55f, h * 0.95f, 14f, 14f, p)
        p.color = 0xFFF4F5F7.toInt(); c.drawRoundRect(w * 0.5f, h * 0.1f, w * 1.0f, h * 0.85f, 14f, 14f, p)
        return b
    }

    @Composable
    private fun WithImage(ui: SessionUi, image: Bitmap? = null) {
        Box(Modifier.fillMaxSize()) {
            Image((image ?: desktop()).asImageBitmap(), null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop)
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

    // MARK: - Manette à l'écran et clavier

    /** Des doigts sur la manette : A et la flèche droite enfoncés, le stick gauche poussé, RT à moitié. */
    private fun SessionUi.pressing() {
        pad.pressed = setOf(Pad.A, Pad.RIGHT)
        pad.left = fr.leslignesclaires.kybernow.session.StickState(0.7f, -0.45f, touched = true)
        pad.rt = 0.55f
    }

    private fun padShots(size: String, form: FormFactor) {
        shot("session-manette", size, form) { WithImage(session(false) { padShown = true }) }
        shot("session-manette-simple", size, form) { WithImage(session(false) { padShown = true; pad.setLayout(true) }) }
        shot("session-manette-appui", size, form) { WithImage(session(false) { padShown = true; pressing() }) }
    }

    @Test @Config(qualifiers = PHONE_LAND)
    fun sessionPadPhone() {
        padShots("telephone", FormFactor.PHONE)
        shot("session-manette-petite", "telephone", FormFactor.PHONE) { WithImage(session(false) { padShown = true; padSize = 0 }) }
        shot("session-manette-grande", "telephone", FormFactor.PHONE) { WithImage(session(false) { padShown = true; padSize = 2 }) }
        shot("session-manette-opacite-basse", "telephone", FormFactor.PHONE) { WithImage(session(false) { padShown = true; padOpacity = 30; pressing() }) }
        shot("session-manette-opacite-haute", "telephone", FormFactor.PHONE) { WithImage(session(false) { padShown = true; padOpacity = 100; pressing() }) }
        shot("session-manette-simple-appui", "telephone", FormFactor.PHONE) {
            WithImage(session(false) { padShown = true; pad.setLayout(true); pad.pressed = setOf(Pad.Y, Pad.UP); pad.left = fr.leslignesclaires.kybernow.session.StickState(-0.3f, 0.8f, touched = true, clicked = false) })
        }
        shot("session-manette-barre", "telephone", FormFactor.PHONE) { WithImage(session(false) { padShown = true; barOpen = true }) }
        shot("session-manette-image-claire", "telephone", FormFactor.PHONE, listOf("fr")) { WithImage(session(false) { padShown = true; pressing() }, bright()) }
        shot("session-manette-image-claire-opacite-basse", "telephone", FormFactor.PHONE, listOf("fr")) { WithImage(session(false) { padShown = true; padOpacity = 30 }, bright()) }
        shot("session-barre-manette-cachee", "telephone", FormFactor.PHONE) { WithImage(session(false) { barOpen = true }) }
        shot("session-clavier", "telephone", FormFactor.PHONE) { WithImage(session(false) { keyboardOpen = true }) }
        shot("session-manette-clavier", "telephone", FormFactor.PHONE) { WithImage(session(false) { padShown = true; keyboardOpen = true }) }
    }

    @Test @Config(qualifiers = TABLET)
    fun sessionPadTablet() {
        padShots("tablette", FormFactor.TABLET)
        shot("session-manette-grande", "tablette", FormFactor.TABLET) { WithImage(session(false) { padShown = true; padSize = 2 }) }
        shot("session-manette-barre", "tablette", FormFactor.TABLET) { WithImage(session(false) { padShown = true; barOpen = true }) }
    }

    @Test @Config(qualifiers = TABLET_PORTRAIT)
    fun sessionPadTabletPortrait() {
        shot("session-manette-portrait", "tablette", FormFactor.TABLET) { WithImage(session(false) { padShown = true; pressing() }) }
    }

    /** Commandes, au téléphone et à la tablette, avec la manette à l'écran affichée ; sans vibreur, la ligne qui le dit. */
    @Test @Config(qualifiers = PHONE_TALL)
    fun settingsPadPhone() {
        val shown = atelier.copy(settings = atelier.settings.copy(onScreenPad = true, padLayout = "simple", padSize = 2, padOpacity = 50, padHaptics = 3))
        shot("reglages-commandes-manette", "telephone", FormFactor.PHONE) { HostSettingsContent(settings(FormFactor.PHONE, host = shown), settingsActions, Rubric.CONTROLS, only = Rubric.CONTROLS) }
        shot("reglages-commandes-sans-vibreur", "telephone", FormFactor.PHONE) {
            HostSettingsContent(settings(FormFactor.PHONE).copy(vibrator = false), settingsActions, Rubric.CONTROLS, only = Rubric.CONTROLS)
        }
    }

    // MARK: - KyberNow Share

    /** L'image reçue d'un invité telle que le cœur la décrit : [label] (« 1080p · 60 Hz ») n'existe que si la taille est connue. */
    private fun image(quality: ShareQuality, label: String?, followsHost: Boolean?) = ShareImage(quality, followsHost = followsHost, reason = followsHost?.let { "asked" }, label = label?.replace(" Hz", "\u00a0Hz"))

    /**
     * Un partage ouvert avec deux invités : un spectateur qui suit l'image de l'hôte (aucune qualité sous son nom) et
     * un joueur qui a le clavier et la souris, sur une image à part (« 720p · 30 Hz »).
     */
    private fun SessionUi.sharing(guests: List<ShareGuest> = listOf(
        ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR, since = now / 1000 - 42, image = image(ShareQuality.HOST, "1080p · 60 Hz", followsHost = true)),
        ShareGuest("g2", "Max", "web", ShareRole.PLAYER_KBM, since = now / 1000 - 12 * 60, image = image(ShareQuality.P720_30, "720p · 30 Hz", followsHost = false)),
    )) {
        state.share.value = ShareState("428913", "428 913", "kybernow://join?host=192.168.1.20&port=41234&fp=sha256:ab&code=428913", "https://192.168.1.20:41234/link/?join=428913", guests)
    }

    @Test @Config(qualifiers = PHONE_LAND)
    fun sharePhone() {
        shot("session-partage-barre", "telephone", FormFactor.PHONE) { WithImage(session(false) { barOpen = true; sharing() }) }
        shot("session-partage", "telephone", FormFactor.PHONE) { WithImage(session(false) { barOpen = true; shareOpen = true; sharing() }) }
        shot("session-partage-copie", "telephone", FormFactor.PHONE, listOf("fr")) {
            WithImage(session(false) { barOpen = true; shareOpen = true; sharing(); shareCopy = ShareCopy.COPIED })
        }
        shot("session-partage-joueur", "telephone", FormFactor.PHONE, listOf("fr")) {
            WithImage(session(false) { shareOpen = true; sharing(listOf(ShareGuest("g3", "Hugo", "android", ShareRole.PLAYER, since = now / 1000 - 3700), ShareGuest("g4", "Zoé", "webos", ShareRole.SPECTATOR), ShareGuest("g5", "Eva", "mac", ShareRole.SPECTATOR), ShareGuest("g6", "Tom", "windows", ShareRole.SPECTATOR))) })
        }
        shot("session-partage-vide", "telephone", FormFactor.PHONE, listOf("fr")) { WithImage(session(false) { shareOpen = true; sharing(emptyList()) }) }
        // « Partager » impossible (ici, Station trop ancienne) : estompé, sélectionnable ; au toucher il ouvre le panneau
        // réduit à la raison (le texte du cœur) ; à la souris, l'infobulle la donne.
        shot("session-partage-inactif", "telephone", FormFactor.PHONE) {
            val off = unavailable(ShareAvailability.UNSUPPORTED)
            WithImage(session(false) { barOpen = true; off() })
        }
        shot("session-partage-raison", "telephone", FormFactor.PHONE) {
            val off = unavailable(ShareAvailability.UNSUPPORTED)
            WithImage(session(false) { barOpen = true; shareOpen = true; off() })
        }
    }

    /** « Partager » impossible : ce que l'hôte a lu dans l'info de la Station, avec le texte du cœur dans la langue de l'app. */
    @Composable
    private fun unavailable(which: ShareAvailability): SessionUi.() -> Unit {
        val fr = androidx.compose.ui.platform.LocalConfiguration.current.locales[0].language == "fr"
        val text = when (which) {
            ShareAvailability.UNSUPPORTED -> if (fr) "Le partage demande KyberNow Station 0.8.0 ou plus récente." else "Sharing needs KyberNow Station 0.8.0 or later."
            else -> if (fr) "Le partage est coupé sur cet ordinateur (Admin › Appareils Link)." else "Sharing is turned off on this computer (Admin › Link devices)."
        }
        return { shareAvailability = which; shareUnavailable = text }
    }

    /** Un petit téléphone en paysage : « Partager » n'a que son icône, la pastille reste. */
    @Test @Config(qualifiers = PHONE_SMALL_LAND)
    fun sharePhoneSmall() {
        shot("session-partage-barre", "petit-telephone", FormFactor.PHONE) { WithImage(session(false) { barOpen = true; sharing() }) }
        shot("invite-joueur-clavier-souris", "petit-telephone", FormFactor.PHONE, listOf("fr")) { WithImage(guest(false, ShareRole.PLAYER_KBM) { barOpen = true }) }
    }

    @Test @Config(qualifiers = TV)
    fun shareTv() {
        shot("session-partage-menu", "tv", FormFactor.TV) { WithImage(session(true) { menuOpen = true; sharing() }) }
        shot("session-partage-inactif", "tv", FormFactor.TV) {
            val off = unavailable(ShareAvailability.DISABLED)
            WithImage(session(true) { menuOpen = true; off() })
        }
        shot("session-partage-raison", "tv", FormFactor.TV) {
            val off = unavailable(ShareAvailability.DISABLED)
            WithImage(session(true) { menuOpen = true; shareOpen = true; off() })
        }
        shot("session-partage", "tv", FormFactor.TV) { WithImage(session(true) { menuOpen = true; shareOpen = true; sharing() }) }
    }

    @Test @Config(qualifiers = TABLET)
    fun shareTablet() {
        shot("session-partage", "tablette", FormFactor.TABLET, listOf("fr")) { WithImage(session(false) { barOpen = true; shareOpen = true; sharing() }) }
        // Les deux droits, chacun donné ou repris à part : aucun droit, les manettes, le clavier et la souris, les deux.
        shot("session-partage-droits", "tablette", FormFactor.TABLET) {
            WithImage(session(false) {
                shareOpen = true
                sharing(listOf(
                    ShareGuest("g1", "Léa", "ios", ShareRole.SPECTATOR, since = now / 1000 - 42),
                    ShareGuest("g2", "Hugo", "android", ShareRole.PLAYER, since = now / 1000 - 3700),
                    ShareGuest("g3", "Zoé", "webos", ShareRole.KBM, since = now / 1000 - 5 * 60),
                    ShareGuest("g4", "Max", "web", ShareRole.PLAYER_KBM, since = now / 1000 - 12 * 60),
                ))
            })
        }
    }

    /** La session d'un invité : « Invité sur … », la barre réduite selon son rôle. */
    @Composable
    private fun guest(tv: Boolean, role: ShareRole, setup: SessionUi.() -> Unit = {}): SessionUi =
        session(tv, title = stringResource(R.string.share_guest_on, "Atelier"), guest = true) { state.shareRole.value = role; setup() }

    @Test @Config(qualifiers = PHONE_LAND)
    fun guestPhone() {
        shot("invite-connexion", "telephone", FormFactor.PHONE) { WithImage(guest(false, ShareRole.SPECTATOR) { passageDone = false; state.phase.value = Phase.CONNECTING; state.firstImageAt.set(0) }) }
        shot("invite-spectateur", "telephone", FormFactor.PHONE) { WithImage(guest(false, ShareRole.SPECTATOR) { barOpen = true }) }
        shot("invite-joueur", "telephone", FormFactor.PHONE) { WithImage(guest(false, ShareRole.PLAYER) { barOpen = true }) }
        shot("invite-joueur-clavier-souris", "telephone", FormFactor.PHONE) { WithImage(guest(false, ShareRole.PLAYER_KBM) { barOpen = true }) }
        shot("invite-clavier-souris", "telephone", FormFactor.PHONE) { WithImage(guest(false, ShareRole.KBM) { barOpen = true }) }
        // « Qualité de l'image » : la barre montre la qualité reçue (écrite par le cœur) ; la feuille donne les quatre choix.
        shot("invite-qualite", "telephone", FormFactor.PHONE) {
            WithImage(guest(false, ShareRole.PLAYER) { barOpen = true; state.shareImage.value = image(ShareQuality.AUTO, "1080p · 60 Hz", followsHost = false) })
        }
        shot("invite-qualite-feuille", "telephone", FormFactor.PHONE) {
            WithImage(guest(false, ShareRole.PLAYER) {
                barOpen = true
                qualityOpen = true
                state.shareQuality.value = ShareQuality.P1080_60
                state.shareImage.value = image(ShareQuality.P1080_60, "1080p · 60 Hz", followsHost = false)
            })
        }
        // Tant que la Station ne dit pas la taille de l'image : aucune qualité reçue à montrer.
        shot("invite-qualite-inconnue", "telephone", FormFactor.PHONE, listOf("fr")) {
            WithImage(guest(false, ShareRole.SPECTATOR) { barOpen = true; state.shareImage.value = image(ShareQuality.AUTO, null, followsHost = null) })
        }
        shot("invite-avis-image", "telephone", FormFactor.PHONE) {
            val fr = androidx.compose.ui.platform.LocalConfiguration.current.locales[0].language == "fr"
            WithImage(guest(false, ShareRole.PLAYER) { state.capsule.value = Capsule.ShareNotice(if (fr) "Image allégée pour suivre." else "Picture lightened to keep up.") })
        }
        shot("invite-manette", "telephone", FormFactor.PHONE, listOf("fr")) { WithImage(guest(false, ShareRole.PLAYER) { padShown = true; barOpen = true }) }
        shot("invite-avis-manettes-reprises", "telephone", FormFactor.PHONE, listOf("fr")) {
            WithImage(guest(false, ShareRole.KBM) { state.capsule.value = Capsule.ShareRights(R.string.share_notice_pads_back) })
        }
        shot("invite-avis", "telephone", FormFactor.PHONE) {
            WithImage(guest(false, ShareRole.PLAYER) { state.capsule.value = Capsule.ShareRights(R.string.share_notice_play) })
        }
        shot("invite-fin", "telephone", FormFactor.PHONE) {
            val fr = androidx.compose.ui.platform.LocalConfiguration.current.locales[0].language == "fr"
            WithImage(guest(false, ShareRole.PLAYER) {
                state.phase.value = Phase.ENDED
                state.ended.value = Ended(if (fr) "Léa a arrêté le partage." else "Léa stopped sharing.", null, null, null)
            })
        }
    }

    @Test @Config(qualifiers = TV)
    fun guestTv() {
        shot("invite-connexion", "tv", FormFactor.TV) { WithImage(guest(true, ShareRole.PLAYER) { passageDone = false; state.phase.value = Phase.CONNECTING; state.firstImageAt.set(0) }) }
        shot("invite-spectateur-menu", "tv", FormFactor.TV) { WithImage(guest(true, ShareRole.SPECTATOR) { menuOpen = true }) }
        shot("invite-joueur-clavier-souris-menu", "tv", FormFactor.TV) {
            WithImage(guest(true, ShareRole.PLAYER_KBM) { menuOpen = true; state.shareImage.value = image(ShareQuality.AUTO, "1080p · 60 Hz", followsHost = true) })
        }
        shot("invite-qualite-feuille", "tv", FormFactor.TV) {
            WithImage(guest(true, ShareRole.PLAYER) {
                menuOpen = true
                qualityOpen = true
                state.shareImage.value = image(ShareQuality.AUTO, "1080p · 60 Hz", followsHost = false)
            })
        }
        shot("invite-clavier-souris-menu", "tv", FormFactor.TV, listOf("fr")) { WithImage(guest(true, ShareRole.KBM) { menuOpen = true }) }
    }

    /** « Rejoindre avec un code » : le code et le prénom ; lien ouvert ; pavé du téléviseur. */
    private val shareLink = ShareLink(ShareTarget("192.168.1.20", 41234, "sha256:ab:cd:ef:01:23:45:67:89"), "428913")

    @Test @Config(qualifiers = PHONE)
    fun joinPhone() {
        shot("rejoindre", "telephone", FormFactor.PHONE) { JoinContent(null, true, "Pixel de Léa", ShareQuality.AUTO, false, null, {}, { _, _, _, _ -> }) }
        shot("rejoindre-lien", "telephone", FormFactor.PHONE, listOf("fr")) { JoinContent(shareLink, false, "Pixel de Léa", ShareQuality.P1080_60, false, null, {}, { _, _, _, _ -> }) }
        shot("rejoindre-recherche", "telephone", FormFactor.PHONE, listOf("fr")) { JoinContent(shareLink, false, "Léa", ShareQuality.AUTO, true, null, {}, { _, _, _, _ -> }) }
        shot("rejoindre-refus", "telephone", FormFactor.PHONE) {
            val fr = androidx.compose.ui.platform.LocalConfiguration.current.locales[0].language == "fr"
            JoinContent(shareLink, false, "Léa", ShareQuality.AUTO, false, if (fr) "Aucun partage avec ce code sur le réseau." else "No share with this code on your network.", {}, { _, _, _, _ -> })
        }
    }

    @Test @Config(qualifiers = TV)
    fun joinTv() {
        shot("rejoindre", "tv", FormFactor.TV) { JoinContent(null, false, "SHIELD", ShareQuality.AUTO, false, null, {}, { _, _, _, _ -> }) }
        shot("rejoindre-lien", "tv", FormFactor.TV, listOf("fr")) { JoinContent(shareLink, false, "SHIELD", ShareQuality.P720_30, false, null, {}, { _, _, _, _ -> }) }
    }

    @Test @Config(qualifiers = TABLET_TALL)
    fun settingsPadTablet() {
        shot("reglages-commandes", "tablette", FormFactor.TABLET) { HostSettingsContent(settings(FormFactor.TABLET), settingsActions, Rubric.CONTROLS, only = Rubric.CONTROLS) }
    }
}
