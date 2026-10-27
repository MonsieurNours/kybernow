package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.model.AutoQuality
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.DecoderLimits
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.ScreenMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * « Auto » de « Qualité de l'image » : ce que l'appareil dit savoir afficher (son écran) et décoder
 * (`VideoCapabilities.areSizeAndRateSupported`, ici une fonction donnée), et le JSON que le cœur lit. La
 * décision, elle, est celle de la Station (docs/link-ui.md, « Partage »).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class AutoQualityTest {
    private fun screen(w: Int, h: Int, hz: Int) = ScreenMode(1, w, h, hz)

    /** Un décodeur qui suit tout ce qui tient dans [pixelsPerSecond] (le débit de pixels, comme les niveaux H.264 et HEVC). */
    private fun follows(pixelsPerSecond: Long, maxWidth: Int = 8192) = { w: Int, h: Int, fps: Int -> w <= maxWidth && w.toLong() * h * fps <= pixelsPerSecond }

    @Test
    fun aDecoderThatFollowsTheScreen_limitsNothingBelowIt() {
        // 1080p à 120 Hz (la Shield) : tout est suivi.
        assertEquals(DecoderLimits(1920, 1080, 120), AutoQuality.limits(screen(1920, 1080, 120), true, follows(Long.MAX_VALUE)))
        // Un téléphone en 2400 × 1080 à 120 Hz, en portrait ou en paysage : côté long d'abord.
        assertEquals(DecoderLimits(2400, 1080, 120), AutoQuality.limits(screen(2400, 1080, 120), true, follows(Long.MAX_VALUE)))
        assertEquals(DecoderLimits(2400, 1080, 120), AutoQuality.limits(screen(1080, 2400, 120), true, follows(Long.MAX_VALUE)))
    }

    @Test
    fun aDecoderThatStopsAt4kAt30Hz_lowersTheRate_notTheDefinition() {
        // 4K à 30 Hz suivi, 4K à 60 Hz non : le 60 Hz tombe à 30 Hz, la définition reste.
        val limit = 3840L * 2160 * 30
        assertEquals(DecoderLimits(3840, 2160, 30), AutoQuality.limits(screen(3840, 2160, 60), true, follows(limit)))
    }

    @Test
    fun theRateIsJudgedAtTheChosenDefinition_not_atTheScreens() {
        // Un décodeur de classe 1080p (largeur 1920 au plus, 120 Hz) : sur un écran 4K à 60 Hz, 1080p et 60 Hz (jamais « 4K à 120 »).
        val limit = 1920L * 1080 * 120
        assertEquals(DecoderLimits(1920, 1080, 60), AutoQuality.limits(screen(3840, 2160, 60), true, follows(limit, maxWidth = 1920)))
        // Même décodeur, écran 1440p à 144 Hz : 1080p à 120 Hz.
        assertEquals(DecoderLimits(1920, 1080, 120), AutoQuality.limits(screen(2560, 1440, 144), true, follows(limit, maxWidth = 1920)))
    }

    @Test
    fun aSlowDecoder_fallsToTheSmallestStep() {
        // Rien de suivi, même pas 720p : 720p à 24 Hz au pire (H.264 compte comme toujours lu).
        assertEquals(DecoderLimits(1280, 720, 24), AutoQuality.limits(screen(1920, 1080, 60), true) { _, _, _ -> false })
        // Un petit écran sous 720p : sa propre définition.
        assertEquals(DecoderLimits(800, 480, 24), AutoQuality.limits(screen(480, 800, 60), true) { _, _, _ -> false })
    }

    @Test
    fun withoutAnyDecoder_nothingIsKnown() {
        assertEquals(DecoderLimits.NONE, AutoQuality.limits(screen(1920, 1080, 60), false) { _, _, _ -> true })
        assertEquals(DecoderLimits.NONE, AutoQuality.limits(screen(0, 0, 0), true) { _, _, _ -> true })
        assertEquals(DecoderLimits(0, 0, 0), DecoderLimits.NONE)
    }

    @Test
    fun theScreenAndTheDecoderAreWrittenAsTheCoreReadsThem() {
        val s = AutoQuality.screenJson(screen(2400, 1080, 120))
        // `DeviceScreen` du cœur : cadre, définition native, fréquence.
        assertEquals(2400, s.getInt("pixel_width"))
        assertEquals(1080, s.getInt("pixel_height"))
        assertEquals(120, s.getInt("refresh"))
        assertEquals(2400.0, s.getDouble("width"), 0.0)
        assertEquals(0.0, s.getDouble("x"), 0.0)
        assertTrue(s.getBoolean("primary"))
        val info = DecoderInfo("c2.test.decoder", 4096, 4096, true)
        val decoders = Decoders(h264 = info, hevc = info, av1 = null, hevc444 = false)
        val d = AutoQuality.decoderJson(decoders, DecoderLimits(1920, 1080, 60))
        // `DecoderCaps` du cœur : à plat, formats puis ce que le décodeur suit.
        assertTrue(d.getBoolean("hevc"))
        assertFalse(d.getBoolean("hevc444"))
        assertFalse(d.getBoolean("av1"))
        assertEquals(1920, d.getInt("max_width"))
        assertEquals(1080, d.getInt("max_height"))
        assertEquals(60, d.getInt("max_fps"))
        assertTrue(decoders.any)
        assertFalse(Decoders(null, null, null, false).any)
    }
}
