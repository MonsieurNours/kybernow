package fr.leslignesclaires.kybernow.core

import fr.leslignesclaires.kybernow.model.Host
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/*
 * Les comptes KyberNow vus de l'app (docs/comptes.md, lot 3 ; textes : docs/link-ui.md, « Comptes KyberNow »). Le cœur
 * (`account`, pont JNI `apps/android/jni/src/account.rs`) parle au relais : connexion par la fenêtre du système (onglet
 * personnalisé) ou par un code (Android TV), ordinateurs du compte, jeton d'accès de chaque appel. Ce fichier range ses
 * réponses JSON pour l'interface et prépare ce que l'app lui rend (les ordinateurs appairés pour « un ordinateur une
 * fois », les paramètres de la session d'un ordinateur du compte). L'app ne crée ni ne supprime de compte : elle renvoie
 * à la page du compte, sur le relais.
 */

private fun JSONObject.text(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

/** Les refus du cœur (`AccountError::code`). */
object AccountCode {
    /** La session de cet appareil est fermée (page du compte, nouveau mot de passe) : oublier le compte et le dire. */
    const val SIGNED_OUT = 70
    /** L'ordinateur n'est plus sur le compte : il quitte l'accueil. */
    const val UNKNOWN_COMPUTER = 71
    /** Code de téléviseur refusé sur le téléphone (« Ce n'est pas moi ») : « Nouveau code ». */
    const val DENIED = 72
    /** Code de téléviseur échu : un nouveau code, de lui-même. */
    const val EXPIRED = 73
    /** Retour de la fenêtre du système refusé. */
    const val BAD_CODE = 74
    /** « Annuler » dans la page du relais : rien à montrer. */
    const val CANCELLED = 75
    const val TOO_MANY = 76
    const val NO_ACCOUNTS = 77
    const val BAD_RELAY = 78
    const val UNREACHABLE = 79
    const val OTHER = 80
}

/** Réponse d'un appel des comptes : la valeur, ou le refus du cœur ([code], [kind], [message] dans la langue de l'app). */
sealed class AccountReply<out T> {
    data class Ok<T>(val value: T) : AccountReply<T>()

    data class Refused(val code: Int, val kind: String, val message: String) : AccountReply<Nothing>() {
        val signedOut get() = code == AccountCode.SIGNED_OUT
        val unknownComputer get() = code == AccountCode.UNKNOWN_COMPUTER
        val cancelled get() = code == AccountCode.CANCELLED
    }
}

/**
 * Un compte connecté sur cet appareil, tel que le cœur le rend (`Account` : `relay`, `token`, `account {id, name,
 * email}`, `device {id, name}`) : [json] se garde **entier** (le jeton compris, la session d'appareil), chiffré, et
 * repart tel quel au cœur. Jamais dans un journal ([toString] ne le montre pas).
 */
class StoredAccount private constructor(
    val json: String,
    /** Le relais, sous sa forme normale (`share.kybernow.app`, `relais.exemple.org:9443`). */
    val relay: String,
    val id: String,
    val name: String,
    val email: String,
    val deviceName: String,
) {
    /** Ce qui distingue ce compte des autres sur l'appareil, comme le cœur (`Account::key`) : le relais et l'identifiant. */
    val key: String get() = "$relay|$id"

    /** Le nom du compte, sinon son e-mail (« Connecté à {compte}. », « Se déconnecter de {compte} ? »). */
    val displayName: String get() = name.ifBlank { email }.ifBlank { id }

    override fun toString() = "StoredAccount($key)"
    override fun equals(other: Any?) = other is StoredAccount && other.json == json
    override fun hashCode() = json.hashCode()

    companion object {
        /** Nul si le JSON ne se lit pas ou n'a ni relais, ni compte, ni jeton. */
        fun parse(json: String?): StoredAccount? = runCatching {
            val o = JSONObject(json ?: return null)
            val relay = o.text("relay") ?: return null
            o.text("token") ?: return null
            val a = o.optJSONObject("account") ?: return null
            val id = a.text("id") ?: return null
            StoredAccount(o.toString(), relay, id, a.optString("name"), a.optString("email"), o.optJSONObject("device")?.optString("name") ?: "")
        }.getOrNull()
    }
}

/** Ce que les réglages « Comptes » montrent d'un compte (`AccountSummary` du cœur), sans le jeton. */
data class AccountSummary(
    val key: String,
    val name: String,
    val email: String,
    /** Le relais à montrer : `share.kybernow.app`, `relais.exemple.org:9443`. */
    val relayLabel: String,
    /** La page du compte (« Gérer le compte ») : `https://{relais}/compte`. */
    val console: String,
) {
    val displayName: String get() = name.ifBlank { email }
}

/**
 * Un ordinateur d'un compte (`AccountComputer` du cœur) : le compte qui le donne ([account], sa clé), le relais, la
 * Station ([station]), le nom, les droits, l'empreinte d'Admin vérifiée ([adminFp], gardée d'une liste à l'autre). [json]
 * repart tel quel au cœur (pastille, route, fusion).
 */
data class AccountComputer(
    val account: String,
    val relay: String,
    val station: String,
    val name: String,
    val rights: List<String> = emptyList(),
    val online: Boolean = false,
    val mine: Boolean = true,
    val adminFp: String? = null,
    val json: String,
) {
    fun can(right: String) = right in rights

    companion object {
        fun parse(o: JSONObject?): AccountComputer? {
            if (o == null) return null
            val station = o.text("station") ?: return null
            val rights = o.optJSONArray("rights")?.let { a -> (0 until a.length()).mapNotNull { a.optString(it).takeIf { r -> r.isNotEmpty() } } } ?: emptyList()
            return AccountComputer(
                account = o.optString("account"), relay = o.optString("relay"), station = station, name = o.optString("name"),
                rights = rights, online = o.optBoolean("online"), mine = o.optBoolean("mine", true), adminFp = o.text("admin_fp"), json = o.toString(),
            )
        }

        fun parseList(a: JSONArray?): List<AccountComputer> = if (a == null) emptyList() else (0 until a.length()).mapNotNull { parse(a.optJSONObject(it)) }

        fun toJson(list: List<AccountComputer>): String = JSONArray().apply { list.forEach { put(JSONObject(it.json)) } }.toString()
    }
}

/**
 * Les ordinateurs à l'accueil, un ordinateur une fois (`Merged` du cœur) : [linked], un ordinateur appairé (son
 * identifiant sur l'appareil) qui est aussi sur un compte (même empreinte d'Admin, ou même Station chez le relais) ;
 * [added], ceux des comptes à ajouter.
 */
data class AccountMerge(val linked: Map<String, AccountComputer> = emptyMap(), val added: List<AccountComputer> = emptyList())

/** Une connexion par la fenêtre du système (`SignIn` du cœur) : la page à ouvrir ([url]) ; [json] est gardé jusqu'au retour. */
data class SignInRequest(val relay: String, val url: String, val json: String) {
    override fun toString() = "SignInRequest($relay)"
}

/** Un code de téléviseur (`TvCode` du cœur) : à montrer avec le QR code de [urlComplete] ; [json] repart au cœur. */
data class TvCode(
    val relay: String,
    /** `KQ7M-4XPA`. */
    val code: String,
    /** La page où taper le code, telle qu'on l'écrit : `share.kybernow.app/tv`. */
    val urlLabel: String,
    val urlComplete: String,
    /** Secondes entre deux demandes (5). */
    val interval: Long,
    val json: String,
) {
    override fun toString() = "TvCode($relay, $code)"

    companion object {
        fun parse(o: JSONObject?): TvCode? {
            if (o == null) return null
            val code = o.text("code") ?: return null
            return TvCode(
                relay = o.optString("relay"), code = code, urlLabel = o.text("url_label") ?: o.optString("url"),
                urlComplete = o.text("url_complete") ?: o.optString("url"), interval = o.optLong("interval", 5).coerceAtLeast(1), json = o.toString(),
            )
        }
    }
}

/** Une demande du code de téléviseur : pas encore approuvé, ou le compte. */
sealed class TvPoll {
    data object Pending : TvPoll()
    data class Approved(val account: StoredAccount) : TvPoll()
}

object Accounts {
    /** Le relais public, proposé d'abord. */
    const val DEFAULT_RELAY = "share.kybernow.app"

    /** Une réponse du cœur. Illisible : un refus sans texte (l'écran en met un), jamais une chute de l'app. */
    internal fun <T> read(text: String?, value: (Any?) -> T?): AccountReply<T> {
        val o = runCatching { JSONObject(text ?: "") }.getOrNull() ?: return AccountReply.Refused(1, "other", "")
        val code = o.optInt("code", 1)
        if (code == 0) return value(if (o.isNull("value")) null else o.opt("value"))?.let { AccountReply.Ok(it) } ?: AccountReply.Refused(1, "other", "")
        return AccountReply.Refused(code, o.text("kind") ?: "other", o.optString("error"))
    }

    internal fun readAccount(text: String?) = read(text) { StoredAccount.parse((it as? JSONObject)?.toString()) }

    internal fun readTvPoll(text: String?): AccountReply<TvPoll> = read(text) { v ->
        val o = v as? JSONObject ?: return@read null
        when (o.optString("status")) {
            "pending" -> TvPoll.Pending
            "approved" -> StoredAccount.parse(o.optJSONObject("account")?.toString())?.let { TvPoll.Approved(it) }
            else -> null
        }
    }

    internal fun readComputers(text: String?) = read(text) { AccountComputer.parseList(it as? JSONArray) }

    internal fun readSummary(text: String?) = read(text) { v ->
        val o = v as? JSONObject ?: return@read null
        val a = o.optJSONObject("account") ?: JSONObject()
        AccountSummary(o.optString("key"), a.optString("name"), a.optString("email"), o.optString("relay_label"), o.optString("console"))
    }

    /** `{linked: [[id, ordinateur]], added: [ordinateur]}` ; nul s'il ne se lit pas. */
    internal fun readMerge(text: String?): AccountMerge? = (read(text) { v ->
        val o = v as? JSONObject ?: return@read null
        val linked = mutableMapOf<String, AccountComputer>()
        o.optJSONArray("linked")?.let { a ->
            for (i in 0 until a.length()) {
                val pair = a.optJSONArray(i) ?: continue
                val id = pair.optString(0).takeIf { it.isNotEmpty() } ?: continue
                AccountComputer.parse(pair.optJSONObject(1))?.let { linked[id] = it }
            }
        }
        AccountMerge(linked, AccountComputer.parseList(o.optJSONArray("added")))
    } as? AccountReply.Ok<AccountMerge>)?.value

    // MARK: - Appels au cœur

    /** « Un autre relais… » : la forme normale de l'adresse tapée, ou le refus (78 : « Adresse du relais illisible. »). */
    fun normalizeRelay(relay: String): AccountReply<String> = read(Native.accountNormalizeRelay(relay)) { it as? String }

    /** La page de connexion du relais pour cet appareil (rien ne part au relais : seulement l'adresse). */
    fun signIn(relay: String, deviceName: String): AccountReply<SignInRequest> = read(Native.accountSignIn(relay, deviceName)) { v ->
        val o = v as? JSONObject ?: return@read null
        SignInRequest(o.optString("relay"), o.text("url") ?: return@read null, o.toString())
    }

    /** Le retour `kybernow://compte?…` échangé contre le compte (75 : « Annuler » dans la page). Hors du fil principal. */
    suspend fun finishSignIn(request: String, callback: String): AccountReply<StoredAccount> =
        withContext(Dispatchers.IO) { readAccount(Native.accountFinishSignIn(request, callback)) }

    /** Un retour de la connexion (`kybernow://compte…`) ; faux sans bibliothèque native. */
    fun isCallback(url: String): Boolean = runCatching { Native.accountIsCallback(url) }.getOrDefault(false)

    suspend fun tvCode(relay: String, deviceName: String): AccountReply<TvCode> =
        withContext(Dispatchers.IO) { read(Native.accountTvCode(relay, deviceName)) { TvCode.parse(it as? JSONObject) } }

    suspend fun tvPoll(code: TvCode): AccountReply<TvPoll> = withContext(Dispatchers.IO) { readTvPoll(Native.accountTvPoll(code.json)) }

    fun tvMinutesLeft(code: TvCode): Int = runCatching { Native.accountTvMinutesLeft(code.json) }.getOrDefault(0)

    /** « Se déconnecter » chez le relais ; l'app oublie le compte quelle que soit la réponse. */
    suspend fun signOut(account: StoredAccount): AccountReply<Unit> =
        withContext(Dispatchers.IO) { read(Native.accountSignOut(account.json)) { } }

    /** Ce que les réglages montrent ; sans bibliothèque native, ce que l'app lit elle-même du compte. */
    fun summary(account: StoredAccount): AccountSummary =
        (runCatching { readSummary(Native.accountSummary(account.json)) }.getOrNull() as? AccountReply.Ok<AccountSummary>)?.value
            ?: AccountSummary(account.key, account.name, account.email, account.relay.substringBefore('#'), "https://${account.relay.substringBefore('#')}/compte")

    suspend fun computers(account: StoredAccount): AccountReply<List<AccountComputer>> =
        withContext(Dispatchers.IO) { readComputers(Native.accountComputers(account.json)) }

    /** La liste qui arrive, avec les empreintes d'Admin déjà vues (le relais ne les donne que pour une Station connectée). */
    fun rememberFingerprints(fresh: List<AccountComputer>, known: List<AccountComputer>): List<AccountComputer> =
        (runCatching { readComputers(Native.accountRememberFingerprints(AccountComputer.toJson(fresh), AccountComputer.toJson(known))) }.getOrNull() as? AccountReply.Ok<List<AccountComputer>>)?.value
            ?: fresh

    /** Un ordinateur une fois (`account::merge` du cœur). */
    fun merge(paired: List<Host>, computers: List<AccountComputer>): AccountMerge? =
        runCatching { readMerge(Native.accountMerge(pairedJson(paired), AccountComputer.toJson(computers))) }.getOrNull()

    /** La pastille d'origine d'un ordinateur du compte, ou « Appairé » ([computer] nul). */
    fun origin(computer: AccountComputer?, severalRelays: Boolean): String =
        runCatching { Native.accountOrigin(computer?.json ?: "{}", severalRelays, computer == null) }.getOrDefault("")

    /** La route par le relais d'un ordinateur du compte (`{server, station, admin_fp}`), nulle sans empreinte d'Admin. */
    fun route(computer: AccountComputer): String? = runCatching { Native.accountRoute(computer.json) }.getOrNull()

    /** Le droit « Regarder » (`view`, `AccountComputer::can_watch` du cœur) ; sans bibliothèque native, d'après les droits lus. */
    fun canWatch(computer: AccountComputer): Boolean = runCatching { Native.accountCanWatch(computer.json) }.getOrElse { computer.can("view") }

    /**
     * « Regarder » sans « Ouvrir le bureau » (`watch_only`) : la carte propose « Regarder » à la place de « Se connecter » ;
     * avec les deux droits, « Regarder » est dans le menu de l'ordinateur.
     */
    fun watchOnly(computer: AccountComputer): Boolean =
        runCatching { Native.accountWatchOnly(computer.json) }.getOrElse { computer.can("view") && !computer.can("session") }

    // MARK: - Ce que l'app rend au cœur

    /**
     * Les ordinateurs appairés pour [merge] : `[{id, fingerprint, station}]`, `station` l'identifiant de la Station chez
     * le relais quand l'app le connaît (`relay` de l'info d'Admin). Les ordinateurs venus d'un compte n'en sont pas.
     */
    fun pairedJson(hosts: List<Host>): String = JSONArray().apply {
        hosts.filterNot { it.fromAccount }.forEach { h ->
            put(JSONObject().put("id", h.id).put("fingerprint", h.fingerprint ?: JSONObject.NULL).put("station", h.relay?.station ?: JSONObject.NULL))
        }
    }.toString()

    /** Le relais tel qu'on l'écrit (« Relais : {relais} ») : la forme normale sans l'empreinte épinglée. */
    fun relayLabel(relay: String): String = relay.substringBefore('#').ifBlank { DEFAULT_RELAY }

    /** Avec des comptes sur plusieurs relais, la pastille dit lequel (« Compte · {relais} »). */
    fun severalRelays(accounts: List<StoredAccount>): Boolean = accounts.map { it.relay.substringBefore('#').lowercase() }.distinct().size > 1

    /**
     * La session d'un ordinateur du compte (`SessionParams::from_json` du cœur) : ni identifiant ni mot de passe, Admin
     * épinglé sur son empreinte, `account` `{account: <le compte entier>, station}`. Vu sur le réseau ([lan], son adresse
     * et son port d'Admin) : `route` `auto`, le réseau d'abord et le relais ([route]) sinon ; pas vu : par le relais
     * seulement. [relayAllowed] faux (`KYBERNOW_RELAY=off`) : le réseau seulement. Faux s'il n'y a aucun chemin (ni vu
     * sur le réseau, ni route par le relais) : la session ne peut pas s'ouvrir.
     */
    fun putSession(params: JSONObject, account: StoredAccount, station: String, adminFp: String?, lan: Pair<String, Int>?, route: String?, relayAllowed: Boolean = true): Boolean {
        val relay = route?.takeIf { relayAllowed }
        if (lan == null && relay == null) return false
        params.put("console_host", lan?.first ?: "").put("console_port", lan?.second ?: 0)
            .put("user", "").put("password", "")
            .put("console_fingerprint", adminFp ?: JSONObject.NULL)
            .put("account", JSONObject().put("account", JSONObject(account.json)).put("station", station))
        params.remove("route")
        params.remove("relay")
        when {
            lan != null && relay != null -> params.put("route", "auto").put("relay", JSONObject(relay))
            lan != null -> params.put("route", "direct")
            else -> params.put("route", "relay").put("relay", JSONObject(relay!!))
        }
        return true
    }

    // MARK: - Réglages synchronisés (docs/comptes.md, « Réglages synchronisés et déplacer un ordinateur »)

    /** Les réglages de tous les ordinateurs du compte (`[{station, updated_at, values}]`). Hors du fil principal. */
    suspend fun settingsFetch(account: StoredAccount): AccountReply<List<JSONObject>> =
        withContext(Dispatchers.IO) { read(Native.accountSettingsFetch(account.json)) { v -> (v as? JSONArray)?.let { a -> (0 until a.length()).mapNotNull { a.optJSONObject(it) } } } }

    /** Envoie les clés changées d'un ordinateur, posées sur les dernières connues ([last]) : les réglages à garder. Hors du fil principal. */
    suspend fun settingsPut(account: StoredAccount, station: String, last: JSONObject?, values: JSONObject): AccountReply<JSONObject> =
        withContext(Dispatchers.IO) { read(Native.accountSettingsPut(account.json, station, last?.toString(), values.toString())) { it as? JSONObject } }

    /** « Synchroniser les réglages des ordinateurs » et son aide, du cœur ; nul sans bibliothèque native. */
    fun syncSettingsTexts(): Pair<String, String>? = runCatching {
        val a = JSONArray(Native.accountSyncSettingsTexts())
        a.getString(0) to a.getString(1)
    }.getOrNull()

    /** `access` de [Native.consoleNewAccount] : `{account: <le compte entier>, station}`. */
    fun access(account: StoredAccount, station: String): String = JSONObject().put("account", JSONObject(account.json)).put("station", station).toString()
}
