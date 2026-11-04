package fr.leslignesclaires.kybernow.ui.playstation

import android.content.Context
import android.media.MediaCodecInfo
import android.media.MediaCodecList
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
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
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.PsNative
import fr.leslignesclaires.kybernow.model.HdrScreen
import fr.leslignesclaires.kybernow.playstation.PsAccount
import fr.leslignesclaires.kybernow.playstation.PsConsole
import fr.leslignesclaires.kybernow.playstation.PsFound
import fr.leslignesclaires.kybernow.playstation.PsStatus
import fr.leslignesclaires.kybernow.playstation.ReachError
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FitTitle
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.home.elides

/** Ce que l'accueil montre des consoles. */
data class PsHomeState(
    val consoles: List<PsConsole> = emptyList(),
    val status: Map<String, PsStatus> = emptyMap(),
    /** Le jeu en cours, quand la console en annonce un. */
    val running: Map<String, String> = emptyMap(),
    /** Les comptes associés à chaque console, et celui choisi. */
    val accounts: Map<String, List<PsAccount>> = emptyMap(),
    val chosen: Map<String, PsAccount> = emptyMap(),
    /** Consoles trouvées sur le réseau et pas encore associées (« Sur le réseau »). */
    val unpaired: List<PsFound> = emptyList(),
    val waking: Set<String> = emptySet(),
    val reaching: Set<String> = emptySet(),
    val reachErrors: Map<String, ReachError> = emptyMap(),
    /** L'écran est HDR et l'appareil décode le HEVC HDR10 : la puce « HDR » existe. */
    val hdrScreen: Boolean = false,
    val hdrDecoder: Boolean = false,
)

/** Ce que l'accueil demande pour les consoles. */
class PsHomeActions(
    val play: (PsConsole) -> Unit = {},
    val pair: (PsFound) -> Unit = {},
    /** Une console qui n'a plus d'association : l'onglet PlayStation, sur son association. */
    val pairConsole: (PsConsole) -> Unit = {},
    val settings: (PsConsole) -> Unit = {},
    val choose: (PsConsole, PsAccount) -> Unit = { _, _ -> },
    val setHdr: (PsConsole, Boolean) -> Unit = { _, _ -> },
)

/** HDR de l'écran et du décodeur (docs/link-ui.md, « HDR » : la puce n'existe que sur un écran HDR). */
object PsHdr {
    /** L'écran principal affiche le HDR10 (la même règle que pour un ordinateur, [HdrScreen]). */
    fun screen(context: Context): Boolean = HdrScreen.of(context).hdr

    /** Un décodeur matériel HEVC qui lit le Main10 HDR10. */
    val decoder: Boolean by lazy {
        runCatching {
            MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos.filter { !it.isEncoder && it.isHardwareAccelerated }.any { info ->
                info.supportedTypes.any { it.equals("video/hevc", true) } &&
                    info.getCapabilitiesForType("video/hevc").profileLevels.any { it.profile == MediaCodecInfo.CodecProfileLevel.HEVCProfileMain10HDR10 }
            }
        }.getOrDefault(false)
    }
}

/** La puce HDR active : écran et décodeur HDR, PS5, format H.265 ou Auto. Nul : la puce est active ; sinon la raison. */
@Composable
private fun hdrReason(c: PsConsole, state: PsHomeState): String? = when {
    !c.ps5 -> stringResource(R.string.ps_hdr_ps4)
    !state.hdrDecoder -> stringResource(R.string.ps_hdr_no_decoder)
    c.settings?.contains("\"codec\":\"h264\"") == true -> stringResource(R.string.ps_hdr_needs_hevc)
    else -> null
}

@Composable
fun psSettingsTitle(name: String) = stringResource(if (elides(name)) R.string.ps_settings_for_elided else R.string.ps_settings_for, name)

