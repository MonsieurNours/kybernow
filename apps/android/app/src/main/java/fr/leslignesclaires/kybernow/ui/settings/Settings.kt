package fr.leslignesclaires.kybernow.ui.settings

import android.content.Intent
import android.net.Uri
import androidx.appcompat.app.AppCompatDelegate
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.core.os.LocaleListCompat
import fr.leslignesclaires.kybernow.BuildConfig
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.ConsoleClient
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.model.AudioOutput
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.ui.AppModel
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FitTitle
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalForm
import fr.leslignesclaires.kybernow.ui.LocalMotion
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.QrCode
import fr.leslignesclaires.kybernow.ui.StationAction
import fr.leslignesclaires.kybernow.ui.StationTask
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.home.profileHelp
import fr.leslignesclaires.kybernow.ui.home.profileLabel
import fr.leslignesclaires.kybernow.ui.home.settingsTitle
import fr.leslignesclaires.kybernow.ui.panel
import fr.leslignesclaires.kybernow.update.UpdateState

// MARK: - Briques

/** En-tête d'une page de réglages : retour et titre (une ligne, qui rapetisse pour tenir). */
@Composable
private fun Header(title: String, onBack: () -> Unit) {
    val s = LocalScale.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        FocusCard(onClick = onBack) {
            Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(10f))) { KnIcon(KnIcons.back, size = 20f) }
        }
        Spacer(Modifier.width(s.dp(14f)))
        FitTitle(title, Modifier.fillMaxWidth(), size = if (s.tv) 34f else 26f, min = 16f)
    }
}

@Composable
private fun Page(title: String, onBack: () -> Unit, content: @Composable () -> Unit) {
    val s = LocalScale.current
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(s.dp(if (s.tv) 56f else 20f)).widthIn(max = s.dp(860f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(12f)),
        ) {
            Header(title, onBack)
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
                Modifier.width(s.dp(46f)).height(s.dp(26f)).background(if (checked && enabled) Kn.spectreFlat else Kn.frost.copy(alpha = 0.14f), RoundedCornerShape(50)),
                contentAlignment = if (checked) Alignment.CenterEnd else Alignment.CenterStart,
            ) {
                Box(Modifier.padding(s.dp(3f)).width(s.dp(20f)).height(s.dp(20f)).background(Kn.neonCore, RoundedCornerShape(50)))
            }
        }
    }
}

/** Une rangée d'information (lecture seule, focalisable au téléviseur) ; au téléphone, la valeur sous le libellé. */
@Composable
fun InfoRow(label: String, value: String, help: String? = null) {
    val s = LocalScale.current
    if (LocalForm.current == FormFactor.PHONE) {
        Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
            Body(label, size = 16f)
            Mono(value, size = 13f, color = Kn.frost)
            help?.let { Body(it, size = 13f, color = Kn.muted) }
        }
        return
    }
    FocusCard({}, Modifier.fillMaxWidth()) {
        Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
                Body(label, size = 16f)
                help?.let { Body(it, size = 13f, color = Kn.muted) }
            }
            Spacer(Modifier.width(s.dp(12f)))
            Mono(value, Modifier.widthIn(max = s.dp(260f)), size = 13f, color = Kn.frost)
        }
    }
}

/** Une action : son bouton, et ce qu'elle fait en une phrase. */
@Composable
private fun ActionRow(label: String, help: String?, enabled: Boolean = true, danger: Boolean = false, onClick: () -> Unit) {
    val s = LocalScale.current
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        GlassButton(label, onClick, enabled = enabled, leading = if (danger) ({ KnIcon(KnIcons.trash, tint = Kn.warn, size = 16f); Spacer(Modifier.width(6.dp)) }) else null)
        help?.let { Body(it, size = 13f, color = Kn.muted) }
    }
}

@Composable
private fun colors() = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, focusedLabelColor = Kn.spectre, cursorColor = Kn.spectre)

// MARK: - Réglages d'un ordinateur

/** Les six rubriques des réglages d'un ordinateur, dans l'ordre de docs/link-ui.md. */
enum class Rubric(val title: Int, val icon: ImageVector) {
    PICTURE(R.string.settings_image, KnIcons.screen),
    SOUND(R.string.settings_sound, KnIcons.sound),
    CONTROLS(R.string.settings_controls, KnIcons.gamepad),
    NETWORK(R.string.settings_network, KnIcons.network),
    STATS(R.string.settings_stats, KnIcons.pulse),
    STATION(R.string.settings_station, KnIcons.power),
}

/** La Station a été réinstallée : l'empreinte qu'Admin présente maintenant ([seen] nul : Admin ne répond pas). */
data class FingerprintCheck(val seen: String?, val reading: Boolean = false)

/** Tout ce que les réglages d'un ordinateur montrent. */
data class HostSettingsState(
    val host: Host,
    val status: HostStatus,
    val screen: Screen,
    val decoders: Decoders,
    /** Adresse d'Admin, `https://adresse:port/` (lien, QR code). */
    val adminUrl: String,
    val station: StationTask? = null,
    val fingerprint: FingerprintCheck? = null,
    /** L'instant présent, pour « hier » (fixé dans les captures). */
    val now: Long = System.currentTimeMillis(),
    /** L'appareil a un vibreur (retour haptique de la manette à l'écran). */
    val vibrator: Boolean = true,
)

