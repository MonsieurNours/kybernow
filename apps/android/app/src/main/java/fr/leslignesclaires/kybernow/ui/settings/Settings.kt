package fr.leslignesclaires.kybernow.ui.settings

import androidx.appcompat.app.AppCompatDelegate
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.core.os.LocaleListCompat
import fr.leslignesclaires.kybernow.BuildConfig
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalForm
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.add.shortFingerprint
import fr.leslignesclaires.kybernow.ui.home.profileLabel
import fr.leslignesclaires.kybernow.ui.panel
import fr.leslignesclaires.kybernow.update.UpdateState

@Composable
private fun Page(title: String, eyebrow: String?, onBack: () -> Unit, content: @Composable () -> Unit) {
    val s = LocalScale.current
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(s.dp(if (s.tv) 56f else 20f)).widthIn(max = s.dp(860f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(12f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                FocusCard(onClick = onBack) {
                    Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(10f))) { KnIcon(KnIcons.back, size = 20f) }
                }
                Spacer(Modifier.width(s.dp(14f)))
                Column {
                    eyebrow?.let { Eyebrow(it) }
                    Title(title, size = if (s.tv) 40f else 26f)
                }
            }
            content()
        }
    }
}

@Composable
fun Section(title: String) {
    val s = LocalScale.current
    Spacer(Modifier.height(s.dp(10f)))
    Eyebrow(title, color = Kn.spectre)
}

/** Une rangée de choix : libellé, aide, et les options en pastilles. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun <T> ChoiceRow(label: String, help: String?, options: List<Pair<T, String>>, selected: T, enabled: Boolean = true, onSelect: (T) -> Unit) {
    val s = LocalScale.current
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Body(label, size = 16f, color = if (enabled) Kn.frost else Kn.frost2)
        help?.let { Body(it, size = 13f, color = Kn.muted) }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            options.forEach { (v, l) -> GlassButton(l, { onSelect(v) }, selected = v == selected, enabled = enabled) }
        }
    }
}

/** Une rangée oui / non. */
@Composable
fun SwitchRow(label: String, help: String?, checked: Boolean, enabled: Boolean = true, onChange: (Boolean) -> Unit) {
    val s = LocalScale.current
    FocusCard({ if (enabled) onChange(!checked) }, Modifier.fillMaxWidth()) {
        Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
                Body(label, size = 16f, color = if (enabled) Kn.frost else Kn.frost2)
                help?.let { Body(it, size = 13f, color = Kn.muted) }
            }
            Spacer(Modifier.width(s.dp(14f)))
            Box(
                Modifier.width(s.dp(46f)).height(s.dp(26f)).background(if (checked && enabled) Kn.spectreFlat else Kn.frost.copy(alpha = 0.14f), androidx.compose.foundation.shape.RoundedCornerShape(50)),
                contentAlignment = if (checked) Alignment.CenterEnd else Alignment.CenterStart,
            ) {
                Box(Modifier.padding(s.dp(3f)).width(s.dp(20f)).height(s.dp(20f)).background(Kn.neonCore, androidx.compose.foundation.shape.RoundedCornerShape(50)))
            }
        }
    }
}

/** Une rangée d'information (lecture seule, focalisable au téléviseur). */
@Composable
fun InfoRow(label: String, value: String, help: String? = null) {
    val s = LocalScale.current
    FocusCard({}, Modifier.fillMaxWidth()) {
        Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
                Body(label, size = 16f)
                help?.let { Body(it, size = 13f, color = Kn.muted) }
            }
            Mono(value, size = 13f, color = Kn.frost)
        }
    }
}

