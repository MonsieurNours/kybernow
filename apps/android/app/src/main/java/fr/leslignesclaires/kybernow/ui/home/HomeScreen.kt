package fr.leslignesclaires.kybernow.ui.home

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
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
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Gamepad
import fr.leslignesclaires.kybernow.model.AudioOutput
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.ui.AppModel
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalForm
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.Reach
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.FitTitle
import fr.leslignesclaires.kybernow.ui.fmt
import fr.leslignesclaires.kybernow.ui.panel
import java.util.Locale

/** Ce que l'accueil demande à l'app. */
class HomeActions(
    val select: (String) -> Unit,
    val setProfile: (String, Profile) -> Unit,
    val remove: (String) -> Unit,
    val connect: (Host) -> Unit,
    val add: () -> Unit,
    val addAnnounced: (Announced) -> Unit,
    val hostSettings: (Host) -> Unit,
    val appSettings: () -> Unit,
    val gamepads: () -> Unit,
    val wake: (Host) -> Unit,
)

/** « Prêt à jouer » : ce que l'appareil a de branché et ce qu'il sait faire. */
data class ReadyInfo(val pads: List<Gamepad>, val audio: AudioOutput, val screen: Screen, val decoders: Decoders)

/** Tout ce que l'accueil montre. */
data class HomeState(
    val hosts: List<Host>,
    val selectedId: String?,
    val status: Map<String, HostStatus>,
    val images: Map<String, ImageBitmap>,
    val announced: List<Announced>,
    val ready: ReadyInfo,
    /** Nouvelle version de l'app proposée (pastille en haut, mène aux réglages de l'app). */
    val update: String? = null,
)

@Composable
fun HomeScreen(model: AppModel, actions: HomeActions) {
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val hosts by model.hosts.collectAsState()
    val selectedId by model.selected.collectAsState()
    val status by model.status.collectAsState()
    val images by model.images.collectAsState()
    val announced by model.announced.collectAsState()
    val pads by app.gamepads.pads.collectAsState()
    val audio = remember { AudioOutput.current(context) }
    val screen = remember { Screen.of(context) }
    val update by app.updates.state.collectAsState()
    HomeContent(HomeState(hosts, selectedId, status, images, announced, ReadyInfo(pads, audio, screen, app.decoders), update.offered), actions)
}

@Composable
fun HomeContent(state: HomeState, actions: HomeActions) {
    val (hosts, selectedId, status, images, announced) = state
    val form = LocalForm.current
    val host = hosts.firstOrNull { it.id == selectedId } ?: hosts.firstOrNull()

    if (host == null) {
        Welcome(announced, actions)
        return
    }
    val st = status[host.id] ?: HostStatus()
    BoxWithConstraints(Modifier.fillMaxSize().background(Kn.ink)) {
        val wide = maxWidth > maxHeight && maxWidth >= 720.dp
        Backdrop(images[host.id], st.online == false, wide)
        if (wide) {
            WideHome(host, st, state, actions, showReady = form != FormFactor.PHONE)
        } else {
            TallHome(host, st, state, actions)
        }
    }
}

/** L'image de l'ordinateur en plein cadre : zoom très lent, désaturée hors ligne. */
@Composable
private fun Backdrop(image: androidx.compose.ui.graphics.ImageBitmap?, offline: Boolean, wide: Boolean) {
    val t = rememberInfiniteTransition(label = "fond")
    val zoom by t.animateFloat(1.0f, 1.06f, infiniteRepeatable(tween(40000, easing = LinearEasing), RepeatMode.Reverse), label = "zoom")
    Box(Modifier.fillMaxSize()) {
        if (image != null) {
            Image(
                image,
                contentDescription = null,
                modifier = Modifier.fillMaxSize().scale(zoom),
                contentScale = ContentScale.Crop,
                colorFilter = if (offline) ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) }) else null,
                alpha = 0.55f,
            )
        }
        // Voiles d'encre : à gauche (texte) et en bas (rangée des ordinateurs).
        Box(
            Modifier.fillMaxSize().drawWithContent {
                drawContent()
                drawRect(Kn.ink.copy(alpha = 0.45f))
                if (wide) drawRect(Kn.ink.copy(alpha = 0.45f), size = size.copy(width = size.width * 0.58f))
                drawRect(Kn.ink.copy(alpha = 0.6f), topLeft = androidx.compose.ui.geometry.Offset(0f, size.height * 0.72f), size = size.copy(height = size.height * 0.28f))
            },
        )
    }
}

