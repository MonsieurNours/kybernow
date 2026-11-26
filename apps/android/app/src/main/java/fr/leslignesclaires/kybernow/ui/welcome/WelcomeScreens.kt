package fr.leslignesclaires.kybernow.ui.welcome

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.model.Announced
import fr.leslignesclaires.kybernow.ui.Body
import fr.leslignesclaires.kybernow.ui.Eyebrow
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
import fr.leslignesclaires.kybernow.ui.account.CodeStep
import fr.leslignesclaires.kybernow.ui.account.Waiting
import fr.leslignesclaires.kybernow.ui.panel

/*
 * Accueil au premier lancement et incitation au compte KyberNow (docs/link-ui.md, « Accueil et compte KyberNow » ;
 * maquettes : https://claude.ai/artifact/K6tgZaZC8dqeBxGnTBenru) : les trois écrans de l'accueil, la carte du compte
 * de la rangée des ordinateurs, la fenêtre « Hors de chez vous ? » et le bandeau après un appairage. Les règles (quand
 * les montrer) sont celles du cœur (`welcome`, [fr.leslignesclaires.kybernow.model.WelcomeStore]) ; ici, seulement
 * l'interface, avec les composants de l'app (boutons, cartes, panneaux, néon au focus).
 */

/** Ce que l'accueil montre. [page] : 0, 1 ou 2. */
data class WelcomeUi(
    val page: Int,
    /** Les ordinateurs vus sur le réseau (ceux de « Sur le réseau » à l'accueil), pour l'écran 3. */
    val announced: List<Announced> = emptyList(),
    /** Le relais de « La page de {relais} s'ouvre… » : `share.kybernow.app`. */
    val relayLabel: String = "share.kybernow.app",
    /** Le lien de l'écran 3 tel qu'on l'écrit : `www.kybernow.app`. */
    val stationLabel: String = "www.kybernow.app",
    /** Au téléviseur, l'écran 2 : la connexion par code (nul ailleurs, ou pas encore demandée). */
    val code: CodeStep? = null,
    /** « Connexion au compte… » : le retour de la page s'échange contre le compte. */
    val signingIn: Boolean = false,
)

/** Ce que l'accueil demande à l'app. */
class WelcomeActions(
    /** « Continuer » (écran 1), « Continuer sans compte » (écran 2). */
    val next: () -> Unit = {},
    /** « Passer » et « Plus tard » : l'accueil est vu, retour à l'accueil de l'app. */
    val skip: () -> Unit = {},
    /** « Connecter un compte » : la connexion de toujours (fenêtre du système ; le code au téléviseur est déjà là). */
    val signIn: () -> Unit = {},
    /** « Créer un compte » : la page d'inscription dans le navigateur, puis l'avis. */
    val createAccount: () -> Unit = {},
    /** « Ajouter » un ordinateur vu sur le réseau. */
    val addAnnounced: (Announced) -> Unit = {},
    /** « Rejoindre avec un code ». */
    val join: () -> Unit = {},
    /** Le lien de téléchargement de KyberNow Station. */
    val download: () -> Unit = {},
    /** Au téléviseur : « Nouveau code » après un refus. */
    val newCode: () -> Unit = {},
)

