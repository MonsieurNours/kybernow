package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.displayCutout
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemGestures
import androidx.compose.foundation.layout.union
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathOperation
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.input.pointer.PointerInputScope
import androidx.compose.ui.input.pointer.PointerType
import androidx.compose.ui.input.pointer.changedToDownIgnoreConsumed
import androidx.compose.ui.input.pointer.changedToUpIgnoreConsumed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.sp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnFonts
import kotlin.math.atan2
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/**
 * La manette à l'écran, par-dessus l'image : tout l'écran prend les doigts (le reste de l'écran
 * ne sert plus de souris), chaque doigt va à la commande qu'il touche ([OnScreenPad]). Placée
 * sous les autres éléments de la session (barre, rangée du clavier, feuilles), qui gardent leurs
 * touchers. Hors des zones sûres : encoche et gestes du système (Retour sur les côtés, accueil en
 * bas, volet en haut), pour qu'un pouce ne déclenche pas un geste d'Android.
 *
 * Charte v2 : verre sombre (fond d'encre, filet de givre, contour d'encre pour rester lisible sur
 * une image claire), signal Spectre à l'appui ; le bouton appuyé s'allume et s'enfonce.
 */
@Composable
fun OnScreenPadLayer(ui: SessionUi) {
    val pad = ui.pad
    val density = LocalDensity.current
    val insets = WindowInsets.displayCutout.union(WindowInsets.systemGestures)
    val l = insets.getLeft(density, LayoutDirection.Ltr).toFloat()
    val t = insets.getTop(density).toFloat()
    val r = insets.getRight(density, LayoutDirection.Ltr).toFloat()
    val b = insets.getBottom(density).toFloat()
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val w = constraints.maxWidth.toFloat()
        val h = constraints.maxHeight.toFloat()
        val simple = pad.simple
        val geo = remember(w, h, l, t, r, b, ui.padSize, simple, density.density) { PadGeometry.compute(w, h, l, t, r, b, density.density, ui.padSize, simple) }
        SideEffect { pad.geometry = geo }
        val labels = padLabels(geo.u)
        val opacity = ui.padOpacity / 100f
        Canvas(Modifier.fillMaxSize().pointerInput(pad) { padTouches(pad) }) { drawPad(pad, geo, opacity, labels) }
        // Le petit bouton de disposition, pour les lecteurs d'écran (le toucher passe par le dessin).
        val sw = geo[PadControl.SWITCH]
        val label = stringResource(if (pad.simple) R.string.pad_switch_full else R.string.pad_switch_simple)
        Box(
            Modifier.offset { IntOffset((sw.left - sw.reach).roundToInt(), (sw.top - sw.reach).roundToInt()) }
                .size(with(density) { ((sw.hw + sw.reach) * 2).toDp() })
                .semantics {
                    contentDescription = label
                    role = Role.Button
                    onClick(label) { pad.onSwitch(); true }
                },
        )
    }
}

/** Les doigts (pas la souris, qui reste une souris) vers la manette ; tout relâché si la couche s'en va. */
private suspend fun PointerInputScope.padTouches(pad: OnScreenPad) {
    try {
        awaitPointerEventScope {
            while (true) {
                val e = awaitPointerEvent()
                for (c in e.changes) {
                    if (c.type == PointerType.Mouse) continue
                    val id = c.id.value
                    when {
                        c.changedToDownIgnoreConsumed() -> pad.down(id, c.position.x, c.position.y, c.uptimeMillis)
                        c.changedToUpIgnoreConsumed() -> pad.up(id, c.uptimeMillis)
                        c.pressed && c.positionChanged() -> pad.move(id, c.position.x, c.position.y)
                    }
                    c.consume()
                }
            }
        }
    } finally {
        pad.releaseAll()
    }
}

// MARK: - Dessin

private class PadLabels(val face: Map<Int, TextLayoutResult>, val shoulders: Map<PadControl, TextLayoutResult>)

@Composable
private fun padLabels(u: Float): PadLabels {
    val measurer: TextMeasurer = rememberTextMeasurer()
    val density = LocalDensity.current
    return remember(u, measurer, density) {
        // Le dessin a sa taille propre : la taille du texte d'Android ne la change pas.
        fun style(px: Float, weight: Int) = TextStyle(
            fontFamily = KnFonts.display,
            fontWeight = FontWeight(weight),
            fontSize = (px / (density.density * density.fontScale)).sp,
            color = Color.White,
        )
        val big = style(18 * u, 620)
        val small = style(12.5f * u, 600)
        PadLabels(
            mapOf(Pad.A to "A", Pad.B to "B", Pad.X to "X", Pad.Y to "Y").mapValues { measurer.measure(it.value, big) },
            mapOf(PadControl.LB to "LB", PadControl.RB to "RB", PadControl.LT to "LT", PadControl.RT to "RT").mapValues { measurer.measure(it.value, small) },
        )
    }
}

