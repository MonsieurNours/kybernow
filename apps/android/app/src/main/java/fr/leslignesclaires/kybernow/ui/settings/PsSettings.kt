package fr.leslignesclaires.kybernow.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusTarget
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.stringResource
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.PsNative
import fr.leslignesclaires.kybernow.playstation.PsConsole
import fr.leslignesclaires.kybernow.playstation.PsKeys
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.LocalForm
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.panel
import fr.leslignesclaires.kybernow.ui.playstation.psSettingsTitle
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.roundToInt

/** Les actions de la manette dans l'ordre du cœur (`Action::ALL`), avec leur texte. */
private val ACTIONS = listOf(
    "left_stick_up" to R.string.ps_action_left_stick_up, "left_stick_down" to R.string.ps_action_left_stick_down,
    "left_stick_left" to R.string.ps_action_left_stick_left, "left_stick_right" to R.string.ps_action_left_stick_right,
    "cross" to R.string.ps_action_cross, "circle" to R.string.ps_action_circle, "square" to R.string.ps_action_square, "triangle" to R.string.ps_action_triangle,
    "l1" to R.string.ps_action_l1, "r1" to R.string.ps_action_r1, "l2" to R.string.ps_action_l2, "r2" to R.string.ps_action_r2,
    "l3" to R.string.ps_action_l3, "r3" to R.string.ps_action_r3,
    "dpad_up" to R.string.ps_action_dpad_up, "dpad_down" to R.string.ps_action_dpad_down, "dpad_left" to R.string.ps_action_dpad_left, "dpad_right" to R.string.ps_action_dpad_right,
    "options" to R.string.ps_action_options, "create" to R.string.ps_action_create, "ps" to R.string.ps_action_ps, "touchpad" to R.string.ps_action_touchpad,
    "right_stick_up" to R.string.ps_action_right_stick_up, "right_stick_down" to R.string.ps_action_right_stick_down,
    "right_stick_left" to R.string.ps_action_right_stick_left, "right_stick_right" to R.string.ps_action_right_stick_right,
)

/** Une entrée de la disposition : `{"key": usage}` ou `{"mouse": bouton}`. */
private data class PsInput(val key: Int?, val mouse: Int?) {
    fun json(): JSONObject = if (key != null) JSONObject().put("key", key) else JSONObject().put("mouse", mouse)

    companion object {
        fun parse(o: JSONObject) = PsInput(if (o.has("key")) o.getInt("key") else null, if (o.has("mouse")) o.getInt("mouse") else null)
    }
}

@Composable
private fun inputLabel(i: PsInput): String = when {
    i.key != null -> PsKeys.label(i.key, stringResource(R.string.ps_input_space))
    i.mouse == 0 -> stringResource(R.string.ps_input_left_click)
    i.mouse == 1 -> stringResource(R.string.ps_input_right_click)
    i.mouse == 2 -> stringResource(R.string.ps_input_middle_click)
    i.mouse == 3 -> stringResource(R.string.ps_input_button4)
    else -> stringResource(R.string.ps_input_button5)
}

/**
 * « Réglages de {nom} » d'une console (docs/link-ui.md, « Réglages de la console »), présentés comme ceux d'un
 * ordinateur : Image (définition, images par seconde, débit, format), Commandes (clavier et souris, intensités de la
 * manette). Pas de plein écran ni d'agrandissement sur Android (toujours en plein écran ; l'agrandissement MetalFX est
 * propre aux appareils Apple).
 */
