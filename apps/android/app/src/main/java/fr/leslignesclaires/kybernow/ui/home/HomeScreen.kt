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
import androidx.compose.foundation.layout.RowScope
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
import androidx.compose.ui.graphics.graphicsLayer
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
import fr.leslignesclaires.kybernow.model.Hdr
import fr.leslignesclaires.kybernow.model.HdrState
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
import fr.leslignesclaires.kybernow.ui.HdrChip
import fr.leslignesclaires.kybernow.ui.rememberHdrScreen
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
import fr.leslignesclaires.kybernow.core.StationOffer
import fr.leslignesclaires.kybernow.ui.StationBannerActions
import fr.leslignesclaires.kybernow.ui.StationBannerView
import fr.leslignesclaires.kybernow.ui.StationTask
import fr.leslignesclaires.kybernow.ui.withUpdateChip
import fr.leslignesclaires.kybernow.ui.stationBannerVisible
import fr.leslignesclaires.kybernow.ui.panel
import fr.leslignesclaires.kybernow.ui.playstation.ConsoleCard
import fr.leslignesclaires.kybernow.ui.playstation.ConsoleHero
import fr.leslignesclaires.kybernow.ui.playstation.FoundCard
import fr.leslignesclaires.kybernow.ui.playstation.PsHdr
import fr.leslignesclaires.kybernow.ui.playstation.PsHomeActions
import fr.leslignesclaires.kybernow.ui.playstation.PsHomeState
import androidx.compose.runtime.DisposableEffect
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
    /** Consoles PlayStation (docs/link-ui.md, « Consoles PlayStation »). */
    val ps: PsHomeActions = PsHomeActions(),
    /** La puce « HDR » d'un ordinateur : le choix gardé pour lui (docs/hdr.md). */
    val setHdr: (Host, Boolean) -> Unit = { _, _ -> },
    /** « Connecter un compte » (accueil sans ordinateur) : onglet personnalisé, ou le code au téléviseur. */
    val signIn: () -> Unit = {},
    /** « Regarder » un ordinateur du compte (droit « Regarder ») : comme un invité spectateur de son partage. */
    val watch: (Host) -> Unit = {},
    // Mise à jour des Stations (docs/link-ui.md, « Mise à jour des Stations »).
    /** « Mettre à jour » du bandeau (`true` : « Mettre à jour quand même »). */
    val stationUpdate: (Host, Boolean) -> Unit = { _, _ -> },
    /** « Plus tard » du bandeau, pour cette version. */
    val stationLater: (Host, String) -> Unit = { _, _ -> },
    /** « Annuler » la confirmation du bandeau. */
    val stationCancel: (Host) -> Unit = {},
    // Accueil et compte KyberNow, sans compte (docs/link-ui.md, « Sans compte, ensuite »).
    /** « Créer un compte » : la page d'inscription dans le navigateur (le code au téléviseur). */
    val createAccount: () -> Unit = {},
    /** La croix de la carte du compte (« Fermer », ou « Ne plus afficher »). */
    val closeAccountCard: () -> Unit = {},
    /** « Plus tard » du bandeau après un appairage (« Connecter un compte » le ferme aussi). */
    val dismissPairedBanner: () -> Unit = {},
    /** Le bandeau après l'appairage de cet ordinateur a sa place (pas de bandeau de la Station) : le compter et l'afficher. */
    val showPairedBanner: (Host) -> Unit = {},
)

/**
 * Tout ce que l'accueil montre. Immuable (copies des états de l'app, jamais modifiées) : Compose compare par contenu et
 * ne refait pas l'accueil quand un sondage rend les mêmes valeurs (AND-29).
 */
