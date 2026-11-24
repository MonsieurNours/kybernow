package fr.leslignesclaires.kybernow.ui.account

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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.AccountCode
import fr.leslignesclaires.kybernow.core.AccountReply
import fr.leslignesclaires.kybernow.core.AccountSummary
import fr.leslignesclaires.kybernow.core.Accounts
import fr.leslignesclaires.kybernow.core.StoredAccount
import fr.leslignesclaires.kybernow.core.TvCode
import fr.leslignesclaires.kybernow.core.TvPoll
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnFonts
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.QrCode
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.panel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive

/*
 * Les écrans des comptes KyberNow (docs/link-ui.md, « Comptes KyberNow » ; contrat : docs/comptes.md) : le code d'Android
 * TV, le choix du relais (« Un autre relais… ») et les comptes des réglages de l'app. L'app ne crée ni ne supprime de
 * compte : elle renvoie à la page du compte, sur le relais.
 */

private val fieldColors
    @Composable get() = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, focusedLabelColor = Kn.spectre, cursorColor = Kn.spectre)

// MARK: - Le code (Android TV)

/** Où en est l'écran du code. */
sealed interface CodeStep {
    /** Le code est demandé au relais. */
    data object Asking : CodeStep
    /** Le code à approuver, et les minutes qui lui restent. */
    data class Showing(val code: TvCode, val minutes: Int) : CodeStep
    /** « Ce n'est pas moi » sur le téléphone : « Connexion refusée sur le téléphone. », « Nouveau code ». */
    data object Denied : CodeStep
    /** Le relais refuse ou se tait (le texte du cœur : relais sans comptes, illisible, muet, trop d'essais…). */
    data class Failed(val message: String) : CodeStep
}

/**
 * La décision après une demande du code (`accountTvPoll`) : continuer d'attendre ([WAIT], aussi quand le relais se tait
 * un moment : le code vaut 10 minutes), attendre plus longtemps ([SLOW], trop d'essais), demander un nouveau code de
 * lui-même ([RENEW], 73), ou s'arrêter (refus : [CodeStep.Denied] pour 72, le texte sinon).
 */
internal enum class PollNext { WAIT, SLOW, RENEW, STOP }

internal fun pollNext(code: Int): PollNext = when (code) {
    AccountCode.EXPIRED -> PollNext.RENEW
    AccountCode.TOO_MANY -> PollNext.SLOW
    AccountCode.UNREACHABLE -> PollNext.WAIT
    else -> PollNext.STOP
}

/**
 * « Se connecter à un compte KyberNow » au téléviseur : un code de huit signes et son QR code, approuvés depuis le
 * téléphone. Le relais est demandé toutes les `interval` secondes tant que l'écran est là (la boucle s'arrête en le
 * quittant) ; un code échu est remplacé de lui-même ; approuvé, [onApproved] garde le compte.
 */
@Composable
fun AccountCodeScreen(relay: String, onApproved: (StoredAccount) -> Unit, onOtherRelay: () -> Unit, onBack: () -> Unit) {
    val app = KyberNowApp.instance
    var step by remember { mutableStateOf<CodeStep>(CodeStep.Asking) }
    // « Nouveau code » relance tout (un nouveau code, une nouvelle attente).
    var attempt by remember { mutableIntStateOf(0) }
    val approved by rememberUpdatedState(onApproved)
    LaunchedEffect(relay, attempt) {
        step = CodeStep.Asking
        while (isActive) {
            val code = when (val r = Accounts.tvCode(relay, app.device.name)) {
                is AccountReply.Ok -> r.value
                is AccountReply.Refused -> {
                    step = CodeStep.Failed(r.message)
                    return@LaunchedEffect
                }
            }
            step = CodeStep.Showing(code, Accounts.tvMinutesLeft(code))
            var renew = false
            while (isActive && !renew) {
                delay(code.interval * 1000)
                val r = Accounts.tvPoll(code)
                step = CodeStep.Showing(code, Accounts.tvMinutesLeft(code))
                when (r) {
                    is AccountReply.Ok -> when (val p = r.value) {
                        TvPoll.Pending -> {}
                        is TvPoll.Approved -> {
                            approved(p.account)
                            return@LaunchedEffect
                        }
                    }
                    is AccountReply.Refused -> when (pollNext(r.code)) {
                        PollNext.WAIT -> {}
                        PollNext.SLOW -> delay(30_000)
                        PollNext.RENEW -> renew = true
                        PollNext.STOP -> {
                            step = if (r.code == AccountCode.DENIED) CodeStep.Denied else CodeStep.Failed(r.message)
                            return@LaunchedEffect
                        }
                    }
                }
                // Échu à l'horloge de l'appareil : un nouveau code sans attendre le relais.
                if (Accounts.tvMinutesLeft(code) <= 0) renew = true
            }
        }
    }
    AccountCodeContent(step, Accounts.relayLabel(relay), onOtherRelay = onOtherRelay, onBack = onBack, onNewCode = { attempt++ })
}

