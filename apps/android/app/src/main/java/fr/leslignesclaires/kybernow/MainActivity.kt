package fr.leslignesclaires.kybernow

import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import android.view.MotionEvent
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
import fr.leslignesclaires.kybernow.core.Share
import fr.leslignesclaires.kybernow.core.ShareFound
import fr.leslignesclaires.kybernow.core.ShareLink
import fr.leslignesclaires.kybernow.model.Host
import fr.leslignesclaires.kybernow.session.SessionActivity
import fr.leslignesclaires.kybernow.tv.TvChannel
import fr.leslignesclaires.kybernow.ui.AppModel
import fr.leslignesclaires.kybernow.ui.KnTheme
import fr.leslignesclaires.kybernow.ui.add.AddHostScreen
import fr.leslignesclaires.kybernow.ui.add.PairingLink
import fr.leslignesclaires.kybernow.ui.add.Target
import fr.leslignesclaires.kybernow.ui.gamepads.GamepadsScreen
import fr.leslignesclaires.kybernow.ui.gamepads.PadMonitor
import fr.leslignesclaires.kybernow.ui.home.HomeActions
import fr.leslignesclaires.kybernow.ui.home.HomeScreen
import fr.leslignesclaires.kybernow.ui.share.JoinScreen
import fr.leslignesclaires.kybernow.ui.settings.AppSettingsScreen
import fr.leslignesclaires.kybernow.ui.settings.HostSettingsScreen

/** Les écrans hors session. */
sealed class Route {
    data object Home : Route()
    data class Add(val target: Target? = null) : Route()
    data class HostSettings(val id: String) : Route()
    data object AppSettings : Route()
    data object Gamepads : Route()
    /** « Rejoindre avec un code » (KyberNow Share) ; [link] : un lien ou un QR code `kybernow://join` qui donne la Station et le code. */
    data class Join(val link: ShareLink? = null) : Route()
}

class MainActivity : AppCompatActivity() {
    private val model: AppModel by viewModels()
    private var route by mutableStateOf<Route>(Route.Home)

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        handle(intent)
        val app = KyberNowApp.instance
        setContent {
            KnTheme(app.form) {
                // Le test des manettes s'ouvre depuis les réglages de l'app : Retour y revient.
                BackHandler(route != Route.Home) { route = if (route == Route.Gamepads) Route.AppSettings else Route.Home }
                when (val r = route) {
                    Route.Home -> HomeScreen(
                        model,
                        HomeActions(
                            select = model::select,
                            setProfile = model::setProfile,
                            connect = ::connect,
                            retry = { h -> model.retry(h.id) },
                            add = { route = Route.Add() },
                            addAnnounced = { a -> route = Route.Add(Target(a.address, a.port, a.name, a.fingerprint)) },
                            hostSettings = { h -> route = Route.HostSettings(h.id) },
                            appSettings = { route = Route.AppSettings },
                            wake = ::wake,
                            join = { route = Route.Join() },
                        ),
                    )
                    is Route.Add -> AddHostScreen(
                        r.target, onDone = { h -> model.select(h.id); route = Route.Home }, onBack = { route = Route.Home },
                        onShareLink = { link -> route = Route.Join(link) },
                    )
                    is Route.Join -> JoinScreen(r.link, onBack = { route = Route.Home }, onJoined = ::joined)
                    is Route.HostSettings -> HostSettingsScreen(model, r.id, onBack = { route = Route.Home }, onRemoved = { route = Route.Home })
                    Route.AppSettings -> AppSettingsScreen(onBack = { route = Route.Home }, onGamepads = { route = Route.Gamepads })
                    Route.Gamepads -> {
                        DisposableEffect(Unit) {
                            PadMonitor.active = true
                            app.gamepads.start()
                            onDispose { PadMonitor.active = false; app.gamepads.stop() }
                        }
                        GamepadsScreen(onBack = { route = Route.AppSettings })
                    }
                }
            }
        }
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
        TvChannel.hostIdOf(data)?.let { id ->
            KyberNowApp.instance.hosts.get(id)?.let { connect(it) }
            return
        }
        // `kybernow://join?host=…&port=…&fp=…&code=…` : « Rejoindre avec un code », la Station et le code remplis.
        Share.parseLink(data.toString())?.let { route = Route.Join(it); return }
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

    override fun onStart() {
        super.onStart()
        model.startPolling()
        model.refreshAfterSession()
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

    private fun connect(h: Host) {
        startActivity(Intent(this, SessionActivity::class.java).putExtra(SessionActivity.EXTRA_HOST, h.id))
    }

    private fun wake(h: Host) = model.wake(h.id)

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
