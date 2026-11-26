package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.model.AutoProfile
import fr.leslignesclaires.kybernow.model.AutoStep
import fr.leslignesclaires.kybernow.model.AutoSummary
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.ScreenMode
import fr.leslignesclaires.kybernow.model.SessionMode
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.DecoderInfo
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Profil Auto (docs/link-ui.md, « Profil Auto et débit adaptatif ») : ce que l'app garde du test et en fait. */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = android.app.Application::class)
class AutoProfileTest {
    /** La Shield au téléviseur : 4K à 60 Hz, 1080p à 120 Hz. */
    private val tv = Screen(
        ScreenMode(1, 3840, 2160, 60),
        listOf(ScreenMode(1, 3840, 2160, 60), ScreenMode(2, 1920, 1080, 120), ScreenMode(3, 1920, 1080, 60)),
        minimalPostProcessing = true, variableRefresh = false,
    )

    private fun result(codec: String = "hevc", w: Int = 3840, h: Int = 2160, hz: Int = 60, network: String? = "Wi-Fi", reliable: Boolean = false, link: String = "wifi") =
        JSONObject()
            .put("at", 1_795_000_000L).put("network", network ?: JSONObject.NULL)
            .put("measures", JSONObject().put("link", link).put("decode", org.json.JSONArray()))
            .put("codec", codec).put("yuv444", false).put("width", w).put("height", h).put("refresh", hz)
            .put("mbps", 120).put("mode_mbps", 120).put("reliable", reliable)
            .toString()

