package fr.leslignesclaires.kybernow.session

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.clickable
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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
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
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.LendAvailable
import fr.leslignesclaires.kybernow.core.LendDuration
import fr.leslignesclaires.kybernow.core.LendRequest
import fr.leslignesclaires.kybernow.core.LendSteam
import fr.leslignesclaires.kybernow.core.RouteTransport
import fr.leslignesclaires.kybernow.core.ShareAvailability
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.model.Profile
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalMotion
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.fmt
import fr.leslignesclaires.kybernow.ui.home.motorsLabel
import fr.leslignesclaires.kybernow.ui.home.profileLabel
import fr.leslignesclaires.kybernow.ui.panel
import fr.leslignesclaires.kybernow.ui.share.QualityChoices
import fr.leslignesclaires.kybernow.ui.share.qualityLabel

/** Ce qu'a donné « Copier le lien ». */
enum class ShareCopy { COPIED, FAILED }

/** Ce que l'interface de session montre et ce qu'elle peut demander. */
class SessionUi(
    /** Le nom de l'ordinateur ; pour un invité, « Invité sur {nom} » (docs/link-ui.md, « Partage »). */
    val hostName: String,
    val tv: Boolean,
    val state: SessionState,
    val actions: SessionActions,
    /** La manette à l'écran (téléphone, tablette) : ses touchers, ce qu'elle envoie, son état pour le dessin. */
    val pad: OnScreenPad = OnScreenPad(PadOut.NONE),
    /** Session d'invité (KyberNow Share) : barre réduite selon son rôle ([SessionState.shareRole]), ni profil ni réglages. */
    val guest: Boolean = false,
    /** Le nom de l'ordinateur tel quel, pour le panneau de partage de l'hôte (« Partager {nom} »). */
    val computerName: String = hostName,
    /**
     * Session d'emprunt (prêt de l'ordinateur, docs/pret.md) : [lender] est le prénom de l'hôte (« Prêté par {hôte} »),
     * [lendUntil] la fin prévue (secondes Unix ; nulle : « Jusqu'à ce que {hôte} reprenne »). Ni profil, ni réglages de
     * l'ordinateur, ni presse-papiers, ni partage.
     */
    val borrower: Boolean = false,
    val lender: String = "",
    val lendUntil: Long? = null,
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
    /** Ce qui ouvre le menu (réglages de l'ordinateur), pour l'aide de l'écran de connexion. */
    var backOpensMenu by mutableStateOf(true)
    var combo by mutableStateOf(true)
    /** Modificateurs collants de la rangée de touches (Ctrl, Alt, ⊞). */
    var sticky by mutableStateOf(emptySet<Int>())
    /** Manette à l'écran affichée (réglage de l'ordinateur, bouton de la barre), sa taille (0 à 2) et son opacité (%). */
    var padShown by mutableStateOf(false)
    var padSize by mutableStateOf(1)
    var padOpacity by mutableStateOf(70)

    // KyberNow Share, côté invité.
    /** La feuille « Qualité de l'image » (les cinq choix, la qualité reçue) est ouverte. */
    var qualityOpen by mutableStateOf(false)

    // KyberNow Share, côté hôte.
    /** Le panneau du partage (code, lien, QR code, invités) est ouvert. */
    var shareOpen by mutableStateOf(false)
    /** Ce que « Partager » peut faire avec cet ordinateur d'après son info ; nul tant qu'on ne le sait pas (« Partager » reste actif : le toucher le vérifie). */
    var shareAvailability by mutableStateOf<ShareAvailability?>(null)
    /** Pourquoi « Partager » est inactif (le texte du cœur) ; nul s'il est disponible. */
    var shareUnavailable by mutableStateOf<String?>(null)
    /** Un appel d'Admin du partage est en cours. */
    var shareBusy by mutableStateOf(false)
    /** Le dernier refus d'Admin, dans le panneau. */
    var shareError by mutableStateOf<String?>(null)
    /** Le résultat de « Copier le lien » (« Lien copié. » ou « Copie impossible… »), montré un instant. */
    var shareCopy by mutableStateOf<ShareCopy?>(null)

    // Prêt de l'ordinateur, côté ami.
    /** « Rendre l'ordinateur » demande confirmation (« Rendre l'ordinateur à {hôte} ? »). */
    var giveBackConfirm by mutableStateOf(false)
    /** « Revenir » après une fin subie (réseau perdu) : où en est le nouvel emprunt ; nul avant. */
    var comeBack by mutableStateOf<ComeBack?>(null)

    // Prêt de l'ordinateur, côté hôte.
    /** Le panneau « Prêter {nom} » (durée, compte Steam, code, demandes) est ouvert. */
    var lendOpen by mutableStateOf(false)
    /** Ce que « Prêter l'ordinateur… » peut faire, d'après l'info de l'ordinateur ; nul tant qu'on ne le sait pas. */
    var lendAvailable by mutableStateOf<LendAvailable?>(null)
    /** Un appel d'Admin du prêt est en cours. */
    var lendBusy by mutableStateOf(false)
    /** Le dernier refus d'Admin, dans le panneau du prêt. */
    var lendError by mutableStateOf<String?>(null)
    /** Le résultat de « Copier le lien » du code de prêt. */
    var lendCopy by mutableStateOf<ShareCopy?>(null)
    /** La durée et le compte Steam choisis pour le prochain code (proposés : 2 heures, son compte Steam). */
    var lendDuration by mutableStateOf(LendDuration.DEFAULT)
    var lendSteam by mutableStateOf(LendSteam.FRIEND)
    /** L'avertissement (« Votre ami joue dans votre session Windows… ») : montré tant qu'aucun code n'a été créé sur cet appareil. */
    var lendWarning by mutableStateOf(true)

    /** « Prêter l'ordinateur… » est possible : l'ordinateur le permet, ou on ne sait pas encore. */
    val lendUsable get() = lendAvailable.let { it == null || it == LendAvailable.ON }

    /** « Partager » est actif : l'ordinateur le permet, ou on ne sait pas encore (le toucher le vérifie). */
    val shareUsable get() = shareAvailability.let { it == null || it == ShareAvailability.AVAILABLE }

    /** Un invité n'a ni le clavier ni la souris à moins que son rôle les donne ; l'hôte, toujours. */
    fun keyboardMouseAllowed(role: ShareRole = state.shareRole.value) = !guest || role.keyboardMouse

    /** Un menu, une feuille, la barre, le clavier… par-dessus l'image (la manette à part). */
    val covering get() = leaving || !passageDone || menuOpen || barOpen || keyboardOpen || statsOpen || typing || shareOpen || qualityOpen || lendOpen || giveBackConfirm

    /** Quelque chose est à l'écran par-dessus l'image. */
    val anything get() = covering || padShown
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
    /** Affiche ou retire la manette à l'écran (retenu pour l'ordinateur). */
    fun setOnScreenPad(on: Boolean)
    /** Disposition de la manette à l'écran : « Simple » ou « Complète » (retenue pour l'ordinateur). */
    fun setPadLayout(simple: Boolean)

    // KyberNow Share, côté invité (docs/link-ui.md, « Partage », « Qualité de l'image »).
    /** La « Qualité de l'image » choisie : dite à la Station, qui relance l'image de l'invité seulement ; retenue pour la fois suivante. */
    fun setShareQuality(quality: ShareQuality)

    // KyberNow Share, côté hôte (docs/link-ui.md, « Partage »).
    /** « Partager » : ouvre le partage de l'écran principal (ou montre celui qui est ouvert) et son panneau. */
    fun share()
    /** « Nouveau code » : les invités déjà là restent. */
    fun shareNewCode()
    /** « Donner les manettes » (`on`) ou « Reprendre les manettes » : le clavier et la souris de l'invité ne changent pas. */
    fun shareSetPads(guest: ShareGuest, on: Boolean)
    /** « Donner le clavier et la souris » (`on`) ou « Reprendre le clavier et la souris » : ses manettes ne changent pas. */
    fun shareSetKeyboardMouse(guest: ShareGuest, on: Boolean)
    /** « Accepter » la demande d'un invité venu par le relais (`pending`) : il entre dans le partage, spectateur. */
    fun shareAccept(guest: ShareGuest)
    /** « Refuser » la demande d'un invité venu par le relais. */
    fun shareRefuse(guest: ShareGuest)
    /** « Retirer » un invité. */
    fun shareRemove(guest: ShareGuest)
    /** « Arrêter le partage » : tout le monde sort. */
    fun shareStop()
    /** « Copier le lien ». */
    fun shareCopyLink()

    // Prêt de l'ordinateur, côté ami (docs/link-ui.md, « Prêt de l'ordinateur ») : rien à faire hors d'un emprunt.
    /** « Rendre l'ordinateur », après la confirmation : la session finit, l'ordinateur revient à l'hôte. */
    fun giveBack() {}
    /** « Revenir » (pendant 15 min après une fin subie) : la même demande d'emprunt, puis une nouvelle session. */
    fun comeBack() {}
    /** « Annuler » l'attente de l'hôte pendant « Revenir ». */
    fun comeBackCancel() {}

    // Prêt de l'ordinateur, côté hôte.
    /** « Prêter l'ordinateur… » : ouvre le panneau (et relit la vue du prêt). */
    fun lend() {}
    /** « Créer un code » avec la durée et le compte Steam choisis. */
    fun lendCreateCode() {}
    /** « Annuler le code ». */
    fun lendCancelCode() {}
    /** « Accepter » ou « Refuser » une demande d'emprunt. */
    fun lendAnswer(request: LendRequest, accept: Boolean) {}
    /** « Copier le lien » du code de prêt. */
    fun lendCopyLink() {}
}

