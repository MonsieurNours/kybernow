package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Native
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.Mono
import kotlin.math.max
import kotlin.math.roundToLong

/*
 * « Débit » des Statistiques (docs/link-ui.md, « Statistiques » ; apps/core/README.md, « Débit visé dans les
 * Statistiques ») : la courbe des 30 dernières secondes avec, au niveau du débit demandé à l'ordinateur, un trait du
 * débit visé ; dessous, la ligne « Débit visé » (“Target bitrate”) et le texte du cœur : « 42 / 60 Mbit/s · baissé,
 * réseau saturé », « 60 Mbit/s · Auto », « 80 Mbit/s · réglé à la main »… Rien du débit visé sans `target`.
 */

/** Le libellé du cœur ; sans bibliothèque native, celui de l'app (même texte). */
@Composable
internal fun targetBitrateLabel(): String {
    val fallback = stringResource(R.string.stats_target_bitrate)
    return remember { runCatching { Native.targetBitrateLabel() }.getOrNull() } ?: fallback
}

/**
 * Le texte du cœur (« 42 / 60 Mbit/s · baissé, réseau saturé ») ; sans bibliothèque native ou pour une raison inconnue,
 * les seuls Mbit/s (« 42 / 60 Mbit/s » quand il est sous celui du profil).
 */
internal fun targetBitrateText(t: TargetBitrate): String =
    runCatching { Native.targetBitrateText(t.bitrate, t.profile, t.reason) }.getOrNull()
        ?: run {
            fun m(v: Long) = (v / 1_000_000.0).roundToLong()
            if (t.profile > t.bitrate) "${m(t.bitrate)} / ${m(t.profile)} Mbit/s" else "${m(t.bitrate)} Mbit/s"
        }

/** Sous « Débit » : la courbe (avec le trait du débit visé), puis la ligne « Débit visé ». */
@Composable
internal fun BitrateStats(history: List<BitratePoint>, target: TargetBitrate?) {
    if (history.size >= 2) BitrateGraph(history)
    target?.let { t ->
        val label = targetBitrateLabel()
        val text = remember(t) { targetBitrateText(t) }
        Mono("$label $text", size = 11f)
    }
}

/**
 * La courbe du débit reçu sur [HISTORY_SECONDS] secondes, la plus récente à droite, et le trait du débit visé (tirets,
 * seconde par seconde comme la courbe ; absent les secondes sans débit visé).
 */
@Composable
internal fun BitrateGraph(history: List<BitratePoint>) {
    val s = LocalScale.current
    val a11y = stringResource(R.string.settings_bitrate)
    Canvas(Modifier.fillMaxWidth().height(s.dp(40f)).semantics { contentDescription = a11y }) {
        val w = size.width
        val h = size.height
        val top = max(1.0, history.maxOf { max(it.mbps, it.target ?: 0.0) }) * 1.15
        val step = w / (HISTORY_SECONDS - 1)
        val first = HISTORY_SECONDS - history.size
        fun x(i: Int) = (first + i) * step
        fun y(v: Double) = (h - (v / top) * h).toFloat()
        drawLine(Kn.frost.copy(alpha = 0.16f), Offset(0f, h - 0.5.dp.toPx()), Offset(w, h - 0.5.dp.toPx()), strokeWidth = 1.dp.toPx())
        val curve = Path()
        history.forEachIndexed { i, p -> if (i == 0) curve.moveTo(x(i), y(p.mbps)) else curve.lineTo(x(i), y(p.mbps)) }
        drawPath(curve, Kn.spectre, style = Stroke(1.5.dp.toPx()))
        // Le débit visé : un trait par seconde, en marches (il change d'un coup).
        val dash = PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 3.dp.toPx()))
        val target = Path()
        var open = false
        history.forEachIndexed { i, p ->
            val t = p.target
            if (t == null) {
                open = false
                return@forEachIndexed
            }
            val ty = y(t)
            if (!open) target.moveTo(x(i), ty) else target.lineTo(x(i), ty)
            target.lineTo(if (i == history.lastIndex) w else x(i + 1), ty)
            open = true
        }
        drawPath(target, Kn.frost.copy(alpha = 0.7f), style = Stroke(1.dp.toPx(), pathEffect = dash))
    }
}