@androidx.compose.runtime.Immutable
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
    /** Consoles PlayStation : dans la même rangée que les ordinateurs, après eux. */
    val ps: PsHomeState = PsHomeState(),
    /** La puce « HDR » de chaque ordinateur (absente : écran SDR, aucune trace du HDR). */
    val hdr: Map<String, HdrState> = emptyMap(),
    /** Comptes KyberNow : la pastille d'origine de chaque ordinateur, seulement quand un compte est connecté. */
    val origins: Map<String, String> = emptyMap(),
    /** Le retour de la page de connexion s'échange contre le compte : « Connexion au compte… ». */
    val signingIn: Boolean = false,
    /**
     * Comptes KyberNow, droit « Regarder » : vrai quand c'est le seul (« Regarder » à la place de « Se connecter »), faux
     * avec « Ouvrir le bureau » aussi (« Regarder » dans le menu de l'ordinateur) ; absent : pas de « Regarder ».
     */
    val watch: Map<String, Boolean> = emptyMap(),
    /** « Regarder » / “Watch”, le mot du cœur. */
    val watchWord: String = "",
    /** Mise à jour des Stations : le bandeau et la puce de chaque ordinateur (calcul du cœur). */
    val offers: Map<String, StationOffer> = emptyMap(),
    /** La mise à jour (ou le redémarrage) de la Station en cours, par ordinateur : le flux des réglages. */
    val stationTasks: Map<String, StationTask> = emptyMap(),
    /** Les mises à jour lancées par le bandeau ou l'avis : leur confirmation et leur échec s'écrivent dans le bandeau. */
    val fromOffer: Set<String> = emptySet(),
    /** Un compte KyberNow est connecté : ni bouton du compte, ni carte, ni bandeau. */
    val hasAccount: Boolean = true,
    /** La carte du compte en fin de rangée (`welcome::card_visible`), et sa croix : « Ne plus afficher » ([accountCardFinal]). */
    val accountCard: Boolean = false,
    val accountCardFinal: Boolean = false,
    /** L'ordinateur qui vient d'être appairé, sous lequel s'affiche le bandeau du compte. */
    val pairedBanner: String? = null,
    /** L'ordinateur appairé dont le bandeau attend que celui de la mise à jour de la Station soit parti. */
    val pairedPending: String? = null,
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
    val origins by model.origins.collectAsState()
    val signingIn by app.accounts.signingIn.collectAsState()
    val watch by model.watch.collectAsState()
    val watchWord = remember { fr.leslignesclaires.kybernow.core.Share.watchWord("action") }
    val offers by model.offers.collectAsState()
    val stationTasks by model.station.collectAsState()
    val fromOffer by model.fromOffer.collectAsState()
    // Sans compte : le bouton et la carte du compte, le bandeau après un appairage (règles du cœur, `welcome`).
    val accountList by app.accounts.accounts.collectAsState()
    val welcomeState by app.welcome.state.collectAsState()
    val pairedBanner by app.welcome.pairedBanner.collectAsState()
    val pairedPending by app.welcome.pairedPending.collectAsState()
    val hasAccount = accountList.isNotEmpty()
    val accountCard = remember(welcomeState, hasAccount) { app.welcome.cardVisible(hasAccount) }
    val accountCardFinal = remember(welcomeState) { app.welcome.cardCloseIsFinal() }
    val known = remember(borrowed, friends, borrowing) { borrowed.filter { model.friendPassword(it.id) != null }.map { it.id }.toSet() }
    val ps = app.ps
    val consoles by ps.store.consoles.collectAsState()
    val pairings by ps.store.pairings.collectAsState()
    val accounts by ps.store.accounts.collectAsState()
    val found by ps.found.collectAsState()
    val searched by ps.searched.collectAsState()
    val awayHosts by ps.awayHosts.collectAsState()
    val psWaking by ps.waking.collectAsState()
    val reaching by ps.reaching.collectAsState()
    val reachErrors by ps.reachErrors.collectAsState()
    // L'écran de l'appareil, suivi : un téléviseur qui coupe le HDR fait disparaître les puces.
    val hdrScreen = rememberHdrScreen()
    val hdr = remember(hosts, status, hdrScreen) { hosts.associate { it.id to Hdr.state(it, status[it.id]?.info, app.decoders, hdrScreen) } }
    val psState = remember(consoles, pairings, accounts, found, searched, awayHosts, psWaking, reaching, reachErrors, hdrScreen) {
        PsHomeState(
            consoles = consoles,
            status = consoles.associate { it.id to ps.status(it) },
            running = consoles.mapNotNull { c -> ps.live(c.hostId)?.runningApp?.let { c.id to it } }.toMap(),
            accounts = consoles.associate { it.id to ps.store.pairedAccounts(it) },
            chosen = consoles.mapNotNull { c -> ps.store.chosenAccount(c)?.let { c.id to it } }.toMap(),
            unpaired = found.values.filter { f -> consoles.none { it.hostId == f.hostId } }.sortedBy { it.name },
            waking = psWaking, reaching = reaching, reachErrors = reachErrors,
            hdrScreen = hdrScreen.hdr, hdrDecoder = PsHdr.decoder,
        )
    }
    // Au premier plan seulement : l'accueil reste composé pendant une session, et la recherche diffusait toutes les 3 s
    // sur le Wi-Fi, verrou de multidiffusion tenu, pendant le streaming (AND-19).
    androidx.lifecycle.compose.LifecycleStartEffect(Unit) {
        ps.start()
        onStopOrDispose { ps.stop() }
    }
    HomeContent(
        HomeState(
            hosts, selectedId, status, images, announced, screen, app.decoders, pads.count { !it.onScreen }, update.offered, waking, connecting, refusals,
            borrowed, friends, borrowing, borrowErrors, known, lend, lendErrors, psState, hdr, origins, signingIn, watch, watchWord,
            offers, stationTasks, fromOffer, hasAccount, accountCard, accountCardFinal, pairedBanner, pairedPending,
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
    // Une console choisie à l'accueil ; sans ordinateur ni ordinateur prêté, la première console.
    val console = state.ps.consoles.firstOrNull { it.id == selectedId }
        ?: if (hosts.isEmpty() && state.borrowed.isEmpty()) state.ps.consoles.firstOrNull() else null
    val friend = if (console != null) null else state.borrowed.firstOrNull { it.id == selectedId } ?: if (hosts.isEmpty()) state.borrowed.firstOrNull() else null
    val host = if (friend != null || console != null) null else hosts.firstOrNull { it.id == selectedId } ?: hosts.firstOrNull()

    if (host == null && friend == null && console == null) {
        Welcome(announced, state.update, actions, state.ps.unpaired, state.signingIn)
        return
    }
    BoxWithConstraints(Modifier.fillMaxSize().background(Kn.ink)) {
        val wide = maxWidth > maxHeight && maxWidth >= 720.dp
        if (host != null) {
            val st = status[host.id] ?: HostStatus()
            Backdrop(images[host.id], st.online == false, wide)
            val hero: @Composable (Float, FocusRequester?, Boolean) -> Unit = { size, focus, stacked -> Hero(host, st, state, actions, size, focus, stacked) }
            if (wide) WideHome(host.id, state, actions, hero) else TallHome(state, actions, hero)
        } else if (console != null) {
            Backdrop(null, false, wide)
            val hero: @Composable (Float, FocusRequester?, Boolean) -> Unit = { size, focus, stacked -> ConsoleHero(console, state.ps, actions.ps, size, focus, stacked) }
            if (wide) WideHome(console.id, state, actions, hero) else TallHome(state, actions, hero)
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
    // Le zoom lent est lu au dessin (`graphicsLayer`), pas à la composition : l'accueil n'est plus recomposé à chaque
    // image, et le filtre gris n'est plus refait à chaque fois (AND-29).
    val zoom = if (LocalMotion.current) {
        val t = rememberInfiniteTransition(label = "fond")
        t.animateFloat(1.0f, 1.06f, infiniteRepeatable(tween(40000, easing = LinearEasing), RepeatMode.Reverse), label = "zoom")
    } else null
    val grey = remember(offline) { if (offline) ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) }) else null }
    Box(Modifier.fillMaxSize()) {
        if (image != null) {
            Image(
                image,
                contentDescription = null,
                modifier = Modifier.fillMaxSize().graphicsLayer {
                    val z = zoom?.value ?: 1f
                    scaleX = z
                    scaleY = z
                },
                contentScale = ContentScale.Crop,
                colorFilter = grey,
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

/**
 * En haut : le logo, la mise à jour de l'app quand il y en a une, « Connecter un compte », les réglages de l'app. Les
 * boutons gardent leur libellé sur une ligne (docs/link-ui.md, « Principes ») : quand ils ne tiennent pas à côté du logo
 * (tablette en portrait, téléphone avec une mise à jour), ceux qui débordent passent dessous, alignés à droite.
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun TopBar(actions: HomeActions, update: String?, modifier: Modifier = Modifier, account: Boolean = false) {
    val s = LocalScale.current
    val phone = LocalForm.current == FormFactor.PHONE
    FlowRow(
        modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(s.dp(10f), Alignment.End),
        verticalArrangement = Arrangement.spacedBy(s.dp(10f)),
        itemVerticalAlignment = Alignment.CenterVertically,
    ) {
        // Le logo prend la place qui reste sur la première ligne : les boutons à droite.
        Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            Image(painterResource(R.drawable.kn_logo_link), contentDescription = "KyberNow Link", modifier = Modifier.height(s.dp(if (s.tv) 40f else 26f)))
        }
        update?.let {
            GlassButton(stringResource(R.string.updates_available, it), actions.appSettings, selected = true, leading = { KnIcon(KnIcons.download, size = 16f); Spacer(Modifier.width(6.dp)) }, singleLine = true)
        }
        // Sans compte : « Connecter un compte », à côté des réglages de l'app (au téléphone, l'icône seule).
        if (account) {
            if (phone) IconPill(KnIcons.person, stringResource(R.string.account_sign_in), actions.signIn)
            else GlassButton(stringResource(R.string.account_sign_in), actions.signIn, selected = true, leading = { KnIcon(KnIcons.person, size = 16f); Spacer(Modifier.width(6.dp)) }, singleLine = true)
        }
        // Au téléphone, la place manque : l'icône seule, nommée pour l'accessibilité.
        if (phone) IconPill(KnIcons.settings, stringResource(R.string.home_app_settings), actions.appSettings)
        else GlassButton(stringResource(R.string.home_app_settings), actions.appSettings, leading = { KnIcon(KnIcons.settings, size = 16f); Spacer(Modifier.width(6.dp)) }, singleLine = true)
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

/** L'écran que la session créera (même calcul que la session, `SessionMode`) ; HDR allumé et possible : « …, en HDR ». */
@Composable
fun displayLine(host: Host, screen: Screen, decoders: Decoders, hdr: Boolean = false): String {
    if (!host.virtualDisplay || !host.settings.virtualDisplay) return stringResource(R.string.home_display_as_is)
    val mode = SessionMode.pick(screen, host.settings, SessionMode.codec(host, decoders), decoders)
    val (w, h, r) = mode
    val match = SessionMode.matchesScreen(screen, host.settings, mode)
    val text = when {
        match && hdr -> R.string.home_display_match_hdr
        match -> R.string.home_display_match
        hdr -> R.string.home_display_custom_hdr
        else -> R.string.home_display_custom
    }
    return stringResource(text, w, h, r)
}

@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun Hero(host: Host, st: HostStatus, state: HomeState, actions: HomeActions, nameSize: Float, focus: FocusRequester?, stacked: Boolean = false) {
    val s = LocalScale.current
    val set = host.settings
    // Au téléviseur (540 dp de haut), tout doit tenir au-dessus de la rangée des ordinateurs.
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(if (s.tv) 7f else 10f))) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            StatusLine(st)
            // D'où vient l'ordinateur (« Compte », « Appairé »…), quand un compte est connecté.
            state.origins[host.id]?.let {
                Spacer(Modifier.width(s.dp(12f)))
                OriginChip(it)
            }
        }
        FitTitle(host.name, Modifier.fillMaxWidth(), size = nameSize, min = nameSize * 0.45f)
        if (st.accountRefused) Body(stringResource(R.string.home_account_refused), color = Kn.warn, size = 13f)
        if (st.certificateChanged) Body(stringResource(R.string.home_certificate_changed), color = Kn.warn, size = 13f)
        state.refusals[host.id]?.let { Body(relayRefusalText(it, host.name), color = Kn.warn, size = 13f) }
        // Prêté à un ami : « Prêté à Léa (jusqu'à 18 h) », le compte à rebours du préavis.
        val lent = state.lend[host.id]?.takeIf { it.lent }
        lent?.let { LentLine(it) }
        state.lendErrors[host.id]?.takeIf { it.isNotBlank() }?.let { Body(it, color = Kn.warn, size = 13f) }
        (st.info?.os ?: host.os)?.takeIf { it.isNotBlank() }?.let { Body(it, size = 16f, color = Kn.frost2, maxLines = 1) }
        val hdr = state.hdr[host.id] ?: HdrState.HIDDEN
        Row(verticalAlignment = Alignment.CenterVertically) {
            KnIcon(KnIcons.screen, tint = Kn.muted, size = 16f)
            Spacer(Modifier.width(s.dp(8f)))
            Body(displayLine(host, state.screen, state.decoders, hdr.on(set.hdr)), size = 14f, color = Kn.frost2)
        }
        Spacer(Modifier.height(s.dp(if (s.tv) 2f else 6f)))
        val first = Modifier.then(if (focus != null) Modifier.focusRequester(focus) else Modifier)
        val a11y = settingsTitle(host.name)
        if (stacked) {
            // Téléphone et tablette en portrait (comme le client web au téléphone) : le bouton sur
            // toute la largeur, puis les profils en commande segmentée, « Personnalisé », la puce
            // « HDR » et l'engrenage au bout. Sur la largeur du téléphone ; à la tablette en portrait,
            // pas plus de 560 dp. Ce qui ne tient pas à côté des profils passe dessous : jamais un
            // libellé serré ou rogné (avec Auto et « Personnalisé », la ligne dépassait le téléphone).
            if (lent != null) TakeBackButton(lent, { now -> actions.takeBack(host, now) }, first.widthIn(max = s.dp(560f)).fillMaxWidth())
            else MainButton(host, st, state, actions, first.widthIn(max = s.dp(560f)).fillMaxWidth())
            FlowRow(
                Modifier.widthIn(max = s.dp(560f)).fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)),
                itemVerticalAlignment = Alignment.CenterVertically,
            ) {
                // Les profils prennent la place qui reste sur leur ligne, jamais moins que leurs libellés.
                ProfileSegments(set, { p -> actions.setProfile(host.id, p) }, Modifier.weight(1f))
                // « Personnalisé » n'est pas un choix : il dit que les réglages s'écartent du profil.
                if (set.custom) GlassButton(stringResource(R.string.profile_custom), { actions.hostSettings(host) }, selected = true, singleLine = true)
                // La puce « HDR » à côté du profil (écran HDR seulement).
                HdrChip(hdr, set.hdr, { actions.setHdr(host, !set.hdr) })
                IconPill(KnIcons.settings, a11y) { actions.hostSettings(host) }
            }
            StationBanner(host, state, actions, Modifier.widthIn(max = s.dp(560f)))
            AccountBanner(host, state, actions, Modifier.widthIn(max = s.dp(560f)))
        } else {
            // Une seule rangée qui passe à la ligne si la place manque : rien n'est écrasé.
            FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
                // Prêté : « Reprendre l'ordinateur » à la place de « Se connecter » (pendant un prêt, pas de session d'un appareil).
                if (lent != null) TakeBackButton(lent, { now -> actions.takeBack(host, now) }, first) else MainButton(host, st, state, actions, first)
                Profile.entries.forEach { p ->
                    GlassButton(profileLabel(p), { actions.setProfile(host.id, p) }, selected = !set.custom && set.profile == p, leading = profileIcon(p), singleLine = true)
                }
                // « Personnalisé » n'est pas un choix : il dit que les réglages s'écartent du profil.
                if (set.custom) GlassButton(stringResource(R.string.profile_custom), { actions.hostSettings(host) }, selected = true, singleLine = true)
                // La puce « HDR », à côté du profil (écran HDR seulement ; à part du profil : jamais « Personnalisé »).
                HdrChip(hdr, set.hdr, { actions.setHdr(host, !set.hdr) })
                GlassButton(
                    stringResource(R.string.home_host_settings), { actions.hostSettings(host) },
                    Modifier.semantics { contentDescription = a11y },
                    leading = { KnIcon(KnIcons.settings, size = 16f); Spacer(Modifier.width(6.dp)) }, singleLine = true,
                )
            }
            StationBanner(host, state, actions)
            AccountBanner(host, state, actions, Modifier.widthIn(max = s.dp(620f)))
        }
        Body(profileHelp(set), size = 14f, color = Kn.muted)
        if (LocalForm.current == FormFactor.TV) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                KnIcon(KnIcons.gamepad, tint = Kn.muted, size = 18f)
                Spacer(Modifier.width(s.dp(8f)))
                // L'ordinateur ne crée pas de manettes (Station Mac sans le droit d'Apple) : il le dit à la place du nombre.
                Body(if (host.gamepads) padsLabel(state.pads) else stringResource(R.string.home_pads_unavailable), size = 14f, color = Kn.frost2)
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
    host.id in state.waking && st.online != true -> PrimaryButton(stringResource(R.string.home_waking), {}, modifier, singleLine = true)
    // Hors de chez vous : l'ordinateur est vérifié par le relais avant d'ouvrir la session.
    host.id in state.connecting -> PrimaryButton(stringResource(R.string.home_connecting), {}, modifier, singleLine = true)
    st.online == false && host.wakeTargets.isNotEmpty() -> PrimaryButton(stringResource(R.string.home_wake), { actions.wake(host) }, modifier, singleLine = true)
    st.online == false -> PrimaryButton(stringResource(R.string.home_retry), { actions.retry(host) }, modifier, singleLine = true)
    // Droit « Regarder » seul (compte KyberNow) : « Regarder » à la place de « Se connecter ».
    state.watch[host.id] == true -> PrimaryButton(state.watchWord, { actions.watch(host) }, modifier, singleLine = true)
    else -> PrimaryButton(stringResource(R.string.home_connect), { actions.connect(host) }, modifier, singleLine = true)
}