/** « Revenir » : l'emprunt refait ([WORKING], « Préparation de l'ordinateur »), l'attente de l'hôte, ou un refus ([message]). */
data class ComeBack(val step: Step, val message: String? = null) {
    enum class Step { WORKING, WAITING, FAILED }
}

@Composable
fun SessionOverlay(ui: SessionUi) {
    val ended by ui.state.ended.collectAsState()
    val role by ui.state.shareRole.collectAsState()
    Box(Modifier.fillMaxSize()) {
        // La manette sous tout le reste : la barre, la rangée du clavier et les feuilles gardent leurs touchers.
        // Un invité sans les manettes n'en a pas : rien de ce qu'il toucherait ne partirait.
        if (!ui.tv && ui.padShown && (!ui.guest || role.pads) && ui.passageDone && !ui.leaving && ended?.error == null) OnScreenPadLayer(ui)
        Capsules(ui, Modifier.align(Alignment.TopCenter))
        if (!ui.passageDone) Passage(ui)
        if (ui.tv) {
            AnimatedVisibility(ui.menuOpen, enter = fadeIn(tween(160)), exit = fadeOut(tween(160))) { TvMenu(ui) }
        } else {
            // Rien sur l'image tant que la barre est fermée : elle s'ouvre par le geste Retour.
            AnimatedVisibility(ui.barOpen, Modifier.align(Alignment.TopCenter), enter = slideInVertically { -it } + fadeIn(), exit = slideOutVertically { -it } + fadeOut()) { PhoneBar(ui) }
            if (ui.keyboardOpen) KeyRow(ui, Modifier.align(Alignment.BottomCenter).imePadding())
        }
        if (ui.statsOpen) Stats(ui, Modifier.align(if (ui.tv) Alignment.TopEnd else Alignment.BottomEnd).padding(LocalScale.current.dp(20f)))
        if (ui.keysOpen) KeysSheet(ui, Modifier.align(Alignment.Center))
        if (ui.qualityOpen && ui.guest) QualitySheet(ui, Modifier.align(Alignment.Center))
        if (ui.typing) TypeText(ui, Modifier.align(Alignment.Center))
        if (ui.shareOpen) SharePanel(ui, Modifier.align(Alignment.Center))
        if (ui.lendOpen && !ui.guest && !ui.borrower) LendPanel(ui, Modifier.align(Alignment.Center))
        if (ui.giveBackConfirm && ui.borrower) GiveBackConfirm(ui, Modifier.align(Alignment.Center))
        ended?.takeIf { it.error != null }?.let { EndedCard(ui, it, Modifier.align(Alignment.Center)) }
        // Un invité et l'ami qui emprunte ne changent rien à l'ordinateur : pas de remise en place à attendre.
        AnimatedVisibility(ui.leaving, enter = fadeIn(tween(220)), exit = fadeOut(tween(160))) { Leaving(ui.guest || ui.borrower) }
    }
}

