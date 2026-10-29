package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
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
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalMotion
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.panel
import fr.leslignesclaires.kybernow.ui.home.duration
import kotlinx.coroutines.delay

/*
 * La session d'emprunt, côté ami (docs/link-ui.md, « Prêt de l'ordinateur (KyberNow Share) », « L'ami ») : la barre et le
 * menu du téléviseur (« Prêté par {hôte} », le temps restant, « Rendre l'ordinateur » avec sa confirmation ; latence et
 * statistiques, son, souris et touches, manettes ; ni profil de la Station, ni réglages, ni presse-papiers), et la carte de
 * fin (« Vous pouvez revenir pendant 15 min. », « Revenir »).
 */

/**
 * « Encore 1 h 20 » (la fin prévue, ou celle du préavis : « Encore 2 min ») ou « Jusqu'à ce que {hôte} reprenne ». Le
 * temps avance de lui-même (chaque seconde pendant un préavis) ; sans animations (captures, essais), une seule fois.
 */
@Composable
internal fun lendTimeText(ui: SessionUi): String {
    val notice by ui.state.lendNotice.collectAsState()
    val at = Lend.endsAt(ui.lendUntil, notice) ?: return stringResource(R.string.lend_until_taken, ui.lender)
    val motion = LocalMotion.current
    val now by produceState(System.currentTimeMillis() / 1000, motion, at) {
        while (motion) {
            delay(if (notice != null) 1000 else 15_000)
            value = System.currentTimeMillis() / 1000
        }
    }
    return stringResource(R.string.lend_left, duration(Lend.roundUpToMinute(Lend.left(at, now))))
}

/**
 * Dans la partie fixe de la barre : « Prêté par {hôte} · Encore 1 h 20 » et « Rendre l'ordinateur » (qui demande
 * confirmation).
 */
@Composable
internal fun LentItems(ui: SessionUi) {
    val s = LocalScale.current
    Body(
        stringResource(R.string.lend_lent_by, ui.lender) + " · " + lendTimeText(ui),
        Modifier.clip(RoundedCornerShape(999.dp)).background(Kn.frost.copy(alpha = 0.06f)).padding(horizontal = s.dp(14f), vertical = s.dp(8f)),
        size = 14f, weight = 600, color = Kn.frost, maxLines = 1,
    )
    GlassButton(stringResource(R.string.lend_give_back), { ui.giveBackConfirm = true })
}

/** Les réglages de la barre d'un emprunt : mesures, toucher, manette à l'écran, souris, clavier, touches (comme l'hôte, sans profil ni écran). */
@Composable
internal fun BorrowerBarItems(ui: SessionUi, sec: Second?, first: FocusRequester) {
    StatsCard(ui, sec, Modifier.focusRequester(first))
    GlassButton(stringResource(if (ui.directTouch) R.string.touch_direct else R.string.touch_trackpad), { ui.actions.setDirectTouch(!ui.directTouch) })
    GlassButton(stringResource(R.string.pad_on_screen), { ui.actions.setOnScreenPad(!ui.padShown) }, selected = ui.padShown)
    GlassButton(
        stringResource(listOf(R.string.session_mouse_auto, R.string.mouse_desktop, R.string.mouse_game)[ui.mouseMode]),
        { ui.actions.setMouseMode((ui.mouseMode + 1) % 3) },
    )
    BarIcon(KnIcons.keyboard, stringResource(R.string.session_keyboard)) { ui.actions.openKeyboard(true) }
    GlassButton(stringResource(R.string.session_keys), { ui.keysOpen = true })
}

