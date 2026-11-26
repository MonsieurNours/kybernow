package fr.leslignesclaires.kybernow.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.StationBanner
import fr.leslignesclaires.kybernow.core.StationOffer
import fr.leslignesclaires.kybernow.ui.settings.Spinner

/*
 * Mise à jour des Stations proposée par l'app (docs/link-ui.md, « Mise à jour des Stations ») : le bandeau de l'accueil
 * sous la rangée « Se connecter » et profils, et l'avis après une mise à jour de l'app. « Mettre à jour » suit le flux de
 * la rubrique Station des réglages ([AppModel.station]) : mêmes confirmation, suivi et textes d'échec.
 */

/** Le texte d'un échec de la Station (rubrique Station des réglages, repris par le bandeau et l'avis). */
@Composable
fun stationFailureText(task: StationTask.Failed, version: String?): String = when (task.reason) {
    StationFailure.TOO_SOON -> stringResource(R.string.station_too_soon)
    StationFailure.TOO_OLD -> stringResource(R.string.station_too_old)
    StationFailure.REQUIRES_SETUP -> stringResource(R.string.station_requires_setup, task.version ?: version ?: "")
    StationFailure.NOT_BACK -> stringResource(R.string.station_not_back)
    StationFailure.UPDATE_FAILED -> stringResource(R.string.station_update_failed)
    StationFailure.OTHER -> task.message.replaceFirstChar { it.uppercase() }
}

/** « {appareil} est connecté » (ou « Un appareil est connecté »). */
@Composable
private fun inSessionTitle(task: StationTask.Confirm): String =
    task.device?.let { stringResource(R.string.station_in_session, it) } ?: stringResource(R.string.station_in_session_unknown)

/** Ce que le bandeau demande à l'app : « Mettre à jour » ([update], `true` : « quand même »), « Plus tard », « Annuler ». */
class StationBannerActions(val update: (force: Boolean) -> Unit, val later: (version: String) -> Unit, val cancel: () -> Unit)

/**
 * Le bandeau de l'ordinateur choisi : la proposition (« Mettre à jour », « Plus tard »), l'installateur ou « Hors de
 * chez vous » (« Plus tard » seul), « Mise à jour de la Station… » pendant ; lancée d'ici ([mine]) : la confirmation
 * quand un appareil est en session, et l'échec avec « Réessayer ». Rien quand il n'y a rien à dire. Au téléviseur, ses
 * boutons sont sous « Se connecter » : la flèche bas y mène.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StationBannerView(offer: StationOffer?, task: StationTask?, mine: Boolean, actions: StationBannerActions, modifier: Modifier = Modifier) {
    val banner = offer?.banner ?: StationBanner.None
    val version = when (banner) {
        is StationBanner.Update -> banner.version
        is StationBanner.Setup -> banner.version
        is StationBanner.Away -> banner.version
        else -> null
    }
    val later: () -> Unit = { if (version != null) actions.later(version) else actions.cancel() }
    when {
        task is StationTask.Done -> Unit
        task is StationTask.Working && task.action == StationAction.UPDATE || banner == StationBanner.Installing && task !is StationTask.Working ->
            BannerPanel(modifier, spinner = true) { Body(stringResource(R.string.station_updating), size = 15f) }
        mine && task is StationTask.Confirm && task.action == StationAction.UPDATE -> BannerPanel(modifier, warn = true) {
            Title(inSessionTitle(task), size = 18f)
            Body(stringResource(R.string.station_in_session_update), size = 14f, color = Kn.frost2)
            Buttons {
                GlassButton(stringResource(R.string.station_update_anyway), { actions.update(true) }, selected = true)
                GlassButton(stringResource(R.string.cancel), actions.cancel)
            }
        }
        mine && task is StationTask.Failed -> BannerPanel(modifier, warn = true) {
            Body(stationFailureText(task, version), size = 14f, color = Kn.warn)
            Buttons {
                GlassButton(stringResource(R.string.home_retry), { actions.update(false) }, selected = true)
                GlassButton(stringResource(R.string.station_offer_later), later)
            }
        }
        banner is StationBanner.Update -> BannerPanel(modifier) {
            Body(stringResource(R.string.station_offer_available, banner.version), size = 15f)
            Buttons {
                PrimaryButton(stringResource(R.string.station_offer_update), { actions.update(false) })
                GlassButton(stringResource(R.string.station_offer_later), later)
            }
        }
        banner is StationBanner.Setup || banner is StationBanner.Away -> BannerPanel(modifier) {
            Body(
                if (banner is StationBanner.Setup) stringResource(R.string.station_offer_setup, banner.version)
                else stringResource(R.string.station_offer_away, version ?: ""),
                size = 15f,
            )
            Buttons { GlassButton(stringResource(R.string.station_offer_later), later) }
        }
        else -> Unit
    }
}

/**
 * [StationBannerView] montre-t-il quelque chose ? Les mêmes cas, dans le même ordre. Le bandeau après un appairage (compte
 * KyberNow) attend qu'il n'y en ait plus : un seul bandeau à la fois sous « Se connecter », la mise à jour d'abord.
 */
