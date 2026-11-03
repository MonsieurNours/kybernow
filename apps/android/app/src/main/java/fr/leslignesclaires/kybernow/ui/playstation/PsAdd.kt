package fr.leslignesclaires.kybernow.ui.playstation

import android.annotation.SuppressLint
import android.content.pm.PackageManager
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.viewinterop.AndroidView
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.core.PsNative
import fr.leslignesclaires.kybernow.playstation.Handoff
import fr.leslignesclaires.kybernow.playstation.PairError
import fr.leslignesclaires.kybernow.playstation.PlayStations
import fr.leslignesclaires.kybernow.playstation.PsAccount
import fr.leslignesclaires.kybernow.playstation.PsConsole
import fr.leslignesclaires.kybernow.playstation.PsFound
import fr.leslignesclaires.kybernow.playstation.PsStatus
import fr.leslignesclaires.kybernow.playstation.Waiting
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalForm
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.panel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

/** Les étapes de l'onglet PlayStation (docs/link-ui.md, « Consoles PlayStation », « Association »). */
sealed interface PsStep {
    /** L'onglet lui-même : comptes, envoi, réception, consoles du réseau. */
    data object Tab : PsStep
    /** Deux comptes ou plus pas encore associés à cette console : « Avec quel compte ? ». */
    data class ChooseAccount(val found: PsFound) : PsStep
    /** Lire un compte : connexion Sony, ou à la main ; [found] : la console à associer ensuite (nul : un compte seul). */
    data class Account(val found: PsFound?, val notRead: Boolean = false) : PsStep
    data class SignIn(val found: PsFound?) : PsStep
    data class ByHand(val found: PsFound?) : PsStep
    data class Code(val found: PsFound, val account: PsAccount) : PsStep
    data class Send(val to: Waiting) : PsStep
}

/** Le nom d'un compte : son pseudo, sinon « Compte PlayStation » (compte d'une version précédente). */
@Composable
fun accountName(a: PsAccount) = a.name ?: stringResource(R.string.ps_legacy_account)

@Composable
fun psStatusLabel(st: PsStatus) = stringResource(
    when (st) {
        PsStatus.SEARCHING -> R.string.home_searching
        PsStatus.ON -> R.string.ps_on
        PsStatus.STANDBY -> R.string.ps_standby
        PsStatus.OFFLINE -> R.string.home_offline
        PsStatus.AWAY -> R.string.home_away
    },
)

/** L'état d'une console trouvée, pour la ligne « Consoles sur ce réseau ». */
@Composable
fun foundStateLabel(f: PsFound) = stringResource(if (f.ready) R.string.ps_on else R.string.ps_standby)

/** « Mac », « Apple TV », « Android TV »… (le `platform` des recherches ; inconnu : tel quel). */
fun platformLabel(p: String) = when (p) {
    "macos" -> "Mac"
    "ios" -> "iPhone"
    "ipados" -> "iPad"
    "tvos" -> "Apple TV"
    "windows" -> "PC"
    "linux" -> "Linux"
    "steamdeck" -> "Steam Deck"
    "raspberrypi" -> "Raspberry Pi"
    "android" -> "Android"
    "androidtv" -> "Android TV"
    "webos" -> "LG"
    else -> p
}

/** Une WebView peut s'ouvrir (pas toujours au téléviseur : la connexion Sony n'y est pas proposée). */
@Composable
private fun canSignIn(): Boolean {
    val context = LocalContext.current
    val tv = LocalForm.current == FormFactor.TV
    return remember { !tv && runCatching { context.packageManager.hasSystemFeature(PackageManager.FEATURE_WEBVIEW) }.getOrDefault(false) }
}

@Composable
private fun PsField(label: String, value: String, onChange: (String) -> Unit, number: Boolean = false, help: String? = null) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        supportingText = help?.let { { Text(it) } },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        keyboardOptions = KeyboardOptions(keyboardType = if (number) KeyboardType.NumberPassword else KeyboardType.Text),
        colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, focusedLabelColor = Kn.spectre, cursorColor = Kn.spectre),
    )
}

