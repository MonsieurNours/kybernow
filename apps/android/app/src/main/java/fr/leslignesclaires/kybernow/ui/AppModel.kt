package fr.leslignesclaires.kybernow.ui

import android.app.Application
import android.graphics.BitmapFactory
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.core.ConsoleClient
import fr.leslignesclaires.kybernow.core.HostInfo
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.core.Reply
import fr.leslignesclaires.kybernow.core.WakeTarget
import fr.leslignesclaires.kybernow.model.Announced
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

/** État d'un ordinateur, sondé toutes les 8 s (comme les autres apps). */
data class HostStatus(
    val online: Boolean? = null,
    val info: HostInfo? = null,
    /** Compte refusé (401) ou certificat changé : à reprendre dans les réglages. */
    val accountRefused: Boolean = false,
    val certificateChanged: Boolean = false,
) {
    val checked get() = online != null
}

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

    private val _station = MutableStateFlow<Map<String, StationTask>>(emptyMap())
    /** Redémarrage ou mise à jour de la Station en cours, par ordinateur. */
    val station: StateFlow<Map<String, StationTask>> = _station.asStateFlow()

    private var polling: Job? = null

    fun current(): Host? = hosts.value.firstOrNull { it.id == _selected.value } ?: hosts.value.firstOrNull()

    fun select(id: String) {
        _selected.value = id
        store.selectedId = id
        hosts.value.firstOrNull { it.id == id }?.let { poll(it) }
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
            is Reply.Failed -> HostStatus(
                online = reply.unauthorized || reply.certificateChanged,
                accountRefused = reply.unauthorized,
                certificateChanged = reply.certificateChanged,
            )
        }
        _status.value = _status.value + (h.id to st)
        if (reply is Reply.Ok) {
            val i = reply.value
            // Une liste vide (Station d'avant la 0.5.0, cartes illisibles) garde les cartes apprises.
            val wake = i.wake.ifEmpty { h.wake }
            val paired = h.paired || i.ownDevice
            if (i.os != h.os || i.gpu != h.gpu || i.av1 != h.av1 || i.virtualDisplay != h.virtualDisplay || wake != h.wake || paired != h.paired) {
                store.update(h.id) { it.copy(os = i.os, gpu = i.gpu, av1 = i.av1, virtualDisplay = i.virtualDisplay, wake = wake, paired = paired) }
            }
            // Pas encore d'image : l'aperçu d'Admin (jamais pendant une session).
            if (!store.imageFile(h.id).exists()) fetchPreview(h, password)
        }
        return st
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
