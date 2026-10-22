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
import fr.leslignesclaires.kybernow.core.Reply
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

/** Comment l'ordinateur est joint. */
enum class Reach { LOCAL, VPN, REMOTE }

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

    private var polling: Job? = null

    fun current(): Host? = hosts.value.firstOrNull { it.id == _selected.value } ?: hosts.value.firstOrNull()

    fun select(id: String) {
        _selected.value = id
        store.selectedId = id
        hosts.value.firstOrNull { it.id == id }?.let { poll(it) }
    }

    fun setProfile(id: String, p: Profile) = store.update(id) { it.copy(settings = it.settings.copy(profile = p)) }

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

    private fun poll(h: Host) = viewModelScope.launch {
        val password = store.password(h.id) ?: ""
        val reply = ConsoleClient(h.address, h.port, h.user, password, h.fingerprint).use { it.info() }
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
            if (i.os != h.os || i.gpu != h.gpu || i.av1 != h.av1 || i.virtualDisplay != h.virtualDisplay) {
                store.update(h.id) { it.copy(os = i.os, gpu = i.gpu, av1 = i.av1, virtualDisplay = i.virtualDisplay) }
            }
            // Pas encore d'image : l'aperçu d'Admin (jamais pendant une session).
            if (!store.imageFile(h.id).exists()) fetchPreview(h, password)
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

    companion object {
        fun reach(address: String): Reach {
            val p = address.split('.').mapNotNull { it.toIntOrNull() }
            if (p.size != 4) return if (address.startsWith("fd") || address.startsWith("fe80")) Reach.LOCAL else Reach.REMOTE
            return when {
                p[0] == 10 || (p[0] == 192 && p[1] == 168) || (p[0] == 172 && p[1] in 16..31) -> Reach.LOCAL
                p[0] == 100 && p[1] in 64..127 -> Reach.VPN
                else -> Reach.REMOTE
            }
        }
    }
}