/** Les trois écrans : le logo, « Passer » (« Plus tard » à l'écran 3), l'étiquette, le titre, le texte, les boutons, l'avancement. */
@Composable
fun WelcomeContent(ui: WelcomeUi, actions: WelcomeActions) {
    val s = LocalScale.current
    val first = remember { FocusRequester() }
    // Au téléviseur, le focus va au premier bouton de chaque écran.
    LaunchedEffect(ui.page) { if (s.tv) runCatching { first.requestFocus() } }
    BoxWithConstraints(
        Modifier.fillMaxSize().background(Kn.ink).drawBehind {
            // La lueur Spectre des maquettes, en haut à droite.
            drawRect(Brush.radialGradient(listOf(Kn.spectreFlat.copy(alpha = 0.28f), Kn.ink.copy(alpha = 0f)), Offset(size.width * 0.85f, size.height * 0.15f), size.maxDimension * 0.6f))
        },
    ) {
        val wide = maxWidth > maxHeight && maxWidth >= 720.dp
        Column(
            Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding()
                .padding(horizontal = s.dp(if (s.tv) 56f else if (wide) 40f else 22f), vertical = s.dp(if (s.tv) 28f else 18f)),
        ) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Image(painterResource(R.drawable.kn_logo_link), contentDescription = "KyberNow Link", modifier = Modifier.height(s.dp(if (s.tv) 34f else 24f)))
                Spacer(Modifier.weight(1f))
                GlassButton(stringResource(if (ui.page == 2) R.string.welcome_later else R.string.welcome_skip), actions.skip)
            }
            Column(
                Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState()).padding(top = s.dp(if (s.tv) 18f else 28f), bottom = s.dp(16f)),
                verticalArrangement = Arrangement.spacedBy(s.dp(if (s.tv) 14f else 18f)),
            ) {
                when (ui.page) {
                    0 -> PageOne(wide, actions, first)
                    1 -> if (s.tv) PageTwoTv(ui, actions, first) else PageTwo(ui, wide, actions, first)
                    else -> PageThree(ui, wide, actions, first)
                }
            }
            Progress(ui.page)
        }
    }
}

/** L'étiquette « BIENVENUE · {n} / 3 ». */
@Composable
private fun Step(page: Int) = Eyebrow(stringResource(R.string.welcome_step, page + 1), color = Kn.spectre)

/** Le grand titre : Archivo étendu, en capitales (maquettes), sur autant de lignes qu'il faut. */
@Composable
fun WelcomeTitle(text: String, size: Float, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    Text(
        text.uppercase(), modifier, color = Kn.frost,
        style = TextStyle(fontFamily = KnFonts.display, fontSize = s.sp(size), fontWeight = FontWeight(720), letterSpacing = s.sp(-size / 50f), lineHeight = s.sp(size * 1.0f)),
    )
}

/** Les trois traits d'avancement, en bas. */
@Composable
private fun Progress(page: Int) {
    val s = LocalScale.current
    val label = stringResource(R.string.welcome_progress, page + 1)
    Row(Modifier.semantics { contentDescription = label }.padding(top = s.dp(6f)), horizontalArrangement = Arrangement.spacedBy(s.dp(8f))) {
        repeat(3) { i ->
            Box(Modifier.width(s.dp(if (i == page) 26f else 10f)).height(s.dp(4f)).clip(RoundedCornerShape(2.dp)).background(if (i == page) Kn.spectre else Kn.line))
        }
    }
}

/** Un lien : du texte Spectre, focalisable (« Continuer sans compte », `www.kybernow.app`). */
@Composable
fun WelcomeLink(text: String, onClick: () -> Unit, modifier: Modifier = Modifier, mono: Boolean = false) {
    val s = LocalScale.current
    FocusCard(onClick, modifier, shape = RoundedCornerShape(10.dp)) {
        Box(Modifier.padding(horizontal = s.dp(6f), vertical = s.dp(8f))) {
            if (mono) Mono(text, size = 14f, color = Kn.spectre, maxLines = 1) else Body(text, size = 16f, color = Kn.spectre, maxLines = 1)
        }
    }
}

// MARK: - Écran 1

@Composable
private fun PageOne(wide: Boolean, actions: WelcomeActions, first: FocusRequester) {
    val s = LocalScale.current
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f).widthIn(max = s.dp(640f)), verticalArrangement = Arrangement.spacedBy(s.dp(if (s.tv) 16f else 20f))) {
            if (!s.tv) Spacer(Modifier.height(s.dp(if (wide) 20f else 60f)))
            Step(0)
            WelcomeTitle(stringResource(R.string.welcome1_title), if (s.tv) 50f else if (wide) 52f else 38f)
            Body(stringResource(R.string.welcome1_body), size = if (s.tv) 19f else 17f, color = Kn.frost2)
            Spacer(Modifier.height(s.dp(4f)))
            PrimaryButton(stringResource(R.string.welcome_continue), actions.next, Modifier.focusRequester(first))
        }
        // L'ordinateur et les appareils autour, en paysage.
        if (wide || s.tv) {
            Spacer(Modifier.width(s.dp(40f)))
            KnIcon(KnIcons.devices, tint = Kn.spectre, size = if (s.tv) 170f else 190f)
        }
    }
}

