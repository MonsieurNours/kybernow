package fr.leslignesclaires.kybernow.session

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.core.RecordingState
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.LocalMotion
import fr.leslignesclaires.kybernow.ui.LocalScale
import kotlinx.coroutines.delay

// Enregistrement de la session (docs/link-ui.md, « Enregistrement de la session ») : la Station écrit le fichier, l'app
// le demande et le montre. Les textes viennent tous du cœur ([RecordingState.texts]). Une Station qui n'enregistre
// pas ne dit rien : ni bouton, ni pastille, ni tuile.

/** L'icône « rond plein » de l'enregistrement. */
private val recordIcon: ImageVector = ImageVector.Builder("record", 24.dp, 24.dp, 24f, 24f).apply {
    addPath(PathParser().parsePathString("M12 5a7 7 0 1 0 0 14a7 7 0 1 0 0-14z").toNodes(), fill = SolidColor(Color.White))
}.build()

/** L'état montré : seulement si la Station enregistre, pour l'hôte et l'invité (jamais pour l'ami qui emprunte ni une console). */
@Composable
private fun shownRecording(ui: SessionUi): RecordingState? {
    val r by ui.state.recording.collectAsState()
    return r?.takeIf { it.available && !ui.borrower && ui.console == null }
}

/** Le temps de l'horloge de la session, chaque seconde tant que l'enregistrement dure. */
@Composable
private fun tick(ui: SessionUi, active: Boolean): Long {
    val now by produceState(ui.state.clockMs(), active) {
        while (active) {
            value = ui.state.clockMs()
            delay(1000 - (value % 1000).coerceIn(0, 999))
        }
    }
    return now
}

/** Le rond rouge qui bat doucement (une pulsation par seconde), fixe quand le système demande moins d'animations. */
@Composable
private fun PulsingDot(color: Color) {
    val s = LocalScale.current
    val a = if (LocalMotion.current) {
        val pulse = rememberInfiniteTransition(label = "enregistrement")
        pulse.animateFloat(1f, 0.35f, infiniteRepeatable(tween(500), RepeatMode.Reverse), label = "battement").value
    } else 1f
    Box(Modifier.size(s.dp(8f)).alpha(a).clip(CircleShape).background(color))
}

/**
 * La pastille rouge en haut de l'image, toujours visible pendant l'enregistrement (barre fermée comprise) : l'hôte
 * « Enregistrement · 02:14 », l'invité « Enregistrement » (sans action ; « {nom} enregistre la session. » pour les
 * lecteurs d'écran). Elle ne prend pas les touchers : l'hôte arrête par le menu de la session.
 */
@Composable
internal fun RecordingPill(ui: SessionUi) {
    val r = shownRecording(ui) ?: return
    val ended by ui.state.ended.collectAsState()
    if (!r.active || ended != null) return
    val s = LocalScale.current
    val now = tick(ui, true)
    val text = if (ui.guest) r.texts.guestPill else r.pill(now)
    val description = if (ui.guest) r.texts.guestTooltip else text
    Row(
        Modifier.clip(RoundedCornerShape(999.dp)).background(Kn.danger).padding(horizontal = s.dp(14f), vertical = s.dp(7f))
            .semantics { contentDescription = description },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        PulsingDot(Color.White)
        Spacer(Modifier.width(s.dp(8f)))
        Body(text, size = 13f, weight = 600, color = Color.White)
    }
}

/**
 * Téléphone et tablette, barre de l'hôte, après « Prêter l'ordinateur… » : « Enregistrer » (rond plein), pendant
 * « Arrêter l'enregistrement ». Coupé dans Admin : estompé mais sélectionnable, il donne la raison en avis.
 */
@Composable
internal fun RecordingItems(ui: SessionUi) {
    val r = shownRecording(ui) ?: return
    if (ui.guest) return
    val s = LocalScale.current
    val usable = r.allowed || r.active
    GlassButton(
        r.action, { ui.actions.record() }, Modifier.alpha(if (usable) 1f else 0.5f),
        selected = r.active, singleLine = true,
        leading = { KnIcon(recordIcon, tint = Kn.danger, size = 14f); Spacer(Modifier.width(s.dp(6f))) },
    )
}

/**
 * Menu en tuiles (Android TV), hôte : « Enregistrer » / « Sur l'ordinateur » ; pendant « Arrêter l'enregistrement » /
 * « 02:14 », en rouge. Coupé dans Admin : estompée, elle donne la raison en avis. Invité : la tuile « Enregistrement »,
 * inactive, seulement pendant l'enregistrement.
 */
@Composable
internal fun RecordingTile(ui: SessionUi) {
    val r = shownRecording(ui) ?: return
    if (ui.guest) {
        if (r.active) Tile(recordIcon, r.texts.guestPill, null, danger = true, dimmed = true) { }
        return
    }
    val now = tick(ui, r.active)
    val detail = if (r.active) RecordingState.clock(r.seconds(now)) else r.texts.onComputer
    Tile(recordIcon, r.action, detail, danger = r.active, dimmed = !r.allowed && !r.active) { ui.actions.record() }
}