fun stationBannerVisible(offer: StationOffer?, task: StationTask?, mine: Boolean): Boolean {
    val banner = offer?.banner ?: StationBanner.None
    return when {
        task is StationTask.Done -> false
        task is StationTask.Working && task.action == StationAction.UPDATE || banner == StationBanner.Installing && task !is StationTask.Working -> true
        mine && task is StationTask.Confirm && task.action == StationAction.UPDATE -> true
        mine && task is StationTask.Failed -> true
        banner is StationBanner.Update || banner is StationBanner.Setup || banner is StationBanner.Away -> true
        else -> false
    }
}

/** Le cadre du bandeau : l'icône (flèche vers le haut, ou la roue pendant la mise à jour), puis le texte et les boutons. */
@Composable
private fun BannerPanel(modifier: Modifier, spinner: Boolean = false, warn: Boolean = false, content: @Composable () -> Unit) {
    val s = LocalScale.current
    val shape = RoundedCornerShape(18.dp)
    Row(
        modifier.fillMaxWidth().panel()
            .then(if (warn) Modifier.border(BorderStroke(1.dp, Kn.warn.copy(alpha = 0.5f)), shape) else Modifier)
            .padding(s.dp(14f)),
        verticalAlignment = if (spinner) Alignment.CenterVertically else Alignment.Top,
    ) {
        if (spinner) Spinner(18f) else KnIcon(KnIcons.arrowUp, tint = if (warn) Kn.warn else Kn.spectre, size = 20f)
        Spacer(Modifier.width(s.dp(12f)))
        Column(verticalArrangement = Arrangement.spacedBy(s.dp(10f))) { content() }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Buttons(content: @Composable () -> Unit) {
    val s = LocalScale.current
    FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) { content() }
}

/** La puce « · Mise à jour » après l'état d'un ordinateur de la rangée (« En ligne · Mise à jour »). */
@Composable
fun withUpdateChip(status: String, chip: Boolean): String = if (chip) stringResource(R.string.station_offer_chip, status) else status

// MARK: - L'avis après une mise à jour de l'app

/** Ce que l'avis demande à l'app : « Mettre à jour » d'une ligne ([update], `true` : « quand même »), « Tout mettre à jour », « Plus tard » ou « Fermer ». */
class StationNoticeActions(val update: (id: String, force: Boolean) -> Unit, val updateAll: () -> Unit, val close: () -> Unit)

/**
 * L'avis : une Station (« {nom} a aussi une mise à jour », « Mettre à jour la Station », « Plus tard ») ou plusieurs
 * (« Vos Stations aussi ont une mise à jour », une ligne par ordinateur, « Tout mettre à jour », « Plus tard »). Quand
 * tout est à jour, « Plus tard » devient « Fermer ». Fenêtre au téléphone et à la tablette, plein écran au téléviseur ;
 * le focus sur le bouton principal. Retour : « Plus tard ».
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StationNoticePanel(notice: StationNotice, tasks: Map<String, StationTask>, actions: StationNoticeActions) {
    val s = LocalScale.current
    BackHandler { actions.close() }
    val first = remember { FocusRequester() }
    val one = notice.entries.singleOrNull()
    // Le focus revient au bouton principal quand il change (proposition, confirmation, « Fermer »).
    val phase = when {
        notice.allDone -> "done"
        one != null -> tasks[one.id]?.let { it::class.simpleName } ?: "idle"
        else -> "list"
    }
    LaunchedEffect(phase) { runCatching { first.requestFocus() } }
    val tv = s.tv
    Box(
        Modifier.fillMaxSize().background(if (tv) Kn.ink else Color.Black.copy(alpha = 0.62f))
            // Rien ne passe à l'écran de dessous.
            .clickable(remember { MutableInteractionSource() }, indication = null) {}
            .statusBarsPadding().navigationBarsPadding().padding(s.dp(if (tv) 48f else 16f)),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier.widthIn(max = s.dp(if (tv) 720f else 560f)).fillMaxWidth()
                .then(if (tv) Modifier else Modifier.panel(22.dp))
                .verticalScroll(rememberScrollState()).padding(s.dp(if (tv) 12f else 22f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(14f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                KnIcon(KnIcons.arrowUp, tint = Kn.spectre, size = 22f)
                Spacer(Modifier.width(s.dp(10f)))
                Title(if (one != null) stringResource(R.string.station_notice_title_one, one.name) else stringResource(R.string.station_notice_title_many), size = if (tv) 30f else 24f)
            }
            Body(
                if (one != null) stringResource(R.string.station_notice_body_one, notice.app) else stringResource(R.string.station_notice_body_many, notice.app),
                size = 15f, color = Kn.frost2,
            )
            if (one != null) {
                val task = tasks[one.id]
                val done = one.id in notice.updated
                when {
                    done -> Row(verticalAlignment = Alignment.CenterVertically) {
                        KnIcon(KnIcons.check, tint = Kn.spectre, size = 18f)
                        Spacer(Modifier.width(s.dp(10f)))
                        Body(stringResource(R.string.station_up_to_date), size = 15f)
                    }
                    task is StationTask.Working -> Row(verticalAlignment = Alignment.CenterVertically) {
                        Spinner(18f)
                        Spacer(Modifier.width(s.dp(10f)))
                        Body(stringResource(R.string.station_notice_updating), size = 15f)
                    }
                    task is StationTask.Confirm -> Column(verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
                        Body(inSessionTitle(task), size = 15f, color = Kn.warn, weight = 600)
                        Body(stringResource(R.string.station_in_session_update), size = 14f, color = Kn.frost2)
                    }
                    task is StationTask.Failed -> Body(stationFailureText(task, one.version), size = 14f, color = Kn.warn)
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
                    when {
                        done -> GlassButton(stringResource(R.string.session_close), actions.close, Modifier.focusRequester(first), selected = true)
                        task is StationTask.Working -> GlassButton(stringResource(R.string.station_offer_later), actions.close, Modifier.focusRequester(first))
                        task is StationTask.Confirm -> {
                            PrimaryButton(stringResource(R.string.station_update_anyway), { actions.update(one.id, true) }, Modifier.focusRequester(first))
                            GlassButton(stringResource(R.string.station_offer_later), actions.close)
                        }
                        else -> {
                            PrimaryButton(stringResource(R.string.station_update), { actions.update(one.id, false) }, Modifier.focusRequester(first))
                            GlassButton(stringResource(R.string.station_offer_later), actions.close)
                        }
                    }
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                    notice.entries.forEach { e -> NoticeRow(e, tasks[e.id], e.id in notice.updated) { force -> actions.update(e.id, force) } }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
                    if (notice.allDone) {
                        GlassButton(stringResource(R.string.session_close), actions.close, Modifier.focusRequester(first), selected = true)
                    } else {
                        PrimaryButton(stringResource(R.string.station_notice_update_all), actions.updateAll, Modifier.focusRequester(first))
                        GlassButton(stringResource(R.string.station_offer_later), actions.close)
                    }
                }
            }
        }
    }
}

/** Une ligne de l'avis : le nom, « Version {v} » (ou le refus, son texte court), puis « Mettre à jour », « Mise à jour… » ou « À jour ». */
@Composable
private fun NoticeRow(e: NoticeEntry, task: StationTask?, done: Boolean, update: (Boolean) -> Unit) {
    val s = LocalScale.current
    Row(Modifier.fillMaxWidth().panel(14.dp).padding(horizontal = s.dp(14f), vertical = s.dp(10f)), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Body(e.name, size = 15f, weight = 600, maxLines = 1)
            when {
                !done && task is StationTask.Confirm -> Body(inSessionTitle(task), size = 13f, color = Kn.warn)
                !done && task is StationTask.Failed -> Body(stationFailureText(task, e.version), size = 13f, color = Kn.warn)
                else -> Mono(stringResource(R.string.station_notice_version, e.version), size = 11f, color = Kn.frost2)
            }
        }
        Spacer(Modifier.width(s.dp(10f)))
        when {
            done -> Row(verticalAlignment = Alignment.CenterVertically) {
                KnIcon(KnIcons.check, tint = Kn.spectre, size = 16f)
                Spacer(Modifier.width(s.dp(6f)))
                Body(stringResource(R.string.station_up_to_date), size = 14f)
            }
            task is StationTask.Working -> Row(verticalAlignment = Alignment.CenterVertically) {
                Spinner(16f)
                Spacer(Modifier.width(s.dp(6f)))
                Body(stringResource(R.string.station_notice_updating), size = 14f)
            }
            task is StationTask.Confirm -> GlassButton(stringResource(R.string.station_update_anyway), { update(true) })
            else -> GlassButton(stringResource(R.string.station_offer_update), { update(false) })
        }
    }
}