/**
 * Pendant la fin de session : l'image s'efface, l'ordinateur se remet (un invité ne change rien à
 * l'ordinateur : pas de remise en place à attendre).
 */
@Composable
private fun Leaving(guest: Boolean) {
    val s = LocalScale.current
    Box(Modifier.fillMaxSize().background(Kn.ink.copy(alpha = 0.9f)), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
            androidx.compose.material3.CircularProgressIndicator(Modifier.size(s.dp(36f)), color = Kn.spectre, strokeWidth = s.dp(3f))
            Body(stringResource(R.string.session_leaving), size = 20f, weight = 600)
            if (!guest) Body(stringResource(R.string.session_leaving_detail), size = 14f, color = Kn.muted)
        }
    }
}

// MARK: - Passage

/**
 * L'écran de connexion : l'accueil se coupe le long de la couture à 45°, le tube néon s'allume ;
 * le nom de l'ordinateur et les trois étapes (docs/link-ui.md), ou « La Station redémarre… »
 * quand le cœur la redémarre en dernier recours, comment ouvrir le menu, Annuler.
 * À la première image, les deux moitiés s'écartent sur l'image seule.
 */
@Composable
private fun Passage(ui: SessionUi) {
    val s = LocalScale.current
    val phase by ui.state.phase.collectAsState()
    val ended by ui.state.ended.collectAsState()
    val restarting by ui.state.stationRestarting.collectAsState()
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
        // Un échec laisse sa carte seule (EndedCard).
        if (open.value < 0.5f && ended == null) {
            val step = when (phase) {
                Phase.PREPARING -> 0
                Phase.CONNECTING -> 1
                Phase.FIRST_IMAGES -> 2
                else -> 3
            }
            Column(
                Modifier.align(Alignment.BottomCenter).padding(bottom = s.dp(if (ui.tv) 56f else 24f)).padding(horizontal = s.dp(20f))
                    .panel(22.dp).padding(horizontal = s.dp(26f), vertical = s.dp(18f)).widthIn(min = s.dp(280f), max = s.dp(520f)),
                verticalArrangement = Arrangement.spacedBy(s.dp(8f)),
            ) {
                Title(ui.hostName, size = if (ui.tv) 30f else 22f)
                Spacer(Modifier.height(s.dp(2f)))
                // La Station redémarre (dernier recours) : à la place des étapes, qui reprennent ensuite.
                if (restarting) StepLine(stringResource(R.string.station_restarting), done = false, current = true)
                // L'emprunt : « Préparation de l'ordinateur » en première étape (le mode console de la Station).
                else listOf(if (ui.borrower) R.string.lend_preparing else R.string.session_preparing, R.string.session_connecting, R.string.session_first_images).forEachIndexed { i, label ->
                    StepLine(stringResource(label), done = i < step, current = i == step)
                }
                hint(ui)?.let { Body(it, size = 13f, color = Kn.muted, modifier = Modifier.padding(top = s.dp(4f))) }
                Spacer(Modifier.height(s.dp(2f)))
                GlassButton(stringResource(R.string.cancel), { ui.actions.quit() })
            }
        }
    }
}

/** Une étape : faite (coche), en cours (point qui bat), à venir (cercle). */
@Composable
private fun StepLine(label: String, done: Boolean, current: Boolean) {
    val s = LocalScale.current
    val a = if (current && LocalMotion.current) {
        val pulse = rememberInfiniteTransition(label = "étape")
        pulse.animateFloat(1f, 0.35f, infiniteRepeatable(tween(700), RepeatMode.Reverse), label = "battement").value
    } else 1f
    Row(verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(s.dp(16f)), contentAlignment = Alignment.Center) {
            when {
                done -> KnIcon(KnIcons.check, tint = Kn.spectre, size = 16f)
                current -> Box(Modifier.size(s.dp(8f)).alpha(a).clip(CircleShape).background(Kn.neonCore))
                else -> Box(Modifier.size(s.dp(8f)).clip(CircleShape).border(1.dp, Kn.muted, CircleShape))
            }
        }
        Spacer(Modifier.width(s.dp(12f)))
        Body(label, size = 15f, color = if (done || current) Kn.frost else Kn.muted, weight = if (current) 600 else 400)
    }
}

