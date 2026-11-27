package fr.leslignesclaires.kybernow.ui.share

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.BorrowAsk
import fr.leslignesclaires.kybernow.core.BorrowReply
import fr.leslignesclaires.kybernow.core.Invite
import fr.leslignesclaires.kybernow.core.InviteCode
import fr.leslignesclaires.kybernow.core.InviteReply
import fr.leslignesclaires.kybernow.core.JoinReply
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.core.Loan
import fr.leslignesclaires.kybernow.core.PendingBorrow
import fr.leslignesclaires.kybernow.core.PendingJoin
import fr.leslignesclaires.kybernow.core.Relays
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareFound
import fr.leslignesclaires.kybernow.core.ShareLink
import fr.leslignesclaires.kybernow.core.ShareQuality
import fr.leslignesclaires.kybernow.core.ShareTarget
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
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.ui.Title
import fr.leslignesclaires.kybernow.ui.add.QrScanner
import fr.leslignesclaires.kybernow.ui.lend.Borrowing
import fr.leslignesclaires.kybernow.ui.panel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private enum class Way { CODE, QR }

/**
 * « Rejoindre avec un code » (KyberNow Share, docs/link-ui.md, « Partage ») : le code à six chiffres
 * (le clavier numérique d'Android au téléphone et à la tablette, un pavé à la télécommande au
 * téléviseur), « Votre prénom » (le nom de l'appareil, retenu), « Qualité de l'image » (« Auto » proposé, le choix retenu),
 * « Rejoindre ». « Auto » envoie ce que l'appareil sait afficher (son écran) et décoder (`MediaCodec`). Le code part aux
 * ordinateurs trouvés sur le réseau, comme l'appairage ; celui qui partage répond. Un lien
 * `kybernow://join` ou un QR code ([link]) fixe la Station et le code : l'écran s'ouvre rempli, et rien ne part
 * avant « Rejoindre » (le QR code lu avec « QR code » de cet écran en fait autant). Rien n'est ajouté aux
 * ordinateurs de l'appareil : [onJoined] ouvre la session d'invité.
 *
 * Hors de chez soi (docs/relais.md) : le code part au réseau et, en même temps, au relais (le relais de KyberNow et ceux des
 * ordinateurs de l'appareil) ; un lien ou un QR code qui porte `relay` et `sid` ouvre le relais de la même façon, Admin
 * épinglé. Un invité venu par le relais attend l'accord de l'hôte (« En attente de l'accord de l'hôte… », « Annuler ») ;
 * s'il refuse, ne répond pas (120 s) ou que le relais se tait, l'écran le dit avec les textes du cœur.
 *
 * Un code d'invitation (docs/invitations.md) : l'écran devient « Ajouter {ordinateur} » et demande le mot de passe de
 * l'invitation ([InviteContent]) ; accepté, l'ordinateur est gardé comme un ordinateur appairé (avec son relais, pour
 * l'ouvrir hors de chez soi ; le même ordinateur déjà sur l'accueil, même empreinte d'Admin, prend le nouvel accès) et
 * [onInvited] le montre sur l'accueil.
 */