/** Réglages d'un ordinateur. */
@Composable
fun HostSettingsScreen(hostId: String, onBack: () -> Unit, onRemoved: () -> Unit) {
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val hosts by app.hosts.hosts.collectAsState()
    val host = hosts.firstOrNull { it.id == hostId } ?: return
    val set = host.settings
    val tv = LocalForm.current == FormFactor.TV
    val screen = remember { Screen.of(context) }.let { if (tv) it.copy(variableRefresh = false) else it }
    val d = app.decoders
    fun update(f: (HostSettings) -> HostSettings) = app.hosts.update(hostId) { it.copy(settings = f(it.settings)) }

    Page(stringResource(R.string.settings_title), host.name, onBack) {
        Section(stringResource(R.string.settings_profile))
        ChoiceRow(stringResource(R.string.settings_profile_default), stringResource(R.string.settings_profile_help), Profile.entries.map { it to profileLabel(it) }, set.profile) { p -> update { it.copy(profile = p) } }

        Section(stringResource(R.string.settings_image))
        val auto = stringResource(R.string.settings_auto)
        val like = stringResource(R.string.settings_like_screen)
        ChoiceRow(
            stringResource(R.string.settings_resolution), null,
            listOf(0 to like, 2160 to "4K", 1440 to "1440p", 1080 to "1080p", 720 to "720p"), set.resolution,
        ) { v -> update { it.copy(resolution = v) } }
        val refreshes = (listOf(0) + screen.modes.map { it.refresh }.filter { it >= 30 }.distinct().sortedDescending())
        ChoiceRow(
            stringResource(R.string.settings_refresh), stringResource(R.string.settings_refresh_help, screen.bestRefresh(1920, 1080), screen.bestRefresh(screen.current.width, screen.current.height)),
            refreshes.map { it to (if (it == 0) auto else "$it Hz") }, set.refresh,
        ) { v -> update { it.copy(refresh = v) } }
        ChoiceRow(
            stringResource(R.string.settings_codec), stringResource(R.string.settings_codec_help),
            listOfNotNull("auto" to auto, if (d.hevc != null) "hevc" to "HEVC" else null, "h264" to "H.264", if (d.av1 != null && host.av1) "av1" to "AV1" else null), set.codec,
        ) { v -> update { it.copy(codec = v) } }
        ChoiceRow(
            stringResource(R.string.settings_bitrate), stringResource(R.string.settings_bitrate_help),
            listOf(0 to stringResource(R.string.settings_bitrate_profile), 20 to "20", 40 to "40", 60 to "60", 90 to "90", 150 to "150", 300 to "300"), set.mbps,
        ) { v -> update { it.copy(mbps = v) } }
        InfoRow(stringResource(R.string.settings_444), if (d.hevc444) stringResource(R.string.settings_available) else stringResource(R.string.settings_unavailable), stringResource(R.string.settings_444_help))
        if (tv || screen.modes.size > 1) {
            SwitchRow(stringResource(R.string.settings_match_display), stringResource(R.string.settings_match_display_help), set.matchDisplay) { v -> update { it.copy(matchDisplay = v) } }
        }
        SwitchRow(stringResource(R.string.settings_game_mode), stringResource(if (screen.minimalPostProcessing) R.string.settings_game_mode_help else R.string.settings_game_mode_unsupported), set.gameMode && screen.minimalPostProcessing, enabled = screen.minimalPostProcessing) { v -> update { it.copy(gameMode = v) } }
        InfoRow(
            stringResource(R.string.settings_vrr),
            stringResource(if (screen.variableRefresh) R.string.settings_vrr_variable else R.string.settings_unavailable),
            stringResource(if (screen.variableRefresh) R.string.settings_vrr_help_yes else R.string.settings_vrr_help_no),
        )

        Section(stringResource(R.string.settings_sound))
        ChoiceRow(
            stringResource(R.string.settings_sound_output), stringResource(R.string.settings_sound_help),
            listOf("auto" to auto, "stereo" to stringResource(R.string.audio_stereo), "5.1" to "5.1", "7.1" to "7.1", "computer" to stringResource(R.string.settings_sound_computer)), set.audio,
        ) { v -> update { it.copy(audio = v) } }

        Section(stringResource(R.string.settings_gamepads))
        ChoiceRow(
            stringResource(R.string.settings_gamepad_seen), stringResource(R.string.settings_gamepad_seen_help),
            listOf(true to "Xbox 360", false to stringResource(R.string.settings_gamepad_faithful)), set.xbox360,
        ) { v -> update { it.copy(xbox360 = v) } }
        SwitchRow(stringResource(R.string.settings_combo), stringResource(R.string.settings_combo_help), set.combo) { v -> update { it.copy(combo = v) } }
        if (tv) SwitchRow(stringResource(R.string.settings_back_menu), stringResource(R.string.settings_back_menu_help), set.backOpensMenu) { v -> update { it.copy(backOpensMenu = v) } }
        ChoiceRow(
            stringResource(R.string.settings_guide), stringResource(R.string.settings_guide_help),
            listOf("game" to stringResource(R.string.settings_guide_game), "long" to stringResource(R.string.settings_guide_long), "system" to stringResource(R.string.settings_guide_system)), set.guide,
        ) { v -> update { it.copy(guide = v) } }

        Section(stringResource(R.string.settings_pointer))
        ChoiceRow(
            stringResource(R.string.settings_mouse), stringResource(R.string.settings_mouse_help),
            listOf(0 to auto, 1 to stringResource(R.string.mouse_desktop), 2 to stringResource(R.string.mouse_game)), set.mouse,
        ) { v -> update { it.copy(mouse = v) } }
        if (!tv) ChoiceRow(
            stringResource(R.string.settings_touch), null,
            listOf(false to stringResource(R.string.touch_trackpad), true to stringResource(R.string.touch_direct)), set.directTouch,
        ) { v -> update { it.copy(directTouch = v) } }
        SwitchRow(stringResource(R.string.settings_clipboard), stringResource(R.string.settings_clipboard_help), set.clipboard) { v -> update { it.copy(clipboard = v) } }

        Section(stringResource(R.string.settings_network))
        SwitchRow(stringResource(R.string.settings_reliable), stringResource(R.string.settings_reliable_help), set.reliable) { v -> update { it.copy(reliable = v) } }
        SwitchRow(stringResource(R.string.settings_reconnect), null, set.reconnect) { v -> update { it.copy(reconnect = v) } }
        MacRow(host)

        Section(stringResource(R.string.settings_account))
        InfoRow(stringResource(R.string.settings_address), "${host.address}:${host.port}", host.fingerprint?.let { shortFingerprint(it) })
        AccountRow(host)
        FocusCard({ app.hosts.remove(host.id); onRemoved() }, Modifier.fillMaxWidth()) {
            Row(Modifier.panel().fillMaxWidth().padding(LocalScale.current.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
                KnIcon(KnIcons.trash, tint = Kn.warn)
                Spacer(Modifier.width(LocalScale.current.dp(12f)))
                Body(stringResource(R.string.settings_remove), color = Kn.warn, size = 16f)
            }
        }
    }
}

@Composable
private fun AccountRow(host: Host) {
    val s = LocalScale.current
    val app = KyberNowApp.instance
    var user by remember(host.id) { mutableStateOf(host.user) }
    var password by remember(host.id) { mutableStateOf("") }
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Body(stringResource(R.string.settings_account_help), size = 13f, color = Kn.muted)
        OutlinedTextField(user, { user = it }, label = { Text(stringResource(R.string.add_user)) }, singleLine = true, modifier = Modifier.fillMaxWidth(), colors = colors())
        OutlinedTextField(password, { password = it }, label = { Text(stringResource(R.string.settings_new_password)) }, singleLine = true, modifier = Modifier.fillMaxWidth(), visualTransformation = PasswordVisualTransformation(), colors = colors())
        GlassButton(stringResource(R.string.add_save), {
            app.hosts.update(host.id) { it.copy(user = user) }
            if (password.isNotEmpty()) app.hosts.setPassword(host.id, password)
            password = ""
        }, enabled = user.isNotBlank())
    }
}

