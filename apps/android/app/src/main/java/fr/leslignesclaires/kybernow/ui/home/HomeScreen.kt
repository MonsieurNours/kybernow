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
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.selection.selectable
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
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.LendView
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.model.BorrowedComputer
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.SessionMode
import fr.leslignesclaires.kybernow.ui.AppModel
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.BorrowStep
import fr.leslignesclaires.kybernow.ui.FriendStatus
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FitTitle
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.HostStatus
import fr.leslignesclaires.kybernow.ui.RelayRefusal
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnFonts
import fr.leslignesclaires.kybernow.ui.neon
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalForm
import fr.leslignesclaires.kybernow.ui.LocalMotion
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.panel
import java.util.Locale

/** Ce que l'accueil demande à l'app. */
class HomeActions(
    val select: (String) -> Unit,
    val setProfile: (String, Profile) -> Unit,
    val connect: (Host) -> Unit,
    /** Hors ligne, sans adresse MAC : interroger Admin de nouveau. */
    val retry: (Host) -> Unit,
    val add: () -> Unit,
    val addAnnounced: (Announced) -> Unit,
    val hostSettings: (Host) -> Unit,
    val appSettings: () -> Unit,
    val wake: (Host) -> Unit,
    /** « Rejoindre avec un code » : KyberNow Share, comme invité (rien n'est ajouté aux ordinateurs). */
    val join: () -> Unit = {},
    // Prêt de l'ordinateur (docs/pret.md).
    /** « Emprunter » / « Demander à {hôte} » : le mot de passe tapé sur l'accueil quand l'app ne le connaît pas. */
    val borrow: (BorrowedComputer, String?) -> Unit = { _, _ -> },
    /** « Annuler » l'attente de l'hôte. */
    val cancelBorrow: (BorrowedComputer) -> Unit = {},
    /** « Retirer cet ordinateur » (un ordinateur prêté), confirmé. */
    val removeBorrowed: (BorrowedComputer) -> Unit = {},
    /** Chez l'hôte : « Reprendre l'ordinateur » (`false` : préavis de 2 min) ou « Reprendre tout de suite » (`true`). */
    val takeBack: (Host, Boolean) -> Unit = { _, _ -> },
)

/** Tout ce que l'accueil montre. */
data class HomeState(
    val hosts: List<Host>,
    val selectedId: String?,
    val status: Map<String, HostStatus>,
    val images: Map<String, ImageBitmap>,
    val announced: List<Announced>,
    /** L'écran et les décodeurs de cet appareil : l'écran que la session demandera. */
    val screen: Screen,
    val decoders: Decoders,
    /** Manettes branchées : la ligne « 2 manettes » des apps de salon (téléviseur). */
    val pads: Int = 0,
    /** Nouvelle version de l'app proposée (en haut, mène aux réglages de l'app). */
    val update: String? = null,
    /** Ordinateurs qu'on réveille en ce moment. */
    val waking: Set<String> = emptySet(),
    /** Ordinateurs « Hors de chez vous » qu'on vérifie par le relais avant d'ouvrir la session. */
    val connecting: Set<String> = emptySet(),
    /** Pourquoi un ordinateur n'a pas pu être ouvert hors de chez soi (docs/link-ui.md, « Hors de chez vous »). */
    val refusals: Map<String, RelayRefusal> = emptyMap(),
    // Prêt de l'ordinateur (docs/pret.md).
    /** Les ordinateurs que des amis prêtent à cet appareil (« Ordinateur de {hôte} »), après les siens. */
    val borrowed: List<BorrowedComputer> = emptyList(),
    val friends: Map<String, FriendStatus> = emptyMap(),
    val borrowing: Map<String, BorrowStep> = emptyMap(),
    val borrowErrors: Map<String, String> = emptyMap(),
    /** Les ordinateurs prêtés dont l'app connaît le mot de passe (retenu, ou tapé pendant cette session de l'app). */
    val knownPasswords: Set<String> = emptySet(),
    /** Chez l'hôte : la vue du prêt de ses ordinateurs, et le refus du dernier « Reprendre l'ordinateur ». */
    val lend: Map<String, LendView> = emptyMap(),
    val lendErrors: Map<String, String> = emptyMap(),
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
    val screen = remember { Screen.of(context) }
    val update by app.updates.state.collectAsState()
    val waking by model.waking.collectAsState()
    val connecting by model.connecting.collectAsState()
    val refusals by model.refusals.collectAsState()
    val borrowed by model.borrowed.collectAsState()
    val friends by model.friends.collectAsState()
    val borrowing by model.borrowing.collectAsState()
    val borrowErrors by model.borrowErrors.collectAsState()
    val lend by model.lend.collectAsState()
    val lendErrors by model.lendErrors.collectAsState()
    val known = remember(borrowed, friends, borrowing) { borrowed.filter { model.friendPassword(it.id) != null }.map { it.id }.toSet() }
    HomeContent(
        HomeState(
            hosts, selectedId, status, images, announced, screen, app.decoders, pads.count { !it.onScreen }, update.offered, waking, connecting, refusals,
            borrowed, friends, borrowing, borrowErrors, known, lend, lendErrors,
        ),
        actions,
    )
}

