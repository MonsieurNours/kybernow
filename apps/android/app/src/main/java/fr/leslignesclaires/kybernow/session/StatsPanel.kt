package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Core
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.panel
import kotlin.math.max

/*
 * Le panneau « Statistiques » d'une session d'ordinateur (docs/link-ui.md, barre de session, « Statistiques », et
 * « Statistiques › En session : trajet d'une image » ; TRANS-38) : le titre, la latence en grand « de la capture à
 * votre écran », la croix ; le trajet d'une image (six étapes, « — » sans mesure) ; « Latence » et « Débit » sur
 * 30 secondes, le repère « 1 image », « Débit visé » ; « Pertes · 30 s » et le total de la session ; enfin « Chemin »,
 * « Réseau », « Image », « Mesures », « Son ». Avant la première image, une phrase seule. Les mesures viennent du cœur
 * (`Second`, chaque seconde) ; les textes de link-ui sont ceux de strings.py (« Débit visé », le chemin : du cœur).
 */

/** Les six étapes du trajet d'une image, leur lieu et leur aide. */
private val steps = listOf(
    Triple(R.string.stats_step_capture, 0, R.string.stats_help_capture),
    Triple(R.string.stats_step_encode, 0, R.string.stats_help_encode),
    Triple(R.string.stats_step_send, 0, R.string.stats_help_send),
    Triple(R.string.stats_step_network, 1, R.string.stats_help_network),
    Triple(R.string.stats_step_decode, 2, R.string.stats_help_decode),
    Triple(R.string.stats_step_display, 2, R.string.stats_help_display),
)

@Composable
internal fun SessionStatsPanel(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val sec by ui.state.second.collectAsState()
    val history by ui.state.history.collectAsState()
    val totals by ui.state.totals.collectAsState()
    val decoder by ui.state.decoder.collectAsState()
    val audio by ui.state.audioOutput.collectAsState()
    val video by ui.state.videoSize.collectAsState()
    val mode by ui.state.mode.collectAsState()
    val route by ui.state.route.collectAsState()
    val hdr by ui.state.hdr.collectAsState()
    val cameraState by ui.state.cameraState.collectAsState()
    val locale = LocalConfiguration.current.locales[0]
    fun ms(v: Double?) = v?.let { String.format(locale, "%.1f ms", it) } ?: "—"
    val frameMs = mode?.third?.takeIf { it > 0 }?.let { 1000.0 / it }
    Column(modifier.panel(18.dp).padding(s.dp(16f)).widthIn(max = s.dp(380f)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        // Titre, raccourci, croix « Fermer les statistiques ».
        Row(verticalAlignment = Alignment.CenterVertically) {
            Eyebrow(stringResource(R.string.session_stats), Modifier.weight(1f))
            if (!ui.tv) Mono(stringResource(R.string.stats_shortcut), size = 10f, color = Kn.muted)
            if (!ui.tv) BarIcon(KnIcons.close, stringResource(R.string.stats_close)) { ui.statsOpen = false }
        }
        val first = sec
        if (first == null) {
            Body(stringResource(R.string.stats_waiting), size = 14f, color = Kn.frost2)
            return@Column
        }
        Row(verticalAlignment = Alignment.Bottom) {
            Body(ms(first.latencyMs), size = 26f, weight = 600)
            Spacer(Modifier.width(s.dp(8f)))
            Mono(stringResource(R.string.stats_from_capture), size = 11f, color = Kn.frost2)
        }
        // Le trajet d'une image.
        Eyebrow(stringResource(R.string.stats_path_title))
        FramePath(ui, first.chain, frameMs)
        // Latence et débit sur 30 secondes.
        Eyebrow(stringResource(R.string.stats_latency))
        LatencyGraph(history, frameMs)
        Eyebrow(stringResource(R.string.stats_bitrate))
        val bitrates by ui.state.bitrateHistory.collectAsState()
        BitrateStats(bitrates, first.target)
        // Pertes sur 30 secondes, puis depuis le début de la session.
        Eyebrow(stringResource(R.string.stats_losses_30s))
        StatRow(stringResource(R.string.stats_lost_frames), history.sumOf { it.lostFrames }.toString())
        StatRow(stringResource(R.string.stats_lost_packets), history.sumOf { it.lostPackets }.toString())
        StatRow(stringResource(R.string.stats_keyframes), history.sumOf { it.keyframes }.toString())
        Mono(stringResource(R.string.stats_session_losses, totals.first, totals.second), size = 11f, color = Kn.frost2)
        PyroStats(ui, first)
        // Chemin, réseau, image, mesures, son.
        route?.let { StatRow(stringResource(R.string.stats_row_path), Core.routeLabel(it.transport)) }
        StatRow(stringResource(R.string.stats_row_network), first.rttMs?.let { stringResource(R.string.stats_round_trip, String.format(locale, "%.1f", it)) } ?: "—")
        video?.let { (w, h) ->
            val parts = listOfNotNull("$w × $h", decoder?.takeIf { it.isNotBlank() }, if (hdr) "HDR" else null, stringResource(R.string.session_fps, first.fps))
            StatRow(stringResource(R.string.stats_row_image), parts.joinToString(" · "))
        }
        if (first.fps > 0) StatRow(stringResource(R.string.stats_row_measures), stringResource(R.string.stats_measured, (100 * first.measured / first.fps).coerceIn(0, 100)))
        audio?.let { (_, buffer) -> StatRow(stringResource(R.string.stats_row_sound), stringResource(R.string.stats_sound_buffer, buffer)) }
        // La caméra de l'appareil, allumée (docs/webcam.md).
        if (cameraState != 0) ui.actions.cameraStatsLine()?.let { StatRow(stringResource(R.string.camera), it) }
    }
}

/** Une ligne « libellé … valeur ». */
@Composable
private fun StatRow(label: String, value: String) {
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Mono(label, Modifier.weight(0.4f), size = 11f, color = Kn.muted)
        Mono(value, Modifier.weight(0.6f), size = 11f, color = Kn.frost)
    }
}