// MARK: - Écran 2

/** Les quatre avantages d'un compte : icône, étiquette, phrase. */
private data class Advantage(val icon: ImageVector, val title: Int, val body: Int)

private val advantages = listOf(
    Advantage(KnIcons.globe, R.string.welcome2_away_title, R.string.welcome2_away_body),
    Advantage(KnIcons.devices, R.string.welcome2_devices_title, R.string.welcome2_devices_body),
    Advantage(KnIcons.friends, R.string.welcome2_share_title, R.string.welcome2_share_body),
    Advantage(KnIcons.sliders, R.string.welcome2_settings_title, R.string.welcome2_settings_body),
)

@Composable
private fun AdvantageCard(a: Advantage, modifier: Modifier, row: Boolean) {
    val s = LocalScale.current
    val text: @Composable ColumnScope.() -> Unit = {
        Eyebrow(stringResource(a.title))
        Body(stringResource(a.body), size = 14f, color = Kn.frost2)
    }
    if (row) {
        Row(modifier.panel(14.dp).padding(s.dp(13f)), horizontalArrangement = Arrangement.spacedBy(s.dp(12f))) {
            KnIcon(a.icon, tint = Kn.spectre, size = 22f)
            Column(verticalArrangement = Arrangement.spacedBy(s.dp(4f)), content = text)
        }
    } else {
        Column(modifier.panel(14.dp).padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
            KnIcon(a.icon, tint = Kn.spectre, size = 24f)
            text()
        }
    }
}