/**
 * L'accueil (docs/link-ui.md, « Accueil ») : pour l'ordinateur choisi, son état, son nom, son
 * système, l'écran que la session créera, « Se connecter », le profil et l'accès à ses réglages ;
 * au téléviseur, les manettes branchées. Puis les autres ordinateurs, leur nom et leur état.
 */
@Composable
fun HomeContent(state: HomeState, actions: HomeActions) {
    val (hosts, selectedId, status, images, announced) = state
    // Un ordinateur prêté par un ami peut être choisi ; sans ordinateur à soi, le premier prêté s'affiche.
    val friend = state.borrowed.firstOrNull { it.id == selectedId } ?: if (hosts.isEmpty()) state.borrowed.firstOrNull() else null
    val host = if (friend != null) null else hosts.firstOrNull { it.id == selectedId } ?: hosts.firstOrNull()

    if (host == null && friend == null) {
        Welcome(announced, state.update, actions)
        return
    }
    BoxWithConstraints(Modifier.fillMaxSize().background(Kn.ink)) {
        val wide = maxWidth > maxHeight && maxWidth >= 720.dp
        if (host != null) {
            val st = status[host.id] ?: HostStatus()
            Backdrop(images[host.id], st.online == false, wide)
            val hero: @Composable (Float, FocusRequester?, Boolean) -> Unit = { size, focus, stacked -> Hero(host, st, state, actions, size, focus, stacked) }
            if (wide) WideHome(host.id, state, actions, hero) else TallHome(state, actions, hero)
        } else if (friend != null) {
            Backdrop(null, false, wide)
            val hero: @Composable (Float, FocusRequester?, Boolean) -> Unit = { size, focus, stacked ->
                FriendHero(
                    friend, state.friends[friend.id], friend.id in state.knownPasswords, state.borrowing[friend.id], state.borrowErrors[friend.id], size, focus,
                    onBorrow = { pw -> actions.borrow(friend, pw) }, onCancel = { actions.cancelBorrow(friend) }, onRemove = { actions.removeBorrowed(friend) }, stacked = stacked,
                )
            }
            if (wide) WideHome(friend.id, state, actions, hero) else TallHome(state, actions, hero)
        }
    }
}

/** L'image de l'ordinateur en plein cadre : zoom très lent, désaturée hors ligne. */
@Composable
private fun Backdrop(image: ImageBitmap?, offline: Boolean, wide: Boolean) {
    val zoom = if (LocalMotion.current) {
        val t = rememberInfiniteTransition(label = "fond")
        t.animateFloat(1.0f, 1.06f, infiniteRepeatable(tween(40000, easing = LinearEasing), RepeatMode.Reverse), label = "zoom").value
    } else 1f
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
                if (wide) drawRect(Kn.ink.copy(alpha = 0.45f), size = size.copy(width = size.width * 0.62f))
                drawRect(Kn.ink.copy(alpha = 0.6f), topLeft = androidx.compose.ui.geometry.Offset(0f, size.height * 0.72f), size = size.copy(height = size.height * 0.28f))
            },
        )
    }
}

/** En haut : le logo, la mise à jour de l'app quand il y en a une, les réglages de l'app. */
@Composable
private fun TopBar(actions: HomeActions, update: String?, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    val phone = LocalForm.current == FormFactor.PHONE
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Image(painterResource(R.drawable.kn_logo_link), contentDescription = "KyberNow Link", modifier = Modifier.height(s.dp(if (s.tv) 40f else 26f)))
        Spacer(Modifier.weight(1f))
        update?.let {
            GlassButton(stringResource(R.string.updates_available, it), actions.appSettings, selected = true, leading = { KnIcon(KnIcons.download, size = 16f); Spacer(Modifier.width(6.dp)) })
            Spacer(Modifier.width(s.dp(10f)))
        }
        // Au téléphone, la place manque : l'icône seule, nommée pour l'accessibilité.
        if (phone) IconPill(KnIcons.settings, stringResource(R.string.home_app_settings), actions.appSettings)
        else GlassButton(stringResource(R.string.home_app_settings), actions.appSettings, leading = { KnIcon(KnIcons.settings, size = 16f); Spacer(Modifier.width(6.dp)) })
    }
}