/** Ce que les réglages d'un ordinateur demandent à l'app. */
class HostSettingsActions(
    val update: ((HostSettings) -> HostSettings) -> Unit,
    val saveAccount: (user: String, password: String) -> Unit,
    val saveMac: (String?) -> Unit,
    val openAdmin: () -> Unit,
    val station: (StationAction, Boolean) -> Unit,
    val stationDismiss: () -> Unit,
    val trustFingerprint: (String) -> Unit,
    val remove: () -> Unit,
    val back: () -> Unit,
)

/** Réglages d'un ordinateur, branchés sur l'app. */
@Composable
fun HostSettingsScreen(model: AppModel, hostId: String, onBack: () -> Unit, onRemoved: () -> Unit) {
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val hosts by model.hosts.collectAsState()
    val status by model.status.collectAsState()
    val tasks by model.station.collectAsState()
    val host = hosts.firstOrNull { it.id == hostId } ?: return
    val st = status[hostId] ?: HostStatus()
    val screen = remember { Screen.of(context) }
    val vibrator = remember { fr.leslignesclaires.kybernow.session.Haptics.available(context) }
    val adminUrl = remember(host.address, host.port) {
        ConsoleClient(host.address, host.port).use { it.adminUrl } ?: "https://${host.address}:${host.port}/"
    }
    // Certificat changé : l'empreinte présentée maintenant, à comparer à celle d'Admin.
    var check by remember { mutableStateOf<FingerprintCheck?>(null) }
    LaunchedEffect(st.certificateChanged, host.fingerprint) {
        check = if (st.certificateChanged) {
            check = FingerprintCheck(null, reading = true)
            FingerprintCheck(model.seenFingerprint(hostId))
        } else null
    }
    val actions = HostSettingsActions(
        update = { f -> app.hosts.update(hostId) { it.copy(settings = f(it.settings)) } },
        saveAccount = { user, password ->
            app.hosts.update(hostId) { it.copy(user = user) }
            if (password.isNotEmpty()) app.hosts.setPassword(hostId, password)
            model.retry(hostId)
        },
        saveMac = { mac -> app.hosts.update(hostId) { it.copy(mac = mac) } },
        openAdmin = { runCatching { context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(adminUrl))) } },
        station = { a, force -> model.station(hostId, a, force) },
        stationDismiss = { model.stationDismiss(hostId) },
        trustFingerprint = { fp -> model.trustFingerprint(hostId, fp) },
        remove = { model.remove(hostId); onRemoved() },
        back = onBack,
    )
    val start = when {
        st.certificateChanged -> Rubric.STATS
        st.accountRefused -> Rubric.STATION
        else -> Rubric.PICTURE
    }
    HostSettingsContent(HostSettingsState(host, st, screen, app.decoders, adminUrl, tasks[hostId], check, vibrator = vibrator), actions, start)
}

/**
 * Réglages d'un ordinateur (docs/link-ui.md) : « Réglages de {nom} », le profil en tête, puis
 * Image, Son, Commandes, Réseau, Statistiques, Station. Au téléviseur et à la tablette, les
 * rubriques en colonne à gauche (le focus en choisit une) et leur contenu à droite ; au
 * téléphone, une page qui défile, rubrique après rubrique. [only] : une seule rubrique au
 * téléphone (captures).
 */
@Composable
fun HostSettingsContent(state: HostSettingsState, actions: HostSettingsActions, start: Rubric = Rubric.PICTURE, only: Rubric? = null) {
    if (LocalForm.current == FormFactor.PHONE) PhoneSettings(state, actions, only) else PaneSettings(state, actions, start)
}

