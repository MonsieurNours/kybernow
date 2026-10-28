package fr.leslignesclaires.kybernow.session

import android.os.SystemClock
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.RelayPhase
import fr.leslignesclaires.kybernow.core.RelayWait
import fr.leslignesclaires.kybernow.core.ShareGuest
import fr.leslignesclaires.kybernow.core.ShareRelay
import fr.leslignesclaires.kybernow.core.ShareRole
import fr.leslignesclaires.kybernow.core.ShareState
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.LocalMotion
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.QrCode
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.home.duration
import kotlinx.coroutines.delay

/**
 * Le panneau du partage de l'hôte (docs/link-ui.md, « Partage ») : « Partager {nom} », le code à six
 * chiffres, le QR code du lien des apps et « Copier le lien » (l'adresse du client web ; pas au
 * téléviseur), les invités (leurs droits, leur app, depuis quand ; « Donner les manettes » ou « Reprendre les
 * manettes », « Donner le clavier et la souris » ou « Reprendre le clavier et la souris », « Retirer »), « Nouveau
 * code » et « Arrêter le partage ». Le partage reste ouvert quand le panneau se ferme : la pastille de
 * la barre le rouvre. « Partager » impossible (Station trop ancienne, partage coupé) : le panneau se réduit à la
 * raison, écrite par le cœur.
 */
