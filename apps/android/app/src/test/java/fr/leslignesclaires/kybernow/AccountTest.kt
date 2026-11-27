package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.core.AccountCode
import fr.leslignesclaires.kybernow.core.AccountComputer
import fr.leslignesclaires.kybernow.core.AccountMerge
import fr.leslignesclaires.kybernow.core.AccountReply
import fr.leslignesclaires.kybernow.core.Accounts
import fr.leslignesclaires.kybernow.core.HostRelay
import fr.leslignesclaires.kybernow.core.Reply
import fr.leslignesclaires.kybernow.core.StoredAccount
import fr.leslignesclaires.kybernow.core.TvPoll
import fr.leslignesclaires.kybernow.model.AccountHosts
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostAccount
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.HostStore
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.ui.account.PollNext
import fr.leslignesclaires.kybernow.ui.account.pollNext
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * Les comptes KyberNow (docs/comptes.md, lot 3) : ce que l'app range des réponses du cœur, sans bibliothèque native (le
 * compte gardé entier sans jamais montrer son jeton, les enveloppes et leurs refus 70 à 80, le code du téléviseur et la
 * suite d'une demande, la fusion « un ordinateur une fois »), les ordinateurs des comptes à l'accueil (ajoutés, suivis,
 * retirés avec le compte, l'appairé qui prend la pastille), et les paramètres de la session (le réseau s'il y est vu,
 * sinon le relais).
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class AccountTest {
    @org.junit.Before fun fakeCore() = FakeCore.install()

    @org.junit.After fun realCore() = FakeCore.reset()

    private val station = "abcdefghijklmnopqrstuvwxyz"
    private val station2 = "bcdefghijklmnopqrstuvwxyza"

    private fun accountJson(relay: String = "share.kybernow.app", id: String = "acc_lea", name: String = "Léa Martin", email: String = "lea@exemple.fr") =
        JSONObject().put("relay", relay).put("token", "SECRET-TOKEN-256")
            .put("account", JSONObject().put("id", id).put("name", name).put("email", email))
            .put("device", JSONObject().put("id", "dev_1").put("name", "Pixel de Léa"))

    private fun account(relay: String = "share.kybernow.app", id: String = "acc_lea", name: String = "Léa Martin") = StoredAccount.parse(accountJson(relay, id, name).toString())!!

    private fun computerJson(st: String = station, name: String = "PC du salon", fp: Any = "sha256:aa", relay: String = "share.kybernow.app", key: String = "share.kybernow.app|acc_lea") =
        JSONObject().put("account", key).put("relay", relay).put("station", st).put("name", name)
            .put("rights", JSONArray().put("session").put("share")).put("owner", JSONObject().put("kind", "person").put("name", "Léa Martin"))
            .put("mine", true).put("online", true).put("admin_fp", fp).put("attached_at", 1_795_000_000L)

    private fun computer(st: String = station, name: String = "PC du salon", fp: Any = "sha256:aa") = AccountComputer.parse(computerJson(st, name, fp))!!

    // MARK: - Le compte

    @Test
    fun theAccount_isKeptWhole_butNeverShowsItsToken() {
        val a = account()
        assertEquals("share.kybernow.app|acc_lea", a.key)
        assertEquals("Léa Martin", a.displayName)
        assertEquals("Pixel de Léa", a.deviceName)
        // Entier, jeton compris : il repart tel quel au cœur.
        assertEquals("SECRET-TOKEN-256", JSONObject(a.json).getString("token"))
        assertFalse(a.toString().contains("SECRET"))
        // Sans nom, l'e-mail (« Connecté à {compte}. »).
        assertEquals("lea@exemple.fr", StoredAccount.parse(accountJson(name = "").toString())!!.displayName)
    }

    @Test
    fun anAccount_withoutToken_orUnreadable_isNotKept() {
        assertNull(StoredAccount.parse(accountJson().apply { remove("token") }.toString()))
        assertNull(StoredAccount.parse(accountJson().apply { remove("account") }.toString()))
        assertNull(StoredAccount.parse("pas du JSON"))
        assertNull(StoredAccount.parse(null))
    }

    // MARK: - Enveloppes du cœur

    @Test
    fun envelopes_giveTheValue_orTheRefusalWithItsCode() {
        val ok = Accounts.readAccount(JSONObject().put("code", 0).put("value", accountJson()).toString())
        assertEquals("acc_lea", (ok as AccountReply.Ok).value.id)
        val signedOut = Accounts.readComputers(JSONObject().put("code", 70).put("error", "Ce compte a été déconnecté de cet appareil : reconnectez-vous.").put("kind", "signed_out").toString())
        signedOut as AccountReply.Refused
        assertTrue(signedOut.signedOut)
        assertEquals("signed_out", signedOut.kind)
        assertEquals("Ce compte a été déconnecté de cet appareil : reconnectez-vous.", signedOut.message)
        val cancelled = Accounts.readAccount(JSONObject().put("code", 75).put("error", "").put("kind", "cancelled").toString()) as AccountReply.Refused
        assertTrue(cancelled.cancelled)
        // Illisible : un refus sans texte, jamais une chute de l'app.
        assertEquals(AccountReply.Refused(1, "other", ""), Accounts.readAccount("{"))
        assertEquals(AccountReply.Refused(1, "other", ""), Accounts.readAccount(null))
        // Une valeur qui ne se lit pas non plus.
        assertEquals(AccountReply.Refused(1, "other", ""), Accounts.readAccount(JSONObject().put("code", 0).put("value", JSONObject()).toString()))
    }

    @Test
    fun theTvCode_andWhatFollowsAPoll() {
        val code = fr.leslignesclaires.kybernow.core.TvCode.parse(
            JSONObject().put("relay", "share.kybernow.app").put("code", "KQ7M-4XPA").put("poll", "p").put("expires", 600).put("interval", 5)
                .put("url", "https://share.kybernow.app/tv").put("url_complete", "https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA")
                .put("url_label", "share.kybernow.app/tv").put("issued_at", 1_795_000_000L),
        )!!
        assertEquals("KQ7M-4XPA", code.code)
        assertEquals("share.kybernow.app/tv", code.urlLabel)
        assertEquals("https://share.kybernow.app/compte/appareil?code=KQ7M-4XPA", code.urlComplete)
        assertEquals(5L, code.interval)
        // `poll` (le secret de la demande) repart au cœur dans le JSON.
        assertEquals("p", JSONObject(code.json).getString("poll"))

        assertEquals(TvPoll.Pending, (Accounts.readTvPoll("""{"code":0,"value":{"status":"pending"}}""") as AccountReply.Ok).value)
        val approved = Accounts.readTvPoll(JSONObject().put("code", 0).put("value", JSONObject().put("status", "approved").put("account", accountJson())).toString())
        assertEquals("acc_lea", ((approved as AccountReply.Ok).value as TvPoll.Approved).account.id)
        // 73 : un nouveau code de lui-même ; 72 : refusé (« Nouveau code ») ; trop d'essais : plus lentement ; relais muet : on redemande.
        assertEquals(PollNext.RENEW, pollNext(AccountCode.EXPIRED))
        assertEquals(PollNext.STOP, pollNext(AccountCode.DENIED))
        assertEquals(PollNext.SLOW, pollNext(AccountCode.TOO_MANY))
        assertEquals(PollNext.WAIT, pollNext(AccountCode.UNREACHABLE))
        assertEquals(PollNext.STOP, pollNext(AccountCode.OTHER))
    }

    @Test
    fun theAccountsComputers_andTheMerge() {
        val list = (Accounts.readComputers(JSONObject().put("code", 0).put("value", JSONArray().put(computerJson()).put(computerJson(station2, "Portable", JSONObject.NULL))).toString()) as AccountReply.Ok).value
        assertEquals(2, list.size)
        assertEquals("sha256:aa", list[0].adminFp)
        assertTrue(list[0].can("session"))
        assertFalse(list[0].can("admin"))
        // Sans empreinte (Station pas connectée au relais) : nulle, la précédente sera gardée.
        assertNull(list[1].adminFp)
        // Une ligne sans Station est ignorée.
        assertEquals(0, AccountComputer.parseList(JSONArray().put(JSONObject().put("name", "x"))).size)

        val merge = Accounts.readMerge(
            JSONObject().put("code", 0).put("value", JSONObject().put("linked", JSONArray().put(JSONArray().put("h1").put(computerJson()))).put("added", JSONArray().put(computerJson(station2, "Portable")))).toString(),
        )!!
        assertEquals(station, merge.linked["h1"]!!.station)
        assertEquals(listOf("Portable"), merge.added.map { it.name })
        assertNull(Accounts.readMerge("""{"code":1,"error":"x"}"""))
    }

    @Test
    fun thePairedComputers_forTheMerge_withTheirStation() {
        val paired = Host(id = "h1", name = "PC du salon", address = "192.168.1.20", port = 47990, fingerprint = "sha256:aa", user = "Pixel-ab12", paired = true, relay = HostRelay("share.kybernow.app", station))
        val plain = Host(id = "h2", name = "Atelier", address = "192.168.1.30", port = 47990, fingerprint = null, user = "lea")
        val fromAccount = Host(id = "compte-1", name = "Portable", address = "", port = 0, fingerprint = "sha256:bb", user = "", account = HostAccount("share.kybernow.app|acc_lea", station2))
        val a = JSONArray(Accounts.pairedJson(listOf(paired, plain, fromAccount)))
        assertEquals(2, a.length())
        assertEquals("h1", a.getJSONObject(0).getString("id"))
        assertEquals("sha256:aa", a.getJSONObject(0).getString("fingerprint"))
        assertEquals(station, a.getJSONObject(0).getString("station"))
        assertTrue(a.getJSONObject(1).isNull("fingerprint"))
        assertTrue(a.getJSONObject(1).isNull("station"))
    }

    @Test
    fun severalRelays_onlyWithAccountsOnTwoRelays() {
        assertFalse(Accounts.severalRelays(listOf(account(), account(id = "acc_hugo"))))
        assertTrue(Accounts.severalRelays(listOf(account(), account(relay = "relais.exemple.org:9443"))))
        assertEquals("relais.exemple.org:9443", Accounts.relayLabel("relais.exemple.org:9443#sha256:cc"))
    }

    // MARK: - L'accueil, un ordinateur une fois

    @Test
    fun accountComputers_arriveOnTheHomeScreen_andFollowTheAccount() {
        val paired = Host(id = "h1", name = "Atelier", address = "192.168.1.30", port = 47990, fingerprint = "sha256:cc", user = "lea")
        val first = AccountHosts.sync(listOf(paired), AccountMerge(added = listOf(computer())))
        assertEquals(2, first.size)
        val added = first[1]
        assertTrue(added.fromAccount)
        assertEquals("PC du salon", added.name)
        assertEquals("sha256:aa", added.fingerprint)
        assertEquals(HostRelay("share.kybernow.app", station, remoteAccess = true), added.relay)
        assertEquals(HostAccount("share.kybernow.app|acc_lea", station), added.account)
        // Sa route par le relais exige l'empreinte d'Admin, comme pour un ordinateur appairé.
        assertEquals(station, JSONObject(added.relayRoute!!).getString("station"))

        // Une nouvelle liste : la même carte (identifiant, réglages, adresse vue sur le réseau), le nouveau nom ; l'empreinte
        // reste si le relais ne la redonne pas.
        val tuned = first.map { if (it.id == added.id) it.copy(settings = HostSettings().apply(Profile.DESKTOP), address = "192.168.1.20", port = 47990) else it }
        val second = AccountHosts.sync(tuned, AccountMerge(added = listOf(computer(name = "PC du séjour", fp = JSONObject.NULL))))
        val kept = second.single { it.fromAccount }
        assertEquals(added.id, kept.id)
        assertEquals("PC du séjour", kept.name)
        assertEquals(Profile.DESKTOP, kept.settings.profile)
        assertEquals("192.168.1.20", kept.address)
        assertEquals("sha256:aa", kept.fingerprint)

        // Rien ne change : la même liste.
        assertSame(second, AccountHosts.sync(second, AccountMerge(added = listOf(computer(name = "PC du séjour", fp = JSONObject.NULL)))))

        // Le compte déconnecté (ou l'ordinateur retiré) : il part ; l'appairé reste.
        assertEquals(listOf("h1"), AccountHosts.sync(second, AccountMerge()).map { it.id })
    }

    @Test
    fun aPairedComputer_onTheAccount_showsOnce_withTheAccountsBadge() {
        val paired = Host(id = "h1", name = "PC du salon", address = "192.168.1.20", port = 47990, fingerprint = "sha256:aa", user = "Pixel-ab12", paired = true)
        val linked = AccountHosts.sync(listOf(paired), AccountMerge(linked = mapOf("h1" to computer())))
        assertEquals(1, linked.size)
        assertEquals(HostAccount("share.kybernow.app|acc_lea", station), linked[0].account)
        // Il reste appairé : son compte sur l'appareil, pas le jeton du relais.
        assertFalse(linked[0].fromAccount)
        assertEquals("Pixel-ab12", linked[0].user)
        // Le compte parti, la pastille aussi ; l'ordinateur reste.
        val alone = AccountHosts.sync(linked, AccountMerge())
        assertEquals(1, alone.size)
        assertNull(alone[0].account)
    }

    @Test
    fun theHostStore_keepsTwoAccountComputersWithoutAddress_apart() {
        val store = HostStore(RuntimeEnvironment.getApplication())
        store.syncAccounts(AccountMerge(added = listOf(computer(), computer(station2, "Portable", "sha256:bb"))))
        assertEquals(2, store.hosts.value.size)
        // Deux ordinateurs sans adresse connue ne sont pas « le même » (même adresse vide, même port 0).
        val extra = Host(name = "Autre", address = "", port = 0, fingerprint = null, user = "")
        store.put(extra)
        assertEquals(3, store.hosts.value.size)
        // Gardés avec les autres : relus tels quels.
        val again = HostStore(RuntimeEnvironment.getApplication())
        assertEquals(store.hosts.value.map { it.account }, again.hosts.value.map { it.account })
        // Le compte parti : ses ordinateurs aussi.
        store.syncAccounts(AccountMerge())
        assertEquals(listOf("Autre"), store.hosts.value.map { it.name })
    }

    @Test
    fun theHost_keepsItsAccount_inItsJson() {
        val h = Host(id = "compte-1", name = "PC du salon", address = "", port = 0, fingerprint = "sha256:aa", user = "", account = HostAccount("share.kybernow.app|acc_lea", station))
        val back = Host.parse(JSONObject(h.json().toString()))
        assertEquals(h, back)
        assertTrue(back.fromAccount)
        // Les fichiers d'avant les comptes : aucun compte.
        assertNull(Host.parse(JSONObject(h.json().toString()).apply { remove("account") }).account)
    }

    // MARK: - La session et la console

    @Test
    fun theSession_ofAnAccountComputer_onTheNetwork_orByTheRelay() {
        val a = account()
        val route = JSONObject().put("server", "share.kybernow.app").put("station", station).put("admin_fp", "sha256:aa").toString()

        // Vu sur le réseau : « Auto », le réseau d'abord, le relais sinon.
        val lan = JSONObject().put("user", "x").put("password", "y")
        assertTrue(Accounts.putSession(lan, a, station, "sha256:aa", "192.168.1.20" to 47990, route))
        assertEquals("192.168.1.20", lan.getString("console_host"))
        assertEquals(47990, lan.getInt("console_port"))
        assertEquals("", lan.getString("user"))
        assertEquals("", lan.getString("password"))
        assertEquals("sha256:aa", lan.getString("console_fingerprint"))
        assertEquals("auto", lan.getString("route"))
        assertEquals(station, lan.getJSONObject("relay").getString("station"))
        assertEquals(station, lan.getJSONObject("account").getString("station"))
        // Le compte entier, jeton compris : le cœur demande le jeton d'accès au relais.
        assertEquals("SECRET-TOKEN-256", lan.getJSONObject("account").getJSONObject("account").getString("token"))

        // Pas vu : par le relais seulement.
        val away = JSONObject()
        assertTrue(Accounts.putSession(away, a, station, "sha256:aa", null, route))
        assertEquals("", away.getString("console_host"))
        assertEquals("relay", away.getString("route"))
        assertEquals("sha256:aa", away.getJSONObject("relay").getString("admin_fp"))

        // Ni vu ni route (empreinte d'Admin inconnue) : pas de session.
        assertFalse(Accounts.putSession(JSONObject(), a, station, null, null, null))
        // `KYBERNOW_RELAY=off` : le réseau seulement.
        val off = JSONObject()
        assertTrue(Accounts.putSession(off, a, station, "sha256:aa", "192.168.1.20" to 47990, route, relayAllowed = false))
        assertEquals("direct", off.getString("route"))
        assertFalse(off.has("relay"))
        assertFalse(Accounts.putSession(JSONObject(), a, station, "sha256:aa", null, route, relayAllowed = false))

        val access = JSONObject(Accounts.access(a, station))
        assertEquals(station, access.getString("station"))
        assertEquals("acc_lea", access.getJSONObject("account").getJSONObject("account").getString("id"))
    }

    @Test
    fun adminRefusals_ofAnAccountComputer_byTheirCode() {
        assertTrue(Reply.Failed(70, "Ce compte a été déconnecté de cet appareil : reconnectez-vous.").accountSignedOut)
        assertTrue(Reply.Failed(71, "Cet ordinateur n'est plus sur votre compte.").accountComputerGone)
        assertFalse(Reply.Failed(31, "").accountComputerGone)
        assertNull(Reply.Failed(70, "").relayFailure)
    }

    @Test
    fun withoutTheNativeLibrary_nothingFalls() {
        // Les essais tournent sans libkybernow.so : les appels du cœur rendent une valeur sûre.
        assertFalse(Accounts.isCallback("kybernow://compte?code=x&etat=y"))
        assertEquals("", Accounts.origin(null, false))
        assertNull(Accounts.merge(emptyList(), listOf(computer())))
        val summary = Accounts.summary(account(relay = "relais.exemple.org:9443"))
        assertEquals("relais.exemple.org:9443", summary.relayLabel)
        assertEquals("https://relais.exemple.org:9443/compte", summary.console)
        assertNotNull(Accounts.rememberFingerprints(listOf(computer()), emptyList()).single().adminFp)
    }
}