@Composable
fun JoinScreen(
    link: ShareLink?,
    unreadable: Boolean = false,
    onBack: () -> Unit,
    onJoined: (ShareFound) -> Unit,
    onBorrowed: (Loan, BorrowAsk) -> Unit = { _, _ -> },
    onInvited: (Host) -> Unit = {},
) {
    val app = KyberNowApp.instance
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val hasCamera = remember { context.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY) }
    var busy by remember { mutableStateOf(false) }
    val unreadableText = stringResource(R.string.share_link_unreadable)
    var error by remember { mutableStateOf(if (unreadable) unreadableText else null) }
    // La demande en attente de l'accord de l'hôte, et le travail qui l'attend.
    var waiting by remember { mutableStateOf<PendingJoin?>(null) }
    var work by remember { mutableStateOf<Job?>(null) }
    // Un code de prêt (docs/pret.md) : l'emprunt en cours (« Préparation de l'ordinateur »), puis l'attente de l'hôte.
    var preparing by remember { mutableStateOf(false) }
    var borrowWaiting by remember { mutableStateOf<PendingBorrow?>(null) }
    // Un code d'invitation : le mot de passe, son envoi et son refus (écrit par le cœur, sous le champ).
    var invite by remember { mutableStateOf<InviteCode?>(null) }
    var adding by remember { mutableStateOf(false) }
    var inviteError by remember { mutableStateOf<String?>(null) }
    // Le cœur écrit les refus ; ce texte ne sert que s'il n'en donne aucun.
    val unreachable = stringResource(R.string.add_unreachable)
    // Avec un lien, la Station est donnée : on ne cherche rien sur le réseau (rien ne part avant « Rejoindre »).
    LaunchedEffect(link) { if (link == null) app.discovery.start() }
    // L'écran se ferme (Retour, la session s'ouvre ailleurs) pendant l'attente : la demande quitte la liste de l'hôte.
    val pending by rememberUpdatedState(waiting)
    val pendingBorrow by rememberUpdatedState(borrowWaiting)
    DisposableEffect(Unit) { onDispose { pending?.let(Share::cancel); pendingBorrow?.let(Lend::cancel) } }

    // Retour ou « Annuler » : l'écran revient au code (rien n'a été ajouté).
    val leaveInvite = { work?.cancel(); invite = null; adding = false; inviteError = null }
    BackHandler(enabled = invite != null) { leaveInvite() }
    invite?.let { code ->
        InviteContent(
            computer = code.computer, busy = adding, error = inviteError,
            onCancel = leaveInvite,
            onAdd = { password ->
                if (!adding) {
                    adding = true
                    inviteError = null
                    work = scope.launch {
                        val reply = Invite.redeem(code, password, app.device)
                        adding = false
                        when (reply) {
                            is InviteReply.Added -> {
                                // Gardé comme un ordinateur appairé par code : son compte, son empreinte, son relais.
                                val host = reply.invited.host(app.hosts.hosts.value, code.computer)
                                app.hosts.put(host, reply.invited.password)
                                invite = null
                                onInvited(app.hosts.get(host.id) ?: host)
                            }
                            is InviteReply.Refused -> inviteError = reply.message.ifBlank { unreachable }
                        }
                    }
                }
            },
        )
        return
    }

    JoinContent(
        link = link, hasCamera = hasCamera, name = app.guestName, quality = app.guestQuality, busy = busy, error = error, onBack = onBack,
        waiting = waiting != null || borrowWaiting != null,
        waitingFor = borrowWaiting?.host?.ifBlank { null },
        preparing = preparing,
        // « Annuler » : la demande est retirée chez l'hôte, l'écran revient au code.
        onCancel = {
            waiting?.let(Share::cancel)
            borrowWaiting?.let(Lend::cancel)
            work?.cancel()
            waiting = null
            borrowWaiting = null
            preparing = false
            busy = false
        },
        // Le code part au lien ou au QR code ([shown]) ou, sans eux, aux ordinateurs trouvés sur le réseau et aux relais.
        onJoin = { digits, name, quality, shown ->
            if (!busy) {
                busy = true
                error = null
                val who = name.trim().ifEmpty { app.device.name }
                work = scope.launch {
                    // « Auto » : l'écran de l'appareil et ce que son décodeur suit (la Station tranche d'après l'écran du partage).
                    val auto = withContext(Dispatchers.Default) { Share.autoQuality(Screen.of(context).current, app.decoders) }
                    val reply = if (shown != null) Share.joinLink(shown, digits, who, app.device, app.decoders, quality, auto)
                    else Share.join(stations(), digits, who, app.device, app.decoders, quality, auto, relays = relays())
                    // Venu par le relais : l'hôte doit accepter (ou refuser, ou ne pas répondre).
                    val answer = if (reply is JoinReply.Pending) {
                        waiting = reply.pending
                        Share.waitForHost(reply.pending, app.decoders)
                    } else reply
                    waiting = null
                    when (answer) {
                        is JoinReply.Joined -> {
                            busy = false
                            app.guestName = who
                            app.guestQuality = quality
                            onJoined(answer.found)
                        }
                        is JoinReply.Refused -> {
                            busy = false
                            error = answer.message.ifBlank { unreachable }
                        }
                        // Jamais : `waitForHost` n'en rend pas.
                        is JoinReply.Pending -> { busy = false; error = unreachable }
                        // Un code d'invitation : « Ajouter {ordinateur} », le mot de passe de l'invitation.
                        is JoinReply.Invite -> {
                            busy = false
                            inviteError = null
                            invite = answer.code
                        }
                        // Un code de prêt : emprunter l'ordinateur avec ce code, l'écran de l'appareil (« Préparation de
                        // l'ordinateur »), puis, s'il est utilisé, attendre l'accord de l'hôte (« En attente de {hôte}… »).
                        is JoinReply.Lend -> {
                            val ask = Borrowing.ask(context, answer.code.target, answer.code.route, who, code = answer.code.code)
                            preparing = true
                            val first = Lend.borrow(ask)
                            val reply = if (first is BorrowReply.Pending) {
                                preparing = false
                                borrowWaiting = first.pending
                                Lend.waitForHost(first.pending)
                            } else first
                            borrowWaiting = null
                            preparing = false
                            busy = false
                            when (reply) {
                                is BorrowReply.Ready -> {
                                    app.guestName = who
                                    onBorrowed(reply.loan, ask)
                                }
                                is BorrowReply.Refused -> error = reply.message.ifBlank { unreachable }
                                is BorrowReply.Pending -> error = unreachable
                            }
                        }
                    }
                }
            }
        },
    )
}