@Composable
private fun PhoneSettings(state: HostSettingsState, actions: HostSettingsActions, only: Rubric?) {
    val s = LocalScale.current
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(s.dp(20f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(12f)),
        ) {
            Header(settingsTitle(state.host.name), actions.back)
            if (only == null) {
                Section(stringResource(R.string.settings_profile))
                ProfileRow(state.host.settings, actions)
            }
            Rubric.entries.filter { only == null || it == only }.forEach { r ->
                Section(stringResource(r.title))
                RubricContent(r, state, actions)
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PaneSettings(state: HostSettingsState, actions: HostSettingsActions, start: Rubric) {
    val s = LocalScale.current
    var rubric by remember { mutableStateOf(start) }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (s.tv) runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().padding(horizontal = s.dp(if (s.tv) 48f else 32f), vertical = s.dp(if (s.tv) 28f else 24f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(14f)),
        ) {
            Header(settingsTitle(state.host.name), actions.back)
            ProfileBar(state.host.settings, actions)
            Row(Modifier.weight(1f).fillMaxWidth()) {
                Column(Modifier.width(s.dp(if (s.tv) 230f else 220f)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
                    Rubric.entries.forEach { r ->
                        val selected = r == rubric
                        FocusCard(
                            { rubric = r },
                            Modifier.fillMaxWidth().then(if (selected) Modifier.focusRequester(first) else Modifier)
                                .onFocusChanged { if (it.isFocused && s.tv) rubric = r },
                            shape = RoundedCornerShape(12.dp),
                        ) {
                            Row(
                                Modifier.fillMaxWidth().background(if (selected) Kn.spectre.copy(alpha = 0.16f) else Kn.panel.copy(alpha = 0.5f))
                                    .padding(horizontal = s.dp(14f), vertical = s.dp(10f)),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                KnIcon(r.icon, tint = if (selected) Kn.spectre else Kn.frost2, size = 18f)
                                Spacer(Modifier.width(s.dp(12f)))
                                Body(stringResource(r.title), size = 15f, weight = if (selected) 600 else 400, color = if (selected) Kn.frost else Kn.frost2)
                            }
                        }
                    }
                }
                Spacer(Modifier.width(s.dp(24f)))
                Column(
                    Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(bottom = s.dp(12f)),
                    verticalArrangement = Arrangement.spacedBy(s.dp(12f)),
                ) {
                    RubricContent(rubric, state, actions)
                }
            }
        }
    }
}

/** Le profil (Jeu, Bureau, Max ; « Personnalisé » quand les réglages s'en écartent), au téléphone. */
@Composable
private fun ProfileRow(set: HostSettings, actions: HostSettingsActions) {
    val options: List<Pair<Profile?, String>> = Profile.entries.map { it to profileLabel(it) } +
        if (set.custom) listOf<Pair<Profile?, String>>(null to stringResource(R.string.profile_custom)) else emptyList()
    ChoiceRow(stringResource(R.string.settings_profile), profileHelp(set), options, if (set.custom) null else set.profile) { p ->
        p?.let { actions.update { it.apply(p) } }
    }
}

/** Le profil en une ligne, au-dessus des rubriques (téléviseur, tablette). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ProfileBar(set: HostSettings, actions: HostSettingsActions) {
    val s = LocalScale.current
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(s.dp(10f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(8f)),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        Eyebrow(stringResource(R.string.settings_profile), color = Kn.spectre, modifier = Modifier.padding(end = s.dp(4f)))
        Profile.entries.forEach { p -> GlassButton(profileLabel(p), { actions.update { it.apply(p) } }, selected = !set.custom && set.profile == p) }
        if (set.custom) GlassButton(stringResource(R.string.profile_custom), {}, selected = true)
        Body(profileHelp(set), size = 14f, color = Kn.muted, modifier = Modifier.padding(start = s.dp(6f)))
    }
}

@Composable
private fun RubricContent(r: Rubric, state: HostSettingsState, actions: HostSettingsActions) {
    when (r) {
        Rubric.PICTURE -> PictureRubric(state, actions)
        Rubric.SOUND -> SoundRubric(state, actions)
        Rubric.CONTROLS -> ControlsRubric(state, actions)
        Rubric.NETWORK -> NetworkRubric(state, actions)
        Rubric.STATS -> StatsRubric(state, actions)
        Rubric.STATION -> StationRubric(state, actions)
    }
}

@Composable
private fun PictureRubric(state: HostSettingsState, actions: HostSettingsActions) {
    val set = state.host.settings
    val tv = LocalForm.current == FormFactor.TV
    val screen = state.screen
    val d = state.decoders
    val auto = stringResource(R.string.settings_auto)
    ChoiceRow(
        stringResource(R.string.settings_resolution), null,
        listOf(0 to stringResource(R.string.settings_like_screen), 2160 to "4K", 1440 to "1440p", 1080 to "1080p", 720 to "720p"), set.resolution,
    ) { v -> actions.update { it.copy(resolution = v) } }
    val refreshes = listOf(0) + screen.modes.map { it.refresh }.filter { it >= 30 }.distinct().sortedDescending()
    ChoiceRow(
        stringResource(R.string.settings_refresh), stringResource(R.string.settings_refresh_help, screen.bestRefresh(1920, 1080), screen.bestRefresh(screen.current.width, screen.current.height)),
        refreshes.map { it to (if (it == 0) auto else "$it Hz") }, set.refresh,
    ) { v -> actions.update { it.copy(refresh = v) } }
    SwitchRow(
        stringResource(R.string.settings_virtual_display),
        stringResource(if (state.host.virtualDisplay) R.string.settings_virtual_display_help else R.string.settings_virtual_display_none),
        set.virtualDisplay && state.host.virtualDisplay, enabled = state.host.virtualDisplay,
    ) { v -> actions.update { it.copy(virtualDisplay = v) } }
    ChoiceRow(
        stringResource(R.string.settings_codec), stringResource(R.string.settings_codec_help),
        listOfNotNull("auto" to auto, if (d.hevc != null) "hevc" to "HEVC" else null, "h264" to "H.264", if (d.av1 != null && state.host.av1) "av1" to "AV1" else null), set.codec,
    ) { v -> actions.update { it.copy(codec = v) } }
    ChoiceRow(
        stringResource(R.string.settings_bitrate), stringResource(R.string.settings_bitrate_help),
        listOf(0 to auto) + listOf(20, 40, 60, 90, 150, 300).map { it to stringResource(R.string.settings_mbps, it) }, set.mbps,
    ) { v -> actions.update { it.copy(mbps = v) } }
    // Le 4:4:4 vient du profil (Bureau, Max) quand l'appareil le décode : pas de choix à faire ici.
    InfoRow(stringResource(R.string.settings_444), stringResource(if (d.hevc444) R.string.settings_444_yes else R.string.settings_444_no), stringResource(R.string.settings_444_help))
    InfoRow(stringResource(R.string.settings_frame_display), stringResource(R.string.settings_frame_immediate), stringResource(R.string.settings_frame_display_help))
    // Au téléviseur seulement : la sortie HDMI change de mode. Le téléphone et la tablette prennent
    // déjà la fréquence de la session à leur définition (SessionActivity.applyDisplay).
    if (tv) {
        SwitchRow(stringResource(R.string.settings_match_display), stringResource(R.string.settings_match_display_help), set.matchDisplay) { v -> actions.update { it.copy(matchDisplay = v) } }
    }
    if (tv || screen.minimalPostProcessing) {
        SwitchRow(
            stringResource(R.string.settings_game_mode),
            stringResource(if (screen.minimalPostProcessing) R.string.settings_game_mode_help else R.string.settings_game_mode_unsupported),
            set.gameMode && screen.minimalPostProcessing, enabled = screen.minimalPostProcessing,
        ) { v -> actions.update { it.copy(gameMode = v) } }
    }
}

@Composable
private fun SoundRubric(state: HostSettingsState, actions: HostSettingsActions) {
    ChoiceRow(
        stringResource(R.string.settings_sound), stringResource(R.string.settings_sound_help),
        listOf(
            "auto" to stringResource(R.string.settings_auto), "stereo" to stringResource(R.string.audio_stereo), "5.1" to "5.1", "7.1" to "7.1",
            "computer" to stringResource(R.string.settings_sound_computer),
        ),
        state.host.settings.audio,
    ) { v -> actions.update { it.copy(audio = v) } }
}

@Composable
private fun ControlsRubric(state: HostSettingsState, actions: HostSettingsActions) {
    val set = state.host.settings
    val tv = LocalForm.current == FormFactor.TV
    ChoiceRow(
        stringResource(R.string.settings_gamepad_seen), stringResource(R.string.settings_gamepad_seen_help),
        listOf(true to "Xbox 360", false to stringResource(R.string.settings_gamepad_faithful)), set.xbox360,
    ) { v -> actions.update { it.copy(xbox360 = v) } }
    SwitchRow(stringResource(R.string.settings_combo), stringResource(R.string.settings_combo_help), set.combo) { v -> actions.update { it.copy(combo = v) } }
    if (tv) SwitchRow(stringResource(R.string.settings_back_menu), stringResource(R.string.settings_back_menu_help), set.backOpensMenu) { v -> actions.update { it.copy(backOpensMenu = v) } }
    ChoiceRow(
        stringResource(R.string.settings_guide), stringResource(R.string.settings_guide_help),
        listOf("game" to stringResource(R.string.settings_guide_game), "long" to stringResource(R.string.settings_guide_long), "system" to stringResource(R.string.settings_guide_system)), set.guide,
    ) { v -> actions.update { it.copy(guide = v) } }
    ChoiceRow(
        stringResource(R.string.settings_mouse), stringResource(R.string.settings_mouse_help),
        listOf(0 to stringResource(R.string.settings_auto), 1 to stringResource(R.string.mouse_desktop), 2 to stringResource(R.string.mouse_game)), set.mouse,
    ) { v -> actions.update { it.copy(mouse = v) } }
    if (!tv) {
        ChoiceRow(
            stringResource(R.string.settings_touch), null,
            listOf(false to stringResource(R.string.touch_trackpad), true to stringResource(R.string.touch_direct)), set.directTouch,
        ) { v -> actions.update { it.copy(directTouch = v) } }
        // Le presse-papiers ne passe pas au téléviseur (SessionActivity.start).
        SwitchRow(stringResource(R.string.settings_clipboard), stringResource(R.string.settings_clipboard_help), set.clipboard) { v -> actions.update { it.copy(clipboard = v) } }
        OnScreenPadRows(state, actions)
    }
}

/** La manette à l'écran (téléphone, tablette ; docs/link-ui.md) : affichée, disposition, taille, opacité, retour haptique. */
@Composable
private fun OnScreenPadRows(state: HostSettingsState, actions: HostSettingsActions) {
    val set = state.host.settings
    SwitchRow(stringResource(R.string.pad_on_screen), stringResource(R.string.pad_on_screen_help), set.onScreenPad) { v -> actions.update { it.copy(onScreenPad = v) } }
    ChoiceRow(
        stringResource(R.string.pad_layout), null,
        listOf("full" to stringResource(R.string.pad_layout_full), "simple" to stringResource(R.string.pad_layout_simple)), set.padLayout,
    ) { v -> actions.update { it.copy(padLayout = v) } }
    ChoiceRow(
        stringResource(R.string.pad_size), null,
        listOf(0 to stringResource(R.string.pad_size_small), 1 to stringResource(R.string.pad_size_medium), 2 to stringResource(R.string.pad_size_large)), set.padSize,
    ) { v -> actions.update { it.copy(padSize = v) } }
    ChoiceRow(
        stringResource(R.string.pad_opacity), null,
        HostSettings.PAD_OPACITIES.map { it to stringResource(R.string.settings_percent, it) }, set.padOpacity,
    ) { v -> actions.update { it.copy(padOpacity = v) } }
    if (state.vibrator) {
        ChoiceRow(
            stringResource(R.string.pad_haptics), null,
            listOf(
                0 to stringResource(R.string.pad_haptics_off), 1 to stringResource(R.string.pad_haptics_light),
                2 to stringResource(R.string.pad_haptics_medium), 3 to stringResource(R.string.pad_haptics_strong),
            ),
            set.padHaptics,
        ) { v -> actions.update { it.copy(padHaptics = v) } }
    } else {
        // Sans vibreur, pas de réglage : une ligne le dit, seul le retour visuel reste.
        val s = LocalScale.current
        Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
            Body(stringResource(R.string.pad_haptics), size = 16f, color = Kn.frost2)
            Body(stringResource(R.string.pad_haptics_none), size = 13f, color = Kn.muted)
        }
    }
}

@Composable
private fun NetworkRubric(state: HostSettingsState, actions: HostSettingsActions) {
    val set = state.host.settings
    ChoiceRow(
        stringResource(R.string.settings_transport), stringResource(R.string.settings_transport_help),
        listOf(false to stringResource(R.string.settings_transport_fast), true to stringResource(R.string.settings_transport_reliable)), set.reliable,
    ) { v -> actions.update { it.withTransport(v) } }
    SwitchRow(stringResource(R.string.settings_reconnect), null, set.reconnect) { v -> actions.update { it.copy(reconnect = v) } }
    WakeRow(state.host, actions)
}

/**
 * « Réveil par le réseau » : « Prêt » et l'adresse MAC apprise d'Admin, ou « Pas encore
 * d'adresse » ; en réglage avancé, une adresse MAC saisie à la main (Station d'avant la 0.5.0).
 */
@Composable
private fun WakeRow(host: Host, actions: HostSettingsActions) {
    val s = LocalScale.current
    var mac by remember(host.id) { mutableStateOf(host.mac ?: "") }
    var manual by remember(host.id) { mutableStateOf(host.mac != null) }
    val target = host.wakeTargets.firstOrNull()
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Body(stringResource(R.string.settings_wake), Modifier.weight(1f), size = 16f)
            if (target != null) Body(stringResource(R.string.settings_wake_ready), size = 15f, color = Kn.spectre, weight = 600)
        }
        if (target != null) Mono(target.mac.uppercase(), size = 13f, color = Kn.frost)
        else Body(stringResource(R.string.settings_wake_none), size = 14f, color = Kn.frost2)
        Body(stringResource(R.string.settings_wake_help), size = 13f, color = Kn.muted)
        if (!manual) {
            GlassButton(stringResource(R.string.settings_mac_enter), { manual = true })
        } else {
            Body(stringResource(R.string.settings_mac_help), size = 13f, color = Kn.muted)
            OutlinedTextField(mac, { mac = it.uppercase() }, label = { Text(stringResource(R.string.settings_mac)) }, singleLine = true, modifier = Modifier.fillMaxWidth(), colors = colors())
            GlassButton(stringResource(R.string.add_save), {
                val clean = mac.filter { it.isLetterOrDigit() }
                actions.saveMac(if (clean.length == 12) clean.chunked(2).joinToString(":") else null)
            })
        }
    }
}