/**
 * L'écran du code (maquette, rangée « Apps Link ») : le titre, les trois étapes, le code en chasse fixe, le QR code de la
 * page avec le code, « En attente de votre téléphone… Le code change dans {n} min. », « Relais : {relais} », « Un autre
 * relais… » et « Retour ». Refusé : « Connexion refusée sur le téléphone. » et « Nouveau code ». Le focus de la
 * télécommande est sur le premier bouton.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AccountCodeContent(step: CodeStep, relayLabel: String, onOtherRelay: () -> Unit, onBack: () -> Unit, onNewCode: () -> Unit) {
    val s = LocalScale.current
    val first = remember { FocusRequester() }
    val again = step is CodeStep.Denied || step is CodeStep.Failed
    LaunchedEffect(again) { runCatching { first.requestFocus() } }
    val code = (step as? CodeStep.Showing)?.code
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        BoxWithConstraints(Modifier.fillMaxSize()) {
            val wide = maxWidth > maxHeight
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(horizontal = s.dp(if (s.tv) 56f else 20f), vertical = s.dp(if (s.tv) 32f else 20f)),
                verticalArrangement = Arrangement.spacedBy(s.dp(16f)),
            ) {
                Title(stringResource(R.string.account_sign_in), size = if (s.tv) 36f else 26f)
                val text: @Composable () -> Unit = {
                    Column(verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
                        Steps(code?.urlLabel ?: "${relayLabel}/tv")
                        when (step) {
                            CodeStep.Asking -> Waiting(null)
                            is CodeStep.Showing -> {
                                Text(
                                    step.code.code,
                                    style = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(if (s.tv) 54f else 40f), letterSpacing = s.sp(4f), color = Kn.frost),
                                )
                                Waiting(step.minutes)
                            }
                            CodeStep.Denied -> Body(stringResource(R.string.account_code_denied), color = Kn.warn, size = 16f)
                            is CodeStep.Failed -> Body(step.message.ifBlank { stringResource(R.string.add_unreachable) }, color = Kn.warn, size = 16f)
                        }
                        Mono(stringResource(R.string.account_code_relay, relayLabel), size = 13f, color = Kn.frost2)
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                            if (again) PrimaryButton(stringResource(R.string.account_code_new), onNewCode, Modifier.focusRequester(first))
                            GlassButton(stringResource(R.string.account_other_relay), onOtherRelay, if (again) Modifier else Modifier.focusRequester(first))
                            GlassButton(stringResource(R.string.account_back), onBack)
                        }
                    }
                }
                val qr: @Composable () -> Unit = {
                    if (code != null) QrCode(code.urlComplete, if (s.tv) 220f else 200f, stringResource(R.string.account_code_qr))
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

/** « Scannez le QR code avec votre téléphone, », « ou ouvrez {adresse} et entrez le code, », « puis touchez « Connecter ». ». */
@Composable
private fun Steps(address: String) {
    val s = LocalScale.current
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        listOf(
            stringResource(R.string.account_code_step_scan),
            stringResource(R.string.account_code_step_open, address),
            stringResource(R.string.account_code_step_connect),
        ).forEachIndexed { i, line ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(Modifier.size(s.dp(26f)).clip(CircleShape).border(BorderStroke(1.dp, Kn.spectre), CircleShape), contentAlignment = Alignment.Center) {
                    Mono("${i + 1}", size = 12f, color = Kn.spectre)
                }
                Spacer(Modifier.width(s.dp(12f)))
                Body(line, size = if (s.tv) 18f else 16f, color = Kn.frost)
            }
        }
    }
}

