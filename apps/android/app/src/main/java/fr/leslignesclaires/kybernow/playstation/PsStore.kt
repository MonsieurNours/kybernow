package fr.leslignesclaires.kybernow.playstation

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyStore
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Un compte PlayStation Network gardé sur cet appareil (son identifiant base64 est chiffré à part) ; [id] : un UUID en
 * majuscules, le même sur tous les appareils qui ont reçu ce compte (docs/playstation.md, « Plusieurs appareils »).
 * [name] : pseudo PSN lu chez Sony, ou nom donné à la main ; nul : « Compte PlayStation ».
 */
data class PsAccount(val id: String, val name: String?) {
    fun json(): JSONObject = JSONObject().put("id", id).put("name", name ?: JSONObject.NULL)

    companion object {
        fun parse(o: JSONObject) = PsAccount(o.getString("id"), o.optString("name").takeIf { !o.isNull("name") && it.isNotEmpty() })
    }
}

/**
 * Une console associée (avec un compte au moins) : [hostId], son identifiant (le `mac` de l'association, en
 * majuscules), [model] « PS5 » ou « PS4 », [address] la dernière adresse vue (la recherche la tient à jour), [target]
 * pour associer de nouveau, [hdr] la puce de l'accueil (allumée la première fois), [settings] et [layout] les JSON du
 * cœur (nuls : par défaut), [account] le compte choisi à l'accueil.
 */