// MARK: - Statistiques

@Composable
private fun StatsRubric(state: HostSettingsState, actions: HostSettingsActions) {
    val s = LocalScale.current
    val host = state.host
    state.host.last?.let { LastSessionCard(it, state.now) } ?: Column(Modifier.panel().fillMaxWidth().padding(s.dp(18f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        Eyebrow(stringResource(R.string.last_title))
        Body(stringResource(R.string.last_none), size = 15f, color = Kn.frost2)
    }
    Spacer(Modifier.height(s.dp(4f)))
    Eyebrow(stringResource(R.string.computer_title), color = Kn.spectre)
    val info = state.status.info
    Column(Modifier.panel().fillMaxWidth().padding(horizontal = s.dp(18f), vertical = s.dp(8f))) {
        Fact(stringResource(R.string.computer_system), info?.os?.takeIf { it.isNotBlank() } ?: host.os ?: "—")
        Fact(stringResource(R.string.computer_gpu), info?.gpu ?: host.gpu ?: "—")
        Fact(stringResource(R.string.computer_address), "${host.address}:${host.port}", mono = true)
        Fact(stringResource(R.string.computer_fingerprint), host.fingerprint?.let { shortFingerprint(it) } ?: "—", mono = true)
    }
    if (state.status.certificateChanged) FingerprintPanel(state.fingerprint, actions)
}

@Composable
private fun Fact(label: String, value: String, mono: Boolean = false) {
    val s = LocalScale.current
    Row(Modifier.fillMaxWidth().padding(vertical = s.dp(8f))) {
        Body(label, Modifier.weight(0.38f), size = 14f, color = Kn.muted)
        if (mono) Mono(value, Modifier.weight(0.62f), size = 13f, color = Kn.frost)
        else Body(value, Modifier.weight(0.62f), size = 15f)
    }
}

/** Les quatre premiers groupes d'une empreinte, comme dans Admin. */
private fun shortFingerprint(fp: String) = fr.leslignesclaires.kybernow.ui.add.shortFingerprint(fp)

/** La Station a été réinstallée : la nouvelle empreinte, à comparer à celle d'Admin avant de l'épingler. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun FingerprintPanel(check: FingerprintCheck?, actions: HostSettingsActions) {
    val s = LocalScale.current
    var refused by remember { mutableStateOf(false) }
    Column(
        Modifier.panel().border(BorderStroke(1.dp, Kn.warn.copy(alpha = 0.5f)), RoundedCornerShape(18.dp)).fillMaxWidth().padding(s.dp(16f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(10f)),
    ) {
        Body(stringResource(R.string.computer_fingerprint_help), size = 14f, color = Kn.warn)
        val seen = check?.seen
        when {
            check == null || check.reading -> Body(stringResource(R.string.computer_fingerprint_reading), size = 13f, color = Kn.muted)
            seen == null -> Body(stringResource(R.string.computer_fingerprint_unreachable), size = 13f, color = Kn.muted)
            else -> {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Body(stringResource(R.string.computer_fingerprint_new), Modifier.weight(0.38f), size = 14f, color = Kn.muted)
                    Mono(shortFingerprint(seen), Modifier.weight(0.62f), size = 15f, color = Kn.frost, weight = 500)
                }
                if (!refused) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                        GlassButton(stringResource(R.string.add_verify_same), { actions.trustFingerprint(seen) }, selected = true)
                        GlassButton(stringResource(R.string.add_verify_different), { refused = true })
                    }
                }
            }
        }
    }
}

// MARK: - Station

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StationRubric(state: HostSettingsState, actions: HostSettingsActions) {
    val s = LocalScale.current
    val tv = LocalForm.current == FormFactor.TV
    val st = state.status
    val info = st.info
    val station = info?.station
    val update = station?.update
    val task = state.station
    // Ce que la Station dit d'elle-même : un redémarrage ou une installation en cours (demandés
    // d'ici ou d'ailleurs).
    val working = task as? StationTask.Working
        ?: if (station?.restarting == true) StationTask.Working(StationAction.RESTART, null)
        else if (update?.installing == true) StationTask.Working(StationAction.UPDATE, update.version) else null
    val busy = working != null || task is StationTask.Confirm

    // Version, et ce qui est disponible.
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
        val version = station?.version?.takeIf { it.isNotEmpty() } ?: info?.adminVersion
        Body(if (version != null) stringResource(R.string.station_version, version) else "KyberNow Station", size = 16f)
        val (line, color) = when {
            st.online == null -> stringResource(R.string.home_searching) to Kn.muted
            st.away -> stringResource(R.string.home_away) to Kn.muted
            info == null -> stringResource(R.string.home_offline) to Kn.muted
            station == null -> stringResource(R.string.station_too_old) to Kn.muted
            update?.available == true && update.version != null -> stringResource(R.string.station_available, update.version) to Kn.spectre
            else -> stringResource(R.string.station_up_to_date) to Kn.muted
        }
        Body(line, size = 13f, color = color)
    }

    // Où en est ce qui a été demandé.
    when {
        working != null -> StatusPanel {
            Spinner(20f)
            Spacer(Modifier.width(s.dp(12f)))
            Body(stringResource(if (working.action == StationAction.RESTART) R.string.station_restarting else R.string.station_updating), size = 15f)
        }
        task is StationTask.Done -> StatusPanel {
            KnIcon(KnIcons.check, tint = Kn.spectre, size = 20f)
            Spacer(Modifier.width(s.dp(12f)))
            Body(
                if (task.action == StationAction.RESTART) stringResource(R.string.station_restarted)
                else stringResource(R.string.station_updated, task.version ?: station?.version ?: ""),
                size = 15f,
            )
        }
        task is StationTask.Failed -> Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
            Body(
                when (task.reason) {
                    fr.leslignesclaires.kybernow.ui.StationFailure.TOO_SOON -> stringResource(R.string.station_too_soon)
                    fr.leslignesclaires.kybernow.ui.StationFailure.TOO_OLD -> stringResource(R.string.station_too_old)
                    fr.leslignesclaires.kybernow.ui.StationFailure.REQUIRES_SETUP -> stringResource(R.string.station_requires_setup, task.version ?: update?.version ?: "")
                    fr.leslignesclaires.kybernow.ui.StationFailure.NOT_BACK -> stringResource(R.string.station_not_back)
                    fr.leslignesclaires.kybernow.ui.StationFailure.UPDATE_FAILED -> stringResource(R.string.station_update_failed)
                    fr.leslignesclaires.kybernow.ui.StationFailure.OTHER -> task.message.replaceFirstChar { it.uppercase() }
                },
                size = 14f, color = Kn.warn,
            )
            GlassButton(stringResource(R.string.session_close), actions.stationDismiss)
        }
        task is StationTask.Confirm -> {
            val first = remember { FocusRequester() }
            LaunchedEffect(Unit) { if (s.tv) runCatching { first.requestFocus() } }
            Column(
                Modifier.panel().border(BorderStroke(1.dp, Kn.warn.copy(alpha = 0.5f)), RoundedCornerShape(18.dp)).fillMaxWidth().padding(s.dp(16f)),
                verticalArrangement = Arrangement.spacedBy(s.dp(10f)),
            ) {
                Title(task.device?.let { stringResource(R.string.station_in_session, it) } ?: stringResource(R.string.station_in_session_unknown), size = 20f)
                Body(stringResource(if (task.action == StationAction.RESTART) R.string.station_in_session_restart else R.string.station_in_session_update), size = 14f, color = Kn.frost2)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                    GlassButton(
                        stringResource(if (task.action == StationAction.RESTART) R.string.station_restart_anyway else R.string.station_update_anyway),
                        { actions.station(task.action, true) }, Modifier.focusRequester(first), selected = true,
                    )
                    GlassButton(stringResource(R.string.cancel), actions.stationDismiss)
                }
            }
        }
    }

    // Admin : un lien au téléphone et à la tablette, un QR code au téléviseur.
    if (tv) {
        Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
            QrCode(state.adminUrl, 132f)
            Spacer(Modifier.width(s.dp(18f)))
            Column(verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                Body(stringResource(R.string.station_admin_qr), size = 15f)
                Mono(state.adminUrl, size = 13f, color = Kn.frost)
            }
        }
    } else {
        ActionRow(stringResource(R.string.station_open_admin), stringResource(R.string.station_admin_help), onClick = actions.openAdmin)
    }

    // Redémarrer, mettre à jour : tout appareil ajouté le peut, avec une Station qui le connaît.
    if (station != null && st.online == true) {
        ActionRow(stringResource(R.string.station_restart), stringResource(R.string.station_restart_help), enabled = !busy) { actions.station(StationAction.RESTART, false) }
        if (update?.available == true && update.version != null) {
            if (update.installable) {
                ActionRow(stringResource(R.string.station_update), stringResource(R.string.station_update_help, update.version), enabled = !busy) { actions.station(StationAction.UPDATE, false) }
            } else if (update.requiresSetup) {
                Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f))) {
                    Body(stringResource(R.string.station_requires_setup, update.version), size = 14f, color = Kn.frost2)
                }
            }
        }
    }

    // Ajouté par un code : l'indicateur gardé à l'ajout, ou Admin (`account.own_device`) pour ceux d'avant la 0.5.0.
    AccountRow(state.host.copy(paired = state.host.paired || info?.ownDevice == true), st.accountRefused, actions)
    RemoveRow(state.host, actions)
}

/** Un arc qui tourne (attente) ; immobile sans animations sans fin ([LocalMotion], captures). */
@Composable
private fun Spinner(size: Float) {
    val s = LocalScale.current
    val turn = if (LocalMotion.current) {
        val t = androidx.compose.animation.core.rememberInfiniteTransition(label = "attente")
        t.animateFloat(
            0f, 360f,
            androidx.compose.animation.core.infiniteRepeatable(androidx.compose.animation.core.tween(900, easing = androidx.compose.animation.core.LinearEasing)),
            label = "tour",
        ).value
    } else -90f
    androidx.compose.foundation.Canvas(Modifier.width(s.dp(size)).height(s.dp(size))) {
        val w = 2.dp.toPx()
        drawCircle(Kn.line, style = androidx.compose.ui.graphics.drawscope.Stroke(w), radius = this.size.minDimension / 2 - w)
        drawArc(
            Kn.spectre, startAngle = turn, sweepAngle = 100f, useCenter = false,
            topLeft = androidx.compose.ui.geometry.Offset(w, w),
            size = androidx.compose.ui.geometry.Size(this.size.width - 2 * w, this.size.height - 2 * w),
            style = androidx.compose.ui.graphics.drawscope.Stroke(w, cap = androidx.compose.ui.graphics.StrokeCap.Round),
        )
    }
}

@Composable
private fun StatusPanel(content: @Composable () -> Unit) {
    val s = LocalScale.current
    Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) { content() }
}

/** Le compte : l'identifiant et le mot de passe d'Admin, ou l'accès propre à cet appareil (ajouté par un code). */
@Composable
private fun AccountRow(host: Host, refused: Boolean, actions: HostSettingsActions) {
    val s = LocalScale.current
    var user by remember(host.id) { mutableStateOf(host.user) }
    var password by remember(host.id) { mutableStateOf("") }
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        // L'identifiant est une donnée (chasse fixe) ; l'accès propre, une phrase. Au téléphone, sous « Compte ».
        val value = @Composable {
            if (host.paired) Body(stringResource(R.string.station_own_access), size = 14f, color = Kn.frost2)
            else Mono(host.user, size = 13f, color = Kn.frost, maxLines = 1)
        }
        if (LocalForm.current == FormFactor.PHONE) {
            Column(verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
                Body(stringResource(R.string.station_account), size = 16f)
                value()
            }
        } else {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Body(stringResource(R.string.station_account), Modifier.weight(1f), size = 16f)
                value()
            }
        }
        if (refused) Body(stringResource(R.string.home_account_refused), size = 13f, color = Kn.warn)
        if (!host.paired || refused) {
            Body(stringResource(R.string.station_account_help), size = 13f, color = Kn.muted)
            OutlinedTextField(user, { user = it }, label = { Text(stringResource(R.string.add_user)) }, singleLine = true, modifier = Modifier.fillMaxWidth(), colors = colors())
            OutlinedTextField(password, { password = it }, label = { Text(stringResource(R.string.station_new_password)) }, singleLine = true, modifier = Modifier.fillMaxWidth(), visualTransformation = PasswordVisualTransformation(), colors = colors())
            GlassButton(stringResource(R.string.add_save), { actions.saveAccount(user, password); password = "" }, enabled = user.isNotBlank())
        }
    }
}

