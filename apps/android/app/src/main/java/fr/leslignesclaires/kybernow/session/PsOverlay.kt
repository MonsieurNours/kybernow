package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.panel
import org.json.JSONObject

/** Les mesures d'une partie, une fois par seconde (`PsNative.stats`). */
data class PsStats(
    val width: Int, val height: Int, val hevc: Boolean, val hdr: Boolean, val targetKbps: Int,
    val fps: Double, val mbps: Double, val lost: Long, val recovered: Long,
) {
    companion object {
        fun parse(json: String?): PsStats? = json?.let {
            runCatching {
                val o = JSONObject(it)
                PsStats(
                    o.optInt("width"), o.optInt("height"), o.optBoolean("hevc"), o.optBoolean("hdr"), o.optInt("target_kbps"),
                    o.optDouble("fps"), o.optDouble("mbps"), o.optLong("lost"), o.optLong("recovered"),
                )
            }.getOrNull()
        }
    }
}

/**
 * Ce qu'une partie sur console ajoute à l'interface de session (docs/link-ui.md, « Barre de la partie », « Session ») :
 * la barre et le menu ne gardent que ce qui a du sens pour une console. Les actions vont à la partie.
 */
class ConsoleUi(
    val onMute: (Boolean) -> Unit,
    val onRestMode: () -> Unit,
    val onKeyboardText: (String) -> Unit,
    val onKeyboardDone: (Boolean) -> Unit,
) {
    var muted by mutableStateOf(false)
    /** Images par seconde demandées (réglages de la console). */
    var fps by mutableStateOf(60)
    /** Contenu protégé : « La console ne peut pas diffuser ce qu'elle affiche. » */
    var cantDisplay by mutableStateOf(false)
    /** La console a ouvert son clavier : le texte de son champ. */
    var keyboard by mutableStateOf<String?>(null)
    var stats by mutableStateOf<PsStats?>(null)
    /** Les mesures depuis le début de la partie. */
    var lostTotal by mutableStateOf(0L)
    var recoveredTotal by mutableStateOf(0L)
}

/** La barre d'une partie : mesures, manette à l'écran, son, veille (« Se déconnecter » et la croix suivent, comme pour un ordinateur). */
@Composable
internal fun ConsoleBarItems(ui: SessionUi, c: ConsoleUi, first: FocusRequester) {
    val sec by ui.state.second.collectAsState()
    StatsCard(ui, sec, Modifier.focusRequester(first))
    GlassButton(stringResource(R.string.pad_on_screen), { ui.actions.setOnScreenPad(!ui.padShown) }, selected = ui.padShown)
    GlassButton(stringResource(if (c.muted) R.string.ps_unmute else R.string.ps_mute), { c.onMute(!c.muted) }, selected = c.muted)
    GlassButton(stringResource(R.string.ps_rest_mode), { c.onRestMode() })
}

/** Le menu du téléviseur d'une partie : reprendre, le chemin, le son, les statistiques, la veille, se déconnecter. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ConsoleTiles(ui: SessionUi, c: ConsoleUi, first: FocusRequester) {
    val s = LocalScale.current
    val route by ui.state.route.collectAsState()
    FlowRow(Modifier.widthIn(max = s.dp(900f)), horizontalArrangement = Arrangement.spacedBy(s.dp(14f), Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(s.dp(14f)), maxItemsInEachRow = 5) {
        Tile(KnIcons.play, stringResource(R.string.session_resume), null, Modifier.focusRequester(first), primary = true) { ui.actions.resume() }
        if (route?.viaRelay == true) Tile(KnIcons.network, stringResource(R.string.route_relay), null, selected = true) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.sound, stringResource(if (c.muted) R.string.ps_unmute else R.string.ps_mute), null, selected = c.muted) { c.onMute(!c.muted) }
        Tile(KnIcons.pulse, stringResource(R.string.session_stats), null) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.power, stringResource(R.string.ps_rest_mode), null) { c.onRestMode() }
        Tile(KnIcons.exit, stringResource(R.string.session_disconnect), null, danger = true) { ui.actions.quit() }
    }
}

/** Les lignes du panneau des statistiques d'une partie (docs/link-ui.md, « Barre de la partie »). */
@Composable
internal fun ConsoleStatsLines(c: ConsoleUi) {
    val st = c.stats ?: return
    val locale = androidx.compose.ui.platform.LocalConfiguration.current.locales[0]
    val format = if (st.hevc) "H.265" else "H.264"
    Mono(
        stringResource(if (st.hdr) R.string.ps_stats_asked_hdr else R.string.ps_stats_asked, st.width, st.height, c.fps, format, st.targetKbps / 1000),
        size = 11f, color = Kn.frost,
    )
    Mono(stringResource(R.string.ps_stats_received, st.fps.toInt(), String.format(locale, "%.1f", st.mbps)), size = 11f)
    Mono(stringResource(R.string.ps_stats_losses, c.lostTotal, c.recoveredTotal), size = 11f)
}

/**
 * Le clavier de la console (docs/playstation.md, « Clavier de la console ») : la console a ouvert son clavier, le clavier
 * de l'appareil (celui du système au toucher, ou un clavier branché) écrit dans son champ ; Entrée valide, Échap ou ✕
 * annule ; le texte entier part à chaque changement.
 */
@Composable
internal fun ConsoleKeyboard(c: ConsoleUi, touch: Boolean, modifier: Modifier) {
    val s = LocalScale.current
    val text = c.keyboard ?: return
    var value by remember { mutableStateOf(text) }
    // Le texte que la console renvoie remplace celui de l'app.
    LaunchedEffect(text) { if (text != value) value = text }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(modifier.padding(s.dp(16f)).panel(18.dp).padding(s.dp(14f)).widthIn(max = s.dp(640f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        Body(stringResource(if (touch) R.string.ps_keyboard_banner_touch else R.string.ps_keyboard_banner), size = 13f, color = Kn.frost2)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            OutlinedTextField(
                value, { v -> value = v; c.onKeyboardText(v) },
                Modifier.weight(1f).focusRequester(focus),
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { c.onKeyboardDone(true) }),
                colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, cursorColor = Kn.spectre),
            )
            GlassButton("✓", { c.onKeyboardDone(true) })
            GlassButton("✕", { c.onKeyboardDone(false) })
        }
    }
}

/** Contenu protégé, sur l'image tant que dure le blocage. */
@Composable
internal fun ConsoleCantDisplay(c: ConsoleUi, modifier: Modifier) {
    val s = LocalScale.current
    if (!c.cantDisplay) return
    Body(stringResource(R.string.ps_cant_display), modifier.fillMaxWidth().padding(s.dp(24f)).panel(18.dp).padding(s.dp(18f)), size = 16f)
}
