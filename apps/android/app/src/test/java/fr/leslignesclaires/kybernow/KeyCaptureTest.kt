package fr.leslignesclaires.kybernow

import android.app.Activity
import fr.leslignesclaires.kybernow.session.KeyCapture
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * La capture des touches du système (⊞, Alt+Tab) : sans l'API de Samsung ni la capture du clavier d'Android (ici,
 * Android 15 sans Samsung), rien ne casse et rien n'est pris ; les touches restent dans le menu « Touches ».
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = android.app.Application::class)
class KeyCaptureTest {
    @Test
    fun withoutAnyApi_nothingIsTaken_andNothingBreaks() {
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        assertEquals(KeyCapture.Result(samsung = false, platform = false), KeyCapture.set(activity, true))
        assertEquals(KeyCapture.Result(samsung = false, platform = false), KeyCapture.set(activity, false))
    }
}
