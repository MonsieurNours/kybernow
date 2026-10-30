package fr.leslignesclaires.kybernow.ui

import android.app.Application
import android.graphics.BitmapFactory
import android.os.SystemClock
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.core.BorrowAsk
import fr.leslignesclaires.kybernow.core.BorrowReply
import fr.leslignesclaires.kybernow.core.ConsoleClient
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.core.LendAvailable
import fr.leslignesclaires.kybernow.core.LendStatus
import fr.leslignesclaires.kybernow.core.LendView
import fr.leslignesclaires.kybernow.core.Loan
import fr.leslignesclaires.kybernow.core.ShareTarget
import fr.leslignesclaires.kybernow.core.HostInfo
import fr.leslignesclaires.kybernow.core.HostRelay
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.core.RelayFailure
import fr.leslignesclaires.kybernow.core.Relays
import fr.leslignesclaires.kybernow.core.Reply
import fr.leslignesclaires.kybernow.core.WakeTarget
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.model.BorrowedComputer
import fr.leslignesclaires.kybernow.ui.lend.Borrowing
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.Profile
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Ce qu'on demande à la Station depuis cet appareil (rubrique « Station »). */
enum class StationAction { RESTART, UPDATE }

/** Pourquoi un redémarrage ou une mise à jour n'a pas eu lieu (textes de docs/link-ui.md). */
enum class StationFailure { TOO_SOON, TOO_OLD, REQUIRES_SETUP, NOT_BACK, UPDATE_FAILED, OTHER }

/** Où en est un redémarrage ou une mise à jour de la Station demandé depuis cet appareil. */
sealed interface StationTask {
    /** Un appareil est en session ([device], son nom si Admin le donne) : confirmer avant de couper sa session. */
    data class Confirm(val action: StationAction, val device: String?) : StationTask
    /** En cours : l'app suit la Station jusqu'à son retour (et, pour une mise à jour, jusqu'à [version]). */
    data class Working(val action: StationAction, val version: String?) : StationTask
    data class Done(val action: StationAction, val version: String?) : StationTask
    /** [message] : celui d'Admin ou du cœur, dans la langue de l'app ([StationFailure.OTHER]). */
    data class Failed(val reason: StationFailure, val message: String = "", val version: String? = null) : StationTask
}

/**
 * État d'un ordinateur, sondé toutes les 8 s (comme les autres apps). [away] : « Hors de chez vous » : l'ordinateur ne
 * répond pas sur le réseau mais il est connecté au relais (docs/relais.md) ; [online] vaut alors vrai (on peut s'y
 * connecter), sans [info].
 */
data class HostStatus(
    val online: Boolean? = null,
    val info: HostInfo? = null,
    /** Compte refusé (401) ou certificat changé : à reprendre dans les réglages. */
    val accountRefused: Boolean = false,
    val certificateChanged: Boolean = false,
    val away: Boolean = false,
) {
    val checked get() = online != null
}

/**
 * Pourquoi l'ordinateur n'a pas pu être ouvert hors de chez soi (textes de docs/link-ui.md, « Hors de chez vous »), choisi
 * par le code du cœur ([RelayFailure]), jamais par le texte du message d'Admin.
 */
sealed interface RelayRefusal {
    /** « {nom} n'est pas joignable : l'ordinateur est éteint ou en veille. » (31) */
    data object StationOffline : RelayRefusal
    /** « {nom} n'est pas joignable hors de chez vous (accès coupé dans Admin). » (32) */
    data object AccessOff : RelayRefusal
    /** Le texte du cœur ou d'Admin tel quel (« Le relais ne répond pas. », le blocage après trop d'essais). */
    data class Message(val text: String) : RelayRefusal

    companion object {
        /** Le refus d'un code du relais ; nul pour un autre ([RelayFailure.UNREACHABLE] garde le texte du cœur : [Message]). */
        fun of(failure: RelayFailure?, message: String): RelayRefusal? = when (failure) {
            RelayFailure.STATION_OFFLINE -> StationOffline
            RelayFailure.ACCESS_OFF -> AccessOff
            RelayFailure.UNREACHABLE -> Message(message)
            null -> null
        }
    }
}