/** « Retirer cet ordinateur », confirmé. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RemoveRow(host: Host, actions: HostSettingsActions) {
    val s = LocalScale.current
    var asking by remember(host.id) { mutableStateOf(false) }
    if (!asking) {
        ActionRow(stringResource(R.string.station_remove), null, danger = true) { asking = true }
        return
    }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (s.tv) runCatching { first.requestFocus() } }
    Column(
        Modifier.panel().border(BorderStroke(1.dp, Kn.warn.copy(alpha = 0.5f)), RoundedCornerShape(18.dp)).fillMaxWidth().padding(s.dp(16f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(12f)),
    ) {
        Body(stringResource(R.string.station_remove_confirm, host.name), size = 15f)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            GlassButton(stringResource(R.string.station_remove_yes), actions.remove, Modifier.focusRequester(first), leading = { KnIcon(KnIcons.trash, tint = Kn.warn, size = 16f); Spacer(Modifier.width(6.dp)) })
            GlassButton(stringResource(R.string.cancel), { asking = false })
        }
    }
}

// MARK: - Réglages de l'app

/** Tout ce que les réglages de l'app montrent. */
data class AppSettingsState(
    /** `auto`, `fr`, `en`. */
    val language: String,
    val deviceName: String,
    val deviceOs: String,
    val decoders: Decoders,
    val screen: Screen,
    val audio: AudioOutput,
    val update: UpdateState,
    val beta: Boolean,
    val version: String = BuildConfig.VERSION_NAME,
)

