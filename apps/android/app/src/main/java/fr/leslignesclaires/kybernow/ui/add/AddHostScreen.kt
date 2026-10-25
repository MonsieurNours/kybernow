package fr.leslignesclaires.kybernow.ui.add

import android.Manifest
import android.content.pm.PackageManager
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.core.content.ContextCompat
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.ConsoleClient
import fr.leslignesclaires.kybernow.core.PairState
import fr.leslignesclaires.kybernow.core.Reply
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
import fr.leslignesclaires.kybernow.ui.FocusCard
import fr.leslignesclaires.kybernow.ui.GlassButton
import fr.leslignesclaires.kybernow.ui.Kn
import fr.leslignesclaires.kybernow.ui.KnIcon
import fr.leslignesclaires.kybernow.ui.KnIcons
import fr.leslignesclaires.kybernow.ui.LocalScale
import fr.leslignesclaires.kybernow.ui.Mono
import fr.leslignesclaires.kybernow.ui.PrimaryButton
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.panel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.random.Random

/** Un ordinateur à joindre : adresse et port d'Admin, nom, empreinte attendue. */
data class Target(val address: String, val port: Int, val name: String, val fingerprint: String?)

/** Lien d'appairage d'Admin : `kybernow://adresse:port?name=…&fp=sha256:…` (aucun secret). */
object PairingLink {
    fun parse(text: String): Target? {
        val uri = runCatching { Uri.parse(text.trim()) }.getOrNull() ?: return null
        if (uri.scheme != "kybernow") return null
        val host = uri.host ?: return null
        val port = if (uri.port > 0) uri.port else return null
        return Target(host, port, uri.getQueryParameter("name") ?: host, uri.getQueryParameter("fp"))
    }
}

/** Les quatre premiers groupes d'une empreinte, à comparer à ceux d'Admin. */
fun shortFingerprint(fp: String): String = fp.removePrefix("sha256:").split(':').take(4).joinToString(":").uppercase()

private enum class Tab { NETWORK, CODE, ADDRESS, QR }

private sealed class Step {
    data object Choose : Step()
    /** Premier contact par adresse : l'empreinte vue, à confirmer. */
    data class Verify(val target: Target, val seen: String) : Step()
    data class Account(val target: Target) : Step()
    data class Code(val targets: List<Target>) : Step()
}

@Composable
fun AddHostScreen(initial: Target?, onDone: (Host) -> Unit, onBack: () -> Unit) {
    val s = LocalScale.current
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val announced by app.discovery.found.collectAsState()
    val hasCamera = remember { context.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY) }
    var tab by remember { mutableStateOf(if (s.tv) Tab.CODE else Tab.NETWORK) }
    var step by remember { mutableStateOf<Step>(if (initial != null) Step.Account(initial) else Step.Choose) }
    LaunchedEffect(Unit) { app.discovery.start() }

    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(s.dp(if (s.tv) 56f else 20f)).widthIn(max = s.dp(760f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(16f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                FocusCard(onClick = { if (step is Step.Choose) onBack() else step = Step.Choose }) {
                    Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(10f))) { KnIcon(KnIcons.back, size = 20f) }
                }
                Spacer(Modifier.width(s.dp(14f)))
                Title(stringResource(R.string.add_title), size = if (s.tv) 40f else 26f)
            }
            when (val st = step) {
                Step.Choose -> {
                    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                        GlassButton(stringResource(R.string.add_tab_network), { tab = Tab.NETWORK }, selected = tab == Tab.NETWORK)
                        GlassButton(stringResource(R.string.add_tab_code), { tab = Tab.CODE }, selected = tab == Tab.CODE)
                        GlassButton(stringResource(R.string.add_tab_address), { tab = Tab.ADDRESS }, selected = tab == Tab.ADDRESS)
                        if (hasCamera && !s.tv) GlassButton(stringResource(R.string.add_tab_qr), { tab = Tab.QR }, selected = tab == Tab.QR)
                    }
                    when (tab) {
                        Tab.NETWORK -> NetworkPane(announced) { a -> step = Step.Account(Target(a.address, a.port, a.name, a.fingerprint)) }
                        Tab.CODE -> CodeIntro(announced) { targets -> step = Step.Code(targets) }
                        Tab.ADDRESS -> AddressPane(
                            onSeen = { t, fp -> step = Step.Verify(t, fp) },
                        )
                        Tab.QR -> QrPane { t -> step = Step.Account(t) }
                    }
                }
                is Step.Verify -> VerifyPane(st.target, st.seen, onAccept = { step = Step.Account(st.target.copy(fingerprint = st.seen)) }, onCancel = { step = Step.Choose })
                is Step.Account -> AccountPane(st.target, onDone, onCode = { step = Step.Code(listOf(st.target)) })
                is Step.Code -> CodePane(st.targets, onDone)
            }
        }
    }
}