@Composable
private fun IconPill(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, Modifier.semantics { contentDescription = label }, shape = RoundedCornerShape(12.dp)) {
        Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(11f))) {
            KnIcon(icon, size = 20f)
        }
    }
}

/** L'écran que la session créera (même calcul que la session, `SessionMode`). */
@Composable
fun displayLine(host: Host, screen: Screen, decoders: Decoders): String {
    if (!host.virtualDisplay || !host.settings.virtualDisplay) return stringResource(R.string.home_display_as_is)
    val mode = SessionMode.pick(screen, host.settings, SessionMode.codec(host, decoders), decoders)
    val (w, h, r) = mode
    return stringResource(if (SessionMode.matchesScreen(screen, host.settings, mode)) R.string.home_display_match else R.string.home_display_custom, w, h, r)
}

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun Hero(host: Host, st: HostStatus, state: HomeState, actions: HomeActions, nameSize: Float, focus: FocusRequester?, stacked: Boolean = false) {
    val s = LocalScale.current
    val set = host.settings
    // Au téléviseur (540 dp de haut), tout doit tenir au-dessus de la rangée des ordinateurs.
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(if (s.tv) 7f else 10f))) {
        StatusLine(st)
        FitTitle(host.name, Modifier.fillMaxWidth(), size = nameSize, min = nameSize * 0.45f)
        if (st.accountRefused) Body(stringResource(R.string.home_account_refused), color = Kn.warn, size = 13f)
        if (st.certificateChanged) Body(stringResource(R.string.home_certificate_changed), color = Kn.warn, size = 13f)
        state.refusals[host.id]?.let { Body(relayRefusalText(it, host.name), color = Kn.warn, size = 13f) }
        // Prêté à un ami : « Prêté à Léa (jusqu'à 18 h) », le compte à rebours du préavis.
        val lent = state.lend[host.id]?.takeIf { it.lent }
        lent?.let { LentLine(it) }
        state.lendErrors[host.id]?.takeIf { it.isNotBlank() }?.let { Body(it, color = Kn.warn, size = 13f) }
        (st.info?.os ?: host.os)?.takeIf { it.isNotBlank() }?.let { Body(it, size = 16f, color = Kn.frost2, maxLines = 1) }
        Row(verticalAlignment = Alignment.CenterVertically) {
            KnIcon(KnIcons.screen, tint = Kn.muted, size = 16f)
            Spacer(Modifier.width(s.dp(8f)))
            Body(displayLine(host, state.screen, state.decoders), size = 14f, color = Kn.frost2)
        }
        Spacer(Modifier.height(s.dp(if (s.tv) 2f else 6f)))
        val first = Modifier.then(if (focus != null) Modifier.focusRequester(focus) else Modifier)
        val a11y = settingsTitle(host.name)
        if (stacked) {
            // Téléphone et tablette en portrait (comme le client web au téléphone) : le bouton sur
            // toute la largeur, puis les profils sur une ligne et l'engrenage au bout.
            // Sur la largeur du téléphone ; à la tablette en portrait, pas plus de 560 dp.
            if (lent != null) TakeBackButton(lent, { now -> actions.takeBack(host, now) }, first.widthIn(max = s.dp(560f)).fillMaxWidth())
            else MainButton(host, st, state, actions, first.widthIn(max = s.dp(560f)).fillMaxWidth())
            Row(Modifier.widthIn(max = s.dp(560f)), verticalAlignment = Alignment.CenterVertically) {
                ProfileSegments(set, { p -> actions.setProfile(host.id, p) }, { actions.hostSettings(host) }, Modifier.weight(1f))
                Spacer(Modifier.width(s.dp(10f)))
                IconPill(KnIcons.settings, a11y) { actions.hostSettings(host) }
            }
        } else {
            // Une seule rangée qui passe à la ligne si la place manque : rien n'est écrasé.
            FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
                // Prêté : « Reprendre l'ordinateur » à la place de « Se connecter » (pendant un prêt, pas de session d'un appareil).
                if (lent != null) TakeBackButton(lent, { now -> actions.takeBack(host, now) }, first) else MainButton(host, st, state, actions, first)
                Profile.entries.forEach { p ->
                    GlassButton(profileLabel(p), { actions.setProfile(host.id, p) }, selected = !set.custom && set.profile == p)
                }
                // « Personnalisé » n'est pas un choix : il dit que les réglages s'écartent du profil.
                if (set.custom) GlassButton(stringResource(R.string.profile_custom), { actions.hostSettings(host) }, selected = true)
                GlassButton(
                    stringResource(R.string.home_host_settings), { actions.hostSettings(host) },
                    Modifier.semantics { contentDescription = a11y },
                    leading = { KnIcon(KnIcons.settings, size = 16f); Spacer(Modifier.width(6.dp)) },
                )
            }
        }
        Body(profileHelp(set), size = 14f, color = Kn.muted)
        if (LocalForm.current == FormFactor.TV) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                KnIcon(KnIcons.gamepad, tint = Kn.muted, size = 18f)
                Spacer(Modifier.width(s.dp(8f)))
                Body(padsLabel(state.pads), size = 14f, color = Kn.frost2)
            }
        }
    }
}

