package fr.leslignesclaires.kybernow.model

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.AccountComputer
import fr.leslignesclaires.kybernow.core.AccountMerge
import fr.leslignesclaires.kybernow.core.AccountReply
import fr.leslignesclaires.kybernow.core.Accounts
import fr.leslignesclaires.kybernow.core.ConsoleClient
import fr.leslignesclaires.kybernow.core.Relays
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareTarget
import fr.leslignesclaires.kybernow.core.WatchReply
import fr.leslignesclaires.kybernow.core.StoredAccount
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONArray
import org.json.JSONObject
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Les comptes KyberNow connectés sur cet appareil (docs/comptes.md, lot 3), plusieurs relais possibles. Chaque compte
 * se garde **entier** (le JSON du cœur, la session d'appareil comprise), chiffré par une clé de l'Android Keystore qui ne
 * quitte jamais l'appareil (comme les mots de passe des ordinateurs, [HostStore]) ; ni la sauvegarde d'Android
 * (`allowBackup` faux) ni le transfert d'un appareil à l'autre (`data_extraction_rules.xml`) ne l'emportent. La dernière
 * liste des ordinateurs de chaque compte (pas secrète : noms, Stations, empreintes d'Admin) se garde en clair, pour
 * l'accueil hors ligne. La connexion en cours par la fenêtre du système ([pendingSignIn]) survit à la recréation de
 * l'activité et du processus, le temps que le relais revienne à `kybernow://compte`.
 */
class AccountStore(context: Context) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    private val _accounts = MutableStateFlow(loadAccounts())
    val accounts: StateFlow<List<StoredAccount>> = _accounts.asStateFlow()
    private val _computers = MutableStateFlow(loadComputers())
    /** La dernière liste des ordinateurs de chaque compte, par clé de compte ([StoredAccount.key]). */
    val computers: StateFlow<Map<String, List<AccountComputer>>> = _computers.asStateFlow()

    /** La connexion par la fenêtre du système en cours (`SignIn` du cœur, en JSON) : jusqu'au retour `kybernow://compte`. */
    var pendingSignIn: String?
        get() = prefs.getString("sign_in", null)
        set(v) = prefs.edit().apply { if (v == null) remove("sign_in") else putString("sign_in", v) }.apply()

    fun account(key: String): StoredAccount? = _accounts.value.firstOrNull { it.key == key }

    /** Ajoute le compte, ou remplace celui qui a la même clé (une reconnexion : nouvelle session d'appareil). */
    fun put(account: StoredAccount) {
        saveAccounts(_accounts.value.filterNot { it.key == account.key } + account)
    }

    /** Oublie le compte et sa liste d'ordinateurs. */
    fun remove(key: String) {
        saveAccounts(_accounts.value.filterNot { it.key == key })
        saveComputers(_computers.value - key)
    }

    fun setComputers(key: String, list: List<AccountComputer>) {
        if (account(key) == null) return
        saveComputers(_computers.value + (key to list))
    }

    // MARK: - Fichiers

    private fun loadAccounts(): List<StoredAccount> {
        val sealed = prefs.getString("accounts", null) ?: return emptyList()
        val text = open(sealed)
        if (text == null) {
            // Clé du Keystore perdue (appareil réinitialisé, données transférées) : les comptes sont à reconnecter.
            Log.w("KyberNow", "comptes : illisibles, oubliés")
            prefs.edit().remove("accounts").remove("computers").apply()
            return emptyList()
        }
        return runCatching { JSONArray(text).let { a -> (0 until a.length()).mapNotNull { StoredAccount.parse(a.optJSONObject(it)?.toString()) } } }.getOrDefault(emptyList())
    }

    private fun saveAccounts(list: List<StoredAccount>) {
        val a = JSONArray().apply { list.forEach { put(JSONObject(it.json)) } }
        prefs.edit().apply { if (list.isEmpty()) remove("accounts") else putString("accounts", seal(a.toString())) }.apply()
        _accounts.value = list
    }

    private fun loadComputers(): Map<String, List<AccountComputer>> = runCatching {
        val o = JSONObject(prefs.getString("computers", null) ?: return emptyMap())
        o.keys().asSequence().associateWith { AccountComputer.parseList(o.optJSONArray(it)) }
    }.getOrDefault(emptyMap())

    private fun saveComputers(map: Map<String, List<AccountComputer>>) {
        val o = JSONObject()
        map.forEach { (k, v) -> o.put(k, JSONArray(AccountComputer.toJson(v))) }
        prefs.edit().putString("computers", o.toString()).apply()
        _computers.value = map
    }

    // MARK: - Chiffrement (Android Keystore, AES-GCM)

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

    private fun seal(text: String): String {
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        return Base64.encodeToString(c.iv + c.doFinal(text.toByteArray()), Base64.NO_WRAP)
    }

    private fun open(sealed: String): String? = runCatching {
        val data = Base64.decode(sealed, Base64.NO_WRAP)
        val c = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, data, 0, 12)) }
        String(c.doFinal(data, 12, data.size - 12))
    }.getOrNull()

    private companion object {
        /** `accounts.xml` : exclu du transfert d'appareil (`res/xml/data_extraction_rules.xml`). */
        const val PREFS = "accounts"
        const val ALIAS = "kybernow-accounts"
    }
}