/**
 * Les relais des ordinateurs de l'appareil (`info.relay.server`, gardés avec eux, sous leur forme normale, sans doublon) :
 * le code leur est demandé aussi. Le relais public (`share.kybernow.app`) n'y est pas : le cœur l'interroge toujours.
 */
private fun relays(): List<String> = KyberNowApp.instance.hosts.hosts.value.mapNotNull { it.relay?.server }.map(Relays::normalize).distinct()

/**
 * L'écran sans l'app (les captures le rendent tel quel) : [name] est « Votre prénom » au départ, [quality] la
 * « Qualité de l'image » retenue ; [onJoin] reçoit le code, le prénom, la qualité et, pour un lien ou un QR code, le
 * lien à ouvrir. [waiting] : la demande attend l'accord de l'hôte (hors de chez soi) : le code, le prénom et la qualité
 * laissent la place à « En attente de l'accord de l'hôte… » et « Annuler » ([onCancel]).
 */
@Composable
internal fun JoinContent(
    link: ShareLink?,
    hasCamera: Boolean,
    name: String,
    quality: ShareQuality,
    busy: Boolean,
    error: String?,
    onBack: () -> Unit,
    onJoin: (String, String, ShareQuality, ShareLink?) -> Unit,
    waiting: Boolean = false,
    onCancel: () -> Unit = {},
    /** Un code de prêt qui attend l'accord de l'hôte : son prénom (« En attente de {hôte}… ») ; nul pour un partage. */
    waitingFor: String? = null,
    /** Un code de prêt : la Station prépare l'ordinateur (« Préparation de l'ordinateur » sur le bouton). */
    preparing: Boolean = false,
) {
    val s = LocalScale.current
    var way by remember { mutableStateOf(Way.CODE) }
    // Un QR code lu ici fait comme un lien : la Station et le code sont remplis, il reste à toucher « Rejoindre ».
    var scanned by remember(link) { mutableStateOf<ShareLink?>(null) }
    val shown = link ?: scanned
    var code by remember(shown) { mutableStateOf(shown?.code ?: "") }
    var who by remember(name) { mutableStateOf(name) }
    var how by remember(quality) { mutableStateOf(quality) }

    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(s.dp(if (s.tv) 56f else 20f)).widthIn(max = s.dp(if (s.tv) 980f else 760f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(16f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                FocusCard(onClick = onBack) {
                    Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(10f))) { KnIcon(KnIcons.back, size = 20f) }
                }
                Spacer(Modifier.width(s.dp(14f)))
                // « Rejoindre avec un code » : un partage, un prêt ou une invitation ; le prénom de l'hôte et le nom de
                // l'ordinateur ne sont connus qu'après l'accord : le titre ne change pas.
                Title(stringResource(R.string.share_join_title), size = if (s.tv) 40f else 26f)
            }
            if (waiting) {
                WaitingForHost(onCancel, waitingFor)
                return@Column
            }
            Body(stringResource(R.string.share_join_help), color = Kn.frost2, size = 15f)
            // Le choix « Code » / « QR code », en haut, avec un appareil photo.
            if (link == null && hasCamera && !s.tv) {
                Row(horizontalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                    GlassButton(stringResource(R.string.add_tab_code), { way = Way.CODE }, selected = way == Way.CODE)
                    GlassButton(stringResource(R.string.add_tab_qr), { way = Way.QR }, selected = way == Way.QR)
                }
            }
            // Un lien ou un QR code dit où va le code : on le montre avant d'envoyer quoi que ce soit.
            if (way == Way.CODE || link != null) shown?.let { Mono("${it.target.host}:${it.target.port}", size = 12f) }
            if (way == Way.QR && link == null) {
                QrWay { l -> scanned = l; way = Way.CODE }
            } else if (s.tv) {
                TvJoin(code, { code = it }, who, { who = it }, how, { how = it }, busy, error, preparing) { onJoin(code, who, how, shown) }
            } else {
                CodeField(code) { code = it }
                NameField(who, { who = it })
                QualityField(how, { how = it })
                error?.let { Body(it, color = Kn.warn, size = 14f) }
                JoinButton(busy, code.length == 6, preparing = preparing) { onJoin(code, who, how, shown) }
            }
        }
    }
}

