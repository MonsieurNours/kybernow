package fr.leslignesclaires.kybernow.session

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.fmt
import fr.leslignesclaires.kybernow.ui.home.motorsLabel
import fr.leslignesclaires.kybernow.ui.home.profileLabel
import fr.leslignesclaires.kybernow.ui.panel

/** Ce que l'interface de session montre et ce qu'elle peut demander. */
class SessionUi(
    val hostName: String,
    val tv: Boolean,
    val state: SessionState,
    val actions: SessionActions,
) {
    var menuOpen by mutableStateOf(false)
    var barOpen by mutableStateOf(false)
    var keyboardOpen by mutableStateOf(false)
    var keysOpen by mutableStateOf(false)
    var statsOpen by mutableStateOf(false)
    var typing by mutableStateOf(false)
    var passageDone by mutableStateOf(false)
    /** « Se déconnecter » demandé : on attend la fin de la session. */
    var leaving by mutableStateOf(false)
    var profile by mutableStateOf(Profile.GAME)
    var mouseMode by mutableStateOf(0)
    var directTouch by mutableStateOf(false)
    var remoteMouse by mutableStateOf(false)
    var padMouse by mutableStateOf(false)
    /** Hors téléviseur, la barre ouverte par la combinaison de la manette : la manette la pilote, le jeu ne reçoit rien. */
    var padBar by mutableStateOf(false)
    /** Modificateurs collants de la rangée de touches (Ctrl, Alt, ⊞). */
    var sticky by mutableStateOf(emptySet<Int>())

    /** Quelque chose est à l'écran par-dessus l'image. */
    val anything get() = leaving || !passageDone || menuOpen || barOpen || keyboardOpen || statsOpen || typing
}

interface SessionActions {
    fun resume()
    fun quit()
    fun setProfile(p: Profile)
    fun setMouseMode(m: Int)
    fun setDirectTouch(on: Boolean)
    fun setRemoteMouse(on: Boolean)
    fun setPadMouse(on: Boolean)
    fun shortcut(s: Shortcut)
    fun key(scancode: Int)
    fun text(s: String)
    fun restoreMode()
    fun pickDisplay(id: Int)
    fun openKeyboard(open: Boolean)
}

@Composable
fun SessionOverlay(ui: SessionUi) {
    Box(Modifier.fillMaxSize()) {
        Capsules(ui, Modifier.align(Alignment.TopCenter))
        if (!ui.passageDone) Passage(ui)
        if (ui.tv) {
            AnimatedVisibility(ui.menuOpen, enter = fadeIn(tween(160)), exit = fadeOut(tween(160))) { TvMenu(ui) }
        } else {
            if (ui.passageDone && !ui.barOpen && !ui.keyboardOpen) Tab(ui, Modifier.align(Alignment.TopCenter))
            AnimatedVisibility(ui.barOpen, Modifier.align(Alignment.TopCenter), enter = slideInVertically { -it } + fadeIn(), exit = slideOutVertically { -it } + fadeOut()) { PhoneBar(ui) }
            if (ui.keyboardOpen) KeyRow(ui, Modifier.align(Alignment.BottomCenter).imePadding())
        }
        if (ui.statsOpen) Stats(ui, Modifier.align(if (ui.tv) Alignment.TopEnd else Alignment.BottomEnd).padding(LocalScale.current.dp(20f)))
        if (ui.keysOpen) KeysSheet(ui, Modifier.align(Alignment.Center))
        if (ui.typing) TypeText(ui, Modifier.align(Alignment.Center))
        val ended by ui.state.ended.collectAsState()
        ended?.takeIf { it.error != null }?.let { EndedCard(ui, it, Modifier.align(Alignment.Center)) }
        AnimatedVisibility(ui.leaving, enter = fadeIn(tween(220)), exit = fadeOut(tween(160))) { Leaving() }
    }
}