/** L'indicateur et « En attente de votre téléphone… Le code change dans {n} min. » ([minutes] nul : le code n'est pas encore là). */
@Composable
private fun Waiting(minutes: Int?) {
    val s = LocalScale.current
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
        CircularProgressIndicator(Modifier.size(s.dp(22f)), color = Kn.spectre, strokeWidth = s.dp(3f))
        if (minutes != null) Body(stringResource(R.string.account_code_waiting, minutes.coerceAtLeast(1)), size = 15f, color = Kn.frost2)
    }
}

// MARK: - Le relais

/**
 * « Ajouter un compte » : le relais d'abord, `share.kybernow.app` proposé, ou « Un autre relais… » (l'adresse tapée,
 * vérifiée par le cœur : « Adresse du relais illisible. »). [onChosen] reçoit la forme normale du relais, puis la
 * connexion suit (onglet personnalisé, ou le code au téléviseur). [other] : le champ ouvert d'emblée.
 */
@Composable
fun AccountRelayScreen(other: Boolean, onChosen: (String) -> Unit, onBack: () -> Unit) {
    var typing by remember { mutableStateOf(other) }
    var error by remember { mutableStateOf<String?>(null) }
    val unreadable = stringResource(R.string.account_bad_relay)
    AccountRelayContent(
        typing = typing, error = error,
        onDefault = { onChosen(Accounts.DEFAULT_RELAY) },
        onOther = { typing = true; error = null },
        onContinue = { text ->
            when (val r = runCatching { Accounts.normalizeRelay(text) }.getOrElse { AccountReply.Refused(AccountCode.BAD_RELAY, "bad_relay", "") }) {
                is AccountReply.Ok -> onChosen(r.value)
                is AccountReply.Refused -> error = r.message.ifBlank { unreadable }
            }
        },
        // « Annuler » dans le champ : retour au choix, sauf s'il était ouvert d'emblée (on revient d'où l'on vient).
        onCancel = { if (other) onBack() else { typing = false; error = null } },
        onBack = onBack,
    )
}

