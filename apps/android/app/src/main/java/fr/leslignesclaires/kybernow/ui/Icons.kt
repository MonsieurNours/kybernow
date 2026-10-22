package fr.leslignesclaires.kybernow.ui

import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.dp

/** Icônes au trait (grille de 24, trait de 1,6), dans l'esprit de Phosphor (charte v2). */
object KnIcons {
    private fun icon(name: String, vararg paths: String): ImageVector {
        val b = ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f)
        for (d in paths) {
            b.addPath(
                pathData = PathParser().parsePathString(d).toNodes(),
                stroke = SolidColor(Color.White),
                strokeLineWidth = 1.6f,
                strokeLineCap = StrokeCap.Round,
                strokeLineJoin = StrokeJoin.Round,
            )
        }
        return b.build()
    }

    val settings = icon("settings", "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6z", "M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1")
    val gamepad = icon("gamepad", "M6 9h12a3 3 0 0 1 3 3l-1 5a2 2 0 0 1-3.5.8L14.5 15h-5l-2 2.8A2 2 0 0 1 4 17l-1-5a3 3 0 0 1 3-3z", "M8 11v3M6.5 12.5h3", "M15.5 12h.01M17.5 13.5h.01")
    val plus = icon("plus", "M12 5v14M5 12h14")
    val back = icon("back", "M15 5l-7 7 7 7")
    val play = icon("play", "M7 4l12 8-12 8z")
    val sound = icon("sound", "M4 9h4l5-4v14l-5-4H4z", "M16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11")
    val screen = icon("screen", "M3 5h18v12H3z", "M8 21h8M12 17v4")
    val keyboard = icon("keyboard", "M2 6h20v12H2z", "M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10")
    val keys = icon("keys", "M3 4h18v16H3z", "M7 9h2M11 9h2M15 9h2M7 13h10")
    val mouse = icon("mouse", "M7 7a5 5 0 0 1 10 0v10a5 5 0 0 1-10 0z", "M12 6v3")
    val remote = icon("remote", "M7 2h10v20H7z", "M12 6a2 2 0 1 0 0 4a2 2 0 1 0 0-4z", "M10 14h4M10 17h4")
    val touch = icon("touch", "M9 11V5a2 2 0 0 1 4 0v6", "M13 10a2 2 0 0 1 4 0v2a2 2 0 0 1 2 2v2a6 6 0 0 1-6 6h-1a6 6 0 0 1-5-3l-2-4a1.8 1.8 0 0 1 3-2l1 1")
    val exit = icon("exit", "M10 4H5v16h5", "M15 8l4 4-4 4M19 12H9")
    val refresh = icon("refresh", "M20 12a8 8 0 1 1-2.3-5.6", "M20 4v5h-5")
    val trash = icon("trash", "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3")
    val check = icon("check", "M5 12l5 5 9-10")
    val close = icon("close", "M6 6l12 12M18 6L6 18")
    val qr = icon("qr", "M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4z", "M14 14h2v2h-2zM18 18h2v2h-2zM14 18h2M18 14h2")
    val network = icon("network", "M2 9a15 15 0 0 1 20 0", "M5 12.5a10 10 0 0 1 14 0", "M8.5 16a5 5 0 0 1 7 0", "M12 19.5h.01")
    val code = icon("code", "M4 7h16v10H4z", "M8 12h.01M12 12h.01M16 12h.01")
    val pin = icon("pin", "M12 21s-7-6.5-7-12a7 7 0 0 1 14 0c0 5.5-7 12-7 12z", "M12 7a2 2 0 1 0 0 4a2 2 0 1 0 0-4z")
    val power = icon("power", "M12 3v8", "M6.5 6.5a8 8 0 1 0 11 0")
    val info = icon("info", "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z", "M12 11v5M12 8h.01")
    val pulse = icon("pulse", "M3 12h4l2-6 4 12 2-6h6")
    val chevron = icon("chevron", "M9 5l7 7-7 7")
}

@Composable
fun KnIcon(icon: ImageVector, modifier: Modifier = Modifier, tint: Color = Kn.frost, size: Float = 22f) {
    val s = LocalScale.current
    Icon(icon, contentDescription = null, modifier = modifier.size(s.dp(size)), tint = tint)
}