/** Comment ouvrir le menu ou la barre, d'après les réglages de l'ordinateur ; nul s'il n'y a rien à dire. */
@Composable
private fun hint(ui: SessionUi): String? {
    val role by ui.state.shareRole.collectAsState()
    // Un invité sans les manettes ne forme pas la combinaison de la manette (le cœur ne reçoit pas ses touches) : Retour ouvre le menu, la combinaison non.
    val combo = ui.combo && !(ui.guest && !role.pads)
    return when {
        !ui.tv -> stringResource(R.string.session_hint_touch)
        ui.backOpensMenu && combo -> stringResource(R.string.session_hint_tv_both)
        ui.backOpensMenu -> stringResource(R.string.session_hint_tv_back)
        combo -> stringResource(R.string.session_hint_combo)
        else -> null
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
            // La demande d'un invité se choisit : elle ouvre le panneau du partage (docs/link-ui.md, « Partage ») ; celle d'un
            // ami qui veut emprunter l'ordinateur, le panneau du prêt.
            val open = when (capsule) {
                is Capsule.ShareRequest -> Modifier.clip(RoundedCornerShape(999.dp)).clickable { ui.actions.share() }
                is Capsule.LendRequest -> Modifier.clip(RoundedCornerShape(999.dp)).clickable { ui.actions.lend() }
                else -> Modifier
            }
            Row(
                Modifier.panel(999.dp).then(open).padding(horizontal = s.dp(16f), vertical = s.dp(9f)),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (capsule is Capsule.Reconnecting) {
                    Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(Kn.spectre))
                    Spacer(Modifier.width(s.dp(10f)))
                }
                Body(capsuleText(capsule), size = 14f)
                if (capsule is Capsule.Reconnecting) {
                    Spacer(Modifier.width(s.dp(12f)))
                    GlassButton(stringResource(if (ui.guest) R.string.share_leave else R.string.session_disconnect), { ui.actions.quit() })
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
    is Capsule.ShareRights -> stringResource(c.text)
    // L'avis de la Station sur l'image de l'invité (« Image allégée pour suivre. ») : le texte du cœur, tel quel.
    is Capsule.ShareNotice -> c.text
    // Hôte : une demande venue par le relais, avis bref « {prénom} demande à rejoindre. » (le choisir ouvre le panneau).
    is Capsule.ShareRequest -> stringResource(R.string.share_request_notice, c.name)
    // Emprunt : le préavis de 2 min, tel que le cœur l'écrit.
    is Capsule.LendNotice -> c.text
    // Hôte : « Léa veut emprunter l'ordinateur. » (la choisir ouvre le panneau du prêt).
    is Capsule.LendRequest -> stringResource(R.string.lend_request_wants, c.name)
    is Capsule.LendHostNotice -> c.text
}

// MARK: - Téléphone et tablette

@Composable
private fun PhoneBar(ui: SessionUi) {
    val s = LocalScale.current
    val sec by ui.state.second.collectAsState()
    val role by ui.state.shareRole.collectAsState()
    val first = remember { FocusRequester() }
    // Ouverte à la manette : le focus sur le premier élément, et de nouveau quand la feuille des
    // touches se ferme. Une image d'attente : la vue d'interface vient de passer de GONE à visible.
    LaunchedEffect(ui.padBar, ui.keysOpen, ui.shareOpen, ui.qualityOpen, ui.lendOpen, ui.giveBackConfirm) {
        if (ui.padBar && !ui.keysOpen && !ui.shareOpen && !ui.qualityOpen && !ui.lendOpen && !ui.giveBackConfirm) {
            withFrameNanos { }
            runCatching { first.requestFocus() }
        }
    }
    // Petit téléphone en paysage : « Partager » n'a plus que son icône, pour laisser de la place aux réglages qui défilent.
    BoxWithConstraints {
        val compact = maxWidth < 700.dp
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            // Les réglages défilent ; « Partager » et sa pastille, « Se déconnecter » et la croix restent toujours visibles, à droite.
            Row(
                Modifier.padding(s.dp(10f)).panel(16.dp).padding(s.dp(8f)),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(s.dp(6f)),
            ) {
                Row(
                    Modifier.weight(1f, fill = false).horizontalScroll(rememberScrollState()),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(s.dp(6f)),
                ) {
                    when {
                        ui.guest -> GuestBarItems(ui, sec, role, first)
                        ui.borrower -> BorrowerBarItems(ui, sec, first)
                        else -> HostBarItems(ui, sec, first)
                    }
                }
                // Par le relais (hors de chez vous) : la puce, dans la partie fixe comme « Partager ».
                RouteChip(ui)
                // Un invité n'a rien à partager ; l'ami qui emprunte a « Prêté par {hôte} », le temps et « Rendre l'ordinateur ».
                if (ui.borrower) LentItems(ui)
                else if (!ui.guest) {
                    ShareItems(ui, compact)
                    LendItems(ui)
                }
                // Un invité quitte le partage : l'ordinateur, lui, ne change pas. L'ami qui emprunte se déconnecte sans rendre
                // l'ordinateur (il peut revenir pendant 15 min).
                GlassButton(stringResource(if (ui.guest) R.string.share_leave else R.string.session_disconnect), { ui.actions.quit() })
                BarIcon(KnIcons.close, stringResource(R.string.session_close)) { ui.barOpen = false }
            }
            if (ui.padBar) Body(stringResource(R.string.session_pad_bar), Modifier.panel(999.dp).padding(horizontal = s.dp(14f), vertical = s.dp(6f)), size = 12f, color = Kn.frost2)
        }
    }
}

/**
 * Les mesures en direct (latence, images par seconde, débit) ; un toucher ouvre leur panneau. Ce qui ne touche
 * que l'appareil : un invité l'a aussi.
 */
@Composable
internal fun StatsCard(ui: SessionUi, sec: Second?, modifier: Modifier) {
    val s = LocalScale.current
    val statsLabel = stringResource(R.string.session_stats)
    FocusCard({ ui.statsOpen = !ui.statsOpen }, modifier.semantics { contentDescription = statsLabel }) {
        Column(Modifier.padding(horizontal = s.dp(8f))) {
            Mono(sec?.latencyMs?.let { fmt("%.1f ms", it) } ?: "—", size = 14f, color = Kn.frost, weight = 500)
            Mono(stringResource(R.string.session_fps, sec?.fps ?: 0) + " · " + fmt("%.0f Mbit/s", sec?.mbps ?: 0.0), size = 9f)
        }
    }
}

/** La barre de l'hôte (ordinateur ajouté à l'appareil) : mesures, profils, toucher, manette, souris, écran, clavier, touches. */
@Composable
private fun HostBarItems(ui: SessionUi, sec: Second?, first: FocusRequester) {
    StatsCard(ui, sec, Modifier.focusRequester(first))
    Profile.entries.forEach { p -> GlassButton(profileLabel(p), { ui.actions.setProfile(p) }, selected = ui.profile == p) }
    GlassButton(stringResource(if (ui.directTouch) R.string.touch_direct else R.string.touch_trackpad), { ui.actions.setDirectTouch(!ui.directTouch) })
    GlassButton(stringResource(R.string.pad_on_screen), { ui.actions.setOnScreenPad(!ui.padShown) }, selected = ui.padShown)
    GlassButton(
        stringResource(listOf(R.string.session_mouse_auto, R.string.mouse_desktop, R.string.mouse_game)[ui.mouseMode]),
        { ui.actions.setMouseMode((ui.mouseMode + 1) % 3) },
    )
    screenChoice(ui)?.let { c -> GlassButton(stringResource(R.string.session_screen, c.index + 1, c.width, c.height), { ui.actions.pickDisplay(c.next) }) }
    BarIcon(KnIcons.keyboard, stringResource(R.string.session_keyboard)) { ui.actions.openKeyboard(true) }
    GlassButton(stringResource(R.string.session_keys), { ui.keysOpen = true })
}

/**
 * La barre réduite d'un invité (docs/link-ui.md, « Partage », comme le client web) : ce qui ne touche que
 * l'appareil (la latence et les statistiques, « Qualité de l'image » qui montre la qualité reçue), son rôle
 * (« Spectateur » sans aucun droit, « Joueur » dès qu'il en a un) et « Quitter le partage » ; la manette à l'écran
 * seulement avec les manettes ; le toucher, le mode de la souris, le clavier et les touches seulement avec le
 * clavier et la souris. Ni profil, ni réglages de l'ordinateur, ni presse-papiers.
 */
@Composable
private fun GuestBarItems(ui: SessionUi, sec: Second?, role: ShareRole, first: FocusRequester) {
    val s = LocalScale.current
    StatsCard(ui, sec, Modifier.focusRequester(first))
    QualityButton(ui)
    Body(
        roleLabel(role), Modifier.clip(RoundedCornerShape(999.dp)).background(Kn.frost.copy(alpha = 0.06f)).padding(horizontal = s.dp(14f), vertical = s.dp(8f)),
        size = 14f, weight = 600, color = Kn.frost,
    )
    if (role.pads) GlassButton(stringResource(R.string.pad_on_screen), { ui.actions.setOnScreenPad(!ui.padShown) }, selected = ui.padShown)
    if (role.keyboardMouse) {
        GlassButton(stringResource(if (ui.directTouch) R.string.touch_direct else R.string.touch_trackpad), { ui.actions.setDirectTouch(!ui.directTouch) })
        GlassButton(
            stringResource(listOf(R.string.session_mouse_auto, R.string.mouse_desktop, R.string.mouse_game)[ui.mouseMode]),
            { ui.actions.setMouseMode((ui.mouseMode + 1) % 3) },
        )
        BarIcon(KnIcons.keyboard, stringResource(R.string.session_keyboard)) { ui.actions.openKeyboard(true) }
        GlassButton(stringResource(R.string.session_keys), { ui.keysOpen = true })
    }
}

/**
 * « Qualité de l'image » d'un invité : montre la qualité reçue (« 1080p · 60 Hz », écrite par le cœur) et ouvre
 * [QualitySheet] pour en changer. Tant que la Station ne dit pas la taille de l'image, aucune qualité n'est montrée.
 */
@Composable
private fun QualityButton(ui: SessionUi) {
    val image by ui.state.shareImage.collectAsState()
    GlassButton(stringResource(R.string.share_quality_title), { ui.qualityOpen = true }, selected = ui.qualityOpen, detail = image?.label)
}

/**
 * La feuille « Qualité de l'image » d'un invité : les cinq choix (« Auto », « Comme l'hôte », « 4K · 60 Hz »,
 * « 1080p · 60 Hz », « 720p · 30 Hz »), celui qu'il a choisi allumé, et la qualité reçue quand la Station la dit. Choisir relance son
 * image seulement (jamais celle de l'hôte), puis la feuille se ferme.
 */
@Composable
private fun QualitySheet(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val chosen by ui.state.shareQuality.collectAsState()
    val image by ui.state.shareImage.collectAsState()
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    // Pleine, comme le panneau du partage : par-dessus le menu du téléviseur, le verre laisserait transparaître les tuiles.
    Column(modifier.padding(s.dp(20f)).widthIn(max = s.dp(720f)).opaquePanel().padding(s.dp(22f)), verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
        Eyebrow(stringResource(R.string.share_quality_title))
        image?.label?.let { Mono(it, size = 14f, color = Kn.spectre) }
        QualityChoices(chosen, { ui.actions.setShareQuality(it); ui.qualityOpen = false }, focus = first)
        GlassButton(stringResource(R.string.session_close), { ui.qualityOpen = false })
    }
}

/**
 * « Partager » ; un partage ouvert se voit aussi par sa pastille (« 2 invités »), qui rouvre le panneau. Dans la
 * partie fixe de la barre, à côté de « Se déconnecter » : on les voit sans la faire défiler. [compact] : un petit
 * écran, « Partager » n'a que son icône (nommée pour l'accessibilité).
 *
 * Impossible (Station trop ancienne, partage coupé), il est estompé mais reste sélectionnable : au toucher et à la
 * télécommande il ouvre le panneau réduit à la raison ; à la souris il est inactif et son infobulle dit pourquoi
 * (`SessionActivity.share`).
 */
@Composable
private fun ShareItems(ui: SessionUi, compact: Boolean) {
    val s = LocalScale.current
    val share by ui.state.share.collectAsState()
    val label = stringResource(R.string.share_button)
    val dim = if (ui.shareUsable) 1f else 0.5f
    WithReason(if (ui.shareUsable) null else ui.shareUnavailable) {
        if (compact) {
            val shape = RoundedCornerShape(12.dp)
            FocusCard({ ui.actions.share() }, Modifier.semantics { contentDescription = label }, shape = shape) {
                Box(Modifier.alpha(dim).background(if (share != null) Kn.spectre.copy(alpha = 0.22f) else Kn.frost.copy(alpha = 0.07f)).padding(s.dp(10f))) {
                    KnIcon(KnIcons.share, size = 18f, tint = Kn.frost)
                }
            }
        } else {
            GlassButton(
                label, { ui.actions.share() }, Modifier.alpha(dim),
                selected = share != null,
                leading = { KnIcon(KnIcons.share, size = 16f); Spacer(Modifier.width(s.dp(6f))) },
            )
        }
    }
    // La pastille, à partir d'un invité (jamais « 0 invité ») : elle rouvre le panneau.
    share?.takeIf { it.guests.isNotEmpty() }?.let { GlassButton(guestsLabel(it.guests.size), { ui.actions.share() }, selected = true) }
    // Les demandes venues par le relais ont la leur, à côté (« 1 demande ») : elle rouvre aussi le panneau.
    share?.takeIf { it.requests.isNotEmpty() }?.let { GlassButton(requestsLabel(it.requests.size), { ui.actions.share() }, selected = true) }
}

/**
 * La puce « Par le relais » (docs/link-ui.md, « Hors de chez vous »), dans la partie toujours visible de la barre ;
 * un toucher ouvre les statistiques, où la ligne « Chemin » dit lequel (« Par le relais (TCP) » compris). Rien sur le réseau.
 */
@Composable
private fun RouteChip(ui: SessionUi) {
    val s = LocalScale.current
    val route by ui.state.route.collectAsState()
    if (route?.viaRelay != true) return
    GlassButton(
        stringResource(R.string.route_relay), { ui.statsOpen = !ui.statsOpen }, selected = true,
        leading = { KnIcon(KnIcons.network, size = 16f); Spacer(Modifier.width(s.dp(6f))) },
    )
}

/** « Direct », « Par le relais », « Par le relais (TCP) » : la ligne « Chemin » des statistiques (textes de docs/link-ui.md). */
@Composable
fun routeLabel(transport: RouteTransport) = stringResource(
    when (transport) {
        RouteTransport.DIRECT -> R.string.route_direct
        RouteTransport.RELAY -> R.string.route_relay
        RouteTransport.RELAY_TCP -> R.string.route_relay_tcp
    },
)

/** L'infobulle d'un élément inactif à la souris : [reason], au survol ; sans raison, l'élément seul. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun WithReason(reason: String?, content: @Composable () -> Unit) {
    if (reason == null) {
        content()
        return
    }
    TooltipBox(
        positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
        tooltip = { PlainTooltip(containerColor = Kn.panel, contentColor = Kn.frost) { Body(reason, size = 13f) } },
        state = rememberTooltipState(),
    ) { content() }
}

/** « Spectateur » (aucun droit) ou « Joueur » (un droit au moins : les manettes, le clavier et la souris, ou les deux). */
@Composable
fun roleLabel(role: ShareRole) = stringResource(if (role.plays) R.string.share_role_player else R.string.share_role_spectator)

/** « 0 invité », « 1 invité », « 2 invités » (le pluriel suit la langue de l'app). */
@Composable
fun guestsLabel(count: Int) = pluralStringResource(R.plurals.share_guests, count, count)

/** « 1 demande », « 2 demandes » (le pluriel suit la langue de l'app) : les invités venus par le relais qui attendent l'hôte. */
@Composable
fun requestsLabel(count: Int) = pluralStringResource(R.plurals.share_requests, count, count)

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
internal fun BarIcon(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, Modifier.semantics { contentDescription = label }, shape = RoundedCornerShape(12.dp)) {
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
            KeyCap(stringResource(R.string.keys_escape)) { ui.actions.key(Sc.ESC) }
            KeyCap("Tab") { ui.actions.key(Sc.TAB) }
            for ((code, label) in listOf(Sc.CTRL to "Ctrl", Sc.ALT to "Alt", Sc.WIN to "⊞")) {
                KeyCap(label, selected = code in ui.sticky) { ui.sticky = if (code in ui.sticky) ui.sticky - code else ui.sticky + code }
            }
            KeyCap("←") { ui.actions.key(Sc.LEFT) }
            KeyCap("↑") { ui.actions.key(Sc.UP) }
            KeyCap("↓") { ui.actions.key(Sc.DOWN) }
            KeyCap("→") { ui.actions.key(Sc.RIGHT) }
            Sc.F.forEachIndexed { i, c -> KeyCap("F${i + 1}") { ui.actions.key(c) } }
            KeyCap(stringResource(R.string.keys_ctrl_alt_del_short)) { ui.actions.shortcut(Shortcut.CTRL_ALT_DEL) }
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
    val audio by ui.state.audioOutput.collectAsState()
    val role by ui.state.shareRole.collectAsState()
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Kn.ink.copy(alpha = 0.72f))) {
        Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = s.dp(32f), vertical = s.dp(28f)), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(16f))) {
            // La santé du flux ne touche que l'appareil : un invité l'a aussi (mais ni profil ni réglages de l'ordinateur).
            Row(Modifier.panel(999.dp).padding(horizontal = s.dp(26f), vertical = s.dp(12f)), horizontalArrangement = Arrangement.spacedBy(s.dp(26f)), verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(Kn.spectre))
                Mono(sec?.latencyMs?.let { fmt("%.1f ms", it) } ?: "—", size = 15f, color = Kn.frost)
                Mono(stringResource(R.string.session_fps, sec?.fps ?: 0), size = 15f, color = Kn.frost)
                Mono(fmt("%.0f Mbit/s", sec?.mbps ?: 0.0), size = 15f, color = Kn.frost)
                mode?.let { (w, h, r) -> Mono("$w × $h · $r Hz", size = 15f, color = Kn.frost) }
                Mono(stringResource(R.string.session_losses, (sec?.lostFrames ?: 0) + (sec?.holes ?: 0)), size = 15f)
            }
            Eyebrow(
                when {
                    ui.guest -> "${ui.hostName} · ${roleLabel(role)}"
                    ui.borrower -> "${ui.hostName} · ${stringResource(R.string.lend_lent_by, ui.lender)}"
                    else -> "${ui.hostName} · ${profileLabel(ui.profile)}"
                },
            )
            when {
                ui.guest -> GuestTiles(ui, role, first, audio)
                ui.borrower -> BorrowerTiles(ui, first, audio)
                else -> TvTiles(ui, first, audio)
            }
        }
    }
}

