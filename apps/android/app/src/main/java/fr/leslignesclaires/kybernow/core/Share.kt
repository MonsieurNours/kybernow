package fr.leslignesclaires.kybernow.core

import fr.leslignesclaires.kybernow.model.AutoQuality
import fr.leslignesclaires.kybernow.model.Decoders
import fr.leslignesclaires.kybernow.model.ScreenMode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

/*
 * KyberNow Share (docs/kybernow-share.md) vu de l'app : les types du partage et « Rejoindre ».
 * Le cœur fait tout le travail (l'échange avec Admin, la session d'invité, les droits, les textes
 * des refus et des fins) ; ce fichier range ses réponses JSON pour l'interface.
 */

/**
 * Le rôle d'un invité : deux droits indépendants, les manettes et le clavier avec la souris (`ShareRole` du
 * cœur, mêmes codes : 0 spectateur, aucun droit ; 1 les manettes ; 2 les manettes, le clavier et la souris ;
 * 3 le clavier et la souris, sans les manettes). Chaque droit se donne ou se reprend sans toucher à l'autre
 * ([withPads], [withKeyboardMouse]).
 */
enum class ShareRole(val code: Int, val id: String) {
    SPECTATOR(0, "spectator"),
    PLAYER(1, "player"),
    PLAYER_KBM(2, "player_kbm"),
    KBM(3, "kbm");

    /** Les manettes de l'invité partent vers l'ordinateur (et la manette à l'écran a un sens). */
    val pads get() = this == PLAYER || this == PLAYER_KBM

    /** Le clavier, la souris et le toucher de l'invité partent vers l'ordinateur. */
    val keyboardMouse get() = this == KBM || this == PLAYER_KBM

    /** Un droit au moins : l'invité a une chaîne d'entrées, et la barre le dit « Joueur » (« Spectateur » sinon). */
    val plays get() = pads || keyboardMouse

    /** « Donner les manettes » (`true`) ou « Reprendre les manettes », le clavier et la souris tels quels. */
    fun withPads(on: Boolean) = of(on, keyboardMouse)

    /** « Donner le clavier et la souris » ou « Reprendre le clavier et la souris », les manettes telles quelles. */
    fun withKeyboardMouse(on: Boolean) = of(pads, on)

    companion object {
        fun of(code: Int) = entries.firstOrNull { it.code == code } ?: SPECTATOR

        /** Le rôle de ces deux droits. */
        fun of(pads: Boolean, keyboardMouse: Boolean) = when {
            pads && keyboardMouse -> PLAYER_KBM
            pads -> PLAYER
            keyboardMouse -> KBM
            else -> SPECTATOR
        }

        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: SPECTATOR
    }
}

/**
 * La « Qualité de l'image » qu'un invité demande (`ShareQuality` du cœur, mêmes codes : 0 Auto, 1 comme l'hôte,
 * 2 1080p · 60 Hz, 3 720p · 30 Hz, 4 4K · 60 Hz ; [id] : le texte du fil). L'ordre des cas est celui des choix
 * (Auto, comme l'hôte, 4K, 1080p, 720p) ; les codes ne le suivent pas, la 4K est venue après. Une qualité inconnue
 * (Station plus récente) se lit « Auto ». L'image reçue n'est jamais au-dessus de celle de l'hôte : c'est la Station
 * qui tranche, « Auto » aussi.
 */
enum class ShareQuality(val code: Int, val id: String) {
    AUTO(0, "auto"),
    HOST(1, "host"),
    P2160_60(4, "2160p60"),
    P1080_60(2, "1080p60"),
    P720_30(3, "720p30");

    companion object {
        fun of(code: Int) = entries.firstOrNull { it.code == code } ?: AUTO

        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: AUTO
    }
}

/**
 * L'image qu'un invité reçoit (`ShareImage` du cœur : `image` du signe de vie et de « Rejoindre », `guests[].image`
 * chez l'hôte). [quality] est la qualité demandée (« Auto » compris) ; tout le reste vaut `null` tant que Kyber ne
 * sait rien (relu toutes les 2 s au plus), et alors l'image compte comme celle de l'hôte. [label] est écrit par le
 * cœur (« 1080p · 60 Hz ») et n'existe que si la taille est connue : sans libellé, l'interface ne montre rien.
 */