/**
 * L'onglet PlayStation et ses étapes. [onPaired] : la console associée (l'accueil la choisit) ; [step] et [setStep] : là
 * où en est l'onglet (le bouton Retour de l'écran d'ajout y revient).
 */
@Composable
fun PlayStationPane(step: PsStep, setStep: (PsStep) -> Unit, onPaired: (PsConsole) -> Unit) {
    val ps = KyberNowApp.instance.ps
    DisposableEffect(Unit) {
        ps.start()
        onDispose { ps.stop() }
    }
    when (step) {
        PsStep.Tab -> PsTab(ps, setStep)
        is PsStep.ChooseAccount -> ChooseAccount(ps, step.found, setStep)
        is PsStep.Account -> AccountStep(step, setStep)
        is PsStep.SignIn -> SignInStep(step, setStep)
        is PsStep.ByHand -> ByHandStep(step, setStep)
        is PsStep.Code -> CodeStep(ps, step, onPaired)
        is PsStep.Send -> SendStep(ps, step.to)
    }
}

/** La première étape d'une association : le compte (choisi, sauté s'il n'y en a qu'un, lu s'il n'y en a pas). */
fun startPairing(ps: PlayStations, f: PsFound): PsStep {
    val candidates = ps.store.unpairedAccounts(f.hostId)
    return when {
        candidates.size >= 2 -> PsStep.ChooseAccount(f)
        candidates.size == 1 -> PsStep.Code(f, candidates[0])
        else -> PsStep.Account(f)
    }
}

/** Après un compte lu : l'association continue, ou l'onglet revient. */
private fun afterAccount(found: PsFound?, a: PsAccount): PsStep = if (found != null) PsStep.Code(found, a) else PsStep.Tab