/**
 * Le menu d'un invité : reprendre, le son, les statistiques et « Qualité de l'image » (ce qui ne touche que
 * l'appareil), puis ce que son rôle permet (la souris, le texte et les touches avec le clavier et la souris), et
 * quitter le partage.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun GuestTiles(ui: SessionUi, role: ShareRole, first: FocusRequester, audio: Pair<Int, Double>?) {
    val s = LocalScale.current
    val image by ui.state.shareImage.collectAsState()
    val chosen by ui.state.shareQuality.collectAsState()
    val route by ui.state.route.collectAsState()
    FlowRow(Modifier.widthIn(max = s.dp(900f)), horizontalArrangement = Arrangement.spacedBy(s.dp(14f), Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(s.dp(14f)), maxItemsInEachRow = 5) {
        Tile(KnIcons.play, stringResource(R.string.session_resume), null, Modifier.focusRequester(first), primary = true) { ui.actions.resume() }
        // Par le relais (hors de chez vous) : une tuile, comme la puce de la barre ; elle ouvre les statistiques (« Chemin »).
        if (route?.viaRelay == true) Tile(KnIcons.network, stringResource(R.string.route_relay), null, selected = true) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.sound, stringResource(R.string.session_sound), audio?.let { (c, _) -> stringResource(R.string.session_sound_channels, c) }) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.pulse, stringResource(R.string.session_stats), null) { ui.statsOpen = !ui.statsOpen }
        // La qualité reçue (« 1080p · 60 Hz ») quand la Station la dit, sinon celle qu'on a choisie (« Auto »).
        Tile(KnIcons.screen, stringResource(R.string.share_quality_title), image?.label ?: qualityLabel(chosen)) { ui.qualityOpen = true }
        if (role.keyboardMouse) {
            Tile(KnIcons.remote, stringResource(R.string.session_remote_mouse), stringResource(if (ui.remoteMouse) R.string.session_on else R.string.session_off), selected = ui.remoteMouse) { ui.actions.setRemoteMouse(!ui.remoteMouse) }
            Tile(KnIcons.gamepad, stringResource(R.string.session_pad_mouse), stringResource(if (ui.padMouse) R.string.session_on else R.string.session_off), selected = ui.padMouse) { ui.actions.setPadMouse(!ui.padMouse) }
            Tile(KnIcons.keyboard, stringResource(R.string.session_type_text), null) { ui.typing = true }
            Tile(KnIcons.keys, stringResource(R.string.session_keys), stringResource(R.string.keys_ctrl_alt_del_short)) { ui.keysOpen = true }
        }
        Tile(KnIcons.exit, stringResource(R.string.share_leave), null, danger = true) { ui.actions.quit() }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun TvTiles(ui: SessionUi, first: FocusRequester, audio: Pair<Int, Double>?) {
    val s = LocalScale.current
    val share by ui.state.share.collectAsState()
    val route by ui.state.route.collectAsState()
    FlowRow(Modifier.widthIn(max = s.dp(900f)), horizontalArrangement = Arrangement.spacedBy(s.dp(14f), Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(s.dp(14f)), maxItemsInEachRow = 5) {
        Tile(KnIcons.play, stringResource(R.string.session_resume), null, Modifier.focusRequester(first), primary = true) { ui.actions.resume() }
        if (route?.viaRelay == true) Tile(KnIcons.network, stringResource(R.string.route_relay), null, selected = true) { ui.statsOpen = !ui.statsOpen }
        Profile.entries.forEach { p ->
            if (p != ui.profile) Tile(KnIcons.screen, profileLabel(p), stringResource(R.string.session_switch_profile)) { ui.actions.setProfile(p) }
        }
        Tile(KnIcons.sound, stringResource(R.string.session_sound), audio?.let { (c, _) -> stringResource(R.string.session_sound_channels, c) }) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.remote, stringResource(R.string.session_remote_mouse), stringResource(if (ui.remoteMouse) R.string.session_on else R.string.session_off), selected = ui.remoteMouse) { ui.actions.setRemoteMouse(!ui.remoteMouse) }
        Tile(KnIcons.gamepad, stringResource(R.string.session_pad_mouse), stringResource(if (ui.padMouse) R.string.session_on else R.string.session_off), selected = ui.padMouse) { ui.actions.setPadMouse(!ui.padMouse) }
        // Le menu reste ouvert : on passe d'un écran à l'autre en voyant l'image derrière.
        screenChoice(ui)?.let { c -> Tile(KnIcons.screen, stringResource(R.string.session_computer_screen), "${c.index + 1} / ${c.count} · ${c.width} × ${c.height}") { ui.actions.pickDisplay(c.next) } }
        Tile(KnIcons.keyboard, stringResource(R.string.session_type_text), null) { ui.typing = true }
        Tile(KnIcons.keys, stringResource(R.string.session_keys), stringResource(R.string.keys_ctrl_alt_del_short)) { ui.keysOpen = true }
        // Sous-titre de la tuile (link-ui) : les demandes d'abord, puis les invités (« 1 demande · 2 invités » ; sans
        // pastille dans un menu à tuiles, c'est ce qui les montre à l'hôte) ; ni l'un ni l'autre : le code du partage
        // ouvert, sinon « Code et QR code ». Impossible : estompée, mais sélectionnable (OK ouvre le panneau réduit à la raison).
        val open = share
        val shareDetail = when {
            open == null -> stringResource(R.string.share_tile_detail)
            open.requests.isNotEmpty() || open.guests.isNotEmpty() ->
                listOfNotNull(
                    open.requests.takeIf { it.isNotEmpty() }?.let { requestsLabel(it.size) },
                    open.guests.takeIf { it.isNotEmpty() }?.let { guestsLabel(it.size) },
                ).joinToString(" · ")
            else -> open.codeText
        }
        Tile(KnIcons.share, stringResource(R.string.share_button), shareDetail, selected = share != null, dimmed = !ui.shareUsable) { ui.actions.share() }
        LendTile(ui)
        Tile(KnIcons.pulse, stringResource(R.string.session_stats), null) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.exit, stringResource(R.string.session_disconnect), null, danger = true) { ui.actions.quit() }
    }
}

@Composable
internal fun Tile(icon: androidx.compose.ui.graphics.vector.ImageVector, title: String, detail: String?, modifier: Modifier = Modifier, primary: Boolean = false, selected: Boolean = false, danger: Boolean = false, dimmed: Boolean = false, onClick: () -> Unit) {
    val s = LocalScale.current
    // Estompée, une tuile reste atteignable et sélectionnable à la télécommande.
    FocusCard(onClick, modifier.width(s.dp(160f)).height(s.dp(104f)), shape = RoundedCornerShape(16.dp)) { focused ->
        Column(
            Modifier.fillMaxSize().alpha(if (dimmed) 0.5f else 1f).background(if (focused && danger) Kn.danger else if (focused) Kn.neonCore else if (danger) Kn.danger.copy(alpha = 0.22f) else if (selected) Kn.spectre.copy(alpha = 0.22f) else Kn.panel.copy(alpha = 0.92f)).padding(s.dp(14f)),
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
        Shortcut.CTRL_ALT_DEL to stringResource(R.string.keys_ctrl_alt_del), Shortcut.ALT_TAB to "Alt + Tab", Shortcut.WIN to "⊞", Shortcut.WIN_D to "⊞ + D",
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
    val route by ui.state.route.collectAsState()
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
        // « Chemin : Direct », « Chemin : Par le relais », « Chemin : Par le relais (TCP) ».
        route?.let { Mono(stringResource(R.string.stats_path, routeLabel(it.transport)), size = 11f) }
        Mono(stringResource(R.string.session_fps, sec?.fps ?: 0) + " · " + fmt("%.1f Mbit/s", sec?.mbps ?: 0.0), size = 11f)
        Mono(stringResource(R.string.stats_losses, sec?.lostFrames ?: 0, sec?.holes ?: 0, sec?.lostPackets ?: 0), size = 11f)
        video?.let { (w, h) -> Mono("$w × $h · ${decoder ?: ""}", size = 11f) }
        audio?.let { (c, ms) -> Mono(stringResource(R.string.session_sound_detail, c, ms), size = 11f) }
        if (!ui.tv) GlassButton(stringResource(R.string.session_close), { ui.statsOpen = false })
    }
}

@Composable
private fun EndedCard(ui: SessionUi, e: Ended, modifier: Modifier) {
    if (ui.borrower) {
        BorrowEndedCard(ui, e, modifier)
        return
    }
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(modifier.padding(s.dp(20f)).panel(22.dp).padding(s.dp(24f)).widthIn(max = s.dp(620f)), verticalArrangement = Arrangement.spacedBy(s.dp(12f))) {
        // Avant la première image, c'est la connexion qui a échoué ; la raison, en une phrase.
        val failed = ui.state.firstImageAt.get() == 0L
        Title(stringResource(if (failed) R.string.session_failed else R.string.session_ended_title), size = 24f)
        // La fin d'un partage est une phrase du cœur, avec le nom de l'hôte (« iPhone de Léa a arrêté le partage. ») : telle quelle.
        val text = e.error ?: ""
        Body(if (text == ui.state.shareEndText) text else sentence(text), color = Kn.frost2, size = 15f)
        if (e.displayRestored == false || e.audioRestored == false) Body(stringResource(R.string.session_not_restored), color = Kn.warn, size = 13f)
        PrimaryButton(stringResource(R.string.session_back_home), { ui.actions.quit() }, Modifier.focusRequester(focus))
    }
}

/** Un message du cœur (« l'ordinateur ne répond plus ») en phrase : majuscule, point final. */
internal fun sentence(m: String): String {
    val t = m.trim()
    if (t.isEmpty()) return t
    val first = t.replaceFirstChar { it.uppercase() }
    return if (first.last() in ".!?…") first else "$first."
}
