package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.core.HostRelay
import fr.leslignesclaires.kybernow.core.Invite
import fr.leslignesclaires.kybernow.core.InviteReply
import fr.leslignesclaires.kybernow.core.Invited
import fr.leslignesclaires.kybernow.core.JoinReply
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareTarget
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Les invitations (docs/invitations.md) : ce que l'app range des réponses du cœur (sans bibliothèque native) : le code
 * d'invitation de « Rejoindre », la réponse de « Ajouter l'ordinateur » et ses refus, l'ordinateur gardé comme un
 * ordinateur appairé (avec son relais), sans doublon pour la même empreinte d'Admin.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class InviteTest {
    private fun target() = JSONObject().put("host", "192.168.1.20").put("port", 47990).put("fingerprint", "sha256:aa")

    private fun route() = JSONObject().put("server", "share.kybernow.app").put("station", "abcdefghijklmnopqrstuvwxyz").put("admin_fp", "sha256:aa")

    private fun invited(fingerprint: Any = "sha256:aa", relay: Any = JSONObject().put("server", "share.kybernow.app").put("id", "abcdefghijklmnopqrstuvwxyz").put("remote_access", true)) =
        JSONObject().put("hostname", "PC du salon").put("address", "192.168.1.20").put("port", 47990).put("fingerprint", fingerprint)
            .put("username", "Pixel-ab12").put("password", "secret").put("relay", relay)
            .put("invite_name", "App Review").put("expires", 1_800_000_000L).put("via_relay", true)

    // MARK: - « Rejoindre avec un code »

    @Test
    fun anInviteCode_fromJoin_givesTheStationItsRouteAndTheComputer() {
        val value = JSONObject().put("code", "123456").put("target", target()).put("route", route()).put("computer", "PC du salon")
        val text = JSONObject().put("code", 33).put("error", "Ce code sert à ajouter l'ordinateur.").put("kind", "invite").put("value", value).toString()
        val code = (Share.readJoin(text) as JoinReply.Invite).code
        assertEquals("123456", code.code)
        assertEquals(ShareTarget("192.168.1.20", 47990, "sha256:aa"), code.target)
        assertEquals("PC du salon", code.computer)
        assertEquals("abcdefghijklmnopqrstuvwxyz", JSONObject(code.route!!).getString("station"))
        // La `value` repart telle quelle au cœur (`inviteRedeem`).
        assertEquals("123456", JSONObject(code.json).getString("code"))
        assertEquals("share.kybernow.app", JSONObject(code.json).getJSONObject("route").getString("server"))
    }

    @Test
    fun anInviteCode_onTheNetwork_hasNoRoute_andMayHaveNoName() {
        val value = JSONObject().put("code", "123456").put("target", target()).put("route", JSONObject.NULL)
        val code = (Share.readJoin(JSONObject().put("code", 33).put("value", value).toString()) as JoinReply.Invite).code
        assertNull(code.route)
        // Une Station qui ne dit pas son nom : « Ajouter l'ordinateur ».
        assertEquals("", code.computer)
        // Un code 33 sans Station lisible reste un refus, avec son texte.
        val refused = Share.readJoin("""{"code":33,"error":"Ce code sert à ajouter l'ordinateur.","kind":"invite"}""")
        assertEquals(JoinReply.Refused("invite", "Ce code sert à ajouter l'ordinateur."), refused)
        // Le code de prêt garde le sien.
        assertTrue(Share.readJoin(JSONObject().put("code", 29).put("value", JSONObject().put("code", "654321").put("target", target())).toString()) is JoinReply.Lend)
    }

    // MARK: - « Ajouter l'ordinateur »

    @Test
    fun theAddedComputer_isReadWithItsAccountAndRelay() {
        val reply = Invite.readRedeem(JSONObject().put("code", 0).put("value", invited()).toString())
        val i = (reply as InviteReply.Added).invited
        assertEquals("PC du salon", i.hostname)
        assertEquals("192.168.1.20", i.address)
        assertEquals(47990, i.port)
        assertEquals("sha256:aa", i.fingerprint)
        assertEquals("Pixel-ab12", i.username)
        assertEquals("secret", i.password)
        // `info.relay` du cœur : `{server, id, remote_access}`.
        assertEquals(HostRelay("share.kybernow.app", "abcdefghijklmnopqrstuvwxyz", true), i.relay)
        assertEquals("App Review", i.inviteName)
        assertEquals(1_800_000_000L, i.expires)
        assertTrue(i.viaRelay)
        // Le mot de passe du compte ne s'écrit jamais dans un journal.
        assertFalse(i.toString().contains("secret"))
    }

    @Test
    fun aRefusal_keepsItsCodeKindAndText() {
        fun refused(code: Int, kind: String, text: String) =
            Invite.readRedeem(JSONObject().put("code", code).put("error", text).put("kind", kind).toString()) as InviteReply.Refused
        assertEquals(InviteReply.Refused(Invite.BAD_PASSWORD, "bad_password", "Mot de passe refusé."), refused(60, "bad_password", "Mot de passe refusé."))
        assertEquals(Invite.NO_INVITE, refused(61, "no_invite", "Ce code ne sert plus.").code)
        assertEquals(Invite.FULL, refused(62, "full", "Cette invitation a déjà ajouté tous ses appareils.").code)
        assertEquals("Trop d'essais : réessayez dans 9 min.", refused(63, "too_many", "Trop d'essais : réessayez dans 9 min.").message)
        assertEquals("access_off", refused(64, "access_off", "PC du salon n'est pas joignable hors de chez vous (accès coupé dans Admin).").kind)
        assertEquals(Invite.OTHER, refused(65, "other", "Le relais ne répond pas.").code)
        // Illisible, ou une réponse sans compte : un refus sans texte (l'écran en met un), jamais une chute de l'app.
        assertEquals(InviteReply.Refused(1, "other", ""), Invite.readRedeem("pas du JSON"))
        assertEquals(InviteReply.Refused(1, "other", ""), Invite.readRedeem(null))
        assertEquals(InviteReply.Refused(1, "other", ""), Invite.readRedeem("""{"code":0,"value":{"hostname":"PC"}}"""))
        // Une demande illisible (code 1 du pont) : son texte, genre `other`.
        assertEquals(InviteReply.Refused(1, "other", "x"), Invite.readRedeem("""{"code":1,"error":"x"}"""))
    }

    @Test
    fun theButton_waitsForAPassword() {
        assertFalse(Invite.ready(""))
        assertFalse(Invite.ready(" - "))
        assertTrue(Invite.ready("k7mq-p2xa-9fz3"))
        assertTrue(Invite.ready("K7MQ P2XA 9FZ3"))
    }

    // MARK: - L'ordinateur gardé

    private fun added(json: JSONObject = invited()) = Invited.parse(json)!!

    @Test
    fun aNewComputer_isKeptAsAPairedComputerWithItsRelay() {
        val h = added().host(emptyList())
        assertEquals("PC du salon", h.name)
        assertEquals("192.168.1.20", h.address)
        assertEquals(47990, h.port)
        assertEquals("sha256:aa", h.fingerprint)
        assertEquals("Pixel-ab12", h.user)
        assertTrue(h.paired)
        // Le relais et l'empreinte épinglée : l'ordinateur s'ouvre hors de chez soi.
        assertEquals("abcdefghijklmnopqrstuvwxyz", JSONObject(h.relayRoute!!).getString("station"))
        assertEquals("sha256:aa", JSONObject(h.relayRoute!!).getString("admin_fp"))
        // Gardé et relu tel quel.
        val back = Host.parse(JSONObject(h.json().toString()))
        assertEquals(h.relay, back.relay)
        assertTrue(back.paired)
        assertEquals(h.relayRoute, back.relayRoute)
    }

    @Test
    fun theSameComputer_bySameAdminFingerprint_takesTheNewAccess() {
        val old = Host(
            id = "pc", name = "Salon", address = "10.0.0.5", port = 47990, fingerprint = "sha256:aa", user = "ancien",
            settings = HostSettings(profile = Profile.DESKTOP), relay = null,
        )
        val other = Host(id = "autre", name = "Bureau", address = "192.168.1.20", port = 47990, fingerprint = "sha256:bb", user = "x")
        val h = added().host(listOf(other, old))
        // Même empreinte d'Admin : la même carte (identifiant, réglages), le nouveau compte, la nouvelle adresse, le relais.
        assertEquals("pc", h.id)
        assertEquals(Profile.DESKTOP, h.settings.profile)
        assertEquals("Pixel-ab12", h.user)
        assertEquals("192.168.1.20", h.address)
        assertTrue(h.paired)
        assertEquals("abcdefghijklmnopqrstuvwxyz", h.relay!!.station)
    }

    @Test
    fun withoutFingerprint_theSameAddressIsTheSameComputer_andTheRelayIsKept() {
        val relay = HostRelay("share.kybernow.app", "abcdefghijklmnopqrstuvwxyz", true)
        val old = Host(id = "pc", name = "Salon", address = "192.168.1.20", port = 47990, fingerprint = "sha256:aa", user = "ancien", relay = relay)
        val h = added(invited(fingerprint = JSONObject.NULL, relay = JSONObject.NULL)).host(listOf(old))
        assertEquals("pc", h.id)
        // L'empreinte et le relais déjà connus restent.
        assertEquals("sha256:aa", h.fingerprint)
        assertEquals(relay, h.relay)
        // Une Station qui ne dit pas son nom : celui du code, sinon l'adresse.
        val unnamed = Invited.parse(invited().put("hostname", ""))!!
        assertEquals("PC", unnamed.host(emptyList(), "PC").name)
        assertEquals("192.168.1.20", unnamed.host(emptyList()).name)
    }
}