data class PsConsole(
    val id: String = UUID.randomUUID().toString().uppercase(),
    val hostId: String,
    val name: String,
    val model: String,
    val address: String,
    val target: String,
    val hdr: Boolean = true,
    val settings: String? = null,
    val layout: String? = null,
    val account: String? = null,
) {
    val ps5 get() = model == "PS5"

    fun json(): JSONObject = JSONObject()
        .put("id", id).put("host_id", hostId).put("name", name).put("model", model).put("address", address).put("target", target)
        .put("hdr", hdr).put("settings", settings ?: JSONObject.NULL).put("layout", layout ?: JSONObject.NULL).put("account", account ?: JSONObject.NULL)

    companion object {
        private fun JSONObject.str(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

        fun parse(o: JSONObject) = PsConsole(
            id = o.getString("id"), hostId = o.getString("host_id").uppercase(), name = o.optString("name"), model = o.optString("model", "PS5"),
            address = o.optString("address"), target = o.optString("target"), hdr = o.optBoolean("hdr", true),
            settings = o.str("settings"), layout = o.str("layout"), account = o.str("account"),
        )
    }
}

/**
 * Les comptes et les consoles de l'app, gardés comme les ordinateurs (préférences), les identifiants des comptes et
 * les associations (qui portent les clés) chiffrés par une clé du Keystore d'Android : `account.<uuid>` et
 * `<HOSTID>.<uuid>`, les noms du Trousseau iCloud des apps Apple. Une association par console et par compte.
 */
class PsStore(context: Context) {
    private val prefs = context.getSharedPreferences("playstation", Context.MODE_PRIVATE)
    private val secrets = context.getSharedPreferences("playstation_secrets", Context.MODE_PRIVATE)

    private val _consoles = MutableStateFlow(loadList("consoles", PsConsole::parse))
    val consoles: StateFlow<List<PsConsole>> = _consoles.asStateFlow()

    private val _accounts = MutableStateFlow(loadList("accounts", PsAccount::parse))
    /** Dans l'ordre d'ajout. */
    val accounts: StateFlow<List<PsAccount>> = _accounts.asStateFlow()

    private val _pairings = MutableStateFlow<Map<String, Set<String>>>(emptyMap())
    /** Les comptes associés à chaque console (par l'identifiant de la console de l'app). */
    val pairings: StateFlow<Map<String, Set<String>>> = _pairings.asStateFlow()

    /** Adresses saisies dans « Par adresse » où une console a répondu : cherchées en direct, en plus du réseau. */
    var addresses: List<String>
        get() = runCatching { JSONArray(prefs.getString("addresses", "[]")).let { a -> List(a.length()) { a.getString(it) } } }.getOrDefault(emptyList())
        set(v) = prefs.edit().putString("addresses", JSONArray(v.distinct()).toString()).apply()

    init {
        refreshPairings()
    }

    private fun <T> loadList(key: String, parse: (JSONObject) -> T): List<T> = runCatching {
        val a = JSONArray(prefs.getString(key, "[]"))
        List(a.length()) { parse(a.getJSONObject(it)) }
    }.getOrDefault(emptyList())

    private fun saveConsoles(list: List<PsConsole>) {
        _consoles.value = list
        prefs.edit().putString("consoles", JSONArray().apply { list.forEach { put(it.json()) } }.toString()).apply()
    }

    private fun saveAccounts(list: List<PsAccount>) {
        _accounts.value = list
        prefs.edit().putString("accounts", JSONArray().apply { list.forEach { put(it.json()) } }.toString()).apply()
    }

    private fun refreshPairings() {
        _pairings.value = _consoles.value.associate { c -> c.id to _accounts.value.filter { secret(pairKey(c.hostId, it.id)) != null }.map { it.id }.toSet() }
            .filterValues { it.isNotEmpty() }
    }

    // MARK: - Les Stations qui voient chaque console (hors de chez soi)

    /** Les ordinateurs de l'app (leurs identifiants) qui voient la console [hostId] sur leur réseau. */
    fun stations(hostId: String): List<String> = runCatching {
        val o = JSONObject(prefs.getString("stations", null) ?: "{}")
        o.optJSONArray(hostId.uppercase())?.let { a -> List(a.length()) { a.getString(it) } }
    }.getOrNull().orEmpty()

    /** La Station [station] voit (ou ne voit plus) ces consoles : retenu par console. */
    fun setSeenBy(station: String, hostIds: Set<String>) {
        val o = runCatching { JSONObject(prefs.getString("stations", null) ?: "{}") }.getOrDefault(JSONObject())
        val all = (o.keys().asSequence().toList() + hostIds).distinct()
        var changed = false
        for (h in all) {
            val list = o.optJSONArray(h)?.let { a -> List(a.length()) { a.getString(it) } }.orEmpty()
            val now = if (h in hostIds) (listOf(station) + list).distinct() else list - station
            if (now != list) {
                changed = true
                if (now.isEmpty()) o.remove(h) else o.put(h, JSONArray(now))
            }
        }
        if (changed) prefs.edit().putString("stations", o.toString()).apply()
    }

    fun addStation(hostId: String, station: String) {
        val o = runCatching { JSONObject(prefs.getString("stations", null) ?: "{}") }.getOrDefault(JSONObject())
        val list = o.optJSONArray(hostId)?.let { a -> List(a.length()) { a.getString(it) } }.orEmpty()
        if (station !in list) prefs.edit().putString("stations", o.put(hostId, JSONArray(list + station)).toString()).apply()
    }

    // MARK: - Comptes

    fun account(id: String?): PsAccount? = _accounts.value.firstOrNull { it.id == id }

    /** L'identifiant PSN (base64) d'un compte. */
    fun psnId(account: String): String? = secret("account.$account")

    /**
     * Garde un compte : même identifiant qu'un compte gardé, son pseudo est mis à jour (rien en double, [Kept.existed]) ;
     * sinon un nouveau compte ([preferredId] s'il est libre : celui d'un envoi).
     */
    fun keep(psnId: String, name: String?, preferredId: String? = null): Kept {
        _accounts.value.firstOrNull { psnId(it.id) == psnId }?.let { a ->
            if (name != null && name != a.name) saveAccounts(_accounts.value.map { if (it.id == a.id) it.copy(name = name) else it })
            return Kept(account(a.id)!!, true)
        }
        val id = preferredId?.uppercase()?.takeIf { p -> _accounts.value.none { it.id == p } } ?: UUID.randomUUID().toString().uppercase()
        setSecret("account.$id", psnId)
        val a = PsAccount(id, name)
        saveAccounts(_accounts.value + a)
        return Kept(a, false)
    }

    data class Kept(val account: PsAccount, val existed: Boolean)

    fun rename(account: String, name: String) {
        val n = name.trim().takeIf { it.isNotEmpty() } ?: return
        saveAccounts(_accounts.value.map { if (it.id == account) it.copy(name = n) else it })
    }

    /** Les consoles associées avec ce compte. */
    fun consolesOf(account: String): List<PsConsole> = _consoles.value.filter { account in _pairings.value[it.id].orEmpty() }

    /** Retire le compte et ses associations ; une console sans plus aucune association part. */
    fun removeAccount(account: String) {
        for (c in _consoles.value) removeSecret(pairKey(c.hostId, account))
        removeSecret("account.$account")
        saveAccounts(_accounts.value.filter { it.id != account })
        refreshPairings()
        saveConsoles(_consoles.value.filter { _pairings.value[it.id].orEmpty().isNotEmpty() }.map { if (it.account == account) it.copy(account = null) else it })
    }

    // MARK: - Consoles et associations

    fun console(id: String?): PsConsole? = _consoles.value.firstOrNull { it.id == id }

    fun consoleByHost(hostId: String): PsConsole? = _consoles.value.firstOrNull { it.hostId.equals(hostId, ignoreCase = true) }

    fun pairedAccounts(c: PsConsole): List<PsAccount> = _accounts.value.filter { it.id in _pairings.value[c.id].orEmpty() }

    fun unpairedAccounts(hostId: String): List<PsAccount> {
        val c = consoleByHost(hostId) ?: return _accounts.value
        return _accounts.value.filter { it.id !in _pairings.value[c.id].orEmpty() }
    }

    /** Le compte avec lequel la console ouvre la partie : celui choisi, sinon le premier associé. */
    fun chosenAccount(c: PsConsole): PsAccount? = pairedAccounts(c).let { list -> list.firstOrNull { it.id == c.account } ?: list.firstOrNull() }

    fun registered(c: PsConsole, account: String? = chosenAccount(c)?.id): String? = account?.let { secret(pairKey(c.hostId, it)) }

    /** Une association réussie (ou reçue) : la console est créée ou mise à jour, l'association gardée. */
    fun paired(hostId: String, name: String, model: String, address: String, target: String, registered: String, account: String): PsConsole {
        val h = hostId.uppercase()
        setSecret(pairKey(h, account), registered)
        val existing = consoleByHost(h)
        val c = existing?.copy(name = name.ifBlank { existing.name }, model = model.ifBlank { existing.model }, address = address.ifBlank { existing.address }, target = target.ifBlank { existing.target })
            ?: PsConsole(hostId = h, name = name, model = model, address = address, target = target, account = account)
        saveConsoles(if (existing == null) _consoles.value + c else _consoles.value.map { if (it.id == c.id) c else it })
        refreshPairings()
        return c
    }

    /** La console ne reconnaît plus ce compte : l'association est effacée (la console part avec la dernière). */
    fun unpair(c: PsConsole, account: String) {
        removeSecret(pairKey(c.hostId, account))
        refreshPairings()
        if (_pairings.value[c.id].isNullOrEmpty()) saveConsoles(_consoles.value.filter { it.id != c.id })
    }

    fun remove(c: PsConsole) {
        for (a in _accounts.value) removeSecret(pairKey(c.hostId, a.id))
        saveConsoles(_consoles.value.filter { it.id != c.id })
        refreshPairings()
    }

    fun update(id: String, f: (PsConsole) -> PsConsole) = saveConsoles(_consoles.value.map { if (it.id == id) f(it) else it })

    private fun pairKey(hostId: String, account: String) = "${hostId.uppercase()}.$account"

    // MARK: - Secrets

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .build(),
        )
        return gen.generateKey()
    }

    private fun setSecret(name: String, value: String) {
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        val data = c.iv + c.doFinal(value.toByteArray())
        secrets.edit().putString(name, Base64.encodeToString(data, Base64.NO_WRAP)).apply()
    }

    private fun secret(name: String): String? = runCatching {
        val data = Base64.decode(secrets.getString(name, null) ?: return null, Base64.NO_WRAP)
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, data, 0, 12)) }
        String(c.doFinal(data, 12, data.size - 12))
    }.getOrNull()

    private fun removeSecret(name: String) = secrets.edit().remove(name).apply()

    private companion object {
        const val ALIAS = "kybernow-playstation"
    }
}
