package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.LendAvailable
import fr.leslignesclaires.kybernow.core.LendCodeView
import fr.leslignesclaires.kybernow.core.LendDuration
import fr.leslignesclaires.kybernow.core.LendRequest
import fr.leslignesclaires.kybernow.core.LendSteam
import fr.leslignesclaires.kybernow.core.LendView
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.QrCode
import fr.leslignesclaires.kybernow.ui.Title

/*
 * Le prêt de l'ordinateur, côté hôte, dans la session (docs/link-ui.md, « Prêt de l'ordinateur (KyberNow Share) ») :
 * « Prêter l'ordinateur… » dans la barre (et sa tuile au téléviseur), et son panneau « Prêter {nom} » : la durée, le
 * compte Steam, « Créer un code », puis le code, le lien et le QR code comme le partage, « Annuler le code » ; au-dessus,
 * les demandes d'amis (« Léa veut emprunter l'ordinateur. », « Accepter », « Refuser »).
 */

/** Pourquoi « Prêter l'ordinateur… » est impossible (Station trop ancienne, prêt coupé dans Admin) ; nul s'il est possible ou inconnu. */
@Composable
internal fun lendUnavailableText(a: LendAvailable?): String? = when (a) {
    LendAvailable.TOO_OLD -> stringResource(R.string.lend_too_old)
    LendAvailable.OFF -> stringResource(R.string.lend_off)
    LendAvailable.ON, null -> null
}

/**
 * « Prêter l'ordinateur… » dans la partie fixe de la barre, à côté de « Partager » ; les demandes d'amis ont leur pastille
 * (« 1 demande »), qui rouvre le panneau. Impossible, il est estompé mais reste sélectionnable : il ouvre le panneau
 * réduit à la raison.
 */
@Composable
internal fun LendItems(ui: SessionUi) {
    val lend by ui.state.lend.collectAsState()
    GlassButton(stringResource(R.string.lend_button), { ui.actions.lend() }, Modifier.alpha(if (ui.lendUsable) 1f else 0.5f), selected = lend?.code != null)
    lend?.requests?.takeIf { it.isNotEmpty() }?.let { GlassButton(requestsLabel(it.size), { ui.actions.lend() }, selected = true) }
}

/** La tuile « Prêter l'ordinateur… » du menu du téléviseur : les demandes, sinon le code de séance, sinon rien dessous. */
@Composable
internal fun LendTile(ui: SessionUi) {
    val lend by ui.state.lend.collectAsState()
    val view = lend
    val detail = when {
        view == null -> null
        view.requests.isNotEmpty() -> requestsLabel(view.requests.size)
        view.code != null -> view.code.codeText
        else -> null
    }
    Tile(KnIcons.code, stringResource(R.string.lend_button), detail, selected = view?.code != null || view?.requests?.isNotEmpty() == true, dimmed = !ui.lendUsable) { ui.actions.lend() }
}

@Composable
internal fun LendPanel(ui: SessionUi, modifier: Modifier) {
    val view by ui.state.lend.collectAsState()
    LendPanelContent(
        computer = ui.computerName,
        view = view,
        unavailable = if (ui.lendUsable) null else lendUnavailableText(ui.lendAvailable),
        busy = ui.lendBusy,
        error = ui.lendError,
        copy = ui.lendCopy,
        tv = ui.tv,
        duration = ui.lendDuration,
        steam = ui.lendSteam,
        warning = ui.lendWarning,
        actions = ui.actions,
        onDuration = { ui.lendDuration = it },
        onSteam = { ui.lendSteam = it },
        onClose = { ui.lendOpen = false },
        modifier = modifier,
    )
}

