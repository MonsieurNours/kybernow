package fr.leslignesclaires.kybernow.playstation

import android.content.Context
import android.net.wifi.WifiManager
import android.os.SystemClock
import android.util.Log
import fr.leslignesclaires.kybernow.core.ConsoleClient
import fr.leslignesclaires.kybernow.core.PsNative
import fr.leslignesclaires.kybernow.core.Relays
import fr.leslignesclaires.kybernow.core.Reply
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/** Une console qui a répondu à la recherche. [target] : pour l'association. */
data class PsFound(
    val address: String,
    val hostId: String,
    val name: String,
    val model: String,
    val state: String,
    val target: String,
    val runningApp: String?,
    val at: Long,
) {
    val ready get() = state == "ready"

    companion object {
        fun list(json: String, at: Long = SystemClock.elapsedRealtime()): List<PsFound> = runCatching {
            val a = JSONArray(json)
            List(a.length()) {
                val o = a.getJSONObject(it)
                PsFound(
                    o.optString("address"), o.optString("host_id").uppercase(), o.optString("name"), o.optString("model"), o.optString("state"),
                    o.optString("target"), o.optString("running_app").takeIf { s -> !o.isNull("running_app") && s.isNotEmpty() }, at,
                )
            }
        }.getOrDefault(emptyList())
    }
}

/** L'état d'une console vu d'ici (docs/link-ui.md, « Consoles PlayStation »). */
enum class PsStatus { SEARCHING, ON, STANDBY, OFFLINE, AWAY }

/** Pourquoi une association n'a pas abouti (textes de link-ui). */
enum class PairError { NO_ANSWER, WRONG_CODE, ACCOUNT, UNREACHABLE }

/** Une console jointe hors de chez soi : l'adresse par le tunnel et la poignée du cœur (signe de vie à la Station). */
class PsRemote(val handle: Long, val address: String, val station: Host) {
    fun close() = PsNative.remoteFree(handle)
}

/** Pourquoi une console n'a pas pu être jointe hors de chez soi. */
sealed interface ReachError {
    /** Refus du relais (30, 31, 32) : les textes d'un ordinateur par le relais, avec le nom de la Station. */
    data class Relay(val code: Int, val message: String, val station: String) : ReachError
    /** La Station ne voit pas la console, ou elle ne se réveille pas : « La console ne répond pas. » */
    data object NotAnswering : ReachError
}

/**
 * Les consoles de l'app tant que l'accueil (ou l'ajout) est à l'écran : recherche sur le réseau toutes les 3 s (en
 * diffusion et en direct vers les adresses connues, verrou de multidiffusion du Wi-Fi pris), réveil, association, et hors
 * de chez soi la Station qui voit chaque console (`GET /api/consoles`, une fois par minute au plus par Station).
 */
