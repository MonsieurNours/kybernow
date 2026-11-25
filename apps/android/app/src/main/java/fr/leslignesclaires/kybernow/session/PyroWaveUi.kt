package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.PyroWaveLink
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import kotlinx.coroutines.delay

// PyroWave en session (docs/link-ui.md, « PyroWave » ; maquettes de l'auteur) : la puce ambre de la barre et sa tuile
// au téléviseur, le bandeau du début avec « Passer en HEVC », et les lignes des statistiques.

/** Fond, bord et texte ambre de l'avertissement (ceux des maquettes). */
private val amberBg = Color(0xFF1E1A12)
private val amberLine = Color(0xFF4A3A1E)
private val amberText = Color(0xFFEAD7A8)
private val amberTitle = Color(0xFFF6E6BD)

/** L'image en cours est en PyroWave : le décodeur l'a dit (`onDecoder`, « PyroWave (Vulkan, …) »). */
@Composable
internal fun pyroActive(ui: SessionUi): Boolean {
    val decoder by ui.state.decoder.collectAsState()
    return decoder?.startsWith("PyroWave") == true
}

/** Par où passe l'image PyroWave : le relais si la route le dit, sinon le réseau de l'appareil au début. */
@Composable
internal fun pyroLink(ui: SessionUi): PyroWaveLink {
    val route by ui.state.route.collectAsState()
    return if (route?.viaRelay == true) PyroWaveLink.RELAY else ui.pyroLink
}

@Composable
private fun pyroTitle(link: PyroWaveLink): String =
    stringResource(if (link == PyroWaveLink.RELAY) R.string.pyrowave_away else R.string.pyrowave_wifi)

/** La puce ambre « PyroWave en Wi-Fi » / « PyroWave hors de chez vous » ; un toucher ouvre les statistiques. Rien par câble. */
@Composable
internal fun PyroChip(ui: SessionUi) {
    if (!pyroActive(ui)) return
    val link = pyroLink(ui)
    if (link == PyroWaveLink.CABLE) return
    val s = LocalScale.current
    val title = pyroTitle(link)
    Row(
        Modifier.clip(RoundedCornerShape(999.dp)).background(amberBg).border(BorderStroke(1.dp, amberLine), RoundedCornerShape(999.dp))
            .clickable { ui.statsOpen = !ui.statsOpen }.semantics { contentDescription = title }
            .padding(horizontal = s.dp(14f), vertical = s.dp(9f)),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        KnIcon(KnIcons.network, tint = Kn.warn, size = 14f)
        Spacer(Modifier.width(s.dp(6f)))
        Body(title, size = 13f, color = amberText, weight = 600)
    }
}

/** Au téléviseur : la même chose en tuile du menu. */
@Composable
internal fun PyroTile(ui: SessionUi) {
    if (!pyroActive(ui)) return
    val link = pyroLink(ui)
    if (link == PyroWaveLink.CABLE) return
    Tile(KnIcons.warning, pyroTitle(link), null, selected = true) { ui.statsOpen = !ui.statsOpen }
}

/**
 * Le bandeau du début : PyroWave choisi à la main, en Wi-Fi ou par le relais. « L'image peut saccader. », « Passer
 * en HEVC » (l'image repart en HEVC pour cette session seulement) et la croix ; parti tout seul après 8 s. Une fois
 * par session, à la première image PyroWave.
 */
@Composable
internal fun PyroBanner(ui: SessionUi) {
    val active = pyroActive(ui)
    val link = pyroLink(ui)
    LaunchedEffect(active, link) {
        if (active && ui.pyroChosen && link != PyroWaveLink.CABLE && !ui.pyroBannerDone) {
            ui.pyroBannerDone = true
            ui.pyroBanner = true
            delay(8_000)
            ui.pyroBanner = false
        }
    }
    if (!ui.pyroBanner) return
    val s = LocalScale.current
    Row(
        Modifier.padding(horizontal = s.dp(16f)).widthIn(max = s.dp(640f)).clip(RoundedCornerShape(14.dp)).background(amberBg)
            .border(BorderStroke(1.dp, amberLine), RoundedCornerShape(14.dp)).padding(s.dp(14f)),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(s.dp(12f)),
    ) {
        KnIcon(KnIcons.warning, tint = Kn.warn, size = 18f)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
            Body(pyroTitle(link), size = 14f, color = amberTitle, weight = 700)
            Body(stringResource(R.string.pyrowave_may_stutter), size = 14f, color = amberText)
        }
        GlassButton(stringResource(R.string.pyrowave_switch_hevc), { ui.actions.switchToHevc() })
        val close = stringResource(R.string.session_close)
        FocusCard({ ui.pyroBanner = false }, Modifier.semantics { contentDescription = close }) {
            KnIcon(KnIcons.close, tint = amberText, size = 16f, modifier = Modifier.padding(s.dp(8f)))
        }
    }
}

/**
 * Les lignes PyroWave des statistiques : « Images perdues » (envoyées par l'ordinateur, jamais montrées, en % des
 * images de la seconde) et, quand l'image reçue est plus petite que l'écran de la session, « Définition ramenée à
 * {l} × {h} pour tenir dans {n} Mbit/s. » (le débit mesuré).
 */
@Composable
internal fun PyroStats(ui: SessionUi, sec: Second?, video: Pair<Int, Int>?) {
    if (!pyroActive(ui)) return
    val locale = LocalConfiguration.current.locales[0]
    val lost = sec?.lostFrames ?: 0
    val total = (sec?.fps ?: 0) + lost
    val pct = if (total > 0) 100.0 * lost / total else 0.0
    Row(verticalAlignment = Alignment.CenterVertically) {
        Mono(stringResource(R.string.stats_lost_frames) + " ", size = 11f)
        Mono(String.format(locale, "%.1f %%", pct), size = 11f, color = if (lost > 0) amberText else Kn.frost)
    }
    val mode by ui.state.mode.collectAsState()
    val m = mode
    if (video != null && m != null && video.first < m.first && video.second < m.second) {
        Mono(stringResource(R.string.pyrowave_lowered, video.first, video.second, Math.round(sec?.mbps ?: 0.0).toInt()), size = 11f, color = amberText)
    }
}
