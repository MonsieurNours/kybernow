package fr.leslignesclaires.kybernow.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.AutoProfile
import fr.leslignesclaires.kybernow.model.AutoStep
import fr.leslignesclaires.kybernow.model.AutoSummary
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.ui.settings.Spinner

/*
 * Profil Auto (docs/link-ui.md, « Profil Auto et débit adaptatif ») : le panneau du test, par-dessus l'écran du moment
 * (accueil, réglages), et ce que les autres écrans en montrent (« Choisi par le test du {date} ({réseau}). »).
 */

/** Ce que le panneau demande à l'app. */
class AutoTestActions(val apply: () -> Unit, val cancel: () -> Unit, val again: () -> Unit)

/**
 * Le panneau : « Profil Auto », la phrase du test, puis les quatre étapes (en attente, une roue, cochée avec sa mesure,
 * passée avec sa raison) et « Annuler » ; à la fin, « Réglages proposés » (une ligne par réglage, valeur puis raison),
 * « Appliquer », « Annuler » et « Refaire le test » ; un échec dit pourquoi, avec « Réessayer ». Retour : « Annuler ».
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AutoTestPanel(state: AutoTestState, actions: AutoTestActions) {
    val s = LocalScale.current
    BackHandler { actions.cancel() }
    val first = remember { FocusRequester() }
    val phase = when {
        state.error != null -> 2
        state.result != null -> 1
        else -> 0
    }
    LaunchedEffect(phase) { runCatching { first.requestFocus() } }
    Box(
        Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.62f))
            // Rien ne passe à l'écran de dessous.
            .clickable(remember { MutableInteractionSource() }, indication = null) {}
            .statusBarsPadding().navigationBarsPadding().padding(s.dp(16f)),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier.widthIn(max = s.dp(560f)).fillMaxWidth().panel(22.dp).verticalScroll(rememberScrollState()).padding(s.dp(if (s.tv) 28f else 22f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(14f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                KnIcon(KnIcons.sparkles, tint = Kn.spectre, size = 22f)
                Spacer(Modifier.width(s.dp(10f)))
                Title(stringResource(R.string.autotest_title), size = 24f)
            }
            Body(stringResource(R.string.autotest_subtitle), size = 14f, color = Kn.frost2)
            when (phase) {
                0 -> {
                    Column(verticalArrangement = Arrangement.spacedBy(s.dp(12f))) { state.steps.forEach { StepRow(it) } }
                    if (state.steps.isEmpty()) Spinner(22f)
                    GlassButton(stringResource(R.string.autotest_cancel), actions.cancel, Modifier.focusRequester(first))
                }
                1 -> {
                    Eyebrow(stringResource(R.string.autotest_summary), color = Kn.spectre)
                    Column(verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                        state.choices.forEach { c ->
                            Column {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Body(c.title, size = 14f, color = Kn.frost2, modifier = Modifier.weight(1f))
                                    Spacer(Modifier.width(s.dp(10f)))
                                    Body(c.value, size = 15f, weight = 600)
                                }
                                Body(c.reason, size = 13f, color = Kn.muted)
                            }
                        }
                    }
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
                        PrimaryButton(stringResource(R.string.autotest_apply), actions.apply, Modifier.focusRequester(first))
                        GlassButton(stringResource(R.string.autotest_cancel), actions.cancel)
                        LinkButton(stringResource(R.string.autotest_again), actions.again)
                    }
                }
                else -> {
                    Row(verticalAlignment = Alignment.Top) {
                        KnIcon(KnIcons.warning, tint = Kn.warn, size = 18f)
                        Spacer(Modifier.width(s.dp(10f)))
                        Body(state.error ?: "", size = 15f)
                    }
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                        PrimaryButton(stringResource(R.string.autotest_retry), actions.again, Modifier.focusRequester(first))
                        GlassButton(stringResource(R.string.autotest_cancel), actions.cancel)
                    }
                }
            }
        }
    }
}

/** Une étape : sa pastille d'état, son titre, puis sa mesure (cochée) ou sa raison (passée). */
@Composable
private fun StepRow(step: AutoStep) {
    val s = LocalScale.current
    Row(verticalAlignment = Alignment.Top) {
        Box(Modifier.size(s.dp(22f)), contentAlignment = Alignment.Center) {
            when (step.state) {
                AutoStep.State.RUNNING -> Spinner(18f)
                AutoStep.State.DONE -> KnIcon(KnIcons.check, tint = Kn.spectre, size = 18f)
                AutoStep.State.SKIPPED -> KnIcon(KnIcons.skip, tint = Kn.muted, size = 18f)
                AutoStep.State.WAITING -> Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(Kn.line))
            }
        }
        Spacer(Modifier.width(s.dp(12f)))
        Column {
            Body(step.title, size = 15f, color = if (step.state == AutoStep.State.WAITING) Kn.muted else Kn.frost, weight = 500)
            step.text?.let { Body(it, size = 13f, color = Kn.frost2) }
        }
    }
}

/** « Refaire le test » : un lien, discret. */
@Composable
fun LinkButton(text: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    FocusCard(onClick, modifier, shape = androidx.compose.foundation.shape.RoundedCornerShape(999.dp)) {
        Body(text, size = 14f, color = Kn.spectre, weight = 600, modifier = Modifier.padding(horizontal = s.dp(12f), vertical = s.dp(9f)))
    }
}

/**
 * Sous le débit, profil Auto : « Choisi par le test du {date} ({réseau}). » ; le réseau est celui du test (« Wi-Fi »,
 * « Ethernet », le nom du Wi-Fi), « Par le relais », ou la liaison mesurée quand le nom manque. Nul sans test lisible.
 */
@Composable
fun autoChosenBy(set: HostSettings): String? {
    val r = AutoSummary.parse(set.autoResult) ?: return null
    val locale = LocalConfiguration.current.locales[0]
    val network = when {
        r.network == AutoProfile.RELAY || r.link == "relay" -> stringResource(R.string.autotest_link_relay)
        r.network != null -> r.network
        r.link == "cable" -> stringResource(R.string.autotest_link_cable)
        else -> stringResource(R.string.autotest_link_wifi)
    }
    return stringResource(R.string.autotest_chosen_by, AutoProfile.date(r.at, locale), network)
}
