package fr.leslignesclaires.kybernow.ui.share

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.LocalScale

/**
 * « Auto », « Comme l'hôte », « 1080p · 60 Hz », « 720p · 30 Hz » (docs/link-ui.md, « Partage », « Qualité de
 * l'image ») : le nom d'une qualité demandée. La qualité reçue (« 1080p · 60 Hz ») est celle du cœur, pas celle-ci.
 */
@Composable
fun qualityLabel(quality: ShareQuality): String = stringResource(qualityText(quality))

@androidx.annotation.StringRes
internal fun qualityText(quality: ShareQuality): Int = when (quality) {
    ShareQuality.AUTO -> R.string.share_quality_auto
    ShareQuality.HOST -> R.string.share_quality_host
    ShareQuality.P1080_60 -> R.string.share_quality_1080p60
    ShareQuality.P720_30 -> R.string.share_quality_720p30
}

/**
 * Les quatre choix de la « Qualité de l'image », celui de [selected] allumé ; [focus] va sur celui-là (la
 * télécommande et la manette arrivent sur le choix en cours).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun QualityChoices(selected: ShareQuality, onSelect: (ShareQuality) -> Unit, modifier: Modifier = Modifier, focus: FocusRequester? = null) {
    val s = LocalScale.current
    FlowRow(modifier, horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        ShareQuality.entries.forEach { q ->
            GlassButton(qualityLabel(q), { onSelect(q) }, if (focus != null && q == selected) Modifier.focusRequester(focus) else Modifier, selected = q == selected)
        }
    }
}

/** « Qualité de l'image » et ses quatre choix, sous le prénom de « Rejoindre ». */
@Composable
fun QualityField(selected: ShareQuality, onSelect: (ShareQuality) -> Unit, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    Column(modifier, verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        Eyebrow(stringResource(R.string.share_quality_title))
        QualityChoices(selected, onSelect)
    }
}