class PlayStations(private val context: Context, val store: PsStore, private val hosts: HostStore) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val _found = MutableStateFlow<Map<String, PsFound>>(emptyMap())
    /** Dernière réponse de chaque console, par identifiant. */
    val found: StateFlow<Map<String, PsFound>> = _found.asStateFlow()
    private val _searched = MutableStateFlow(false)
    val searched: StateFlow<Boolean> = _searched.asStateFlow()
    private val _waking = MutableStateFlow<Set<String>>(emptySet())
    /** Consoles qu'on réveille (« Réveil de la console… »). */
    val waking: StateFlow<Set<String>> = _waking.asStateFlow()
    private val _reaching = MutableStateFlow<Set<String>>(emptySet())
    /** Hors de chez soi : consoles qu'on joint par une Station (« Connexion à la console… »). */
    val reaching: StateFlow<Set<String>> = _reaching.asStateFlow()
    private val _reachErrors = MutableStateFlow<Map<String, ReachError>>(emptyMap())
    val reachErrors: StateFlow<Map<String, ReachError>> = _reachErrors.asStateFlow()
    private val _away = MutableStateFlow<Set<String>>(emptySet())
    /** Les ordinateurs joignables seulement par le relais en ce moment (sondage de l'accueil). */
    val awayHosts: StateFlow<Set<String>> = _away.asStateFlow()

    private var job: Job? = null
    private var users = 0
    private val asked = mutableMapOf<String, Long>()
    private var lock: WifiManager.MulticastLock? = null

    /** La recherche tourne tant qu'un écran la demande (accueil, ajout). */
    @Synchronized
    fun start() {
        users++
        if (job?.isActive == true) return
        lock = (context.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager)
            ?.createMulticastLock("kybernow-playstation")?.apply { setReferenceCounted(false); acquire() }
        job = scope.launch {
            while (isActive) {
                searchOnce()
                delay(3000)
            }
        }
    }

    @Synchronized
    fun stop() {
        users = (users - 1).coerceAtLeast(0)
        if (users > 0) return
        job?.cancel()
        job = null
        runCatching { lock?.release() }
        lock = null
    }

    /** Toutes les adresses à chercher en direct : celles saisies et la dernière de chaque console. */
    private fun direct(): List<String> = (store.addresses + store.consoles.value.map { it.address }).filter { it.isNotBlank() }.distinct()

    suspend fun searchOnce(waitMs: Int = 1500) {
        val list = withContext(Dispatchers.IO) { PsFound.list(PsNative.search(JSONArray(direct()).toString(), waitMs)) }
        val now = SystemClock.elapsedRealtime()
        _found.value = (_found.value.filterValues { now - it.at < FORGET_AFTER_MS } + list.associateBy { it.hostId })
        _searched.value = true
        // La console a changé d'adresse ou de nom : on suit.
        for (f in list) {
            val c = store.consoleByHost(f.hostId) ?: continue
            if (c.address != f.address || (f.name.isNotBlank() && c.name != f.name)) store.update(c.id) { it.copy(address = f.address, name = f.name.ifBlank { it.name }) }
        }
    }

    /** Une console à une adresse saisie (« Par adresse ») : 1,5 s, sans diffusion. */
    suspend fun searchAt(address: String, waitMs: Int = 1500): PsFound? = withContext(Dispatchers.IO) {
        PsFound.list(PsNative.searchAt(address, waitMs)).firstOrNull()
    }

    fun rememberAddress(address: String) {
        if (address !in store.addresses) store.addresses = store.addresses + address
    }

    /** La dernière réponse de la console, si elle est récente. */
    fun live(hostId: String): PsFound? = _found.value[hostId.uppercase()]?.takeIf { SystemClock.elapsedRealtime() - it.at < OFFLINE_AFTER_MS }

    fun status(c: PsConsole): PsStatus {
        val f = live(c.hostId)
        return when {
            f != null && f.ready -> PsStatus.ON
            f != null -> PsStatus.STANDBY
            !_searched.value -> PsStatus.SEARCHING
            awayStation(c) != null -> PsStatus.AWAY
            else -> PsStatus.OFFLINE
        }
    }

    /** Hors de chez soi : une Station qui voit la console et qu'on joint par le relais. */
    fun awayStation(c: PsConsole): Host? {
        if (Relays.off) return null
        return store.stations(c.hostId).mapNotNull { hosts.get(it) }.firstOrNull { it.id in _away.value && it.relayRoute != null }
    }

    // MARK: - Les Stations qui voient les consoles

    /** Le sondage de l'accueil : l'ordinateur ne répond que par le relais (ou plus). */
    fun setAway(hostId: String, away: Boolean) {
        _away.value = if (away) _away.value + hostId else _away.value - hostId
    }

    /** Un ordinateur répond sur le réseau : ses consoles (`GET /api/consoles`), une fois par minute au plus. */
    fun stationReachable(h: Host, password: String) {
        val now = SystemClock.elapsedRealtime()
        synchronized(asked) {
            if ((asked[h.id] ?: 0L) + STATIONS_EVERY_MS > now && asked.containsKey(h.id)) return
            asked[h.id] = now
        }
        scope.launch {
            when (val r = ConsoleClient(h.address, h.port, h.user, password, h.fingerprint).use { it.playStations() }) {
                is Reply.Ok -> store.setSeenBy(h.id, (0 until r.value.length()).mapNotNull { r.value.optJSONObject(it)?.optString("host_id")?.uppercase() }.toSet())
                // Station d'avant les consoles, ou refus : rien n'est changé.
                is Reply.Failed -> {}
            }
        }
    }

    // MARK: - Réveil

    /** « Réveiller et jouer » : le paquet de réveil, puis la recherche jusqu'à ce qu'elle soit allumée (30 s au plus). */
    suspend fun wake(c: PsConsole, address: String = c.address): Boolean {
        val reg = store.registered(c) ?: return false
        _waking.value = _waking.value + c.id
        try {
            withContext(Dispatchers.IO) { PsNative.wake(address, reg) }
            val start = SystemClock.elapsedRealtime()
            while (SystemClock.elapsedRealtime() - start < 30_000) {
                delay(1500)
                val f = if (address == c.address) { searchOnce(1000); live(c.hostId) } else searchAt(address, 1000)
                if (f?.ready == true) return true
                // Toutes les 10 s, le paquet repart (la console peut l'avoir manqué).
                if ((SystemClock.elapsedRealtime() - start) % 10_000 < 1500) withContext(Dispatchers.IO) { PsNative.wake(address, reg) }
            }
            return false
        } finally {
            _waking.value = _waking.value - c.id
        }
    }

    // MARK: - Hors de chez soi

    /**
     * Joint la console par une Station qui la voit (relais) : signe de vie à Admin, état lu par le tunnel, réveil s'il
     * le faut. Rend la console jointe (à fermer par [PsRemote.close] à la fin de la partie), ou nul (refus dans
     * [reachErrors]).
     */
    suspend fun reach(c: PsConsole): PsRemote? {
        val station = awayStation(c) ?: run { _reachErrors.value = _reachErrors.value + (c.id to ReachError.NotAnswering); return null }
        _reachErrors.value = _reachErrors.value - c.id
        _reaching.value = _reaching.value + c.id
        try {
            val reply = ConsoleClient(station.address, station.port, station.user, hosts.password(station.id) ?: "", station.fingerprint, station.relayRoute)
                .use { it.playStationRemote(c.hostId) }
            val handle = when (reply) {
                is Reply.Ok -> reply.value
                is Reply.Failed -> {
                    _reachErrors.value = _reachErrors.value + (c.id to if (reply.relayFailure != null) ReachError.Relay(reply.code, reply.message, station.name) else ReachError.NotAnswering)
                    return null
                }
            }
            val address = PsNative.remoteAddress(handle) ?: run { PsNative.remoteFree(handle); return null }
            val remote = PsRemote(handle, address, station)
            val seen = searchAt(address, 2000)
            if (seen?.ready != true) {
                _reaching.value = _reaching.value - c.id
                if (!wake(c, address)) {
                    remote.close()
                    _reachErrors.value = _reachErrors.value + (c.id to ReachError.NotAnswering)
                    return null
                }
            }
            return remote
        } finally {
            _reaching.value = _reaching.value - c.id
        }
    }

    // MARK: - Association

    sealed interface PairResult {
        data class Ok(val console: PsConsole) : PairResult
        data class Failed(val error: PairError) : PairResult
    }

    /** Associe la console trouvée avec le compte et le code à 8 chiffres affiché par la console (bloque quelques secondes). */
    suspend fun pair(f: PsFound, account: PsAccount, pin: Int): PairResult {
        val id = store.psnId(account.id) ?: return PairResult.Failed(PairError.ACCOUNT)
        val o = withContext(Dispatchers.IO) { JSONObject(PsNative.regist(f.address, f.target, id, pin)) }
        o.optJSONObject("registered")?.let { reg ->
            return PairResult.Ok(store.paired(f.hostId, f.name, f.model, f.address, f.target, reg.toString(), account.id))
        }
        Log.w("KyberNow", "PlayStation : association refusée (${o.optString("error")})")
        return PairResult.Failed(
            when (o.optString("error")) {
                "no_answer" -> PairError.NO_ANSWER
                "wrong_code" -> PairError.WRONG_CODE
                "account" -> PairError.ACCOUNT
                else -> PairError.UNREACHABLE
            },
        )
    }

    private companion object {
        /** Sans réponse depuis 12 s : hors ligne. */
        const val OFFLINE_AFTER_MS = 12_000L
        const val FORGET_AFTER_MS = 120_000L
        const val STATIONS_EVERY_MS = 60_000L
    }
}
