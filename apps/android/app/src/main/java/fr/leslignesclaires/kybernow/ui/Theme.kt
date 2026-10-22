package fr.leslignesclaires.kybernow.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.FormFactor

/** Couleurs de la charte v2 (brand/v2/tokens/kybernow.css). */
object Kn {
    val ink = Color(0xFF05070A)
    val ink2 = Color(0xFF0B0F16)
    val panel = Color(0xFF0E131C)
    val frost = Color(0xFFE3EDF0)
    val frost2 = Color(0xFFB8C6CC)
    val muted = Color(0xFF8C9BA2)
    val line = Color(0x1FE3EDF0)
    val spectre = Color(0xFF9A8CFF)
    val spectreFlat = Color(0xFF6D5BFF)
    val neonCore = Color(0xFFF4F1FF)
    val warn = Color(0xFFFFB86B)
    val danger = Color(0xFFFF5A6E)
}

@OptIn(ExperimentalTextApi::class)
private fun archivo(weight: Int, width: Float) = Font(
    R.font.archivo,
    weight = FontWeight(weight),
    variationSettings = FontVariation.Settings(FontVariation.weight(weight), FontVariation.width(width)),
)

@OptIn(ExperimentalTextApi::class)
private fun martian(weight: Int) = Font(
    R.font.martian_mono,
    weight = FontWeight(weight),
    variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
)

/** Archivo largeur 100 (texte), largeur 125 (titres), Martian Mono (données). */
object KnFonts {
    val text = FontFamily(archivo(400, 100f), archivo(500, 100f), archivo(600, 100f), archivo(700, 100f))
    val display = FontFamily(archivo(420, 125f), archivo(600, 125f), archivo(720, 125f))
    val mono = FontFamily(martian(400), martian(500), martian(600))
}

/** Échelle de l'interface : le téléviseur se lit à trois mètres. */
data class KnScale(val tv: Boolean, val unit: Float) {
    fun dp(v: Float): Dp = (v * unit).dp
    fun sp(v: Float): TextUnit = (v * unit).sp
}

val LocalScale = staticCompositionLocalOf { KnScale(false, 1f) }
val LocalForm = staticCompositionLocalOf { FormFactor.PHONE }

private val scheme: ColorScheme = darkColorScheme(
    primary = Kn.spectre,
    onPrimary = Kn.ink,
    background = Kn.ink,
    onBackground = Kn.frost,
    surface = Kn.panel,
    onSurface = Kn.frost,
    surfaceVariant = Kn.panel,
    onSurfaceVariant = Kn.frost2,
    outline = Kn.line,
)

@Composable
fun KnTheme(form: FormFactor, content: @Composable () -> Unit) {
    val scale = when (form) {
        FormFactor.TV -> KnScale(true, 1.0f)
        FormFactor.TABLET -> KnScale(false, 1.05f)
        FormFactor.PHONE -> KnScale(false, 1f)
    }
    MaterialTheme(colorScheme = scheme, typography = MaterialTheme.typography) {
        CompositionLocalProvider(LocalScale provides scale, LocalForm provides form, content = content)
    }
}

// MARK: - Textes

@Composable
fun Eyebrow(text: String, modifier: Modifier = Modifier, color: Color = Kn.muted) {
    val s = LocalScale.current
    Text(text.uppercase(), modifier, color = color, style = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(11f), letterSpacing = s.sp(2f)))
}

@Composable
fun Mono(text: String, modifier: Modifier = Modifier, size: Float = 12f, color: Color = Kn.frost2, weight: Int = 400, maxLines: Int = Int.MAX_VALUE) {
    val s = LocalScale.current
    Text(text, modifier, color = color, maxLines = maxLines, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis, style = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(size), fontWeight = FontWeight(weight)))
}

@Composable
fun Body(text: String, modifier: Modifier = Modifier, size: Float = 15f, color: Color = Kn.frost, weight: Int = 400, maxLines: Int = Int.MAX_VALUE) {
    val s = LocalScale.current
    Text(text, modifier, color = color, maxLines = maxLines, style = TextStyle(fontFamily = KnFonts.text, fontSize = s.sp(size), fontWeight = FontWeight(weight), lineHeight = s.sp(size * 1.4f)))
}

@Composable
fun Title(text: String, modifier: Modifier = Modifier, size: Float = 26f, weight: Int = 720) {
    val s = LocalScale.current
    Text(text, modifier, color = Kn.frost, maxLines = 2, style = TextStyle(fontFamily = KnFonts.display, fontSize = s.sp(size), fontWeight = FontWeight(weight), letterSpacing = s.sp(-size / 40f), lineHeight = s.sp(size * 1.0f)))
}

/**
 * Un titre sur une seule ligne qui rapetisse pour tenir (nom d'un ordinateur,
 * « PC-DE-MATTHIEU ») : jamais coupé au milieu d'un mot ni sur deux lignes.
 */
@Composable
fun FitTitle(text: String, modifier: Modifier = Modifier, size: Float = 26f, min: Float = 18f, weight: Int = 720) {
    val s = LocalScale.current
    androidx.compose.foundation.text.BasicText(
        text,
        modifier,
        style = TextStyle(color = Kn.frost, fontFamily = KnFonts.display, fontWeight = FontWeight(weight), letterSpacing = s.sp(-size / 40f), lineHeight = s.sp(size * 1.05f)),
        maxLines = 1,
        autoSize = androidx.compose.foundation.text.TextAutoSize.StepBased(minFontSize = s.sp(min), maxFontSize = s.sp(size), stepSize = s.sp(2f)),
    )
}