/** Pendant la fin de session : l'image s'efface, l'ordinateur se remet. */
@Composable
private fun Leaving() {
    val s = LocalScale.current
    Box(Modifier.fillMaxSize().background(Kn.ink.copy(alpha = 0.9f)), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
            androidx.compose.material3.CircularProgressIndicator(Modifier.size(s.dp(36f)), color = Kn.spectre, strokeWidth = s.dp(3f))
            Body(stringResource(R.string.session_leaving), size = 20f, weight = 600)
            Body(stringResource(R.string.session_leaving_detail), size = 14f, color = Kn.muted)
        }
    }
}

// MARK: - Passage

/**
 * Le passage : l'accueil se coupe le long de la couture à 45°, le tube néon
 * s'allume, puis les deux moitiés s'écartent sur l'image. Capsule « Vous
 * entrez dans… » : comment ouvrir le menu, le temps jusqu'à la première image.
 */
@Composable
private fun Passage(ui: SessionUi) {
    val s = LocalScale.current
    val phase by ui.state.phase.collectAsState()
    val open = remember { Animatable(0f) }
    val neon = remember { Animatable(0f) }
    LaunchedEffect(Unit) { neon.animateTo(1f, tween(900)) }
    LaunchedEffect(phase) {
        if (phase == Phase.STREAMING) {
            open.animateTo(1f, tween(900, easing = FastOutSlowInEasing))
            ui.passageDone = true
        }
    }
    Box(Modifier.fillMaxSize()) {
        Canvas(Modifier.fillMaxSize()) {
            val w = size.width
            val h = size.height
            val d = (w + h) * 0.6f * open.value
            // Moitié haute-gauche (l'ordinateur) et basse-droite (vous), le long de la couture.
            val top = Path().apply { moveTo(0f, 0f); lineTo(w, 0f); lineTo(0f, h); close() }
            val bottom = Path().apply { moveTo(w, 0f); lineTo(w, h); lineTo(0f, h); close() }
            translate(left = -d * 0.7f, top = -d * 0.7f) { drawPath(top, Kn.ink) }
            translate(left = d * 0.7f, top = d * 0.7f) { drawPath(bottom, Kn.ink2) }
            val a = neon.value * (1f - open.value)
            if (a > 0f) {
                for (i in 4 downTo 1) drawLine(Kn.spectreFlat.copy(alpha = 0.12f * a), Offset(0f, h), Offset(w, 0f), strokeWidth = i * 9.dp.toPx(), cap = StrokeCap.Round)
                drawLine(Kn.spectre.copy(alpha = a), Offset(0f, h), Offset(w, 0f), strokeWidth = 3.dp.toPx(), cap = StrokeCap.Round)
                drawLine(Kn.neonCore.copy(alpha = a), Offset(0f, h), Offset(w, 0f), strokeWidth = 1.dp.toPx(), cap = StrokeCap.Round)
            }
        }
        if (open.value < 0.5f) {
            Column(
                Modifier.align(Alignment.BottomCenter).padding(bottom = s.dp(if (ui.tv) 64f else 28f)).panel(22.dp).padding(horizontal = s.dp(24f), vertical = s.dp(16f)),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(s.dp(6f)),
            ) {
                Eyebrow(stringResource(R.string.session_entering))
                Title(ui.hostName, size = if (ui.tv) 34f else 22f)
                Mono(
                    stringResource(
                        when (phase) {
                            Phase.PREPARING -> R.string.session_preparing
                            Phase.CONNECTING -> R.string.session_connecting
                            else -> R.string.session_first_images
                        },
                    ),
                    size = 12f,
                )
                Body(stringResource(if (ui.tv) R.string.session_menu_hint_tv else R.string.session_menu_hint_touch), size = 13f, color = Kn.muted)
                Spacer(Modifier.height(s.dp(4f)))
                GlassButton(stringResource(R.string.session_cancel), { ui.actions.quit() })
            }
        }
    }
}

// MARK: - Capsules

