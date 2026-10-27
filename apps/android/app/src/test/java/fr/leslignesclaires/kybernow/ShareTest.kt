package fr.leslignesclaires.kybernow

import android.app.Application
import android.os.Looper
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareFound
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.SessionState
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

/**
 * KyberNow Share : les réponses JSON du cœur rangées pour l'interface (sans bibliothèque native : le
 * code se met en forme avec une fonction donnée), les rôles et les avis de changement de droits.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class ShareTest {
    private val format = { code: String -> code.take(3) + " " + code.drop(3) }

    @Test
    fun roles_followTheCoreCodes() {
        assertEquals(ShareRole.SPECTATOR, ShareRole.of(0))
        assertEquals(ShareRole.PLAYER, ShareRole.of(1))
        assertEquals(ShareRole.PLAYER_KBM, ShareRole.of(2))
        // Un rôle inconnu n'ouvre rien : spectateur.
        assertEquals(ShareRole.SPECTATOR, ShareRole.of(9))
        assertEquals(ShareRole.SPECTATOR, ShareRole.parse("admin"))
        assertEquals(ShareRole.PLAYER_KBM, ShareRole.parse("player_kbm"))
        assertFalse(ShareRole.SPECTATOR.plays)
        assertTrue(ShareRole.PLAYER.plays)
        assertFalse(ShareRole.PLAYER.keyboardMouse)
        assertTrue(ShareRole.PLAYER_KBM.keyboardMouse)
    }

    @Test
    fun theCodeIsTypedWithDigitsOnly_sixAtMost() {
        assertEquals("123456", Share.digits("123 456"))
        assertEquals("123456", Share.digits("12-34a56789"))
        assertEquals("", Share.digits("abc"))
    }

    @Test
    fun openedShare_codeLinkAndGuests() {
        val json = """
            {"open":true,"code":"428913","link":"kybernow://join?host=192.168.1.20&port=41234&fp=sha256:ab&code=428913",
             "web":"https://192.168.1.20:41234/link/?join=428913","display":{"id":1,"label":"Écran 1"},
             "guests":[{"id":"g1","name":"Léa","app":"ios","role":"spectator","since":1761561600},
                       {"id":"g2","name":"Max","app":"web","role":"player_kbm","since":null},
                       {"id":"g3","name":"Zoé","app":"android","role":"admin"}]}
        """
        val s = ShareState.parse(json, format)!!
        assertEquals("428913", s.code)
        assertEquals("428 913", s.codeText)
        assertTrue(s.link.startsWith("kybernow://join?"))
        assertTrue(s.web.startsWith("https://"))
        assertEquals(listOf("Léa", "Max", "Zoé"), s.guests.map { it.name })
        assertEquals(listOf(ShareRole.SPECTATOR, ShareRole.PLAYER_KBM, ShareRole.SPECTATOR), s.guests.map { it.role })
        assertEquals(1761561600L, s.guests[0].since)
        assertNull(s.guests[1].since)
        assertNull(s.guests[2].since)
        assertFalse(s.full)
    }

    @Test
    fun fourGuestsFillTheShare() {
        val guests = (1..4).joinToString(",") { """{"id":"g$it","name":"I$it","app":"mac","role":"spectator"}""" }
        assertTrue(ShareState.parse("""{"open":true,"code":"111222","link":"l","guests":[$guests]}""", format)!!.full)
    }

    @Test
    fun noShare_isNull() {
        assertNull(ShareState.parse("""{"open":false,"code":"","guests":[]}""", format))
        assertNull(ShareState.parse(null as String?, format))
        assertNull(ShareState.parse("pas du json", format))
        // La réponse de « Partager » (ShareOpened) n'a pas de `open`.
        assertEquals("123456", ShareState.parse("""{"code":"123456","link":"l","web":"w"}""", format)!!.code)
    }

    @Test
    fun foundStation_keepsTheCoreAnswerAsIs_andNeverPrintsItsSecrets() {
        val value = JSONObject(
            """
            {"target":{"host":"192.168.1.20","port":41234,"fingerprint":"sha256:ab:cd"},
             "joined":{"guest":"g1","secret":"SECRET-DE-L-INVITE","role":"player","host":"Léa","computer":"Atelier",
               "kyber":{"port":12345,"fingerprint":"sha256:ef","token":"JETON-KYBER","expires":120},
               "display":{"id":2221634456,"width":1920,"height":1080,"refresh":60},"codec":"hevc","yuv444":false,"beat":5}}
            """,
        )
        val f = ShareFound.parse(value)
        assertEquals("Atelier", f.computer)
        assertEquals("Léa", f.host)
        assertEquals(ShareRole.PLAYER, f.role)
        assertEquals(Triple(1920, 1080, 60), Triple(f.width, f.height, f.refresh))
        assertEquals("192.168.1.20", f.target.host)
        assertEquals("sha256:ab:cd", f.target.fingerprint)
        // La session reçoit `joined` tel quel.
        assertEquals("SECRET-DE-L-INVITE", f.joined.getString("secret"))
        assertEquals("JETON-KYBER", f.joined.getJSONObject("kyber").getString("token"))
        // Aucun jeton dans ce qui pourrait finir dans un journal.
        assertFalse(f.toString().contains("SECRET"))
        assertFalse(f.toString().contains("JETON"))
    }

    // MARK: - Avis de changement de droits

    private fun notices(vararg steps: Pair<Int, Int>): List<Int?> {
        val state = SessionState(ApplicationProvider.getApplicationContext(), Gamepads(ApplicationProvider.getApplicationContext()))
        return steps.map { (role, previous) ->
            state.capsule.value = null
            state.onShareRole(role, previous)
            shadowOf(Looper.getMainLooper()).idle()
            (state.capsule.value as? Capsule.ShareRights)?.text
        }
    }

    @Test
    fun rightsNotices_followTheCommonTexts() {
        val n = notices(0 to -1, 1 to 0, 2 to 1, 1 to 2, 0 to 2, 0 to 1, 2 to 0)
        // Au départ : rien. Spectateur → joueur : « Vous pouvez jouer. ».
        assertNull(n[0])
        assertEquals(R.string.share_notice_play, n[1])
        // Joueur → clavier et souris : « Vous avez le clavier et la souris. ».
        assertEquals(R.string.share_notice_keyboard_mouse, n[2])
        // Clavier et souris retirés, le joueur reste joueur : « Le clavier et la souris sont repris. ».
        assertEquals(R.string.share_notice_keyboard_mouse_back, n[3])
        // Ramené spectateur (« Faire regarder »), avec ou sans le clavier et la souris : « Vous regardez de nouveau. ».
        assertEquals(R.string.share_notice_watch, n[4])
        assertEquals(R.string.share_notice_watch, n[5])
        // Spectateur → clavier et souris d'un coup : le plus précis.
        assertEquals(R.string.share_notice_keyboard_mouse, n[6])
    }

    @Test
    fun theGuestRoleIsKeptAndTheHostListFollowsTheHeartbeat() {
        val state = SessionState(ApplicationProvider.getApplicationContext(), Gamepads(ApplicationProvider.getApplicationContext()))
        state.onShareRole(2, -1)
        assertEquals(ShareRole.PLAYER_KBM, state.shareRole.value)
        state.onShareRole(0, 2)
        assertEquals(ShareRole.SPECTATOR, state.shareRole.value)
        // Sans partage (l'hôte n'en a pas, ou il est fermé) : pas de liste.
        state.onGuests(null)
        assertNull(state.share.value)
    }
}
