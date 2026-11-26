package fr.leslignesclaires.kybernow

import androidx.lifecycle.lifecycleScope
import fr.leslignesclaires.kybernow.playstation.PsConsole
import fr.leslignesclaires.kybernow.playstation.PsRemote
import fr.leslignesclaires.kybernow.playstation.PsStatus
import fr.leslignesclaires.kybernow.session.PsSessionActivity
import fr.leslignesclaires.kybernow.ui.playstation.PsHomeActions
import fr.leslignesclaires.kybernow.ui.settings.ConsoleSettingsScreen
import kotlinx.coroutines.launch

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.KeyEvent
import android.view.MotionEvent
import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.repeatOnLifecycle
import fr.leslignesclaires.kybernow.core.Accounts
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.ui.account.AccountCodeScreen
import fr.leslignesclaires.kybernow.ui.account.AccountRelayScreen
import fr.leslignesclaires.kybernow.core.BorrowAsk
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.core.LendInvite
import fr.leslignesclaires.kybernow.core.Loan
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareFound
import fr.leslignesclaires.kybernow.core.ShareLink
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.session.SessionActivity
import fr.leslignesclaires.kybernow.tv.TvChannel
import fr.leslignesclaires.kybernow.ui.AppModel
import fr.leslignesclaires.kybernow.ui.AutoTestActions
import fr.leslignesclaires.kybernow.ui.AutoTestPanel
import androidx.compose.runtime.collectAsState
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.StationNoticeActions
import fr.leslignesclaires.kybernow.ui.StationNoticePanel
import fr.leslignesclaires.kybernow.ui.add.AddHostScreen
import fr.leslignesclaires.kybernow.ui.add.PairingLink
import fr.leslignesclaires.kybernow.ui.add.Target
import fr.leslignesclaires.kybernow.ui.gamepads.GamepadsScreen
import fr.leslignesclaires.kybernow.ui.gamepads.PadMonitor
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeScreen
import fr.leslignesclaires.kybernow.ui.share.JoinScreen
import fr.leslignesclaires.kybernow.core.Welcome
import fr.leslignesclaires.kybernow.ui.welcome.AwayHintDialog
import fr.leslignesclaires.kybernow.ui.welcome.WelcomeActions
import fr.leslignesclaires.kybernow.ui.welcome.WelcomeContent
import fr.leslignesclaires.kybernow.ui.welcome.WelcomeUi
import fr.leslignesclaires.kybernow.ui.settings.AppSettingsScreen
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsScreen

/** Les écrans hors session. */
sealed class Route {
    data object Home : Route()
    /**
     * « Ajouter un ordinateur » ; [invite] : un lien d'invitation `kybernow://borrow` (prêt de l'ordinateur, compte d'ami) ;
     * [inviteUnreadable] : un lien `kybernow://borrow` illisible.
     */
    data class Add(val target: Target? = null, val invite: LendInvite? = null, val inviteUnreadable: Boolean = false, val pairConsole: String? = null) : Route()
    /** « Réglages de {nom} » d'une console PlayStation. */
    data class ConsoleSettings(val id: String) : Route()
    data class HostSettings(val id: String) : Route()
    data object AppSettings : Route()
    data object Gamepads : Route()
    /** « Rejoindre avec un code » (KyberNow Share) ; [link] : un lien ou un QR code `kybernow://join` qui donne la Station et le code. */
    data class Join(val link: ShareLink? = null) : Route()
    /**
     * Comptes KyberNow, Android TV : le code à approuver depuis le téléphone, sur [relay] ; [back] : l'écran d'où l'on
     * vient (l'accueil, les réglages de l'app).
     */
    data class AccountCode(val relay: String, val back: Route = Home) : Route()
    /**
     * Comptes KyberNow : « Ajouter un compte », le relais d'abord (`share.kybernow.app` proposé, « Un autre relais… ») ;
     * [other] : le champ « Un autre relais » ouvert d'emblée (depuis l'écran du code).
     */
    data class AccountRelay(val other: Boolean = false, val back: Route = AppSettings) : Route()
    /**
     * L'accueil en trois écrans (docs/link-ui.md, « Accueil et compte KyberNow ») : [page] 0, 1 ou 2 ; [accounts] : les
     * comptes connectés à son ouverture (un compte de plus : la connexion a réussi, l'accueil se termine).
     */
    data class Welcome(val page: Int = 0, val accounts: Int = 0) : Route()
}