@Composable
private fun Capsules(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val c by ui.state.capsule.collectAsState()
    val restore by ui.state.restoreOffer.collectAsState()
    Column(modifier.padding(top = s.dp(if (ui.barOpen && ui.padBar) 116f else if (ui.barOpen) 84f else 14f)), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        c?.let { capsule ->
            Row(
                Modifier.panel(999.dp).padding(horizontal = s.dp(16f), vertical = s.dp(9f)),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (capsule is Capsule.Reconnecting) {
                    Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(Kn.spectre))
                    Spacer(Modifier.width(s.dp(10f)))
                }
                Body(capsuleText(capsule), size = 14f)
                if (capsule is Capsule.Reconnecting) {
                    Spacer(Modifier.width(s.dp(12f)))
                    GlassButton(stringResource(R.string.session_quit), { ui.actions.quit() })
                }
            }
        }
        if (ui.menuOpen || ui.barOpen) restore?.let { (w, h) ->
            GlassButton(stringResource(R.string.session_restore, w, h), { ui.actions.restoreMode() })
        }
    }
}

@Composable
private fun capsuleText(c: Capsule): String = when (c) {
    is Capsule.Reconnecting -> stringResource(R.string.capsule_reconnecting, c.attempt, c.max)
    Capsule.Relaunched -> stringResource(R.string.capsule_relaunched)
    is Capsule.ModeChanged -> stringResource(if (c.back) R.string.capsule_mode_back else R.string.capsule_mode_changed, c.width, c.height)
    is Capsule.Fallback -> if (c.level >= 2) stringResource(R.string.capsule_fallback_420) else stringResource(R.string.capsule_fallback_bitrate, c.mbps)
    is Capsule.Adapting -> stringResource(R.string.capsule_adapting, c.width, c.height)
    is Capsule.AdaptFailed -> stringResource(R.string.capsule_adapt_failed)
    is Capsule.GamepadConnected -> stringResource(R.string.capsule_gamepad, c.name, c.slot + 1, motorsLabel(c.motors))
}

// MARK: - Téléphone et tablette

/** La languette : latence en direct ; un toucher ouvre la barre. */
@Composable
private fun Tab(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val sec by ui.state.second.collectAsState()
    FocusCard({ ui.barOpen = true }, modifier.padding(top = s.dp(6f)), shape = RoundedCornerShape(bottomStart = 14.dp, bottomEnd = 14.dp)) {
        Row(Modifier.background(Kn.panel.copy(alpha = 0.72f)).padding(horizontal = s.dp(14f), vertical = s.dp(5f)), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(s.dp(6f)).clip(CircleShape).background(Kn.spectre))
            Spacer(Modifier.width(s.dp(7f)))
            Mono(sec?.latencyMs?.let { fmt("%.1f ms", it) } ?: "—", size = 11f, color = Kn.frost)
        }
    }
}

