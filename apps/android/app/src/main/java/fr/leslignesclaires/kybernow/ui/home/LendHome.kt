package fr.leslignesclaires.kybernow.ui.home

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
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.core.LendAvailability
import fr.leslignesclaires.kybernow.core.LendSlot
import fr.leslignesclaires.kybernow.core.LendView
import fr.leslignesclaires.kybernow.model.BorrowedComputer
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.BorrowStep
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FitTitle
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.FriendStatus
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalMotion
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import kotlinx.coroutines.delay

/*
 * Le prêt de l'ordinateur sur l'accueil (docs/link-ui.md, « Prêt de l'ordinateur (KyberNow Share) ») : chez l'ami, la
 * carte « Ordinateur de {hôte} » (son état, son créneau, « Emprunter » ou « Demander à {hôte} », l'attente de l'accord) ;
 * chez l'hôte, sur l'accueil de l'ordinateur prêté, « Prêté à {prénom} » et « Reprendre l'ordinateur ».
 */

/** La langue de l'app est le français (les heures : « 18 h » ; sinon « 6 PM »). */
@Composable
private fun french(): Boolean = LocalConfiguration.current.locales[0].language == "fr"

/** « Disponible », « Occupé », « Prêté à quelqu'un d'autre », « Verrouillé », « Prêt coupé ». */
@Composable
fun availabilityLabel(a: LendAvailability) = stringResource(
    when (a) {
        LendAvailability.FREE -> R.string.lend_state_free
        LendAvailability.BUSY -> R.string.lend_state_busy
        LendAvailability.LENT -> R.string.lend_state_lent
        LendAvailability.LOCKED -> R.string.lend_state_locked
        LendAvailability.OFF -> R.string.lend_state_off
    },
)

/**
 * L'état d'une carte d'ami : celui de la Station, « Hors ligne » si elle ne répond pas, « Recherche… » tant qu'on ne sait
 * pas ; nul sans mot de passe connu (rien à demander). Le prêt en cours est celui de l'ami : « Disponible » (il revient).
 */
@Composable
fun friendStatusLabel(st: FriendStatus?, known: Boolean): String? = when {
    st?.status != null -> availabilityLabel(if (st.status.state == LendAvailability.LENT && st.status.mine) LendAvailability.FREE else st.status.state)
    st?.offline == true -> stringResource(R.string.home_offline)
    st?.refused != null -> null
    known -> stringResource(R.string.home_searching)
    else -> null
}

/** « Votre créneau : mercredi, 14 h – 18 h » ou, pendant le créneau, « Votre créneau, jusqu'à 18 h » ; nul sans créneau. */
@Composable
fun slotLabel(slot: LendSlot?): String? {
    val next = slot?.next ?: return null
    val fr = french()
    return if (slot.now) stringResource(R.string.lend_slot_now, Lend.hour(next.end, fr))
    else stringResource(R.string.lend_slot_next, Lend.day(next.start, fr), Lend.hour(next.start, fr), Lend.hour(next.end, fr))
}