/**
 * Le bouton principal : « Se connecter » ; hors ligne, « Réveiller l'ordinateur » quand l'app
 * connaît ses cartes réseau (apprises d'Admin, ou une adresse MAC saisie), sinon « Réessayer » ;
 * pendant le réveil, « Réveil de l'ordinateur… ».
 */
@Composable
private fun MainButton(host: Host, st: HostStatus, state: HomeState, actions: HomeActions, modifier: Modifier) = when {
    // Sans effet, mais toujours focalisable : au téléviseur, le focus ne se perd pas pendant le réveil.
    host.id in state.waking && st.online != true -> PrimaryButton(stringResource(R.string.home_waking), {}, modifier)
    // Hors de chez vous : l'ordinateur est vérifié par le relais avant d'ouvrir la session.
    host.id in state.connecting -> PrimaryButton(stringResource(R.string.home_connecting), {}, modifier)
    st.online == false && host.wakeTargets.isNotEmpty() -> PrimaryButton(stringResource(R.string.home_wake), { actions.wake(host) }, modifier)
    st.online == false -> PrimaryButton(stringResource(R.string.home_retry), { actions.retry(host) }, modifier)
    else -> PrimaryButton(stringResource(R.string.home_connect), { actions.connect(host) }, modifier)
}

/** Les profils en commande segmentée (téléphone et tablette en portrait) ; « Personnalisé » au bout quand il y a lieu. */
@Composable
private fun ProfileSegments(set: HostSettings, pick: (Profile) -> Unit, custom: () -> Unit, modifier: Modifier) {
    val s = LocalScale.current
    val pill = RoundedCornerShape(999.dp)
    Row(
        modifier.clip(pill).background(Kn.frost.copy(alpha = 0.06f)).border(BorderStroke(1.dp, Kn.frost.copy(alpha = 0.16f)), pill).padding(s.dp(3f)),
        horizontalArrangement = Arrangement.spacedBy(s.dp(2f)),
    ) {
        Profile.entries.forEach { p -> Segment(profileLabel(p), !set.custom && set.profile == p, Modifier.weight(1f)) { pick(p) } }
        // Plus long : il prend un peu plus de place que les autres.
        if (set.custom) Segment(stringResource(R.string.profile_custom), true, Modifier.weight(1.7f), custom)
    }
}

@Composable
private fun Segment(label: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val s = LocalScale.current
    val pill = RoundedCornerShape(999.dp)
    val i = remember { MutableInteractionSource() }
    Box(
        modifier.neon(i, pill).clip(pill)
            .background(if (selected) Kn.spectre.copy(alpha = 0.22f) else Kn.frost.copy(alpha = 0f))
            .border(BorderStroke(1.dp, if (selected) Kn.spectre else Kn.frost.copy(alpha = 0f)), pill)
            .selectable(selected = selected, interactionSource = i, indication = null, role = Role.Tab, onClick = onClick)
            .padding(horizontal = s.dp(4f), vertical = s.dp(10f)),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label, maxLines = 1, color = if (selected) Kn.frost else Kn.frost2,
            style = TextStyle(fontFamily = KnFonts.text, fontSize = s.sp(15f), fontWeight = FontWeight(if (selected) 600 else 500)),
        )
    }
}

@Composable
private fun padsLabel(n: Int) = when (n) {
    0 -> stringResource(R.string.home_pads_none)
    1 -> stringResource(R.string.home_pads_one)
    else -> stringResource(R.string.home_pads_many, n)
}