data class ShareImage(
    val quality: ShareQuality,
    val width: Int? = null,
    val height: Int? = null,
    val fps: Int? = null,
    val codec: String? = null,
    val followsHost: Boolean? = null,
    val reason: String? = null,
    val label: String? = null,
) {
    /** Une image à part (à lui ou à ceux de même qualité) : su, et pas celle de l'hôte. Inconnu : celle de l'hôte. */
    val own get() = followsHost == false

    companion object {
        /** [label] donne le texte du cœur pour ce JSON (`Native.shareImageLabel` ; les essais sans bibliothèque native en donnent un). */
        fun parse(o: JSONObject, label: (String) -> String? = Share::imageLabel): ShareImage {
            fun int(key: String) = (o.opt(key) as? Number)?.toInt()
            return ShareImage(
                quality = ShareQuality.parse(o.optString("quality")),
                width = int("width"),
                height = int("height"),
                fps = int("fps"),
                codec = (o.opt("codec") as? String)?.takeIf { it.isNotEmpty() },
                followsHost = o.opt("follows_host") as? Boolean,
                reason = (o.opt("reason") as? String)?.takeIf { it.isNotEmpty() },
                // « 1080p · 60 Hz » : jamais deviné ici, seulement ce que le cœur écrit quand il sait la taille (avec
                // l'espace insécable de l'app avant « Hz », comme les autres textes : jamais « 60 » seul en fin de ligne).
                label = runCatching { label(o.toString()) }.getOrNull()?.takeIf { it.isNotBlank() }?.replace(" Hz", "\u00a0Hz"),
            )
        }

        fun parse(json: String?, label: (String) -> String? = Share::imageLabel): ShareImage? =
            runCatching { json?.let { parse(JSONObject(it), label) } }.getOrNull()
    }
}

/**
 * Un invité, dans la liste de l'hôte : [app] (`mac`, `windows`, `linux`, `ios`, `tvos`, `android`,
 * `webos`, `web`), [since], son arrivée en secondes depuis 1970 (l'horloge de la Station ; `null`
 * si elle ne la donne pas), [image], celle qu'il reçoit (`null` si la Station n'en dit rien), [via], par où il est
 * venu, et [pending] : une demande venue par le relais que l'hôte n'a pas encore acceptée (spectatrice, sans place ;
 * [since] est l'heure de la demande).
 */
data class ShareGuest(
    val id: String,
    val name: String,
    val app: String,
    val role: ShareRole,
    val since: Long? = null,
    val image: ShareImage? = null,
    val pending: Boolean = false,
    val via: ShareVia = ShareVia.LAN,
) {
    /**
     * Sa qualité d'image pour le panneau de l'hôte (« 1080p · 60 Hz »), seulement si ce n'est pas celle de l'hôte et
     * si le cœur sait l'écrire : une image encore inconnue compte comme celle de l'hôte.
     */
    val ownImageLabel get() = image?.takeIf { it.own }?.label

    /** Venu par le relais : « hors de chez vous ». */
    val away get() = via == ShareVia.RELAY

    companion object {
        fun parse(o: JSONObject, label: (String) -> String? = Share::imageLabel) = ShareGuest(
            o.optString("id"), o.optString("name"), o.optString("app"), ShareRole.parse(o.optString("role")),
            since = if (o.isNull("since") || !o.has("since")) null else o.optLong("since").takeIf { it > 0 },
            image = o.optJSONObject("image")?.let { ShareImage.parse(it, label) },
            pending = o.optBoolean("pending"),
            via = ShareVia.parse(o.optString("via")),
        )
    }
}

/** Par où un invité est venu (`via` de la liste de l'hôte) : le réseau de l'ordinateur, ou le relais (hors de chez soi). */
enum class ShareVia(val id: String) {
    LAN("lan"),
    RELAY("relay");

    companion object {
        /** Une voie inconnue (Station plus récente) ou absente (Station d'avant le relais) est le réseau. */
        fun parse(id: String?) = entries.firstOrNull { it.id == id } ?: LAN
    }
}

/**
 * Ce que le relais fait du partage (`relay.state` de la vue d'Admin, docs/relais.md) : [JOINED], le code est réservé
 * chez le relais et le lien vaut hors de chez soi ; [UNREACHABLE], pas encore. Un partage sans relais réglé n'a aucun
 * état (`ShareState.relay` nul).
 */
enum class ShareRelay {
    JOINED,
    UNREACHABLE;

    companion object {
        /** Un état inconnu n'est pas « joint ». */
        fun parse(o: JSONObject?): ShareRelay? = o?.let { if (it.optString("state") == "joined") JOINED else UNREACHABLE }
    }
}

