package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.util.Log
import fr.leslignesclaires.kybernow.core.AccountReply
import fr.leslignesclaires.kybernow.core.Accounts
import fr.leslignesclaires.kybernow.core.Core
import fr.leslignesclaires.kybernow.core.StoredAccount
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONObject

/*
 * Réglages synchronisés (docs/comptes.md, « Réglages synchronisés et déplacer un ordinateur » ; ce que fait chaque app :
 * apps/core/README.md, « Réglages synchronisés »). Le profil, le format vidéo, le débit, le texte net (4:4:4), le HDR
 * et « Stylet : Pression / Souris » (`pen`) de chaque ordinateur d'un compte suivent la personne d'un appareil à
 * l'autre ; Android n'a pas de mode immersif : il ne l'envoie pas et n'y touche pas.
 *
 * Les réglages d'Android ([HostSettings]) : un profil toujours choisi, dont « Format vidéo » et « Débit » s'écartent
 * (« Personnalisé » pour le relais : `"profile": null`, avec `codec`, `yuv444` et `mbps`) ; le 4:4:4 suit le profil
 * (Bureau, Max) et ce que l'appareil décode, il n'est jamais réglé à la main ici ; le transport ne se synchronise pas.
 */

/** Les clés d'un ordinateur, entre les réglages d'Android et le relais ; sans état, pour les essais. */
object SyncedKeys {
    /** « Personnalisé » pour le relais : le format ou le débit s'écartent du profil (le transport ne compte pas). */
    fun custom(set: HostSettings) = set.codec != "auto" || set.mbps != 0

    /**
     * Les réglages de session de cet ordinateur tels que le relais les garde : le plan du cœur ([SessionMode.plan],
     * `plan::plan`, comme l'app Windows) pour la 4K à 60 Hz (le débit d'un profil y est calé), avec son choix « HDR ».
     */
    fun stream(host: Host, decoders: Decoders): JSONObject =
        SessionMode.plan(host, decoders, Triple(3840, 2160, 60), host.settings.profile).settings.put("hdr", host.settings.hdr)

    /**
     * Toutes les clés (`SyncedValues::from_stream` du cœur, puis `with_pen` : le choix du stylet [pen], quand il est
     * donné) : la première fois qu'un ordinateur n'a rien au relais.
     */
    fun full(stream: JSONObject, pen: String? = null): JSONObject =
        (Core.settingsFromStream(stream) ?: JSONObject()).also { o -> PenMode.parse(pen)?.let { o.put("pen", it) } }

    /**
     * Les seules clés changées entre [before] et [after] (`stream` : les réglages de session d'[after]) ; nul si rien ne
     * change pour le relais. Un profil → `{profile}` ; passer en « Personnalisé » → `{profile: null, codec, yuv444,
     * mbps}` ; le format ou le débit d'un réglage déjà personnalisé → la clé changée et `profile: null` ; le HDR → `{hdr}` ;
     * le stylet → `{pen}`.
     */
    fun changed(before: HostSettings, after: HostSettings, stream: JSONObject): JSONObject? {
        val v = JSONObject()
        when {
            !custom(after) -> if (custom(before) || before.profile != after.profile) v.put("profile", after.profile.id)
            !custom(before) -> v.put("profile", JSONObject.NULL).put("codec", stream.optString("codec")).put("yuv444", stream.optBoolean("yuv444")).put("mbps", stream.optInt("mbps"))
            else -> {
                if (before.codec != after.codec) v.put("profile", JSONObject.NULL).put("codec", stream.optString("codec")).put("yuv444", stream.optBoolean("yuv444"))
                if (before.mbps != after.mbps) v.put("profile", JSONObject.NULL).put("mbps", stream.optInt("mbps"))
            }
        }
        if (before.hdr != after.hdr) v.put("hdr", after.hdr)
        if (before.pen != after.pen) v.put("pen", after.pen)
        return v.takeIf { it.length() > 0 }
    }