class AppSettingsActions(
    val setLanguage: (String) -> Unit,
    val gamepads: () -> Unit,
    /** Rechercher, télécharger ou installer la mise à jour, selon l'état. */
    val updateAct: () -> Unit,
    val setBeta: (Boolean) -> Unit,
    val back: () -> Unit,
)

@Composable
fun AppSettingsScreen(onBack: () -> Unit, onGamepads: () -> Unit) {
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val screen = remember { Screen.of(context) }
    val audio = remember { AudioOutput.current(context) }
    val update by app.updates.state.collectAsState()
    val beta by app.updates.beta.collectAsState()
    val language = AppCompatDelegate.getApplicationLocales().let { if (it.isEmpty) "auto" else it[0]!!.language }
    AppSettingsContent(
        AppSettingsState(language, app.device.name, app.device.os, app.decoders, screen, audio, update, beta),
        AppSettingsActions(
            setLanguage = { v -> AppCompatDelegate.setApplicationLocales(if (v == "auto") LocaleListCompat.getEmptyLocaleList() else LocaleListCompat.forLanguageTags(v)) },
            gamepads = onGamepads,
            updateAct = { app.updates.act() },
            setBeta = { app.updates.setBeta(it) },
            back = onBack,
        ),
    )
}

/**
 * Réglages de l'app : langue, cet appareil (nom, ce qu'il décode, son écran, son son), manettes
 * (test), mises à jour, à propos. Ce qui vaut pour toute l'app, pas pour un ordinateur.
 */