@Composable
private fun NetworkPane(announced: List<Announced>, onPick: (Announced) -> Unit) {
    val s = LocalScale.current
    Body(stringResource(R.string.add_network_help), color = Kn.frost2, size = 15f)
    if (announced.isEmpty()) {
        Row(Modifier.panel().fillMaxWidth().padding(s.dp(18f)), verticalAlignment = Alignment.CenterVertically) {
            KnIcon(KnIcons.network, tint = Kn.muted)
            Spacer(Modifier.width(s.dp(12f)))
            Body(stringResource(R.string.add_network_searching), color = Kn.frost2, size = 15f)
        }
    }
    announced.forEach { a ->
        FocusCard({ onPick(a) }, Modifier.fillMaxWidth()) {
            Row(Modifier.panel().fillMaxWidth().padding(s.dp(16f)), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Body(a.name, size = 17f, weight = 600)
                    Mono("${a.address}:${a.port}", size = 11f)
                }
                KnIcon(KnIcons.chevron, size = 18f)
            }
        }
    }
}

@Composable
private fun Field(label: String, value: String, onChange: (String) -> Unit, password: Boolean = false, number: Boolean = false) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        visualTransformation = if (password) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = if (password) KeyboardType.Password else if (number) KeyboardType.Number else KeyboardType.Uri),
        colors = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, focusedLabelColor = Kn.spectre, cursorColor = Kn.spectre),
    )
}

@Composable
private fun AddressPane(onSeen: (Target, String) -> Unit) {
    val scope = rememberCoroutineScope()
    var address by remember { mutableStateOf("") }
    var port by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val unreachable = stringResource(R.string.add_unreachable)
    Body(stringResource(R.string.add_address_help), color = Kn.frost2, size = 15f)
    Field(stringResource(R.string.add_address), address, { address = it.trim() })
    Field(stringResource(R.string.add_port), port, { port = it.filter(Char::isDigit) }, number = true)
    error?.let { Body(it, color = Kn.warn, size = 14f) }
    PrimaryButton(stringResource(if (busy) R.string.add_contacting else R.string.add_continue), {
        val p = port.toIntOrNull() ?: return@PrimaryButton
        busy = true
        error = null
        scope.launch {
            val t = Target(address, p, address, null)
            val seen = ConsoleClient(address, p).use { c ->
                c.probe()
                c.fingerprintSeen
            }
            busy = false
            if (seen == null) error = unreachable else onSeen(t, seen)
        }
    }, enabled = !busy && address.isNotBlank() && port.isNotBlank())
}

@Composable
private fun VerifyPane(target: Target, seen: String, onAccept: () -> Unit, onCancel: () -> Unit) {
    val s = LocalScale.current
    Body(stringResource(R.string.add_verify_help), color = Kn.frost2, size = 15f)
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(20f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        Eyebrow("${target.address}:${target.port}")
        Mono(shortFingerprint(seen), size = 22f, color = Kn.frost, weight = 500)
    }
    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
        PrimaryButton(stringResource(R.string.add_verify_same), onAccept)
        GlassButton(stringResource(R.string.add_verify_different), onCancel)
    }
}

@Composable
private fun AccountPane(target: Target, onDone: (Host) -> Unit, onCode: () -> Unit) {
    val s = LocalScale.current
    val app = KyberNowApp.instance
    val scope = rememberCoroutineScope()
    var user by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val refused = stringResource(R.string.add_account_refused)
    val changed = stringResource(R.string.add_certificate_mismatch)
    val unreachable = stringResource(R.string.add_unreachable)
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(18f))) {
        Body(target.name, size = 18f, weight = 600)
        Mono("${target.address}:${target.port}" + (target.fingerprint?.let { " · " + shortFingerprint(it) } ?: ""), size = 11f)
    }
    Body(stringResource(R.string.add_account_help), color = Kn.frost2, size = 15f)
    Field(stringResource(R.string.add_user), user, { user = it })
    Field(stringResource(R.string.add_password), password, { password = it }, password = true)
    error?.let { Body(it, color = Kn.warn, size = 14f) }
    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(12f)), verticalAlignment = Alignment.CenterVertically) {
        PrimaryButton(stringResource(if (busy) R.string.add_contacting else R.string.add_save), {
            busy = true
            error = null
            scope.launch {
                val r = ConsoleClient(target.address, target.port, user, password, target.fingerprint).use { c ->
                    val r = c.info()
                    r to c.fingerprintSeen
                }
                busy = false
                when (val reply = r.first) {
                    is Reply.Ok -> {
                        val h = Host(
                            name = reply.value.hostname.ifBlank { target.name },
                            address = target.address,
                            port = target.port,
                            fingerprint = target.fingerprint ?: r.second,
                            user = user,
                            os = reply.value.os,
                            gpu = reply.value.gpu,
                            av1 = reply.value.av1,
                            virtualDisplay = reply.value.virtualDisplay,
                        )
                        app.hosts.put(h, password)
                        onDone(app.hosts.get(h.id) ?: h)
                    }
                    is Reply.Failed -> error = when {
                        reply.unauthorized -> refused
                        reply.certificateChanged -> changed
                        else -> unreachable
                    }
                }
            }
        }, enabled = !busy && user.isNotBlank())
        GlassButton(stringResource(R.string.add_by_code_instead), onCode)
    }
}