    /**
     * Les réglages de session [base] avec les clés reçues [values], ramenés à ce que l'appareil décode ([caps], `{hevc,
     * hevc444, av1, hdr, pyrowave}`) : `SyncedValues::apply_to` du cœur.
     */
    fun applyStream(values: JSONObject, base: JSONObject, caps: JSONObject): JSONObject =
        Core.settingsApply(values, base, caps) ?: JSONObject(base.toString())

    /**
     * Les réglages d'Android après les clés reçues ([values], déjà vérifiées par le cœur) et leur application par le cœur ([result],
     * [applyStream]) : un profil le reprend entier (format et débit d'Android remis à « Auto », le transport gardé) ;
     * « Personnalisé » prend le format et le débit retenus par le cœur ; un format ou un débit sans profil ne touche
     * qu'un réglage déjà personnalisé ; le HDR tel quel ; le stylet s'il est lisible (`pressure`, `mouse` ; une clé d'une
     * version plus récente ne change rien).
     */
    fun applied(current: HostSettings, values: JSONObject, result: JSONObject): HostSettings {
        var s = current
        val codec = result.optString("codec").takeIf { it in listOf("h264", "hevc", "av1", "pyrowave") }
        val mbps = result.optInt("mbps").takeIf { it > 0 }
        if (values.has("profile")) {
            if (values.isNull("profile")) s = s.copy(codec = codec ?: s.codec, mbps = mbps ?: s.mbps)
            else Profile.entries.firstOrNull { it.id == values.optString("profile") }?.let { p -> s = s.copy(profile = p, codec = "auto", mbps = 0) }
        } else if (custom(current)) {
            if (values.has("codec") && codec != null) s = s.copy(codec = codec)
            if (values.has("mbps") && mbps != null) s = s.copy(mbps = mbps)
        }
        if (values.has("hdr")) s = s.copy(hdr = values.optBoolean("hdr"))
        if (values.has("pen")) PenMode.parse(values.optString("pen"))?.let { s = s.copy(pen = it) }
        return s
    }
}

/** Ce que [SettingsSync] demande au relais (le cœur), remplaçable dans les essais. */
interface SettingsRemote {
    suspend fun fetch(account: StoredAccount): AccountReply<List<JSONObject>>
    suspend fun put(account: StoredAccount, station: String, last: JSONObject?, values: JSONObject): AccountReply<JSONObject>

    object Core : SettingsRemote {
        override suspend fun fetch(account: StoredAccount) = Accounts.settingsFetch(account)
        override suspend fun put(account: StoredAccount, station: String, last: JSONObject?, values: JSONObject) = Accounts.settingsPut(account, station, last, values)
    }
}

/**
 * Les réglages synchronisés sur cet appareil : l'interrupteur « Synchroniser les réglages des ordinateurs » (allumé par
 * défaut, gardé ici, jamais synchronisé), et pour chaque compte la mémoire du cœur (`account::AccountSync`, comme
 * Windows et Linux : la dernière liste lue, le `updated_at` déjà appliqué de chaque ordinateur, et les changements que le
 * relais n'a pas pris, à renvoyer au rafraîchissement suivant), tenue en JSON par l'app et menée par le cœur
 * ([Core.accountSync]). Rien de secret : en clair, à côté des comptes.
 *
 * [signedOut] : un refus 70 (la session de l'appareil est fermée) ; le compte est oublié par [AccountManager].
 */