/**
 * « En attente de l'accord de l'hôte… » et « Annuler » (docs/link-ui.md, « Partage », hors de chez soi) : un invité venu par
 * le relais attend que l'hôte l'accepte. « Annuler » quitte la demande. Au téléviseur, le focus y est tout de suite.
 */
@Composable
private fun WaitingForHost(onCancel: () -> Unit, host: String? = null) {
    val s = LocalScale.current
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(Modifier.panel().fillMaxWidth().padding(s.dp(22f)), verticalArrangement = Arrangement.spacedBy(s.dp(16f))) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(14f))) {
            CircularProgressIndicator(Modifier.size(s.dp(26f)), color = Kn.spectre, strokeWidth = s.dp(3f))
            // Un code de prêt : « En attente de {hôte}… » (le prénom vient de la Station, même par un code).
            Body(if (host != null) stringResource(R.string.lend_waiting, host) else stringResource(R.string.share_join_waiting), size = 18f, weight = 600, color = Kn.frost)
        }
        GlassButton(stringResource(R.string.cancel), onCancel, Modifier.focusRequester(focus))
    }
}

/**
 * Les ordinateurs trouvés sur le réseau (ceux de l'appairage) ; on les attend 2 s au plus s'il n'y en a pas encore : le code
 * part aussi aux relais, qui n'ont pas à attendre un réseau sur lequel on n'est peut-être pas.
 */
private suspend fun stations(): List<ShareTarget> {
    val discovery = KyberNowApp.instance.discovery
    var waited = 0
    while (discovery.found.value.isEmpty() && waited < 2000) {
        delay(250)
        waited += 250
    }
    return discovery.found.value.map { ShareTarget(it.address, it.port, it.fingerprint) }
}

/**
 * « Ajouter {ordinateur} » (docs/link-ui.md, « Invitations ») : le mot de passe donné avec le code (ni correction ni
 * majuscule automatiques ; tirets et espaces facultatifs : le clavier de la télécommande suffit), « Ajouter l'ordinateur »
 * (inactif tant que le champ est vide) et « Annuler » ; le mot de passe est masqué, « Afficher » / « Masquer » à droite du
 * champ ; pendant l'appel, « Ajout de {ordinateur}… » ; un refus sous le
 * champ ([error], le texte du cœur). [computer] vide : « Ajouter l'ordinateur ». Au téléviseur, le focus est sur le champ.
 */
