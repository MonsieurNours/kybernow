package fr.leslignesclaires.kybernow.core

import fr.leslignesclaires.kybernow.model.Decoders
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
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
 * Un invité, dans la liste de l'hôte : [app] (`mac`, `windows`, `linux`, `ios`, `tvos`, `android`,
 * `webos`, `web`) et [since], son arrivée en secondes depuis 1970 (l'horloge de la Station ; `null`
 * si elle ne la donne pas).
 */
data class ShareGuest(val id: String, val name: String, val app: String, val role: ShareRole, val since: Long? = null) {
    companion object {
        fun parse(o: JSONObject) = ShareGuest(
            o.optString("id"), o.optString("name"), o.optString("app"), ShareRole.parse(o.optString("role")),
            since = if (o.isNull("since") || !o.has("since")) null else o.optLong("since").takeIf { it > 0 },
        )
    }
}

/**
 * Le partage ouvert par l'hôte (`ShareOpened` et `ShareState` du cœur) : le code ([code], et [codeText]
 * tel qu'on l'affiche, « 123 456 »), le lien du QR code, ses invités. Quatre invités au plus.
 */
data class ShareState(
    val code: String,
    val codeText: String,
    val link: String,
    val web: String = "",
    val guests: List<ShareGuest> = emptyList(),
) {
    val full get() = guests.size >= MAX_GUESTS

    /** Un invité d'après son identifiant ; `null` s'il est parti. */
    fun guest(id: String) = guests.firstOrNull { it.id == id }

    companion object {
        const val MAX_GUESTS = 4

        /**
         * `null` sans partage (`open` faux, ou une réponse qui n'est pas un objet). [format] écrit le code
         * comme on l'affiche (le cœur ; les essais sans bibliothèque native en donnent un).
         */
        fun parse(o: JSONObject?, format: (String) -> String = Share::formatCode): ShareState? {
            if (o == null || (o.has("open") && !o.optBoolean("open"))) return null
            val code = o.optString("code")
            if (code.isEmpty()) return null
            val list = o.optJSONArray("guests")
            val guests = if (list == null) emptyList() else (0 until list.length()).mapNotNull { i -> list.optJSONObject(i)?.let(ShareGuest::parse) }
            return ShareState(code, format(code), o.optString("link"), o.optString("web"), guests.take(MAX_GUESTS))
        }

        fun parse(json: String?, format: (String) -> String = Share::formatCode): ShareState? =
            runCatching { json?.let { parse(JSONObject(it), format) } }.getOrNull()
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

/** Un lien de partage lu (`kybernow://join?host=…&port=…&fp=…&code=…`) : où et avec quel code. */
data class ShareLink(val target: ShareTarget, val code: String)

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

/** Réponse de « Rejoindre » : la Station trouvée, ou le texte du cœur (`error`, dans la langue de l'app). */
sealed class JoinReply {
    data class Joined(val found: ShareFound) : JoinReply()

    /** [kind] : `no_share`, `full`, `too_many`, `unreadable`, `other` ; [message] : le texte à montrer. */
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
            ShareLink(ShareTarget(o.getString("host"), o.getInt("port"), if (o.isNull("fingerprint")) null else o.optString("fingerprint").takeIf { it.isNotEmpty() }), o.getString("code"))
        }.getOrNull()
    }

    /** `request` : `{code, name, device, app}` ; `capabilities` : ce que l'appareil décode (le format de l'hôte est vérifié avant d'entrer). */
    private fun capabilities(d: Decoders) = JSONObject().put("hevc", d.hevc != null).put("hevc444", d.hevc444).put("av1", d.av1 != null)

    /**
     * Envoie [code] à ces Stations à la fois et rend la première qui l'accepte (6 s au plus). Si
     * l'appelant renonce pendant l'attente, l'invité repart tout de suite au lieu d'attendre la fin
     * de son signe de vie.
     */
    suspend fun join(targets: List<ShareTarget>, code: String, name: String, device: DeviceIdentity, decoders: Decoders): JoinReply {
        val request = JSONObject().put("code", code).put("name", name).put("device", device.id).put("app", "android")
        val list = JSONArray().apply { targets.forEach { put(it.json()) } }
        val text = withContext(Dispatchers.IO) { Native.shareJoin(list.toString(), request.toString(), capabilities(decoders).toString()) }
        // Une réponse illisible ne fait pas tomber l'app : un refus sans texte (l'écran en met un).
        val o = runCatching { JSONObject(text) }.getOrNull() ?: return JoinReply.Refused("other", "")
        if (o.optInt("code", 1) != 0) return JoinReply.Refused(o.optString("kind", "other"), o.optString("error"))
        val found = runCatching { ShareFound.parse(o.getJSONObject("value")) }.getOrNull() ?: return JoinReply.Refused("other", "")
        if (!currentCoroutineContext().isActive) {
            leave(found)
            throw CancellationException("rejoindre annulé")
        }
        return JoinReply.Joined(found)
    }

    /** L'invité repart avant d'avoir ouvert sa session (bloquant : hors du fil principal). */
    fun leave(found: ShareFound) {
        Thread({ Native.shareLeave(found.json) }, "kybernow-partage-depart").start()
    }

    /** La capacité de « Partager » d'après l'info d'Admin (`HostInfo.raw`). */
    fun availability(infoJson: String): ShareAvailability = ShareAvailability.of(Native.shareAvailability(infoJson))

    /** Le texte de [availability] quand « Partager » est inactif ; `null` s'il est disponible. */
    fun availabilityText(a: ShareAvailability): String? = if (a == ShareAvailability.AVAILABLE) null else Native.shareAvailabilityText(a.code)
}