/**
 * Le partage ouvert par l'hôte (`ShareOpened` et `ShareState` du cœur) : le code ([code], et [codeText]
 * tel qu'on l'affiche, « 123 456 »), le lien du QR code, ses invités ([guests] : les invités acceptés, quatre au
 * plus) et ses demandes ([requests] : venues par le relais, pas encore acceptées ; elles ne comptent pas dans les
 * invités). [relay] : ce que le relais fait du partage.
 */
data class ShareState(
    val code: String,
    val codeText: String,
    val link: String,
    val web: String = "",
    val guests: List<ShareGuest> = emptyList(),
    val relay: ShareRelay? = null,
    val requests: List<ShareGuest> = emptyList(),
) {
    val full get() = guests.size >= MAX_GUESTS

    /** Un invité accepté d'après son identifiant ; `null` s'il est parti. */
    fun guest(id: String) = guests.firstOrNull { it.id == id }

    companion object {
        const val MAX_GUESTS = 4

        /**
         * `null` sans partage (`open` faux, ou une réponse qui n'est pas un objet). [format] écrit le code
         * comme on l'affiche, [label] la qualité d'image d'un invité (le cœur ; les essais sans bibliothèque
         * native en donnent un).
         */
        fun parse(o: JSONObject?, format: (String) -> String = Share::formatCode, label: (String) -> String? = Share::imageLabel): ShareState? {
            if (o == null || (o.has("open") && !o.optBoolean("open"))) return null
            val code = o.optString("code")
            if (code.isEmpty()) return null
            val list = o.optJSONArray("guests")
            val all = if (list == null) emptyList() else (0 until list.length()).mapNotNull { i -> list.optJSONObject(i)?.let { g -> ShareGuest.parse(g, label) } }
            return ShareState(
                code, format(code), o.optString("link"), o.optString("web"),
                guests = all.filterNot { it.pending }.take(MAX_GUESTS),
                relay = ShareRelay.parse(o.optJSONObject("relay")),
                requests = all.filter { it.pending },
            )
        }

        fun parse(json: String?, format: (String) -> String = Share::formatCode, label: (String) -> String? = Share::imageLabel): ShareState? =
            runCatching { json?.let { parse(JSONObject(it), format, label) } }.getOrNull()
    }
}

/** Ce que « Partager » peut faire avec l'ordinateur (`ShareAvailability` du cœur, d'après `info.share`). */
enum class ShareAvailability(val code: Int) {
    /** Station antérieure à la 0.8.0. */
    UNSUPPORTED(0),
    /** Partage coupé sur l'ordinateur (Admin › Appareils Link). */
    DISABLED(1),
    AVAILABLE(2);

    companion object {
        fun of(code: Int) = entries.firstOrNull { it.code == code } ?: UNSUPPORTED
    }
}

/** Une Station à qui envoyer le code : l'adresse et le port d'Admin, et son empreinte si on la connaît. */
data class ShareTarget(val host: String, val port: Int, val fingerprint: String?) {
    fun json(): JSONObject = JSONObject().put("host", host).put("port", port).put("fingerprint", fingerprint ?: JSONObject.NULL)

    companion object {
        fun parse(o: JSONObject) = ShareTarget(o.optString("host"), o.optInt("port"), if (o.isNull("fingerprint")) null else o.optString("fingerprint").takeIf { it.isNotEmpty() })
    }
}

/**
 * Un lien de partage lu (`kybernow://join?host=…&port=…&fp=…&code=…`) : où et avec quel code. Un partage dont le relais
 * est joint ajoute `relay` ([relay], `hôte:port`) et `sid` ([sid], l'identifiant de la Station chez le relais) : le
 * réseau d'abord, puis le relais, Admin épinglé sur `fp` (docs/relais.md, « Liens »).
 */
data class ShareLink(val target: ShareTarget, val code: String, val relay: String? = null, val sid: String? = null) {
    /** Le lien tel que le cœur le lit (`JoinLink` : `{host, port, fingerprint, code, relay, sid}`), pour `shareJoinLink`. */
    fun json(): JSONObject = target.json().put("code", code).put("relay", relay ?: JSONObject.NULL).put("sid", sid ?: JSONObject.NULL)
}

/**
 * Une demande par le relais, en attente de l'accord de l'hôte (« En attente de l'accord de l'hôte… ») : [json] est le
 * `pending` du cœur, donné tel quel à `shareWait` et à `shareCancel`. Il porte le secret de la demande : il ne se
 * garde nulle part et ne s'écrit jamais dans un journal. [expires] : le délai sans réponse, en secondes (120).
 */