@Composable
private fun PsTab(ps: PlayStations, setStep: (PsStep) -> Unit) {
    val s = LocalScale.current
    val tv = LocalForm.current == FormFactor.TV
    val accounts by ps.store.accounts.collectAsState()
    val consoles by ps.store.consoles.collectAsState()
    val pairings by ps.store.pairings.collectAsState()
    val found by ps.found.collectAsState()
    var renaming by remember { mutableStateOf<PsAccount?>(null) }
    var removing by remember { mutableStateOf<PsAccount?>(null) }

    // Une télé sans compte : recevoir d'abord (pas de navigateur).
    if (tv && accounts.isEmpty()) ReceiveSection(ps, autoStart = true)

    Eyebrow(stringResource(R.string.ps_accounts_title))
    if (accounts.isEmpty()) Body(stringResource(R.string.ps_no_account), color = Kn.frost2, size = 15f)
    accounts.forEach { a ->
        val n = consoles.count { a.id in pairings[it.id].orEmpty() }
        Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Body(accountName(a), size = 17f, weight = 600, maxLines = 1)
                Mono(
                    when (n) {
                        0 -> stringResource(R.string.ps_no_console_of_account)
                        1 -> stringResource(R.string.ps_one_console)
                        else -> stringResource(R.string.ps_n_consoles, n)
                    },
                    size = 11f,
                )
            }
            GlassButton(stringResource(R.string.ps_rename), { renaming = a })
            Spacer(Modifier.width(s.dp(8f)))
            GlassButton(stringResource(R.string.ps_remove), { removing = a })
        }
    }
    GlassButton(stringResource(R.string.ps_add_account), { setStep(PsStep.Account(null)) }, leading = { KnIcon(KnIcons.plus, size = 16f); Spacer(Modifier.width(s.dp(6f))) })

    // Envoyer : seulement avec des comptes à envoyer (téléphone, tablette ; la télé reçoit).
    if (accounts.isNotEmpty() && !tv) SendSection(setStep)
    if (!(tv && accounts.isEmpty())) ReceiveSection(ps, autoStart = false)

    Eyebrow(stringResource(R.string.ps_consoles_title))
    val list = found.values.sortedBy { it.name }
    if (list.isEmpty()) Body(stringResource(R.string.ps_no_console_found), color = Kn.frost2, size = 15f)
    list.forEach { f ->
        val c = consoles.firstOrNull { it.hostId == f.hostId }
        val paired = c?.let { ps.store.pairedAccounts(it) }.orEmpty()
        val others = ps.store.unpairedAccounts(f.hostId)
        Column(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Body(f.name, size = 17f, weight = 600, maxLines = 1)
                    Mono(stringResource(R.string.ps_model_line, f.model, foundStateLabel(f)), size = 11f)
                }
                if (paired.isEmpty()) PrimaryButton(stringResource(R.string.ps_pair), { setStep(startPairing(ps, f)) })
            }
            if (paired.isNotEmpty()) {
                Body(stringResource(R.string.ps_paired_with, paired.map { it.name ?: "" }.filter { it.isNotEmpty() }.ifEmpty { listOf("—") }.joinToString(", ")), color = Kn.frost2, size = 14f)
                if (others.isNotEmpty()) GlassButton(stringResource(R.string.ps_pair_other), { setStep(startPairing(ps, f)) })
            }
        }
    }

    renaming?.let { a ->
        var name by remember(a.id) { mutableStateOf(a.name.orEmpty()) }
        AlertDialog(
            onDismissRequest = { renaming = null },
            title = { Text(stringResource(R.string.ps_rename)) },
            text = { PsField(stringResource(R.string.ps_account_name), name, { name = it }, help = stringResource(R.string.ps_account_name_help)) },
            confirmButton = { TextButton({ ps.store.rename(a.id, name); renaming = null }, enabled = name.isNotBlank()) { Text(stringResource(R.string.ps_rename)) } },
            dismissButton = { TextButton({ renaming = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    removing?.let { a ->
        val n = ps.store.consolesOf(a.id).size
        val name = accountName(a)
        AlertDialog(
            onDismissRequest = { removing = null },
            title = {
                Text(
                    when (n) {
                        0 -> stringResource(R.string.ps_remove_account_none, name)
                        1 -> stringResource(R.string.ps_remove_account_one, name)
                        else -> stringResource(R.string.ps_remove_account_many, name, n)
                    },
                )
            },
            text = { Text(stringResource(R.string.ps_remove_account_body)) },
            confirmButton = { TextButton({ ps.store.removeAccount(a.id); removing = null }) { Text(stringResource(R.string.ps_remove)) } },
            dismissButton = { TextButton({ removing = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@Composable
private fun ChooseAccount(ps: PlayStations, f: PsFound, setStep: (PsStep) -> Unit) {
    val s = LocalScale.current
    Title(stringResource(R.string.ps_which_account), size = 22f)
    ps.store.unpairedAccounts(f.hostId).forEach { a ->
        FocusCard({ setStep(PsStep.Code(f, a)) }, Modifier.fillMaxWidth()) {
            Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
                Body(accountName(a), size = 17f, weight = 600, modifier = Modifier.weight(1f))
                KnIcon(KnIcons.chevron, size = 18f)
            }
        }
    }
    GlassButton(stringResource(R.string.ps_add_account), { setStep(PsStep.Account(f)) })
}

@Composable
private fun AccountStep(step: PsStep.Account, setStep: (PsStep) -> Unit) {
    val tv = LocalForm.current == FormFactor.TV
    val signIn = canSignIn()
    Body(stringResource(R.string.ps_account_intro), color = Kn.frost2, size = 15f)
    if (step.notRead) Body(stringResource(R.string.ps_id_not_read), color = Kn.warn, size = 14f)
    // La télé (sans navigateur) : recevoir d'un autre appareil, ou l'identifiant à la main.
    if (signIn) PrimaryButton(stringResource(R.string.ps_sign_in), { setStep(PsStep.SignIn(step.found)) })
    if (tv || !signIn) ReceiveSection(KyberNowApp.instance.ps, autoStart = false)
    GlassButton(stringResource(R.string.ps_by_hand), { setStep(PsStep.ByHand(step.found)) })
}

/**
 * La page de connexion de Sony dans une WebView : à l'adresse de retour, la page s'arrête et le cœur lit l'identifiant
 * et le pseudo du compte (une fois). Les cookies de la page sont effacés avant et après : rien ne reste de la connexion.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun SignInStep(step: PsStep.SignIn, setStep: (PsStep) -> Unit) {
    val s = LocalScale.current
    val ps = KyberNowApp.instance.ps
    val scope = rememberCoroutineScope()
    var reading by remember { mutableStateOf(false) }
    var exists by remember { mutableStateOf(false) }
    Title(stringResource(R.string.ps_sign_in_title), size = 22f)
    if (exists) Body(stringResource(R.string.ps_account_exists), color = Kn.frost2, size = 14f)
    if (reading) {
        androidx.compose.material3.LinearProgressIndicator(Modifier.fillMaxWidth(), color = Kn.spectre)
        return
    }
    DisposableEffect(Unit) {
        CookieManager.getInstance().removeAllCookies(null)
        onDispose { CookieManager.getInstance().removeAllCookies(null) }
    }
    Box(Modifier.fillMaxWidth().height(s.dp(560f)).background(Kn.frost)) {
        AndroidView(
            factory = { ctx ->
                WebView(ctx).apply {
                    layoutParams = android.view.ViewGroup.LayoutParams(android.view.ViewGroup.LayoutParams.MATCH_PARENT, android.view.ViewGroup.LayoutParams.MATCH_PARENT)
                    settings.javaScriptEnabled = true
                    settings.domStorageEnabled = true
                    webViewClient = object : WebViewClient() {
                        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                            val url = request.url.toString()
                            if (!PsNative.isRedirect(url)) return false
                            reading = true
                            scope.launch {
                                val o = withContext(Dispatchers.IO) { JSONObject(PsNative.account(url)) }
                                val id = o.optString("account").takeIf { !o.isNull("account") && it.isNotEmpty() }
                                if (id == null) {
                                    setStep(PsStep.Account(step.found, notRead = true))
                                    return@launch
                                }
                                val name = o.optString("online_id").takeIf { !o.isNull("online_id") && it.isNotEmpty() }
                                val kept = ps.store.keep(id, name)
                                exists = kept.existed
                                if (kept.existed && step.found == null) {
                                    reading = false
                                    delay(1500)
                                }
                                setStep(afterAccount(step.found, kept.account))
                            }
                            return true
                        }
                    }
                    loadUrl(PsNative.loginUrl())
                }
            },
            modifier = Modifier.fillMaxSize(),
        )
    }
}

@Composable
private fun ByHandStep(step: PsStep.ByHand, setStep: (PsStep) -> Unit) {
    val ps = KyberNowApp.instance.ps
    var id by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var exists by remember { mutableStateOf(false) }
    val valid = id.isNotBlank() && PsNative.accountIdValid(id.trim())
    PsField(stringResource(R.string.ps_account_id), id, { id = it.trim(); exists = false })
    PsField(stringResource(R.string.ps_account_name), name, { name = it }, help = stringResource(R.string.ps_account_name_help))
    if (exists) Body(stringResource(R.string.ps_account_exists), color = Kn.frost2, size = 14f)
    PrimaryButton(stringResource(R.string.ps_continue), {
        val kept = ps.store.keep(id.trim(), name.trim().takeIf { it.isNotEmpty() })
        exists = kept.existed
        if (!kept.existed || step.found != null) setStep(afterAccount(step.found, kept.account))
    }, enabled = valid && name.isNotBlank())
}

@Composable
private fun CodeStep(ps: PlayStations, step: PsStep.Code, onPaired: (PsConsole) -> Unit) {
    val scope = rememberCoroutineScope()
    var code by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<PairError?>(null) }
    Title(step.found.name, size = 22f)
    Mono(stringResource(R.string.ps_account_line, accountName(step.account)), size = 12f)
    Body(stringResource(R.string.ps_code_help), color = Kn.frost2, size = 15f)
    PsField(stringResource(R.string.ps_code_field), code, { code = it.filter(Char::isDigit).take(8); error = null }, number = true)
    error?.let {
        Body(
            stringResource(
                when (it) {
                    PairError.NO_ANSWER -> R.string.ps_no_answer
                    PairError.WRONG_CODE -> R.string.ps_wrong_code
                    PairError.ACCOUNT -> R.string.ps_account_refused
                    PairError.UNREACHABLE -> R.string.ps_console_unreachable
                },
            ),
            color = Kn.warn, size = 14f,
        )
    }
    PrimaryButton(stringResource(if (busy) R.string.add_contacting else R.string.ps_pair), {
        busy = true
        scope.launch {
            when (val r = ps.pair(step.found, step.account, code.toInt())) {
                is PlayStations.PairResult.Ok -> onPaired(r.console)
                is PlayStations.PairResult.Failed -> error = r.error
            }
            busy = false
        }
    }, enabled = !busy && code.length == 8)
}

// MARK: - Plusieurs appareils

/** « Envoyer à un appareil » : les appareils du réseau qui attendent un envoi, cherchés toutes les 3 s. */
@Composable
private fun SendSection(setStep: (PsStep) -> Unit) {
    val s = LocalScale.current
    var waiting by remember { mutableStateOf(emptyList<Waiting>()) }
    LaunchedEffect(Unit) {
        while (isActive) {
            waiting = withContext(Dispatchers.IO) { Handoff.parseWaiting(PsNative.handoffDiscover(1200)) }
            delay(2000)
        }
    }
    Eyebrow(stringResource(R.string.ps_send_title))
    if (waiting.isEmpty()) Body(stringResource(R.string.ps_send_none), color = Kn.frost2, size = 14f)
    waiting.forEach { w ->
        FocusCard({ setStep(PsStep.Send(w)) }, Modifier.fillMaxWidth()) {
            Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
                Body(stringResource(R.string.ps_model_line, w.name, platformLabel(w.platform)), size = 16f, weight = 600, modifier = Modifier.weight(1f))
                KnIcon(KnIcons.chevron, size = 18f)
            }
        }
    }
}

@Composable
private fun SendStep(ps: PlayStations, to: Waiting) {
    val app = KyberNowApp.instance
    val scope = rememberCoroutineScope()
    var code by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var result by remember { mutableStateOf<Pair<Boolean, Int>?>(null) }
    Body(stringResource(R.string.ps_send_code_help, to.name), color = Kn.frost2, size = 15f)
    PsField(stringResource(R.string.ps_send_code_field), code, { code = it.filter(Char::isDigit).take(6); result = null }, number = true)
    result?.let { (ok, text) -> Body(stringResource(text, to.name), color = if (ok) Kn.spectre else Kn.warn, size = 14f) }
    PrimaryButton(stringResource(if (busy) R.string.add_contacting else R.string.ps_send), {
        busy = true
        scope.launch {
            val bundle = Handoff.bundle(ps.store, app.hosts.hosts.value, app.device.name)
            val o = withContext(Dispatchers.IO) { JSONObject(PsNative.handoffSend(to.address, code, bundle)) }
            result = when {
                o.optBoolean("ok") -> true to R.string.ps_sent
                o.optString("error") == "refused" -> false to R.string.ps_send_refused
                else -> false to R.string.ps_send_unreachable
            }
            busy = false
        }
    }, enabled = !busy && code.length == 6 && result?.first != true)
}

/**
 * « Recevoir d'un autre appareil » : un code en grand, l'attente de l'envoi (5 min), puis « {n} compte(s) et {n}
 * console(s) reçus. ». [autoStart] : l'attente commence tout de suite (une télé sans compte).
 */
@Composable
fun ReceiveSection(ps: PlayStations, autoStart: Boolean) {
    val s = LocalScale.current
    val app = KyberNowApp.instance
    val context = LocalContext.current
    var on by remember { mutableStateOf(autoStart) }
    var round by remember { mutableStateOf(0) }
    var code by remember { mutableStateOf<String?>(null) }
    var expired by remember { mutableStateOf(false) }
    var received by remember { mutableStateOf<Handoff.Received?>(null) }
    Eyebrow(stringResource(R.string.ps_receive_title))
    if (!on) {
        GlassButton(stringResource(R.string.ps_receive_title), { on = true; received = null })
        received?.let { ReceivedLine(it) }
        return
    }
    DisposableEffect(round) {
        val wifi = context.applicationContext.getSystemService(android.content.Context.WIFI_SERVICE) as? android.net.wifi.WifiManager
        // Les recherches arrivent en diffusion (port 9340) : le Wi-Fi ne les laisse passer qu'avec ce verrou.
        val lock = wifi?.createMulticastLock("kybernow-handoff")?.apply { setReferenceCounted(false); acquire() }
        val platform = if (app.form == FormFactor.TV) "androidtv" else "android"
        val h = PsNative.handoffReceive(app.device.name, platform)
        code = if (h != 0L) PsNative.handoffCode(h) else null
        expired = false
        val job = kotlinx.coroutines.MainScope().launch {
            while (isActive && h != 0L) {
                val e = withContext(Dispatchers.IO) { PsNative.handoffNext(h, 1000) } ?: continue
                val o = JSONObject(e)
                when (o.optString("event")) {
                    "received" -> {
                        received = Handoff.merge(ps.store, app.hosts.hosts.value, o.optString("bundle"))
                        on = false
                        break
                    }
                    "new_code" -> code = o.optString("code")
                    "expired" -> { expired = true; break }
                }
            }
        }
        onDispose {
            job.cancel()
            if (h != 0L) PsNative.handoffStop(h)
            runCatching { lock?.release() }
        }
    }
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(18f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        if (expired) {
            Body(stringResource(R.string.ps_code_expired), color = Kn.warn, size = 15f)
            GlassButton(stringResource(R.string.ps_new_code), { round++ })
        } else {
            Title(code?.let { if (it.length == 6) it.substring(0, 3) + " " + it.substring(3) else it } ?: "…", size = if (s.tv) 56f else 44f)
            Body(stringResource(R.string.ps_receive_help), color = Kn.frost2, size = 14f)
        }
    }
}

@Composable
private fun ReceivedLine(r: Handoff.Received) {
    val a = if (r.accounts == 1) stringResource(R.string.ps_received_accounts_one) else stringResource(R.string.ps_received_accounts_many, r.accounts)
    val c = if (r.consoles == 1) stringResource(R.string.ps_received_consoles_one) else stringResource(R.string.ps_received_consoles_many, r.consoles)
    Body(stringResource(R.string.ps_received, a, c), color = Kn.spectre, size = 15f)
}

/** Retour depuis une étape : l'étape d'avant, ou l'onglet ; faux sur l'onglet lui-même. */
fun psStepBack(step: PsStep, setStep: (PsStep) -> Unit): Boolean = when (step) {
    PsStep.Tab -> false
    is PsStep.SignIn -> { setStep(PsStep.Account(step.found)); true }
    is PsStep.ByHand -> { setStep(PsStep.Account(step.found)); true }
    else -> { setStep(PsStep.Tab); true }
}