@Composable
fun AppSettingsContent(state: AppSettingsState, actions: AppSettingsActions) {
    val s = LocalScale.current
    val tv = LocalForm.current == FormFactor.TV
    val d = state.decoders
    val screen = state.screen
    Page(stringResource(R.string.app_settings_title), actions.back) {
        Section(stringResource(R.string.app_settings_language))
        ChoiceRow(
            stringResource(R.string.app_settings_language), stringResource(R.string.app_settings_language_help),
            listOf("auto" to stringResource(R.string.settings_auto), "fr" to "Français", "en" to "English"), state.language,
        ) { v -> actions.setLanguage(v) }

        Section(stringResource(R.string.app_settings_device))
        InfoRow(stringResource(R.string.app_settings_device_name), state.deviceName, state.deviceOs)
        InfoRow(stringResource(R.string.app_settings_decoders), listOfNotNull("H.264".takeIf { d.h264 != null }, "HEVC".takeIf { d.hevc != null }, "AV1".takeIf { d.av1 != null }).joinToString(" · "), (d.hevc ?: d.h264)?.name)
        InfoRow(stringResource(R.string.app_settings_screen), "${screen.current.width} × ${screen.current.height} · ${screen.current.refresh} Hz", screen.modes.map { "${it.height}p${it.refresh}" }.distinct().joinToString(" · "))
        // Au téléviseur, plusieurs fréquences ne font pas du VRR : Android ne le pilote pas en HDMI.
        val vrr = screen.variableRefresh && !tv
        InfoRow(
            stringResource(R.string.app_settings_vrr), stringResource(if (vrr) R.string.app_settings_available else R.string.app_settings_unavailable),
            stringResource(if (vrr) R.string.app_settings_vrr_help_yes else R.string.app_settings_vrr_help_no),
        )
        if (tv || screen.minimalPostProcessing) {
            InfoRow(stringResource(R.string.settings_game_mode), stringResource(if (screen.minimalPostProcessing) R.string.app_settings_available else R.string.app_settings_unavailable))
        }
        InfoRow(stringResource(R.string.app_settings_sound), audioLabel(state.audio), if (state.audio.bluetooth) stringResource(R.string.audio_bluetooth_delay) else null)

        Section(stringResource(R.string.app_settings_gamepads))
        FocusCard(actions.gamepads, Modifier.fillMaxWidth()) {
            Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
                KnIcon(KnIcons.gamepad)
                Spacer(Modifier.width(s.dp(12f)))
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
                    Body(stringResource(R.string.gamepads_title), size = 16f)
                    Body(stringResource(R.string.app_settings_gamepads_help), size = 13f, color = Kn.muted)
                }
                KnIcon(KnIcons.chevron, size = 18f)
            }
        }

        Section(stringResource(R.string.updates_section))
        UpdateRow(state.update, state.version, actions.updateAct)
        ChoiceRow(
            stringResource(R.string.updates_channel), stringResource(R.string.updates_channel_help),
            listOf(false to stringResource(R.string.updates_stable), true to stringResource(R.string.updates_beta)), state.beta,
        ) { v -> actions.setBeta(v) }

        Section(stringResource(R.string.app_settings_about))
        InfoRow("KyberNow Link", state.version, stringResource(R.string.app_settings_about_body))
        InfoRow(stringResource(R.string.app_settings_licenses), "AGPL-3.0", stringResource(R.string.app_settings_licenses_body))
    }
}