/**
 * Le menu d'un emprunt au téléviseur : reprendre, « Prêté par {hôte} » et le temps restant, le son, les statistiques, la
 * souris à la télécommande et à la manette, le texte, les touches, « Rendre l'ordinateur » et « Se déconnecter » (sans
 * rendre l'ordinateur : retour possible pendant 15 min).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun BorrowerTiles(ui: SessionUi, first: FocusRequester, audio: Pair<Int, Double>?) {
    val s = LocalScale.current
    val route by ui.state.route.collectAsState()
    FlowRow(Modifier.widthIn(max = s.dp(900f)), horizontalArrangement = Arrangement.spacedBy(s.dp(14f), Alignment.CenterHorizontally), verticalArrangement = Arrangement.spacedBy(s.dp(14f)), maxItemsInEachRow = 5) {
        Tile(KnIcons.play, stringResource(R.string.session_resume), null, Modifier.focusRequester(first), primary = true) { ui.actions.resume() }
        if (route?.viaRelay == true) Tile(KnIcons.network, stringResource(R.string.route_relay), null, selected = true) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.info, stringResource(R.string.lend_lent_by, ui.lender), lendTimeText(ui), selected = true) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.sound, stringResource(R.string.session_sound), audio?.let { (c, _) -> stringResource(R.string.session_sound_channels, c) }) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.pulse, stringResource(R.string.session_stats), null) { ui.statsOpen = !ui.statsOpen }
        Tile(KnIcons.remote, stringResource(R.string.session_remote_mouse), stringResource(if (ui.remoteMouse) R.string.session_on else R.string.session_off), selected = ui.remoteMouse) { ui.actions.setRemoteMouse(!ui.remoteMouse) }
        Tile(KnIcons.gamepad, stringResource(R.string.session_pad_mouse), stringResource(if (ui.padMouse) R.string.session_on else R.string.session_off), selected = ui.padMouse) { ui.actions.setPadMouse(!ui.padMouse) }
        Tile(KnIcons.keyboard, stringResource(R.string.session_type_text), null) { ui.typing = true }
        Tile(KnIcons.keys, stringResource(R.string.session_keys), stringResource(R.string.keys_ctrl_alt_del_short)) { ui.keysOpen = true }
        Tile(KnIcons.power, stringResource(R.string.lend_give_back), null, danger = true) { ui.giveBackConfirm = true }
        Tile(KnIcons.exit, stringResource(R.string.session_disconnect), null) { ui.actions.quit() }
    }
}

/** « Rendre l'ordinateur à {hôte} ? » : « Rendre l'ordinateur » (le focus y va) ou « Annuler ». */
@Composable
internal fun GiveBackConfirm(ui: SessionUi, modifier: Modifier) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(modifier.padding(s.dp(20f)).widthIn(max = s.dp(560f)).opaquePanel().padding(s.dp(22f)), verticalArrangement = Arrangement.spacedBy(s.dp(16f))) {
        Title(stringResource(R.string.lend_give_back_confirm, ui.lender), size = 22f)
        Row(horizontalArrangement = Arrangement.spacedBy(s.dp(10f))) {
            PrimaryButton(stringResource(R.string.lend_give_back), { ui.actions.giveBack() }, Modifier.focusRequester(focus))
            GlassButton(stringResource(R.string.cancel), { ui.giveBackConfirm = false })
        }
    }
}

/**
 * La fin d'un emprunt. Le prêt a fini (repris, durée passée, créneau terminé) : la raison du cœur et « Fermer ». La session
 * a fini autrement (réseau perdu, Kyber coupé) : l'ordinateur reste prêté 15 min, « Vous pouvez revenir pendant 15 min. »
 * et « Revenir » (la même demande d'emprunt), puis « Fermer ».
 */
@Composable
internal fun BorrowEndedCard(ui: SessionUi, e: Ended, modifier: Modifier) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    val over = ui.state.lendEnd != null
    val back = ui.comeBack
    LaunchedEffect(back?.step) { runCatching { focus.requestFocus() } }
    Column(modifier.padding(s.dp(20f)).panel(22.dp).padding(s.dp(24f)).widthIn(max = s.dp(620f)), verticalArrangement = Arrangement.spacedBy(s.dp(12f))) {
        val failed = ui.state.firstImageAt.get() == 0L
        Title(stringResource(if (failed && !over) R.string.session_failed else R.string.session_ended_title), size = 24f)
        // La fin du prêt est une phrase du cœur, avec le prénom de l'hôte : telle quelle.
        val text = e.error ?: ""
        Body(if (over) text else sentence(text), color = Kn.frost2, size = 15f)
        if (over) {
            PrimaryButton(stringResource(R.string.session_close), { ui.actions.quit() }, Modifier.focusRequester(focus))
            return@Column
        }
        Body(stringResource(R.string.lend_come_back_hint), color = Kn.frost, size = 15f, weight = 600)
        when (back?.step) {
            ComeBack.Step.WAITING -> {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
                    CircularProgressIndicator(Modifier.size(s.dp(22f)), color = Kn.spectre, strokeWidth = s.dp(3f))
                    Body(stringResource(R.string.lend_waiting, ui.lender), size = 16f, weight = 600, color = Kn.frost)
                }
                GlassButton(stringResource(R.string.cancel), { ui.actions.comeBackCancel() }, Modifier.focusRequester(focus))
            }
            ComeBack.Step.WORKING -> PrimaryButton(stringResource(R.string.lend_preparing), {}, Modifier.focusRequester(focus))
            else -> {
                back?.message?.takeIf { it.isNotBlank() }?.let { Body(it, color = Kn.warn, size = 14f) }
                Row(horizontalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                    PrimaryButton(stringResource(R.string.lend_come_back), { ui.actions.comeBack() }, Modifier.focusRequester(focus))
                    GlassButton(stringResource(R.string.session_close), { ui.actions.quit() })
                }
            }
        }
    }
}
