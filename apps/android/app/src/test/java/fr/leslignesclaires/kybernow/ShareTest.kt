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
import fr.leslignesclaires.kybernow.session.shareRightsNotice
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
        assertEquals(ShareRole.KBM, ShareRole.of(3))
        // Un rôle inconnu n'ouvre rien : spectateur.
        assertEquals(ShareRole.SPECTATOR, ShareRole.of(9))
        assertEquals(ShareRole.SPECTATOR, ShareRole.parse("admin"))
        assertEquals(ShareRole.PLAYER_KBM, ShareRole.parse("player_kbm"))
        assertEquals(ShareRole.KBM, ShareRole.parse("kbm"))
        assertEquals(listOf(0, 1, 2, 3), ShareRole.entries.map { it.code })
    }

    @Test
    fun roles_haveTwoIndependentRights() {
        // Les deux droits de chaque rôle (`ShareRole::pads` et `kbm` du cœur).
        assertEquals(listOf(false, true, true, false), ShareRole.entries.map { it.pads })
        assertEquals(listOf(false, false, true, true), ShareRole.entries.map { it.keyboardMouse })
        // « Joueur » dans la barre : un droit au moins.
        assertEquals(listOf(false, true, true, true), ShareRole.entries.map { it.plays })
        assertEquals(ShareRole.SPECTATOR, ShareRole.of(false, false))
        assertEquals(ShareRole.PLAYER, ShareRole.of(true, false))
        assertEquals(ShareRole.KBM, ShareRole.of(false, true))
        assertEquals(ShareRole.PLAYER_KBM, ShareRole.of(true, true))
        for (r in ShareRole.entries) assertEquals(r, ShareRole.of(r.pads, r.keyboardMouse))
    }

    @Test
    fun givingOrTakingOneRight_neverTouchesTheOther() {
        for (r in ShareRole.entries) {
            for (on in listOf(true, false)) {
                assertEquals("manettes $on de $r", on, r.withPads(on).pads)
                assertEquals("clavier et souris inchangés par les manettes ($r)", r.keyboardMouse, r.withPads(on).keyboardMouse)
                assertEquals("clavier et souris $on de $r", on, r.withKeyboardMouse(on).keyboardMouse)
                assertEquals("manettes inchangées par le clavier et la souris ($r)", r.pads, r.withKeyboardMouse(on).pads)
            }
        }
        // Le dernier droit repris : spectateur.
        assertEquals(ShareRole.SPECTATOR, ShareRole.PLAYER.withPads(false))
        assertEquals(ShareRole.SPECTATOR, ShareRole.KBM.withKeyboardMouse(false))
        // Les parcours des deux boutons, dans les deux ordres.
        assertEquals(ShareRole.PLAYER_KBM, ShareRole.SPECTATOR.withPads(true).withKeyboardMouse(true))
        assertEquals(ShareRole.PLAYER_KBM, ShareRole.SPECTATOR.withKeyboardMouse(true).withPads(true))
        assertEquals(ShareRole.KBM, ShareRole.PLAYER_KBM.withPads(false))
        assertEquals(ShareRole.PLAYER, ShareRole.PLAYER_KBM.withKeyboardMouse(false))
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
        assertEquals("Max", s.guest("g2")?.name)
        assertNull(s.guest("g9"))
        assertEquals(1761561600L, s.guests[0].since)
        assertNull(s.guests[1].since)
        assertNull(s.guests[2].since)
        assertFalse(s.full)
    }

    @Test
    fun theKeyboardAndMouseOnlyRole_isReadFromTheGuestList() {
        val json = """{"open":true,"code":"428913","link":"l","guests":[{"id":"g1","name":"Léa","app":"mac","role":"kbm"}]}"""
        assertEquals(ShareRole.KBM, ShareState.parse(json, format)!!.guests.single().role)
        // Le rôle de « Rejoindre » aussi.
        val found = ShareFound.parse(JSONObject("""{"target":{"host":"h","port":1},"joined":{"role":"kbm","host":"Léa","computer":"Atelier"}}"""))
        assertEquals(ShareRole.KBM, found.role)
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
        // 0 spectateur, 1 manettes, 2 manettes avec clavier et souris, 3 clavier et souris.
        val n = notices(0 to -1, 1 to 0, 0 to 1, 3 to 0, 0 to 3, 2 to 1, 1 to 2, 2 to 3, 3 to 2)
        // Au départ : rien.
        assertNull(n[0])
        // Spectateur → manettes : « Vous pouvez jouer à la manette. » ; et plus aucun droit : « Vous regardez de nouveau. ».
        assertEquals(R.string.share_notice_play, n[1])
        assertEquals(R.string.share_notice_watch, n[2])
        // Spectateur → clavier et souris : « Vous avez le clavier et la souris. » ; et plus aucun droit : « Vous regardez de nouveau. ».
        assertEquals(R.string.share_notice_keyboard_mouse, n[3])
        assertEquals(R.string.share_notice_watch, n[4])
        // Un droit donné en plus de l'autre : celui-là seulement ; repris, l'autre reste.
        assertEquals(R.string.share_notice_keyboard_mouse, n[5])
        assertEquals(R.string.share_notice_keyboard_mouse_back, n[6])
        assertEquals(R.string.share_notice_play, n[7])
        assertEquals(R.string.share_notice_pads_back, n[8])
    }

    @Test
    fun rightsNotice_everyPassage() {
        val s = ShareRole.SPECTATOR
        val p = ShareRole.PLAYER
        val k = ShareRole.KBM
        val pk = ShareRole.PLAYER_KBM
        // Rien de changé : pas d'avis.
        for (r in ShareRole.entries) assertNull(shareRightsNotice(r, r))
        // Plus aucun droit : « Vous regardez de nouveau. » à la place de l'avis de reprise, d'où qu'on vienne.
        for (r in listOf(p, k, pk)) assertEquals(R.string.share_notice_watch, shareRightsNotice(r, s))
        // Un droit donné.
        assertEquals(R.string.share_notice_play, shareRightsNotice(s, p))
        assertEquals(R.string.share_notice_play, shareRightsNotice(k, pk))
        assertEquals(R.string.share_notice_keyboard_mouse, shareRightsNotice(s, k))
        assertEquals(R.string.share_notice_keyboard_mouse, shareRightsNotice(p, pk))
        // Un droit repris, l'autre gardé.
        assertEquals(R.string.share_notice_pads_back, shareRightsNotice(pk, k))
        assertEquals(R.string.share_notice_keyboard_mouse_back, shareRightsNotice(pk, p))
        // Les deux changent d'un coup : le droit donné passe avant le droit repris, les manettes avant le clavier et la souris.
        assertEquals(R.string.share_notice_keyboard_mouse, shareRightsNotice(p, k))
        assertEquals(R.string.share_notice_play, shareRightsNotice(k, p))
        assertEquals(R.string.share_notice_play, shareRightsNotice(s, pk))
    }

    @Test
    fun theGuestRoleIsKeptAndTheHostListFollowsTheHeartbeat() {
        val state = SessionState(ApplicationProvider.getApplicationContext(), Gamepads(ApplicationProvider.getApplicationContext()))
        state.onShareRole(2, -1)
        assertEquals(ShareRole.PLAYER_KBM, state.shareRole.value)
        state.onShareRole(0, 2)
        assertEquals(ShareRole.SPECTATOR, state.shareRole.value)
        state.onShareRole(3, 0)
        assertEquals(ShareRole.KBM, state.shareRole.value)
        // Sans partage (l'hôte n'en a pas, ou il est fermé) : pas de liste.
        state.onGuests(null)
        assertNull(state.share.value)
    }
}