@Composable
private fun PhoneBar(ui: SessionUi) {
    val s = LocalScale.current
    val sec by ui.state.second.collectAsState()
    val first = remember { FocusRequester() }
    // Ouverte à la manette : le focus sur le premier élément, et de nouveau quand la feuille des
    // touches se ferme. Une image d'attente : la vue d'interface vient de passer de GONE à visible.
    LaunchedEffect(ui.padBar, ui.keysOpen) {
        if (ui.padBar && !ui.keysOpen) {
            withFrameNanos { }
            runCatching { first.requestFocus() }
        }
    }
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Row(
            Modifier.padding(s.dp(10f)).panel(16.dp).horizontalScroll(rememberScrollState()).padding(s.dp(8f)),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(s.dp(6f)),
        ) {
            FocusCard({ ui.statsOpen = !ui.statsOpen }, Modifier.focusRequester(first)) {
                Column(Modifier.padding(horizontal = s.dp(8f))) {
                    Mono(sec?.latencyMs?.let { fmt("%.1f ms", it) } ?: "—", size = 14f, color = Kn.frost, weight = 500)
                    Mono("${sec?.fps ?: 0} i/s · ${fmt("%.0f", sec?.mbps ?: 0.0)} Mbit/s", size = 9f)
                }
            }
            Profile.entries.forEach { p -> GlassButton(profileLabel(p), { ui.actions.setProfile(p) }, selected = ui.profile == p) }
            GlassButton(stringResource(if (ui.directTouch) R.string.touch_direct else R.string.touch_trackpad), { ui.actions.setDirectTouch(!ui.directTouch) })
            GlassButton(
                stringResource(R.string.session_mouse, stringResource(listOf(R.string.settings_auto, R.string.mouse_desktop, R.string.mouse_game)[ui.mouseMode])),
                { ui.actions.setMouseMode((ui.mouseMode + 1) % 3) },
            )
            screenChoice(ui)?.let { c -> GlassButton(stringResource(R.string.session_screen, c.index + 1, c.width, c.height), { ui.actions.pickDisplay(c.next) }) }
            BarIcon(KnIcons.keyboard) { ui.actions.openKeyboard(true) }
            GlassButton(stringResource(R.string.session_keys), { ui.keysOpen = true })
            BarIcon(KnIcons.close) { ui.barOpen = false }
            GlassButton(stringResource(R.string.session_quit), { ui.actions.quit() })
        }
        if (ui.padBar) Body(stringResource(R.string.session_pad_bar), Modifier.panel(999.dp).padding(horizontal = s.dp(14f), vertical = s.dp(6f)), size = 12f, color = Kn.frost2)
    }
}

/** L'écran de l'ordinateur à l'image : son rang, sa taille, et le suivant (un toucher passe à lui). */
private class ScreenChoice(val index: Int, val count: Int, val width: Int, val height: Int, val next: Int)

/** Sans écran virtuel, quand l'ordinateur a plusieurs écrans ; `null` sinon. */
@Composable
private fun screenChoice(ui: SessionUi): ScreenChoice? {
    val list by ui.state.displays.collectAsState()
    val shown by ui.state.display.collectAsState()
    val virtual by ui.state.virtualDisplay.collectAsState()
    if (virtual || list.size < 2) return null
    val i = list.indexOfFirst { it.id == shown?.id }.coerceAtLeast(0)
    return ScreenChoice(i, list.size, list[i].width, list[i].height, list[(i + 1) % list.size].id)
}

@Composable
private fun BarIcon(icon: androidx.compose.ui.graphics.vector.ImageVector, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, shape = RoundedCornerShape(12.dp)) {
        Box(Modifier.background(Kn.frost.copy(alpha = 0.07f)).padding(s.dp(10f))) { KnIcon(icon, size = 18f) }
    }
}

/**
 * Au-dessus du clavier de l'écran : Échap, Tab, Ctrl, Alt et ⊞ collants,
 * flèches, F1 à F12, Ctrl + Alt + Suppr ; et le champ invisible qui reçoit
 * le texte tapé (envoyé en Unicode) et l'effacement.
 */