/** Téléphone et tablette : les avantages, « Connecter un compte », « Créer un compte », « Continuer sans compte », l'aide. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PageTwo(ui: WelcomeUi, wide: Boolean, actions: WelcomeActions, first: FocusRequester) {
    val s = LocalScale.current
    Step(1)
    WelcomeTitle(stringResource(R.string.welcome2_title), if (wide) 46f else 32f)
    Body(stringResource(R.string.welcome2_body), size = 17f, color = Kn.frost2)
    if (wide) {
        Row(horizontalArrangement = Arrangement.spacedBy(s.dp(14f))) {
            advantages.forEach { AdvantageCard(it, Modifier.weight(1f), row = false) }
        }
    } else {
        Column(verticalArrangement = Arrangement.spacedBy(s.dp(10f))) {
            advantages.forEach { AdvantageCard(it, Modifier.fillMaxWidth(), row = true) }
        }
    }
    val signIn = stringResource(if (ui.signingIn) R.string.account_signing_in else R.string.account_sign_in)
    if (wide) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(12f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
            PrimaryButton(signIn, { if (!ui.signingIn) actions.signIn() }, Modifier.focusRequester(first))
            GlassButton(stringResource(R.string.welcome_create), actions.createAccount)
            WelcomeLink(stringResource(R.string.welcome_without), actions.next)
        }
    } else {
        Column(verticalArrangement = Arrangement.spacedBy(s.dp(10f)), horizontalAlignment = Alignment.CenterHorizontally) {
            PrimaryButton(signIn, { if (!ui.signingIn) actions.signIn() }, Modifier.fillMaxWidth().focusRequester(first))
            GlassButton(stringResource(R.string.welcome_create), actions.createAccount, Modifier.fillMaxWidth())
            WelcomeLink(stringResource(R.string.welcome_without), actions.next)
        }
    }
    Body(stringResource(R.string.welcome2_help, ui.relayLabel), size = 13f, color = Kn.muted)
}

/**
 * Téléviseur : à gauche les avantages et « Continuer sans compte » (un bouton) ; à droite la carte « CONNECTER UN
 * COMPTE » avec le QR code et le code de la connexion par code, « Pas encore de compte ? Créez-le sur cette page. » et
 * l'attente. Approuvé depuis le téléphone, le compte arrive et l'accueil se termine.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PageTwoTv(ui: WelcomeUi, actions: WelcomeActions, first: FocusRequester) {
    val s = LocalScale.current
    // Tout tient dans les 540 dp du téléviseur : les avantages en deux colonnes, la carte du code à droite.
    Row(horizontalArrangement = Arrangement.spacedBy(s.dp(32f))) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(s.dp(12f))) {
            Step(1)
            WelcomeTitle(stringResource(R.string.welcome2_title), 34f)
            Body(stringResource(R.string.welcome2_body), size = 16f, color = Kn.frost2)
            advantages.chunked(2).forEach { pair ->
                Row(horizontalArrangement = Arrangement.spacedBy(s.dp(18f))) {
                    pair.forEach { a ->
                        Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(s.dp(10f)), verticalAlignment = Alignment.Top) {
                            KnIcon(a.icon, tint = Kn.spectre, size = 20f)
                            Column(verticalArrangement = Arrangement.spacedBy(s.dp(2f))) {
                                Eyebrow(stringResource(a.title))
                                Body(stringResource(a.body), size = 13f, color = Kn.frost2)
                            }
                        }
                    }
                }
            }
            Spacer(Modifier.height(s.dp(2f)))
            PrimaryButton(stringResource(R.string.welcome_without), actions.next, Modifier.focusRequester(first))
        }
        CodeCard(ui.code, ui.relayLabel, actions.newCode, Modifier.width(s.dp(330f)))
    }
}

/** La carte du code de l'écran 2 au téléviseur (textes de « Comptes KyberNow »), centrée comme dans la maquette. */
@Composable
private fun CodeCard(step: CodeStep?, relayLabel: String, newCode: () -> Unit, modifier: Modifier) {
    val s = LocalScale.current
    Column(modifier.panel(22.dp).padding(s.dp(18f)), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(s.dp(9f))) {
        Eyebrow(stringResource(R.string.account_sign_in), color = Kn.frost2)
        val code = (step as? CodeStep.Showing)?.code
        Box(Modifier.size(s.dp(140f)), contentAlignment = Alignment.Center) {
            if (code != null) QrCode(code.urlComplete, 140f, stringResource(R.string.account_code_qr))
            else KnIcon(KnIcons.qr, tint = Kn.muted, size = 50f)
        }
        when (step) {
            is CodeStep.Showing -> Text(step.code.code, style = TextStyle(fontFamily = KnFonts.mono, fontSize = s.sp(26f), letterSpacing = s.sp(2f), color = Kn.frost, fontWeight = FontWeight(600)))
            CodeStep.Denied -> Body(stringResource(R.string.account_code_denied), color = Kn.warn, size = 14f)
            is CodeStep.Failed -> Body(step.message.ifBlank { stringResource(R.string.add_unreachable) }, color = Kn.warn, size = 14f)
            else -> {}
        }
        val center = androidx.compose.ui.text.style.TextAlign.Center
        listOf(
            stringResource(R.string.account_code_step_scan),
            stringResource(R.string.account_code_step_open, code?.urlLabel ?: "$relayLabel/tv"),
            stringResource(R.string.account_code_step_connect),
            stringResource(R.string.welcome_tv_no_account),
        ).forEach { line ->
            Text(line, color = Kn.frost2, textAlign = center, style = TextStyle(fontFamily = KnFonts.text, fontSize = s.sp(13f), lineHeight = s.sp(17f)))
        }
        when (step) {
            is CodeStep.Showing -> Waiting(step.minutes)
            CodeStep.Denied, is CodeStep.Failed -> GlassButton(stringResource(R.string.account_code_new), newCode)
            else -> Waiting(null)
        }
    }
}

// MARK: - Écran 3