/**
 * La carte « Ordinateur de {hôte} » d'un ordinateur prêté par un ami (docs/pret.md) : ce que la Station dit pour son compte
 * ([status] ; nul tant qu'on ne sait pas, ou sans mot de passe retenu), [offline] (« Hors ligne » : la Station ne répond
 * pas), [refused] : le compte refusé (« Identifiant ou mot de passe refusé. », le texte du cœur).
 */
data class FriendStatus(val status: LendStatus? = null, val offline: Boolean = false, val refused: String? = null)

/** « Emprunter » en cours depuis l'accueil : la Station prépare l'ordinateur, ou l'hôte doit accepter (« En attente de {hôte}… »). */
sealed interface BorrowStep {
    data object Preparing : BorrowStep
    data class Waiting(val host: String) : BorrowStep
}

/** Le relais n'est pas interrogé plus d'une fois toutes les 30 s pour savoir qui est connecté. */
private const val PRESENCE_EVERY_MS = 30_000L

class AppModel(app: Application) : AndroidViewModel(app) {
    private val kn = app as KyberNowApp
    val store = kn.hosts
    val hosts: StateFlow<List<Host>> = store.hosts

    private val _selected = MutableStateFlow(store.selectedId)
    val selected: StateFlow<String?> = _selected.asStateFlow()

    private val _status = MutableStateFlow<Map<String, HostStatus>>(emptyMap())
    val status: StateFlow<Map<String, HostStatus>> = _status.asStateFlow()

    private val _images = MutableStateFlow<Map<String, ImageBitmap>>(emptyMap())
    val images: StateFlow<Map<String, ImageBitmap>> = _images.asStateFlow()

    /** Ordinateurs annoncés sur le réseau et pas encore ajoutés. */
    val announced: StateFlow<List<Announced>> = combine(kn.discovery.found, store.hosts) { found, hosts ->
        found.filter { a -> hosts.none { (it.address == a.address && it.port == a.port) || (a.fingerprint != null && it.fingerprint == a.fingerprint) } }
    }.stateIn(viewModelScope, SharingStarted.Eagerly, emptyList())

    private val _refusals = MutableStateFlow<Map<String, RelayRefusal>>(emptyMap())
    /** Le refus d'ouvrir un ordinateur hors de chez soi, par ordinateur : jusqu'au prochain sondage ou au prochain essai. */
    val refusals: StateFlow<Map<String, RelayRefusal>> = _refusals.asStateFlow()

    private val _connecting = MutableStateFlow<Set<String>>(emptySet())
    /** Ordinateurs qu'on vérifie par le relais avant d'ouvrir la session (« Connexion… »). */
    val connecting: StateFlow<Set<String>> = _connecting.asStateFlow()

    private val _station = MutableStateFlow<Map<String, StationTask>>(emptyMap())
    /** Redémarrage ou mise à jour de la Station en cours, par ordinateur. */
    val station: StateFlow<Map<String, StationTask>> = _station.asStateFlow()

    private var polling: Job? = null

    // Prêt de l'ordinateur (docs/pret.md).
    /** Les ordinateurs que des amis prêtent à cet appareil (« Ordinateur de {hôte} »). */
    val borrowed: StateFlow<List<BorrowedComputer>> = store.borrowed

    private val _friends = MutableStateFlow<Map<String, FriendStatus>>(emptyMap())
    val friends: StateFlow<Map<String, FriendStatus>> = _friends.asStateFlow()

    private val _borrowing = MutableStateFlow<Map<String, BorrowStep>>(emptyMap())
    /** « Emprunter » en cours, par ordinateur prêté. */
    val borrowing: StateFlow<Map<String, BorrowStep>> = _borrowing.asStateFlow()

    private val _borrowErrors = MutableStateFlow<Map<String, String>>(emptyMap())
    /** Le refus du dernier « Emprunter » (« {hôte} a refusé. », « L'ordinateur est verrouillé. »…), jusqu'au prochain essai. */
    val borrowErrors: StateFlow<Map<String, String>> = _borrowErrors.asStateFlow()

    private val borrowJobs = mutableMapOf<String, Job>()

    /** Le mot de passe d'un compte d'ami tapé sur l'accueil sans être retenu : gardé en mémoire le temps de l'app. */
    private val typed = mutableMapOf<String, String>()