@Composable
internal fun SharePanel(ui: SessionUi, modifier: Modifier) {
    val share by ui.state.share.collectAsState()
    val unavailable = if (ui.shareUsable) null else ui.shareUnavailable
    // Le partage s'est arrêté ailleurs (Admin, la Station) pendant que le panneau était ouvert : il n'y a plus rien à y montrer.
    LaunchedEffect(share == null, ui.shareBusy, ui.shareError, unavailable) {
        if (share == null && !ui.shareBusy && ui.shareError == null && unavailable == null) ui.shareOpen = false
    }
    SharePanelContent(
        computer = ui.computerName,
        share = share,
        busy = ui.shareBusy,
        error = ui.shareError,
        copy = ui.shareCopy,
        tv = ui.tv,
        actions = ui.actions,
        onClose = { ui.shareOpen = false },
        modifier = modifier,
        unavailable = unavailable,
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun SharePanelContent(
    computer: String,
    share: ShareState?,
    busy: Boolean,
    error: String?,
    copy: ShareCopy?,
    tv: Boolean,
    actions: SessionActions,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    /** « Partager » est impossible : la raison, seule dans le panneau (le texte du cœur). */
    unavailable: String? = null,
) {
    val s = LocalScale.current
    // À la télécommande et à la manette, le focus arrive sur « Copier le lien » (au téléviseur, qui ne le
    // propose pas, ou quand il n'y a que la raison : sur « Fermer ») dès que le partage est ouvert.
    val first = remember { FocusRequester() }
    // Une demande attend l'hôte : le focus va à son « Accepter » (tout le monde, au téléviseur aussi).
    val firstRequest = remember { FocusRequester() }
    val requested = share?.requests?.isNotEmpty() == true
    val relay = share?.let { relayPhase(it) }
    // « Copier le lien » inactif (lien du relais en préparation) ne prend pas le focus : il le reçoit quand il s'active.
    LaunchedEffect(share != null, unavailable != null, requested, relay == RelayPhase.WAITING) {
        if (requested) runCatching { firstRequest.requestFocus() }
        else if (share != null || unavailable != null) runCatching { first.requestFocus() }
    }
    BoxWithConstraints(modifier.padding(s.dp(20f))) {
        val wide = maxWidth >= 600.dp
        val copyFocus = if (tv) Modifier else Modifier.focusRequester(first)
        Column(
            Modifier.widthIn(max = s.dp(840f)).heightIn(max = maxHeight).opaquePanel().verticalScroll(rememberScrollState()).padding(s.dp(22f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(14f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
                Title(stringResource(R.string.share_title, computer), Modifier.weight(1f), size = 22f)
                GlassButton(stringResource(R.string.session_close), onClose, if (tv || unavailable != null) Modifier.focusRequester(first) else Modifier)
            }
            when {
                unavailable != null -> Body(unavailable, color = Kn.frost2, size = 15f)
                share == null -> if (busy) androidx.compose.material3.CircularProgressIndicator(Modifier.size(s.dp(28f)), color = Kn.spectre, strokeWidth = s.dp(3f))
                wide -> Row(horizontalArrangement = Arrangement.spacedBy(s.dp(28f))) {
                    ShareCode(share, relay, copy, tv, actions, copyFocus)
                    ShareGuests(share, relay, busy, actions, firstRequest, Modifier.weight(1f))
                }
                else -> {
                    ShareCode(share, relay, copy, tv, actions, copyFocus)
                    ShareGuests(share, relay, busy, actions, firstRequest, Modifier.fillMaxWidth())
                }
            }
            error?.let { Body(it, color = Kn.warn, size = 14f) }
        }
    }
}

/**
 * Où en est le lien du relais de [share] ([RelayPhase]) : `connecting` vaut « en préparation » 10 s au plus, comptées
 * de la première fois que l'app le voit pour ce code ([RelayWait.shared]), puis « injoignable ». Le panneau se
 * recompose de lui-même à la fin des 10 s, sans attendre un autre état de la Station.
 */
@Composable
private fun relayPhase(share: ShareState): RelayPhase? {
    val connecting = share.relay == ShareRelay.CONNECTING
    val waited = RelayWait.shared.waited(share, SystemClock.elapsedRealtime())
    var expired by remember(share.code, connecting) { mutableStateOf(false) }
    LaunchedEffect(share.code, connecting) {
        if (!connecting) return@LaunchedEffect
        val left = RelayPhase.WAIT_MS - RelayWait.shared.waited(share, SystemClock.elapsedRealtime())
        if (left > 0) delay(left)
        expired = true
    }
    return RelayPhase.of(share.relay, if (expired) RelayPhase.WAIT_MS else waited)
}

/**
 * Le panneau est plein : il porte le code et le QR code, qui doivent rester lisibles par-dessus la barre ou
 * le menu (le verre des autres feuilles les laisserait transparaître).
 */
internal fun Modifier.opaquePanel(): Modifier = this
    .clip(RoundedCornerShape(22.dp))
    .background(Kn.panel)
    .border(BorderStroke(1.dp, Kn.line), RoundedCornerShape(22.dp))

/**
 * « Code » et le code en grand, le QR code du lien des apps, « Copier le lien » (l'adresse du client
 * web : le lien des apps ne s'ouvre pas dans un navigateur ; pas au téléviseur) et son résultat.
 */
@Composable
private fun ShareCode(share: ShareState, relay: RelayPhase?, copy: ShareCopy?, tv: Boolean, actions: SessionActions, copyModifier: Modifier) {
    val s = LocalScale.current
    // Lien du relais en préparation : le QR code et « Copier le lien » l'attendent (le lien du réseau serait copié par erreur).
    val waiting = relay == RelayPhase.WAITING
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Eyebrow(stringResource(R.string.share_code_label))
        Mono(share.codeText, size = if (tv) 52f else 44f, color = Kn.frost, weight = 500)
        val qrSize = if (tv) 150f else 140f
        if (waiting) {
            val preparing = stringResource(R.string.share_relay_preparing)
            Box(Modifier.size(s.dp(qrSize)).semantics { contentDescription = preparing }, contentAlignment = Alignment.Center) {
                androidx.compose.material3.CircularProgressIndicator(Modifier.size(s.dp(28f)), color = Kn.spectre, strokeWidth = s.dp(3f))
            }
        } else {
            QrCode(share.link, size = qrSize, description = stringResource(R.string.share_qr_label))
        }
        if (!tv) GlassButton(stringResource(R.string.share_copy_link), { actions.shareCopyLink() }, copyModifier, enabled = !waiting, selected = copy == ShareCopy.COPIED)
        when (copy) {
            ShareCopy.COPIED -> Body(stringResource(R.string.share_link_copied), size = 13f, color = Kn.frost2)
            // Android refuse (presse-papiers indisponible) : l'adresse s'affiche, à sélectionner à la main.
            ShareCopy.FAILED -> Column(Modifier.widthIn(max = s.dp(260f)), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
                Body(stringResource(R.string.share_copy_failed), size = 13f, color = Kn.warn)
                SelectionContainer { Mono(share.web.ifBlank { share.link }, size = 11f, color = Kn.frost) }
            }
            null -> {}
        }
    }
}

/** L'aide, « Invités » et la liste, « Nouveau code », « Arrêter le partage ». */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ShareGuests(share: ShareState, relay: RelayPhase?, busy: Boolean, actions: SessionActions, firstRequest: FocusRequester, modifier: Modifier) {
    val s = LocalScale.current
    // Les durées (« depuis 3 min ») avancent d'elles-mêmes ; sans animations (captures), une seule fois.
    val motion = LocalMotion.current
    val now by produceState(System.currentTimeMillis() / 1000, motion) {
        while (motion) {
            delay(15_000)
            value = System.currentTimeMillis() / 1000
        }
    }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(s.dp(12f))) {
        // L'aide dit où le lien vaut : sur le même réseau, ou ailleurs quand le relais est joint (et alors, chaque invité
        // venu d'ailleurs s'accepte) ; le lien du relais en préparation (10 s au plus), on le dit avec un indicateur ; le
        // relais injoignable (ou trop long à joindre), le lien ne vaut pas encore hors de chez soi.
        Body(stringResource(if (relay == RelayPhase.JOINED) R.string.share_help_relay else R.string.share_help), color = Kn.frost2, size = 14f)
        when (relay) {
            RelayPhase.WAITING -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                androidx.compose.material3.CircularProgressIndicator(Modifier.size(s.dp(14f)), color = Kn.spectre, strokeWidth = s.dp(2f))
                Body(stringResource(R.string.share_relay_preparing), color = Kn.muted, size = 13f)
            }
            RelayPhase.UNREACHABLE -> Body(stringResource(R.string.share_relay_unreachable), color = Kn.muted, size = 13f)
            else -> {}
        }
        // Les demandes venues par le relais, au-dessus des invités, seulement s'il y en a.
        if (share.requests.isNotEmpty()) {
            Eyebrow(stringResource(R.string.share_requests_title))
            share.requests.forEachIndexed { i, r -> RequestRow(r, full = share.full, busy = busy, actions = actions, focus = if (i == 0) firstRequest else null) }
        }
        Eyebrow(stringResource(R.string.share_guests_title))
        if (share.guests.isEmpty()) {
            Body(stringResource(R.string.share_nobody), color = Kn.muted, size = 15f)
        } else {
            share.guests.forEach { GuestRow(it, now, busy, actions) }
        }
        if (share.full) Body(stringResource(R.string.share_limit), color = Kn.muted, size = 13f)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            GlassButton(stringResource(R.string.share_new_code), { actions.shareNewCode() }, enabled = !busy)
            GlassButton(stringResource(R.string.share_stop), { actions.shareStop() }, enabled = !busy)
        }
    }
}

