package fr.leslignesclaires.kybernow

import android.app.Application
import android.os.Looper
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.HostInfo
import fr.leslignesclaires.kybernow.core.HostRelay
import fr.leslignesclaires.kybernow.core.JoinReply
import fr.leslignesclaires.kybernow.core.PendingJoin
import fr.leslignesclaires.kybernow.core.RelayFailure
import fr.leslignesclaires.kybernow.core.Relays
import fr.leslignesclaires.kybernow.core.Reply
import fr.leslignesclaires.kybernow.core.RouteInfo
import fr.leslignesclaires.kybernow.core.RouteTransport
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareLink
import fr.leslignesclaires.kybernow.core.ShareRelay
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.core.ShareTarget
import fr.leslignesclaires.kybernow.core.ShareVia
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.LastSession
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.SessionState
import fr.leslignesclaires.kybernow.ui.RelayRefusal
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
 * Le relais de KyberNow (docs/relais.md) : ce que l'app range des réponses du cœur (sans bibliothèque native) : les demandes
 * et la voie des invités de l'hôte, l'état du relais, les liens avec `relay` et `sid`, les réponses de « Rejoindre » et de
 * l'attente, le relais gardé avec l'ordinateur, le chemin d'une session.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class RelayTest {
    private val format = { code: String -> code.take(3) + " " + code.drop(3) }
    private val noLabel = { _: String -> null as String? }
    private val station = "abcdefghijklmnopqrstuvwxyz"

    // MARK: - Le partage de l'hôte

    @Test
    fun requestsAreNotGuests_andComeInWithTheirRoute() {
        val json = """
            {"open":true,"code":"428913","link":"kybernow://join?x&relay=r&sid=s","web":"https://share.kybernow.app/j/428913",
             "web_local":"https://192.168.1.20:24811/link/?join=428913","relay":{"state":"joined"},
             "guests":[{"id":"g1","name":"Léa","app":"ios","role":"spectator","since":10,"pending":true,"via":"relay"},
                       {"id":"g2","name":"Tom","app":"mac","role":"player","since":5,"pending":false,"via":"lan"},
                       {"id":"g3","name":"Eva","app":"android","role":"spectator","since":7,"pending":false,"via":"relay"}]}
        """
        val s = ShareState.parse(json, format, noLabel)!!
        // La demande ne compte pas dans les invités, ni dans les places.
        assertEquals(listOf("g2", "g3"), s.guests.map { it.id })
        assertEquals(listOf("g1"), s.requests.map { it.id })
        assertFalse(s.full)
        assertEquals(ShareRelay.JOINED, s.relay)
        assertTrue(s.requests.single().pending)
        assertEquals(ShareVia.RELAY, s.requests.single().via)
        assertTrue(s.requests.single().away)
        // Un invité du réseau n'est pas « hors de chez vous » ; un invité venu par le relais l'est.
        assertEquals(listOf(false, true), s.guests.map { it.away })
        // Le lien du relais est celui de Copier le lien (`web`).
        assertEquals("https://share.kybernow.app/j/428913", s.web)
        // Les trois demandes en attente ne remplissent pas le partage : les quatre places restent libres.
        val three = (1..3).joinToString(",") { """{"id":"p$it","name":"P$it","app":"ios","pending":true,"via":"relay"}""" }
        val four = (1..4).joinToString(",") { """{"id":"g$it","name":"I$it","app":"mac"}""" }
        assertFalse(ShareState.parse("""{"open":true,"code":"1","guests":[$three]}""", format, noLabel)!!.full)
        val full = ShareState.parse("""{"open":true,"code":"1","guests":[$four,$three]}""", format, noLabel)!!
        assertTrue(full.full)
        assertEquals(3, full.requests.size)
    }

    @Test
    fun aStationBeforeTheRelay_hasNoRequestsNorRoute() {
        val s = ShareState.parse("""{"open":true,"code":"428913","link":"l","guests":[{"id":"g1","name":"Léa","app":"mac","role":"spectator"}]}""", format, noLabel)!!
        assertNull(s.relay)
        assertTrue(s.requests.isEmpty())
        assertEquals(ShareVia.LAN, s.guests.single().via)
        assertFalse(s.guests.single().pending)
        assertFalse(s.guests.single().away)
        // Une voie inconnue est le réseau.
        assertEquals(ShareVia.LAN, ShareVia.parse("satellite"))
        assertEquals(ShareVia.LAN, ShareVia.parse(null))
    }

    @Test
    fun theRelayState_isJoinedOrNot_andAnUnknownStateIsNotJoined() {
        fun relay(state: String) = ShareState.parse("""{"open":true,"code":"1","relay":$state}""", format, noLabel)!!.relay
        assertEquals(ShareRelay.JOINED, relay("""{"state":"joined"}"""))
        assertEquals(ShareRelay.UNREACHABLE, relay("""{"state":"unreachable"}"""))
        assertEquals(ShareRelay.UNREACHABLE, relay("""{"state":"autre"}"""))
        // Aucun relais réglé : rien à dire.
        assertNull(relay("null"))
    }

    @Test
    fun aNewRequest_isAnnouncedOnce_inACapsule() {
        val context = ApplicationProvider.getApplicationContext<Application>()
        val state = SessionState(context, Gamepads(context))
        state.imageLabel = noLabel
        state.codeFormat = format
        fun beat(vararg names: String) = """{"open":true,"code":"428913","link":"l","guests":[${names.joinToString(",") { """{"id":"$it","name":"$it","app":"ios","pending":true,"via":"relay"}""" }}]}"""
        state.onGuests(beat("Léa"))
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(Capsule.ShareRequest("Léa"), state.capsule.value)
        assertEquals(1, state.share.value!!.requests.size)
        // Le même signe de vie ne la dit pas deux fois.
        state.capsule.value = null
        state.onGuests(beat("Léa"))
        shadowOf(Looper.getMainLooper()).idle()
        assertNull(state.capsule.value)
        // Une autre demande, oui.
        state.onGuests(beat("Léa", "Tom"))
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(Capsule.ShareRequest("Tom"), state.capsule.value)
        // Une demande acceptée, refusée ou partie n'en annonce rien.
        state.capsule.value = null
        state.onGuests("""{"open":true,"code":"428913","link":"l","guests":[{"id":"Léa","name":"Léa","app":"ios","role":"spectator","via":"relay"}]}""")
        shadowOf(Looper.getMainLooper()).idle()
        assertNull(state.capsule.value)
        assertEquals(1, state.share.value!!.guests.size)
        assertTrue(state.share.value!!.requests.isEmpty())
    }

    // MARK: - Rejoindre, par un lien

    @Test
    fun aShareLinkCarriesItsRelay_asTheCoreReadsIt() {
        val link = ShareLink(ShareTarget("192.168.1.20", 24811, "sha256:ab:cd"), "428913", relay = "share.kybernow.app", sid = station)
        val o = link.json()
        assertEquals("192.168.1.20", o.getString("host"))
        assertEquals(24811, o.getInt("port"))
        assertEquals("sha256:ab:cd", o.getString("fingerprint"))
        assertEquals("428913", o.getString("code"))
        assertEquals("share.kybernow.app", o.getString("relay"))
        assertEquals(station, o.getString("sid"))
        // Sans relais, le lien du réseau seulement : `relay` et `sid` nuls.
        val plain = ShareLink(ShareTarget("192.168.1.20", 24811, null), "428913").json()
        assertTrue(plain.isNull("relay"))
        assertTrue(plain.isNull("sid"))
        assertTrue(plain.isNull("fingerprint"))
        // Le code tapé remplace celui du lien : le même lien, un autre code.
        assertEquals("111222", link.copy(code = "111222").json().getString("code"))
    }

    // MARK: - Rejoindre, les réponses du cœur

    private val found = """{"target":{"host":"$station","port":0,"fingerprint":"sha256:ad"},"joined":{"guest":"g1","secret":"S","role":"spectator","host":"Léa","computer":"Atelier","display":{"id":1,"width":1920,"height":1080,"refresh":60},"route":{"server":"share.kybernow.app","station":"$station","admin_fp":"sha256:ad"}}}"""

    @Test
    fun joiningOnTheNetwork_enters() {
        val reply = Share.readJoin("""{"code":0,"value":{"joined":$found}}""")
        reply as JoinReply.Joined
        assertEquals("Atelier", reply.found.computer)
        // La route par le relais voyage avec `joined`, jusqu'à la session d'invité.
        assertEquals("share.kybernow.app", reply.found.joined.getJSONObject("route").getString("server"))
    }

    @Test
    fun joiningByTheRelay_waitsForTheHost_andNeverPrintsTheSecret() {
        val pending = """{"route":{"server":"share.kybernow.app","station":"$station","admin_fp":"sha256:ad"},"guest":"g1","secret":"SECRET-DE-LA-DEMANDE","expires":120}"""
        val reply = Share.readJoin("""{"code":0,"value":{"pending":$pending}}""")
        reply as JoinReply.Pending
        assertEquals(120, reply.pending.expires)
        // `shareWait` et `shareCancel` reçoivent le `pending` du cœur tel quel.
        assertEquals("SECRET-DE-LA-DEMANDE", JSONObject(reply.pending.json).getString("secret"))
        assertFalse(reply.pending.toString().contains("SECRET"))
        assertFalse(reply.toString().contains("SECRET"))
    }

    @Test
    fun theRefusals_keepTheirKind_andTheCoreText() {
        fun refused(code: Int, error: String, kind: String? = null): JoinReply.Refused {
            val o = JSONObject().put("code", code).put("error", error)
            if (kind != null) o.put("kind", kind)
            return Share.readJoin(o.toString()) as JoinReply.Refused
        }
        // « L'hôte a refusé. », « L'hôte n'a pas répondu. » (120 s), trop de demandes : les textes sont ceux du cœur.
        assertEquals(JoinReply.Refused("refused", "L'hôte a refusé."), refused(24, "L'hôte a refusé.", "refused"))
        assertEquals(JoinReply.Refused("expired", "L'hôte n'a pas répondu."), refused(25, "L'hôte n'a pas répondu.", "expired"))
        assertEquals(JoinReply.Refused("pending_full", "Trop de demandes en attente : réessayez dans un instant."), refused(26, "Trop de demandes en attente : réessayez dans un instant.", "pending_full"))
        assertEquals(JoinReply.Refused("no_share", "Aucun partage avec ce code sur le réseau."), refused(20, "Aucun partage avec ce code sur le réseau.", "no_share"))
        // Le relais ne répond pas (30) ou l'ordinateur n'est pas chez lui (31) : arrivent comme une erreur de console, sans `kind`.
        assertEquals(JoinReply.Refused("relay", "Le relais ne répond pas."), refused(30, "Le relais ne répond pas."))
        assertEquals("relay", refused(31, "L'ordinateur n'est pas joignable : il est éteint ou en veille.").kind)
        assertEquals("other", refused(1, "").kind)
        // Une réponse illisible ne fait pas tomber l'app.
        assertEquals(JoinReply.Refused("other", ""), Share.readJoin("pas du JSON"))
        assertEquals(JoinReply.Refused("other", ""), Share.readJoin(null))
        assertEquals(JoinReply.Refused("other", ""), Share.readJoin("""{"code":0,"value":{}}"""))
    }

    @Test
    fun cancelling_asksTheCoreOnce() {
        val pending = PendingJoin("""{"guest":"g1","secret":"S"}""", 120)
        assertFalse(pending.cancelled.get())
        // Le drapeau passe à vrai à la première demande (la seconde ne fait plus rien) ; l'appel du cœur part sur son fil.
        assertTrue(pending.cancelled.compareAndSet(false, true))
        assertFalse(pending.cancelled.compareAndSet(false, true))
    }

    // MARK: - L'ordinateur et son relais

    private val infoWithRelay = """{"hostname":"Atelier","os":"Windows 11 Pro","relay":{"server":"share.kybernow.app","id":"$station","remote_access":true}}"""

    @Test
    fun theInfoGivesTheRelayOfTheComputer() {
        val info = HostInfo.parse(JSONObject(infoWithRelay))
        assertEquals(HostRelay("share.kybernow.app", station, true), info.relay)
        // Une Station sans relais réglé, ou d'avant le relais : rien.
        assertNull(HostInfo.parse(JSONObject("""{"hostname":"Atelier"}""")).relay)
        assertNull(HostInfo.parse(JSONObject("""{"hostname":"Atelier","relay":null}""")).relay)
        assertNull(HostInfo.parse(JSONObject("""{"hostname":"Atelier","relay":{"server":"","id":""}}""")).relay)
        // L'accès coupé dans Admin n'efface pas le relais : l'ordinateur le garde, l'accès est refusé à la connexion.
        assertFalse(HostInfo.parse(JSONObject("""{"relay":{"server":"share.kybernow.app","id":"$station","remote_access":false}}""")).relay!!.remoteAccess)
    }

    @Test
    fun theRouteNeedsThePinnedFingerprint_neverAFirstContactByTheRelay() {
        val relay = HostRelay("share.kybernow.app", station, true)
        val route = JSONObject(relay.route("sha256:ad:be")!!)
        assertEquals("share.kybernow.app", route.getString("server"))
        assertEquals(station, route.getString("station"))
        assertEquals("sha256:ad:be", route.getString("admin_fp"))
        assertNull(relay.route(null))
        assertNull(relay.route("  "))
    }

    @Test
    fun aComputerKeepsItsRelay_withItsOtherData() {
        val host = Host(
            id = "a", name = "Atelier", address = "192.168.1.20", port = 24811, fingerprint = "sha256:ad", user = "iphone",
            relay = HostRelay("share.kybernow.app", station, false),
        )
        val back = Host.parse(JSONObject(host.json().toString()))
        assertEquals(host.relay, back.relay)
        assertEquals(JSONObject(host.relayRoute!!).getString("admin_fp"), "sha256:ad")
        // La route suit l'empreinte épinglée : une nouvelle empreinte, une nouvelle route.
        assertEquals("sha256:ff", JSONObject(back.copy(fingerprint = "sha256:ff").relayRoute!!).getString("admin_fp"))
        // Sans empreinte, pas de route ; sans relais non plus (un ordinateur d'avant le relais se relit comme avant).
        assertNull(back.copy(fingerprint = null).relayRoute)
        val before = Host.parse(JSONObject("""{"id":"b","name":"Salon","address":"192.168.1.30","port":24811,"fingerprint":"sha256:ad","user":"u"}"""))
        assertNull(before.relay)
        assertNull(before.relayRoute)
    }

    // MARK: - Le chemin d'une session

    @Test
    fun theRouteOfASession_isDirectOrByTheRelay() {
        val relay = RouteInfo.parse("""{"transport":"relay","relay":"share.kybernow.app","rtt_client":23.5,"rtt_station":4.0}""")!!
        assertEquals(RouteTransport.RELAY, relay.transport)
        assertTrue(relay.viaRelay)
        assertEquals(23.5, relay.rttClient!!, 0.0)
        assertEquals("share.kybernow.app", relay.relay)
        val tcp = RouteInfo.parse("""{"transport":"relay_tcp","relay":"r","rtt_client":null,"rtt_station":null}""")!!
        assertEquals(RouteTransport.RELAY_TCP, tcp.transport)
        assertTrue(tcp.viaRelay)
        assertNull(tcp.rttClient)
        val direct = RouteInfo.parse("""{"transport":"direct","relay":null,"rtt_client":null,"rtt_station":null}""")!!
        assertFalse(direct.viaRelay)
        // Un chemin inconnu est le réseau ; un JSON illisible n'annonce rien.
        assertEquals(RouteTransport.DIRECT, RouteInfo.parse("""{"transport":"pigeon"}""")!!.transport)
        assertNull(RouteInfo.parse("pas du JSON"))
        assertNull(RouteInfo.parse(null))
    }

    @Test
    fun theSessionKeepsTheRoute_forTheChipAndTheStatistics() {
        val context = ApplicationProvider.getApplicationContext<Application>()
        val state = SessionState(context, Gamepads(context))
        assertNull(state.route.value)
        state.onRoute("""{"transport":"direct","relay":null,"rtt_client":null,"rtt_station":null}""")
        assertFalse(state.route.value!!.viaRelay)
        state.onRoute("""{"transport":"relay","relay":"share.kybernow.app","rtt_client":12.0,"rtt_station":3.5}""")
        assertTrue(state.route.value!!.viaRelay)
        // Un avis illisible garde le dernier chemin.
        state.onRoute("???")
        assertEquals(RouteTransport.RELAY, state.route.value!!.transport)
    }

    @Test
    fun theLastSessionKeepsItsRoute() {
        val last = LastSession(8.9, 14.2, 2520, 1_761_000_000_000, 2560, 1440, 120, "hevc", 80, route = "relay")
        assertEquals("relay", LastSession.parse(JSONObject(last.json().toString())).route)
        // Une dernière session d'avant le relais se relit sans chemin : la ligne « Chemin » n'y est pas.
        val before = JSONObject(last.json().toString()).apply { remove("route") }
        assertNull(LastSession.parse(before).route)
        assertNull(LastSession.parse(JSONObject(last.copy(route = null).json().toString())).route)
    }

    // MARK: - Les refus d'Admin par le relais

    @Test
    fun theRelayRefusals_haveTheirOwnNames() {
        assertTrue(Reply.Failed(31, "").stationOffline)
        assertTrue(Reply.Failed(30, "Le relais ne répond pas.").relayUnreachable)
        // L'accès coupé dans Admin arrive avec son code (32, le cœur a lu le 403 `remote_access_off`) : jamais pris pour un autre refus.
        assertTrue(Reply.Failed(32, "L'accès hors de chez vous est coupé dans Admin.").remoteAccessOff)
        assertFalse(Reply.Failed(-401, "").remoteAccessOff)
        assertFalse(Reply.Failed(31, "").remoteAccessOff)
        assertFalse(Reply.Failed(32, "").stationOffline)
    }

    @Test
    fun theRelayRefusals_areChosenByTheirCode_neverByTheirText() {
        assertEquals(RelayFailure.UNREACHABLE, RelayFailure.of(30))
        assertEquals(RelayFailure.STATION_OFFLINE, RelayFailure.of(31))
        assertEquals(RelayFailure.ACCESS_OFF, RelayFailure.of(32))
        // Un autre code (compte refusé, certificat, partage…) n'est pas un refus du relais, quel que soit son texte.
        listOf(0, 1, 3, 20, 28, -401, -403, -404).forEach { assertNull(it.toString(), RelayFailure.of(it)) }
        assertNull(Reply.Failed(-403, "L'accès hors de chez vous est coupé dans Admin.").relayFailure)
        assertNull(Reply.Failed(1, "n'est pas joignable : l'ordinateur est éteint ou en veille.").relayFailure)
        // Le refus de l'accueil : par le code, le texte du cœur gardé seulement pour le relais muet.
        assertEquals(RelayRefusal.StationOffline, RelayRefusal.of(RelayFailure.STATION_OFFLINE, "x"))
        assertEquals(RelayRefusal.AccessOff, RelayRefusal.of(RelayFailure.ACCESS_OFF, "x"))
        assertEquals(RelayRefusal.Message("Le relais ne répond pas."), RelayRefusal.of(RelayFailure.UNREACHABLE, "Le relais ne répond pas."))
        assertNull(RelayRefusal.of(null, "x"))
    }

    @Test
    fun aShareStoppedWhileWaiting_andTheRelayCodes_keepTheirKind() {
        fun kind(code: Int, error: String = "", kind: String? = null): String {
            val o = JSONObject().put("code", code).put("error", error)
            if (kind != null) o.put("kind", kind)
            return (Share.readJoin(o.toString()) as JoinReply.Refused).kind
        }
        // « Ce partage est arrêté. » : code 28, `kind` « stopped » (jamais « Aucun partage avec ce code »).
        assertEquals("stopped", kind(28, "Ce partage est arrêté.", "stopped"))
        assertEquals("stopped", kind(28, "Ce partage est arrêté."))
        assertEquals("relay", kind(32, "L'accès hors de chez vous est coupé dans Admin."))
        assertEquals("relay", kind(31))
        assertEquals("relay", kind(30))
        assertEquals("other", kind(-403))
    }

    @Test
    fun theSessionEndedByTheRelay_saysItWithTheComputerName_chosenByTheCode() {
        val context = ApplicationProvider.getApplicationContext<Application>()
        fun ended(code: Int, coreText: String): String? {
            val state = SessionState(context, Gamepads(context))
            state.computerName = "Atelier"
            state.onRelayError(code, coreText)
            state.onStatus(6, 0, 0, coreText, false, false, false, null)
            return state.ended.value?.error
        }
        // Les textes de link-ui avec le nom (la langue de l'essai : celle du catalogue d'Android).
        assertEquals(context.getString(R.string.relay_access_off, "Atelier"), ended(32, "L'accès hors de chez vous est coupé dans Admin."))
        assertEquals(context.getString(R.string.relay_station_offline, "Atelier"), ended(31, "L'ordinateur n'est pas joignable."))
        assertTrue(ended(32, "x")!!.startsWith("Atelier "))
        // Relais muet : le texte du cœur, tel quel.
        assertEquals("Le relais ne répond pas.", ended(30, "Le relais ne répond pas."))
        // Une fin sans refus du relais garde le texte du cœur ; un code qui n'est pas au relais n'y change rien.
        val plain = SessionState(context, Gamepads(context))
        plain.onRelayError(28, "x")
        plain.onStatus(6, 0, 0, "La connexion a échoué.", false, false, false, null)
        assertEquals("La connexion a échoué.", plain.ended.value?.error)
    }

    @Test
    fun theRelayAddress_hasNoPortFor443_andKeepsAnotherOne() {
        // Sans bibliothèque native (essais), la forme normale est l'adresse sans espaces ni majuscules ; le cœur fait le reste.
        assertEquals("share.kybernow.app", Relays.normalize("  Share.Kybernow.app "))
        assertEquals("relais.exemple.org:9443", Relays.normalize("relais.exemple.org:9443"))
    }
}
