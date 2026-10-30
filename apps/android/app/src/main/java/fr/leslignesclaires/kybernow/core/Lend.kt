package fr.leslignesclaires.kybernow.core

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.time.DayOfWeek
import java.time.Instant
import java.time.ZoneId
import java.time.format.TextStyle
import java.util.Locale

/*
 * Prêt de l'ordinateur (KyberNow Share, docs/pret.md) vu de l'app : l'ami qui emprunte (la carte « Ordinateur de {hôte} »,
 * « Emprunter », l'attente de l'accord, la session d'emprunt) et l'hôte qui prête (la vue du prêt : code, demandes,
 * emprunteur). Le cœur fait l'échange avec Admin, la session et les textes des refus, des préavis et des fins ; ce fichier
 * range ses réponses JSON pour l'interface et garde les petits calculs (temps restant, compte à rebours, créneau).
 */

private fun JSONObject.text(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

private fun JSONObject.seconds(key: String): Long? = if (isNull(key) || !has(key)) null else optLong(key).takeIf { it > 0 }

/** L'état de l'ordinateur d'un ami pour son compte (`state` de `POST /api/lend/status`) ; « Hors ligne » est à l'app. */
enum class LendAvailability(val id: String) {
    FREE("free"),
    BUSY("busy"),
    LENT("lent"),
    LOCKED("locked"),
    OFF("off");

    /** « Emprunter » démarre le prêt ([FREE]) ou demande à l'hôte ([BUSY]) ; prêté, verrouillé ou coupé : rien à faire. */
    val borrowable get() = this == FREE || this == BUSY

    companion object {
        /** Un état inconnu (Station plus récente) se lit « Prêt coupé » : rien ne part. */
        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: OFF
    }
}

/** Un créneau de la semaine, en secondes Unix (l'heure du PC de l'hôte, rendue en heure de l'appareil). */
data class LendPeriod(val start: Long, val end: Long)

/** Le créneau d'un compte d'ami : en cours ([now], jusqu'à [next]`.end`), ou le prochain. */
data class LendSlot(val now: Boolean, val next: LendPeriod?)

/**
 * La carte « Ordinateur de {hôte} » : l'état, le prénom de l'hôte, le nom de l'ordinateur, le créneau ; [mine] : le prêt en
 * cours est celui de cet ami (il peut y revenir, au lieu de « Prêté à quelqu'un d'autre »).
 */
data class LendStatus(val state: LendAvailability, val host: String, val computer: String, val slot: LendSlot? = null, val mine: Boolean = false) {
    /** « Emprunter » ou « Demander à {hôte} » a un sens : libre, occupé, ou prêté à cet ami (il revient). */
    val borrowable get() = state.borrowable || (state == LendAvailability.LENT && mine)

    companion object {
        fun parse(o: JSONObject): LendStatus {
            val slot = o.optJSONObject("slot")?.let { s ->
                LendSlot(s.optBoolean("now"), s.optJSONObject("next")?.let { LendPeriod(it.optLong("start"), it.optLong("end")) })
            }
            return LendStatus(LendAvailability.parse(o.optString("state")), o.optString("host"), o.optString("computer"), slot, o.optBoolean("mine"))
        }
    }
}

/** Le compte Steam du prêt (`LendSteam` du cœur, mêmes codes) : celui de l'ami (proposé) ou celui de l'hôte. */
enum class LendSteam(val code: Int, val id: String) {
    FRIEND(0, "friend"),
    HOST(1, "host");

    companion object {
        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: FRIEND
    }
}

/** La durée d'un code de prêt : 1, 2 (proposée) ou 4 heures, ou « Jusqu'à ce que je reprenne » ([seconds] 0). */
enum class LendDuration(val seconds: Long) {
    ONE_HOUR(3600),
    TWO_HOURS(7200),
    FOUR_HOURS(14400),
    OPEN(0);

    companion object {
        val DEFAULT = TWO_HOURS

        /** Une durée que la Station donne (`code.duration`) ; absente : sans fin. */
        fun of(seconds: Long?) = entries.firstOrNull { it.seconds == (seconds ?: 0L) } ?: OPEN
    }
}

/** Ce que fait le prêt chez l'hôte (`state` de la vue du prêt). */
enum class LendHostState(val id: String) {
    FREE("free"),
    PREPARING("preparing"),
    LENT("lent"),
    ENDING("ending");

    companion object {
        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: FREE
    }
}

/** Comment l'ami est venu : un code, son compte, son créneau. */
enum class LendKind(val id: String) {
    CODE("code"),
    FRIEND("friend"),
    SLOT("slot");

    companion object {
        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: CODE
    }
}

/** Pourquoi le préavis de 2 minutes. */
enum class LendNoticeReason(val id: String) {
    TAKEN_BACK("taken_back"),
    TIME_UP("time_up"),
    SLOT_END("slot_end"),
    /** Chez l'hôte : le créneau d'un ami commence. */
    SLOT_START("slot_start");

    companion object {
        fun parse(id: String?) = entries.firstOrNull { it.id == id }
    }
}

/** Le préavis : sa raison et l'heure de la fin (secondes Unix). */
data class LendNotice(val reason: LendNoticeReason, val at: Long) {
    fun json(): JSONObject = JSONObject().put("reason", reason.id).put("at", at)

    companion object {
        fun parse(o: JSONObject?): LendNotice? {
            val reason = LendNoticeReason.parse(o?.optString("reason")) ?: return null
            return LendNotice(reason, o!!.optLong("at"))
        }

        fun parse(json: String?): LendNotice? = runCatching { json?.let { parse(JSONObject(it)) } }.getOrNull()
    }
}

/** La fin du prêt, en session (`SessionListener.onLendEnd`, mêmes codes). */
enum class LendEnd(val code: Int) {
    TAKEN_BACK(0),
    TIME_UP(1),
    SLOT_END(2),
    /** L'ami a rendu l'ordinateur : une fin voulue, sans texte. */
    LEFT(3),
    STOPPED(4);

    companion object {
        fun of(code: Int) = entries.firstOrNull { it.code == code } ?: STOPPED
    }
}

/** L'ami à qui l'ordinateur est prêté, chez l'hôte. */
data class LendBorrower(
    val name: String,
    val kind: LendKind,
    val app: String,
    val via: ShareVia,
    val since: Long?,
    val until: Long?,
    val steam: LendSteam,
    val connected: Boolean,
) {
    companion object {
        fun parse(o: JSONObject) = LendBorrower(
            o.optString("name"), LendKind.parse(o.optString("kind")), o.optString("app"), ShareVia.parse(o.optString("via")),
            o.seconds("since"), o.seconds("until"), LendSteam.parse(o.optString("steam")), o.optBoolean("connected"),
        )
    }
}

/**
 * Une demande d'emprunt (ordinateur utilisé), à accepter ou à refuser ; [steam] : le compte Steam proposé (celui du compte
 * d'ami ou du code), donné à « Accepter » ; [slot] : elle vient du créneau de l'ami (l'ordinateur lui est prêté dans 2 min
 * sans réponse).
 */
data class LendRequest(
    val id: String,
    val name: String,
    val kind: LendKind,
    val app: String,
    val via: ShareVia,
    val since: Long?,
    val steam: LendSteam = LendSteam.FRIEND,
    val slot: Boolean = false,
) {
    /** Venu par le relais : « hors de chez vous ». */
    val away get() = via == ShareVia.RELAY

    companion object {
        fun parse(o: JSONObject) = LendRequest(
            o.optString("id"), o.optString("name"), LendKind.parse(o.optString("kind")), o.optString("app"), ShareVia.parse(o.optString("via")), o.seconds("since"),
            LendSteam.parse(o.optString("steam")), o.optBoolean("slot"),
        )
    }
}

/** Le code d'une séance : le code ([codeText] tel qu'on l'affiche, « 123 456 »), le lien du QR code, l'adresse web. */
data class LendCodeView(
    val code: String,
    val codeText: String,
    val link: String,
    val web: String,
    val duration: LendDuration,
    val steam: LendSteam,
    val expires: Long?,
) {
    companion object {
        fun parse(o: JSONObject, format: (String) -> String): LendCodeView? {
            val code = o.optString("code").takeIf { it.isNotEmpty() } ?: return null
            return LendCodeView(code, format(code), o.optString("link"), o.optString("web"), LendDuration.of(o.seconds("duration")), LendSteam.parse(o.optString("steam")), o.seconds("expires"))
        }
    }
}

/**
 * La vue du prêt chez l'hôte (`LendView` du cœur : les routes `/api/lend…` et le signe de vie de la session) : le prêt
 * activé dans Admin ([enabled]), ce qu'il fait, l'ami qui a l'ordinateur, le préavis, les demandes et le code de séance.
 */
data class LendView(
    val enabled: Boolean,
    val state: LendHostState,
    val borrower: LendBorrower? = null,
    val notice: LendNotice? = null,
    val requests: List<LendRequest> = emptyList(),
    val code: LendCodeView? = null,
) {
    /** L'ordinateur est prêté (ou va l'être, ou finit de l'être) : « Prêté à {prénom} ». */
    val lent get() = borrower != null && state != LendHostState.FREE

    /** « Reprendre l'ordinateur » a lancé le préavis : « Reprise dans 1:42 » et « Reprendre tout de suite ». */
    val takingBack get() = notice?.reason == LendNoticeReason.TAKEN_BACK

    companion object {
        fun parse(o: JSONObject?, format: (String) -> String = Share::formatCode): LendView? {
            if (o == null) return null
            val list = o.optJSONArray("requests") ?: JSONArray()
            return LendView(
                enabled = o.optBoolean("enabled"),
                state = LendHostState.parse(o.optString("state")),
                borrower = o.optJSONObject("borrower")?.let(LendBorrower::parse),
                notice = LendNotice.parse(o.optJSONObject("notice")),
                requests = (0 until list.length()).mapNotNull { i -> list.optJSONObject(i)?.let(LendRequest::parse) }.filter { it.id.isNotEmpty() },
                code = o.optJSONObject("code")?.let { LendCodeView.parse(it, format) },
            )
        }

        fun parse(json: String?, format: (String) -> String = Share::formatCode): LendView? =
            runCatching { json?.let { parse(JSONObject(it), format) } }.getOrNull()
    }
}

/**
 * Ce que « Prêter l'ordinateur… » peut faire avec l'ordinateur, d'après son `info` (`lend` : absent avant la Station
 * 0.9.0 ; `enabled` faux : le prêt est coupé dans Admin).
 */
enum class LendAvailable {
    TOO_OLD,
    OFF,
    ON;

    companion object {
        fun of(infoJson: String?): LendAvailable {
            val lend = runCatching { JSONObject(infoJson ?: "").optJSONObject("lend") }.getOrNull() ?: return TOO_OLD
            return if (lend.optBoolean("available", true) && lend.optBoolean("enabled")) ON else OFF
        }
    }
}

/**
 * Un lien d'invitation (`kybernow://borrow?host=…&port=…&fp=…&login=…[&relay=…&sid=…]`, lu par le cœur) : la Station,
 * l'identifiant de l'ami et, hors de chez soi, la route par le relais ([route], `{server, station, admin_fp}`).
 */
data class LendInvite(val target: ShareTarget, val login: String, val relay: String? = null, val sid: String? = null, val route: String? = null) {
    companion object {
        fun parse(o: JSONObject): LendInvite? {
            val host = o.text("host") ?: return null
            val port = o.optInt("port").takeIf { it > 0 } ?: return null
            val login = o.text("login") ?: return null
            return LendInvite(ShareTarget(host, port, o.text("fingerprint")), login, o.text("relay"), o.text("sid"), o.optJSONObject("route")?.toString())
        }
    }
}

/** « Rejoindre avec un code » est tombé sur un code de prêt (code 29) : la Station qui l'a, et sa route par le relais. */
data class LendCode(val code: String, val target: ShareTarget, val route: String? = null) {
    companion object {
        fun parse(o: JSONObject): LendCode? {
            val code = o.text("code") ?: return null
            val target = o.optJSONObject("target")?.let(ShareTarget::parse) ?: return null
            return LendCode(code, target, o.optJSONObject("route")?.toString())
        }
    }
}

/**
 * Ce qu'il faut pour emprunter, et pour revenir avec la même demande pendant 15 min (« Revenir ») : la Station, sa route
 * par le relais et la demande (`{code}` ou `{login, password}`, l'appareil, son écran). Elle porte le code ou le mot de
 * passe : seulement en mémoire, jamais dans un journal.
 */
class BorrowAsk(val target: ShareTarget, val route: String?, val request: JSONObject) {
    override fun toString() = "BorrowAsk(${target.host}:${target.port})"
}

/**
 * Le prêt accordé (`{target, borrowed}` du cœur) : [json] va tel quel à la session (`borrower.borrowed`) et à
 * `lendLeave`. Il porte le secret du prêt et le jeton de Kyber : il ne se garde nulle part et ne s'écrit jamais.
 */
class Loan(
    val target: ShareTarget,
    /** Le prénom de l'hôte (« Prêté par {hôte} ») ; le nom de l'ordinateur si l'hôte n'en a pas. */
    val host: String,
    val computer: String,
    /** La fin prévue (secondes Unix) ; nulle : « Jusqu'à ce que {hôte} reprenne ». */
    val until: Long?,
    /** L'écran virtuel du prêt. */
    val width: Int,
    val height: Int,
    val refresh: Int,
    val json: String,
) {
    /** Le `borrowed` de la session d'emprunt. */
    val borrowed: JSONObject get() = JSONObject(json).getJSONObject("borrowed")

    override fun toString() = "Loan(${target.host}:${target.port}, $computer)"

    companion object {
        fun parse(value: JSONObject): Loan {
            val b = value.getJSONObject("borrowed")
            val d = b.optJSONObject("display")
            val computer = b.optString("computer")
            return Loan(
                target = ShareTarget.parse(value.getJSONObject("target")),
                host = b.optString("host").ifBlank { computer },
                computer = computer,
                until = b.seconds("until"),
                width = d?.optInt("width") ?: 0,
                height = d?.optInt("height") ?: 0,
                refresh = d?.optInt("refresh") ?: 0,
                json = value.toString(),
            )
        }
    }
}

/** Une demande d'emprunt qui attend l'accord de l'hôte : [json] va tel quel à `lendWait` et `lendCancel` (il porte un secret). */
class PendingBorrow(val json: String, val host: String, val computer: String) {
    internal val cancelled = java.util.concurrent.atomic.AtomicBoolean(false)

    override fun toString() = "PendingBorrow(en attente de $host)"

    companion object {
        fun parse(o: JSONObject) = PendingBorrow(o.toString(), o.optString("host"), o.optString("computer"))
    }
}

/** Réponse de « Emprunter » et de l'attente : le prêt, l'attente de l'hôte, ou un refus (le texte du cœur). */
sealed class BorrowReply {
    data class Ready(val loan: Loan) : BorrowReply()
    data class Pending(val pending: PendingBorrow) : BorrowReply()

    /**
     * [code] : 40 identifiant ou mot de passe refusé, 41 code qui ne sert plus, 42 déjà prêté, 43 verrouillé, 44 prêt
     * coupé, 45 trop d'essais, 46 refusé, 47 sans réponse, 48 Station sans prêt, et ceux d'Admin ou du relais ; [message]
     * : le texte à montrer.
     */
    data class Refused(val code: Int, val kind: String, val message: String) : BorrowReply() {
        val badCredentials get() = code == Lend.BAD_CREDENTIALS
    }
}

object Lend {
    const val BAD_CREDENTIALS = 40
    const val TOO_OLD = 48
    private const val WAITING = 49

    /** Les formats que l'appareil décode, pour Admin (`codecs`). */
    fun codecs(hevc: Boolean, av1: Boolean): JSONArray = JSONArray().apply {
        if (hevc) put("hevc")
        if (av1) put("av1")
        put("h264")
    }

    /**
     * La demande d'emprunt : `{code}` ou `{login, password}`, le prénom de l'ami, l'appareil, `app` `android`, l'écran de
     * l'appareil (la taille de l'écran virtuel du prêt) et les formats qu'il décode.
     */
    fun request(code: String?, login: String?, password: String?, name: String, deviceId: String, width: Int, height: Int, refresh: Int, codecs: JSONArray): JSONObject {
        val o = JSONObject()
        if (code != null) o.put("code", code) else o.put("login", login ?: "").put("password", password ?: "")
        return o.put("name", name).put("device", deviceId).put("app", "android")
            .put("screen", JSONObject().put("width", width).put("height", height).put("refresh", refresh))
            .put("codecs", codecs)
    }

    /** Un lien d'invitation, ou `null` si ce n'en est pas un. */
    fun parseInvite(url: String): LendInvite? = runCatching { Native.lendParseInvite(url)?.let { LendInvite.parse(JSONObject(it)) } }.getOrNull()

    /** La carte « Ordinateur de {hôte} » : l'état pour ce compte, ou un refus (`Reply.Failed`, 40 : identifiant ou mot de passe). */
    suspend fun status(target: ShareTarget, route: String?, login: String, password: String): Reply<LendStatus> = withContext(Dispatchers.IO) {
        readStatus(Native.lendStatus(target.json().toString(), route, login, password))
    }

    internal fun readStatus(text: String?): Reply<LendStatus> {
        val o = runCatching { JSONObject(text ?: "") }.getOrNull() ?: return Reply.Failed(1, "")
        val code = o.optInt("code", 1)
        if (code != 0) return Reply.Failed(code, o.optString("error"))
        val v = o.optJSONObject("value") ?: return Reply.Failed(1, "")
        return Reply.Ok(LendStatus.parse(v))
    }

    /** « Emprunter » (bloquant jusqu'à 30 s : le mode console) ; renoncer pendant l'appel retire la demande ou rend le prêt. */
    suspend fun borrow(ask: BorrowAsk): BorrowReply {
        // Jusqu'au bout même si l'appelant renonce : le prêt accordé entre-temps est rendu, la demande retirée (ci-dessous).
        val text = withContext(NonCancellable + Dispatchers.IO) { Native.lendBorrow(ask.target.json().toString(), ask.route, ask.request.toString()) }
        val reply = readBorrow(text)
        if (currentCoroutineContext().isActive) return reply
        when (reply) {
            is BorrowReply.Ready -> leave(reply.loan)
            is BorrowReply.Pending -> cancel(reply.pending)
            is BorrowReply.Refused -> {}
        }
        throw CancellationException("emprunt annulé")
    }

    /** La réponse du cœur à `lendBorrow` : `value` `{"ready": …}` ou `{"pending": …}`. */
    internal fun readBorrow(text: String?): BorrowReply {
        val o = runCatching { JSONObject(text ?: "") }.getOrNull() ?: return BorrowReply.Refused(1, "other", "")
        if (o.optInt("code", 1) != 0) return refusal(o)
        val value = o.optJSONObject("value") ?: return BorrowReply.Refused(1, "other", "")
        value.optJSONObject("pending")?.let { return BorrowReply.Pending(PendingBorrow.parse(it)) }
        val ready = value.optJSONObject("ready") ?: return BorrowReply.Refused(1, "other", "")
        return runCatching { BorrowReply.Ready(Loan.parse(ready)) }.getOrNull() ?: BorrowReply.Refused(1, "other", "")
    }

    private fun refusal(o: JSONObject) = BorrowReply.Refused(o.optInt("code", 1), o.optString("kind").ifEmpty { "other" }, o.optString("error"))

    /**
     * « En attente de {hôte}… » : redemande à l'hôte (un appel tenu 30 s à chaque fois) jusqu'à sa réponse : le prêt, ou
     * un refus (« {hôte} a refusé. », « {hôte} n'a pas répondu. »). Un à-coup du relais, une ou deux fois de suite, ne coupe
     * pas l'attente. Annulée (« Annuler », l'écran qui se ferme), la demande est retirée chez l'hôte.
     */
    suspend fun waitForHost(pending: PendingBorrow): BorrowReply {
        var failures = 0
        try {
            while (true) {
                val text = withContext(NonCancellable + Dispatchers.IO) { Native.lendWait(pending.json) }
                val o = runCatching { JSONObject(text) }.getOrNull() ?: return BorrowReply.Refused(1, "other", "")
                when (o.optInt("code", 1)) {
                    0 -> {
                        val loan = runCatching { Loan.parse(o.getJSONObject("value")) }.getOrNull() ?: return BorrowReply.Refused(1, "other", "")
                        if (!currentCoroutineContext().isActive) {
                            leave(loan)
                            throw CancellationException("emprunt annulé")
                        }
                        return BorrowReply.Ready(loan)
                    }
                    WAITING -> failures = 0
                    RelayFailure.UNREACHABLE.code -> if (++failures >= 3) return refusal(o) else delay(2000)
                    else -> return refusal(o)
                }
                currentCoroutineContext().ensureActive()
            }
        } catch (e: CancellationException) {
            cancel(pending)
            throw e
        }
    }

    /** « Annuler » : la demande est retirée chez l'hôte (sur son propre fil). Sans effet la seconde fois. */
    fun cancel(pending: PendingBorrow) {
        if (pending.cancelled.getAndSet(true)) return
        Thread({ Native.lendCancel(pending.json) }, "kybernow-pret-annuler").start()
    }

    /** « Rendre l'ordinateur » hors d'une session (un prêt accordé que l'app ne peut pas ouvrir), sur son propre fil. */
    fun leave(loan: Loan) {
        Thread({ Native.lendLeave(loan.json) }, "kybernow-pret-rendre").start()
    }

    // MARK: - Temps

    /**
     * La fin à montrer dans la barre de l'ami : l'heure du préavis s'il y en a un (2 min), sinon la fin prévue ; nulle
     * sans fin (« Jusqu'à ce que {hôte} reprenne »).
     */
    fun endsAt(until: Long?, notice: LendNotice?): Long? = notice?.at?.takeIf { it > 0 } ?: until

    /** Les secondes qui restent jusqu'à [at] (secondes Unix) à [now] ; jamais négatives. */
    fun left(at: Long, now: Long): Long = (at - now).coerceAtLeast(0)

    /** Arrondi à la minute du dessus (« Encore 1 h 20 » ne tombe pas à « 1 h 19 » dès la première seconde). */
    fun roundUpToMinute(seconds: Long): Long = (seconds.coerceAtLeast(0) + 59) / 60 * 60

    /** « 1:42 » : le compte à rebours d'un préavis (minutes et secondes). */
    fun countdown(seconds: Long): String {
        val s = seconds.coerceAtLeast(0)
        return "${s / 60}:${String.format(Locale.ROOT, "%02d", s % 60)}"
    }

    /**
     * « 18 h », « 18 h 30 » en français ; « 6 PM », « 6:30 PM » en anglais (docs/link-ui.md : « jusqu'à 18 h », “until
     * 6 PM”). [at] en secondes Unix, à l'heure de l'appareil ([zone]).
     */
    fun hour(at: Long, french: Boolean, zone: ZoneId = ZoneId.systemDefault()): String {
        val t = Instant.ofEpochSecond(at).atZone(zone)
        val h = t.hour
        val m = t.minute
        return if (french) {
            if (m == 0) "$h h" else "$h h ${String.format(Locale.ROOT, "%02d", m)}"
        } else {
            val h12 = if (h % 12 == 0) 12 else h % 12
            val suffix = if (h < 12) "AM" else "PM"
            if (m == 0) "$h12 $suffix" else "$h12:${String.format(Locale.ROOT, "%02d", m)} $suffix"
        }
    }

    /** « mercredi », “Wednesday” : le jour d'un créneau, à l'heure de l'appareil. */
    fun day(at: Long, french: Boolean, zone: ZoneId = ZoneId.systemDefault()): String {
        val d: DayOfWeek = Instant.ofEpochSecond(at).atZone(zone).dayOfWeek
        return d.getDisplayName(TextStyle.FULL, if (french) Locale.FRENCH else Locale.ENGLISH)
    }
}
