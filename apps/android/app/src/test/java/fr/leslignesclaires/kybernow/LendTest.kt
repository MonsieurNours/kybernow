package fr.leslignesclaires.kybernow

import android.app.Application
import android.os.Looper
import androidx.test.core.app.ApplicationProvider
import fr.leslignesclaires.kybernow.core.BorrowReply
import fr.leslignesclaires.kybernow.core.JoinReply
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.core.LendAvailability
import fr.leslignesclaires.kybernow.core.LendAvailable
import fr.leslignesclaires.kybernow.core.LendDuration
import fr.leslignesclaires.kybernow.core.LendEnd
import fr.leslignesclaires.kybernow.core.LendHostState
import fr.leslignesclaires.kybernow.core.LendInvite
import fr.leslignesclaires.kybernow.core.LendKind
import fr.leslignesclaires.kybernow.core.LendNotice
import fr.leslignesclaires.kybernow.core.LendNoticeReason
import fr.leslignesclaires.kybernow.core.LendStatus
import fr.leslignesclaires.kybernow.core.LendSteam
import fr.leslignesclaires.kybernow.core.LendView
import fr.leslignesclaires.kybernow.core.RelayPhase
import fr.leslignesclaires.kybernow.core.RelayWait
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareRelay
import fr.leslignesclaires.kybernow.core.ShareTarget
import fr.leslignesclaires.kybernow.core.ShareVia
import fr.leslignesclaires.kybernow.model.BorrowedComputer
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.session.Capsule
import fr.leslignesclaires.kybernow.session.SessionState
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.time.ZoneId