/**
 * Le choix du relais : titre « Ajouter un compte », `share.kybernow.app` et « Un autre relais… » ; [typing] : « Un autre
 * relais », le champ « Relais » (exemple `relais.exemple.org:9443`), son aide, « Continuer », « Annuler », et le refus
 * sous le champ ([error]).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AccountRelayContent(typing: Boolean, error: String?, onDefault: () -> Unit, onOther: () -> Unit, onContinue: (String) -> Unit, onCancel: () -> Unit, onBack: () -> Unit) {
    val s = LocalScale.current
    var text by remember { mutableStateOf("") }
    val first = remember { FocusRequester() }
    LaunchedEffect(typing) { if (s.tv) runCatching { first.requestFocus() } }
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(s.dp(if (s.tv) 56f else 20f)).widthIn(max = s.dp(760f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(16f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                FocusCard(onClick = onBack) {
                    Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(10f))) { KnIcon(KnIcons.back, size = 20f) }
                }
                Spacer(Modifier.width(s.dp(14f)))
                Title(stringResource(if (typing) R.string.account_relay_title else R.string.account_add), size = if (s.tv) 36f else 26f)
            }
            if (!typing) {
                FocusCard(onDefault, Modifier.fillMaxWidth().focusRequester(first)) {
                    Row(Modifier.panel().padding(s.dp(16f)).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                        KnIcon(KnIcons.network, tint = Kn.spectre)
                        Spacer(Modifier.width(s.dp(12f)))
                        Mono(Accounts.DEFAULT_RELAY, Modifier.weight(1f), size = 15f, color = Kn.frost)
                        KnIcon(KnIcons.chevron, size = 18f)
                    }
                }
                GlassButton(stringResource(R.string.account_other_relay), onOther)
                return@Column
            }
            OutlinedTextField(
                value = text,
                onValueChange = { text = it.take(300) },
                label = { Text(stringResource(R.string.account_relay_field)) },
                placeholder = { Text("relais.exemple.org:9443", fontFamily = KnFonts.mono, color = Kn.muted) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().focusRequester(first),
                textStyle = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(18f), color = Kn.frost),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                colors = fieldColors,
            )
            Body(stringResource(R.string.account_relay_help), size = 13f, color = Kn.muted)
            error?.let { Body(it, color = Kn.warn, size = 14f) }
            FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                PrimaryButton(stringResource(R.string.add_continue), { onContinue(text) }, enabled = text.isNotBlank())
                GlassButton(stringResource(R.string.cancel), onCancel)
            }
        }
    }
}

// MARK: - Réglages de l'app › Comptes

/**
 * Les comptes dans les réglages de l'app : chacun avec son nom, son e-mail, son relais, « Gérer le compte » (la page du
 * compte dans le navigateur) et « Se déconnecter » (confirmé : « Se déconnecter de {compte} ? Ses ordinateurs quittent
 * cet appareil ; ceux que vous avez appairés restent. ») ; sans compte, « Aucun compte. » et « Se connecter à un compte
 * KyberNow » ; avec, « Ajouter un compte » ; puis l'aide (créer, changer le mot de passe, supprimer : sur la page du compte).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AccountsRows(
    accounts: List<AccountSummary>, signingIn: Boolean,
    onManage: (AccountSummary) -> Unit, onSignOut: (AccountSummary) -> Unit, onSignIn: () -> Unit, onAdd: () -> Unit,
) {
    val s = LocalScale.current
    if (accounts.isEmpty()) {
        Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
            Body(stringResource(R.string.account_none), size = 16f, color = Kn.frost2)
            PrimaryButton(stringResource(if (signingIn) R.string.account_signing_in else R.string.account_sign_in), { if (!signingIn) onSignIn() })
        }
    }
    accounts.forEach { a -> AccountRow(a, { onManage(a) }, { onSignOut(a) }) }
    if (accounts.isNotEmpty()) {
        if (signingIn) Body(stringResource(R.string.account_signing_in), size = 14f, color = Kn.frost2)
        GlassButton(stringResource(R.string.account_add), onAdd, leading = { KnIcon(KnIcons.plus, size = 16f); Spacer(Modifier.width(6.dp)) })
    }
    Body(stringResource(R.string.account_section_help), size = 13f, color = Kn.muted)
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AccountRow(a: AccountSummary, onManage: () -> Unit, onSignOut: () -> Unit) {
    val s = LocalScale.current
    var asking by remember(a.key) { mutableStateOf(false) }
    val confirm = remember { FocusRequester() }
    LaunchedEffect(asking) { if (asking && s.tv) runCatching { confirm.requestFocus() } }
    Column(
        Modifier.panel().then(if (asking) Modifier.border(BorderStroke(1.dp, Kn.warn.copy(alpha = 0.5f)), RoundedCornerShape(18.dp)) else Modifier).fillMaxWidth().padding(s.dp(16f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(8f)),
    ) {
        Body(a.displayName, size = 17f, weight = 600, maxLines = 1)
        if (a.name.isNotBlank() && a.email.isNotBlank()) Body(a.email, size = 14f, color = Kn.frost2, maxLines = 1)
        Mono(a.relayLabel, size = 12f, color = Kn.muted, maxLines = 1)
        if (!asking) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                GlassButton(stringResource(R.string.account_manage), onManage)
                GlassButton(stringResource(R.string.account_sign_out), { asking = true })
            }
        } else {
            Body(stringResource(R.string.account_sign_out_confirm, a.displayName), size = 15f)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                GlassButton(stringResource(R.string.account_sign_out), { asking = false; onSignOut() }, Modifier.focusRequester(confirm))
                GlassButton(stringResource(R.string.cancel), { asking = false })
            }
        }
    }
}