class PendingJoin(val json: String, val expires: Int) {
    /** « Annuler » a déjà été demandé ([Share.cancel]). */
    internal val cancelled = java.util.concurrent.atomic.AtomicBoolean(false)

    override fun toString() = "PendingJoin(en attente de l'hôte)"

    companion object {
        fun parse(o: JSONObject) = PendingJoin(o.toString(), o.optInt("expires", 120))
    }
}

/**
 * La Station qui a accepté le code, et sa réponse. [json] est la `value` de `shareJoin`, donnée telle
 * quelle à la session (`guest.joined`) et à `shareLeave` : elle porte les jetons, elle ne se garde
 * nulle part et ne s'écrit jamais dans un journal.
 */
class ShareFound(
    val target: ShareTarget,
    /** Le nom de l'ordinateur (« Invité sur {nom} »). */
    val computer: String,
    /** Le prénom de l'hôte, ou le nom de l'ordinateur pour un partage lancé depuis le PC. */
    val host: String,
    val role: ShareRole,
    /** L'écran partagé : définition et fréquence de l'image. */
    val width: Int,
    val height: Int,
    val refresh: Int,
    val json: String,
) {
    /** Le JSON `joined` de la session d'invité. */
    val joined: JSONObject get() = JSONObject(json).getJSONObject("joined")

    override fun toString() = "ShareFound(${target.host}:${target.port}, $computer, $role)"

    companion object {
        fun parse(value: JSONObject): ShareFound {
            val joined = value.getJSONObject("joined")
            val display = joined.optJSONObject("display")
            return ShareFound(
                target = ShareTarget.parse(value.getJSONObject("target")),
                computer = joined.optString("computer"),
                host = joined.optString("host"),
                role = ShareRole.parse(joined.optString("role")),
                width = display?.optInt("width") ?: 0,
                height = display?.optInt("height") ?: 0,
                refresh = display?.optInt("refresh") ?: 0,
                json = value.toString(),
            )
        }
    }
}

/**
 * Réponse de « Rejoindre » (et de l'attente de l'hôte) : la Station trouvée, la demande en attente de l'accord de
 * l'hôte (hors de chez soi), ou le texte du cœur (`error`, dans la langue de l'app).
 */
sealed class JoinReply {
    data class Joined(val found: ShareFound) : JoinReply()

    /** Venu par le relais : « En attente de l'accord de l'hôte… », jusqu'à [Share.waitForHost] ou « Annuler » ([Share.cancel]). */
    data class Pending(val pending: PendingJoin) : JoinReply()

    /**
     * [kind] : `no_share`, `full`, `too_many`, `refused` (« L'hôte a refusé. »), `expired` (« L'hôte n'a pas
     * répondu. »), `pending_full` (trois demandes attendent déjà l'hôte), `relay` (« Le relais ne répond pas. », ou
     * l'ordinateur absent du relais), `other` (et `unreadable`, seulement si l'appareil ne lit aucun format possible,
     * ce que H.264 empêche : jamais en pratique) ; [message] : le texte à montrer.
     */
    data class Refused(val kind: String, val message: String) : JoinReply()
}

object Share {
    /** Six chiffres au plus, tels qu'on les tape ; pas de formatage ni de séparateur. */
    fun digits(text: String): String = text.filter { it in '0'..'9' }.take(6)

    /** Le code tel qu'on l'affiche : « 123 456 » (`share::format_code` du cœur). */
    fun formatCode(code: String): String = Native.shareFormatCode(code)

    /** Le lien d'un partage ouvert, scanné ou ouvert depuis une autre app ; `null` si ce n'en est pas un. */
    fun parseLink(url: String): ShareLink? {
        val json = Native.shareParseLink(url) ?: return null
        return runCatching {
            val o = JSONObject(json)
            fun text(key: String) = if (o.isNull(key)) null else o.optString(key).takeIf { it.isNotEmpty() }
            ShareLink(ShareTarget(o.getString("host"), o.getInt("port"), text("fingerprint")), o.getString("code"), relay = text("relay"), sid = text("sid"))
        }.getOrNull()
    }

    /**
     * `capabilities` : ce que l'appareil décode. Le cœur demande à Kyber le format de l'hôte si l'appareil le lit,
     * sinon un format qu'il lit (une image à part) : le format de l'hôte ne refuse plus personne.
     */
    private fun capabilities(d: Decoders) = JSONObject().put("hevc", d.hevc != null).put("hevc444", d.hevc444).put("av1", d.av1 != null)

