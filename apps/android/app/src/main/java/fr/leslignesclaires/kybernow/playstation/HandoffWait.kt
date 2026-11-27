package fr.leslignesclaires.kybernow.playstation

import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * L'attente d'un envoi (« Recevoir d'un autre appareil »), sans mémoire libérée en cours d'usage (AND-09) : la
 * poignée native n'est libérée ([stop]) qu'une fois l'appel bloquant [next] revenu, même quand l'écran se ferme
 * pendant l'attente (Retour, « Nouveau code »). Une nouvelle attente n'écoute qu'après la fin de la précédente
 * ([previous]) : les deux voudraient le même port.
 */
object HandoffWait {
    fun launch(
        scope: CoroutineScope,
        previous: Job?,
        start: () -> Long,
        next: (Long) -> String?,
        stop: (Long) -> Unit,
        onStart: (Long) -> Unit,
        /** Vrai : l'attente est finie (reçu, code expiré). */
        onEvent: (JSONObject) -> Boolean,
        io: CoroutineDispatcher = Dispatchers.IO,
    ): Job = scope.launch {
        previous?.join()
        val h = withContext(io) { start() }
        try {
            onStart(h)
            while (isActive && h != 0L) {
                // Bloque jusqu'à son délai : annulée, la coroutine n'en sort qu'à son retour.
                val e = withContext(io) { next(h) } ?: continue
                val o = runCatching { JSONObject(e) }.getOrNull() ?: continue
                if (onEvent(o)) break
            }
        } finally {
            if (h != 0L) withContext(NonCancellable + io) { stop(h) }
        }
    }
}