    private val _lend = MutableStateFlow<Map<String, LendView>>(emptyMap())
    /** Chez l'hôte : la vue du prêt de chacun de ses ordinateurs (« Prêté à Léa », « Reprendre l'ordinateur »). */
    val lend: StateFlow<Map<String, LendView>> = _lend.asStateFlow()

    private val _lendErrors = MutableStateFlow<Map<String, String>>(emptyMap())
    /** Le refus du dernier « Reprendre l'ordinateur ». */
    val lendErrors: StateFlow<Map<String, String>> = _lendErrors.asStateFlow()

    fun current(): Host? = hosts.value.firstOrNull { it.id == _selected.value } ?: hosts.value.firstOrNull()

    fun select(id: String) {
        _selected.value = id
        store.selectedId = id
        hosts.value.firstOrNull { it.id == id }?.let { poll(it) }
        borrowed.value.firstOrNull { it.id == id }?.let { viewModelScope.launch { checkFriend(it) } }
    }

    /** Choisir un profil remet le format, le débit et le transport aux siens (« Personnalisé » s'efface). */
    fun setProfile(id: String, p: Profile) = store.update(id) { it.copy(settings = it.settings.apply(p)) }

    /** « Réessayer » : « Recherche… » le temps d'interroger Admin de nouveau. */
    fun retry(id: String) {
        val h = hosts.value.firstOrNull { it.id == id } ?: return
        _status.value = _status.value + (id to HostStatus())
        poll(h)
    }

    fun remove(id: String) {
        store.remove(id)
        _selected.value = store.selectedId
        _images.value = _images.value - id
    }

    /** Sondage d'Admin et découverte, tant que l'accueil est à l'écran. */
    fun startPolling() {
        kn.discovery.start()
        kn.gamepads.start()
        if (polling?.isActive == true) return
        polling = viewModelScope.launch {
            while (true) {
                hosts.value.forEach { loadImage(it.id) }
                hosts.value.forEach { poll(it) }
                borrowed.value.forEach { b -> launch { checkFriend(b) } }
                delay(8000)
            }
        }
    }

    fun stopPolling() {
        polling?.cancel()
        polling = null
        kn.discovery.stop()
        kn.gamepads.stop()
    }

    private fun client(h: Host, pinned: String? = h.fingerprint) = ConsoleClient(h.address, h.port, h.user, store.password(h.id) ?: "", pinned)

    private fun poll(h: Host) = viewModelScope.launch { check(h) }

    /**
     * Interroge Admin une fois et range ce qu'il dit : l'état, et ce qui se garde avec l'ordinateur
     * (système, carte graphique, cartes réseau pour le réveil, accès propre à l'appareil).
     */
    private suspend fun check(h: Host): HostStatus {
        val password = store.password(h.id) ?: ""
        val reply = client(h).use { it.info() }
        val st = when (reply) {
            is Reply.Ok -> HostStatus(true, reply.value)
            is Reply.Failed -> {
                val answered = reply.unauthorized || reply.certificateChanged
                // Rien ne répond sur le réseau : peut-être hors de chez soi, l'ordinateur connecté au relais.
                if (!answered && reachableByRelay(h)) HostStatus(online = true, away = true)
                else HostStatus(online = answered, accountRefused = reply.unauthorized, certificateChanged = reply.certificateChanged)
            }
        }
        _status.value = _status.value + (h.id to st)
        // Un nouveau sondage efface le refus d'un essai précédent (l'état peut avoir changé).
        if (_refusals.value.containsKey(h.id) && h.id !in _connecting.value) _refusals.value = _refusals.value - h.id
        if (reply is Reply.Ok) {
            val i = reply.value
            // Une liste vide (Station d'avant la 0.5.0, cartes illisibles) garde les cartes apprises.
            val wake = i.wake.ifEmpty { h.wake }
            val paired = h.paired || i.ownDevice
            // Le relais de l'ordinateur (hors de chez soi) : gardé avec lui, hors ligne ; retiré quand Admin n'en règle plus.
            // Sans empreinte épinglée (premier contact pas fait), la route n'est pas valable : l'ancienne reste.
            val relay = if (i.relay == null) null else HostRelay.of(i, h.fingerprint) ?: h.relay
            if (i.os != h.os || i.gpu != h.gpu || i.av1 != h.av1 || i.virtualDisplay != h.virtualDisplay || wake != h.wake || paired != h.paired || relay != h.relay) {
                store.update(h.id) { it.copy(os = i.os, gpu = i.gpu, av1 = i.av1, virtualDisplay = i.virtualDisplay, wake = wake, paired = paired, relay = relay) }
            }
            // Pas encore d'image : l'aperçu d'Admin (jamais pendant une session).
            if (!store.imageFile(h.id).exists()) fetchPreview(h, password)
            // Le prêt activé dans Admin : sa vue, pour « Prêté à {prénom} » et « Reprendre l'ordinateur ».
            if (i.lend == LendAvailable.ON) {
                (client(h).use { it.lendState() } as? Reply.Ok)?.value.let { v -> _lend.value = if (v == null) _lend.value - h.id else _lend.value + (h.id to v) }
            } else if (_lend.value.containsKey(h.id)) {
                _lend.value = _lend.value - h.id
            }
        }
        return st
    }