/**
 * La carte « Ordinateur de {hôte} » en grand (l'ordinateur prêté choisi sur l'accueil) : son état, le nom de l'ordinateur,
 * le créneau, le refus du dernier essai ; « Emprunter » (libre, ou le prêt en cours est le sien), « Demander à {hôte} »
 * (occupé), inactif sinon ; sans mot de passe retenu, « Mot de passe » au-dessus. Pendant l'emprunt : « Préparation de
 * l'ordinateur », puis « En attente de {hôte}… » et « Annuler ». « Retirer cet ordinateur » demande confirmation.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun FriendHero(
    computer: BorrowedComputer,
    status: FriendStatus?,
    knownPassword: Boolean,
    step: BorrowStep?,
    error: String?,
    nameSize: Float,
    focus: FocusRequester?,
    onBorrow: (String?) -> Unit,
    onCancel: () -> Unit,
    onRemove: () -> Unit,
    stacked: Boolean = false,
) {
    val s = LocalScale.current
    var password by remember(computer.id) { mutableStateOf("") }
    var confirmRemove by remember(computer.id) { mutableStateOf(false) }
    val lendStatus = status?.status
    val host = lendStatus?.host?.ifBlank { null } ?: computer.host
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(if (s.tv) 7f else 10f))) {
        friendStatusLabel(status, knownPassword)?.let { label ->
            val on = lendStatus?.borrowable == true
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(if (on) Kn.spectre else Kn.muted))
                Spacer(Modifier.width(s.dp(10f)))
                Eyebrow(label, color = Kn.frost2)
            }
        }
        FitTitle(stringResource(R.string.lend_card_title, host), Modifier.fillMaxWidth(), size = nameSize, min = nameSize * 0.45f)
        (lendStatus?.computer?.ifBlank { null } ?: computer.computer).takeIf { it.isNotBlank() }?.let { Body(it, size = 16f, color = Kn.frost2, maxLines = 1) }
        slotLabel(lendStatus?.slot)?.let { Body(it, size = 14f, color = Kn.frost2) }
        (error ?: status?.refused)?.let { Body(it, color = Kn.warn, size = 13f) }
        Spacer(Modifier.height(s.dp(if (s.tv) 2f else 6f)))
        val first = Modifier.then(if (focus != null) Modifier.focusRequester(focus) else Modifier)
        val wide = if (stacked) Modifier.widthIn(max = s.dp(560f)).fillMaxWidth() else Modifier
        when (step) {
            is BorrowStep.Waiting -> {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
                    CircularProgressIndicator(Modifier.size(s.dp(22f)), color = Kn.spectre, strokeWidth = s.dp(3f))
                    Body(stringResource(R.string.lend_waiting, step.host.ifBlank { host }), size = 17f, weight = 600)
                }
                GlassButton(stringResource(R.string.cancel), onCancel, first)
            }
            // Sans effet, mais focalisable : au téléviseur, le focus ne se perd pas pendant la préparation.
            BorrowStep.Preparing -> PrimaryButton(stringResource(R.string.lend_preparing), {}, first.then(wide))
            null -> {
                // Mot de passe pas retenu : on le dit avant le champ (un refus, lui, s'écrit plus haut).
                if (!knownPassword && status?.refused == null && error == null) Body(stringResource(R.string.lend_password_needed), size = 14f, color = Kn.frost2)
                if (!knownPassword || status?.refused != null) {
                    OutlinedTextField(
                        value = password,
                        onValueChange = { password = it },
                        label = { Text(stringResource(R.string.add_password)) },
                        singleLine = true,
                        modifier = Modifier.widthIn(max = s.dp(420f)).fillMaxWidth(),
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                        colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, focusedLabelColor = Kn.spectre, cursorColor = Kn.spectre),
                    )
                }
                val needs = !knownPassword || status?.refused != null
                val busy = lendStatus?.state == LendAvailability.BUSY && !lendStatus.mine
                // Inconnu (pas encore de réponse, ou pas de mot de passe) : « Emprunter » reste possible, la Station tranchera.
                val possible = lendStatus == null || lendStatus.borrowable
                val label = if (busy) stringResource(R.string.lend_ask, host) else stringResource(R.string.lend_borrow)
                FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
                    PrimaryButton(label, { onBorrow(password.takeIf { needs }) }, first.then(wide), enabled = possible && status?.offline != true && (!needs || password.isNotEmpty()))
                    if (!confirmRemove) GlassButton(stringResource(R.string.station_remove), { confirmRemove = true })
                }
                if (confirmRemove) {
                    // « Retirer l'ordinateur de Léa de cet appareil ? », « … d'Hugo … ».
                    Body(stringResource(if (elides(host)) R.string.lend_remove_confirm_elided else R.string.lend_remove_confirm, host), size = 14f, color = Kn.frost2)
                    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                        GlassButton(stringResource(R.string.station_remove_yes), onRemove)
                        GlassButton(stringResource(R.string.cancel), { confirmRemove = false })
                    }
                }
            }
        }
    }
}

/** Dans la rangée des ordinateurs : la carte d'un ordinateur prêté (« Ordinateur de {hôte} », son état). */
@Composable
fun FriendCard(computer: BorrowedComputer, status: FriendStatus?, knownPassword: Boolean, selected: Boolean, card: Float, onClick: () -> Unit) {
    val s = LocalScale.current
    Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        FocusCard(onClick) {
            Box(
                Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(Kn.panel)
                    .border(BorderStroke(if (selected) 2.dp else 1.dp, if (selected) Kn.frost.copy(alpha = 0.6f) else Kn.spectre.copy(alpha = 0.5f)), RoundedCornerShape(16.dp)),
                contentAlignment = Alignment.Center,
            ) { KnIcon(KnIcons.share, tint = Kn.spectre, size = 26f) }
        }
        Column {
            Body(stringResource(R.string.lend_card_title, status?.status?.host?.ifBlank { null } ?: computer.host), size = 15f, weight = if (selected) 600 else 400, maxLines = 1)
            friendStatusLabel(status, knownPassword)?.let { Mono(it, size = 10f, color = if (status?.status?.borrowable == true) Kn.frost2 else Kn.muted, maxLines = 1) }
        }
    }
}

/**
 * Chez l'hôte, sur l'accueil de l'ordinateur prêté : « Prêté à Léa » (« Prêté à Léa jusqu'à 18 h » avec une fin), et
 * pendant le préavis « Reprise dans 1:42 » (le compte à rebours avance chaque seconde).
 */
@Composable
fun LentLine(view: LendView) {
    val b = view.borrower ?: return
    val fr = french()
    val until = b.until
    Body(
        if (until != null) stringResource(R.string.lend_lent_to_until, b.name, Lend.hour(until, fr)) else stringResource(R.string.lend_lent_to, b.name),
        size = 16f, weight = 600, color = Kn.frost,
    )
    val notice = view.notice
    if (view.takingBack && notice != null) {
        val motion = LocalMotion.current
        val now by produceState(System.currentTimeMillis() / 1000, motion, notice.at) {
            while (motion) {
                delay(1000)
                value = System.currentTimeMillis() / 1000
            }
        }
        Mono(stringResource(R.string.lend_taking_back, Lend.countdown(Lend.left(notice.at, now))), size = 14f, color = Kn.spectre)
    }
}

/** « Reprendre l'ordinateur » ; pendant le préavis, « Reprendre tout de suite ». */
@Composable
fun TakeBackButton(view: LendView, onTakeBack: (Boolean) -> Unit, modifier: Modifier) {
    if (view.takingBack) PrimaryButton(stringResource(R.string.lend_take_back_now), { onTakeBack(true) }, modifier)
    else PrimaryButton(stringResource(R.string.lend_take_back), { onTakeBack(false) }, modifier)
}