@Composable
private fun TopBar(actions: HomeActions, update: String?, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Image(painterResource(R.drawable.kn_logo_link), contentDescription = "KyberNow Link", modifier = Modifier.height(s.dp(if (s.tv) 40f else 26f)))
        Spacer(Modifier.weight(1f))
        update?.let {
            GlassButton(stringResource(R.string.updates_home, it), actions.appSettings, selected = true, leading = { KnIcon(KnIcons.download, size = 16f); Spacer(Modifier.width(6.dp)) })
            Spacer(Modifier.width(s.dp(10f)))
        }
        IconPill(KnIcons.gamepad, stringResource(R.string.home_gamepads), actions.gamepads)
        Spacer(Modifier.width(s.dp(10f)))
        IconPill(KnIcons.settings, stringResource(R.string.home_app_settings), actions.appSettings)
    }
}

@Composable
private fun IconPill(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, shape = RoundedCornerShape(12.dp)) {
        Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(11f)).semantics { contentDescription = label }) {
            KnIcon(icon, size = 20f)
        }
    }
}

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun Hero(host: Host, st: HostStatus, actions: HomeActions, nameSize: Float, focus: FocusRequester?) {
    val s = LocalScale.current
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        StatusLine(host, st)
        FitTitle(host.name, Modifier.fillMaxWidth(), size = nameSize, min = nameSize * 0.45f)
        val details = listOfNotNull(st.info?.os ?: host.os, st.info?.gpu ?: host.gpu, host.address).joinToString(" · ")
        Mono(details, size = 12f, maxLines = 1)
        Spacer(Modifier.height(s.dp(8f)))
        // Une seule rangée qui passe à la ligne si l'écran est étroit : rien n'est écrasé.
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
            if (st.online == false && host.mac != null) {
                PrimaryButton(stringResource(R.string.home_wake), { actions.wake(host) }, Modifier.then(if (focus != null) Modifier.focusRequester(focus) else Modifier))
            } else {
                PrimaryButton(
                    stringResource(R.string.home_connect),
                    { actions.connect(host) },
                    Modifier.then(if (focus != null) Modifier.focusRequester(focus) else Modifier),
                    enabled = st.online != false,
                )
            }
            Profile.entries.forEach { p ->
                GlassButton(profileLabel(p), { actions.setProfile(host.id, p) }, selected = host.settings.profile == p)
            }
            GlassButton(stringResource(R.string.home_host_settings), { actions.hostSettings(host) }, leading = { KnIcon(KnIcons.settings, size = 16f); Spacer(Modifier.width(6.dp)) })
        }
        if (st.accountRefused) Body(stringResource(R.string.home_account_refused), color = Kn.warn, size = 13f)
        if (st.certificateChanged) Body(stringResource(R.string.home_certificate_changed), color = Kn.warn, size = 13f)
    }
}

@Composable
private fun StatusLine(host: Host, st: HostStatus) {
    val s = LocalScale.current
    val online = st.online == true
    val label = when {
        st.online == null -> stringResource(R.string.home_checking)
        !online -> stringResource(R.string.home_offline)
        else -> when (AppModel.reach(host.address)) {
            Reach.LOCAL -> stringResource(R.string.home_online_local)
            Reach.VPN -> stringResource(R.string.home_online_vpn)
            Reach.REMOTE -> stringResource(R.string.home_online_remote)
        }
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(if (online) Kn.spectre else Kn.muted))
        Spacer(Modifier.width(s.dp(10f)))
        Eyebrow(label, color = Kn.frost2)
    }
}

@Composable
fun profileLabel(p: Profile) = stringResource(
    when (p) {
        Profile.GAME -> R.string.profile_game
        Profile.DESKTOP -> R.string.profile_desktop
        Profile.MAX -> R.string.profile_max
    },
)

