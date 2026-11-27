package fr.leslignesclaires.kybernow

import android.app.Application
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.model.Multicast
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Un seul verrou de multidiffusion pour l'app, rendu quand la dernière recherche s'arrête (AND-30). */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class MulticastTest {
    @Test
    fun theLockIsHeld_whileOneSearchNeedsIt_andReleasedOnce() {
        val context = ApplicationProvider.getApplicationContext<Application>()
        val a = Multicast.acquire(context)
        val b = Multicast.acquire(context)
        a.release()
        // Rendu deux fois : compté une seule.
        a.release()
        assertTrue(Multicast.held)
        b.release()
        assertFalse(Multicast.held)
    }
}