@Composable
private fun KeyRow(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    // Le champ garde toujours un caractère : l'effacement se voit comme une suppression.
    var value by remember { mutableStateOf(TextFieldValue(" ", TextRange(1))) }
    LaunchedEffect(Unit) { focus.requestFocus(); keyboard?.show() }
    Column(modifier.fillMaxWidth()) {
        BasicTextField(
            value,
            onValueChange = { v ->
                val t = v.text
                when {
                    t.isEmpty() -> ui.actions.key(Sc.BACKSPACE)
                    t.length > 1 -> {
                        val typed = t.substring(1)
                        if (typed == "\n") ui.actions.key(Sc.ENTER) else ui.actions.text(typed)
                    }
                }
                value = TextFieldValue(" ", TextRange(1))
            },
            modifier = Modifier.size(1.dp).alpha(0f).focusRequester(focus).onPreviewKeyEvent { e ->
                if (e.type == KeyEventType.KeyDown && e.key == Key.Enter) { ui.actions.key(Sc.ENTER); true } else false
            },
            textStyle = TextStyle(fontSize = 1.sp),
        )
        Row(
            Modifier.fillMaxWidth().background(Kn.panel.copy(alpha = 0.95f)).horizontalScroll(rememberScrollState()).padding(s.dp(6f)),
            horizontalArrangement = Arrangement.spacedBy(s.dp(5f)),
        ) {
            KeyCap("Échap") { ui.actions.key(Sc.ESC) }
            KeyCap("Tab") { ui.actions.key(Sc.TAB) }
            for ((code, label) in listOf(Sc.CTRL to "Ctrl", Sc.ALT to "Alt", Sc.WIN to "⊞")) {
                KeyCap(label, selected = code in ui.sticky) { ui.sticky = if (code in ui.sticky) ui.sticky - code else ui.sticky + code }
            }
            KeyCap("←") { ui.actions.key(Sc.LEFT) }
            KeyCap("↑") { ui.actions.key(Sc.UP) }
            KeyCap("↓") { ui.actions.key(Sc.DOWN) }
            KeyCap("→") { ui.actions.key(Sc.RIGHT) }
            Sc.F.forEachIndexed { i, c -> KeyCap("F${i + 1}") { ui.actions.key(c) } }
            KeyCap("Ctrl+Alt+Suppr") { ui.actions.shortcut(Shortcut.CTRL_ALT_DEL) }
            KeyCap("✕") { ui.actions.openKeyboard(false) }
        }
    }
}

@Composable
private fun KeyCap(label: String, selected: Boolean = false, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, shape = RoundedCornerShape(8.dp)) {
        Box(Modifier.background(if (selected) Kn.spectre.copy(alpha = 0.3f) else Kn.frost.copy(alpha = 0.08f)).padding(horizontal = s.dp(11f), vertical = s.dp(8f))) {
            Mono(label, size = 12f, color = Kn.frost)
        }
    }
}

// MARK: - Téléviseur

@Composable
private fun TvMenu(ui: SessionUi) {
    val s = LocalScale.current
    val sec by ui.state.second.collectAsState()
    val mode by ui.state.mode.collectAsState()
    val decoder by ui.state.decoder.collectAsState()
    val audio by ui.state.audioOutput.collectAsState()
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Kn.ink.copy(alpha = 0.72f))) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = s.dp(32f), vertical = s.dp(28f)), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(16f))) {
            Row(Modifier.panel(999.dp).padding(horizontal = s.dp(26f), vertical = s.dp(12f)), horizontalArrangement = Arrangement.spacedBy(s.dp(26f)), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(Kn.spectre))
                Mono(sec?.latencyMs?.let { fmt("%.1f ms", it) } ?: "—", size = 15f, color = Kn.frost)
                Mono("${sec?.fps ?: 0} i/s", size = 15f, color = Kn.frost)
                Mono(fmt("%.0f Mbit/s", sec?.mbps ?: 0.0), size = 15f, color = Kn.frost)
                mode?.let { (w, h, r) -> Mono("$w × $h · $r Hz", size = 15f, color = Kn.frost) }
                Mono(stringResource(R.string.session_losses, (sec?.lostFrames ?: 0) + (sec?.holes ?: 0)), size = 15f)
            }
            Eyebrow("${ui.hostName} · ${profileLabel(ui.profile)}")
            TvTiles(ui, first, decoder, audio)
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TvTiles(ui: SessionUi, first: FocusRequester, decoder: String?, audio: Pair<Int, Double>?) {
    val s = LocalScale.current
    FlowRow(Modifier.widthIn(max = s.dp(900f)), horizontalArrangement = Arrangement.spacedBy(s.dp(14f), Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(s.dp(14f)), maxItemsInEachRow = 5) {
        Tile(KnIcons.play, stringResource(R.string.session_resume), null, Modifier.focusRequester(first), primary = true) { ui.actions.resume() }
        Profile.entries.forEach { p ->
            if (p != ui.profile) Tile(KnIcons.screen, profileLabel(p), stringResource(R.string.session_switch_profile)) { ui.actions.setProfile(p) }
        }
        Tile(KnIcons.sound, stringResource(R.string.session_sound), audio?.let { (c, ms) -> stringResource(R.string.session_sound_detail, c, ms) }) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.remote, stringResource(R.string.session_remote_mouse), stringResource(if (ui.remoteMouse) R.string.session_on else R.string.session_off), selected = ui.remoteMouse) { ui.actions.setRemoteMouse(!ui.remoteMouse) }
        Tile(KnIcons.gamepad, stringResource(R.string.session_pad_mouse), stringResource(if (ui.padMouse) R.string.session_on else R.string.session_off), selected = ui.padMouse) { ui.actions.setPadMouse(!ui.padMouse) }
        // Le menu reste ouvert : on passe d'un écran à l'autre en voyant l'image derrière.
        screenChoice(ui)?.let { c -> Tile(KnIcons.screen, stringResource(R.string.session_computer_screen), "${c.index + 1} / ${c.count} · ${c.width} × ${c.height}") { ui.actions.pickDisplay(c.next) } }
        Tile(KnIcons.keyboard, stringResource(R.string.session_type_text), null) { ui.typing = true }
        Tile(KnIcons.keys, stringResource(R.string.session_keys), "Ctrl+Alt+Suppr, Alt+Tab…") { ui.keysOpen = true }
        Tile(KnIcons.pulse, stringResource(R.string.session_stats), decoder) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.exit, stringResource(R.string.session_disconnect), null, danger = true) { ui.actions.quit() }
    }
}

