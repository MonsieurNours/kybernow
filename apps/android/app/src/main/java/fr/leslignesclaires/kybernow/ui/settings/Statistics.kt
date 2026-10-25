package fr.leslignesclaires.kybernow.ui.settings

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.LastSession
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnFonts
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.home.duration
import fr.leslignesclaires.kybernow.ui.panel
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min

private const val NB = " "

/**
 * « Dernière session » (docs/link-ui.md, « Statistiques »), sur le modèle de la carte de l'app Mac :
 * la médiane en grand, la règle des images (repères 160, 120 et 60 Hz, bande de la médiane au
 * 95e centile), « 95 % des images sous … », la durée et le moment, ce qui a été transmis.
 */
@Composable
fun LastSessionCard(last: LastSession, now: Long) {
    val s = LocalScale.current
    val locale = LocalConfiguration.current.locales[0]
    fun ms(v: Double) = String.format(locale, "%.1f", v)
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(18f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Eyebrow(stringResource(R.string.last_title), Modifier.weight(1f))
            Body(listOf(lasted(last.seconds), moment(last.at, now)).joinToString(" · "), size = 13f, color = Kn.muted, maxLines = 1)
        }
        Row(verticalAlignment = Alignment.Bottom) {
            Mono(ms(last.median) + NB + "ms", size = 30f, color = Kn.frost, weight = 500)
            Spacer(Modifier.width(s.dp(10f)))
            Mono(stringResource(R.string.last_median), Modifier.padding(bottom = s.dp(5f)), size = 12f, color = Kn.muted)
        }
        FrameRuler(last.median, last.p95)
        Body(stringResource(R.string.last_p95, ms(last.p95)), size = 14f, color = Kn.frost2)
        if (last.width > 0 && last.height > 0) {
            val sent = listOfNotNull(
                "${last.width}$NB×$NB${last.height}",
                last.refresh.takeIf { it > 0 }?.let { "$it${NB}Hz" },
                last.codec?.let(::codecLabel),
                last.mbps.takeIf { it > 0 }?.let { "$it${NB}Mbit/s" },
            ).joinToString(" · ")
            Body(sent, size = 14f, color = Kn.frost2)
        }
        Body(stringResource(R.string.last_help), size = 13f, color = Kn.muted)
    }
}

fun codecLabel(codec: String) = when (codec) {
    "hevc" -> "HEVC"
    "av1" -> "AV1"
    else -> "H.264"
}

/** « 42 min », « 1 h 12 », « 25 s ». */
@Composable
private fun lasted(seconds: Long) = if (seconds < 60) stringResource(R.string.last_seconds, seconds.toInt()) else duration(seconds)

/** « aujourd'hui », « hier », « 12 octobre », « 12 octobre 2025 ». */
@Composable
private fun moment(at: Long, now: Long): String {
    val locale = LocalConfiguration.current.locales[0]
    val zone = ZoneId.systemDefault()
    val day = Instant.ofEpochMilli(at).atZone(zone).toLocalDate()
    val today = Instant.ofEpochMilli(now).atZone(zone).toLocalDate()
    return when {
        day == today -> stringResource(R.string.last_today)
        day == today.minusDays(1) -> stringResource(R.string.last_yesterday)
        day.year == today.year -> day.format(DateTimeFormatter.ofPattern("d MMMM", locale))
        else -> day.format(DateTimeFormatter.ofPattern("d MMMM yyyy", locale))
    }
}

/**
 * La règle des images : la latence lue contre la durée d'une image à 160, 120 et 60 Hz (6,25 ;
 * 8,33 ; 16,67 ms), de 0 à 20 ms (plus si la mesure dépasse) ; la bande va de la médiane au
 * 95e centile, le trait est la médiane.
 */
@Composable
fun FrameRuler(median: Double, p95: Double?, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    val tm = rememberTextMeasurer()
    val locale = LocalConfiguration.current.locales[0]
    val scale = max(20.0, ceil(max(median, p95 ?: 0.0) * 1.25 / 10) * 10)
    val label = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(9f), color = Kn.muted)
    val a11y = stringResource(R.string.last_median) + " " + String.format(locale, "%.1f", median) + " ms"
    Canvas(modifier.fillMaxWidth().height(s.dp(52f)).semantics { contentDescription = a11y }) {
        val w = size.width
        fun x(ms: Double) = (min(ms, scale) / scale * w).toFloat()
        val base = s.dp(26f).toPx()
        val tick = s.dp(9f).toPx()
        val gap = s.dp(3f).toPx()
        drawRect(Kn.frost.copy(alpha = 0.16f), topLeft = Offset(0f, base - 1.dp.toPx()), size = Size(w, 2.dp.toPx()))
        p95?.let {
            val left = x(median)
            val width = max(4.dp.toPx(), x(it) - left)
            drawRoundRect(Kn.spectre.copy(alpha = 0.2f), topLeft = Offset(left, base - 5.dp.toPx()), size = Size(width, 10.dp.toPx()), cornerRadius = CornerRadius(3.dp.toPx()))
            drawRoundRect(Kn.spectre.copy(alpha = 0.5f), topLeft = Offset(left, base - 5.dp.toPx()), size = Size(width, 10.dp.toPx()), cornerRadius = CornerRadius(3.dp.toPx()), style = Stroke(1.dp.toPx()))
        }
        // Repères d'une image à 160, 120 et 60 Hz : 160 au-dessus, les autres dessous ; une
        // étiquette seulement si elle a la place.
        val ticks = listOf(Triple(1000 / 160.0, "160${NB}Hz", true), Triple(1000 / 120.0, "120${NB}Hz", false), Triple(1000 / 60.0, "60${NB}Hz", false))
        ticks.forEach { (ms, text, up) ->
            val tx = x(ms)
            drawLine(Kn.muted.copy(alpha = 0.7f), Offset(tx, base - tick), Offset(tx, base + tick), strokeWidth = 1.dp.toPx())
            val crowded = ticks.any { it.third == up && it.first > ms && x(it.first) - tx < 38.dp.toPx() }
            if (!crowded) {
                val m = tm.measure(text, label)
                val lx = (tx - m.size.width / 2f).coerceIn(0f, w - m.size.width)
                val ly = if (up) base - tick - gap - m.size.height else base + tick + gap
                drawText(m, topLeft = Offset(lx, ly))
            }
        }
        // Les bouts de la règle, au-dessus : 0 et l'échelle.
        val zero = tm.measure("0", label)
        drawText(zero, topLeft = Offset(0f, base - tick - gap - zero.size.height))
        val end = tm.measure(String.format(locale, "%.0f", scale) + NB + "ms", label)
        drawText(end, topLeft = Offset(w - end.size.width, base - tick - gap - end.size.height))
        // La médiane.
        val mx = x(median)
        drawRoundRect(Kn.neonCore, topLeft = Offset(mx - 1.5.dp.toPx(), base - 12.dp.toPx()), size = Size(3.dp.toPx(), 24.dp.toPx()), cornerRadius = CornerRadius(2.dp.toPx()))
    }
}