/**
 * Le bandeau de mise à jour de la Station (docs/link-ui.md, « Mise à jour des Stations »), sous la rangée « Se connecter »
 * et profils : un seul bandeau à cet endroit. Pendant un prêt, rien (l'ordinateur est à l'ami).
 */
@Composable
private fun StationBanner(host: Host, state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    if (state.lend[host.id]?.lent == true) return
    StationBannerView(
        state.offers[host.id], state.stationTasks[host.id], host.id in state.fromOffer,
        StationBannerActions(
            update = { force -> actions.stationUpdate(host, force) },
            later = { v -> actions.stationLater(host, v) },
            cancel = { actions.stationCancel(host) },
        ),
        modifier,
    )
}

/**
 * Après un appairage, sans compte : le bandeau du compte sous les boutons de l'ordinateur ajouté (une fois). Un seul
 * bandeau à la fois à cet endroit : tant que celui de la mise à jour de la Station est là, il attend (et n'est pas
 * compté comme vu).
 */
@Composable
private fun AccountBanner(host: Host, state: HomeState, actions: HomeActions, modifier: Modifier = Modifier) {
    if (state.hasAccount) return
    val lent = state.lend[host.id]?.lent == true
    val station = !lent && stationBannerVisible(state.offers[host.id], state.stationTasks[host.id], host.id in state.fromOffer)
    if (station) return
    if (state.pairedPending == host.id) LaunchedEffect(host.id) { actions.showPairedBanner(host) }
    if (state.pairedBanner != host.id) return
    fr.leslignesclaires.kybernow.ui.welcome.PairedBanner(
        onSignIn = { actions.dismissPairedBanner(); actions.signIn() }, onLater = actions.dismissPairedBanner, modifier = modifier,
    )
}