class MainActivity : AppCompatActivity() {
    private val model: AppModel by viewModels()
    private var route by mutableStateOf<Route>(Route.Home)
    /** « Hors de chez vous ? » : l'ordinateur, et ce que « Pas maintenant » laisse faire (réessayer, réveiller). */
    private var awayHint by mutableStateOf<Pair<Host, () -> Unit>?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        handle(intent)
        val app = KyberNowApp.instance
        // Au premier lancement, et une fois après la mise à jour qui l'apporte, sans compte : l'accueil en trois écrans
        // (pas par-dessus un lien qui ouvre un autre écran).
        if (route == Route.Home && app.welcome.shouldShow(app.accounts.accounts.value.isNotEmpty())) route = Route.Welcome()
        // Les avis brefs des comptes (« Connecté à {compte}. », « Ce compte a été déconnecté de cet appareil… »).
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                app.accounts.notices.collect { Toast.makeText(this@MainActivity, it, Toast.LENGTH_LONG).show() }
            }
        }
        // L'avis bref de la mise à jour de la Station lancée par le bandeau (« KyberNow Station {v} est en place. »).
        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                model.notices.collect { Toast.makeText(this@MainActivity, it, Toast.LENGTH_LONG).show() }
            }
        }
        setContent {
            KnTheme(app.form) {
                // Le test des manettes s'ouvre depuis les réglages de l'app : Retour y revient.
                BackHandler(route != Route.Home) {
                    route = when (val r = route) {
                        Route.Gamepads -> Route.AppSettings
                        is Route.AccountCode -> r.back
                        is Route.AccountRelay -> r.back
                        // L'accueil : l'écran d'avant ; au premier, comme « Passer ».
                        is Route.Welcome -> if (r.page > 0) r.copy(page = r.page - 1) else { app.welcome.done(); Route.Home }
                        else -> Route.Home
                    }
                }
                when (val r = route) {
                    Route.Home -> HomeScreen(
                        model,
                        HomeActions(
                            select = model::select,
                            setProfile = model::pickProfile,
                            setHdr = { h, on -> app.hosts.update(h.id) { it.copy(settings = it.settings.copy(hdr = on)) } },
                            connect = { h -> awayFirst(h) { connect(h) } },
                            retry = { h -> awayFirst(h) { model.retry(h.id) } },
                            add = { route = Route.Add() },
                            addAnnounced = { a -> route = Route.Add(Target(a.address, a.port, a.name, a.fingerprint)) },
                            hostSettings = { h -> route = Route.HostSettings(h.id) },
                            appSettings = { route = Route.AppSettings },
                            // Réveiller : l'ordinateur dort chez vous, il n'est pas « hors de chez vous ».
                            wake = { h -> wake(h) },
                            join = { route = Route.Join() },
                            signIn = { signIn(Accounts.DEFAULT_RELAY, Route.Home) },
                            // Sans compte : « Créer un compte », la carte du compte, le bandeau après un appairage.
                            createAccount = { createAccount(Route.Home) },
                            closeAccountCard = { app.welcome.closeCard() },
                            dismissPairedBanner = app.welcome::dismissPairedBanner,
                            showPairedBanner = { h -> app.welcome.showPaired(app.accounts.accounts.value.isNotEmpty(), h.id) },
                            // « Regarder » (droit d'un compte) : la session d'un invité spectateur, comme « Rejoindre avec un code ».
                            watch = { h -> model.watch(h.id, fr.leslignesclaires.kybernow.model.Screen.of(this).current, ::joined) },
                            borrow = { b, password -> model.borrow(b.id, password, ::borrowed) },
                            cancelBorrow = { b -> model.cancelBorrow(b.id) },
                            removeBorrowed = { b -> model.removeBorrowed(b.id) },
                            takeBack = { h, now -> model.takeBack(h.id, now) },
                            stationUpdate = { h, force -> model.offerUpdate(h.id, force) },
                            stationLater = { h, version -> model.offerLater(h.id, version) },
                            stationCancel = { h -> model.offerCancel(h.id) },
                            ps = PsHomeActions(
                                play = ::playConsole,
                                pair = { f -> route = Route.Add(pairConsole = f.hostId) },
                                pairConsole = { c -> route = Route.Add(pairConsole = c.hostId) },
                                settings = { c -> route = Route.ConsoleSettings(c.id) },
                                choose = { c, a -> app.ps.store.update(c.id) { it.copy(account = a.id) } },
                                setHdr = { c, on -> app.ps.store.update(c.id) { it.copy(hdr = on) } },
                            ),
                        ),
                    )
                    is Route.Add -> AddHostScreen(
                        r.target, onDone = { h -> model.select(h.id); paired(h); route = Route.Home }, onBack = { route = Route.Home },
                        onShareLink = { link -> route = Route.Join(link) },
                        invite = r.invite,
                        inviteUnreadable = r.inviteUnreadable,
                        onFriendAdded = { c -> model.select(c.id); route = Route.Home },
                        pairConsole = r.pairConsole,
                        onConsolePaired = { c -> model.select(c.id); route = Route.Home },
                    )
                    is Route.ConsoleSettings -> ConsoleSettingsScreen(r.id, onBack = { route = Route.Home }, onRemoved = { route = Route.Home })
                    is Route.Join -> JoinScreen(r.link, onBack = { route = Route.Home }, onJoined = ::joined, onBorrowed = ::borrowed, onInvited = ::invited)
                    is Route.HostSettings -> HostSettingsScreen(model, r.id, onBack = { route = Route.Home }, onRemoved = { route = Route.Home })
                    Route.AppSettings -> AppSettingsScreen(
                        onBack = { route = Route.Home }, onGamepads = { route = Route.Gamepads },
                        onSignIn = { signIn(Accounts.DEFAULT_RELAY, Route.AppSettings) },
                        onAddAccount = { route = Route.AccountRelay(back = Route.AppSettings) },
                        onOtherRelay = { route = Route.AccountRelay(other = true, back = Route.AppSettings) },
                        onReplayWelcome = { route = Route.Welcome(accounts = app.accounts.accounts.value.size) },
                    )
                    is Route.Welcome -> WelcomeRoute(r)
                    is Route.AccountCode -> AccountCodeScreen(
                        r.relay,
                        onApproved = { a -> app.accounts.added(a); route = Route.Home },
                        onOtherRelay = { route = Route.AccountRelay(other = true, back = r) },
                        onBack = { route = r.back },
                    )
                    is Route.AccountRelay -> AccountRelayScreen(
                        r.other,
                        onChosen = { relay -> signIn(relay, if (r.back is Route.AccountCode) r.back.back else r.back) },
                        onBack = { route = r.back },
                    )
                    Route.Gamepads -> {
                        DisposableEffect(Unit) {
                            PadMonitor.active = true
                            app.gamepads.start()
                            onDispose { PadMonitor.active = false; app.gamepads.stop() }
                        }
                        GamepadsScreen(onBack = { route = Route.AppSettings })
                    }
                }
                // « Hors de chez vous ? » (sans compte), par-dessus l'accueil.
                awayHint?.let { (h, then) ->
                    AwayHintDialog(
                        h.name,
                        onNotNow = { awayHint = null; then() },
                        onCreate = { awayHint = null; createAccount(Route.Home) },
                        onSignIn = { awayHint = null; signIn(Accounts.DEFAULT_RELAY, Route.Home) },
                    )
                }
                // Profil Auto : le panneau du test, par-dessus l'écran du moment (docs/link-ui.md).
                val autoTest by model.autoTest.collectAsState()
                autoTest?.let { st ->
                    AutoTestPanel(st, AutoTestActions(apply = model::applyAutoTest, cancel = model::closeAutoTest, again = { model.openAutoTest(st.hostId) }))
                }
                // Après une mise à jour de l'app : l'avis des Stations à mettre à jour aussi (docs/link-ui.md, « Mise à jour des Stations »).
                val notice by model.notice.collectAsState()
                val stationTasks by model.station.collectAsState()
                notice?.let { n ->
                    StationNoticePanel(n, stationTasks, StationNoticeActions(update = model::noticeUpdate, updateAll = model::noticeUpdateAll, close = model::noticeClose))
                }
            }
        }
    }

    /**
     * L'accueil en trois écrans : « Continuer », « Passer » et « Plus tard » ; « Connecter un compte » (la fenêtre du
     * système, ou le code de l'écran 2 au téléviseur) : réussie, l'écran 3 est passé ; « Créer un compte » ; à l'écran 3,
     * les ordinateurs vus sur le réseau, « Rejoindre avec un code » et le lien de la Station.
     */
    @androidx.compose.runtime.Composable
    private fun WelcomeRoute(r: Route.Welcome) {
        val app = KyberNowApp.instance
        val accounts by app.accounts.accounts.collectAsState()
        // Un compte de plus qu'à l'ouverture : la connexion a réussi, l'accueil se termine (ses ordinateurs arrivent).
        androidx.compose.runtime.LaunchedEffect(accounts.size) { if (accounts.size > r.accounts) finishWelcome(Route.Home) }
        val announced by model.announced.collectAsState()
        val signingIn by app.accounts.signingIn.collectAsState()
        var attempt by androidx.compose.runtime.remember { androidx.compose.runtime.mutableIntStateOf(0) }
        val code = if (app.form == FormFactor.TV && r.page == 1) {
            fr.leslignesclaires.kybernow.ui.account.rememberTvCode(Accounts.DEFAULT_RELAY, attempt) { a -> app.accounts.added(a) }
        } else null
        val station = Welcome.stationUrl
        WelcomeContent(
            WelcomeUi(r.page, announced, Accounts.relayLabel(Accounts.DEFAULT_RELAY), Welcome.stationLabel(station), code, signingIn),
            WelcomeActions(
                next = { route = r.copy(page = (r.page + 1).coerceAtMost(2)) },
                skip = { finishWelcome(Route.Home) },
                signIn = { signIn(Accounts.DEFAULT_RELAY, r) },
                createAccount = { createAccount(r) },
                addAnnounced = { a -> finishWelcome(Route.Add(Target(a.address, a.port, a.name, a.fingerprint))) },
                join = { finishWelcome(Route.Join()) },
                download = { openBrowser(station) },
                newCode = { attempt++ },
            ),
        )
    }

    /** L'accueil est vu (« Passer », « Plus tard », fin, ou un compte connecté) : l'écran [next]. */
    private fun finishWelcome(next: Route) {
        KyberNowApp.instance.welcome.done()
        route = next
    }

    /**
     * « Créer un compte » : la page d'inscription du relais dans le navigateur (l'app ne crée pas de compte), puis l'avis
     * « Créez votre compte dans le navigateur, puis revenez ici : « Connecter un compte ». » ; au téléviseur, l'écran du
     * code, qui dit « Pas encore de compte ? Créez-le sur cette page. ».
     */
    private fun createAccount(back: Route) {
        val app = KyberNowApp.instance
        if (app.form == FormFactor.TV) {
            route = Route.AccountCode(Accounts.DEFAULT_RELAY, back)
            return
        }
        if (openBrowser(Welcome.signUpUrl("", app.language()))) Toast.makeText(this, R.string.welcome_create_notice, Toast.LENGTH_LONG).show()
    }

    /** Une page dans le navigateur de l'appareil ; faux s'il n'y en a pas (téléviseur sans navigateur). */
    private fun openBrowser(url: String): Boolean =
        runCatching { startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }.isSuccess

    /** Un ordinateur vient d'être appairé (ou ajouté par une invitation) : sans compte, le bandeau du compte, une fois. */
    private fun paired(h: Host) {
        val app = KyberNowApp.instance
        app.welcome.paired(app.accounts.accounts.value.isNotEmpty(), h.id)
    }

    /**
     * « Hors de chez vous ? » (sans compte, une fois par ordinateur et par jour) : l'ordinateur appairé ne répond pas sur ce
     * réseau et n'a pas de route par le relais ; « Pas maintenant » laisse faire [then]. Sinon, [then] tout de suite.
     */
    private fun awayFirst(h: Host, then: () -> Unit) {
        val app = KyberNowApp.instance
        val st = model.status.value[h.id]
        val absent = !h.fromAccount && st?.online == false && h.relayRoute == null
        if (absent && app.welcome.takeAwayHint(app.accounts.accounts.value.isNotEmpty(), h.id)) awayHint = h to then else then()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    /**
     * Lien `kybernow://` (QR code d'Admin, lien de partage de KyberNow Share, lien partagé) ou rangée de
     * l'accueil d'Android TV.
     */
    private fun handle(intent: Intent?) {
        val data = intent?.data ?: return
        // `kybernow://compte?…` : le retour de la page de connexion du relais (comptes KyberNow), avant tout autre lien.
        if (Accounts.isCallback(data.toString())) {
            KyberNowApp.instance.accounts.finishSignIn(data.toString())
            return
        }
        TvChannel.hostIdOf(data)?.let { id ->
            KyberNowApp.instance.hosts.get(id)?.let { connect(it) }
            return
        }
        // `kybernow://join?host=…&port=…&fp=…&code=…` : « Rejoindre avec un code », la Station et le code remplis.
        Share.parseLink(data.toString())?.let { route = Route.Join(it); return }
        // `kybernow://borrow?host=…&port=…&fp=…&login=…` : le lien d'invitation d'un compte d'ami (prêt de l'ordinateur).
        Lend.parseInvite(data.toString())?.let { route = Route.Add(invite = it); return }
        // Un lien d'invitation abîmé (coupé dans un message, recopié à la main) : « Ce lien d'invitation est illisible. »
        if (Lend.isInviteLink(data.toString())) { route = Route.Add(inviteUnreadable = true); return }
        PairingLink.parse(data.toString())?.let { route = Route.Add(it) }
    }

    /** Un invité a rejoint un partage : sa session s'ouvre (la Station et ses jetons restent en mémoire, rien n'est enregistré). */
    private fun joined(found: ShareFound) {
        // L'app est partie en arrière-plan pendant la recherche : Android ne laisse pas ouvrir de fenêtre, la session ne
        // s'ouvrirait jamais. L'invité repart tout de suite (la Station l'attendrait 20 s) et ses jetons ne restent nulle part.
        if (!lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) {
            Share.leave(found)
            return
        }
        KyberNowApp.instance.handOverGuest(found)
        startActivity(Intent(this, SessionActivity::class.java).putExtra(SessionActivity.EXTRA_GUEST, true))
        route = Route.Home
    }

    /**
     * L'ordinateur d'un ami est emprunté (code ou compte d'ami) : la session d'emprunt s'ouvre (le prêt et sa demande
     * restent en mémoire, rien n'est enregistré).
     */
    private fun borrowed(loan: Loan, ask: BorrowAsk) {
        // L'app est partie en arrière-plan pendant la préparation : Android n'ouvrirait pas la session. Le prêt reste 15 min
        // à la Station (rien n'est rendu) : l'ami y revient avec « Emprunter » ou le même code.
        if (!lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) return
        KyberNowApp.instance.handOverLoan(loan, ask)
        startActivity(Intent(this, SessionActivity::class.java).putExtra(SessionActivity.EXTRA_BORROW, true))
        route = Route.Home
    }

    /**
     * Une invitation a ajouté l'ordinateur (docs/invitations.md) : il est sur l'accueil, sélectionné, avec un avis bref
     * « {ordinateur} est ajouté. ». Il s'ouvre ensuite par « Se connecter », sur le réseau ou par le relais.
     */
    private fun invited(h: Host) {
        model.select(h.id)
        paired(h)
        route = Route.Home
        Toast.makeText(this, getString(R.string.invite_added, h.name), Toast.LENGTH_SHORT).show()
    }

    /**
     * « Connecter un compte » sur [relay] : au téléviseur, l'écran du code ; ailleurs, la page de connexion
     * du relais dans un onglet personnalisé (le retour `kybernow://compte` arrive à [handle]), puis l'écran [back].
     */
    private fun signIn(relay: String, back: Route) {
        if (KyberNowApp.instance.form == FormFactor.TV) {
            route = Route.AccountCode(relay, back)
            return
        }
        route = back
        val url = KyberNowApp.instance.accounts.beginSignIn(relay) ?: return
        openCustomTab(url)
    }

    /**
     * Un onglet personnalisé (Custom Tabs) sans la bibliothèque androidx.browser : un navigateur qui les connaît (Chrome,
     * Firefox, Samsung Internet…) ouvre la page par-dessus l'app ; un autre l'ouvre comme un lien. Le mot de passe, la
     * passkey et la double authentification se font dans cette page, jamais dans l'app.
     */
    private fun openCustomTab(url: String) {
        val i = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            .putExtras(Bundle().apply { putBinder("android.support.customtabs.extra.SESSION", null) })
            .putExtra("android.support.customtabs.extra.TITLE_VISIBILITY", 1)
        runCatching { startActivity(i) }.onFailure { android.util.Log.w("KyberNow", "comptes : aucun navigateur pour la page de connexion") }
    }

    override fun onStart() {
        super.onStart()
        // Les ordinateurs des comptes : au lancement et à chaque retour au premier plan.
        KyberNowApp.instance.accounts.refresh()
        model.startPolling()
        model.refreshAfterSession()
        // « Refaire le test » demandé pendant la session : le panneau du test s'ouvre maintenant.
        KyberNowApp.instance.takeAutoTest()?.let(model::openAutoTest)
        TvChannel.publish(this, KyberNowApp.instance.hosts.hosts.value)
    }

    override fun onResume() {
        super.onResume()
        KyberNowApp.instance.updates.onResume()
    }

    override fun onStop() {
        super.onStop()
        model.stopPolling()
    }

    /**
     * « Se connecter » : un ordinateur « Hors de chez vous » est vérifié par le relais d'abord (le refus s'écrit sur
     * l'accueil : éteint, accès coupé, relais muet) ; les autres s'ouvrent aussitôt. La session part en « Auto » : le
     * réseau d'abord, le relais sinon.
     */
    private fun connect(h: Host) {
        model.connect(h.id) { open ->
            // Un ordinateur du compte : ses réglages synchronisés d'abord (un instant au plus, jamais un refus).
            if (open.account == null) startActivity(Intent(this, SessionActivity::class.java).putExtra(SessionActivity.EXTRA_HOST, open.id))
            else lifecycleScope.launch {
                KyberNowApp.instance.accounts.syncBeforeSession(open)
                startActivity(Intent(this@MainActivity, SessionActivity::class.java).putExtra(SessionActivity.EXTRA_HOST, open.id))
            }
        }
    }

    private fun wake(h: Host) = model.wake(h.id)

    /**
     * « Jouer » une console (docs/link-ui.md, « Consoles PlayStation ») : allumée, la partie s'ouvre ; en veille, elle
     * est réveillée d'abord (« Réveil de la console… ») ; hors de chez vous, elle est jointe par une Station qui la voit
     * (« Connexion à la console… »), réveillée s'il le faut, puis la partie passe par le tunnel du relais.
     */
    private fun playConsole(c: PsConsole) {
        val ps = KyberNowApp.instance.ps
        lifecycleScope.launch {
            var address = c.address
            var remote: PsRemote? = null
            when (ps.status(c)) {
                PsStatus.STANDBY -> if (!ps.wake(c)) return@launch
                PsStatus.AWAY -> {
                    remote = ps.reach(c) ?: return@launch
                    address = remote.address
                }
                PsStatus.OFFLINE -> return@launch
                else -> ps.live(c.hostId)?.let { address = it.address }
            }
            if (!lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) {
                remote?.close()
                return@launch
            }
            val i = Intent(this@MainActivity, PsSessionActivity::class.java).putExtra(PsSessionActivity.EXTRA_CONSOLE, c.id).putExtra(PsSessionActivity.EXTRA_ADDRESS, address)
            remote?.let { KyberNowApp.instance.handOverRemote(it); i.putExtra(PsSessionActivity.EXTRA_REMOTE, true) }
            startActivity(i)
        }
    }

    // L'écran de test des manettes lit tout ce qu'elles envoient.
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        PadMonitor.key(event)
        return super.dispatchKeyEvent(event)
    }

    override fun dispatchGenericMotionEvent(event: MotionEvent): Boolean {
        PadMonitor.motion(event)
        return super.dispatchGenericMotionEvent(event)
    }
}
