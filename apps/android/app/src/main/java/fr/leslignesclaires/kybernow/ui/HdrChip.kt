package fr.leslignesclaires.kybernow.ui

import android.view.Display
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.HdrScreen
import fr.leslignesclaires.kybernow.model.HdrState

/**
 * L'écran principal de l'appareil vu par le HDR, suivi tant que l'écran est affiché (téléviseur qui coupe le HDR,
 * autre sortie HDMI) : la puce « HDR » de l'accueil et l'interrupteur des réglages apparaissent ou disparaissent avec lui.
 */
@Composable
fun rememberHdrScreen(): HdrScreen {
    val context = LocalContext.current
    var screen by remember { mutableStateOf(HdrScreen.of(context)) }
    DisposableEffect(context) {
        val watch = HdrScreen.watch(context, Display.DEFAULT_DISPLAY, screen) { screen = it }
        onDispose { watch.close() }
    }
    return screen
}

/** L'aide de la puce : la raison quand le HDR n'est pas possible, sinon ce qu'il apporte (docs/link-ui.md, « HDR »). */
@Composable
fun hdrHelp(state: HdrState, settings: Boolean = false): String =
    state.reason ?: stringResource(if (settings) R.string.hdr_help_settings else R.string.hdr_help)

/**
 * La puce « HDR » (accueil, à côté du profil ; barre de session) : allumée ou éteinte (le choix de cet ordinateur),
 * inactive avec la raison du cœur dans son aide. Rien sur un écran SDR ([HdrState.shown] faux).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HdrChip(state: HdrState, choice: Boolean, onToggle: () -> Unit, modifier: Modifier = Modifier, icon: Boolean = false) {
    if (!state.shown) return
    val on = state.on(choice)
    val desc = stringResource(if (on) R.string.hdr_on else R.string.hdr_off)
    val help = hdrHelp(state)
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
        tooltip = { PlainTooltip(containerColor = Kn.panel, contentColor = Kn.frost) { Body(help, size = 13f) } },
        state = rememberTooltipState(),
    ) {
        GlassButton(
            stringResource(R.string.hdr), onToggle, modifier.semantics { stateDescription = desc },
            selected = on, enabled = state.available,
            leading = if (icon) ({ KnIcon(KnIcons.sparkle, tint = if (on) Kn.spectre else Kn.frost2, size = 16f); Spacer(Modifier.width(6.dp)) }) else null,
        )
    }
}