private fun DrawScope.drawPad(pad: OnScreenPad, g: PadGeometry, opacity: Float, labels: PadLabels) {
    val pressed = pad.pressed
    for (s in g.shapes(pad.simple)) {
        when (s.control) {
            PadControl.LEFT_STICK -> stick(s, pad.left, g, opacity)
            PadControl.RIGHT_STICK -> stick(s, pad.right, g, opacity)
            PadControl.DPAD -> dpad(s, pressed, g, opacity)
            PadControl.FACE -> face(pressed, g, opacity, labels)
            PadControl.LB -> bumper(s, Pad.L1 in pressed, labels.shoulders.getValue(PadControl.LB), g.u, opacity)
            PadControl.RB -> bumper(s, Pad.R1 in pressed, labels.shoulders.getValue(PadControl.RB), g.u, opacity)
            PadControl.LT -> trigger(s, pad.lt, labels.shoulders.getValue(PadControl.LT), g.u, opacity)
            PadControl.RT -> trigger(s, pad.rt, labels.shoulders.getValue(PadControl.RT), g.u, opacity)
            PadControl.VIEW -> small(s, Pad.BACK in pressed, g.u, opacity) { c, w -> viewGlyph(s, g.u, c, w) }
            PadControl.GUIDE -> small(s, Pad.GUIDE in pressed, g.u, opacity) { c, w -> guideGlyph(s, g.u, c, w) }
            PadControl.MENU -> small(s, Pad.START in pressed, g.u, opacity) { c, w -> menuGlyph(s, g.u, c, w) }
            PadControl.SWITCH -> small(s, pad.switchDown, g.u, opacity) { c, w -> switchGlyph(s, g.u, c, w, toFull = pad.simple) }
        }
    }
}

/** Appuyé, une commande reste bien visible même à faible opacité. */
private fun alphaOf(on: Boolean, opacity: Float) = if (on) max(opacity, 0.92f) else opacity

/**
 * Le verre d'une commande : halo Spectre (appuyée), contour d'encre, fond d'encre, filet de givre.
 * L'opacité éclaircit surtout le fond : le filet et les signes restent lisibles à 30 %.
 */
private fun DrawScope.glass(path: Path, on: Boolean, u: Float, opacity: Float, fill: Boolean = true) {
    val a = alphaOf(on, opacity)
    if (on) for (i in 3 downTo 1) drawPath(path, Kn.spectreFlat.copy(alpha = 0.16f * a / i), style = Stroke(i * 5f * u))
    drawPath(path, Kn.ink.copy(alpha = 0.5f * a), style = Stroke(3.5f * u))
    if (fill) drawPath(path, if (on) Kn.spectreFlat.copy(alpha = 0.7f * a) else Kn.ink2.copy(alpha = 0.78f * a))
    drawPath(path, if (on) Kn.spectre.copy(alpha = a) else Kn.frost.copy(alpha = 0.3f + 0.25f * a), style = Stroke(1.5f * u))
}

/** Lettres et signes : givre, à 55 % au moins, sur un liseré d'encre (lisibles sur une image claire). */
private fun labelColor(on: Boolean, opacity: Float) = if (on) Kn.neonCore.copy(alpha = alphaOf(true, opacity)) else Kn.frost.copy(alpha = 0.55f + 0.45f * opacity)

private fun haloColor(opacity: Float) = Kn.ink.copy(alpha = 0.35f + 0.3f * opacity)

private fun circle(cx: Float, cy: Float, radius: Float) = Path().apply { addOval(Rect(Offset(cx, cy), radius)) }

private fun rounded(left: Float, top: Float, right: Float, bottom: Float, radius: Float) =
    Path().apply { addRoundRect(RoundRect(left, top, right, bottom, CornerRadius(radius))) }

private fun DrawScope.label(t: TextLayoutResult, cx: Float, cy: Float, color: Color, u: Float, opacity: Float) =
    drawText(t, color, Offset(cx - t.size.width / 2f, cy - t.size.height / 2f), shadow = Shadow(haloColor(opacity), Offset.Zero, 4 * u))