/**
 * Les profils en commande segmentée (téléphone et tablette en portrait), à parts égales. « Personnalisé » n'y est plus :
 * à côté, en puce (avec Auto, cinq segments ne tenaient pas sur un téléphone de 360 dp sans rogner les libellés).
 */
@Composable
private fun ProfileSegments(set: HostSettings, pick: (Profile) -> Unit, modifier: Modifier) {
    val s = LocalScale.current
    val pill = RoundedCornerShape(999.dp)
    Row(
        modifier.clip(pill).background(Kn.frost.copy(alpha = 0.06f)).border(BorderStroke(1.dp, Kn.frost.copy(alpha = 0.16f)), pill).padding(s.dp(3f)),
        horizontalArrangement = Arrangement.spacedBy(s.dp(2f)),
    ) {
        Profile.entries.forEach { p -> Segment(profileLabel(p), !set.custom && set.profile == p, Modifier.weight(1f), icon = p == Profile.AUTO) { pick(p) } }
    }
}

@Composable
private fun Segment(label: String, selected: Boolean, modifier: Modifier, icon: Boolean = false, onClick: () -> Unit) {
    val s = LocalScale.current
    val pill = RoundedCornerShape(999.dp)
    val i = remember { MutableInteractionSource() }
    Box(
        modifier.neon(i, pill).clip(pill)
            .background(if (selected) Kn.spectre.copy(alpha = 0.22f) else Kn.frost.copy(alpha = 0f))
            .border(BorderStroke(1.dp, if (selected) Kn.spectre else Kn.frost.copy(alpha = 0f)), pill)
            .selectable(selected = selected, interactionSource = i, indication = null, role = Role.Tab, onClick = onClick)
            .padding(horizontal = s.dp(8f), vertical = s.dp(10f)),
        contentAlignment = Alignment.Center,
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            // Auto : sa baguette devant le nom (docs/link-ui.md, « Profil Auto et débit adaptatif »).
            if (icon) {
                KnIcon(KnIcons.sparkles, tint = if (selected) Kn.frost else Kn.frost2, size = 14f)
                Spacer(Modifier.width(s.dp(4f)))
            }
            Text(
                label, maxLines = 1, softWrap = false, color = if (selected) Kn.frost else Kn.frost2,
                style = TextStyle(fontFamily = KnFonts.text, fontSize = s.sp(15f), fontWeight = FontWeight(if (selected) 600 else 500)),
            )
        }
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
        Profile.AUTO -> R.string.profile_auto
    },
)

