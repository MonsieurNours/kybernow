package fr.leslignesclaires.kybernow

import android.app.Application
import android.view.Display
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.core.SessionListener
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Hdr
import fr.leslignesclaires.kybernow.model.HdrLuminance
import fr.leslignesclaires.kybernow.model.HdrScreen
import fr.leslignesclaires.kybernow.model.HdrState
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.LastSession
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.SessionState
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowLooper

/**
 * Le HDR des sessions d'ordinateur (docs/hdr.md) : écran HDR et ses luminosités, format et décodeur en HDR, réglages
 * envoyés au cœur, choix gardé pour chaque ordinateur (hors du profil), puce lue du cœur, demande d'emprunt, mesures
 * et avis de la session. La règle de la puce elle-même est celle du cœur (`profile::hdr_state`, essayée dans
 * `apps/android/jni/src/hdr.rs`).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class HdrTest {
    private val context get() = ApplicationProvider.getApplicationContext<Application>()

    private fun decoder(name: String) = DecoderInfo(name, 8192, 4320, true)
    private val shield = Decoders(decoder("avc"), decoder("hevc"), null, false, hevcHdr = true)
    private val sdrOnly = Decoders(decoder("avc"), decoder("hevc"), decoder("av1"), false)
    private val av1Only = Decoders(decoder("avc"), null, decoder("av1"), false, av1Hdr = true)
    private fun host(codec: String = "auto", av1: Boolean = false, hdr: Boolean = true) =
        Host(name = "Atelier", address = "192.168.1.20", port = 41234, fingerprint = null, user = "m", av1 = av1, settings = HostSettings(codec = codec, hdr = hdr))

    @Test
    fun luminosites_de_l_ecran() {
        // Rien de lisible : les valeurs par défaut du cœur.
        assertNull(HdrLuminance.of(0f, 0f, 0f))
        assertNull(HdrLuminance.of(-1f, 500f, 0.1f))
        assertNull(HdrLuminance.of(Float.NaN, 500f, 0.1f))
        assertEquals(HdrLuminance(1000.0, 1000.0, 0.0), HdrLuminance.of(1000f, 0f, 0f))
        // Plein écran au plus la maximale ; un noir lu tel quel.
        val l = HdrLuminance.of(800f, 1200f, 0.05f)!!
        assertEquals(800.0, l.fullFrame, 0.0)
        assertEquals(0.05, l.min, 1e-6)
        assertEquals(HdrLuminance.SDR_WHITE, l.sdrWhite, 0.0)
        val o = HdrLuminance(700.0, 350.0, 0.01).json()
        assertEquals(listOf(700.0, 350.0, 0.01, 203.0), listOf("max", "full_frame", "min", "sdr_white").map { o.getDouble(it) })
    }

    @Test
    fun ecran_hdr10() {
        assertTrue(HdrScreen.hdr10(intArrayOf(Display.HdrCapabilities.HDR_TYPE_HLG, Display.HdrCapabilities.HDR_TYPE_HDR10)))
        assertFalse(HdrScreen.hdr10(intArrayOf(Display.HdrCapabilities.HDR_TYPE_HLG)))
        assertFalse(HdrScreen.hdr10(intArrayOf()))
        assertFalse(HdrScreen.hdr10(null))
        // L'écran de Robolectric (comme l'émulateur) n'est pas HDR : aucune trace du HDR.
        assertEquals(HdrScreen.SDR, HdrScreen.of(context))
        assertEquals(HdrState.HIDDEN, Hdr.state(host(), null, shield, HdrScreen.SDR))
    }

    @Test
    fun format_et_decodeur_en_hdr() {
        // Auto et HEVC : HEVC Main 10 ; la Shield le lit en HDR10.
        assertEquals("hevc", Hdr.codec(host(), shield))
        assertTrue(Hdr.decodes(host(), shield))
        assertTrue(Hdr.decodes(host("hevc"), shield))
        // H.264 choisi : pas de HDR (« Le HDR demande le format HEVC ou AV1. »).
        assertNull(Hdr.codec(host("h264"), shield))
        assertFalse(Hdr.decodes(host("h264"), shield))
        // Un décodeur HEVC sans le profil HDR10.
        assertFalse(Hdr.decodes(host(), sdrOnly))
        assertFalse(Hdr.decodes(host("av1", av1 = true), sdrOnly))
        // Sans HEVC : l'AV1 10 bits, si l'ordinateur l'encode.
        assertEquals("av1", Hdr.codec(host(av1 = true), av1Only))
        assertTrue(Hdr.decodes(host(av1 = true), av1Only))
        assertTrue(Hdr.decodes(host("av1", av1 = true), av1Only))
        val caps = Hdr.capabilities(host(av1 = true), av1Only)
        assertEquals(listOf(false, false, true, true), listOf("hevc", "hevc444", "av1", "hdr").map { caps.getBoolean(it) })
    }

    @Test
    fun reglages_lus_par_le_coeur() {
        // La puce : le format que la session demandera, le profil sauf avec H.264 choisi à la main.
        val auto = Hdr.chipSettings(host(), shield)
        assertEquals("hevc", auto.getString("codec"))
        assertEquals("jeu", auto.getString("profile"))
        val h264 = Hdr.chipSettings(host("h264"), shield)
        assertEquals("h264", h264.getString("codec"))
        assertFalse(h264.has("profile"))
        // Le choix envoyé à la session : jamais avec H.264 choisi à la main.
        assertTrue(Hdr.wanted(host()))
        assertFalse(Hdr.wanted(host(hdr = false)))
        assertFalse(Hdr.wanted(host("h264")))
    }

    @Test
    fun choix_garde_pour_chaque_ordinateur_hors_du_profil() {
        // Allumé la première fois, et pour un fichier d'avant le HDR.
        assertTrue(HostSettings().hdr)
        assertTrue(HostSettings.parse(JSONObject().put("profile", "bureau")).hdr)
        val off = HostSettings(profile = Profile.DESKTOP, hdr = false)
        assertFalse(HostSettings.parse(off.json()).hdr)
        // Il ne fait pas passer en « Personnalisé », et un profil choisi le garde.
        assertFalse(off.custom)
        assertFalse(off.apply(Profile.MAX).hdr)
        // La dernière session dit si l'image a été en HDR.
        val last = LastSession(8.9, 14.2, 60, 0, 3840, 2160, 60, "hevc", 80, hdr = true)
        assertTrue(LastSession.parse(last.json()).hdr)
        assertFalse(LastSession.parse(JSONObject().put("median", 8.9)).hdr)
    }

    @Test
    fun puce_lue_du_coeur() {
        val on = HdrState.parse("""{"shown": true, "available": true, "reason": null, "kind": null}""")
        assertEquals(HdrState(true, true, null), on)
        assertTrue(on.on(true))
        assertFalse(on.on(false))
        val off = HdrState.parse("""{"shown": true, "available": false, "reason": "L'ordinateur ne propose pas le HDR.", "kind": "off"}""")
        assertEquals("L'ordinateur ne propose pas le HDR.", off.reason)
        assertFalse(off.on(true))
        assertFalse(HdrState.parse("""{"shown": false, "available": true}""").on(true))
        assertEquals(HdrState.HIDDEN, HdrState.parse(null))
        assertEquals(HdrState.HIDDEN, HdrState.parse("pas du JSON"))
    }

    @Test
    fun demande_d_emprunt_avec_le_hdr() {
        val codecs = JSONArray().put("hevc").put("h264")
        val sdr = Lend.request("123456", null, null, "Léa", "dev", 3840, 2160, 60, codecs).getJSONObject("screen")
        assertFalse(sdr.has("hdr"))
        assertFalse(sdr.has("luminance"))
        val hdr = Lend.request("123456", null, null, "Léa", "dev", 3840, 2160, 60, codecs, hdr = true, luminance = HdrLuminance(800.0, 400.0, 0.05).json())
            .getJSONObject("screen")
        assertTrue(hdr.getBoolean("hdr"))
        assertEquals(800.0, hdr.getJSONObject("luminance").getDouble("max"), 0.0)
        // HDR sans valeurs : la Station prend celles par défaut.
        assertFalse(Lend.request(null, "lea", "x", "Léa", "dev", 1920, 1080, 60, codecs, hdr = true).getJSONObject("screen").has("luminance"))
    }

    @Test
    fun session_mesures_et_avis() {
        val state = SessionState(context, Gamepads(context))
        // Les mesures de la seconde : l'image en HDR (dernière valeur ; absente d'un cœur plus ancien : non).
        val values = DoubleArray(18).also { it[17] = 1.0 }
        state.onSecond(values)
        assertTrue(state.second.value!!.hdr)
        state.onSecond(DoubleArray(17))
        assertFalse(state.second.value!!.hdr)
        state.onHdr(true)
        assertTrue(state.hdr.value)
        // « L'ordinateur n'a pas pu passer en HDR : image en SDR. », le texte du cœur.
        state.onNotice(SessionListener.NOTICE_HDR_REFUSED, 0, 0, false, 0, 0, false, "L'ordinateur n'a pas pu passer en HDR : image en SDR.")
        ShadowLooper.idleMainLooper()
        assertEquals(Capsule.HdrRefused("L'ordinateur n'a pas pu passer en HDR : image en SDR."), state.capsule.value)
        state.close()
    }
}