// MARK: - Focus et néon

/**
 * Le focus de la charte : l'élément grossit (× 1,06 au téléviseur), se
 * soulève et s'entoure du néon Spectre ; au toucher, un éclair au clic.
 */
@Composable
fun Modifier.neon(interaction: MutableInteractionSource, shape: Shape = RoundedCornerShape(14.dp), strong: Boolean = false): Modifier {
    val focused by interaction.collectIsFocusedAsState()
    val pressed by interaction.collectIsPressedAsState()
    val tv = LocalScale.current.tv
    val grow by animateFloatAsState(if (focused && tv) 1.06f else if (pressed) 0.98f else 1f, tween(160), label = "focus")
    val glow by animateFloatAsState(if (focused || (strong && pressed)) 1f else 0f, tween(160), label = "neon")
    return this
        .scale(grow)
        .drawBehind {
            if (glow > 0f) {
                val r = 18.dp.toPx()
                for (i in 3 downTo 1) {
                    val spread = i * 5.dp.toPx() * glow
                    drawRoundRect(
                        Kn.spectreFlat.copy(alpha = 0.16f * glow / i),
                        topLeft = androidx.compose.ui.geometry.Offset(-spread, -spread),
                        size = androidx.compose.ui.geometry.Size(size.width + spread * 2, size.height + spread * 2),
                        cornerRadius = CornerRadius(r + spread),
                    )
                }
                drawRoundRect(Kn.spectre.copy(alpha = glow), cornerRadius = CornerRadius(r), style = Stroke(2.dp.toPx() * glow))
            }
        }
}

/** Bouton principal (néon allumé au focus). */
@Composable
fun PrimaryButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
    val s = LocalScale.current
    val i = remember { MutableInteractionSource() }
    val focused by i.collectIsFocusedAsState()
    Box(
        modifier
            .neon(i, strong = true)
            .clip(RoundedCornerShape(14.dp))
            .background(if (!enabled) Kn.panel else if (focused || !s.tv) Kn.neonCore else Kn.frost)
            .clickable(interactionSource = i, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = s.dp(28f), vertical = s.dp(15f)),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, color = if (enabled) Kn.ink else Kn.muted, style = TextStyle(fontFamily = KnFonts.text, fontSize = s.sp(17f), fontWeight = FontWeight(650)))
    }
}

/** Bouton secondaire, en verre. */
@Composable
fun GlassButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, selected: Boolean = false, enabled: Boolean = true, leading: (@Composable RowScope.() -> Unit)? = null) {
    val s = LocalScale.current
    val i = remember { MutableInteractionSource() }
    Row(
        modifier
            .neon(i, RoundedCornerShape(999.dp))
            .clip(RoundedCornerShape(999.dp))
            .background(if (selected) Kn.spectre.copy(alpha = 0.18f) else Kn.frost.copy(alpha = 0.06f))
            .border(BorderStroke(1.dp, if (selected) Kn.spectre else Kn.frost.copy(alpha = 0.16f)), RoundedCornerShape(999.dp))
            .selectable(selected = selected, interactionSource = i, indication = null, enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = s.dp(18f), vertical = s.dp(10f)),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        leading?.invoke(this)
        Text(text, color = if (!enabled) Kn.muted else if (selected) Kn.frost else Kn.frost2, style = TextStyle(fontFamily = KnFonts.text, fontSize = s.sp(15f), fontWeight = FontWeight(if (selected) 600 else 500)))
    }
}

/** Carte du panneau (verre sombre). */
fun Modifier.panel(radius: Dp = 18.dp): Modifier = this
    .clip(RoundedCornerShape(radius))
    .background(Kn.panel.copy(alpha = 0.86f))
    .border(BorderStroke(1.dp, Kn.line), RoundedCornerShape(radius))

/** Une surface qui prend le focus et s'allume (cartes, tuiles). */
@Composable
fun FocusCard(onClick: () -> Unit, modifier: Modifier = Modifier, shape: Shape = RoundedCornerShape(16.dp), onLongClick: (() -> Unit)? = null, content: @Composable (focused: Boolean) -> Unit) {
    val i = remember { MutableInteractionSource() }
    val focused by i.collectIsFocusedAsState()
    Box(
        modifier
            .neon(i, shape)
            .clip(shape)
            .then(
                if (onLongClick != null) Modifier.combinedClickableCompat(i, onClick, onLongClick)
                else Modifier.clickable(interactionSource = i, indication = null, role = Role.Button, onClick = onClick),
            ),
    ) { content(focused) }
}

private fun Modifier.combinedClickableCompat(i: MutableInteractionSource, onClick: () -> Unit, onLongClick: () -> Unit) =
    this.combinedClickable(interactionSource = i, indication = null, role = Role.Button, onClick = onClick, onLongClick = onLongClick)

/** Rend un élément focalisable sans action (lecture au téléviseur). */
fun Modifier.readable(): Modifier = this.focusable()

/** Nombre mis en forme dans la langue de l'interface (observée par Compose). */
@Composable
fun fmt(pattern: String, vararg args: Any?): String =
    String.format(androidx.compose.ui.platform.LocalConfiguration.current.locales[0], pattern, *args)