/** L'icône d'un profil devant son nom : la baguette d'Auto ; rien pour les autres. */
fun profileIcon(p: Profile): (@Composable RowScope.() -> Unit)? =
    if (p != Profile.AUTO) null else ({
        KnIcon(KnIcons.sparkles, size = 16f)
        Spacer(Modifier.width(6.dp))
    })

/** La phrase du profil, ou celle de « Personnalisé ». */
@Composable
fun profileHelp(set: HostSettings) = stringResource(
    when {
        set.custom -> R.string.profile_custom_help
        set.profile == Profile.GAME -> R.string.profile_game_help
        set.profile == Profile.DESKTOP -> R.string.profile_desktop_help
        set.profile == Profile.AUTO -> R.string.profile_auto_help
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
                    // « En ligne · Mise à jour » : la Station a une mise à jour que cet appareil peut proposer.
                    Mono(withUpdateChip(statusLabel(status[h.id]), state.offers[h.id]?.chip == true), size = 10f, color = if (status[h.id]?.online == true) Kn.frost2 else Kn.muted, maxLines = 1)
                    state.origins[h.id]?.let { Mono(it, size = 10f, color = Kn.spectre, maxLines = 1) }
                }
                DropdownMenu(menu, { menu = false }) {
                    // Avec « Ouvrir le bureau » et « Regarder », « Regarder » est ici (la carte garde « Se connecter »).
                    if (state.watch[h.id] == false) DropdownMenuItem({ Text(state.watchWord) }, { menu = false; actions.watch(h) })
                    DropdownMenuItem({ Text(settingsTitle(h.name)) }, { menu = false; actions.hostSettings(h) })
                }
            }
        }
        items(state.ps.consoles, key = { it.id }) { c ->
            ConsoleCard(c, state.ps.status[c.id] ?: fr.leslignesclaires.kybernow.playstation.PsStatus.SEARCHING, c.id == selected, card, { actions.select(c.id) }, { actions.ps.settings(c) })
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
        items(state.ps.unpaired, key = { "ps:" + it.hostId }) { f -> FoundCard(f, card) { actions.ps.pair(f) } }
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
        // Sans compte : la carte du compte, en dernier (docs/link-ui.md, « Sans compte, ensuite »).
        if (state.accountCard && !state.hasAccount) item(key = "compte") {
            fr.leslignesclaires.kybernow.ui.welcome.AccountCard(
                state.accountCardFinal, actions.signIn, actions.createAccount, actions.closeAccountCard, Modifier.width(s.dp(card * 2f)),
            )
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
        TopBar(actions, state.update, account = !state.hasAccount)
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
        TopBar(actions, state.update, account = !state.hasAccount)
        Spacer(Modifier.height(s.dp(150f)))
        hero(54f, null, true)
        Spacer(Modifier.height(s.dp(6f)))
        Eyebrow(stringResource(R.string.home_your_computers))
        HostRow(state, actions, card = 160f)
    }
}