    /**
     * Le `request` de « Rejoindre » : `{code, name, device, app, quality}` et `auto` (ce que l'appareil sait afficher et
     * décoder, [autoQuality]), quel que soit le choix : la Station l'ignore hors « Auto », et le cœur le garde pour un
     * passage à « Auto » en cours de session ; sans `auto`, « Auto » garde l'image de l'hôte.
     */
    internal fun joinRequest(code: String, name: String, deviceId: String, quality: ShareQuality, auto: JSONObject?): JSONObject =
        JSONObject().put("code", code).put("name", name).put("device", deviceId).put("app", "android").put("quality", quality.id)
            .also { if (auto != null) it.put("auto", auto) }

    /**
     * « Auto » : l'écran de l'appareil ([screen] : sa définition native et sa fréquence) et ce que son décodeur suit
     * (`MediaCodec`, [AutoQuality.limits]) ; le cœur (`share::auto_quality`) en tire le `auto` de [join], avec les
     * formats lus. `null` si le cœur ne lit pas la mesure : « Auto » part alors sans `auto` (l'image de l'hôte).
     */
    fun autoQuality(screen: ScreenMode, decoders: Decoders): JSONObject? {
        val limits = AutoQuality.limits(screen, decoders.any, decoders.follows())
        val json = Native.shareAutoQuality(AutoQuality.screenJson(screen).toString(), AutoQuality.decoderJson(decoders, limits).toString(), null) ?: return null
        return runCatching { JSONObject(json) }.getOrNull()
    }

    /** « 1080p · 60 Hz » d'une image reçue (`ShareImage::label` du cœur) ; `null` tant que la taille n'est pas sue. */
    fun imageLabel(imageJson: String): String? = Native.shareImageLabel(imageJson)

    /**
     * Envoie [code] à ces Stations à la fois et rend la première qui l'accepte (6 s au plus), avec la
     * « Qualité de l'image » [quality] (et, pour « Auto », [auto] : [autoQuality]). Si l'appelant renonce
     * pendant l'attente, l'invité repart tout de suite au lieu d'attendre la fin de son signe de vie.
     */
    suspend fun join(
        targets: List<ShareTarget>, code: String, name: String, device: DeviceIdentity, decoders: Decoders,
        quality: ShareQuality = ShareQuality.AUTO, auto: JSONObject? = null, relays: List<String> = emptyList(),
    ): JoinReply {
        val request = joinRequest(code, name, device.id, quality, auto)
        val list = JSONArray().apply { targets.forEach { put(it.json()) } }
        val servers = JSONArray().apply { relays.distinct().forEach { put(it) } }
        val text = withContext(Dispatchers.IO) { Native.shareJoinAnywhere(list.toString(), servers.toString(), request.toString(), capabilities(decoders).toString()) }
        return settle(readJoin(text))
    }

    /**
     * Rejoindre avec le lien (ou le QR code) d'un partage : la Station du lien sur le réseau d'abord, puis, si le
     * lien porte `relay` et `sid`, par le relais, de bout en bout (Admin épinglé sur `fp`). [code] : celui qui est
     * tapé à l'écran (le lien le donne au départ).
     */
    suspend fun joinLink(
        link: ShareLink, code: String, name: String, device: DeviceIdentity, decoders: Decoders,
        quality: ShareQuality = ShareQuality.AUTO, auto: JSONObject? = null,
    ): JoinReply {
        val request = joinRequest(code, name, device.id, quality, auto)
        val text = withContext(Dispatchers.IO) { Native.shareJoinLink(link.copy(code = code).json().toString(), request.toString(), capabilities(decoders).toString()) }
        return settle(readJoin(text))
    }

    /**
     * La réponse du cœur à « Rejoindre » (`shareJoinAnywhere`, `shareJoinLink`) : `value` est `{"joined": …}` ou
     * `{"pending": …}`. Une réponse illisible ne fait pas tomber l'app : un refus sans texte (l'écran en met un).
     */
    internal fun readJoin(text: String?): JoinReply {
        val o = runCatching { JSONObject(text ?: "") }.getOrNull() ?: return JoinReply.Refused("other", "")
        if (o.optInt("code", 1) != 0) return refusal(o)
        val value = o.optJSONObject("value") ?: return JoinReply.Refused("other", "")
        value.optJSONObject("pending")?.let { return JoinReply.Pending(PendingJoin.parse(it)) }
        val joined = value.optJSONObject("joined") ?: return JoinReply.Refused("other", "")
        return runCatching { JoinReply.Joined(ShareFound.parse(joined)) }.getOrNull() ?: JoinReply.Refused("other", "")
    }