/** « En ligne », « Hors ligne », « Recherche… » ; « Hors de chez vous » quand l'ordinateur n'est joignable que par le relais. */
@Composable
fun statusLabel(st: HostStatus?) = stringResource(
    when {
        st == null || st.online == null -> R.string.home_searching
        st.away -> R.string.home_away
        st.online -> R.string.home_online
        else -> R.string.home_offline
    },
)

/**
 * Pourquoi un ordinateur n'a pas pu être ouvert hors de chez soi : « {nom} n'est pas joignable : l'ordinateur est
 * éteint ou en veille. », « {nom} n'est pas joignable hors de chez vous (accès coupé dans Admin). », ou le texte du
 * cœur (« Le relais ne répond pas. »).
 */
@Composable
fun relayRefusalText(why: RelayRefusal, name: String): String = when (why) {
    RelayRefusal.StationOffline -> stringResource(R.string.relay_station_offline, name)
    RelayRefusal.AccessOff -> stringResource(R.string.relay_access_off, name)
    is RelayRefusal.Message -> why.text.ifBlank { stringResource(R.string.add_unreachable) }
}

@Composable
private fun StatusLine(st: HostStatus) {
    val s = LocalScale.current
    val online = st.online == true
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(if (online) Kn.spectre else Kn.muted))
        Spacer(Modifier.width(s.dp(10f)))
        Eyebrow(statusLabel(st), color = Kn.frost2)
    }
}

/**
 * « Réglages de Bureau-Salon », « Réglages d'Atelier », « Réglages d'Hugo-PC » : en français, « de »
 * s'élide devant une voyelle (accentuée comprise) ou un h.
 */
@Composable
fun settingsTitle(name: String): String =
    stringResource(if (elides(name)) R.string.settings_for_elided else R.string.settings_for, name)

/** Le nom commence par une voyelle ou un h : « d' » plutôt que « de ». */
fun elides(name: String): Boolean = name.firstOrNull()?.lowercaseChar()?.let { it in "aeiouyhàâäéèêëîïôöùûüÿœæ" } == true

@Composable
fun profileLabel(p: Profile) = stringResource(
    when (p) {
        Profile.GAME -> R.string.profile_game
        Profile.DESKTOP -> R.string.profile_desktop
        Profile.MAX -> R.string.profile_max
    },
)

/** La phrase du profil, ou celle de « Personnalisé ». */
@Composable
fun profileHelp(set: HostSettings) = stringResource(
    when {
        set.custom -> R.string.profile_custom_help
        set.profile == Profile.GAME -> R.string.profile_game_help
        set.profile == Profile.DESKTOP -> R.string.profile_desktop_help
        else -> R.string.profile_max_help
    },
)

@Composable
fun motorsLabel(m: Int) = when (m) {
    2 -> stringResource(R.string.motors_two)
    1 -> stringResource(R.string.motors_one)
    else -> stringResource(R.string.motors_none)
}

/** « 42 min », « 1 h 12 » (espaces insécables). */
fun duration(seconds: Long): String = if (seconds >= 3600) "${seconds / 3600}\u00a0h\u00a0${String.format(Locale.ROOT, "%02d", (seconds % 3600) / 60)}" else "${seconds / 60}\u00a0min"

