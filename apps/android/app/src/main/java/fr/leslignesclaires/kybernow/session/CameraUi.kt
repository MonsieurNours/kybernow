package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale

/**
 * « Caméra » dans la barre (docs/link-ui.md, « Caméra de l'appareil ») : seulement si l'ordinateur la propose ;
 * estompée éteinte, rouge avec un point quand elle part ; le menu l'allume ou l'éteint, puis liste les caméras de
 * l'appareil (« Caméra avant », « Caméra arrière »), celle choisie cochée.
 */
@Composable
internal fun CameraButton(ui: SessionUi) {
    val offered by ui.state.cameraOffered.collectAsState()
    val state by ui.state.cameraState.collectAsState()
    if (!offered) return
    var open by remember { mutableStateOf(false) }
    Box {
        GlassButton(
            stringResource(if (state == 1) R.string.camera_starting else R.string.camera),
            { open = true },
            selected = state == 2 || state == 3,
            leading = {
                if (state == 2) Box(Modifier.padding(end = 4.dp).clip(RoundedCornerShape(99.dp)).background(Kn.danger).padding(3.dp))
                KnIcon(if (state == 0) KnIcons.cameraOff else KnIcons.camera, tint = if (state == 2) Kn.danger else Kn.frost, size = 18f)
            },
        )
        DropdownMenu(open, { open = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(if (state == 0) R.string.camera_turn_on else R.string.camera_turn_off)) },
                onClick = { open = false; ui.actions.toggleCamera() },
            )
            val current = ui.actions.currentCamera()
            ui.actions.cameras().forEach { (id, label) ->
                DropdownMenuItem(
                    text = { Text(if (id == current) "✓  $label" else label) },
                    onClick = { open = false; ui.actions.pickCamera(id) },
                )
            }
        }
    }
}

/** Le voyant : barre fermée, tant que la caméra part, une petite capsule « ● Caméra » en haut à gauche. */
@Composable
internal fun CameraIndicator(ui: SessionUi, modifier: Modifier) {
    val state by ui.state.cameraState.collectAsState()
    if (state != 2 || ui.barOpen) return
    val s = LocalScale.current
    Body(
        stringResource(R.string.camera_indicator),
        modifier.padding(s.dp(10f)).clip(RoundedCornerShape(999.dp)).background(Color.Black.copy(alpha = 0.55f))
            .padding(horizontal = s.dp(8f), vertical = s.dp(3f)),
        size = 11f, weight = 600, color = Color(0xFFFFC9C4),
    )
}