    @Test
    fun appareilPourLeTest() {
        val o = JSONObject(AutoProfile.deviceJson(tv, JSONObject().put("hevc", true), hdr = false, network = "Ethernet", wired = true))
        val screen = o.getJSONObject("screen")
        assertEquals(3840, screen.getInt("width"))
        assertEquals(2160, screen.getInt("height"))
        assertEquals(60, screen.getInt("refresh"))
        assertTrue(o.getJSONObject("caps").getBoolean("hevc"))
        assertEquals("Ethernet", o.getString("network"))
        assertTrue(o.getBoolean("wired"))
        // Un téléphone en portrait : l'écran en paysage, comme la session le prend (« Comme cet écran »).
        val phone = Screen(ScreenMode(1, 1080, 2400, 60), listOf(ScreenMode(1, 1080, 2400, 60), ScreenMode(2, 1080, 2400, 120)), false, true)
        val d = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 7680, 4320, true), null, hevc444 = false)
        assertEquals(SessionMode.pick(phone, HostSettings(), "hevc", d), AutoProfile.screenMode(phone))
        assertEquals(SessionMode.pick(tv, HostSettings(), "hevc", d), AutoProfile.screenMode(tv))
        assertTrue(JSONObject(AutoProfile.deviceJson(phone, JSONObject(), false, null, false)).isNull("network"))
    }

    @Test
    fun appliquerUnTest() {
        val before = HostSettings(profile = Profile.DESKTOP, codec = "h264", mbps = 40, transport = "fast", resolution = 1080, refresh = 120, adaptive = false)
        // Comme cet écran : « Comme cet écran », fréquence « Auto ».
        val same = AutoProfile.apply(before, result(), tv)
        assertEquals(Profile.AUTO, same.profile)
        assertFalse("Auto n'est pas « Personnalisé »", same.custom)
        assertEquals(0, same.resolution)
        assertEquals(0, same.refresh)
        assertTrue("le débit adaptatif revient", same.adaptive)
        assertEquals(result(), same.autoResult)
        // Décodage limité : 1080p à 120 Hz, la plus haute fréquence de l'écran à cette définition.
        val lower = AutoProfile.apply(before, result(w = 1920, h = 1080, hz = 120), tv)
        assertEquals(1080, lower.resolution)
        assertEquals(0, lower.refresh)
        val slower = AutoProfile.apply(before, result(w = 1920, h = 1080, hz = 60), tv)
        assertEquals(60, slower.refresh)
        // Le transport du test devient celui du profil.
        val reliable = AutoProfile.apply(before, result(reliable = true), tv)
        assertTrue(reliable.reliable)
        assertEquals("auto", reliable.transport)
        assertEquals("auto", reliable.withTransport(true).transport)
        // Gardé avec l'ordinateur.
        assertEquals(reliable, HostSettings.parse(reliable.json()))
        assertEquals(Profile.AUTO, HostSettings.parse(JSONObject().put("profile", "auto")).profile)
        assertTrue("allumé par défaut", HostSettings.parse(JSONObject()).adaptive)
    }

    @Test
    fun plafondDeLEncodeur() {
        // Gardé avec l'ordinateur ; absent, nul ou 0 : aucun.
        val set = HostSettings(encoderMax = 200)
        assertEquals(200, HostSettings.parse(set.json()).encoderMax)
        assertNull(HostSettings.parse(JSONObject()).encoderMax)
        assertNull(HostSettings.parse(HostSettings().json()).encoderMax)
        assertNull(HostSettings.parse(JSONObject().put("encoderMax", 0)).encoderMax)
        // Hors du profil : jamais « Personnalisé ».
        assertFalse(HostSettings(profile = Profile.AUTO, encoderMax = 150).custom)
        // Remettre Auto avec le résultat gardé (accueil, profil synchronisé) ne touche pas au plafond d'une session.
        assertEquals(200, AutoProfile.apply(set, result(), tv).encoderMax)
        // « Appliquer » un nouveau test : `next_encoder_max` du cœur ; sans bibliothèque native (ici), celui d'avant.
        assertEquals(200, AutoProfile.applyTest(set, result(), tv).encoderMax)
        assertNull(AutoProfile.applyTest(HostSettings(), result(), tv).encoderMax)
        assertEquals(Profile.AUTO, AutoProfile.applyTest(set, result(), tv).profile)
    }

    @Test
    fun resultatLu() {
        val s = AutoSummary.parse(result(codec = "pyrowave", network = null, link = "cable"))!!
        assertEquals("pyrowave", s.codec)
        assertNull(s.network)
        assertEquals("cable", s.link)
        assertNull(AutoSummary.parse("illisible"))
        assertNull(AutoSummary.parse(null))
        assertEquals("pyrowave", AutoProfile.codec(result(codec = "pyrowave")))
    }

    @Test
    fun formatDeLaSession() {
        val d = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 7680, 4320, true), DecoderInfo("av1", 7680, 4320, true), hevc444 = false)
        val host = Host(name = "PC", address = "1.2.3.4", port = 1, fingerprint = null, user = "u", av1 = true)
        val auto = host.copy(settings = HostSettings(profile = Profile.AUTO, autoResult = result(codec = "av1")))
        assertEquals("av1", SessionMode.codec(auto, d))
        val pw = host.copy(settings = HostSettings(profile = Profile.AUTO, autoResult = result(codec = "pyrowave")))
        assertEquals("PyroWave se juge comme HEVC", "hevc", SessionMode.codec(pw, d))
        assertEquals("sans test : celui du profil", "hevc", SessionMode.codec(host.copy(settings = HostSettings(profile = Profile.AUTO)), d))
    }

    @Test
    fun avancementDuTest() {
        val steps = AutoStep.parseProgress(
            """{"steps":[{"step":"link","title":"Liaison","state":"done","text":"Câble"},{"step":"latency","title":"Latence au repos","state":"running"},""" +
                """{"step":"bandwidth","title":"Débit vers cet appareil","state":"skipped","text":"Mettez à jour PC pour mesurer le débit."},{"step":"decode","title":"Décodage de cet appareil","state":"waiting"},""" +
                """{"step":"encoder","title":"Encodeur de l'ordinateur","state":"waiting"}]}""",
        )
        assertEquals(listOf("link", "latency", "bandwidth", "decode", "encoder"), steps.map { it.key })
        assertEquals(listOf(AutoStep.State.DONE, AutoStep.State.RUNNING, AutoStep.State.SKIPPED, AutoStep.State.WAITING, AutoStep.State.WAITING), steps.map { it.state })
        assertEquals("Câble", steps[0].text)
        assertNull(steps[1].text)
        assertTrue(AutoStep.parseProgress("pas du JSON").isEmpty())
    }

    @Test
    fun nomDuWifi() {
        assertEquals("Maison", AutoProfile.ssidName("\"Maison\""))
        assertNull(AutoProfile.ssidName("<unknown ssid>"))
        assertNull(AutoProfile.ssidName("\"\""))
    }
}