/**
 * La console choisie à l'accueil (docs/link-ui.md, « Consoles PlayStation ») : état, nom, modèle, jeu en cours, bouton
 * principal (« Jouer », « Réveiller et jouer », « Associer »), puce du compte, puce HDR, puce « Auto » ou
 * « Personnalisé », engrenage « Réglages de {nom} ».
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun ConsoleHero(c: PsConsole, state: PsHomeState, actions: PsHomeActions, nameSize: Float, focus: FocusRequester?, stacked: Boolean) {
    val s = LocalScale.current
    val st = state.status[c.id] ?: PsStatus.SEARCHING
    val accounts = state.accounts[c.id].orEmpty()
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(if (s.tv) 7f else 10f))) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(Modifier.size(s.dp(8f)).clip(CircleShape).background(if (st == PsStatus.ON || st == PsStatus.AWAY) Kn.spectre else Kn.muted))
            Spacer(Modifier.width(s.dp(10f)))
            Eyebrow(psStatusLabel(st), color = Kn.frost2)
        }
        FitTitle(c.name, Modifier.fillMaxWidth(), size = nameSize, min = nameSize * 0.45f)
        Body(c.model, size = 16f, color = Kn.frost2, maxLines = 1)
        state.running[c.id]?.let { Body(it, size = 15f, color = Kn.frost2, maxLines = 1) }
        state.reachErrors[c.id]?.let { e ->
            Body(
                when (e) {
                    is ReachError.Relay -> when (e.code) {
                        31 -> stringResource(R.string.relay_station_offline, e.station)
                        32 -> stringResource(R.string.relay_access_off, e.station)
                        else -> e.message.ifBlank { stringResource(R.string.ps_end_unreachable) }
                    }
                    ReachError.NotAnswering -> stringResource(R.string.ps_end_unreachable)
                },
                color = Kn.warn, size = 13f,
            )
        }
        Spacer(Modifier.height(s.dp(if (s.tv) 2f else 6f)))
        val first = Modifier.then(if (focus != null) Modifier.focusRequester(focus) else Modifier)
        val main = if (stacked) first.widthIn(max = s.dp(560f)).fillMaxWidth() else first
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
            when {
                accounts.isEmpty() -> PrimaryButton(stringResource(R.string.ps_pair), { actions.pairConsole(c) }, main)
                c.id in state.waking -> PrimaryButton(stringResource(R.string.ps_waking), {}, main)
                c.id in state.reaching -> PrimaryButton(stringResource(R.string.ps_connecting), {}, main)
                st == PsStatus.STANDBY -> PrimaryButton(stringResource(R.string.ps_wake_play), { actions.play(c) }, main)
                st == PsStatus.OFFLINE -> PrimaryButton(stringResource(R.string.ps_play), { actions.play(c) }, main, enabled = false)
                else -> PrimaryButton(stringResource(R.string.ps_play), { actions.play(c) }, main)
            }
            // Associée avec plusieurs comptes : le profil avec lequel la console ouvre la partie.
            if (accounts.size > 1) AccountChip(c, accounts, state.chosen[c.id], actions)
            if (state.hdrScreen) HdrChip(c, state, actions)
            val custom = c.settings?.let(PsNative::settingsCustom) == true
            GlassButton(stringResource(if (custom) R.string.ps_custom else R.string.ps_auto), { actions.settings(c) }, selected = custom)
            val a11y = psSettingsTitle(c.name)
            GlassButton(
                stringResource(R.string.home_host_settings), { actions.settings(c) }, Modifier.semantics { contentDescription = a11y },
                leading = { KnIcon(KnIcons.settings, size = 16f); Spacer(Modifier.width(6.dp)) },
            )
        }
    }
}

@Composable
private fun AccountChip(c: PsConsole, accounts: List<PsAccount>, chosen: PsAccount?, actions: PsHomeActions) {
    var open by remember { mutableStateOf(false) }
    Box {
        GlassButton(chosen?.let { accountName(it) } ?: "", { open = true })
        DropdownMenu(open, { open = false }) {
            accounts.forEach { a -> DropdownMenuItem({ Text(accountName(a)) }, { open = false; actions.choose(c, a) }) }
        }
    }
}

@Composable
private fun HdrChip(c: PsConsole, state: PsHomeState, actions: PsHomeActions) {
    val reason = hdrReason(c, state)
    val on = c.hdr && reason == null
    val desc = stringResource(if (on) R.string.ps_hdr_on else R.string.ps_hdr_off)
    Column {
        GlassButton(stringResource(R.string.ps_hdr), { actions.setHdr(c, !c.hdr) }, Modifier.semantics { stateDescription = desc }, selected = on, enabled = reason == null)
    }
}

/** La carte d'une console dans la rangée : son nom et son état. */
@Composable
fun ConsoleCard(c: PsConsole, status: PsStatus, selected: Boolean, card: Float, onClick: () -> Unit, onSettings: () -> Unit) {
    val s = LocalScale.current
    var menu by remember { mutableStateOf(false) }
    Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        FocusCard(onClick, onLongClick = { menu = true }) {
            Box(
                Modifier.fillMaxWidth().aspectRatio(16f / 9f).background(Kn.panel)
                    .border(BorderStroke(if (selected) 2.dp else 1.dp, if (selected) Kn.frost.copy(alpha = 0.6f) else Kn.line), RoundedCornerShape(16.dp)),
                contentAlignment = Alignment.Center,
            ) { Mono(c.model, size = 18f, color = if (status == PsStatus.ON) Kn.frost else Kn.muted, weight = 600) }
        }
        Column {
            Body(c.name, size = 15f, weight = if (selected) 600 else 400, maxLines = 1)
            Mono(psStatusLabel(status), size = 10f, color = if (status == PsStatus.ON || status == PsStatus.AWAY) Kn.frost2 else Kn.muted, maxLines = 1)
        }
        DropdownMenu(menu, { menu = false }) {
            DropdownMenuItem({ Text(psSettingsTitle(c.name)) }, { menu = false; onSettings() })
        }
    }
}

/** Une console trouvée sur le réseau et pas encore associée : « Sur le réseau », ouvre son association. */
@Composable
fun FoundCard(f: PsFound, card: Float, onClick: () -> Unit) {
    val s = LocalScale.current
    Column(Modifier.width(s.dp(card)), verticalArrangement = Arrangement.spacedBy(s.dp(6f))) {
        FocusCard(onClick) {
            Box(
                Modifier.fillMaxWidth().aspectRatio(16f / 9f).border(BorderStroke(1.dp, Kn.spectre.copy(alpha = 0.6f)), RoundedCornerShape(16.dp)),
                contentAlignment = Alignment.Center,
            ) { Mono(f.model, size = 18f, color = Kn.spectre, weight = 600) }
        }
        Column {
            Body(f.name, size = 15f, maxLines = 1)
            Mono(stringResource(R.string.home_on_network), size = 10f, color = Kn.spectre, maxLines = 1)
        }
    }
}
