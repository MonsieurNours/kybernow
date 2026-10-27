package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.ShareGuest
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
 * téléviseur), les invités (leur rôle, leur app, depuis quand ; « Faire jouer », « Faire regarder »,
 * « Donner le clavier et la souris », « Reprendre le clavier et la souris », « Retirer »), « Nouveau
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
    LaunchedEffect(share != null, unavailable != null) { if (share != null || unavailable != null) runCatching { first.requestFocus() } }
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
                    ShareCode(share, copy, tv, actions, copyFocus)
                    ShareGuests(share, busy, actions, Modifier.weight(1f))
                }
                else -> {
                    ShareCode(share, copy, tv, actions, copyFocus)
                    ShareGuests(share, busy, actions, Modifier.fillMaxWidth())
                }
            }
            error?.let { Body(it, color = Kn.warn, size = 14f) }
        }
    }
}

/**
 * Le panneau est plein : il porte le code et le QR code, qui doivent rester lisibles par-dessus la barre ou
 * le menu (le verre des autres feuilles les laisserait transparaître).
 */
private fun Modifier.opaquePanel(): Modifier = this
    .clip(RoundedCornerShape(22.dp))
    .background(Kn.panel)
    .border(BorderStroke(1.dp, Kn.line), RoundedCornerShape(22.dp))

/**
 * « Code » et le code en grand, le QR code du lien des apps, « Copier le lien » (l'adresse du client
 * web : le lien des apps ne s'ouvre pas dans un navigateur ; pas au téléviseur) et son résultat.
 */
@Composable
private fun ShareCode(share: ShareState, copy: ShareCopy?, tv: Boolean, actions: SessionActions, copyModifier: Modifier) {
    val s = LocalScale.current
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Eyebrow(stringResource(R.string.share_code_label))
        Mono(share.codeText, size = if (tv) 52f else 44f, color = Kn.frost, weight = 500)
        QrCode(share.link, size = if (tv) 150f else 140f, description = stringResource(R.string.share_qr_label))
        if (!tv) GlassButton(stringResource(R.string.share_copy_link), { actions.shareCopyLink() }, copyModifier, selected = copy == ShareCopy.COPIED)
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
private fun ShareGuests(share: ShareState, busy: Boolean, actions: SessionActions, modifier: Modifier) {
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
        Body(stringResource(R.string.share_help), color = Kn.frost2, size = 14f)
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

/** « Léa · Spectateur », son app et « depuis 3 min », et ce que l'hôte peut en faire. */
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
            val detail = listOfNotNull(appLabel(guest.app), sinceLabel(guest.since, now)).joinToString(" · ")
            if (detail.isNotEmpty()) Mono(detail, size = 11f, color = Kn.muted)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            // Spectateur : « Faire jouer ». Joueur : « Faire regarder » à la place (il redevient spectateur), puis
            // « Donner le clavier et la souris » ou « Reprendre le clavier et la souris » (docs/link-ui.md, « Partage »).
            if (guest.role.plays) GlassButton(stringResource(R.string.share_watch_only), { actions.shareSetRole(guest, ShareRole.SPECTATOR) }, enabled = !busy)
            else GlassButton(stringResource(R.string.share_let_play), { actions.shareSetRole(guest, ShareRole.PLAYER) }, enabled = !busy)
            when (guest.role) {
                ShareRole.PLAYER -> GlassButton(stringResource(R.string.share_give_keyboard_mouse), { actions.shareSetRole(guest, ShareRole.PLAYER_KBM) }, enabled = !busy)
                ShareRole.PLAYER_KBM -> GlassButton(stringResource(R.string.share_take_keyboard_mouse), { actions.shareSetRole(guest, ShareRole.PLAYER) }, enabled = !busy)
                ShareRole.SPECTATOR -> {}
            }
            GlassButton(stringResource(R.string.share_remove), { actions.shareRemove(guest) }, enabled = !busy)
        }
    }
}

/** « Léa · Spectateur », « Léa · Joueur », « Léa · Joueur, clavier et souris ». */
@Composable
private fun guestLabel(g: ShareGuest): String = stringResource(
    when (g.role) {
        ShareRole.SPECTATOR -> R.string.share_guest_spectator
        ShareRole.PLAYER -> R.string.share_guest_player
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
