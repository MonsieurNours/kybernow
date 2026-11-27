package fr.leslignesclaires.kybernow.session

import android.app.Activity
import android.content.ComponentName
import android.util.Log
import android.view.WindowManager

/**
 * Les touches que le système garde pour lui pendant une session (la touche ⊞ / Meta, Alt+Tab et les raccourcis du
 * système), demandées pour la fenêtre de session tant qu'elle a le focus, et rendues dès qu'elle le perd. Deux moyens,
 * essayés l'un et l'autre, sans rien casser là où ils manquent :
 *
 * - **Samsung DeX** (et One UI au clavier) : `SemWindowManager.requestMetaKeyEvent(ComponentName, Boolean)`, l'API de
 *   Samsung que les clients de bureau à distance appellent par réflexion ; elle donne la touche Meta et Alt+Tab à
 *   l'activité au premier plan.
 * - **Android, capture du clavier** : `WindowManager.LayoutParams.setHasKeyboardCapture` (nommée d'abord
 *   `setKeyboardCaptureEnabled`) avec la permission `android.permission.CAPTURE_KEYBOARD` (protection « normal », donnée
 *   à l'installation ; déclarée dans le manifeste), sur les versions qui l'ont (Android 16 QPR2 et plus récentes, derrière
 *   l'indicateur `request_key_capture_api`) ; la fenêtre reçoit alors les touches avant les raccourcis du système.
 *
 * Ce qui ne passe par aucun des deux (Android d'avant, constructeurs sans API) reste dans le menu « Touches » de la barre
 * (« Touche ⊞ seule », « Alt + Tab »…), qui envoie la combinaison à l'ordinateur. Le raccourci Ctrl + Alt + Maj + M
 * ouvre toujours le menu de la session.
 */
object KeyCapture {
    /** Ce qui a pris les touches (pour le journal et les essais). */
    data class Result(val samsung: Boolean, val platform: Boolean)

    @Volatile private var logged = false

    /** Demande (ou rend, [on] faux) les touches du système pour la fenêtre de [activity]. */
    fun set(activity: Activity, on: Boolean): Result {
        val r = Result(samsung(activity.componentName, on), platform(activity, on))
        if (on && !logged) {
            logged = true
            Log.i("KyberNow", "clavier : touches du système demandées (Samsung ${if (r.samsung) "oui" else "non"}, Android ${if (r.platform) "oui" else "non"})")
        }
        return r
    }

    /** Samsung : `SemWindowManager.getInstance().requestMetaKeyEvent(composant, on)` ; faux sans l'API. */
    internal fun samsung(component: ComponentName, on: Boolean): Boolean = runCatching {
        val cls = Class.forName("com.samsung.android.view.SemWindowManager")
        val manager = cls.getMethod("getInstance").invoke(null) ?: return false
        cls.getMethod("requestMetaKeyEvent", ComponentName::class.java, Boolean::class.javaPrimitiveType).invoke(manager, component, on)
        true
    }.getOrDefault(false)

    /** La méthode de capture des paramètres de fenêtre, si cette version d'Android l'a. */
    internal fun captureMethod(): java.lang.reflect.Method? =
        listOf("setHasKeyboardCapture", "setKeyboardCaptureEnabled").firstNotNullOfOrNull { name ->
            runCatching { WindowManager.LayoutParams::class.java.getMethod(name, Boolean::class.javaPrimitiveType) }.getOrNull()
        }

    /** Android : la capture du clavier posée sur la fenêtre ; faux sans l'API (ou refusée). */
    internal fun platform(activity: Activity, on: Boolean): Boolean {
        val m = captureMethod() ?: return false
        return runCatching {
            val window = activity.window
            val lp = WindowManager.LayoutParams().apply { copyFrom(window.attributes) }
            m.invoke(lp, on)
            window.attributes = lp
            true
        }.getOrDefault(false)
    }
}
