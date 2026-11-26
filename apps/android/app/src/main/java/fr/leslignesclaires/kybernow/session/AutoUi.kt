package fr.leslignesclaires.kybernow.session

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.AutoProfile
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.panel
import kotlinx.coroutines.delay

/**
 * Profil Auto en session (docs/link-ui.md, « Profil Auto et débit adaptatif », « En session ») : le bandeau discret
 * « Autre réseau que celui du test » au début, quand le réseau du moment (ou le relais) n'est pas celui du test ;
 * « Le débit adaptatif s'en charge. », « Refaire le test » (le panneau s'ouvre après la session) et la croix ; parti
 * tout seul après 8 s. Une fois par session, hôte seulement, dès que le chemin de la session est connu.
 */
@Composable
internal fun OtherNetworkBanner(ui: SessionUi) {
    val result = ui.autoResult ?: return
    val route by ui.state.route.collectAsState()
    val known = route != null
    LaunchedEffect(known) {
        val r = route ?: return@LaunchedEffect
        if (ui.otherNetworkDone) return@LaunchedEffect
        ui.otherNetworkDone = true
        val network = if (r.viaRelay) AutoProfile.RELAY else ui.autoNetwork
        if (AutoProfile.sameNetwork(result, network)) return@LaunchedEffect
        ui.otherNetwork = true
        delay(8_000)
        ui.otherNetwork = false
    }
    if (!ui.otherNetwork) return
    val s = LocalScale.current
    Row(
        Modifier.padding(horizontal = s.dp(16f)).widthIn(max = s.dp(640f)).panel(14.dp).padding(s.dp(14f)),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(s.dp(12f)),
    ) {
        KnIcon(KnIcons.network, tint = Kn.frost2, size = 18f)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(4f))) {
            Body(stringResource(R.string.autotest_other_network), size = 14f, weight = 700)
            Body(stringResource(R.string.autotest_other_network_text), size = 14f, color = Kn.frost2)
        }
        GlassButton(stringResource(R.string.autotest_again), { ui.otherNetwork = false; ui.actions.retestAuto() })
        val close = stringResource(R.string.session_close)
        FocusCard({ ui.otherNetwork = false }, Modifier.semantics { contentDescription = close }) {
            KnIcon(KnIcons.close, tint = Kn.frost2, size = 16f, modifier = Modifier.padding(s.dp(8f)))
        }
    }
}
