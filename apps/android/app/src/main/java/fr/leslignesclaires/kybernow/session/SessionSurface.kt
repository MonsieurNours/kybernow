package fr.leslignesclaires.kybernow.session

import android.os.Build
import android.view.Gravity
import android.view.Surface
import android.view.SurfaceView
import android.view.View
import android.widget.FrameLayout
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * Ce que les deux activités de session (ordinateur, console) font de la même façon avec leur image (AND-31) : la
 * surface calée au plus grand dans son cadre, aux proportions de l'image, et la fréquence de l'écran demandée.
 */
object SessionSurface {
    /** L'image de [vw] × [vh], centrée et aussi grande que le cadre le permet. */
    fun fit(view: SurfaceView, vw: Int, vh: Int) {
        val root = view.parent as? View ?: return
        val (w, h) = root.width to root.height
        if (w == 0 || h == 0 || vw == 0 || vh == 0) return
        val k = min(w.toDouble() / vw, h.toDouble() / vh)
        val lp = view.layoutParams as FrameLayout.LayoutParams
        lp.width = (vw * k).roundToInt()
        lp.height = (vh * k).roundToInt()
        lp.gravity = Gravity.CENTER
        view.layoutParams = lp
    }

    /** La fréquence de la session pour cette surface (l'écran s'y cale quand il le peut). */
    fun frameRate(s: Surface, refresh: Int) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            s.setFrameRate(refresh.toFloat(), Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE, Surface.CHANGE_FRAME_RATE_ALWAYS)
        } else {
            s.setFrameRate(refresh.toFloat(), Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE)
        }
    }
}