/**
 * Le trajet d'une image : au téléphone et à la tablette, une cascade (chaque étape commence où la précédente finit,
 * sur une échelle commune, un pointillé à la durée d'une image), le lieu de chaque groupe, la valeur à droite et l'aide
 * à l'appui long ; au téléviseur, les six valeurs regroupées par lieu, avec le total de chacun, sans aide.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FramePath(ui: SessionUi, chain: DoubleArray?, frameMs: Double?) {
    val s = LocalScale.current
    val locale = LocalConfiguration.current.locales[0]
    fun ms(v: Double?) = v?.takeIf { it >= 0 }?.let { String.format(locale, "%.1f", it) } ?: "—"
    val device = stringResource(if (ui.tv) R.string.stats_this_tv else R.string.stats_this_device)
    val places = listOf(stringResource(R.string.stats_on_computer), stringResource(R.string.stats_place_network), stringResource(R.string.stats_on_device, device))
    val values = (0 until 6).map { chain?.getOrNull(it)?.takeIf { v -> v >= 0 } }
    if (ui.tv) {
        // Une rangée lisible de loin : un lieu, son total, ses étapes.
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
            places.forEachIndexed { p, place ->
                val mine = steps.indices.filter { steps[it].second == p }
                val total = mine.mapNotNull { values[it] }.takeIf { it.size == mine.size }?.sum()
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(2f))) {
                    Mono(place, size = 10f, color = Kn.muted)
                    Body("${ms(total)} ms", size = 17f, weight = 600)
                    mine.forEach { i -> Mono("${stringResource(steps[i].first)} ${ms(values[i])}", size = 11f, color = Kn.frost2) }
                }
            }
        }
        return
    }
    val known = values.filterNotNull()
    val scale = max(known.sum(), frameMs ?: 0.0).takeIf { it > 0 } ?: 1.0
    var start = 0.0
    var lastPlace = -1
    steps.forEachIndexed { i, (name, place, help) ->
        if (place != lastPlace) {
            Mono(places[place], size = 10f, color = Kn.muted)
            lastPlace = place
        }
        val v = values[i]
        val from = start
        start += v ?: 0.0
        val helpText = stringResource(help, device)
        TooltipBox(
            positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
            tooltip = { PlainTooltip(containerColor = Kn.panel, contentColor = Kn.frost) { Body(helpText, size = 13f) } },
            state = rememberTooltipState(),
        ) {
            Row(Modifier.fillMaxWidth().semantics { contentDescription = helpText }, verticalAlignment = Alignment.CenterVertically) {
                Mono(stringResource(name), Modifier.weight(0.3f), size = 11f, color = Kn.frost)
                Canvas(Modifier.weight(0.5f).height(s.dp(8f))) {
                    val w = size.width
                    // Le pointillé à la durée d'une image.
                    frameMs?.let { f ->
                        val x = (f / scale * w).toFloat().coerceAtMost(w)
                        drawLine(Kn.frost.copy(alpha = 0.35f), Offset(x, 0f), Offset(x, size.height), 1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(2.dp.toPx(), 2.dp.toPx())))
                    }
                    if (v != null && v > 0) {
                        val x0 = (from / scale * w).toFloat()
                        val x1 = ((from + v) / scale * w).toFloat().coerceAtLeast(x0 + 1.dp.toPx())
                        drawLine(Kn.spectre, Offset(x0, size.height / 2), Offset(x1, size.height / 2), size.height * 0.7f)
                    }
                }
                Mono("${ms(v)} ms", Modifier.weight(0.2f).padding(start = s.dp(6f)), size = 11f, color = Kn.frost)
            }
        }
    }
}

/** La latence des 30 dernières secondes, la plus récente à droite, et le repère « 1 image » à la durée d'une image. */
@Composable
private fun LatencyGraph(history: List<Second>, frameMs: Double?) {
    val s = LocalScale.current
    val points = history.map { it.latencyMs }
    if (points.count { it != null } < 2) return
    val a11y = stringResource(R.string.stats_latency)
    val oneFrame = stringResource(R.string.stats_one_frame)
    Column {
        Canvas(Modifier.fillMaxWidth().height(s.dp(40f)).semantics { contentDescription = a11y }) {
            val w = size.width
            val h = size.height
            val top = max(points.filterNotNull().maxOrNull() ?: 1.0, frameMs ?: 0.0) * 1.15
            val step = w / (HISTORY_SECONDS - 1)
            val first = HISTORY_SECONDS - points.size
            fun y(v: Double) = (h - (v / top) * h).toFloat()
            frameMs?.let { f -> drawLine(Kn.frost.copy(alpha = 0.35f), Offset(0f, y(f)), Offset(w, y(f)), 1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(4.dp.toPx(), 3.dp.toPx()))) }
            val curve = Path()
            var open = false
            points.forEachIndexed { i, v ->
                if (v == null) { open = false; return@forEachIndexed }
                val x = (first + i) * step
                if (!open) curve.moveTo(x, y(v)) else curve.lineTo(x, y(v))
                open = true
            }
            drawPath(curve, Kn.spectre, style = Stroke(1.5.dp.toPx()))
        }
        if (frameMs != null) Mono(oneFrame + " · " + String.format(LocalConfiguration.current.locales[0], "%.1f ms", frameMs), size = 10f, color = Kn.muted)
    }
}
