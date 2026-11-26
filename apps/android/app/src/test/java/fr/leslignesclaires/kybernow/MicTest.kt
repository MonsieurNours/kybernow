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
    fun parsesCoreStates() {
        assertEquals(Mic.Off, Mic.parse("""{"state":"off"}"""))
        assertEquals(Mic.Waiting, Mic.parse("""{"state":"waiting"}"""))
        assertEquals(Mic.On, Mic.parse("""{"state":"on"}"""))
        val r = Mic.parse("""{"state":"refused","reason":"no_device","text":"L'ordinateur n'a pas de micro virtuel : voyez Admin › Image et son."}""")
        assertEquals(Mic.Refused("no_device", "L'ordinateur n'a pas de micro virtuel : voyez Admin › Image et son."), r)
        assertFalse(r.capturing)
        assertTrue(Mic.Waiting.capturing && Mic.On.capturing)
        assertEquals(Mic.Off, Mic.parse("illisible"))
        assertEquals(Mic.Off, Mic.parse(null))
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