/** La sortie du son de l'appareil : « HDMI · 7.1 », « Bluetooth · Stéréo ». */
@Composable
private fun audioLabel(a: AudioOutput): String {
    val kind = when {
        a.hdmi -> stringResource(R.string.audio_hdmi)
        a.bluetooth -> stringResource(R.string.audio_bluetooth)
        else -> a.label.ifEmpty { stringResource(R.string.audio_device) }
    }
    val ch = when {
        a.maxChannels >= 8 -> "7.1"
        a.maxChannels >= 6 -> "5.1"
        else -> stringResource(R.string.audio_stereo)
    }
    return "$kind · $ch"
}

/** État de la mise à jour de l'app et son bouton (rechercher, télécharger, installer). */
@Composable
private fun UpdateRow(st: UpdateState, version: String, act: () -> Unit) {
    val s = LocalScale.current
    val context = LocalContext.current
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
                Body(stringResource(R.string.updates_title, version), size = 16f)
                Body(status, size = 13f, color = if (st is UpdateState.Failed) Kn.warn else if (st.offered != null) Kn.spectre else Kn.muted)
            }
            if (st is UpdateState.Checking || st is UpdateState.Downloading || st is UpdateState.Installing) {
                Spinner(22f)
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
        button?.let { GlassButton(it, act, selected = st.offered != null) }
    }
}