/** La pastille d'origine d'un ordinateur (docs/link-ui.md, « Comptes KyberNow ») : « Compte », « Appairé »… */
@Composable
private fun OriginChip(text: String) {
    val s = LocalScale.current
    val pill = RoundedCornerShape(999.dp)
    Box(Modifier.clip(pill).background(Kn.spectre.copy(alpha = 0.14f)).border(BorderStroke(1.dp, Kn.spectre.copy(alpha = 0.5f)), pill).padding(horizontal = s.dp(10f), vertical = s.dp(3f))) {
        Mono(text, size = 11f, color = Kn.frost, maxLines = 1)
    }
}

/**
 * Aucun ordinateur (docs/link-ui.md, « Comptes KyberNow ») : « Retrouvez vos ordinateurs », « Connecter un compte »
 * et son aide (la page du relais au téléphone et à la tablette, un code au téléviseur), puis « ou sans
 * compte » et les boutons de toujours : ceux annoncés sur le réseau, l'ajout, « Rejoindre avec un code ». Pendant
 * l'échange du retour de la page contre le compte, « Connexion au compte… ».
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
private fun Welcome(
    announced: List<Announced>, update: String?, actions: HomeActions, consoles: List<fr.leslignesclaires.kybernow.playstation.PsFound> = emptyList(),
    signingIn: Boolean = false,
) {
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
                // Sans effet pendant l'échange, mais toujours focalisable : au téléviseur, le focus ne se perd pas.
                PrimaryButton(stringResource(if (signingIn) R.string.account_signing_in else R.string.account_sign_in), { if (!signingIn) actions.signIn() }, Modifier.focusRequester(focus))
                Body(stringResource(if (s.tv) R.string.account_sign_in_help_code else R.string.account_sign_in_help_window), color = Kn.muted, size = 14f)
                Spacer(Modifier.height(s.dp(4f)))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Box(Modifier.weight(1f).height(1.dp).background(Kn.line))
                    Spacer(Modifier.width(s.dp(12f)))
                    Eyebrow(stringResource(R.string.account_without))
                    Spacer(Modifier.width(s.dp(12f)))
                    Box(Modifier.weight(1f).height(1.dp).background(Kn.line))
                }
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
                // Les consoles du réseau pas encore associées : « {nom} · PS5 », « Sur le réseau ».
                consoles.forEach { f ->
                    FocusCard({ actions.ps.pair(f) }, Modifier.fillMaxWidth()) {
                        Row(Modifier.panel().padding(s.dp(16f)).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                            KnIcon(KnIcons.gamepad, tint = Kn.spectre)
                            Spacer(Modifier.width(s.dp(12f)))
                            Column(Modifier.weight(1f)) {
                                Body(stringResource(R.string.ps_model_line, f.name, f.model), size = 17f, weight = 600)
                                Mono(stringResource(R.string.home_on_network), size = 11f, color = Kn.spectre)
                            }
                            KnIcon(KnIcons.chevron, size = 18f)
                        }
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                    GlassButton(stringResource(R.string.home_add), actions.add, leading = { KnIcon(KnIcons.plus, size = 16f); Spacer(Modifier.width(6.dp)) })
                    GlassButton(stringResource(R.string.home_join), actions.join, leading = { KnIcon(KnIcons.code, size = 16f); Spacer(Modifier.width(6.dp)) })
                }
            }
        }
    }
}