/** « Prêt à jouer » : manettes, sortie du son, écran, dernière session. */
@Composable
private fun ReadyPanel(host: Host, ready: ReadyInfo, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    val (pads, audio, screen) = ready
    Column(modifier.panel().padding(s.dp(22f)), verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
        Eyebrow(stringResource(R.string.ready_title))
        ReadyRow(stringResource(R.string.ready_gamepads)) {
            if (pads.isEmpty()) Body(stringResource(R.string.ready_no_gamepad), size = 15f, color = Kn.frost2)
            pads.take(4).forEach { p ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Body("${if (p.slot >= 0) "J${p.slot + 1}" else "…"}  ${p.name}", size = 15f, maxLines = 1, modifier = Modifier.weight(1f))
                    Mono(listOfNotNull(p.battery?.let { "$it %" }, motorsLabel(p.motors)).joinToString(" · "), size = 11f)
                }
            }
        }
        ReadyRow(stringResource(R.string.ready_sound)) {
            Body(audioLabel(audio), size = 15f)
            if (audio.bluetooth) Mono(stringResource(R.string.ready_bluetooth_delay), size = 11f, color = Kn.warn)
        }
        ReadyRow(stringResource(if (LocalForm.current == FormFactor.TV) R.string.ready_tv else R.string.ready_screen)) {
            val best = screen.bestRefresh(screen.current.width, screen.current.height)
            val refresh = if (LocalForm.current == FormFactor.TV) screen.bestRefresh(1920, 1080) else best
            Body("${screen.current.height}p · $refresh Hz" + if (screen.minimalPostProcessing) " · " + stringResource(R.string.ready_game_mode) else "", size = 15f)
            val d = ready.decoders
            Mono(
                listOfNotNull(
                    if (d.hevc != null) "HEVC" else "H.264",
                    if (d.av1 != null) "AV1" else null,
                    // Au téléviseur, plusieurs fréquences ne font pas du VRR : Android ne le pilote pas en HDMI.
                    stringResource(if (screen.variableRefresh && LocalForm.current != FormFactor.TV) R.string.ready_vrr_yes else R.string.ready_vrr_no),
                ).joinToString(" · "),
                size = 11f,
            )
        }
        host.last?.let { last ->
            ReadyRow(stringResource(R.string.ready_last_session)) {
                Mono(fmt("%.1f ms", last.median), size = 22f, color = Kn.frost, weight = 500)
                Mono(fmt("p95 %.1f ms · %s", last.p95, duration(last.seconds)), size = 11f)
            }
        }
    }
}

@Composable
private fun ReadyRow(label: String, content: @Composable () -> Unit) {
    val s = LocalScale.current
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
        Body(label, size = 12f, color = Kn.muted)
        content()
    }
}

@Composable
fun motorsLabel(m: Int) = when (m) {
    2 -> stringResource(R.string.motors_two)
    1 -> stringResource(R.string.motors_one)
    else -> stringResource(R.string.motors_none)
}

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

fun duration(seconds: Long): String = if (seconds >= 3600) "${seconds / 3600} h ${String.format(Locale.ROOT, "%02d", (seconds % 3600) / 60)}" else "${seconds / 60} min"

@Composable
private fun HostRow(state: HomeState, actions: HomeActions, card: Float) {
    val s = LocalScale.current
    val (hosts, selected, status, images, announced) = state
    LazyRow(horizontalArrangement = Arrangement.spacedBy(s.dp(16f)), contentPadding = PaddingValues(horizontal = s.dp(4f), vertical = s.dp(10f))) {
        items(hosts, key = { it.id }) { h ->
            var menu by remember { mutableStateOf(false) }
            Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                FocusCard({ actions.select(h.id) }, onLongClick = { menu = true }) {
                    Box(
                        Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(Kn.panel)
                            .border(BorderStroke(if (h.id == selected) 2.dp else 1.dp, if (h.id == selected) Kn.frost.copy(alpha = 0.6f) else Kn.line), RoundedCornerShape(16.dp)),
                    ) {
                        images[h.id]?.let {
                            Image(
                                it, null, Modifier.fillMaxSize(), contentScale = ContentScale.Crop,
                                colorFilter = if (status[h.id]?.online == false) ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) }) else null,
                            )
                        }
                    }
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Body(h.name, size = 15f, weight = if (h.id == selected) 600 else 400, maxLines = 1, modifier = Modifier.weight(1f, fill = false))
                    if (status[h.id]?.online == false) {
                        Spacer(Modifier.width(8.dp))
                        Mono(stringResource(R.string.home_offline_short), size = 10f, color = Kn.muted)
                    }
                }
                DropdownMenu(menu, { menu = false }) {
                    DropdownMenuItem({ Text(stringResource(R.string.home_host_settings)) }, { menu = false; actions.hostSettings(h) })
                    DropdownMenuItem({ Text(stringResource(R.string.home_remove)) }, { menu = false; actions.remove(h.id) })
                }
            }
        }
        items(announced, key = { "a:" + it.serviceName }) { a ->
            Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                FocusCard({ actions.addAnnounced(a) }) {
                    Box(
                        Modifier.fillMaxWidth().aspectRatio(16f / 9f).border(BorderStroke(1.dp, Kn.spectre.copy(alpha = 0.6f)), RoundedCornerShape(16.dp)),
                        contentAlignment = Alignment.Center,
                    ) { Eyebrow(stringResource(R.string.home_on_network), color = Kn.spectre) }
                }
                Body(a.name, size = 15f, maxLines = 1)
            }
        }
        item(key = "ajout") {
            Column(Modifier.width(s.dp(card * 9f / 16f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                FocusCard(actions.add) {
                    Box(Modifier.fillMaxWidth().aspectRatio(1f).background(Kn.frost.copy(alpha = 0.06f)), contentAlignment = Alignment.Center) {
                        KnIcon(KnIcons.plus, size = 30f)
                    }
                }
                Body(stringResource(R.string.home_add), size = 15f)
            }
        }
    }
}