    // MARK: - Prêt de l'ordinateur : l'hôte

    /**
     * « Reprendre l'ordinateur » (l'ami a 2 min pour sauvegarder) ou, [now], « Reprendre tout de suite » : la vue rendue
     * remplace l'ancienne ; un refus s'écrit sous l'état.
     */
    fun takeBack(id: String, now: Boolean) {
        val h = hosts.value.firstOrNull { it.id == id } ?: return
        _lendErrors.value = _lendErrors.value - id
        viewModelScope.launch {
            when (val r = client(h).use { it.lendTakeBack(now) }) {
                is Reply.Ok -> _lend.value = r.value?.let { _lend.value + (id to it) } ?: (_lend.value - id)
                is Reply.Failed -> _lendErrors.value = _lendErrors.value + (id to r.message)
            }
        }
    }

    // MARK: - Prêt de l'ordinateur : l'ami

    /** Le mot de passe d'un compte d'ami : retenu (Keystore), sinon tapé pendant cette session de l'app. */
    fun friendPassword(id: String): String? = store.password(id) ?: typed[id]

    /** L'état de la carte « Ordinateur de {hôte} » (sans mot de passe connu : rien à demander). */
    private suspend fun checkFriend(b: BorrowedComputer) {
        val password = friendPassword(b.id) ?: return
        val target = ShareTarget(b.address, b.port, b.fingerprint)
        val st = when (val r = Lend.status(target, b.route, b.login, password)) {
            is Reply.Ok -> {
                // Le prénom de l'hôte ou le nom de l'ordinateur ont changé : la carte suit.
                if (r.value.host.isNotBlank() && (r.value.host != b.host || r.value.computer != b.computer)) {
                    store.updateBorrowed(b.id) { it.copy(host = r.value.host, computer = r.value.computer.ifBlank { it.computer }) }
                }
                FriendStatus(r.value)
            }
            is Reply.Failed -> if (r.code == Lend.BAD_CREDENTIALS) FriendStatus(refused = r.message) else FriendStatus(offline = true)
        }
        _friends.value = _friends.value + (b.id to st)
    }

    /**
     * « Emprunter » (ou « Demander à {hôte} ») : la demande avec le compte d'ami ; la Station prépare l'ordinateur
     * (« Préparation de l'ordinateur », 30 s au plus), ou l'hôte doit accepter (« En attente de {hôte}… »). [password] :
     * tapé sur l'accueil (le mot de passe n'est pas retenu), sinon celui que l'app connaît. [open] ouvre la session.
     */
    fun borrow(id: String, password: String?, open: (Loan, BorrowAsk) -> Unit) {
        val b = borrowed.value.firstOrNull { it.id == id } ?: return
        if (id in _borrowing.value) return
        val pw = password?.takeIf { it.isNotEmpty() } ?: friendPassword(id) ?: return
        // Un mot de passe retapé (l'ancien refusé) remplace celui qui était retenu ; sinon, il reste en mémoire seulement.
        if (password != null) {
            if (store.password(id) != null) store.setPassword(id, pw) else typed[id] = pw
        }
        _borrowErrors.value = _borrowErrors.value - id
        _borrowing.value = _borrowing.value + (id to BorrowStep.Preparing)
        val ask = Borrowing.ask(getApplication(), ShareTarget(b.address, b.port, b.fingerprint), b.route, kn.guestName, login = b.login, password = pw)
        borrowJobs[id] = viewModelScope.launch {
            try {
                val first = Lend.borrow(ask)
                val reply = if (first is BorrowReply.Pending) {
                    _borrowing.value = _borrowing.value + (id to BorrowStep.Waiting(first.pending.host.ifBlank { b.host }))
                    Lend.waitForHost(first.pending)
                } else first
                when (reply) {
                    is BorrowReply.Ready -> open(reply.loan, ask)
                    is BorrowReply.Refused -> {
                        _borrowErrors.value = _borrowErrors.value + (id to reply.message)
                        // Un mot de passe refusé n'est plus gardé en mémoire : il faudra le retaper.
                        if (reply.badCredentials) typed.remove(id)
                    }
                    is BorrowReply.Pending -> {}
                }
            } finally {
                _borrowing.value = _borrowing.value - id
                borrowJobs.remove(id)
                checkFriend(b)
            }
        }
    }