@Composable
private fun CodeIntro(announced: List<Announced>, onStart: (List<Target>) -> Unit) {
    val s = LocalScale.current
    Body(stringResource(R.string.add_code_help), color = Kn.frost2, size = 15f)
    if (announced.isEmpty()) {
        Body(stringResource(R.string.add_code_no_station), color = Kn.frost2, size = 15f)
    } else {
        Mono(announced.joinToString(" · ") { it.name }, size = 12f)
    }
    Spacer(Modifier.height(s.dp(4f)))
    PrimaryButton(stringResource(R.string.add_code_show), {
        onStart(announced.map { Target(it.address, it.port, it.name, it.fingerprint) })
    }, enabled = announced.isNotEmpty())
}

/**
 * Code à six chiffres présenté à chaque Station trouvée (`POST /api/pair/request`) ;
 * on l'accepte dans KyberNow Admin (Appareils Link, ou le panneau de l'icône) ;
 * l'appareil reçoit un compte Kyber à lui, révocable sur sa carte.
 */
@Composable
private fun CodePane(targets: List<Target>, onDone: (Host) -> Unit) {
    val s = LocalScale.current
    val app = KyberNowApp.instance
    val code = remember { String.format("%06d", Random.nextInt(0, 1_000_000)) }
    var message by remember { mutableStateOf<Int?>(null) }
    LaunchedEffect(code) {
        // Une demande par Station, puis l'état de chacune toutes les 2 s.
        val pending = mutableMapOf<Target, String>()
        for (t in targets) {
            ConsoleClient(t.address, t.port, pinned = t.fingerprint).use { c ->
                when (val r = c.pairRequest(code, app.device)) {
                    is Reply.Ok -> pending[t] = r.value
                    is Reply.Failed -> {
                        android.util.Log.w("KyberNow", "code : ${t.address}:${t.port} refuse la demande (${r.code} ${r.message})")
                        message = if (r.unknownCall) R.string.add_code_update_station else R.string.add_unreachable
                    }
                }
            }
        }
        while (pending.isNotEmpty()) {
            delay(2000)
            for ((t, id) in pending.toMap()) {
                val r = ConsoleClient(t.address, t.port, pinned = t.fingerprint).use { c -> c.pairStatus(id) to c.fingerprintSeen }
                val st = (r.first as? Reply.Ok)?.value ?: continue
                when (st.state) {
                    PairState.ACCEPTED -> {
                        val h = Host(name = t.name, address = t.address, port = t.port, fingerprint = t.fingerprint ?: r.second, user = st.username ?: "", paired = true)
                        app.hosts.put(h, st.password ?: "")
                        onDone(app.hosts.get(h.id) ?: h)
                        return@LaunchedEffect
                    }
                    PairState.REFUSED -> { pending.remove(t); message = R.string.add_code_refused }
                    PairState.EXPIRED -> { pending.remove(t); message = R.string.add_code_expired }
                    PairState.PENDING -> {}
                }
            }
        }
    }
    Body(stringResource(R.string.add_code_accept), color = Kn.frost2, size = 15f)
    Box(Modifier.panel().fillMaxWidth().padding(s.dp(28f)), contentAlignment = Alignment.Center) {
        Mono(code.chunked(3).joinToString(" "), size = if (s.tv) 72f else 48f, color = Kn.frost, weight = 500)
    }
    Mono(targets.joinToString(" · ") { it.name }, size = 12f)
    message?.let { Body(stringResource(it), color = Kn.warn, size = 14f) }
}

/** QR code d'Admin à la caméra (téléphone, tablette). */
@Composable
private fun QrPane(onFound: (Target) -> Unit) {
    val context = LocalContext.current
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted = it }
    LaunchedEffect(Unit) { if (!granted) ask.launch(Manifest.permission.CAMERA) }
    Body(stringResource(R.string.add_qr_help), color = Kn.frost2, size = 15f)
    if (granted) {
        QrScanner(Modifier.fillMaxWidth().height(LocalScale.current.dp(320f))) { text ->
            PairingLink.parse(text)?.let(onFound)
        }
    } else {
        Body(stringResource(R.string.add_qr_permission), color = Kn.warn, size = 14f)
    }
}