/** Téléviseur, tablette en paysage, téléphone en paysage. */
@Composable
private fun WideHome(host: Host, st: HostStatus, state: HomeState, actions: HomeActions, showReady: Boolean) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(host.id) { if (s.tv) runCatching { focus.requestFocus() } }
    val pad = if (s.tv) 56f else 28f
    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().padding(horizontal = s.dp(pad), vertical = s.dp(if (s.tv) 32f else 18f))) {
        TopBar(actions, state.update)
        Row(Modifier.weight(1f).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.weight(1f)) { Hero(host, st, actions, if (s.tv) 64f else 52f, focus) }
            if (showReady) {
                Spacer(Modifier.width(s.dp(28f)))
                ReadyPanel(host, state.ready, Modifier.width(s.dp(if (s.tv) 330f else 300f)))
            }
        }
        HostRow(state, actions, card = if (s.tv) 180f else 170f)
    }
}

/** Téléphone et tablette en portrait. */
@Composable
private fun TallHome(host: Host, st: HostStatus, state: HomeState, actions: HomeActions) {
    val s = LocalScale.current
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).statusBarsPadding().navigationBarsPadding().padding(s.dp(20f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(18f)),
    ) {
        TopBar(actions, state.update)
        Spacer(Modifier.height(s.dp(150f)))
        Hero(host, st, actions, 54f, null)
        Spacer(Modifier.height(s.dp(6f)))
        Eyebrow(stringResource(R.string.home_your_computers))
        HostRow(state, actions, card = 160f)
        if (LocalForm.current == FormFactor.TABLET) ReadyPanel(host, state.ready, Modifier.fillMaxWidth())
    }
}

/** Premier lancement : rien encore. */
@Composable
private fun Welcome(announced: List<Announced>, actions: HomeActions) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (s.tv) runCatching { focus.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        // Défilant : au téléviseur (540 dp de haut), tout ne tient pas toujours ; le focus fait défiler.
        Column(
            Modifier.align(Alignment.CenterStart).fillMaxWidth(if (s.tv) 0.62f else 1f).verticalScroll(rememberScrollState())
                .padding(horizontal = s.dp(if (s.tv) 56f else 24f), vertical = s.dp(if (s.tv) 28f else 24f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(14f)),
        ) {
            Image(painterResource(R.drawable.kn_logo_link), contentDescription = "KyberNow Link", modifier = Modifier.height(s.dp(if (s.tv) 40f else 36f)))
            Spacer(Modifier.height(s.dp(6f)))
            Title(stringResource(R.string.welcome_title), size = if (s.tv) 40f else 34f)
            Body(stringResource(R.string.welcome_body), color = Kn.frost2, size = 16f)
            Spacer(Modifier.height(s.dp(6f)))
            announced.forEach { a ->
                FocusCard({ actions.addAnnounced(a) }, Modifier.fillMaxWidth()) {
                    Row(Modifier.panel().padding(s.dp(16f)).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        KnIcon(KnIcons.network, tint = Kn.spectre)
                        Spacer(Modifier.width(s.dp(12f)))
                        Column(Modifier.weight(1f)) {
                            Body(a.name, size = 17f, weight = 600)
                            Mono("${a.address}:${a.port}", size = 11f)
                        }
                        KnIcon(KnIcons.chevron, size = 18f)
                    }
                }
            }
            PrimaryButton(stringResource(R.string.welcome_add), actions.add, Modifier.focusRequester(focus))
        }
    }
}