/**
 * La demande d'un invité venu par le relais (docs/relais.md) : « Léa demande à rejoindre », dessous son app et
 * « hors de chez vous », et « Accepter » / « Refuser ». Elle ne prend pas de place tant qu'elle n'est pas acceptée :
 * « Accepter » est inactif quand les quatre places sont prises (« Quatre invités au plus. » est dit plus bas).
 * [focus] : le focus du premier « Accepter » (télécommande, manette).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun RequestRow(request: ShareGuest, full: Boolean, busy: Boolean, actions: SessionActions, focus: FocusRequester?) {
    val s = LocalScale.current
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Kn.spectre.copy(alpha = 0.10f)).padding(s.dp(12f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(8f)),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(s.dp(2f))) {
            Body(stringResource(R.string.share_request_wants, request.name), size = 15f, weight = 600, color = Kn.frost)
            // « {app} · hors de chez vous » (une demande ne vient que du relais).
            Mono(listOfNotNull(appLabel(request.app), stringResource(R.string.share_away)).joinToString(" · "), size = 11f, color = Kn.muted)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            GlassButton(stringResource(R.string.share_accept), { actions.shareAccept(request) }, if (focus != null) Modifier.focusRequester(focus) else Modifier, enabled = !busy && !full, selected = true)
            GlassButton(stringResource(R.string.share_refuse), { actions.shareRefuse(request) }, enabled = !busy)
        }
    }
}

/**
 * « Léa · Spectateur », son app, sa qualité d'image (« 1080p · 60 Hz », en lecture seule, seulement si ce n'est pas
 * celle de l'hôte et si le cœur sait l'écrire) et « depuis 3 min », et ce que l'hôte peut en faire (ses deux droits,
 * à part).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun GuestRow(guest: ShareGuest, now: Long, busy: Boolean, actions: SessionActions) {
    val s = LocalScale.current
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Kn.frost.copy(alpha = 0.05f)).padding(s.dp(12f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(8f)),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(s.dp(2f))) {
            Body(guestLabel(guest), size = 15f, weight = 600, color = Kn.frost)
            // Son app, « hors de chez vous » s'il est venu par le relais, sa qualité d'image si elle n'est pas celle de
            // l'hôte, puis depuis quand (docs/link-ui.md, « Un invité »).
            val away = if (guest.away) stringResource(R.string.share_away) else null
            val detail = listOfNotNull(appLabel(guest.app), away, guest.ownImageLabel, sinceLabel(guest.since, now)).joinToString(" · ")
            if (detail.isNotEmpty()) Mono(detail, size = 11f, color = Kn.muted)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            // Deux droits indépendants, chacun donné ou repris sans toucher à l'autre : les manettes, puis le clavier et
            // la souris (docs/link-ui.md, « Partage »). Reprendre le dernier droit ramène l'invité spectateur.
            if (guest.role.pads) GlassButton(stringResource(R.string.share_take_pads), { actions.shareSetPads(guest, false) }, enabled = !busy)
            else GlassButton(stringResource(R.string.share_give_pads), { actions.shareSetPads(guest, true) }, enabled = !busy)
            if (guest.role.keyboardMouse) GlassButton(stringResource(R.string.share_take_keyboard_mouse), { actions.shareSetKeyboardMouse(guest, false) }, enabled = !busy)
            else GlassButton(stringResource(R.string.share_give_keyboard_mouse), { actions.shareSetKeyboardMouse(guest, true) }, enabled = !busy)
            GlassButton(stringResource(R.string.share_remove), { actions.shareRemove(guest) }, enabled = !busy)
        }
    }
}

/** « Léa · Spectateur », « Léa · Joueur », « Léa · Clavier et souris », « Léa · Joueur, clavier et souris ». */
@Composable
private fun guestLabel(g: ShareGuest): String = stringResource(
    when (g.role) {
        ShareRole.SPECTATOR -> R.string.share_guest_spectator
        ShareRole.PLAYER -> R.string.share_guest_player
        ShareRole.KBM -> R.string.share_guest_kbm
        ShareRole.PLAYER_KBM -> R.string.share_guest_player_kbm
    },
    g.name,
)

/** L'app de l'invité (`mac`, `windows`, `linux`, `ios`, `tvos`, `android`, `webos`, `web`) ; nul si la Station en annonce une autre. */
@Composable
private fun appLabel(app: String): String? = when (app) {
    "mac" -> stringResource(R.string.share_app_mac)
    "windows" -> stringResource(R.string.share_app_windows)
    "linux" -> stringResource(R.string.share_app_linux)
    "ios" -> stringResource(R.string.share_app_ios)
    "tvos" -> stringResource(R.string.share_app_tvos)
    "android" -> stringResource(R.string.share_app_android)
    "webos" -> stringResource(R.string.share_app_webos)
    "web" -> stringResource(R.string.share_app_web)
    else -> null
}

/**
 * « depuis 3 min », « depuis 1 h 12 » ; moins d'une minute : « depuis moins d'une minute ». Compté
 * d'après l'horloge de la Station ([ShareGuest.since]) : jamais négatif, nul si elle ne la donne pas.
 */
@Composable
private fun sinceLabel(since: Long?, now: Long): String? {
    val seconds = since?.let { (now - it).coerceAtLeast(0) } ?: return null
    return stringResource(R.string.share_since, if (seconds < 60) stringResource(R.string.share_duration_short) else duration(seconds))
}
