package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.model.Journal
import fr.leslignesclaires.kybernow.model.JournalPost
import fr.leslignesclaires.kybernow.model.JournalRelay
import fr.leslignesclaires.kybernow.model.JournalSeal
import fr.leslignesclaires.kybernow.model.JournalState
import fr.leslignesclaires.kybernow.model.PinnedTrust
import fr.leslignesclaires.kybernow.ui.settings.JournalQrStep
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.security.MessageDigest
import java.util.Base64
import kotlin.random.Random

/**
 * « Envoyer le journal » au téléviseur (docs/relais.md, « Dépôt d'un journal ») : le dépôt `KNJ1` (gzip puis AES-256-GCM,
 * clé seulement dans le QR code), la coupe à 2 Mio compressés, l'adresse du QR code et les réponses du relais.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class JournalUploadTest {
    @Test
    fun depositOpensWithTheKeyOnly() {
        val text = "KyberNow Link 0.11.8\nlignes du journal\n".repeat(200).toByteArray()
        val key = JournalSeal.newKey()
        val nonce = ByteArray(12) { it.toByte() }
        val deposit = JournalSeal.seal(JournalSeal.gzip(text), key, nonce)
        // `KNJ1`, le nonce, puis le chiffré et son étiquette de 16 octets.
        assertArrayEquals("KNJ1".toByteArray(), deposit.copyOfRange(0, 4))
        assertArrayEquals(nonce, deposit.copyOfRange(4, 16))
        assertEquals(4 + 12 + JournalSeal.gzip(text).size + 16, deposit.size)
        assertArrayEquals(text, JournalSeal.gunzip(JournalSeal.open(deposit, key)!!))
        // Une autre clé, ou un octet changé : rien ne s'ouvre.
        assertNull(JournalSeal.open(deposit, JournalSeal.newKey()))
        val broken = deposit.copyOf().also { it[30] = (it[30] + 1).toByte() }
        assertNull(JournalSeal.open(broken, key))
        assertNull(JournalSeal.open("KNJ0".toByteArray() + deposit.copyOfRange(4, deposit.size), key))
        // Deux dépôts du même journal ne se ressemblent pas (nonce et clé tirés).
        assertFalse(JournalSeal.seal(JournalSeal.gzip(text), key).contentEquals(JournalSeal.seal(JournalSeal.gzip(text), key)))
    }

    @Test
    fun smallJournalIsKeptWhole() {
        val text = "en-tête\n===== java.log =====\nrien\n".toByteArray()
        assertArrayEquals(text, JournalSeal.gunzip(JournalSeal.fit(text, Journal.headEnd(text))))
    }

    @Test
    fun bigJournalKeepsTheHeadAndTheEnd() {
        // Un journal qui se compresse mal (texte tiré au hasard), bien plus gros que la limite.
        val random = Random(7)
        val head = "KyberNow Link 0.11.8 (118)\nAppareil : Google Chromecast\n\n===== fins.log =====\nplantage natif\n"
        val body = buildString { repeat(6000) { i -> append("ligne $i ").append(Base64.getEncoder().encodeToString(random.nextBytes(48))).append('\n') } }
        val text = (head + Journal.AFTER_HEAD + body + "dernière ligne\n").toByteArray()
        assertEquals(head.length, Journal.headEnd(text))
        val max = 64 * 1024
        val z = JournalSeal.fit(text, Journal.headEnd(text), max)
        assertTrue("${z.size} > $max", z.size <= max)
        val out = JournalSeal.gunzip(z).toString(Charsets.UTF_8)
        assertTrue(out.startsWith(head))
        assertTrue(out.endsWith("dernière ligne\n"))
        assertTrue(out.contains("début du journal coupé"))
        // La fin gardée commence à une ligne entière, et tient une bonne part de la limite.
        val kept = out.substringAfter("2 Mio =====\n")
        assertTrue(kept.startsWith("ligne "))
        assertTrue(out.length > max)
    }

    @Test
    fun qrAddressCarriesTheKeyAfterTheHash() {
        val key = ByteArray(32) { 0xFB.toByte() }
        val url = JournalSeal.qrUrl(JournalRelay.parse("share.kybernow.app")!!, "AAAAAAAAAAAAAAAAAAAAAA", key)
        val k = url.substringAfter("#k=")
        assertEquals("https://share.kybernow.app/journal/AAAAAAAAAAAAAAAAAAAAAA#k=$k", url)
        assertEquals(43, k.length)
        assertFalse(k.contains('=') || k.contains('+') || k.contains('/'))
        assertArrayEquals(key, Base64.getUrlDecoder().decode(k))
        assertEquals(43, JournalSeal.keyText(JournalSeal.newKey()).length)
        // Le port d'un relais hors du 443 reste dans l'adresse ; l'empreinte, jamais.
        val pinned = JournalRelay.parse("relais.exemple.org:9443#sha256:" + "ab".repeat(32))!!
        assertTrue(JournalSeal.qrUrl(pinned, "AAAAAAAAAAAAAAAAAAAAAA", key).startsWith("https://relais.exemple.org:9443/journal/"))
    }

    @Test
    fun relayAddresses() {
        assertEquals(JournalRelay("share.kybernow.app", 443, null), JournalRelay.parse("https://Share.KyberNow.app/"))
        assertEquals("relais.exemple.org:9443", JournalRelay.parse("relais.exemple.org:9443")!!.authority)
        assertEquals("[fe80::1]:9443", JournalRelay.parse("[fe80::1]:9443")!!.authority)
        assertEquals("[fe80::1]", JournalRelay.parse("fe80::1")!!.authority)
        val pin = JournalRelay.parse("r.exemple.org#sha256:" + (0 until 32).joinToString(":") { "%02x".format(it) })!!.pin!!
        assertArrayEquals(ByteArray(32) { it.toByte() }, pin)
        assertNull(JournalRelay.parse("r.exemple.org#sha256:abcd"))
        assertNull(JournalRelay.parse(""))
        assertNull(JournalRelay.parse("r.exemple.org:0"))
        assertNull(JournalRelay.parse("moi@r.exemple.org"))
        // L'empreinte épinglée : celle du certificat entier.
        val der = "certificat".toByteArray()
        assertTrue(PinnedTrust.matches(der, MessageDigest.getInstance("SHA-256").digest(der)))
        assertFalse(PinnedTrust.matches(der, ByteArray(32)))
    }

    @Test
    fun relayAnswers() {
        val id = "Q2hhcmxvdHRlIGV0IGxlcw"
        val owner = "b3duZXItdG9rZW4tMTI4Yg"
        assertEquals(JournalPost.Posted(id, owner, 3600), JournalSeal.readPost(201, """{"id":"$id","owner":"$owner","expires_in":3600}""", null))
        assertEquals(JournalPost.Unreachable, JournalSeal.readPost(201, """{"id":"../x","owner":"$owner"}""", null))
        assertEquals(JournalPost.Unreachable, JournalSeal.readPost(201, "pas du JSON", null))
        assertEquals(JournalPost.TooMany(120), JournalSeal.readPost(429, """{"error":"too_many","retry_after":300}""", "120"))
        assertEquals(JournalPost.TooMany(300), JournalSeal.readPost(429, """{"error":"too_many","retry_after":300}""", null))
        assertEquals(JournalPost.Full, JournalSeal.readPost(503, """{"error":"full"}""", null))
        assertEquals(JournalPost.TooLarge, JournalSeal.readPost(413, """{"error":"too_large"}""", null))
        assertEquals(JournalPost.OldRelay, JournalSeal.readPost(404, "Not Found", null))
        assertEquals(JournalPost.Unreachable, JournalSeal.readPost(-1, null, null))
        assertEquals(JournalPost.Unreachable, JournalSeal.readPost(500, null, null))

        assertEquals(JournalState.WAITING, JournalSeal.readState(200, """{"state":"waiting"}"""))
        assertEquals(JournalState.OPENED, JournalSeal.readState(200, """{"state":"opened"}"""))
        assertEquals(JournalState.GONE, JournalSeal.readState(404, """{"error":"gone"}"""))
        assertEquals(JournalState.UNKNOWN, JournalSeal.readState(500, null))
    }

    @Test
    fun stepAfterTheDeposit() {
        val relay = JournalRelay.parse("share.kybernow.app")!!
        val key = ByteArray(32)
        val shown = JournalQrStep.after(JournalPost.Posted("AAAAAAAAAAAAAAAAAAAAAA", "BBBBBBBBBBBBBBBBBBBBBB", 3600), relay, key, 1000L)
        assertTrue(shown is JournalQrStep.Shown)
        shown as JournalQrStep.Shown
        assertEquals(1000L + 3_600_000L, shown.until)
        assertNotNull(shown.url.substringAfter("#k=", "").takeIf { it.length == 43 })
        assertEquals(JournalQrStep.TooMany, JournalQrStep.after(JournalPost.TooMany(60), relay, key, 0))
        assertEquals(JournalQrStep.TooMany, JournalQrStep.after(JournalPost.Full, relay, key, 0))
        assertEquals(JournalQrStep.OldRelay, JournalQrStep.after(JournalPost.OldRelay, relay, key, 0))
        assertEquals(JournalQrStep.Unreachable, JournalQrStep.after(JournalPost.Unreachable, relay, key, 0))
    }
}