/**
 * Le panneau « Prêter {nom} » sans l'app (les essais le rendent tel quel). [unavailable] : la raison, seule dans le panneau.
 * [warning] : l'avertissement, montré tant qu'aucun code n'a été créé depuis cet appareil.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun LendPanelContent(
    computer: String,
    view: LendView?,
    unavailable: String?,
    busy: Boolean,
    error: String?,
    copy: ShareCopy?,
    tv: Boolean,
    duration: LendDuration,
    steam: LendSteam,
    warning: Boolean,
    actions: SessionActions,
    onDuration: (LendDuration) -> Unit,
    onSteam: (LendSteam) -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val s = LocalScale.current
    val code = view?.code
    val requests = view?.requests.orEmpty()
    // À la télécommande : une demande d'abord (son « Accepter »), sinon « Créer un code » ou « Copier le lien » (au
    // téléviseur, qui ne copie pas : « Annuler le code »), ou « Fermer » quand il n'y a que la raison.
    val first = remember { FocusRequester() }
    LaunchedEffect(unavailable != null, code?.code, requests.firstOrNull()?.id) { runCatching { first.requestFocus() } }
    BoxWithConstraints(modifier.padding(s.dp(20f))) {
        val wide = maxWidth >= 600.dp
        Column(
            Modifier.widthIn(max = s.dp(840f)).heightIn(max = maxHeight).opaquePanel().verticalScroll(rememberScrollState()).padding(s.dp(22f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(14f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
                Title(stringResource(R.string.lend_title, computer), Modifier.weight(1f), size = 22f)
                GlassButton(stringResource(R.string.session_close), onClose, if (unavailable != null) Modifier.focusRequester(first) else Modifier)
            }
            if (unavailable != null) {
                Body(unavailable, color = Kn.frost2, size = 15f)
                return@Column
            }
            requests.forEachIndexed { i, r -> LendRequestRow(r, busy, actions, if (i == 0) first else null) }
            val focusMain = if (requests.isEmpty()) first else null
            if (code == null) {
                LendChoices(duration, steam, busy, actions, onDuration, onSteam, focusMain)
            } else if (wide) {
                Row(horizontalArrangement = Arrangement.spacedBy(s.dp(28f))) {
                    LendCode(code, copy, tv, actions, focusMain)
                    LendCodeHelp(busy, actions, Modifier.weight(1f), if (tv) focusMain else null)
                }
            } else {
                LendCode(code, copy, tv, actions, focusMain)
                LendCodeHelp(busy, actions, Modifier.fillMaxWidth(), if (tv) focusMain else null)
            }
            if (warning) Body(stringResource(R.string.lend_warning), color = Kn.warn, size = 13f)
            error?.let { Body(it, color = Kn.warn, size = 14f) }
        }
    }
}

/** « Durée » et ses quatre choix, « Compte Steam » et ses deux choix, « Créer un code ». */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LendChoices(duration: LendDuration, steam: LendSteam, busy: Boolean, actions: SessionActions, onDuration: (LendDuration) -> Unit, onSteam: (LendSteam) -> Unit, focus: FocusRequester?) {
    val s = LocalScale.current
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        Eyebrow(stringResource(R.string.lend_duration))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            LendDuration.entries.forEach { d -> GlassButton(durationLabel(d), { onDuration(d) }, selected = d == duration) }
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        Eyebrow(stringResource(R.string.lend_steam))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            LendSteam.entries.forEach { st -> GlassButton(steamLabel(st), { onSteam(st) }, selected = st == steam) }
        }
    }
    PrimaryButton(stringResource(R.string.lend_create), { actions.lendCreateCode() }, if (focus != null) Modifier.focusRequester(focus) else Modifier, enabled = !busy)
}

/** « 1 heure », « 2 heures », « 4 heures », « Jusqu'à ce que je reprenne ». */
@Composable
internal fun durationLabel(d: LendDuration) = stringResource(
    when (d) {
        LendDuration.ONE_HOUR -> R.string.lend_duration_1h
        LendDuration.TWO_HOURS -> R.string.lend_duration_2h
        LendDuration.FOUR_HOURS -> R.string.lend_duration_4h
        LendDuration.OPEN -> R.string.lend_duration_open
    },
)

