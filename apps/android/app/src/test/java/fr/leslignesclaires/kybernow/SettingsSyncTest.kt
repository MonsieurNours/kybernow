package fr.leslignesclaires.kybernow

import android.app.Application
import fr.leslignesclaires.kybernow.core.AccountCode
import fr.leslignesclaires.kybernow.core.AccountReply
import fr.leslignesclaires.kybernow.core.StoredAccount
import fr.leslignesclaires.kybernow.model.DecoderInfo
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostAccount
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.HostStore
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.SettingsRemote
import fr.leslignesclaires.kybernow.model.SettingsSync
import fr.leslignesclaires.kybernow.model.SyncedKeys
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

/**
 * Réglages synchronisés (docs/comptes.md, « Réglages synchronisés » ; apps/core/README.md), sans bibliothèque native : les
 * clés d'Android (seulement celles qui changent, « Personnalisé » entier, jamais le mode immersif), ce qui est appliqué
 * en arrivant (le plus récent seulement, sans rien renvoyer), le relais muet (renvoyé au rafraîchissement suivant),
 * l'ordinateur oublié, le relais d'avant, l'interrupteur coupé.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], application = Application::class)
class SettingsSyncTest {
    private val key = "share.kybernow.app|acc_lea"
    private val station = "abcdefghijklmnopqrstuvwxyz"
    private val decoders = Decoders(DecoderInfo("avc", 4096, 2160, true), DecoderInfo("hevc", 4096, 2160, true), null, true)

    private val account = StoredAccount.parse(
        JSONObject().put("relay", "share.kybernow.app").put("token", "T")
            .put("account", JSONObject().put("id", "acc_lea").put("name", "Léa").put("email", "lea@exemple.fr")).toString(),
    )!!

    private fun stream(set: HostSettings) = SyncedKeys.stream(set, SyncedKeys.profileBase(set.profile, hevc = true, hevc444 = true, av1 = false), hevc = true, av1 = false)

    private fun synced(at: Long, values: JSONObject) = JSONObject().put("station", station).put("updated_at", at).put("values", values)

    /** Un faux relais : ce qu'il rend, et ce qu'on lui envoie. */
    private class Relay : SettingsRemote {
        var list: AccountReply<List<JSONObject>> = AccountReply.Ok(emptyList())
        var refuse: AccountReply.Refused? = null
        var at = 100L
        val puts = mutableListOf<Pair<String, JSONObject>>()
        var fetches = 0

        override suspend fun fetch(account: StoredAccount): AccountReply<List<JSONObject>> {
            fetches++
            return list
        }

        override suspend fun put(account: StoredAccount, station: String, last: JSONObject?, values: JSONObject): AccountReply<JSONObject> {
            puts += station to JSONObject(values.toString())
            refuse?.let { return it }
            val merged = JSONObject((last?.optJSONObject("values") ?: JSONObject()).toString())
            values.keys().forEach { merged.put(it, values.get(it)) }
            return AccountReply.Ok(JSONObject().put("station", station).put("updated_at", ++at).put("values", merged))
        }
    }

    private fun setup(relay: Relay, settings: HostSettings = HostSettings()): Triple<HostStore, SettingsSync, Host> {
        val app = RuntimeEnvironment.getApplication()
        val hosts = HostStore(app)
        val sync = SettingsSync(app, hosts, { decoders }, { k -> account.takeIf { it.key == k } }, CoroutineScope(Dispatchers.Unconfined), relay)
        hosts.onSettingsChanged = sync::changed
        val h = Host(id = "pc", name = "PC du salon", address = "", port = 0, fingerprint = "sha256:aa", user = "", settings = settings, account = HostAccount(key, station))
        hosts.put(h)
        return Triple(hosts, sync, h)
    }

    // MARK: - Les clés

    @Test
    fun onlyTheChangedKeys_areSent() {
        val game = HostSettings(profile = Profile.GAME)
        // Un profil : {profile}.
        val desk = game.apply(Profile.DESKTOP)
        assertEquals("""{"profile":"bureau"}""", SyncedKeys.changed(game, desk, stream(desk)).toString())
        // Passer en « Personnalisé » : tout ce qui le fait.
        val h264 = game.copy(codec = "h264")
        val custom = SyncedKeys.changed(game, h264, stream(h264))!!
        assertTrue(custom.isNull("profile"))
        assertEquals("h264", custom.getString("codec"))
        assertFalse(custom.getBoolean("yuv444"))
        assertEquals(60, custom.getInt("mbps"))
        // Déjà personnalisé : la clé changée et profile: null.
        val faster = h264.copy(mbps = 150)
        assertEquals(setOf("profile", "mbps"), SyncedKeys.changed(h264, faster, stream(faster))!!.keys().asSequence().toSet())
        assertEquals(150, SyncedKeys.changed(h264, faster, stream(faster))!!.getInt("mbps"))
        // Le HDR seul ; le transport ou la manette à l'écran ne partent pas ; jamais le mode immersif.
        assertEquals("""{"hdr":false}""", SyncedKeys.changed(game, game.copy(hdr = false), stream(game)).toString())
        assertNull(SyncedKeys.changed(game, game.withTransport(true), stream(game)))
        assertNull(SyncedKeys.changed(game, game.copy(onScreenPad = true), stream(game)))
        // Revenir à un profil depuis « Personnalisé ».
        assertEquals("""{"profile":"jeu"}""", SyncedKeys.changed(faster, faster.apply(Profile.GAME), stream(game)).toString())
    }

    @Test
    fun theFirstTime_everythingIsSent() {
        val full = SyncedKeys.full(stream(HostSettings(profile = Profile.DESKTOP)))
        assertEquals("bureau", full.getString("profile"))
        assertEquals("hevc", full.getString("codec"))
        assertTrue(full.getBoolean("yuv444"))
        assertEquals(90, full.getInt("mbps"))
        assertTrue(full.getBoolean("hdr"))
        assertFalse(full.has("immersive"))
    }

    @Test
    fun whatArrives_isAppliedAsThisDeviceCan() {
        val custom = HostSettings(profile = Profile.MAX, codec = "h264", mbps = 40, transport = "reliable")
        // Un profil : repris entier, le transport gardé.
        val p = JSONObject().put("profile", "jeu")
        val toGame = SyncedKeys.applied(custom, p, SyncedKeys.applyStream(p, stream(custom), JSONObject().put("hevc", true)))
        assertEquals(Profile.GAME, toGame.profile)
        assertEquals("auto", toGame.codec)
        assertEquals(0, toGame.mbps)
        assertEquals("reliable", toGame.transport)
        // « Personnalisé » en AV1, que cet appareil ne décode pas : le format reste, le débit arrive.
        val av1 = JSONObject().put("profile", JSONObject.NULL).put("codec", "av1").put("mbps", 80)
        val kept = SyncedKeys.applied(custom, av1, SyncedKeys.applyStream(av1, stream(custom), JSONObject().put("hevc", true).put("av1", false)))
        assertEquals("h264", kept.codec)
        assertEquals(80, kept.mbps)
        // Un format seul ne touche pas un réglage qui suit son profil ; le HDR, tel quel.
        val game = HostSettings(profile = Profile.GAME)
        val codec = JSONObject().put("codec", "h264").put("hdr", false)
        val g = SyncedKeys.applied(game, codec, SyncedKeys.applyStream(codec, stream(game), JSONObject().put("hevc", true)))
        assertEquals("auto", g.codec)
        assertFalse(g.hdr)
    }

    // MARK: - Lire et envoyer

    @Test
    fun aChange_isSent_firstEverything_thenOnlyTheKey() = runBlocking {
        val relay = Relay()
        val (hosts, sync, h) = setup(relay)
        hosts.update(h.id) { it.copy(settings = it.settings.apply(Profile.DESKTOP)) }
        assertEquals(1, relay.puts.size)
        assertEquals(setOf("profile", "codec", "yuv444", "mbps", "hdr"), relay.puts[0].second.keys().asSequence().toSet())
        assertEquals(101L, sync.appliedAt(key, station))
        hosts.update(h.id) { it.copy(settings = it.settings.copy(hdr = false)) }
        assertEquals("""{"hdr":false}""", relay.puts[1].second.toString())
        // Rien de synchronisé n'a changé : rien ne part.
        hosts.update(h.id) { it.copy(settings = it.settings.copy(padSize = 2)) }
        assertEquals(2, relay.puts.size)
    }

    @Test
    fun whatArrives_newerOnly_andNeverSentBack() = runBlocking {
        val relay = Relay()
        val (hosts, sync, h) = setup(relay)
        relay.list = AccountReply.Ok(listOf(synced(50, JSONObject().put("profile", "max").put("hdr", false).put("immersive", "always"))))
        sync.sync(account)
        assertEquals(Profile.MAX, hosts.get(h.id)!!.settings.profile)
        assertFalse(hosts.get(h.id)!!.settings.hdr)
        assertTrue(relay.puts.isEmpty())
        // Déjà appliqué : un réglage local changé depuis n'est pas écrasé par la même version.
        hosts.applySynced(h.id) { it.copy(settings = it.settings.apply(Profile.GAME)) }
        sync.sync(account)
        assertEquals(Profile.GAME, hosts.get(h.id)!!.settings.profile)
        // Plus récent : appliqué.
        relay.list = AccountReply.Ok(listOf(synced(60, JSONObject().put("profile", "bureau"))))
        sync.sync(account)
        assertEquals(Profile.DESKTOP, hosts.get(h.id)!!.settings.profile)
        assertTrue(relay.puts.isEmpty())
    }

    @Test
    fun aMuteRelay_keepsTheChange_andSendsItBack_beforeApplying() = runBlocking {
        val relay = Relay()
        val (hosts, sync, h) = setup(relay)
        relay.list = AccountReply.Ok(listOf(synced(50, JSONObject().put("profile", "jeu"))))
        sync.sync(account)
        relay.refuse = AccountReply.Refused(AccountCode.UNREACHABLE, "unreachable", "Le relais ne répond pas.")
        hosts.update(h.id) { it.copy(settings = it.settings.copy(hdr = false)) }
        assertEquals("""{"hdr":false}""", sync.pending(key, station).toString())
        // Au rafraîchissement suivant : renvoyé d'abord, puis la liste (déjà à jour pour cet ordinateur).
        relay.refuse = null
        relay.list = AccountReply.Ok(listOf(synced(101, JSONObject().put("profile", "jeu").put("hdr", false))))
        sync.sync(account)
        assertEquals(2, relay.puts.size)
        assertNull(sync.pending(key, station))
        assertFalse(hosts.get(h.id)!!.settings.hdr)
    }

    @Test
    fun aComputerGone_forgetsItsSettings_theLocalOnesStay() = runBlocking {
        val relay = Relay()
        val (hosts, sync, h) = setup(relay)
        relay.list = AccountReply.Ok(listOf(synced(50, JSONObject().put("profile", "max"))))
        sync.sync(account)
        assertEquals(50L, sync.appliedAt(key, station))
        relay.list = AccountReply.Ok(emptyList())
        sync.sync(account)
        assertTrue(sync.list(key).isEmpty())
        assertEquals(0L, sync.appliedAt(key, station))
        assertEquals(Profile.MAX, hosts.get(h.id)!!.settings.profile)
        // 71 en envoyant : oublié aussi.
        relay.refuse = AccountReply.Refused(AccountCode.UNKNOWN_COMPUTER, "unknown_computer", "")
        hosts.update(h.id) { it.copy(settings = it.settings.copy(hdr = false)) }
        assertTrue(sync.list(key).isEmpty())
        assertNull(sync.pending(key, station))
    }

    @Test
    fun anOlderRelay_isSilent_andSwitchedOff_nothingHappens() = runBlocking {
        val relay = Relay()
        val (hosts, sync, h) = setup(relay)
        relay.list = AccountReply.Refused(AccountCode.NO_ACCOUNTS, "no_accounts", "")
        relay.refuse = AccountReply.Refused(AccountCode.NO_ACCOUNTS, "no_accounts", "")
        sync.sync(account)
        hosts.update(h.id) { it.copy(settings = it.settings.copy(hdr = false)) }
        assertNull(sync.pending(key, station))
        assertFalse(hosts.get(h.id)!!.settings.hdr)
        // Coupé : ni lecture ni envoi.
        sync.setEnabled(false)
        val fetches = relay.fetches
        val puts = relay.puts.size
        sync.sync(account)
        hosts.update(h.id) { it.copy(settings = it.settings.copy(hdr = true)) }
        assertEquals(fetches, relay.fetches)
        assertEquals(puts, relay.puts.size)
        // Allumé par défaut, gardé sur l'appareil.
        assertFalse(SettingsSync(RuntimeEnvironment.getApplication(), hosts, { decoders }, { null }, CoroutineScope(Dispatchers.Unconfined), relay).enabled.value)
    }
}