    /** « Annuler » l'attente de l'hôte (ou la préparation) : la demande est retirée chez l'hôte. */
    fun cancelBorrow(id: String) {
        borrowJobs.remove(id)?.cancel()
        _borrowing.value = _borrowing.value - id
    }

    /** « Retirer cet ordinateur » : la carte et son mot de passe quittent l'appareil. */
    fun removeBorrowed(id: String) {
        cancelBorrow(id)
        typed.remove(id)
        store.removeBorrowed(id)
        _friends.value = _friends.value - id
        _selected.value = store.selectedId
    }

    // MARK: - Hors de chez vous (relais)

    /** Ce que le relais a dit de ses Stations, à quelle heure (ms, horloge de l'appareil). */
    private class Presence(val at: Long, val present: Map<String, Boolean>)

    private val presence = mutableMapOf<String, Presence>()
    private val presenceLock = Mutex()

    /**
     * L'ordinateur est-il connecté à son relais ? Le relais est interrogé pour tous les ordinateurs qui s'y trouvent à
     * la fois, au plus toutes les 30 s (une connexion brève à chaque fois). Faux sans relais connu ni empreinte d'Admin
     * épinglée, et avec `KYBERNOW_RELAY=off`.
     */
    private suspend fun reachableByRelay(h: Host): Boolean {
        val relay = h.relay ?: return false
        if (h.relayRoute == null || Relays.off) return false
        // Deux relais sont le même si leurs formes normales sont égales (`share.kybernow.app` et `share.kybernow.app:443`).
        val server = Relays.normalize(relay.server)
        return presenceLock.withLock {
            val now = SystemClock.elapsedRealtime()
            val known = presence[server]
            if (known == null || now - known.at >= PRESENCE_EVERY_MS) {
                val ids = hosts.value.mapNotNull { it.relay }.filter { Relays.normalize(it.server) == server }.map { it.station }.distinct()
                val present = mutableMapOf<String, Boolean>()
                for (chunk in ids.chunked(Relays.MAX_STATIONS)) {
                    val answer = Relays.presence(server, chunk) ?: break
                    chunk.forEachIndexed { i, id -> present[id] = answer.getOrElse(i) { false } }
                }
                presence[server] = Presence(now, present)
            }
            presence[server]?.present?.get(relay.station) == true
        }
    }

    /**
     * « Se connecter » à un ordinateur « Hors de chez vous » : l'ordinateur est vérifié par le relais d'abord (son
     * `info`, par le tunnel, Admin épinglé), pour dire pourquoi si ce n'est pas possible : « {nom} n'est pas joignable
     * : l'ordinateur est éteint ou en veille. », « … hors de chez vous (accès coupé dans Admin). », « Le relais ne
     * répond pas. » ([refusals]). [open] ouvre la session quand tout va bien (ou quand l'ordinateur est sur le réseau).
     */
    fun connect(id: String, open: (Host) -> Unit) {
        val h = hosts.value.firstOrNull { it.id == id } ?: return
        val route = h.relayRoute
        if (_status.value[id]?.away != true || route == null) return open(h)
        if (id in _connecting.value) return
        _connecting.value = _connecting.value + id
        _refusals.value = _refusals.value - id
        viewModelScope.launch {
            val reply = ConsoleClient(h.address, h.port, h.user, store.password(h.id) ?: "", h.fingerprint, route).use { it.info() }
            _connecting.value = _connecting.value - id
            when (reply) {
                is Reply.Ok -> open(h)
                is Reply.Failed -> when {
                    // Par le code du cœur (30, 31, 32), jamais par le texte du message.
                    reply.relayFailure != null -> refuse(id, RelayRefusal.of(reply.relayFailure, reply.message)!!)
                    reply.unauthorized -> _status.value = _status.value + (id to HostStatus(online = true, accountRefused = true, away = true))
                    reply.certificateChanged -> _status.value = _status.value + (id to HostStatus(online = true, certificateChanged = true, away = true))
                    else -> refuse(id, RelayRefusal.Message(reply.message))
                }
            }
        }
    }

