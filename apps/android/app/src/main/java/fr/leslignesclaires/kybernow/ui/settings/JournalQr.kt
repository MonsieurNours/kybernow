package fr.leslignesclaires.kybernow.ui.settings

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Core
import fr.leslignesclaires.kybernow.model.Journal
import fr.leslignesclaires.kybernow.model.JournalPost
import fr.leslignesclaires.kybernow.model.JournalState
import fr.leslignesclaires.kybernow.model.JournalUpload
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.QrCode
import fr.leslignesclaires.kybernow.ui.Title
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** Les étapes de « Envoyer le journal » par QR code (docs/link-ui.md, « Réglages de l'app › À propos »). */
sealed class JournalQrStep {
    /** « Préparation du journal… » : le fichier, compressé et chiffré. */
    data object Preparing : JournalQrStep()
    /** « Envoi du journal… » : le dépôt sur le relais. */
    data object Uploading : JournalQrStep()
    /** Le QR code ; [relay] : celui du dépôt, tel que l'app l'a donné ; [until] : l'heure (ms, horloge de l'appareil) où il expire. */
    data class Shown(val url: String, val relay: String, val id: String, val owner: String, val until: Long) : JournalQrStep()
    /** « Journal ouvert sur le téléphone. » */
    data object Opened : JournalQrStep()
    /** « Ce QR code a expiré. », « Nouveau QR code ». */
    data object Expired : JournalQrStep()
    /** « Le relais ne répond pas… », « Réessayer ». */
    data object Unreachable : JournalQrStep()
    /** « Ce relais ne prend pas encore les journaux… » */
    data object OldRelay : JournalQrStep()
    /** « Trop d'envois : réessayez dans une heure. » */
    data object TooMany : JournalQrStep()

    companion object {
        /** L'étape après le dépôt. Un relais plein (503) dit, comme trop d'envois, de réessayer plus tard. */
        fun after(post: JournalPost, now: Long): JournalQrStep = when (post) {
            is JournalPost.Posted -> post.deposit.let { Shown(it.url, it.relay, it.id, it.owner, now + it.expiresIn * 1000L) }
            JournalPost.TooMany -> TooMany
            JournalPost.OldRelay -> OldRelay
            JournalPost.Unreachable -> Unreachable
        }
    }
}

/** Prépare le journal, puis le cœur le chiffre et le dépose : l'étape à montrer ensuite. Hors du fil principal. */
private fun upload(context: Context, relayAddress: String): JournalQrStep {
    val text = runCatching { Journal.build(context).readBytes() }.getOrNull() ?: return JournalQrStep.Unreachable
    val (header, body) = JournalUpload.split(text)
    return JournalQrStep.after(JournalUpload.readPost(Core.journalDeposit(relayAddress, header, body)), System.currentTimeMillis())
}

/**
 * Android TV : « Envoyer le journal » par un QR code que le téléphone scanne. Le journal est chiffré ici (par le cœur),
 * la clé n'est que dans le QR code ; le relais du compte de l'appareil ([relay], sinon le public) le garde une heure,
 * pour une ouverture. L'écran suit le dépôt toutes les 2 s ; « Terminé » (ou Retour) retire un dépôt pas encore ouvert.
 */
@Composable
fun JournalQrScreen(relay: String, onClose: () -> Unit) {
    val context = LocalContext.current
    var attempt by remember { mutableIntStateOf(0) }
    var step by remember { mutableStateOf<JournalQrStep>(JournalQrStep.Preparing) }
    val current by rememberUpdatedState(step)
    BackHandler(onBack = onClose)
    LaunchedEffect(attempt) {
        step = JournalQrStep.Preparing
        // Après une seconde de préparation, le dépôt a commencé : l'étape suivante est dite pendant l'envoi.
        val job = launch { delay(1000); if (step == JournalQrStep.Preparing) step = JournalQrStep.Uploading }
        val next = withContext(Dispatchers.IO) { upload(context.applicationContext, relay) }
        job.cancel()
        step = next
        val shown = next as? JournalQrStep.Shown ?: return@LaunchedEffect
        while (true) {
            delay(2000)
            if (System.currentTimeMillis() >= shown.until) { step = JournalQrStep.Expired; return@LaunchedEffect }
            when (withContext(Dispatchers.IO) { JournalUpload.readState(Core.journalState(shown.relay, shown.id, shown.owner)) }) {
                JournalState.OPENED -> { step = JournalQrStep.Opened; return@LaunchedEffect }
                JournalState.GONE -> { step = JournalQrStep.Expired; return@LaunchedEffect }
                // Un suivi manqué (réseau) ne change rien : le QR code reste.
                JournalState.WAITING, JournalState.UNKNOWN -> {}
            }
        }
    }
    // En quittant l'écran, un dépôt pas encore ouvert est retiré du relais.
    DisposableEffect(Unit) {
        onDispose {
            val s = current as? JournalQrStep.Shown ?: return@onDispose
            CoroutineScope(Dispatchers.IO).launch { Core.journalDiscard(s.relay, s.id, s.owner) }
        }
    }
    JournalQrContent(step, onDone = onClose, onRetry = { attempt++ })
}

/** L'écran, d'après l'étape : titre, phrase, QR code et petite ligne ; « Terminé », et « Réessayer » ou « Nouveau QR code ». */
@Composable
fun JournalQrContent(step: JournalQrStep, onDone: () -> Unit, onRetry: () -> Unit) {
    val s = LocalScale.current
    val first = remember { FocusRequester() }
    val retry = step == JournalQrStep.Expired || step == JournalQrStep.Unreachable
    LaunchedEffect(step::class) { runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val wide = maxWidth > maxHeight
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = s.dp(if (s.tv) 56f else 20f), vertical = s.dp(if (s.tv) 32f else 20f)),
                verticalArrangement = Arrangement.spacedBy(s.dp(16f)),
            ) {
                Title(stringResource(R.string.journal_send), size = if (s.tv) 36f else 26f)
                val text: @Composable () -> Unit = {
                    Column(verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
                        when (step) {
                            JournalQrStep.Preparing, JournalQrStep.Uploading -> Row(
                                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(12f)),
                            ) {
                                CircularProgressIndicator(Modifier.size(s.dp(22f)), color = Kn.spectre, strokeWidth = s.dp(3f))
                                Body(
                                    stringResource(if (step == JournalQrStep.Preparing) R.string.journal_preparing else R.string.journal_uploading),
                                    size = if (s.tv) 18f else 16f, color = Kn.frost2,
                                )
                            }
                            is JournalQrStep.Shown -> {
                                Body(stringResource(R.string.journal_qr_body), size = if (s.tv) 18f else 16f, color = Kn.frost)
                                Body(stringResource(R.string.journal_qr_note), size = 13f, color = Kn.muted)
                            }
                            JournalQrStep.Opened -> Body(stringResource(R.string.journal_opened), size = if (s.tv) 18f else 16f, color = Kn.frost)
                            JournalQrStep.Expired -> Body(stringResource(R.string.journal_expired), size = 16f, color = Kn.warn)
                            JournalQrStep.Unreachable -> Body(stringResource(R.string.journal_unreachable), size = 16f, color = Kn.warn)
                            JournalQrStep.OldRelay -> Body(stringResource(R.string.journal_old_relay), size = 16f, color = Kn.warn)
                            JournalQrStep.TooMany -> Body(stringResource(R.string.journal_too_many), size = 16f, color = Kn.warn)
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                            if (retry) PrimaryButton(
                                stringResource(if (step == JournalQrStep.Expired) R.string.journal_new_qr else R.string.home_retry),
                                onRetry, Modifier.focusRequester(first),
                            )
                            GlassButton(stringResource(R.string.journal_done), onDone, if (retry) Modifier else Modifier.focusRequester(first))
                        }
                    }
                }
                val qr: @Composable () -> Unit = {
                    if (step is JournalQrStep.Shown) QrCode(step.url, if (s.tv) 260f else 220f, stringResource(R.string.journal_send))
                }
                if (wide) {
                    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(40f)), verticalAlignment = Alignment.Top) {
                        Box(Modifier.weight(1f)) { text() }
                        qr()
                    }
                } else {
                    qr()
                    text()
                }
            }
        }
    }
}