@Composable
internal fun InviteContent(computer: String, busy: Boolean, error: String?, onCancel: () -> Unit, onAdd: (String) -> Unit) {
    val s = LocalScale.current
    var password by remember { mutableStateOf("") }
    // Masqué (des points) au départ ; « Afficher » / « Masquer » à droite du champ.
    var visible by remember { mutableStateOf(false) }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (s.tv) runCatching { first.requestFocus() } }
    val title = if (computer.isBlank()) stringResource(R.string.invite_title_unnamed) else stringResource(R.string.invite_title, computer)
    Box(Modifier.fillMaxSize().background(Kn.ink).statusBarsPadding().navigationBarsPadding()) {
        Column(
            Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(s.dp(if (s.tv) 56f else 20f)).widthIn(max = s.dp(if (s.tv) 980f else 760f)),
            verticalArrangement = Arrangement.spacedBy(s.dp(16f)),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                FocusCard(onClick = onCancel) {
                    Box(Modifier.background(Kn.frost.copy(alpha = 0.08f)).padding(s.dp(10f))) { KnIcon(KnIcons.back, size = 20f) }
                }
                Spacer(Modifier.width(s.dp(14f)))
                Title(title, size = if (s.tv) 40f else 26f)
            }
            Body(stringResource(R.string.invite_help), color = Kn.frost2, size = 15f)
            OutlinedTextField(
                value = password,
                onValueChange = { password = it.take(64) },
                label = { Text(stringResource(R.string.invite_password)) },
                placeholder = { Text(stringResource(R.string.invite_password_placeholder), fontFamily = KnFonts.mono, color = Kn.muted) },
                singleLine = true,
                enabled = !busy,
                modifier = Modifier.fillMaxWidth().focusRequester(first),
                textStyle = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(22f), letterSpacing = s.sp(1f), color = Kn.frost),
                visualTransformation = if (visible) VisualTransformation.None else PasswordVisualTransformation(),
                // Un bouton à part entière : au téléviseur, la croix y mène depuis le champ (à droite) et OK bascule.
                trailingIcon = {
                    GlassButton(stringResource(if (visible) R.string.invite_password_hide else R.string.invite_password_show), { visible = !visible }, Modifier.padding(end = s.dp(6f)))
                },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                colors = fieldColors,
            )
            error?.let { Body(it, color = Kn.warn, size = if (s.tv) 15f else 14f) }
            if (busy) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(s.dp(14f))) {
                    CircularProgressIndicator(Modifier.size(s.dp(26f)), color = Kn.spectre, strokeWidth = s.dp(3f))
                    Body(if (computer.isBlank()) stringResource(R.string.invite_adding_unnamed) else stringResource(R.string.invite_adding, computer), size = 18f, weight = 600, color = Kn.frost)
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(s.dp(12f)), verticalAlignment = Alignment.CenterVertically) {
                PrimaryButton(stringResource(R.string.invite_add), { onAdd(password) }, enabled = !busy && Invite.ready(password))
                GlassButton(stringResource(R.string.cancel), onCancel)
            }
        }
    }
}

@Composable
private fun JoinButton(busy: Boolean, ready: Boolean, modifier: Modifier = Modifier, preparing: Boolean = false, onClick: () -> Unit) {
    val label = when {
        preparing -> R.string.lend_preparing
        busy -> R.string.share_join_searching
        else -> R.string.share_join_button
    }
    PrimaryButton(stringResource(label), onClick, modifier, enabled = ready && !busy)
}

/** « 123 456 » : la espace après le troisième chiffre ne se tape pas ; le curseur la saute. */
private object CodeGroups : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val d = text.text
        val shown = if (d.length > 3) d.substring(0, 3) + " " + d.substring(3) else d
        return TransformedText(
            AnnotatedString(shown),
            object : OffsetMapping {
                override fun originalToTransformed(offset: Int) = if (offset > 3) offset + 1 else offset
                override fun transformedToOriginal(offset: Int) = (if (offset > 3) offset - 1 else offset).coerceIn(0, d.length)
            },
        )
    }
}

private val fieldColors
    @Composable get() = OutlinedTextFieldDefaults.colors(focusedBorderColor = Kn.spectre, unfocusedBorderColor = Kn.line, focusedLabelColor = Kn.spectre, cursorColor = Kn.spectre)

/** Le code, au clavier numérique d'Android (téléphone, tablette). */
@Composable
private fun CodeField(code: String, onChange: (String) -> Unit) {
    val s = LocalScale.current
    OutlinedTextField(
        value = code,
        onValueChange = { onChange(Share.digits(it)) },
        label = { Text(stringResource(R.string.share_code_label)) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        visualTransformation = CodeGroups,
        textStyle = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(28f), fontWeight = FontWeight.Medium, letterSpacing = s.sp(2f), color = Kn.frost),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
        colors = fieldColors,
    )
}