    private fun refuse(id: String, why: RelayRefusal) {
        _refusals.value = _refusals.value + (id to why)
        // L'ordinateur a pu couper son accès depuis le dernier sondage : le relais est redemandé au prochain.
        presence.clear()
    }

    // MARK: - Réveil par le réseau

    private val _waking = MutableStateFlow<Set<String>>(emptySet())
    /** Ordinateurs qu'on réveille en ce moment (« Réveil de l'ordinateur… »). */
    val waking: StateFlow<Set<String>> = _waking.asStateFlow()

    /**
     * « Réveiller l'ordinateur » : le paquet de réveil vers ses cartes (le cœur : diffusion du
     * sous-réseau, 255.255.255.255 et en direct, ports 9 et 7), puis Admin interrogé toutes les
     * 2 s, 2 min au plus, jusqu'à ce que l'ordinateur soit en ligne. Sans cible : « Réessayer ».
     */
    fun wake(id: String) {
        val h = hosts.value.firstOrNull { it.id == id } ?: return
        val targets = h.wakeTargets
        if (targets.isEmpty()) return retry(id)
        if (id in _waking.value) return
        _waking.value = _waking.value + id
        viewModelScope.launch {
            val code = withContext(Dispatchers.IO) { Native.wake(WakeTarget.toJson(targets)) }
            if (code != 0) android.util.Log.w("KyberNow", "réveil : aucun paquet parti ($code)")
            val start = System.currentTimeMillis()
            while (System.currentTimeMillis() - start < 120_000) {
                delay(2000)
                val current = hosts.value.firstOrNull { it.id == id } ?: break
                if (check(current).online == true) break
            }
            _waking.value = _waking.value - id
        }
    }

    private suspend fun fetchPreview(h: Host, password: String) {
        val width = getApplication<Application>().resources.displayMetrics.widthPixels.coerceIn(640, 1920)
        val bytes = ConsoleClient(h.address, h.port, h.user, password, h.fingerprint).use { it.preview(width) } ?: return
        if (bytes.isEmpty()) return
        withContext(Dispatchers.IO) { store.imageFile(h.id).writeBytes(bytes) }
        loadImage(h.id, force = true)
    }

    fun loadImage(id: String, force: Boolean = false) {
        if (!force && _images.value.containsKey(id)) return
        viewModelScope.launch {
            val bmp = withContext(Dispatchers.IO) {
                val f = store.imageFile(id)
                if (f.exists()) runCatching { BitmapFactory.decodeFile(f.path)?.asImageBitmap() }.getOrNull() else null
            } ?: return@launch
            _images.value = _images.value + (id to bmp)
        }
    }

    /** Après une session : la dernière image a changé. */
    fun refreshAfterSession() {
        hosts.value.forEach { loadImage(it.id, force = true) }
        current()?.let { poll(it) }
    }

    // MARK: - Station

