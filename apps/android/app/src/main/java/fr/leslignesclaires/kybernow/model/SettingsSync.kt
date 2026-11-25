package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.util.Log
import fr.leslignesclaires.kybernow.core.AccountReply
import fr.leslignesclaires.kybernow.core.Accounts
import fr.leslignesclaires.kybernow.core.Native
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
 * apps/core/README.md, « Réglages synchronisés »). Le profil, le format vidéo, le débit, le texte net (4:4:4) et le HDR
 * de chaque ordinateur d'un compte suivent la personne d'un appareil à l'autre ; Android n'a pas de mode immersif : il
 * ne l'envoie pas et n'y touche pas.
 *
 * Les réglages d'Android ([HostSettings]) : un profil toujours choisi, dont « Format vidéo » et « Débit » s'écartent
 * (« Personnalisé » pour le relais : `"profile": null`, avec `codec`, `yuv444` et `mbps`) ; le 4:4:4 suit le profil
 * (Bureau, Max) et ce que l'appareil décode, il n'est jamais réglé à la main ici ; le transport ne se synchronise pas.
 */

/** Les clés d'un ordinateur, entre les réglages d'Android et le relais ; sans état, pour les essais. */
object SyncedKeys {
    /** « Personnalisé » pour le relais : le format ou le débit s'écartent du profil (le transport ne compte pas). */
    fun custom(set: HostSettings) = set.codec != "auto" || set.mbps != 0

    /** Les réglages d'un profil sur cet appareil (`StreamSettings::for_profile` du cœur), le JSON de `sessionStart`. */
    fun profileBase(profile: Profile, hevc: Boolean, hevc444: Boolean, av1: Boolean): JSONObject =
        runCatching { JSONObject(Native.profileSettings(profile.id, hevc, hevc444, av1)) }.getOrElse {
            // Sans bibliothèque native : la même règle que le cœur.
            val (yuv, mbps, reliable) = when (profile) {
                Profile.GAME -> Triple(false, 60, false)
                Profile.DESKTOP -> Triple(true, 90, true)
                Profile.MAX -> Triple(true, 300, false)
            }
            JSONObject().put("profile", profile.id).put("codec", if (hevc) "hevc" else "h264").put("yuv444", yuv && hevc && hevc444)
                .put("mbps", mbps).put("reliable", reliable)
        }

    /**
     * Les réglages de session de [set] (le JSON de `sessionStart`, comme `SessionActivity.settingsJson`) d'après la base de
     * son profil ([profileBase]) ; `profile` nul quand ils s'écartent du profil ([custom]).
     */
    fun stream(set: HostSettings, base: JSONObject, hevc: Boolean, av1: Boolean): JSONObject {
        val o = JSONObject(base.toString())
        when (set.codec) {
            "pyrowave" -> o.put("codec", "pyrowave").put("yuv444", false)
            "hevc" -> if (hevc) o.put("codec", "hevc")
            "h264" -> o.put("codec", "h264").put("yuv444", false)
            "av1" -> if (av1) o.put("codec", "av1").put("yuv444", false)
        }
        if (set.mbps > 0) o.put("mbps", set.mbps)
        if (custom(set)) o.put("profile", JSONObject.NULL)
        if (set.transport != "auto") o.put("reliable", set.reliable)
        return o.put("hdr", set.hdr).put("wired", false)
    }

    /** Toutes les clés (`SyncedValues::from_stream` du cœur) : la première fois qu'un ordinateur n'a rien au relais. */
    fun full(stream: JSONObject): JSONObject =
        runCatching { (Accounts.read(Native.accountSettingsFromStream(stream.toString(), null)) { it as? JSONObject } as? AccountReply.Ok)?.value }.getOrNull()
            ?: JSONObject().put("profile", stream.opt("profile") ?: JSONObject.NULL).put("codec", stream.optString("codec"))
                .put("yuv444", stream.optBoolean("yuv444")).put("mbps", stream.optInt("mbps")).put("hdr", stream.optBoolean("hdr"))

    /**
     * Les seules clés changées entre [before] et [after] (`stream` : les réglages de session d'[after]) ; nul si rien ne
     * change pour le relais. Un profil → `{profile}` ; passer en « Personnalisé » → `{profile: null, codec, yuv444,
     * mbps}` ; le format ou le débit d'un réglage déjà personnalisé → la clé changée et `profile: null` ; le HDR → `{hdr}`.
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
        return v.takeIf { it.length() > 0 }
    }

    /** Les clés connues et vérifiées d'un `{station, updated_at, values}` (`SyncedSettings::known` du cœur). */
    fun known(synced: JSONObject): JSONObject =
        runCatching { (Accounts.read(Native.accountSettingsKnown(synced.toString())) { it as? JSONObject } as? AccountReply.Ok)?.value }.getOrNull()
            ?: run {
                // Sans bibliothèque native : les clés d'Android, à peu près vérifiées.
                val raw = synced.optJSONObject("values") ?: JSONObject()
                val o = JSONObject()
                if (raw.has("profile")) {
                    if (raw.isNull("profile")) o.put("profile", JSONObject.NULL)
                    else Profile.entries.firstOrNull { it.id == raw.optString("profile") }?.let { o.put("profile", it.id) }
                }
                raw.optString("codec").takeIf { it in listOf("h264", "hevc", "av1", "pyrowave") }?.let { o.put("codec", it) }
                (raw.opt("yuv444") as? Boolean)?.let { o.put("yuv444", it) }
                (raw.opt("mbps") as? Number)?.toInt()?.takeIf { it in 1..4294 }?.let { o.put("mbps", it) }
                (raw.opt("hdr") as? Boolean)?.let { o.put("hdr", it) }
                o
            }

