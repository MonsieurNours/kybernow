package fr.leslignesclaires.kybernow.ui.lend

import android.content.Context
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.core.BorrowAsk
import fr.leslignesclaires.kybernow.core.Lend
import fr.leslignesclaires.kybernow.core.ShareTarget
import fr.leslignesclaires.kybernow.model.HostSettings
import fr.leslignesclaires.kybernow.model.Screen
import fr.leslignesclaires.kybernow.model.SessionMode

/** La demande d'emprunt de cet appareil (docs/pret.md, `POST /api/lend/borrow`). */
object Borrowing {
    /**
     * La demande pour cette Station : le code d'une séance ([code]) ou le compte d'ami ([login], [password]), le prénom
     * de l'ami, l'appareil, et l'écran que la Station fera à sa taille : le même calcul qu'une session d'un ordinateur de
     * l'appareil avec les réglages par défaut (« comme cet écran », à la plus haute fréquence ; ramené à ce que le
     * décodeur accepte), et les formats que l'appareil décode.
     */
    fun ask(context: Context, target: ShareTarget, route: String?, name: String, code: String? = null, login: String? = null, password: String? = null): BorrowAsk {
        val app = KyberNowApp.instance
        val d = app.decoders
        val (w, h, r) = SessionMode.pick(Screen.of(context), HostSettings(), if (d.hevc != null) "hevc" else "h264", d)
        val request = Lend.request(code, login, password, name, app.device.id, w, h, r, Lend.codecs(d.hevc != null, d.av1 != null))
        return BorrowAsk(target, route, request)
    }
}