@Composable
fun ConsoleSettingsScreen(consoleId: String, onBack: () -> Unit, onRemoved: () -> Unit) {
    val ps = KyberNowApp.instance.ps
    val consoles by ps.store.consoles.collectAsState()
    val c = consoles.firstOrNull { it.id == consoleId } ?: return
    val settings = remember(c.settings) { JSONObject(c.settings ?: PsNative.defaultSettings()) }
    fun save(f: (JSONObject) -> Unit) {
        val o = JSONObject(settings.toString()).also(f)
        ps.store.update(c.id) { it.copy(settings = o.toString()) }
    }
    var removing by remember { mutableStateOf(false) }
    Page(psSettingsTitle(c.name), onBack) {
        Section(stringResource(R.string.ps_section_picture))
        val res = settings.optString("resolution", "auto")
        ChoiceRow(
            stringResource(R.string.ps_resolution), if (res == "auto") stringResource(R.string.ps_resolution_auto_help) else null,
            listOf("auto" to stringResource(R.string.ps_auto), "p1080" to "1080p", "p720" to "720p", "p540" to "540p", "p360" to "360p"), res,
        ) { v -> save { it.put("resolution", v) } }
        ChoiceRow(
            stringResource(R.string.ps_fps), null, listOf(60 to stringResource(R.string.ps_fps_n, 60), 30 to stringResource(R.string.ps_fps_n, 30)),
            settings.optInt("fps", 60),
        ) { v -> save { it.put("fps", v) } }
        BitrateRow(settings.takeIf { !it.isNull("bitrate_kbps") && it.has("bitrate_kbps") }?.optInt("bitrate_kbps")) { kbps ->
            save { if (kbps == null) it.put("bitrate_kbps", JSONObject.NULL) else it.put("bitrate_kbps", kbps) }
        }
        val codec = settings.optString("codec", "auto")
        ChoiceRow(
            stringResource(R.string.ps_format),
            when {
                !c.ps5 -> stringResource(R.string.ps_hdr_ps4)
                codec == "auto" -> stringResource(R.string.ps_format_auto_help)
                codec == "h264" -> stringResource(R.string.ps_format_h264_help)
                else -> null
            },
            listOf("auto" to stringResource(R.string.ps_auto), "h264" to "H.264", "h265" to "H.265"), codec, enabled = c.ps5,
        ) { v -> save { it.put("codec", v) } }

        Section(stringResource(R.string.ps_section_controls))
        Body(stringResource(R.string.ps_controls_help), size = 13f, color = Kn.muted)
        IntensityRow(stringResource(R.string.ps_rumble), settings.optInt("rumble", 100)) { v -> save { it.put("rumble", v) } }
        IntensityRow(stringResource(R.string.ps_triggers), settings.optInt("triggers", 100)) { v -> save { it.put("triggers", v) } }
        Body(stringResource(R.string.ps_intensity_help), size = 13f, color = Kn.muted)
        Body(stringResource(R.string.ps_android_triggers), size = 13f, color = Kn.muted)
        // Le clavier et la souris : seulement là où l'on en branche (téléphone, tablette, téléviseur avec un clavier).
        LayoutRows(c)
        GlassButton(stringResource(R.string.ps_remove_console), { removing = true })
    }
    if (removing) {
        AlertDialog(
            onDismissRequest = { removing = false },
            title = { Text(stringResource(R.string.ps_remove_console_confirm, c.name)) },
            confirmButton = { TextButton({ removing = false; ps.store.remove(c); onRemoved() }) { Text(stringResource(R.string.ps_remove)) } },
            dismissButton = { TextButton({ removing = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@Composable
private fun BitrateRow(kbps: Int?, onChange: (Int?) -> Unit) {
    val s = LocalScale.current
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Body(stringResource(R.string.ps_bitrate), size = 16f)
        Row(horizontalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            GlassButton(stringResource(R.string.ps_auto), { onChange(null) }, selected = kbps == null)
            GlassButton(stringResource(R.string.ps_mbps, (kbps ?: 15_000) / 1000), { if (kbps == null) onChange(15_000) }, selected = kbps != null)
        }
        if (kbps == null) Body(stringResource(R.string.ps_bitrate_auto_help), size = 13f, color = Kn.muted)
        else Slider(
            kbps / 1000f, { v -> onChange((v.roundToInt().coerceIn(2, 100)) * 1000) }, valueRange = 2f..100f, steps = 97,
            colors = SliderDefaults.colors(thumbColor = Kn.spectre, activeTrackColor = Kn.spectre),
        )
    }
}

@Composable
private fun IntensityRow(label: String, value: Int, onChange: (Int) -> Unit) {
    val s = LocalScale.current
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Body(label, size = 16f, modifier = Modifier.weight(1f))
            Mono(if (value == 0) stringResource(R.string.ps_percent_off) else stringResource(R.string.ps_percent, value), size = 13f, color = Kn.frost)
        }
        Slider(
            value / 10f, { v -> onChange((v.roundToInt() * 10).coerceIn(0, 100)) }, valueRange = 0f..10f, steps = 9,
            colors = SliderDefaults.colors(thumbColor = Kn.spectre, activeTrackColor = Kn.spectre),
        )
    }
}

/** La disposition du clavier et de la souris de la console : réglages de la souris, puis une ligne par action. */
@Composable
private fun LayoutRows(c: PsConsole) {
    val s = LocalScale.current
    val ps = KyberNowApp.instance.ps
    val layout = remember(c.layout) { JSONObject(c.layout ?: PsNative.defaultLayout()) }
    fun save(f: (JSONObject) -> Unit) {
        val o = JSONObject(layout.toString()).also(f)
        ps.store.update(c.id) { it.copy(layout = o.toString()) }
    }
    val bindings = remember(c.layout) {
        val a = layout.optJSONArray("bindings") ?: JSONArray()
        List(a.length()) { a.getJSONArray(it).let { p -> PsInput.parse(p.getJSONObject(0)) to p.getString(1) } }
    }
    fun write(list: List<Pair<PsInput, String>>) = save { o -> o.put("bindings", JSONArray().apply { list.forEach { (i, a) -> put(JSONArray().put(i.json()).put(a)) } }) }
    var editing by remember { mutableStateOf<Pair<String, PsInput?>?>(null) }

    SwitchRow(stringResource(R.string.ps_mouse_stick), null, layout.optBoolean("mouse_stick", true)) { v -> save { it.put("mouse_stick", v) } }
    val sens = layout.optDouble("sensitivity", 1.0).toFloat()
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        Row { Body(stringResource(R.string.ps_sensitivity), size = 16f, modifier = Modifier.weight(1f)); Mono("%.1f".format(sens), size = 13f, color = Kn.frost) }
        Slider(sens, { v -> save { it.put("sensitivity", ((v * 10).roundToInt() / 10.0).coerceIn(0.2, 5.0)) } }, valueRange = 0.2f..5f, colors = SliderDefaults.colors(thumbColor = Kn.spectre, activeTrackColor = Kn.spectre))
    }
    val curve = layout.optDouble("curve", 0.8)
    ChoiceRow(
        stringResource(R.string.ps_curve), null,
        listOf(1.2 to stringResource(R.string.ps_curve_soft), 1.0 to stringResource(R.string.ps_curve_normal), 0.8 to stringResource(R.string.ps_curve_brisk)),
        listOf(1.2, 1.0, 0.8).minByOrNull { kotlin.math.abs(it - curve) }!!,
    ) { v -> save { it.put("curve", v) } }
    val dz = (layout.optDouble("deadzone", 0.2) * 100).roundToInt()
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        Row { Body(stringResource(R.string.ps_deadzone), size = 16f, modifier = Modifier.weight(1f)); Mono(stringResource(R.string.ps_percent, dz), size = 13f, color = Kn.frost) }
        Body(stringResource(R.string.ps_deadzone_help), size = 13f, color = Kn.muted)
        Slider(dz / 5f, { v -> save { it.put("deadzone", (v.roundToInt() * 5) / 100.0) } }, valueRange = 0f..10f, steps = 9, colors = SliderDefaults.colors(thumbColor = Kn.spectre, activeTrackColor = Kn.spectre))
    }
    SwitchRow(stringResource(R.string.ps_invert_y), null, layout.optBoolean("invert_y", false)) { v -> save { it.put("invert_y", v) } }

    ACTIONS.forEach { (action, label) ->
        val inputs = bindings.filter { it.second == action }.map { it.first }
        Row(Modifier.panel().fillMaxWidth().padding(s.dp(12f)), verticalAlignment = Alignment.CenterVertically) {
            Body(stringResource(label), size = 15f, modifier = Modifier.weight(1f))
            if (inputs.isEmpty()) Mono(stringResource(R.string.ps_input_none), size = 12f, color = Kn.muted)
            inputs.forEach { i ->
                Spacer(Modifier.width(s.dp(6f)))
                GlassButton(inputLabel(i), { editing = action to i })
            }
            Spacer(Modifier.width(s.dp(6f)))
            GlassButton("+", { editing = action to null })
        }
    }
    GlassButton(stringResource(R.string.ps_default_layout), { ps.store.update(c.id) { it.copy(layout = null) } })

    editing?.let { (action, old) ->
        val focus = remember { FocusRequester() }
        AlertDialog(
            onDismissRequest = { editing = null },
            title = { Text(stringResource(R.string.ps_press_key)) },
            text = {
                Column(
                    Modifier.widthIn(min = s.dp(240f)).focusRequester(focus).focusTarget().onPreviewKeyEvent { e ->
                        if (e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent true
                        val native = e.nativeKeyEvent
                        when (native.keyCode) {
                            android.view.KeyEvent.KEYCODE_ESCAPE, android.view.KeyEvent.KEYCODE_BACK -> editing = null
                            android.view.KeyEvent.KEYCODE_DEL -> {
                                if (old != null) write(bindings.filterNot { it.first == old && it.second == action })
                                editing = null
                            }
                            else -> {
                                val usage = PsKeys.usage(native)
                                if (usage >= 0) {
                                    val input = PsInput(usage, null)
                                    // Une touche déjà prise ailleurs y est retirée.
                                    val rest = bindings.filterNot { it.first == input || (old != null && it.first == old && it.second == action) }
                                    write(rest + (input to action))
                                    editing = null
                                }
                            }
                        }
                        true
                    },
                ) {
                    Body(stringResource(R.string.ps_press_key_help), size = 13f, color = Kn.muted)
                    // Les clics de la souris : un bouton par clic (pas de capture de la souris dans une boîte de dialogue).
                    if (LocalForm.current != FormFactor.TV) Row(horizontalArrangement = Arrangement.spacedBy(s.dp(6f))) {
                        (0..2).forEach { b ->
                            val input = PsInput(null, b)
                            GlassButton(inputLabel(input), {
                                val rest = bindings.filterNot { it.first == input || (old != null && it.first == old && it.second == action) }
                                write(rest + (input to action))
                                editing = null
                            })
                        }
                    }
                }
                androidx.compose.runtime.LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
            },
            confirmButton = { TextButton({ editing = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}