/** « Votre prénom » : le nom de l'appareil au début, puis celui qu'on a donné. */
@Composable
private fun NameField(name: String, onChange: (String) -> Unit, modifier: Modifier = Modifier) {
    OutlinedTextField(
        value = name,
        onValueChange = { onChange(it.take(40)) },
        label = { Text(stringResource(R.string.share_join_name)) },
        singleLine = true,
        modifier = modifier.fillMaxWidth(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Text, capitalization = KeyboardCapitalization.Words),
        colors = fieldColors,
    )
}

/** Le QR code de l'hôte, à la caméra (téléphone, tablette) ; un autre QR code est ignoré. */
@Composable
private fun QrWay(onFound: (ShareLink) -> Unit) {
    val context = LocalContext.current
    var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted = it }
    LaunchedEffect(Unit) { if (!granted) ask.launch(Manifest.permission.CAMERA) }
    if (granted) {
        QrScanner(Modifier.fillMaxWidth().height(LocalScale.current.dp(320f))) { text -> Share.parseLink(text)?.let(onFound) }
    } else {
        Body(stringResource(R.string.add_qr_permission), color = Kn.warn, size = 14f)
    }
}

// MARK: - Téléviseur

/** Au téléviseur : le pavé à gauche, le code en grand, le prénom, la qualité de l'image et « Rejoindre » à droite. */
@Composable
private fun TvJoin(code: String, onCode: (String) -> Unit, name: String, onName: (String) -> Unit, quality: ShareQuality, onQuality: (ShareQuality) -> Unit, busy: Boolean, error: String?, preparing: Boolean, onJoin: () -> Unit) {
    val s = LocalScale.current
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(40f))) {
        CodePad(code, onCode, Modifier.focusRequester(first))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(16f))) {
            Box(Modifier.panel().fillMaxWidth().padding(s.dp(24f)), contentAlignment = Alignment.Center) {
                Mono(codeShown(code), size = 56f, color = Kn.frost, weight = 500)
            }
            NameField(name, onName)
            QualityField(quality, onQuality)
            error?.let { Body(it, color = Kn.warn, size = 15f) }
            JoinButton(busy, code.length == 6, preparing = preparing, onClick = onJoin)
        }
    }
}

/** Le code tapé, les chiffres manquants en points : « 12 »  → « 12· ··· ». */
private fun codeShown(code: String): String {
    val six = code.padEnd(6, '·')
    return six.substring(0, 3) + " " + six.substring(3)
}

/** Le pavé numérique à la télécommande : 1 à 9, ⌫, 0 ; la croix déplace, OK tape. */
@Composable
private fun CodePad(code: String, onCode: (String) -> Unit, firstKey: Modifier) {
    val s = LocalScale.current
    Column(verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
        listOf(listOf("1", "2", "3"), listOf("4", "5", "6"), listOf("7", "8", "9"), listOf("⌫", "0", "")).forEachIndexed { r, row ->
            Row(horizontalArrangement = Arrangement.spacedBy(s.dp(10f))) {
                row.forEachIndexed { c, key ->
                    val mod = if (r == 0 && c == 0) firstKey else Modifier
                    if (key.isEmpty()) {
                        Spacer(Modifier.width(s.dp(76f)).height(s.dp(60f)))
                    } else {
                        PadKey(key, mod) { onCode(if (key == "⌫") code.dropLast(1) else Share.digits(code + key)) }
                    }
                }
            }
        }
    }
}

@Composable
private fun PadKey(label: String, modifier: Modifier, onClick: () -> Unit) {
    val s = LocalScale.current
    FocusCard(onClick, modifier) { focused ->
        Box(
            Modifier.width(s.dp(76f)).height(s.dp(60f)).background(if (focused) Kn.neonCore else Kn.panel.copy(alpha = 0.92f)),
            contentAlignment = Alignment.Center,
        ) {
            Mono(label, size = 24f, color = if (focused) Kn.ink else Kn.frost, weight = 500)
        }
    }
}
