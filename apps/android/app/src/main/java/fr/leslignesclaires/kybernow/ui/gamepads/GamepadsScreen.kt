package fr.leslignesclaires.kybernow.ui.gamepads

import android.view.KeyEvent
import android.view.MotionEvent
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Pad
import fr.leslignesclaires.kybernow.model.Gamepad
import fr.leslignesclaires.kybernow.model.Gamepads
import fr.leslignesclaires.kybernow.model.PadAxes
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.home.motorsLabel
import fr.leslignesclaires.kybernow.ui.panel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow

/** État en direct d'une manette pour l'écran de test (nourri par l'activité). */
data class PadLive(val deviceId: Int, val buttons: Set<Int>, val axes: PadAxes)

object PadMonitor {
    @Volatile var active = false
    private val _live = MutableStateFlow<PadLive?>(null)
    val live = _live.asStateFlow()

    fun key(e: KeyEvent) {
        if (!active || !Gamepads.isGamepadEvent(e)) return
        val b = Gamepads.button(e.keyCode).takeIf { it >= 0 } ?: Gamepads.digitalTrigger(e.keyCode).let { if (it >= 0) 100 + it else return }
        val cur = _live.value?.takeIf { it.deviceId == e.deviceId } ?: PadLive(e.deviceId, emptySet(), PadAxes.REST)
        _live.value = cur.copy(buttons = if (e.action == KeyEvent.ACTION_DOWN) cur.buttons + b else cur.buttons - b)
    }

    fun motion(e: MotionEvent) {
        if (!active) return
        val cur = _live.value?.takeIf { it.deviceId == e.deviceId } ?: PadLive(e.deviceId, emptySet(), PadAxes.REST)
        _live.value = cur.copy(axes = PadAxes.read(e))
    }
}

@Composable
fun GamepadsScreen(onBack: () -> Unit) {
    val app = KyberNowApp.instance
    val pads by app.gamepads.pads.collectAsState()
    val live by PadMonitor.live.collectAsState()
    // Les vraies manettes : la manette à l'écran n'existe qu'en session.
    GamepadsContent(pads.filterNot { it.onScreen }, live, app.gamepads::test, onBack)
}

/** `test(manette, moteur)` : vibration d'essai (0 gauche, 1 droit, 2 les deux). */
@Composable
fun GamepadsContent(pads: List<Gamepad>, live: PadLive?, test: (Int, Int) -> Unit, onBack: () -> Unit) {
    val s = LocalScale.current
    var chosen by remember { mutableStateOf<Int?>(null) }
    val current = pads.firstOrNull { it.deviceId == (live?.deviceId ?: chosen) } ?: pads.firstOrNull()

    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        BoxWithConstraints(Modifier.fillMaxSize().padding(s.dp(if (s.tv) 56f else 20f))) {
            val wide = maxWidth > 760.dp
            val list = @Composable {
                Column(verticalArrangement = Arrangement.spacedBy(s.dp(12f))) {
                    if (pads.isEmpty()) Body(stringResource(R.string.gamepads_none), color = Kn.frost2, size = 15f)
                    pads.forEach { p -> PadCard(p, p.deviceId == current?.deviceId) { chosen = p.deviceId } }
                    Body(stringResource(R.string.gamepads_four), color = Kn.muted, size = 13f)
                }
            }
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(s.dp(16f))) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    FocusCard(onClick = onBack) {
                        Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(10f))) { KnIcon(KnIcons.back, size = 20f) }
                    }
                    Spacer(Modifier.width(s.dp(14f)))
                    Title(stringResource(R.string.gamepads_title), size = if (s.tv) 40f else 26f)
                }
                if (wide) {
                    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(28f))) {
                        Box(Modifier.weight(0.42f)) { list() }
                        Box(Modifier.weight(0.58f)) { current?.let { LivePanel(it, live?.takeIf { l -> l.deviceId == it.deviceId }, test) } }
                    }
                } else {
                    list()
                    current?.let { LivePanel(it, live?.takeIf { l -> l.deviceId == it.deviceId }, test) }
                }
            }
        }
    }
}

@Composable
private fun PadCard(p: Gamepad, selected: Boolean, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, Modifier.fillMaxWidth()) {
        Row(
            Modifier.panel().background(if (selected) Kn.spectre.copy(alpha = 0.12f) else Kn.panel.copy(alpha = 0f)).fillMaxWidth().padding(s.dp(16f)),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                Modifier.size(s.dp(44f)).clip(RoundedCornerShape(10.dp)).background(if (p.slot >= 0) Kn.spectreFlat else Kn.frost.copy(alpha = 0.1f)),
                contentAlignment = Alignment.Center,
            ) { Mono(if (p.slot >= 0) stringResource(R.string.gamepads_player, p.slot + 1) else "…", size = 15f, color = Kn.neonCore, weight = 600) }
            Spacer(Modifier.width(s.dp(14f)))
            Column(Modifier.weight(1f)) {
                Body(p.name, size = 17f, weight = 600, maxLines = 1)
                Mono(
                    listOfNotNull(
                        stringResource(if (p.builtIn) R.string.gamepads_builtin else R.string.gamepads_external),
                        p.battery?.let { "$it %" },
                        if (p.slot < 0) stringResource(R.string.gamepads_waiting) else null,
                    ).joinToString(" · "),
                    size = 11f,
                )
            }
            Box(
                Modifier.border(BorderStroke(1.dp, if (p.motors > 0) Kn.spectre else Kn.line), RoundedCornerShape(999.dp)).padding(horizontal = s.dp(10f), vertical = s.dp(5f)),
            ) { Mono(motorsLabel(p.motors), size = 11f, color = Kn.frost) }
        }
    }
}