/**
 * « Ajoutez votre ordinateur » : « ORDINATEURS À PORTÉE », les ordinateurs vus sur le réseau (« SUR LE RÉSEAU »,
 * « Ajouter »), la carte « UN CODE » (« Rejoindre avec un code ») et la carte « PAS ENCORE INSTALLÉ » (le lien de
 * téléchargement de la Station).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PageThree(ui: WelcomeUi, wide: Boolean, actions: WelcomeActions, first: FocusRequester) {
    val s = LocalScale.current
    Step(2)
    WelcomeTitle(stringResource(R.string.welcome3_title), if (s.tv) 40f else if (wide) 46f else 32f)
    Body(stringResource(R.string.welcome3_body), Modifier.widthIn(max = s.dp(760f)), size = 17f, color = Kn.frost2)
    Spacer(Modifier.height(s.dp(2f)))
    Eyebrow(stringResource(R.string.welcome3_in_reach))
    val cardWidth = if (wide || s.tv) Modifier.width(s.dp(if (s.tv) 250f else 270f)) else Modifier.fillMaxWidth()
    FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(14f)), verticalArrangement = Arrangement.spacedBy(s.dp(14f))) {
        ui.announced.forEachIndexed { i, a ->
            // La lueur de « Sur le réseau », comme sa carte à l'accueil.
            ReachCard(cardWidth, glow = true) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    KnIcon(KnIcons.network, tint = Kn.spectre, size = 20f)
                    Spacer(Modifier.width(s.dp(8f)))
                    Eyebrow(stringResource(R.string.welcome3_on_network), color = Kn.spectre)
                }
                Body(a.name, size = 18f, weight = 600, maxLines = 1)
                GlassButton(stringResource(R.string.welcome3_add), { actions.addAnnounced(a) }, if (i == 0) Modifier.focusRequester(first) else Modifier, selected = true)
            }
        }
        ReachCard(cardWidth) {
            Eyebrow(stringResource(R.string.welcome3_code_title))
            Body(stringResource(R.string.welcome3_code_body), size = 14f, color = Kn.frost2)
            GlassButton(
                stringResource(R.string.home_join), actions.join, if (ui.announced.isEmpty()) Modifier.focusRequester(first) else Modifier,
                leading = { KnIcon(KnIcons.code, size = 16f); Spacer(Modifier.width(6.dp)) },
            )
        }
        ReachCard(cardWidth) {
            Eyebrow(stringResource(R.string.welcome3_station_title))
            Body(stringResource(R.string.welcome3_station_body), size = 14f, color = Kn.frost2)
            WelcomeLink(ui.stationLabel, actions.download, mono = true)
        }
    }
}

@Composable
private fun ReachCard(modifier: Modifier, glow: Boolean = false, content: @Composable ColumnScope.() -> Unit) {
    val s = LocalScale.current
    val shape = RoundedCornerShape(16.dp)
    val m = if (glow) modifier.clip(shape).background(Kn.spectre.copy(alpha = 0.08f)).border(BorderStroke(1.dp, Kn.spectre.copy(alpha = 0.6f)), shape)
    else modifier.panel(16.dp)
    Column(m.padding(s.dp(16f)), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), content = content)
}

// MARK: - Sans compte, ensuite

/**
 * La carte du compte, en fin de rangée des ordinateurs : « COMPTE KYBERNOW », « Retrouvez vos ordinateurs partout »,
 * « Sur tous vos appareils, même hors de chez vous. », « Connecter un compte », « Créer un compte » et la croix
 * (« Fermer », puis « Ne plus afficher » quand elle est revenue : [final]), avec la lueur de « Sur le réseau ».
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AccountCard(final: Boolean, onSignIn: () -> Unit, onCreate: () -> Unit, onClose: () -> Unit, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    val shape = RoundedCornerShape(16.dp)
    val close = stringResource(if (final) R.string.account_card_never else R.string.account_card_close)
    Column(
        modifier.clip(shape).background(Brush.linearGradient(listOf(Kn.spectreFlat.copy(alpha = 0.22f), Kn.panel.copy(alpha = 0.9f))))
            .border(BorderStroke(1.dp, Kn.spectre.copy(alpha = 0.6f)), shape).padding(s.dp(14f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(8f)),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            KnIcon(KnIcons.globe, tint = Kn.spectre, size = 16f)
            Spacer(Modifier.width(s.dp(8f)))
            Eyebrow(stringResource(R.string.account_card_eyebrow), Modifier.weight(1f), color = Kn.frost2)
            FocusCard(onClose, Modifier.semantics { contentDescription = close }, shape = RoundedCornerShape(8.dp)) {
                Box(Modifier.padding(s.dp(6f))) { KnIcon(KnIcons.close, tint = Kn.frost2, size = 14f) }
            }
        }
        Body(stringResource(R.string.account_card_title), size = 17f, weight = 650)
        Body(stringResource(R.string.account_card_body), size = 13f, color = Kn.frost2)
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            GlassButton(stringResource(R.string.account_sign_in), onSignIn, selected = true)
            GlassButton(stringResource(R.string.welcome_create), onCreate)
        }
    }
}

/**
 * Après un appairage, sans compte, sous les boutons de l'ordinateur ajouté : « Retrouvez-le aussi hors de chez vous et
 * sur vos autres appareils : connectez un compte KyberNow. », « Connecter un compte », « Plus tard ».
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PairedBanner(onSignIn: () -> Unit, onLater: () -> Unit, modifier: Modifier = Modifier) {
    val s = LocalScale.current
    val shape = RoundedCornerShape(14.dp)
    Column(
        modifier.clip(shape).background(Brush.linearGradient(listOf(Kn.spectreFlat.copy(alpha = 0.22f), Kn.panel.copy(alpha = 0.9f))))
            .border(BorderStroke(1.dp, Kn.spectre.copy(alpha = 0.6f)), shape).padding(s.dp(14f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(10f)),
    ) {
        Row(verticalAlignment = Alignment.Top) {
            KnIcon(KnIcons.globe, tint = Kn.spectre, size = 22f)
            Spacer(Modifier.width(s.dp(12f)))
            Body(stringResource(R.string.paired_hint), size = 14f, color = Kn.frost)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(s.dp(8f)), verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
            GlassButton(stringResource(R.string.account_sign_in), onSignIn, selected = true)
            GlassButton(stringResource(R.string.welcome_later), onLater)
        }
    }
}

/**
 * « Hors de chez vous ? » : « Se connecter » sur un ordinateur appairé qui n'est pas sur ce réseau, sans route par le
 * relais. « Pas maintenant » (l'action demandée suit), « Créer un compte », « Connecter un compte ».
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AwayHintDialog(name: String, onNotNow: () -> Unit, onCreate: () -> Unit, onSignIn: () -> Unit) {
    Dialog(onDismissRequest = onNotNow) { AwayHintContent(name, onNotNow, onCreate, onSignIn) }
}

/** Le contenu de « Hors de chez vous ? » (à part pour les captures). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AwayHintContent(name: String, onNotNow: () -> Unit, onCreate: () -> Unit, onSignIn: () -> Unit) {
    val s = LocalScale.current
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (s.tv) runCatching { first.requestFocus() } }
    Column(
        Modifier.widthIn(max = s.dp(600f)).clip(RoundedCornerShape(22.dp)).background(Kn.ink).border(BorderStroke(1.dp, Kn.line), RoundedCornerShape(22.dp)).padding(s.dp(26f)),
        verticalArrangement = Arrangement.spacedBy(s.dp(20f)),
    ) {
        Row(verticalAlignment = Alignment.Top) {
            Box(Modifier.size(s.dp(52f)).clip(RoundedCornerShape(14.dp)).background(Kn.spectreFlat.copy(alpha = 0.18f)), contentAlignment = Alignment.Center) {
                KnIcon(KnIcons.globe, tint = Kn.spectre, size = 26f)
            }
            Spacer(Modifier.width(s.dp(16f)))
            Column(verticalArrangement = Arrangement.spacedBy(s.dp(8f))) {
                Text(stringResource(R.string.away_hint_title), color = Kn.frost, style = TextStyle(fontFamily = KnFonts.display, fontSize = s.sp(26f), fontWeight = FontWeight(700)))
                Body(stringResource(R.string.away_hint_body, name), size = 15f, color = Kn.frost2)
            }
        }
        FlowRow(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(s.dp(10f), Alignment.End), verticalArrangement = Arrangement.spacedBy(s.dp(10f)), itemVerticalAlignment = Alignment.CenterVertically) {
            WelcomeLink(stringResource(R.string.away_hint_not_now), onNotNow)
            GlassButton(stringResource(R.string.welcome_create), onCreate)
            PrimaryButton(stringResource(R.string.account_sign_in), onSignIn, Modifier.focusRequester(first))
        }
    }
}