@Composable
private fun MacRow(host: Host) {
    val s = LocalScale.current
    val app = KyberNowApp.instance
    var mac by remember(host.id) { mutableStateOf(host.mac ?: "") }
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Body(stringResource(R.string.settings_wake), size = 16f)
        Body(stringResource(R.string.settings_wake_help), size = 13f, color = Kn.muted)
        OutlinedTextField(mac, { mac = it.uppercase() }, label = { Text(stringResource(R.string.settings_mac)) }, singleLine = true, modifier = Modifier.fillMaxWidth(), colors = colors())
        GlassButton(stringResource(R.string.add_save), {
            val clean = mac.filter { it.isLetterOrDigit() }
            app.hosts.update(host.id) { it.copy(mac = if (clean.length == 12) clean.chunked(2).joinToString(":") else null) }
        })
    }
}

/** État de la mise à jour de l'app et son bouton (rechercher, télécharger, installer). */
@Composable
private fun UpdateRow() {
    val s = LocalScale.current
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val st by app.updates.state.collectAsState()
    val tv = LocalForm.current == FormFactor.TV
    fun mb(b: Long) = android.text.format.Formatter.formatShortFileSize(context, b)
    val status = when (val u = st) {
        UpdateState.Idle -> stringResource(R.string.updates_idle)
        UpdateState.Checking -> stringResource(R.string.updates_checking)
        is UpdateState.UpToDate -> stringResource(R.string.updates_up_to_date)
        is UpdateState.Available -> stringResource(R.string.updates_available, u.version) + " · " + mb(u.size)
        is UpdateState.Downloading -> stringResource(R.string.updates_downloading, u.version, mb(u.received), mb(u.size))
        is UpdateState.Ready -> stringResource(R.string.updates_ready, u.version)
        is UpdateState.NeedsPermission -> stringResource(if (tv) R.string.updates_permission_tv else R.string.updates_permission)
        is UpdateState.Installing -> stringResource(R.string.updates_installing, u.version)
        is UpdateState.Failed -> stringResource(R.string.updates_failed, u.message)
    }
    val button = when (st) {
        is UpdateState.Available -> stringResource(R.string.updates_download)
        is UpdateState.Ready, is UpdateState.NeedsPermission -> stringResource(R.string.updates_install)
        is UpdateState.Checking, is UpdateState.Downloading, is UpdateState.Installing -> null
        else -> stringResource(R.string.updates_check)
    }
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
                Body(stringResource(R.string.updates_title, BuildConfig.VERSION_NAME), size = 16f)
                Body(status, size = 13f, color = if (st is UpdateState.Failed) Kn.warn else if (st.offered != null) Kn.spectre else Kn.muted)
            }
            if (st is UpdateState.Checking || st is UpdateState.Downloading || st is UpdateState.Installing) {
                androidx.compose.material3.CircularProgressIndicator(Modifier.width(s.dp(22f)).height(s.dp(22f)), color = Kn.spectre, strokeWidth = s.dp(2f))
            }
        }
        (st as? UpdateState.Downloading)?.let { d ->
            androidx.compose.material3.LinearProgressIndicator(
                progress = { if (d.size > 0) d.received.toFloat() / d.size else 0f },
                modifier = Modifier.fillMaxWidth(), color = Kn.spectre, trackColor = Kn.line,
            )
        }
        (st as? UpdateState.Available)?.notes?.takeIf { it.isNotBlank() }?.let { notes ->
            Eyebrow(stringResource(R.string.updates_notes))
            Body(notes.trim(), size = 13f, color = Kn.frost2, maxLines = 12)
        }
        Body(stringResource(R.string.updates_signed), size = 12f, color = Kn.muted)
        button?.let { GlassButton(it, { app.updates.act() }, selected = st.offered != null) }
    }
}