@Composable
private fun LivePanel(p: Gamepad, live: PadLive?, test: (Int, Int) -> Unit) {
    val s = LocalScale.current
    val b = live?.buttons ?: emptySet()
    val a = live?.axes ?: PadAxes.REST
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(22f)), verticalArrangement = Arrangement.spacedBy(s.dp(18f))) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Body(p.name, size = 19f, weight = 650, maxLines = 1, modifier = Modifier.weight(1f))
            Mono(stringResource(R.string.gamepads_press), size = 11f, color = Kn.muted)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly, verticalAlignment = Alignment.CenterVertically) {
            Stick(a.lx, a.ly, Pad.LEFT_STICK in b)
            Face(b, p)
            Stick(a.rx, a.ry, Pad.RIGHT_STICK in b)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(s.dp(20f))) {
            Trigger("L2", maxOf(a.l2, if (100 + Pad.L2 in b) 32767 else 0), Modifier.weight(1f))
            Trigger("R2", maxOf(a.r2, if (100 + Pad.R2 in b) 32767 else 0), Modifier.weight(1f))
        }
        Mono(
            listOf(Pad.L1 to "L1", Pad.R1 to "R1", Pad.BACK to "Select", Pad.START to "Start", Pad.GUIDE to "Guide", Pad.UP to "↑", Pad.DOWN to "↓", Pad.LEFT to "←", Pad.RIGHT to "→")
                .joinToString("  ") { (k, l) -> if (k in b || (k == Pad.UP && a.hatY < 0) || (k == Pad.DOWN && a.hatY > 0) || (k == Pad.LEFT && a.hatX < 0) || (k == Pad.RIGHT && a.hatX > 0)) "[$l]" else l },
            size = 13f, color = Kn.frost,
        )
        Spacer(Modifier.height(s.dp(4f)))
        Eyebrow(stringResource(R.string.gamepads_test_rumble))
        if (p.motors == 0) {
            Body(stringResource(R.string.gamepads_no_rumble), size = 13f, color = Kn.muted)
        } else {
            Row(horizontalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                if (p.motors >= 2) {
                    GlassButton(stringResource(R.string.gamepads_left_low), { test(p.deviceId, 0) })
                    GlassButton(stringResource(R.string.gamepads_right_high), { test(p.deviceId, 1) })
                }
                GlassButton(stringResource(R.string.gamepads_both), { test(p.deviceId, 2) })
            }
            if (p.motors == 1) Body(stringResource(R.string.gamepads_one_motor_help), size = 13f, color = Kn.muted)
        }
    }
}

@Composable
private fun Stick(x: Int, y: Int, pressed: Boolean) {
    val s = LocalScale.current
    val r = s.dp(56f)
    Box(Modifier.size(r * 2).clip(CircleShape).border(BorderStroke(2.dp, if (pressed) Kn.spectre else Kn.frost.copy(alpha = 0.25f)), CircleShape)) {
        val moved = x != 0 || y != 0
        Box(
            Modifier.align(Alignment.Center)
                .offset(r * (x / 32767f) * 0.8f, r * (y / 32767f) * 0.8f)
                .size(s.dp(22f)).clip(CircleShape).background(if (moved || pressed) Kn.spectre else Kn.frost.copy(alpha = 0.5f)),
        )
    }
}

@Composable
private fun Face(b: Set<Int>, p: Gamepad) {
    val s = LocalScale.current
    val labels = if (p.sony) listOf("△", "□", "○", "✕") else if (p.nintendo) listOf("X", "Y", "A", "B") else listOf("Y", "X", "B", "A")
    @Composable
    fun dot(k: Int, l: String) = Box(
        Modifier.size(s.dp(34f)).clip(CircleShape).background(if (k in b) Kn.spectre else Kn.frost.copy(alpha = 0.06f)).border(BorderStroke(1.5.dp, Kn.frost.copy(alpha = 0.3f)), CircleShape),
        contentAlignment = Alignment.Center,
    ) { Mono(l, size = 13f, color = if (k in b) Kn.ink else Kn.frost) }
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        dot(Pad.Y, labels[0])
        Row(horizontalArrangement = Arrangement.spacedBy(s.dp(34f))) { dot(Pad.X, labels[1]); dot(Pad.B, labels[2]) }
        dot(Pad.A, labels[3])
    }
}

@Composable
private fun Trigger(label: String, v: Int, modifier: Modifier) {
    val s = LocalScale.current
    Column(modifier, verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
            Mono(label, size = 12f)
            Mono("${v * 100 / 32767} %", size = 12f)
        }
        Box(Modifier.fillMaxWidth().height(s.dp(10f)).clip(RoundedCornerShape(5.dp)).background(Kn.frost.copy(alpha = 0.1f))) {
            Box(Modifier.fillMaxWidth(v / 32767f).height(s.dp(10f)).background(Kn.spectre))
        }
    }
}
