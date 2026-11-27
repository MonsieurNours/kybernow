package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.Mic
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.panel

/**
 * « Micro » de la barre (hôte, invité que l'hôte laisse parler, ami qui emprunte) : le libellé, le sous-titre
 * (« Envoyé », « Envoi… », « Coupé ») et l'infobulle du cœur (`MicUi`) ; un toucher l'allume ou le coupe. L'ordinateur
 * ne le prend pas : estompé, mais toujours touchable : la raison en avis (et en infobulle à la souris). Rien sans micro
 * sur l'appareil, ni chez un invité que l'hôte ne laisse pas parler.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun MicChip(ui: SessionUi) {
    if (!ui.micShown || !ui.mic.shown) return
    val s = LocalScale.current
    val mic = ui.mic
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
        tooltip = { PlainTooltip(containerColor = Kn.panel, contentColor = Kn.frost) { Body(mic.tooltip, size = 13f) } },
        state = rememberTooltipState(),
    ) {
        GlassButton(
            mic.label, { ui.actions.setMicrophone(!ui.micChoice) },
            Modifier.alpha(if (mic.refused) 0.45f else 1f).semantics { stateDescription = mic.tile },
            selected = mic.lit, detail = mic.tile, singleLine = true,
            leading = { KnIcon(if (mic.lit) KnIcons.mic else KnIcons.micOff, tint = if (mic.lit) Kn.spectre else Kn.frost2, size = 16f); Spacer(Modifier.width(s.dp(6f))) },
        )
    }
}

/** Barre fermée : la petite icône du micro, en haut à droite, tant qu'il est envoyé (docs/link-ui.md, « Indicateur »). */
@Composable
internal fun MicIndicator(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val label = ui.mic.label + ", " + ui.mic.tile
    Box(modifier.padding(s.dp(12f)).panel(999.dp).padding(s.dp(6f)).semantics { contentDescription = label }) {
        KnIcon(KnIcons.mic, tint = Kn.spectre, size = 14f)
    }
}