class SettingsSync(
    context: Context,
    private val hosts: HostStore,
    private val decoders: () -> Decoders,
    private val account: (String) -> StoredAccount?,
    private val scope: CoroutineScope,
    private val remote: SettingsRemote = SettingsRemote.Core,
    private val signedOut: (key: String, message: String) -> Unit = { _, _ -> },
) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val lock = Mutex()
    private val _enabled = MutableStateFlow(prefs.getBoolean("enabled", true))
    /** « Synchroniser les réglages des ordinateurs » : coupé, ni lecture ni envoi. */
    val enabled: StateFlow<Boolean> = _enabled.asStateFlow()

    fun setEnabled(on: Boolean) {
        prefs.edit().putBoolean("enabled", on).apply()
        _enabled.value = on
    }

    // MARK: - Ce qui est gardé : la mémoire du cœur, une par compte

    private fun memories(): JSONObject = runCatching { JSONObject(prefs.getString(MEMORIES, null) ?: "{}") }.getOrDefault(JSONObject())

    /** La mémoire d'un compte (JSON de `AccountSync`) ; vide : neuve. */
    private fun memory(key: String): String = memories().optJSONObject(key)?.toString().orEmpty()

    private fun keep(key: String, sync: JSONObject?) {
        val all = memories()
        if (sync == null) all.remove(key) else all.put(key, sync)
        prefs.edit().putString(MEMORIES, all.toString()).apply()
    }

    /** Une opération du cœur sur la mémoire d'un compte, gardée ; rend sa valeur (nul sans réponse). */
    private fun op(key: String, name: String, station: String = "", a: String = "", b: String = ""): Any? {
        val r = Core.accountSync(name, memory(key), station, a, b) ?: return null
        r.optJSONObject("sync")?.let { keep(key, it) }
        return r.opt("value")
    }

    /** La dernière liste lue d'un compte, par Station. */
    fun list(key: String): Map<String, JSONObject> {
        val a = runCatching { JSONObject(memory(key)).optJSONArray("list") }.getOrNull() ?: return emptyMap()
        return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.associateBy { it.optString("station") }
    }

    /** Le `updated_at` déjà appliqué (ou envoyé) pour cet ordinateur. */
    fun appliedAt(key: String, station: String): Long =
        runCatching { JSONObject(memory(key)).optJSONObject("applied")?.optLong(station, 0) }.getOrNull() ?: 0

    /** Les clés que le relais n'a pas prises (relais muet), à renvoyer. */
    fun pending(key: String, station: String): JSONObject? =
        runCatching { JSONObject(memory(key)).optJSONObject("pending")?.optJSONObject(station) }.getOrNull()

    /** Le compte est oublié : tout ce qui le concerne aussi. */
    fun forgetAccount(key: String) = keep(key, null)

    /**
     * La mémoire gardée par une version d'avant la 0.11.8 (`lists`, `applied`, `pending`, l'ordinateur noté
     * « compte\nStation ») : reprise une fois dans la forme du cœur, pour qu'un changement pas encore envoyé ne se perde
     * pas, puis retirée.
     */
    private fun migrate() {
        if (OLD.none(prefs::contains)) return
        fun obj(name: String) = runCatching { JSONObject(prefs.getString(name, null) ?: "{}") }.getOrDefault(JSONObject())
        val lists = obj("lists")
        val all = memories()
        fun mem(key: String) = all.optJSONObject(key) ?: JSONObject().put("list", org.json.JSONArray()).put("applied", JSONObject())
            .put("pending", JSONObject()).also { all.put(key, it) }
        lists.keys().forEach { key -> mem(key).put("list", lists.optJSONArray(key) ?: org.json.JSONArray()) }
        for (name in listOf("applied", "pending")) {
            val o = obj(name)
            o.keys().forEach { k -> if ('\n' in k) mem(k.substringBefore('\n')).getJSONObject(name).put(k.substringAfter('\n'), o.get(k)) }
        }
        prefs.edit().putString(MEMORIES, all.toString()).apply { OLD.forEach(::remove) }.apply()
    }

    init {
        migrate()
    }

    // MARK: - Lire

    /**
     * Les réglages du compte : d'abord les changements que le relais n'avait pas pris, puis la liste, dont le cœur rend
     * les ordinateurs à appliquer (plus récents que ce qui l'est déjà ; un ordinateur sorti du compte est oublié).
     * Relais sans réglages synchronisés (77) ou muet : rien, sans rien dire.
     */
    suspend fun sync(a: StoredAccount) {
        if (!enabled.value) return
        lock.withLock {
            val waiting = op(a.key, "pending") as? JSONObject ?: JSONObject()
            for (station in waiting.keys().asSequence().toList()) {
                val values = waiting.optJSONObject(station) ?: continue
                if (!send(a, station, values)) return
            }
            when (val r = remote.fetch(a)) {
                is AccountReply.Ok -> {
                    val fresh = org.json.JSONArray().apply { r.value.forEach { put(it) } }
                    val apply = op(a.key, "fetched", a = fresh.toString()) as? org.json.JSONArray ?: return
                    for (i in 0 until apply.length()) {
                        val o = apply.optJSONObject(i) ?: continue
                        apply(a.key, o.optString("station"), o.optJSONObject("values") ?: JSONObject())
                    }
                }
                is AccountReply.Refused -> refused(a, null, r)
            }
        }
    }

    /** Les clés reçues, écrites dans les réglages de chaque ordinateur de l'appareil qui est cet ordinateur du compte. */
    private fun apply(key: String, station: String, values: JSONObject) {
        if (values.length() == 0) return
        val d = decoders()
        for (h in hosts.hosts.value.filter { it.account?.let { a -> a.key == key && a.station == station } == true }) {
            val stream = SyncedKeys.stream(h, d)
            val result = SyncedKeys.applyStream(values, stream, Hdr.capabilities(h, d))
            val next = SyncedKeys.applied(h.settings, values, result)
            if (next != h.settings) hosts.applySynced(h.id) { it.copy(settings = next) }
        }
    }

    // MARK: - Envoyer

    /**
     * Un réglage synchronisé changé par la personne pour un ordinateur du compte (réglages de l'ordinateur, profil ou HDR
     * de la session) : seulement les clés changées, avec ce qui attendait encore ; tout la première fois qu'il n'a rien
     * au relais (`AccountSync::to_send` du cœur).
     */
    fun changed(before: Host, after: Host) {
        if (!enabled.value || before.settings == after.settings) return
        val acc = after.account ?: return
        val a = account(acc.key) ?: return
        val stream = SyncedKeys.stream(after, decoders())
        val keys = SyncedKeys.changed(before.settings, after.settings, stream) ?: return
        scope.launch {
            lock.withLock {
                val values = op(acc.key, "to_send", acc.station, keys.toString(), SyncedKeys.full(stream, after.settings.pen).toString()) as? JSONObject ?: return@withLock
                send(a, acc.station, values)
            }
        }
    }

    /** Envoie ; faux pour arrêter (relais muet, compte fermé). Muet : gardé pour le rafraîchissement suivant. */
    private suspend fun send(a: StoredAccount, station: String, values: JSONObject): Boolean {
        val last = op(a.key, "last", station) as? JSONObject
        return when (val r = remote.put(a, station, last, values)) {
            is AccountReply.Ok -> {
                op(a.key, "sent", a = r.value.toString())
                true
            }
            is AccountReply.Refused -> refused(a, station to values, r)
        }
    }

    private fun refused(a: StoredAccount, sent: Pair<String, JSONObject>?, r: AccountReply.Refused): Boolean {
        when {
            r.signedOut -> {
                signedOut(a.key, r.message)
                return false
            }
            r.unknownComputer -> sent?.let { op(a.key, "forget", it.first) }
            r.code == fr.leslignesclaires.kybernow.core.AccountCode.UNREACHABLE -> {
                sent?.let { op(a.key, "unsent", it.first, it.second.toString()) }
                return false
            }
            // Relais d'avant les réglages synchronisés, trop grand, valeurs refusées : rien à renvoyer.
            else -> sent?.let { op(a.key, "refused", it.first, it.second.toString()) }
        }
        Log.i("KyberNow", "réglages synchronisés : ${a.relay} refuse (${r.kind})")
        return true
    }

    private companion object {
        const val PREFS = "settings_sync"
        /** Les mémoires des comptes (`AccountSync` du cœur), par clé de compte. */
        const val MEMORIES = "memories"
        /** Ce que gardaient les versions d'avant la 0.11.8 ([migrate]). */
        val OLD = listOf("lists", "applied", "pending")
    }
}