    /**
     * Les réglages de session [base] avec les clés reçues [values], ramenés à ce que l'appareil décode ([caps], `{hevc,
     * hevc444, av1, hdr, pyrowave}`) : `SyncedValues::apply_to` du cœur.
     */
    fun applyStream(values: JSONObject, base: JSONObject, caps: JSONObject): JSONObject =
        runCatching { (Accounts.read(Native.accountSettingsApply(values.toString(), base.toString(), caps.toString())) { it as? JSONObject } as? AccountReply.Ok)?.value }.getOrNull()
            ?: JSONObject(base.toString()).also { o ->
                // Sans bibliothèque native : un format que l'appareil ne décode pas ne sert pas.
                val c = values.optString("codec")
                val decodes = when (c) {
                    "h264" -> true
                    "hevc", "av1", "pyrowave" -> caps.optBoolean(c)
                    else -> false
                }
                if (decodes) o.put("codec", c)
                if (values.has("mbps")) o.put("mbps", values.optInt("mbps"))
                if (values.has("hdr")) o.put("hdr", values.optBoolean("hdr"))
            }

    /**
     * Les réglages d'Android après les clés reçues ([values], [known]) et leur application par le cœur ([result],
     * [applyStream]) : un profil le reprend entier (format et débit d'Android remis à « Auto », le transport gardé) ;
     * « Personnalisé » prend le format et le débit retenus par le cœur ; un format ou un débit sans profil ne touche
     * qu'un réglage déjà personnalisé ; le HDR tel quel.
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
 * défaut, gardé ici, jamais synchronisé), et pour chaque compte la dernière liste lue (`[{station, updated_at, values}]`),
 * le `updated_at` déjà appliqué de chaque ordinateur, et les changements que le relais n'a pas pris (relais muet), à
 * renvoyer au rafraîchissement suivant. Rien de secret : en clair, à côté des comptes.
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

    // MARK: - Ce qui est gardé

    private fun obj(name: String): JSONObject = runCatching { JSONObject(prefs.getString(name, null) ?: "{}") }.getOrDefault(JSONObject())
    private fun save(name: String, o: JSONObject) = prefs.edit().putString(name, o.toString()).apply()

    /** La dernière liste lue d'un compte, par Station. */
    fun list(key: String): Map<String, JSONObject> {
        val a = obj("lists").optJSONArray(key) ?: return emptyMap()
        return (0 until a.length()).mapNotNull { a.optJSONObject(it) }.associateBy { it.optString("station") }
    }

    private fun saveList(key: String, list: Map<String, JSONObject>) =
        save("lists", obj("lists").put(key, org.json.JSONArray().apply { list.values.forEach { put(it) } }))

    private fun id(key: String, station: String) = "$key\n$station"

    /** Le `updated_at` déjà appliqué (ou envoyé) pour cet ordinateur. */
    fun appliedAt(key: String, station: String): Long = obj("applied").optLong(id(key, station), 0)

    private fun setApplied(key: String, station: String, at: Long) = save("applied", obj("applied").put(id(key, station), at))

    /** Les clés que le relais n'a pas prises (relais muet), à renvoyer. */
    fun pending(key: String, station: String): JSONObject? = obj("pending").optJSONObject(id(key, station))

    private fun setPending(key: String, station: String, values: JSONObject?) =
        save("pending", obj("pending").apply { if (values == null) remove(id(key, station)) else put(id(key, station), values) })

    /** L'ordinateur n'est plus au compte : ses réglages gardés partent (les réglages locaux restent). */
    private fun forgetStation(key: String, station: String) {
        saveList(key, list(key) - station)
        save("applied", obj("applied").apply { remove(id(key, station)) })
        setPending(key, station, null)
    }

    /** Le compte est oublié : tout ce qui le concerne aussi. */
    fun forgetAccount(key: String) {
        save("lists", obj("lists").apply { remove(key) })
        for (name in listOf("applied", "pending")) save(name, obj(name).apply { keys().asSequence().toList().filter { it.startsWith("$key\n") }.forEach { remove(it) } })
    }