/** Appuyée, la commande s'enfonce un peu. */
private inline fun DrawScope.pressedScale(on: Boolean, cx: Float, cy: Float, block: DrawScope.() -> Unit) {
    if (on) scale(0.93f, Offset(cx, cy)) { block() } else block()
}

private fun DrawScope.stick(s: PadShape, st: StickState, g: PadGeometry, opacity: Float) {
    val u = g.u
    val ring = circle(s.cx, s.cy, s.hw)
    glass(ring, false, u, opacity)
    val a = alphaOf(st.touched, opacity)
    if (st.touched) drawPath(ring, Kn.spectre.copy(alpha = 0.7f * a), style = Stroke(1.5f * u))
    // La direction poussée : un arc Spectre sur le bord, plus vif à mesure qu'on pousse.
    val m = min(1f, hypot(st.x, st.y))
    if (m > 0.2f) {
        val deg = Math.toDegrees(atan2(st.y, st.x).toDouble()).toFloat()
        val r = s.hw - 5 * u
        drawArc(
            Kn.spectre.copy(alpha = a * m), deg - 26f, 52f, false,
            topLeft = Offset(s.cx - r, s.cy - r), size = Size(r * 2, r * 2),
            style = Stroke(3.5f * u, cap = StrokeCap.Round),
        )
    }
    val kx = s.cx + st.x * g.knobTravel
    val ky = s.cy + st.y * g.knobTravel
    pressedScale(st.clicked, kx, ky) {
        val knob = circle(kx, ky, g.knobRadius)
        if (st.clicked) glass(knob, true, u, opacity)
        else {
            drawPath(knob, Kn.ink.copy(alpha = 0.45f * a), style = Stroke(3.5f * u))
            drawPath(knob, Kn.frost.copy(alpha = (if (st.touched) 0.3f else 0.2f) * a))
            drawPath(knob, (if (st.touched) Kn.spectre else Kn.frost).copy(alpha = 0.7f * a), style = Stroke(1.5f * u))
        }
        drawCircle(Kn.frost.copy(alpha = 0.3f * a), g.knobRadius * 0.55f, Offset(kx, ky), style = Stroke(1.2f * u))
    }
}

private fun DrawScope.dpad(s: PadShape, pressed: Set<Int>, g: PadGeometry, opacity: Float) {
    val u = g.u
    val arm = g.dpadArm
    val half = s.hw
    val corner = 7 * u
    val cross = Path().apply {
        op(
            rounded(s.cx - half, s.cy - arm, s.cx + half, s.cy + arm, corner),
            rounded(s.cx - arm, s.cy - half, s.cx + arm, s.cy + half, corner),
            PathOperation.Union,
        )
    }
    glass(cross, false, u, opacity)
    val arms = mapOf(
        Pad.UP to Rect(s.cx - arm, s.cy - half, s.cx + arm, s.cy - arm * 0.2f),
        Pad.DOWN to Rect(s.cx - arm, s.cy + arm * 0.2f, s.cx + arm, s.cy + half),
        Pad.LEFT to Rect(s.cx - half, s.cy - arm, s.cx - arm * 0.2f, s.cy + arm),
        Pad.RIGHT to Rect(s.cx + arm * 0.2f, s.cy - arm, s.cx + half, s.cy + arm),
    )
    for ((b, rect) in arms) {
        val on = b in pressed
        if (on) {
            val path = rounded(rect.left, rect.top, rect.right, rect.bottom, corner)
            pressedScale(true, s.cx, s.cy) { clipPath(cross) { glass(path, true, u, opacity) } }
        }
        // La flèche de la branche.
        val c = rect.center
        val (dx, dy) = when (b) {
            Pad.UP -> 0f to -1f
            Pad.DOWN -> 0f to 1f
            Pad.LEFT -> -1f to 0f
            else -> 1f to 0f
        }
        val tip = Offset(c.x + dx * 6 * u, c.y + dy * 6 * u)
        val back = Offset(c.x - dx * 3 * u, c.y - dy * 3 * u)
        val side = Offset(-dy * 6 * u, dx * 6 * u)
        val arrow = Path().apply {
            moveTo(tip.x, tip.y)
            lineTo(back.x + side.x, back.y + side.y)
            lineTo(back.x - side.x, back.y - side.y)
            close()
        }
        drawPath(arrow, haloColor(opacity), style = Stroke(2.6f * u, join = StrokeJoin.Round))
        drawPath(arrow, labelColor(on, opacity))
    }
}