/**
 * Le prêt de l'ordinateur (docs/pret.md) : ce que l'app range des réponses du cœur (sans bibliothèque native) : la carte
 * d'un ami, la vue de l'hôte, « Emprunter » et l'attente, le code de prêt de « Rejoindre », la demande envoyée, les
 * temps (restant, compte à rebours, heures, jours), les avis et la fin d'une session d'emprunt, l'ordinateur prêté gardé.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class LendTest {
    private val format = { code: String -> code.take(3) + " " + code.drop(3) }
    private val paris = ZoneId.of("Europe/Paris")
    private val nbsp = " "

    // MARK: - La carte de l'ami

    @Test
    fun theStatus_isReadWithItsSlot() {
        val r = Lend.readStatus("""{"code":0,"value":{"state":"busy","host":"Matthieu","computer":"PC-SALON","slot":{"now":false,"next":{"start":1000,"end":2000}}}}""")
        val s = (r as fr.leslignesclaires.kybernow.core.Reply.Ok).value
        assertEquals(LendAvailability.BUSY, s.state)
        assertEquals("Matthieu", s.host)
        assertEquals("PC-SALON", s.computer)
        assertFalse(s.slot!!.now)
        assertEquals(1000L, s.slot!!.next!!.start)
        // Occupé : on peut demander à l'hôte.
        assertTrue(s.borrowable)
    }

    @Test
    fun aRefusedAccount_keepsItsCodeAndText() {
        val r = Lend.readStatus("""{"code":40,"error":"Identifiant ou mot de passe refusé.","kind":"bad_credentials"}""")
        val f = r as fr.leslignesclaires.kybernow.core.Reply.Failed
        assertEquals(Lend.BAD_CREDENTIALS, f.code)
        assertEquals("Identifiant ou mot de passe refusé.", f.message)
        // Illisible : un refus sans texte, jamais une chute de l'app.
        assertTrue(Lend.readStatus("pas du json") is fr.leslignesclaires.kybernow.core.Reply.Failed)
    }

    @Test
    fun whatCanBeBorrowed_followsTheState() {
        fun st(state: String, mine: Boolean = false) = LendStatus.parse(JSONObject().put("state", state).put("mine", mine))
        assertTrue(st("free").borrowable)
        assertTrue(st("busy").borrowable)
        assertFalse(st("lent").borrowable)
        // Le prêt en cours est le sien : il revient.
        assertTrue(st("lent", mine = true).borrowable)
        assertFalse(st("locked").borrowable)
        assertFalse(st("off").borrowable)
        // Un état inconnu (Station plus récente) ne laisse rien partir.
        assertEquals(LendAvailability.OFF, st("ailleurs").state)
    }

    // MARK: - « Emprunter »

    private fun borrowed(until: Any? = 1_800_000_000L) = JSONObject()
        .put("lend", "l1").put("secret", "s3cr3t").put("host", "Matthieu").put("computer", "PC-SALON")
        .put("kyber", JSONObject().put("port", 47990).put("fingerprint", "sha256:kk").put("token", "JWT").put("expires", 120))
        .put("display", JSONObject().put("id", 3).put("width", 2400).put("height", 1080).put("refresh", 120))
        .put("until", until ?: JSONObject.NULL).put("steam", "friend").put("beat", 5)

    private fun target() = JSONObject().put("host", "192.168.1.20").put("port", 41234).put("fingerprint", "sha256:ab")

    @Test
    fun aReadyLoan_isReadForTheSession() {
        val text = JSONObject().put("code", 0).put("value", JSONObject().put("ready", JSONObject().put("target", target()).put("borrowed", borrowed()))).toString()
        val loan = (Lend.readBorrow(text) as BorrowReply.Ready).loan
        assertEquals("Matthieu", loan.host)
        assertEquals("PC-SALON", loan.computer)
        assertEquals(1_800_000_000L, loan.until)
        assertEquals(Triple(2400, 1080, 120), Triple(loan.width, loan.height, loan.refresh))
        assertEquals(ShareTarget("192.168.1.20", 41234, "sha256:ab"), loan.target)
        // La session reçoit `borrowed` tel quel.
        assertEquals("l1", loan.borrowed.getString("lend"))
        // Le secret et le jeton ne s'écrivent jamais dans un journal.
        assertFalse(loan.toString().contains("s3cr3t"))
        assertFalse(loan.toString().contains("JWT"))
    }

    @Test
    fun aLoanWithoutEnd_isUntilTheHostTakesItBack() {
        val text = JSONObject().put("code", 0).put("value", JSONObject().put("ready", JSONObject().put("target", target()).put("borrowed", borrowed(until = null)))).toString()
        assertNull((Lend.readBorrow(text) as BorrowReply.Ready).loan.until)
    }

    @Test
    fun aPendingBorrow_waitsForTheHost_withItsName() {
        val pending = JSONObject().put("target", target()).put("route", JSONObject.NULL).put("request", "r1").put("secret", "p4ss").put("host", "Matthieu").put("computer", "PC-SALON")
        val reply = Lend.readBorrow(JSONObject().put("code", 0).put("value", JSONObject().put("pending", pending)).toString())
        val p = (reply as BorrowReply.Pending).pending
        assertEquals("Matthieu", p.host)
        assertFalse(p.toString().contains("p4ss"))
        // Le JSON va tel quel à `lendWait` et `lendCancel`.
        assertEquals("r1", JSONObject(p.json).getString("request"))
    }

    @Test
    fun theRefusals_keepTheCoreTexts() {
        val cases = mapOf(
            40 to "Identifiant ou mot de passe refusé.", 41 to "Ce code ne sert plus.", 42 to "L'ordinateur est déjà prêté.",
            43 to "L'ordinateur est verrouillé.", 44 to "Le prêt est coupé sur cet ordinateur.", 46 to "Matthieu a refusé.",
            47 to "Matthieu n'a pas répondu.", 48 to "Le prêt demande KyberNow Station 0.9.0 ou plus récente.",
        )
        for ((code, text) in cases) {
            val r = Lend.readBorrow(JSONObject().put("code", code).put("error", text).put("kind", "x").toString()) as BorrowReply.Refused
            assertEquals(code, r.code)
            assertEquals(text, r.message)
        }
        assertTrue((Lend.readBorrow("""{"code":40,"error":"…"}""") as BorrowReply.Refused).badCredentials)
        assertTrue(Lend.readBorrow(null) is BorrowReply.Refused)
    }

    @Test
    fun theRequest_carriesTheCodeOrTheAccount_theScreenAndTheCodecs() {
        val byCode = Lend.request("123456", null, null, "Léa", "dev1", 2400, 1080, 120, Lend.codecs(hevc = true, av1 = false))
        assertEquals("123456", byCode.getString("code"))
        assertFalse(byCode.has("login"))
        assertEquals("android", byCode.getString("app"))
        assertEquals("Léa", byCode.getString("name"))
        assertEquals("dev1", byCode.getString("device"))
        assertEquals(2400, byCode.getJSONObject("screen").getInt("width"))
        assertEquals(120, byCode.getJSONObject("screen").getInt("refresh"))
        assertEquals(listOf("hevc", "h264"), byCode.getJSONArray("codecs").let { a -> (0 until a.length()).map { a.getString(it) } })
        val byAccount = Lend.request(null, "lea", "pw", "Léa", "dev1", 1920, 1080, 60, JSONArray().put("h264"))
        assertEquals("lea", byAccount.getString("login"))
        assertEquals("pw", byAccount.getString("password"))
        assertFalse(byAccount.has("code"))
    }

    // MARK: - « Rejoindre avec un code »

    @Test
    fun aLendCode_fromJoin_givesTheStationAndItsRoute() {
        val route = JSONObject().put("server", "share.kybernow.app").put("station", "abcdefghijklmnopqrstuvwxyz").put("admin_fp", "sha256:ab")
        val text = JSONObject().put("code", 29).put("error", "Ce code sert à emprunter l'ordinateur.").put("kind", "lend")
            .put("value", JSONObject().put("code", "654321").put("target", target()).put("route", route)).toString()
        val reply = Share.readJoin(text)
        val lend = (reply as JoinReply.Lend).code
        assertEquals("654321", lend.code)
        assertEquals(ShareTarget("192.168.1.20", 41234, "sha256:ab"), lend.target)
        assertEquals("share.kybernow.app", JSONObject(lend.route!!).getString("server"))
        // Sans route : sur le réseau.
        val local = Share.readJoin(JSONObject().put("code", 29).put("value", JSONObject().put("code", "654321").put("target", target()).put("route", JSONObject.NULL)).toString())
        assertNull((local as JoinReply.Lend).code.route)
        // Un code 29 sans Station lisible reste un refus.
        assertTrue(Share.readJoin("""{"code":29,"error":"x"}""") is JoinReply.Refused)
    }

    // MARK: - L'hôte

    @Test
    fun theHostView_isReadWithBorrowerNoticeRequestsAndCode() {
        val json = """
            {"enabled":true,"state":"lent","available":true,"computer":"PC-SALON","host":"Matthieu",
             "borrower":{"name":"Léa","kind":"friend","app":"ios","via":"relay","since":10,"until":20,"steam":"host","connected":true},
             "notice":{"reason":"taken_back","at":1234},
             "requests":[{"id":"r1","name":"Tom","kind":"code","app":"android","via":"lan","since":5,"steam":"host","slot":false},
                         {"id":"r2","name":"Eva","kind":"friend","app":"web","via":"relay","since":6,"slot":true},{"oups":1}],
             "code":{"code":"123456","link":"kybernow://join?code=123456","web":"https://x/link/?join=123456","duration":7200,"steam":"friend","expires":99}}
        """
        val v = LendView.parse(json, format)!!
        assertTrue(v.enabled)
        assertEquals(LendHostState.LENT, v.state)
        assertTrue(v.lent)
        assertEquals("Léa", v.borrower!!.name)
        assertEquals(LendKind.FRIEND, v.borrower!!.kind)
        assertEquals(ShareVia.RELAY, v.borrower!!.via)
        assertEquals(20L, v.borrower!!.until)
        assertEquals(LendSteam.HOST, v.borrower!!.steam)
        assertTrue(v.takingBack)
        assertEquals(LendNotice(LendNoticeReason.TAKEN_BACK, 1234), v.notice)
        // Une demande illisible est ignorée.
        assertEquals(listOf("r1", "r2"), v.requests.map { it.id })
        assertEquals(LendSteam.HOST, v.requests[0].steam)
        assertFalse(v.requests[0].away)
        assertTrue(v.requests[1].slot)
        assertTrue(v.requests[1].away)
        assertEquals("123 456", v.code!!.codeText)
        assertEquals(LendDuration.TWO_HOURS, v.code!!.duration)
        assertEquals("https://x/link/?join=123456", v.code!!.web)
    }

    @Test
    fun aFreeComputer_isNotLent_andACodeWithoutDuration_isOpen() {
        val v = LendView.parse("""{"enabled":true,"state":"free","requests":[],"code":{"code":"111222","link":"l","duration":null,"steam":"host"}}""", format)!!
        assertFalse(v.lent)
        assertFalse(v.takingBack)
        assertEquals(LendDuration.OPEN, v.code!!.duration)
        assertEquals(LendSteam.HOST, v.code!!.steam)
        assertNull(LendView.parse("pas du json", format))
    }

    @Test
    fun theDurations_areThoseOfLinkUi() {
        assertEquals(listOf(3600L, 7200L, 14400L, 0L), LendDuration.entries.map { it.seconds })
        assertEquals(LendDuration.TWO_HOURS, LendDuration.DEFAULT)
        assertEquals(listOf(0, 1), LendSteam.entries.map { it.code })
    }

    @Test
    fun lending_dependsOnTheStation() {
        // Station antérieure à la 0.9.0 : pas de `lend` dans `info`.
        assertEquals(LendAvailable.TOO_OLD, LendAvailable.of("""{"hostname":"PC"}"""))
        assertEquals(LendAvailable.OFF, LendAvailable.of("""{"lend":{"available":true,"enabled":false}}"""))
        assertEquals(LendAvailable.ON, LendAvailable.of("""{"lend":{"available":true,"enabled":true}}"""))
        assertEquals(LendAvailable.TOO_OLD, LendAvailable.of(null))
        // La Station ne peut pas prêter sur cet ordinateur : « Le prêt n'est pas possible sur cet ordinateur. », même activé.
        assertEquals(LendAvailable.UNAVAILABLE, LendAvailable.of("""{"lend":{"available":false,"enabled":true}}"""))
        assertEquals(LendAvailable.UNAVAILABLE, LendAvailable.of("""{"lend":{"available":false,"enabled":false}}"""))
        // `available` absent (Station qui ne le dit pas) : comme vrai.
        assertEquals(LendAvailable.ON, LendAvailable.of("""{"lend":{"enabled":true}}"""))
    }

    // MARK: - Code de prêt et relais

    @Test
    fun theLendCode_carriesItsRelay_likeTheShare() {
        fun code(relay: String) = LendView.parse("""{"enabled":true,"state":"free","code":{"code":"123456","link":"l","web":"w","relay":$relay}}""", format)!!.code!!.relay
        assertEquals(ShareRelay.JOINED, code("""{"state":"joined"}"""))
        assertEquals(ShareRelay.CONNECTING, code("""{"state":"connecting"}"""))
        assertEquals(ShareRelay.UNREACHABLE, code("""{"state":"unreachable"}"""))
        // Un état inconnu n'est pas « joint ».
        assertEquals(ShareRelay.UNREACHABLE, code("""{"state":"plus-tard"}"""))
        // Sans relais réglé (ou Station d'avant) : rien à dire.
        assertNull(code("null"))
        assertNull(LendView.parse("""{"enabled":true,"state":"free","code":{"code":"123456","link":"l"}}""", format)!!.code!!.relay)
    }

    @Test
    fun theLendCodeRelay_waitsTenSecondsAtMost_apartFromTheShare() {
        val share = RelayWait()
        val lend = RelayWait()
        // En préparation : « Préparation du lien hors de chez vous… », le QR code et « Copier le lien » retenus.
        assertEquals(0, lend.waited("111222", ShareRelay.CONNECTING, 1_000))
        assertEquals(RelayPhase.WAITING, RelayPhase.of(ShareRelay.CONNECTING, lend.waited("111222", ShareRelay.CONNECTING, 10_999)))
        // Le partage compte de son côté : il ne relance pas l'attente du code de prêt.
        assertEquals(0, share.waited("999888", ShareRelay.CONNECTING, 5_000))
        // Au-delà de 10 s : le lien du réseau, « Pas encore joignable hors de chez vous. ».
        assertEquals(RelayPhase.UNREACHABLE, RelayPhase.of(ShareRelay.CONNECTING, lend.waited("111222", ShareRelay.CONNECTING, 11_000)))
        // Un nouveau code de prêt repart de zéro ; joint, l'attente est oubliée.
        assertEquals(0, lend.waited("333444", ShareRelay.CONNECTING, 12_000))
        assertEquals(0, lend.waited("333444", ShareRelay.JOINED, 13_000))
        assertEquals(RelayPhase.JOINED, RelayPhase.of(ShareRelay.JOINED, 0))
        // Les deux attentes de l'app sont distinctes.
        assertTrue(RelayWait.shared !== RelayWait.lend)
    }

    // MARK: - Invitation et ordinateur prêté gardé

    @Test
    fun theInvite_isReadFromTheCoreJson() {
        val route = JSONObject().put("server", "share.kybernow.app").put("station", "abcdefghijklmnopqrstuvwxyz").put("admin_fp", "sha256:ab")
        val i = LendInvite.parse(JSONObject().put("host", "192.168.1.20").put("port", 41234).put("fingerprint", "sha256:ab").put("login", "lea")
            .put("relay", "share.kybernow.app").put("sid", "abcdefghijklmnopqrstuvwxyz").put("route", route))!!
        assertEquals("lea", i.login)
        assertEquals(ShareTarget("192.168.1.20", 41234, "sha256:ab"), i.target)
        assertNotNull(i.route)
        // Un lien d'avant : sans le prénom de l'hôte.
        assertNull(i.name)
        assertNull(LendInvite.parse(JSONObject().put("host", "x").put("port", 1)))
    }

    @Test
    fun theInvite_givesTheHostsFirstName_whenTheLinkHasIt() {
        fun invite(name: Any?) = LendInvite.parse(JSONObject().put("host", "192.168.1.20").put("port", 41234).put("login", "lea").put("name", name ?: JSONObject.NULL))!!
        assertEquals("Matthieu", invite("Matthieu").name)
        assertEquals("Matthieu", invite("  Matthieu ").name)
        assertNull(invite("").name)
        assertNull(invite("   ").name)
        assertNull(invite(null).name)
    }

    @Test
    fun anInvitationLink_isRecognisedEvenWhenUnreadable() {
        assertTrue(Lend.isInviteLink("kybernow://borrow?host=192.168.1.20&port=41234&login=lea"))
        assertTrue(Lend.isInviteLink("  KyberNow://Borrow?host="))
        // Coupé dans un message : toujours un lien d'invitation (« Ce lien d'invitation est illisible. »).
        assertTrue(Lend.isInviteLink("kybernow://borrow"))
        assertFalse(Lend.isInviteLink("kybernow://join?host=x&port=1&code=123456"))
        assertFalse(Lend.isInviteLink("kybernow://192.168.1.20:41234?name=PC"))
        assertFalse(Lend.isInviteLink("https://192.168.1.20:41234/link/?borrow=lea"))
    }

    @Test
    fun aBorrowedComputer_isKeptAndItsRouteNeedsTheFingerprint() {
        val c = BorrowedComputer(host = "Matthieu", computer = "PC-SALON", address = "192.168.1.20", port = 41234, fingerprint = "sha256:ab", login = "lea", relay = "share.kybernow.app", sid = "abcdefghijklmnopqrstuvwxyz")
        assertEquals(c, BorrowedComputer.parse(c.json()))
        assertTrue(c.id.startsWith("pret-"))
        assertEquals("sha256:ab", JSONObject(c.route!!).getString("admin_fp"))
        // Par le relais, jamais de premier contact : sans empreinte, pas de route.
        assertNull(c.copy(fingerprint = null).route)
        assertNull(c.copy(sid = null).route)
        // Le mot de passe n'est jamais dans ce qui se garde.
        assertFalse(c.json().has("password"))
    }

    @Test
    fun theStore_keepsBorrowedComputersApart_andForgetsAPasswordNotRemembered() {
        val store = fr.leslignesclaires.kybernow.model.HostStore(ApplicationProvider.getApplicationContext())
        val c = store.putBorrowed(BorrowedComputer(host = "Matthieu", computer = "PC", address = "10.0.0.2", port = 1, fingerprint = null, login = "lea"), null)
        assertEquals(listOf(c.id), store.borrowed.value.map { it.id })
        assertTrue(store.hosts.value.isEmpty())
        assertEquals(c.id, store.selectedId)
        // La même Station et le même identifiant : la même carte.
        val again = store.putBorrowed(c.copy(id = "autre", host = "Matt"), null)
        assertEquals(c.id, again.id)
        assertEquals("Matt", store.borrowed.value.single().host)
        store.removeBorrowed(c.id)
        assertTrue(store.borrowed.value.isEmpty())
    }

    // MARK: - Temps

    @Test
    fun theTimes_areWrittenAsInLinkUi() {
        assertEquals("1:42", Lend.countdown(102))
        assertEquals("0:05", Lend.countdown(5))
        assertEquals("0:00", Lend.countdown(-3))
        assertEquals(0L, Lend.left(10, 20))
        assertEquals(4800L, Lend.roundUpToMinute(4799))
        assertEquals(4800L, Lend.roundUpToMinute(4800))
        // Le préavis passe avant la fin prévue ; sans l'un ni l'autre : jusqu'à ce que l'hôte reprenne.
        assertEquals(500L, Lend.endsAt(9000, LendNotice(LendNoticeReason.TIME_UP, 500)))
        assertEquals(9000L, Lend.endsAt(9000, null))
        assertNull(Lend.endsAt(null, null))
    }

    @Test
    fun theHours_areFrenchOrEnglish() {
        // 2026-10-28 17:00 UTC = 18 h à Paris ; 14 h 30 = 13:30 UTC.
        val six = 1_793_206_800L
        assertEquals("18${nbsp}h", Lend.hour(six, true, paris))
        assertEquals("6${nbsp}PM", Lend.hour(six, false, paris))
        val half = six - 3 * 3600 - 1800
        assertEquals("14${nbsp}h${nbsp}30", Lend.hour(half, true, paris))
        assertEquals("2:30${nbsp}PM", Lend.hour(half, false, paris))
        assertEquals("12${nbsp}AM", Lend.hour(six + 6 * 3600, false, paris))
        assertEquals("mercredi", Lend.day(six, true, paris))
        assertEquals("Wednesday", Lend.day(six, false, paris))
    }

    // MARK: - La session

    private fun state() = SessionState(ApplicationProvider.getApplicationContext(), Gamepads(ApplicationProvider.getApplicationContext())).apply {
        codeFormat = format
        lendNoticeText = { n, name -> "Créneau de $name : ${n.reason.id}" }
    }

    @Test
    fun aNotice_isShownOnce_andKeptForTheBar() {
        val s = state()
        s.onLendNotice("""{"reason":"time_up","at":1234}""", "Fin du prêt dans 2 min : sauvegardez votre partie.")
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(LendNotice(LendNoticeReason.TIME_UP, 1234), s.lendNotice.value)
        assertEquals(Capsule.LendNotice("Fin du prêt dans 2 min : sauvegardez votre partie."), s.capsule.value)
    }

    @Test
    fun theEndOfTheLoan_isTheCoreText_andGivingBackIsSaid() {
        val taken = state()
        taken.onLendEnd(LendEnd.TAKEN_BACK.code, "Matthieu a repris l'ordinateur.")
        taken.onStatus(6, 0, 0, "erreur du cœur", false, false, false, null)
        assertEquals("Matthieu a repris l'ordinateur.", taken.ended.value!!.error)
        assertEquals(LendEnd.TAKEN_BACK, taken.lendEnd)
        // Rendu (depuis l'app ou le lanceur) : « Vous avez rendu l'ordinateur. » sur la carte de fin, jamais l'erreur de la session.
        val left = state()
        left.onLendEnd(LendEnd.LEFT.code, "Vous avez rendu l'ordinateur.")
        left.onStatus(6, 0, 0, "session arrêtée", false, false, false, null)
        assertEquals("Vous avez rendu l'ordinateur.", left.ended.value!!.error)
        // Sans le texte du cœur : celui de l'app.
        val leftQuiet = state()
        leftQuiet.onLendEnd(LendEnd.LEFT.code, "")
        leftQuiet.onStatus(6, 0, 0, null, false, false, false, null)
        assertEquals("You gave back the computer.", leftQuiet.ended.value!!.error)
        // Arrêté sans texte ni erreur : « Le prêt s'est arrêté. ».
        val stopped = state()
        stopped.onLendEnd(LendEnd.STOPPED.code, "")
        stopped.onStatus(6, 0, 0, null, false, false, false, null)
        assertEquals("The loan has stopped.", stopped.ended.value!!.error)
        // Le réseau perdu : pas de fin du prêt, l'erreur du cœur (l'ami peut revenir).
        val lost = state()
        lost.onStatus(6, 0, 0, "l'ordinateur prêté ne répond plus", false, false, false, null)
        assertNull(lost.lendEnd)
        assertEquals("l'ordinateur prêté ne répond plus", lost.ended.value!!.error)
        assertEquals(LendEnd.STOPPED, LendEnd.of(99))
    }

    @Test
    fun aNewRequest_isAnnouncedOnce_andASlotStartToo() {
        val s = state()
        val one = """{"enabled":true,"state":"free","requests":[{"id":"r1","name":"Léa","kind":"code","app":"ios","via":"lan","since":1}]}"""
        s.onLend(one)
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(Capsule.LendRequest("Léa"), s.capsule.value)
        s.capsule.value = null
        s.onLend(one)
        shadowOf(Looper.getMainLooper()).idle()
        assertNull(s.capsule.value)
        assertEquals(1, s.lend.value!!.requests.size)
        s.onLend("""{"enabled":true,"state":"free","borrower":{"name":"Tom"},"notice":{"reason":"slot_start","at":50},"requests":[]}""")
        shadowOf(Looper.getMainLooper()).idle()
        assertEquals(Capsule.LendHostNotice("Créneau de Tom : slot_start"), s.capsule.value)
        s.onLend(null)
        assertNull(s.lend.value)
    }

    @Test
    fun aSlotRequest_isAnnouncedWithItsSlot() {
        val s = state()
        s.onLend("""{"enabled":true,"state":"free","requests":[{"id":"r1","name":"Léa","kind":"slot","app":"ios","via":"lan","since":1,"slot":true}]}""")
        shadowOf(Looper.getMainLooper()).idle()
        // « Créneau de Léa : l'ordinateur lui est prêté dans 2 min. » (rien à refuser).
        assertEquals(Capsule.LendRequest("Léa", slot = true), s.capsule.value)
    }
}