/** « Son compte Steam », « Votre compte Steam ». */
@Composable
internal fun steamLabel(st: LendSteam) = stringResource(if (st == LendSteam.FRIEND) R.string.lend_steam_friend else R.string.lend_steam_host)

/** « Code » et le code en grand, le QR code du lien, « Copier le lien » (pas au téléviseur) et son résultat, comme le partage. */
@Composable
private fun LendCode(code: LendCodeView, copy: ShareCopy?, tv: Boolean, actions: SessionActions, focus: FocusRequester?) {
    val s = LocalScale.current
    Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        Eyebrow(stringResource(R.string.share_code_label))
        Mono(code.codeText, size = if (tv) 52f else 44f, color = Kn.frost, weight = 500)
        QrCode(code.link, size = if (tv) 150f else 140f, description = stringResource(R.string.lend_qr_label))
        if (!tv) GlassButton(stringResource(R.string.share_copy_link), { actions.lendCopyLink() }, if (focus != null) Modifier.focusRequester(focus) else Modifier, selected = copy == ShareCopy.COPIED)
        when (copy) {
            ShareCopy.COPIED -> Body(stringResource(R.string.share_link_copied), size = 13f, color = Kn.frost2)
            ShareCopy.FAILED -> Column(Modifier.widthIn(max = s.dp(260f)), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
                Body(stringResource(R.string.share_copy_failed), size = 13f, color = Kn.warn)
                SelectionContainer { Mono(code.web.ifBlank { code.link }, size = 11f, color = Kn.frost) }
            }
            null -> {}
        }
        Body(stringResource(R.string.lend_code_valid), size = 13f, color = Kn.muted)
    }
}

/** L'aide, « Votre session se fermera quand votre ami arrivera. » (le code créé depuis la session vaut accord), « Annuler le code ». */
@Composable
private fun LendCodeHelp(busy: Boolean, actions: SessionActions, modifier: Modifier, focus: FocusRequester?) {
    val s = LocalScale.current
    Column(modifier, verticalArrangement = Arrangement.spacedBy(s.dp(12f))) {
        Body(stringResource(R.string.lend_help), color = Kn.frost2, size = 14f)
        Body(stringResource(R.string.lend_session_closes), color = Kn.frost, size = 14f, weight = 600)
        GlassButton(stringResource(R.string.lend_cancel_code), { actions.lendCancelCode() }, if (focus != null) Modifier.focusRequester(focus) else Modifier, enabled = !busy)
    }
}

/**
 * Une demande d'emprunt : « Léa veut emprunter l'ordinateur. », dessous son app (et « hors de chez vous » par le relais),
 * « Votre session va se fermer. » (accepter depuis sa session la ferme), « Accepter » et « Refuser ».
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun LendRequestRow(request: LendRequest, busy: Boolean, actions: SessionActions, focus: FocusRequester?) {
    val s = LocalScale.current
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(Kn.spectre.copy(alpha = 0.10f)).padding(s.dp(12f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(8f)),
    ) {
        Column(verticalArrangement = Arrangement.spacedBy(s.dp(2f))) {
            // Le créneau de l'ami : il passe avant l'hôte (« Créneau de Léa : l'ordinateur lui est prêté dans 2 min. »).
            Body(stringResource(if (request.slot) R.string.lend_slot_start else R.string.lend_request_wants, request.name), size = 15f, weight = 600, color = Kn.frost)
            val away = if (request.away) stringResource(R.string.share_away) else null
            val detail = listOfNotNull(appLabel(request.app), away).joinToString(" · ")
            if (detail.isNotEmpty()) Mono(detail, size = 11f, color = Kn.muted)
            Body(stringResource(R.string.lend_accept_closes), size = 13f, color = Kn.frost2)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            GlassButton(stringResource(R.string.share_accept), { actions.lendAnswer(request, true) }, if (focus != null) Modifier.focusRequester(focus) else Modifier, enabled = !busy, selected = true)
            if (!request.slot) GlassButton(stringResource(R.string.share_refuse), { actions.lendAnswer(request, false) }, enabled = !busy)
        }
    }
}