@Composable
private fun Tile(icon: androidx.compose.ui.graphics.vector.ImageVector, title: String, detail: String?, modifier: Modifier = Modifier, primary: Boolean = false, selected: Boolean = false, danger: Boolean = false, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, modifier.width(s.dp(160f)).height(s.dp(104f)), shape = RoundedCornerShape(16.dp)) { focused ->
        Column(
            Modifier.fillMaxSize().background(if (focused && danger) Kn.danger else if (focused) Kn.neonCore else if (danger) Kn.danger.copy(alpha = 0.22f) else if (selected) Kn.spectre.copy(alpha = 0.22f) else Kn.panel.copy(alpha = 0.92f)).padding(s.dp(14f)),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            KnIcon(icon, tint = if (focused) Kn.ink else if (danger) Kn.danger else Kn.frost, size = 22f)
            Column(verticalArrangement = Arrangement.spacedBy(s.dp(2f))) {
                Body(title, size = 15f, weight = 600, color = if (focused) Kn.ink else if (danger) Kn.danger else Kn.frost, maxLines = 2)
                detail?.let { Mono(it, size = 10f, color = if (focused) Kn.ink2 else Kn.frost2, maxLines = 1) }
            }
        }
    }
}

// MARK: - Feuilles

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun KeysSheet(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    val items = listOf(
        Shortcut.CTRL_ALT_DEL to "Ctrl + Alt + Suppr", Shortcut.ALT_TAB to "Alt + Tab", Shortcut.WIN to "⊞", Shortcut.WIN_D to "⊞ + D",
        Shortcut.PRINT to stringResource(R.string.keys_print), Shortcut.ALT_F4 to "Alt + F4", Shortcut.WIN_L to "⊞ + L",
        Shortcut.TASK_MANAGER to stringResource(R.string.keys_task_manager), Shortcut.F11 to "F11", Shortcut.ESC to stringResource(R.string.keys_escape),
    )
    Column(modifier.padding(s.dp(20f)).panel(22.dp).padding(s.dp(22f)).widthIn(max = s.dp(720f)), verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
        Eyebrow(stringResource(R.string.session_keys))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            items.forEachIndexed { i, (sc, label) ->
                GlassButton(label, { ui.actions.shortcut(sc); ui.keysOpen = false }, if (i == 0) Modifier.focusRequester(first) else Modifier)
            }
        }
        GlassButton(stringResource(R.string.session_close), { ui.keysOpen = false })
    }
}