/**
 * Ce que l'app fait des comptes (docs/link-ui.md, « Comptes KyberNow ») : se connecter (onglet personnalisé, ou code au
 * téléviseur), rafraîchir les ordinateurs des comptes (au lancement, au retour au premier plan, après une connexion),
 * les mettre à l'accueil un ordinateur une fois, se déconnecter. Les avis brefs (« Connecté à {compte}. », « Ce compte
 * a été déconnecté de cet appareil : reconnectez-vous. ») partent par [notices], montrés par l'activité.
 */
class AccountManager(private val app: KyberNowApp) {
    private val store = AccountStore(app)
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val lock = Mutex()

    val accounts: StateFlow<List<StoredAccount>> = store.accounts
    val computers: StateFlow<Map<String, List<AccountComputer>>> = store.computers

    private val _signingIn = MutableStateFlow(false)
    /** Le retour de la fenêtre du système s'échange contre le compte : « Connexion au compte… ». */
    val signingIn: StateFlow<Boolean> = _signingIn.asStateFlow()

    // Gardés jusqu'à ce que l'activité les montre (un refus vu au lancement, avant qu'elle écoute, n'est pas perdu).
    private val _notices = Channel<String>(capacity = 8)
    val notices: Flow<String> = _notices.receiveAsFlow()

    private fun notice(text: String) {
        if (text.isNotBlank()) _notices.trySend(text)
    }

    fun account(key: String): StoredAccount? = store.account(key)

    // MARK: - Ordinateurs des comptes

    /**
     * Redemande les ordinateurs de chaque compte, avec les empreintes d'Admin déjà vues, puis met l'accueil à jour. Code
     * 70 : la session de l'appareil est fermée, le compte est oublié et l'app le dit ; un relais muet garde la liste d'avant.
     */
    fun refresh(): Job = scope.launch {
        lock.withLock {
            for (a in accounts.value) {
                when (val r = Accounts.computers(a)) {
                    is AccountReply.Ok -> store.setComputers(a.key, Accounts.rememberFingerprints(r.value, store.computers.value[a.key] ?: emptyList()))
                    is AccountReply.Refused -> if (r.signedOut) signedOut(a.key, r.message) else Log.i("KyberNow", "comptes : liste de ${a.relay} pas à jour (${r.kind})")
                }
            }
        }
        syncHosts()
    }

    /**
     * L'accueil, un ordinateur une fois ([AccountHosts.sync]) : à chaque liste reçue et quand les ordinateurs de l'appareil
     * changent (l'accueil le rappelle à chaque sondage). Sans bibliothèque native, rien ne change.
     */
    fun syncHosts() {
        val all = accounts.value.flatMap { computers.value[it.key] ?: emptyList() }
        val merge = if (all.isEmpty()) AccountMerge() else Accounts.merge(app.hosts.hosts.value, all) ?: return
        app.hosts.syncAccounts(merge)
    }

    /** Code 70 : la session de cet appareil est fermée (page du compte, nouveau mot de passe) ; ses ordinateurs partent. */
    fun signedOut(key: String, message: String) {
        if (store.account(key) == null) return
        forget(key)
        notice(message.ifBlank { app.getString(R.string.account_signed_out) })
    }

    /** Code 71 : « Cet ordinateur n'est plus sur votre compte. » ; il quitte l'accueil. */
    fun computerGone(h: Host, message: String) {
        val acc = h.account ?: return
        store.setComputers(acc.key, (computers.value[acc.key] ?: emptyList()).filterNot { it.station == acc.station })
        syncHosts()
        notice(message.ifBlank { app.getString(R.string.account_unknown_computer) })
    }

    /** L'ordinateur du compte d'une carte de l'accueil ; nul pour un ordinateur seulement appairé. */
    fun computerOf(h: Host): AccountComputer? {
        val acc = h.account ?: return null
        return computers.value[acc.key]?.firstOrNull { it.station == acc.station }
    }

    /**
     * Les pastilles d'origine, par ordinateur de l'accueil, seulement quand un compte est connecté : celle du compte
     * (« Compte », « Compte · {relais} » avec des comptes sur plusieurs relais, l'organisation, « Ouvert par {prénom} »),
     * « Appairé » pour un ordinateur appairé qui n'est sur aucun compte.
     */
    fun origins(hosts: List<Host>): Map<String, String> {
        if (accounts.value.isEmpty()) return emptyMap()
        val several = Accounts.severalRelays(accounts.value)
        return hosts.associate { h -> h.id to if (h.account != null) computerOf(h)?.let { Accounts.origin(it, several) } ?: "" else Accounts.origin(null, several) }
            .filterValues { it.isNotBlank() }
    }

    /** L'ordinateur est vu sur le réseau (découverte : même empreinte d'Admin) : son adresse et son port d'Admin. */
    fun lan(h: Host): Announced? {
        val fp = h.fingerprint?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        return app.discovery.found.value.firstOrNull { it.fingerprint?.trim().equals(fp, ignoreCase = true) }
    }

