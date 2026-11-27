package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Mic
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Le micro (docs/micro.md) : l'état du cœur, le choix retenu par ordinateur, « Laisser les invités parler ». */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class MicTest {
    @Test
    fun parsesTheCoresMicUi() {
        // `MicUi::ui_json` du cœur, plus `shown` du pont : les textes viennent tels quels.
        val waiting = Mic.parse("""{"on":true,"state":"waiting","lit":true,"sent":false,"shown":true,"reason":null,"label":"Micro","tooltip":"Micro : envoyé à l'ordinateur (Ctrl + Alt + Maj + V)","tile":"Envoi…"}""")
        assertTrue(waiting.capturing && waiting.lit && !waiting.sent && !waiting.refused)
        assertEquals("Envoi…", waiting.tile)
        assertNull(waiting.reason)
        val on = Mic.parse("""{"on":true,"state":"on","lit":true,"sent":true,"reason":null,"label":"Micro","tooltip":"t","tile":"Envoyé"}""")
        assertTrue(on.capturing && on.sent && on.shown)
        val refused = Mic.parse("""{"on":true,"state":"refused","lit":false,"sent":false,"shown":false,"reason":"L'hôte ne laisse pas les invités parler.","label":"Micro coupé","tooltip":"L'hôte ne laisse pas les invités parler.","tile":"Coupé"}""")
        assertFalse(refused.capturing || refused.lit || refused.shown)
        assertTrue(refused.refused)
        assertEquals("L'hôte ne laisse pas les invités parler.", refused.reason)
        assertEquals(Mic(), Mic.parse("illisible"))
        assertEquals(Mic(), Mic.parse(null))
        assertFalse(Mic().capturing)
    }

    @Test
    fun choiceKeptPerComputerOffByDefault() {
        assertFalse(HostSettings.parse(JSONObject()).microphone)
        val on = HostSettings().copy(microphone = true)
        assertTrue(HostSettings.parse(on.json()).microphone)
        // Hors du profil : jamais « Personnalisé ».
        assertFalse(on.custom)
    }

    @Test
    fun guestsSpeakSwitch() {
        val base = JSONObject().put("open", true).put("code", "123456").put("link", "l")
        assertNull(ShareState.parse(base, { it }, { null })!!.guestsSpeak)
        assertEquals(false, ShareState.parse(JSONObject(base.toString()).put("guests_speak", false), { it }, { null })!!.guestsSpeak)
        assertEquals(true, ShareState.parse(JSONObject(base.toString()).put("guests_speak", true), { it }, { null })!!.guestsSpeak)
    }
}