private fun DrawScope.face(pressed: Set<Int>, g: PadGeometry, opacity: Float, labels: PadLabels) {
    for (b in listOf(Pad.Y, Pad.X, Pad.B, Pad.A)) {
        val (cx, cy) = g.faceCenter(b)
        val on = b in pressed
        pressedScale(on, cx, cy) {
            glass(circle(cx, cy, g.faceRadius), on, g.u, opacity)
            label(labels.face.getValue(b), cx, cy, labelColor(on, opacity), g.u, opacity)
        }
    }
}

private fun DrawScope.bumper(s: PadShape, on: Boolean, text: TextLayoutResult, u: Float, opacity: Float) {
    pressedScale(on, s.cx, s.cy) {
        glass(rounded(s.left, s.top, s.right, s.bottom, 14 * u), on, u, opacity)
        label(text, s.cx, s.cy, labelColor(on, opacity), u, opacity)
    }
}

/** Une gâchette : la jauge Spectre descend avec la valeur (de haut en bas, comme on la tire). */
private fun DrawScope.trigger(s: PadShape, value: Float, text: TextLayoutResult, u: Float, opacity: Float) {
    val on = value > 0f
    val a = alphaOf(on, opacity)
    pressedScale(on, s.cx, s.cy) {
        val pill = rounded(s.left, s.top, s.right, s.bottom, s.hw)
        if (on) for (i in 3 downTo 1) drawPath(pill, Kn.spectreFlat.copy(alpha = 0.16f * a / i), style = Stroke(i * 5f * u))
        glass(pill, false, u, opacity)
        if (on) {
            clipPath(pill) { drawRect(Kn.spectreFlat.copy(alpha = 0.62f * a), Offset(s.left, s.top), Size(s.hw * 2, s.hh * 2 * value)) }
            drawPath(pill, Kn.spectre.copy(alpha = a), style = Stroke(1.5f * u))
        }
        label(text, s.cx, s.bottom - 16 * u, labelColor(on, opacity), u, opacity)
    }
}

/** Affichage, Guide, Menu, le bouton de disposition : un petit bouton et son signe. */
private inline fun DrawScope.small(s: PadShape, on: Boolean, u: Float, opacity: Float, glyph: DrawScope.(Color, Float) -> Unit) {
    pressedScale(on, s.cx, s.cy) {
        val path = if (s.round) circle(s.cx, s.cy, s.hw) else rounded(s.left, s.top, s.right, s.bottom, s.hh)
        glass(path, on, u, opacity)
        // Le signe, sur son liseré d'encre.
        glyph(haloColor(opacity), 3.4f * u)
        glyph(labelColor(on, opacity), 1.8f * u)
    }
}

/** Affichage : deux fenêtres l'une sur l'autre ([w] : épaisseur du trait). */
private fun DrawScope.viewGlyph(s: PadShape, u: Float, c: Color, w: Float) {
    val d = 9 * u
    val st = Stroke(w)
    drawRoundRect(c, Offset(s.cx - d * 0.85f, s.cy - d * 0.75f), Size(d, d * 0.8f), CornerRadius(2 * u), style = st)
    drawRoundRect(c, Offset(s.cx - d * 0.15f, s.cy - d * 0.05f), Size(d, d * 0.8f), CornerRadius(2 * u), style = st)
}

/** Menu : trois traits. */
private fun DrawScope.menuGlyph(s: PadShape, u: Float, c: Color, w: Float) {
    for (i in -1..1) drawLine(c, Offset(s.cx - 7 * u, s.cy + i * 4.5f * u), Offset(s.cx + 7 * u, s.cy + i * 4.5f * u), w, StrokeCap.Round)
}

/** Guide : un anneau et son centre. */
private fun DrawScope.guideGlyph(s: PadShape, u: Float, c: Color, w: Float) {
    drawCircle(c, 9 * u, Offset(s.cx, s.cy), style = Stroke(w))
    drawCircle(c, 2.1f * u + w / 2, Offset(s.cx, s.cy))
}

/** Le bouton de disposition : « − » vers la manette simple, « + » vers la complète. */
private fun DrawScope.switchGlyph(s: PadShape, u: Float, c: Color, w: Float, toFull: Boolean) {
    drawLine(c, Offset(s.cx - 6 * u, s.cy), Offset(s.cx + 6 * u, s.cy), w, StrokeCap.Round)
    if (toFull) drawLine(c, Offset(s.cx, s.cy - 6 * u), Offset(s.cx, s.cy + 6 * u), w, StrokeCap.Round)
}