    /** La route par le relais d'un ordinateur du compte (`accountRoute` du cœur), sinon celle de sa carte. */
    fun route(h: Host): String? = computerOf(h)?.let(Accounts::route) ?: h.relayRoute

    /**
     * La console d'Admin d'un ordinateur venu d'un compte (jeton d'accès du relais à chaque appel) : sur le réseau s'il y
     * est vu (et pas [viaRelay]), sinon par le relais ; nulle sans compte ni chemin (ni vu, ni empreinte d'Admin connue).
     */
    fun console(h: Host, viaRelay: Boolean = false): ConsoleClient? {
        val acc = h.account ?: return null
        val account = store.account(acc.key) ?: return null
        val lan = if (viaRelay) null else lan(h)
        val route = if (lan == null && !Relays.off) route(h) else null
        if (lan == null && route == null) return null
        return ConsoleClient.account(lan?.address ?: "", lan?.port ?: 0, h.fingerprint, Accounts.access(account, acc.station), route)
    }

    /** Les paramètres de la session d'un ordinateur venu d'un compte ([Accounts.putSession]) ; faux sans chemin. */
    fun putSession(h: Host, params: JSONObject): Boolean {
        val acc = h.account ?: return false
        val account = store.account(acc.key) ?: return false
        val lan = lan(h)
        return Accounts.putSession(params, account, acc.station, h.fingerprint, lan?.let { it.address to it.port }, route(h), relayAllowed = !Relays.off)
    }

    // MARK: - Regarder (droit « Regarder » d'un compte, docs/comptes.md)

    /**
     * Les ordinateurs venus d'un compte qui ont le droit « Regarder » : vrai quand c'est le seul (« Regarder » à la place
     * de « Se connecter »), faux avec « Ouvrir le bureau » aussi (« Regarder » dans le menu de l'ordinateur).
     */
    fun watchRights(hosts: List<Host>): Map<String, Boolean> = hosts.filter { it.fromAccount }.mapNotNull { h ->
        computerOf(h)?.takeIf(Accounts::canWatch)?.let { h.id to Accounts.watchOnly(it) }
    }.toMap()

    /**
     * « Regarder » un ordinateur venu d'un compte : sur le réseau s'il y est vu, sinon par le relais (le cœur essaie dans
     * cet ordre). Nul sans compte ni chemin (ni vu sur le réseau, ni route par le relais).
     */
    suspend fun watch(h: Host, decoders: Decoders, quality: ShareQuality, auto: JSONObject?): WatchReply? {
        val acc = h.account ?: return null
        val account = store.account(acc.key) ?: return null
        val lan = lan(h)?.let { ShareTarget(it.address, it.port, h.fingerprint ?: it.fingerprint) }
        val route = if (Relays.off) null else route(h)
        if (lan == null && route == null) return null
        return Share.watch(Accounts.access(account, acc.station), lan, route, decoders, quality, auto)
    }

    // MARK: - Se connecter

    /**
     * « Se connecter à un compte KyberNow » par la fenêtre du système : la page de connexion du relais, à ouvrir dans un
     * onglet personnalisé ; la demande est gardée jusqu'au retour. Nul (et l'avis du refus) si le relais ne se lit pas.
     */
    fun beginSignIn(relay: String): String? = when (val r = Accounts.signIn(relay, app.device.name)) {
        is AccountReply.Ok -> {
            store.pendingSignIn = r.value.json
            r.value.url
        }
        is AccountReply.Refused -> {
            notice(r.message)
            null
        }
    }

    /**
     * Le retour `kybernow://compte?…` : échangé contre le compte (« Connexion au compte… »), puis « Connecté à
     * {compte}. » ; « Annuler » dans la page (75) : rien. Sans demande en cours (un vieux lien), rien non plus.
     */
    fun finishSignIn(callback: String) {
        val pending = store.pendingSignIn ?: return
        store.pendingSignIn = null
        scope.launch {
            _signingIn.value = true
            val r = Accounts.finishSignIn(pending, callback)
            _signingIn.value = false
            when (r) {
                is AccountReply.Ok -> added(r.value)
                is AccountReply.Refused -> if (!r.cancelled) notice(r.message)
            }
        }
    }

    /** Un compte connecté (fenêtre du système ou code approuvé) : gardé, annoncé, et ses ordinateurs arrivent. */
    fun added(account: StoredAccount) {
        store.put(account)
        notice(app.getString(R.string.account_signed_in, account.displayName))
        refresh()
    }

    /** « Se déconnecter » : la session d'appareil est fermée chez le relais, puis le compte oublié même s'il ne répond pas. */
    fun signOut(key: String) {
        val a = store.account(key) ?: return
        scope.launch {
            if (Accounts.signOut(a) is AccountReply.Refused) Log.i("KyberNow", "comptes : le relais n'a pas fermé la session, compte oublié quand même")
            forget(key)
        }
    }

    private fun forget(key: String) {
        store.remove(key)
        syncHosts()
    }
}