@Composable
private fun TypeText(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    var text by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(modifier.padding(s.dp(20f)).panel(22.dp).padding(s.dp(22f)).widthIn(min = s.dp(420f), max = s.dp(720f)), verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
        Eyebrow(stringResource(R.string.session_type_text))
        BasicTextField(
            text, { text = it },
            Modifier.fillMaxWidth().background(Kn.frost.copy(alpha = 0.06f), RoundedCornerShape(10.dp)).padding(s.dp(14f)).focusRequester(focus),
            textStyle = TextStyle(color = Kn.frost, fontSize = s.sp(18f)),
            singleLine = true,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(s.dp(10f))) {
            PrimaryButton(stringResource(R.string.session_send), { ui.actions.text(text); text = ""; ui.typing = false })
            GlassButton(stringResource(R.string.session_send_enter), { ui.actions.text(text); ui.actions.key(Sc.ENTER); text = ""; ui.typing = false })
            GlassButton(stringResource(R.string.session_close), { ui.typing = false })
        }
    }
}

/** Mesures détaillées : trajet d'une image, pertes, aller-retour, décodeur, son. */
@Composable
private fun Stats(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val sec by ui.state.second.collectAsState()
    val decoder by ui.state.decoder.collectAsState()
    val audio by ui.state.audioOutput.collectAsState()
    val video by ui.state.videoSize.collectAsState()
    val locale = androidx.compose.ui.platform.LocalConfiguration.current.locales[0]
    fun f(v: Double?) = v?.let { String.format(locale, "%.1f", it) } ?: "—"
    Column(modifier.panel(18.dp).padding(s.dp(16f)).widthIn(max = s.dp(340f)), verticalArrangement = Arrangement.spacedBy(s.dp(5f))) {
        Eyebrow(stringResource(R.string.session_stats))
        Mono("${stringResource(R.string.stats_latency)} ${f(sec?.latencyMs)} ms", color = Kn.frost)
        sec?.chain?.let { c ->
            val names = listOf(R.string.stats_capture, R.string.stats_encode, R.string.stats_send, R.string.stats_network, R.string.stats_decode, R.string.stats_display)
            names.forEachIndexed { i, n -> Mono("  ${stringResource(n)} ${f(c[i])} ms", size = 11f) }
        }
        Mono("${stringResource(R.string.stats_rtt)} ${f(sec?.rttMs)} ms", size = 11f)
        Mono("${sec?.fps ?: 0} i/s · ${fmt("%.1f", sec?.mbps ?: 0.0)} Mbit/s", size = 11f)
        Mono(stringResource(R.string.stats_losses, sec?.lostFrames ?: 0, sec?.holes ?: 0, sec?.lostPackets ?: 0), size = 11f)
        video?.let { (w, h) -> Mono("$w × $h · ${decoder ?: ""}", size = 11f) }
        audio?.let { (c, ms) -> Mono(stringResource(R.string.session_sound_detail, c, ms), size = 11f) }
        if (!ui.tv) GlassButton(stringResource(R.string.session_close), { ui.statsOpen = false })
    }
}

@Composable
private fun EndedCard(ui: SessionUi, e: Ended, modifier: Modifier) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(modifier.padding(s.dp(20f)).panel(22.dp).padding(s.dp(24f)).widthIn(max = s.dp(620f)), verticalArrangement = Arrangement.spacedBy(s.dp(12f))) {
        Title(stringResource(R.string.session_ended_title), size = 24f)
        Body(e.error ?: "", color = Kn.frost2, size = 15f)
        if (e.displayRestored == false || e.audioRestored == false) Body(stringResource(R.string.session_not_restored), color = Kn.warn, size = 13f)
        PrimaryButton(stringResource(R.string.session_back_home), { ui.actions.quit() }, Modifier.focusRequester(focus))
    }
}