@Composable
private fun colors() = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, focusedLabelColor = Kn.spectre, cursorColor = Kn.spectre)

/** Réglages de l'app : langue, appareil, mises à jour, à propos. */
@Composable
fun AppSettingsScreen(onBack: () -> Unit) {
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val screen = remember { Screen.of(context) }
    val d = app.decoders
    val current = AppCompatDelegate.getApplicationLocales().let { if (it.isEmpty) "auto" else it[0]!!.language }
    Page(stringResource(R.string.app_settings_title), null, onBack) {
        Section(stringResource(R.string.app_settings_language))
        ChoiceRow(
            stringResource(R.string.app_settings_language), stringResource(R.string.app_settings_language_help),
            listOf("auto" to stringResource(R.string.settings_auto), "fr" to "Français", "en" to "English"), current,
        ) { v -> AppCompatDelegate.setApplicationLocales(if (v == "auto") LocaleListCompat.getEmptyLocaleList() else LocaleListCompat.forLanguageTags(v)) }

        Section(stringResource(R.string.app_settings_device))
        InfoRow(stringResource(R.string.app_settings_device_name), app.device.name, app.device.os)
        InfoRow(stringResource(R.string.app_settings_decoders), listOfNotNull("H.264".takeIf { d.h264 != null }, "HEVC".takeIf { d.hevc != null }, "AV1".takeIf { d.av1 != null }).joinToString(" · "), (d.hevc ?: d.h264)?.name)
        InfoRow(stringResource(R.string.app_settings_screen), "${screen.current.width} × ${screen.current.height} · ${screen.current.refresh} Hz", screen.modes.map { "${it.height}p${it.refresh}" }.distinct().joinToString(" · "))
        InfoRow(stringResource(R.string.settings_game_mode), stringResource(if (screen.minimalPostProcessing) R.string.settings_available else R.string.settings_unavailable))

        Section(stringResource(R.string.updates_section))
        UpdateRow()
        val beta by app.updates.beta.collectAsState()
        ChoiceRow(
            stringResource(R.string.updates_channel), stringResource(R.string.updates_channel_help),
            listOf(false to stringResource(R.string.updates_stable), true to stringResource(R.string.updates_beta)), beta,
        ) { v -> app.updates.setBeta(v) }

        Section(stringResource(R.string.app_settings_about))
        InfoRow("KyberNow Link", BuildConfig.VERSION_NAME, stringResource(R.string.app_settings_about_body))
        InfoRow(stringResource(R.string.app_settings_licenses), "AGPL-3.0", stringResource(R.string.app_settings_licenses_body))
    }
}
