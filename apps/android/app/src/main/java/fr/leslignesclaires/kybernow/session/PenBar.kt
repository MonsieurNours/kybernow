package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.PenMode
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale

/*
 * « Stylet : Pression / Souris » en session (docs/link-ui.md, « Stylet ») : sur les barres en tuiles d'Android, un
 * élément « Stylet » dont le sous-titre dit « Pression » ou « Souris » (les textes du cœur, `PenChoice`), qui bascule
 * d'un toucher. Seulement quand le cœur le montre : un stylet a servi dans cette session ou l'appareil en a un, et
 * l'ordinateur reçoit le stylet ; sinon rien. Le raccourci Ctrl+Alt+Maj+P fait la même chose, avec un avis bref.
 */

/** Barre du téléphone et de la tablette : « Stylet » et son choix, l'infobulle du raccourci à la souris. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun PenChip(ui: SessionUi) {
    val pen = ui.pen
    if (!pen.shown) return
    val s = LocalScale.current
    val shortcut = stringResource(R.string.pen_shortcut)
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
        tooltip = { PlainTooltip(containerColor = Kn.panel, contentColor = Kn.frost) { Body(shortcut, size = 13f) } },
        state = rememberTooltipState(),
    ) {
        GlassButton(
            pen.title, { ui.actions.togglePen() }, Modifier.semantics { stateDescription = pen.label },
            selected = pen.mode == PenMode.MOUSE, detail = pen.label, singleLine = true,
            leading = { KnIcon(KnIcons.pen, size = 16f); Spacer(Modifier.width(s.dp(6f))) },
        )
    }
}

/** Menu du téléviseur : la tuile « Stylet », « Pression » ou « Souris » en sous-titre. */
@Composable
internal fun PenTile(ui: SessionUi) {
    val pen = ui.pen
    if (!pen.shown) return
    Tile(KnIcons.pen, pen.title, pen.label, Modifier.semantics { stateDescription = pen.label }, selected = pen.mode == PenMode.MOUSE) { ui.actions.togglePen() }
}