    /**
     * Redémarre ou met à jour la Station ([force] : même si un appareil est en session, coupée),
     * puis la suit jusqu'à son retour. Un appareil en session donne [StationTask.Confirm].
     */
    fun station(id: String, action: StationAction, force: Boolean = false) {
        val h = hosts.value.firstOrNull { it.id == id } ?: return
        if (_station.value[id] is StationTask.Working) return
        // Par le relais (« Hors de chez vous »), jamais de redémarrage ni de mise à jour : l'interface ne les propose pas.
        if (_status.value[id]?.away == true) return
        val target = _status.value[id]?.info?.station?.update?.version
        setStation(id, StationTask.Working(action, target.takeIf { action == StationAction.UPDATE }))
        viewModelScope.launch {
            val reply: Reply<String?> = client(h).use { c ->
                when (action) {
                    StationAction.RESTART -> when (val r = c.stationRestart(kn.device.id, force)) {
                        is Reply.Ok -> Reply.Ok(null)
                        is Reply.Failed -> r
                    }
                    StationAction.UPDATE -> c.stationUpdate(kn.device.id, force)
                }
            }
            when (reply) {
                is Reply.Ok -> follow(id, action, reply.value?.takeIf { it.isNotEmpty() } ?: target)
                is Reply.Failed -> setStation(
                    id,
                    when {
                        reply.inSession -> StationTask.Confirm(action, reply.device)
                        reply.tooSoon -> StationTask.Failed(StationFailure.TOO_SOON)
                        reply.stationTooOld -> StationTask.Failed(StationFailure.TOO_OLD)
                        reply.requiresSetup -> StationTask.Failed(StationFailure.REQUIRES_SETUP, version = target)
                        // Admin a refusé ou raté le téléchargement (erreur HTTP) : il en dit plus.
                        action == StationAction.UPDATE && reply.code <= -400 && !reply.unauthorized -> StationTask.Failed(StationFailure.UPDATE_FAILED)
                        else -> StationTask.Failed(StationFailure.OTHER, reply.message)
                    },
                )
            }
        }
    }

    /** Ferme une confirmation, un échec ou un résultat. */
    fun stationDismiss(id: String) {
        if (_station.value[id] !is StationTask.Working) _station.value = _station.value - id
    }

    private fun setStation(id: String, task: StationTask) {
        _station.value = _station.value + (id to task)
    }

    /**
     * Admin s'arrête puis revient (environ 30 s pour un redémarrage, une minute pour une mise à
     * jour) : interrogé toutes les 2 s, 4 min au plus. Revenu après une absence (ou un
     * redémarrage annoncé par `restarting`) : fait ; pour une mise à jour, dans la [version] attendue.
     */
    private suspend fun follow(id: String, action: StationAction, version: String?) {
        val start = System.currentTimeMillis()
        var away = false
        // Admin ne répondait plus : la Station a vraiment redémarré.
        var down = false
        delay(3000)
        while (System.currentTimeMillis() - start < 240_000) {
            val h = hosts.value.firstOrNull { it.id == id } ?: return
            when (val reply = client(h).use { it.info() }) {
                is Reply.Ok -> {
                    _status.value = _status.value + (id to HostStatus(true, reply.value))
                    val st = reply.value.station
                    val busy = st?.restarting == true || st?.update?.installing == true
                    val back = when {
                        busy -> false
                        action == StationAction.UPDATE && version != null -> st?.version == version
                        else -> away
                    }
                    // Revenue sans la nouvelle version : la mise à jour n'a pas abouti.
                    if (!busy && !back && down && action == StationAction.UPDATE && version != null) {
                        setStation(id, StationTask.Failed(StationFailure.UPDATE_FAILED))
                        return
                    }
                    if (busy) away = true
                    if (back) {
                        setStation(id, StationTask.Done(action, st?.version))
                        delay(20_000)
                        if (_station.value[id] is StationTask.Done) _station.value = _station.value - id
                        return
                    }
                }
                is Reply.Failed -> { away = true; down = true }
            }
            delay(2000)
        }
        setStation(id, StationTask.Failed(StationFailure.NOT_BACK))
    }

    // MARK: - Empreinte

    /**
     * L'empreinte du certificat que présente Admin maintenant (la Station a été réinstallée) :
     * premier contact sans épingle, comme à l'ajout. Nul si Admin ne répond pas.
     */
    suspend fun seenFingerprint(id: String): String? {
        val h = hosts.value.firstOrNull { it.id == id } ?: return null
        return client(h, pinned = null).use { c -> c.probe(); c.fingerprintSeen }
    }

    /** L'empreinte comparée à celle d'Admin : épinglée à la place de l'ancienne. */
    fun trustFingerprint(id: String, fingerprint: String) {
        store.update(id) { it.copy(fingerprint = fingerprint) }
        hosts.value.firstOrNull { it.id == id }?.let { poll(it) }
    }
}