    /** Un refus du cœur : son genre (`kind`), ou `relay` pour un relais muet (30) ou un ordinateur absent du relais (31) ; son texte. */
    private fun refusal(o: JSONObject): JoinReply.Refused {
        val code = o.optInt("code", 1)
        val kind = o.optString("kind").takeIf { it.isNotEmpty() } ?: if (code == RELAY_UNREACHABLE || code == STATION_OFFLINE) "relay" else "other"
        return JoinReply.Refused(kind, o.optString("error"))
    }

    /**
     * Si l'appelant a renoncé pendant l'attente, l'invité repart tout de suite au lieu d'attendre la fin de son signe
     * de vie, et une demande en attente est retirée de la liste de l'hôte.
     */
    private suspend fun settle(reply: JoinReply): JoinReply {
        if (currentCoroutineContext().isActive) return reply
        when (reply) {
            is JoinReply.Joined -> leave(reply.found)
            is JoinReply.Pending -> cancel(reply.pending)
            is JoinReply.Refused -> {}
        }
        throw CancellationException("rejoindre annulé")
    }

    /**
     * « En attente de l'accord de l'hôte… » : redemande à l'hôte (un appel tenu 25 s à chaque fois) jusqu'à sa réponse :
     * [JoinReply.Joined] (accepté : la session s'ouvre, par le relais), ou un refus (`refused` « L'hôte a refusé. »,
     * `expired` « L'hôte n'a pas répondu. » au bout de 120 s, `no_share` si le partage s'arrête, `relay` si le relais
     * ne répond plus). Un à-coup du réseau (le relais muet une ou deux fois de suite) ne coupe pas l'attente. Annulée
     * (« Annuler », l'écran qui se ferme), la demande est retirée chez l'hôte ([cancel]).
     */
    suspend fun waitForHost(pending: PendingJoin, decoders: Decoders): JoinReply {
        val caps = capabilities(decoders).toString()
        var failures = 0
        try {
            while (true) {
                val text = withContext(Dispatchers.IO) { Native.shareWait(pending.json, caps) }
                val o = runCatching { JSONObject(text) }.getOrNull() ?: return JoinReply.Refused("other", "")
                when (val code = o.optInt("code", 1)) {
                    0 -> {
                        val found = runCatching { ShareFound.parse(o.getJSONObject("value")) }.getOrNull() ?: return JoinReply.Refused("other", "")
                        if (!currentCoroutineContext().isActive) {
                            leave(found)
                            throw CancellationException("rejoindre annulé")
                        }
                        return JoinReply.Joined(found)
                    }
                    WAITING -> failures = 0
                    RELAY_UNREACHABLE -> if (++failures >= 3) return refusal(o) else delay(2000)
                    else -> return refusal(o).also { android.util.Log.i("KyberNow", "partage : attente finie (code $code)") }
                }
                currentCoroutineContext().ensureActive()
            }
        } catch (e: CancellationException) {
            cancel(pending)
            throw e
        }
    }

    /** « Annuler » : la demande est retirée chez l'hôte (bloquant : sur son propre fil). Sans effet la seconde fois. */
    fun cancel(pending: PendingJoin) {
        if (pending.cancelled.getAndSet(true)) return
        Thread({ Native.shareCancel(pending.json) }, "kybernow-partage-annuler").start()
    }

    /** L'invité repart avant d'avoir ouvert sa session (bloquant : hors du fil principal). */
    fun leave(found: ShareFound) {
        Thread({ Native.shareLeave(found.json) }, "kybernow-partage-depart").start()
    }

    /** Codes du cœur : `shareWait` toujours en attente ; relais muet ; ordinateur absent du relais. */
    private const val WAITING = 27
    private const val RELAY_UNREACHABLE = 30
    private const val STATION_OFFLINE = 31

    /** La capacité de « Partager » d'après l'info d'Admin (`HostInfo.raw`). */
    fun availability(infoJson: String): ShareAvailability = ShareAvailability.of(Native.shareAvailability(infoJson))

    /** Le texte de [availability] quand « Partager » est inactif ; `null` s'il est disponible. */
    fun availabilityText(a: ShareAvailability): String? = if (a == ShareAvailability.AVAILABLE) null else Native.shareAvailabilityText(a.code)
}