    // MARK: - Lire

    /**
     * Les réglages du compte : d'abord les changements que le relais n'avait pas pris, puis la liste, appliquée pour
     * chaque ordinateur dont `updated_at` dépasse celui déjà appliqué (sans rien renvoyer). Un ordinateur absent de la
     * liste perd ses réglages gardés. Relais sans réglages synchronisés (77) ou muet : rien, sans rien dire.
     */
    suspend fun sync(a: StoredAccount) {
        if (!enabled.value) return
        lock.withLock {
            for (station in obj("pending").keys().asSequence().toList().filter { it.startsWith("${a.key}\n") }.map { it.substringAfter('\n') }) {
                val values = pending(a.key, station) ?: continue
                if (!send(a, station, values)) return
            }
            when (val r = remote.fetch(a)) {
                is AccountReply.Ok -> {
                    val fresh = r.value.associateBy { it.optString("station") }.filterKeys { it.isNotEmpty() }
                    (list(a.key).keys - fresh.keys).forEach { forgetStation(a.key, it) }
                    saveList(a.key, fresh)
                    fresh.forEach { (station, synced) ->
                        val at = synced.optLong("updated_at")
                        if (at > appliedAt(a.key, station)) {
                            apply(a.key, station, SyncedKeys.known(synced))
                            setApplied(a.key, station, at)
                        }
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
        for (h in hosts.hosts.value.filter { it.account?.key == key && it.account?.station == station }) {
            val hevc = d.hevc != null
            val av1 = d.av1 != null && h.av1
            val stream = SyncedKeys.stream(h.settings, SyncedKeys.profileBase(h.settings.profile, hevc, d.hevc444, av1), hevc, av1)
            val result = SyncedKeys.applyStream(values, stream, Hdr.capabilities(h, d))
            val next = SyncedKeys.applied(h.settings, values, result)
            if (next != h.settings) hosts.applySynced(h.id) { it.copy(settings = next) }
        }
    }

    // MARK: - Envoyer

    /**
     * Un réglage synchronisé changé par la personne pour un ordinateur du compte (réglages de l'ordinateur, profil ou HDR
     * de la session) : seulement les clés changées, ou tout la première fois qu'il n'a rien au relais.
     */
    fun changed(before: Host, after: Host) {
        if (!enabled.value || before.settings == after.settings) return
        val acc = after.account ?: return
        val a = account(acc.key) ?: return
        val d = decoders()
        val hevc = d.hevc != null
        val av1 = d.av1 != null && after.av1
        val stream = SyncedKeys.stream(after.settings, SyncedKeys.profileBase(after.settings.profile, hevc, d.hevc444, av1), hevc, av1)
        val keys = SyncedKeys.changed(before.settings, after.settings, stream) ?: return
        scope.launch {
            lock.withLock {
                val values = if (list(acc.key)[acc.station] == null) SyncedKeys.full(stream) else keys
                // Ce qui attendait encore part avec, le plus récent l'emportant.
                val merged = pending(acc.key, acc.station)?.let { p -> JSONObject(p.toString()).also { m -> values.keys().forEach { k -> m.put(k, values.get(k)) } } } ?: values
                send(a, acc.station, merged)
            }
        }
    }

    /** Envoie ; faux pour arrêter (relais muet, compte fermé). Muet : gardé pour le rafraîchissement suivant. */
    private suspend fun send(a: StoredAccount, station: String, values: JSONObject): Boolean =
        when (val r = remote.put(a, station, list(a.key)[station], values)) {
            is AccountReply.Ok -> {
                saveList(a.key, list(a.key) + (station to r.value))
                setApplied(a.key, station, r.value.optLong("updated_at"))
                setPending(a.key, station, null)
                true
            }
            is AccountReply.Refused -> refused(a, station to values, r)
        }

    private fun refused(a: StoredAccount, sent: Pair<String, JSONObject>?, r: AccountReply.Refused): Boolean {
        when {
            r.signedOut -> {
                signedOut(a.key, r.message)
                return false
            }
            r.unknownComputer -> sent?.let { forgetStation(a.key, it.first) }
            // Relais d'avant les réglages synchronisés : les réglages locaux restent, rien à renvoyer.
            r.code == fr.leslignesclaires.kybernow.core.AccountCode.NO_ACCOUNTS -> sent?.let { setPending(a.key, it.first, null) }
            r.code == fr.leslignesclaires.kybernow.core.AccountCode.UNREACHABLE -> {
                sent?.let { setPending(a.key, it.first, it.second) }
                return false
            }
            // Trop grand, valeurs refusées : rien à renvoyer.
            else -> sent?.let { setPending(a.key, it.first, null) }
        }
        Log.i("KyberNow", "réglages synchronisés : ${a.relay} refuse (${r.kind})")
        return true
    }

    private companion object {
        const val PREFS = "settings_sync"
    }
}