/** Les ordinateurs : leur nom et leur état, ceux annoncés sur le réseau, l'ajout. */
@Composable
private fun HostRow(state: HomeState, actions: HomeActions, card: Float) {
    val s = LocalScale.current
    val (hosts, selected, status, images, announced) = state
    LazyRow(horizontalArrangement = Arrangement.spacedBy(s.dp(16f)), contentPadding = PaddingValues(horizontal = s.dp(4f), vertical = s.dp(10f))) {
        items(hosts, key = { it.id }) { h ->
            var menu by remember { mutableStateOf(false) }
            Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
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
                Column {
                    Body(h.name, size = 15f, weight = if (h.id == selected) 600 else 400, maxLines = 1)
                    Mono(statusLabel(status[h.id]), size = 10f, color = if (status[h.id]?.online == true) Kn.frost2 else Kn.muted, maxLines = 1)
                }
                DropdownMenu(menu, { menu = false }) {
                    DropdownMenuItem({ Text(settingsTitle(h.name)) }, { menu = false; actions.hostSettings(h) })
                }
            }
        }
        items(state.borrowed, key = { it.id }) { b ->
            FriendCard(b, state.friends[b.id], b.id in state.knownPasswords, b.id == selected, card) { actions.select(b.id) }
        }
        items(announced, key = { "a:" + it.serviceName }) { a ->
            Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
                FocusCard({ actions.addAnnounced(a) }) {
                    Box(
                        Modifier.fillMaxWidth().aspectRatio(16f / 9f).border(BorderStroke(1.dp, Kn.spectre.copy(alpha = 0.6f)), RoundedCornerShape(16.dp)),
                        contentAlignment = Alignment.Center,
                    ) { KnIcon(KnIcons.network, tint = Kn.spectre, size = 26f) }
                }
                Column {
                    Body(a.name, size = 15f, maxLines = 1)
                    Mono(stringResource(R.string.home_on_network), size = 10f, color = Kn.spectre, maxLines = 1)
                }
            }
        }
        item(key = "ajout") {
            Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
                FocusCard(actions.add) {
                    Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(Kn.frost.copy(alpha = 0.06f)), contentAlignment = Alignment.Center) {
                        KnIcon(KnIcons.plus, size = 30f)
                    }
                }
                Body(stringResource(R.string.home_add), size = 15f, maxLines = 2)
            }
        }
        // À côté de « Ajouter un ordinateur » : rejoindre le partage de quelqu'un d'autre.
        item(key = "rejoindre") {
            Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
                FocusCard(actions.join) {
                    Box(Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(Kn.frost.copy(alpha = 0.06f)), contentAlignment = Alignment.Center) {
                        KnIcon(KnIcons.code, size = 30f)
                    }
                }
                Body(stringResource(R.string.home_join), size = 15f, maxLines = 2)
            }
        }
    }
}

/** Téléviseur, tablette en paysage, téléphone en paysage. [hero] : l'ordinateur choisi (taille du nom, focus, empilé). */
@Composable
private fun WideHome(key: String, state: HomeState, actions: HomeActions, hero: @Composable (Float, FocusRequester?, Boolean) -> Unit) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(key) { if (s.tv) runCatching { focus.requestFocus() } }
    val pad = if (s.tv) 56f else 28f
    Column(Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().padding(horizontal = s.dp(pad), vertical = s.dp(if (s.tv) 32f else 18f))) {
        TopBar(actions, state.update)
        // Centré dans la place qui reste ; s'il ne tient pas (alertes, « Personnalisé »), il défile.
        Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.CenterStart) {
            Box(Modifier.fillMaxWidth(if (s.tv) 0.8f else 0.66f).widthIn(max = s.dp(720f)).verticalScroll(rememberScrollState())) {
                hero(if (s.tv) 46f else 52f, focus, false)
            }
        }
        HostRow(state, actions, card = if (s.tv) 150f else 170f)
    }
}

/** Téléphone et tablette en portrait. */
@Composable
private fun TallHome(state: HomeState, actions: HomeActions, hero: @Composable (Float, FocusRequester?, Boolean) -> Unit) {
    val s = LocalScale.current
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).statusBarsPadding().navigationBarsPadding().padding(s.dp(20f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(18f)),
    ) {
        TopBar(actions, state.update)
        Spacer(Modifier.height(s.dp(150f)))
        hero(54f, null, true)
        Spacer(Modifier.height(s.dp(6f)))
        Eyebrow(stringResource(R.string.home_your_computers))
        HostRow(state, actions, card = 160f)
    }
}

/** Aucun ordinateur ajouté : la bienvenue, ceux annoncés sur le réseau et l'ajout. */
@Composable
private fun Welcome(announced: List<Announced>, update: String?, actions: HomeActions) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (s.tv) runCatching { focus.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        // Défilant : au téléviseur (540 dp de haut), tout ne tient pas toujours ; le focus fait défiler.
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState())
                .padding(horizontal = s.dp(if (s.tv) 56f else 24f), vertical = s.dp(if (s.tv) 28f else 24f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(14f)),
        ) {
            TopBar(actions, update)
            Spacer(Modifier.height(s.dp(if (s.tv) 24f else 40f)))
            Column(Modifier.fillMaxWidth(if (s.tv) 0.62f else 1f), verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
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
                                Mono(stringResource(R.string.home_on_network), size = 11f, color = Kn.spectre)
                            }
                            KnIcon(KnIcons.chevron, size = 18f)
                        }
                    }
                }
                PrimaryButton(stringResource(R.string.home_add), actions.add, Modifier.focusRequester(focus))
                GlassButton(stringResource(R.string.home_join), actions.join)
            }
        }
    }
}
