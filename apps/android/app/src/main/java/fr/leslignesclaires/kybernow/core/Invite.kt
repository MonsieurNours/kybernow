package fr.leslignesclaires.kybernow.core

import fr.leslignesclaires.kybernow.model.Host
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject

/*
 * Les invitations vues de l'app (docs/invitations.md) : ajouter un ordinateur de loin avec le code et le mot de passe
 * d'une invitation, sans que personne ait à l'accepter sur place. « Rejoindre avec un code » reconnaît le code (code 33
 * du cœur, [JoinReply.Invite]) ; l'app demande le mot de passe (« Ajouter {ordinateur} ») et le cœur rend l'ordinateur à
 * garder comme un ordinateur appairé ([Invite.redeem]). Ce fichier range ses réponses JSON pour l'interface.
 */

private fun JSONObject.text(key: String): String? = if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }

/**
 * « Rejoindre avec un code » est tombé sur un code d'invitation (code 33, `InviteCode` du cœur) : le code, la Station qui
 * l'a ([target]), sa route par le relais ([route], `{server, station, admin_fp}`, nulle sur le réseau) et le nom de
 * l'ordinateur ([computer], vide pour une Station qui ne le dit pas). [json] repart tel quel au cœur ([Invite.redeem]).
 */
class InviteCode(val code: String, val target: ShareTarget, val route: String?, val computer: String, val json: String) {
    override fun toString() = "InviteCode(${target.host}:${target.port}, $computer)"

    companion object {
        fun parse(o: JSONObject): InviteCode? {
            val code = o.text("code") ?: return null
            val target = o.optJSONObject("target")?.let(ShareTarget::parse) ?: return null
            return InviteCode(code, target, o.optJSONObject("route")?.toString(), o.text("computer") ?: "", o.toString())
        }
    }
}

/**
 * L'ordinateur ajouté par l'invitation (`Invited` du cœur) : à garder comme un ordinateur appairé : son nom, l'adresse et
 * le port d'Admin, l'empreinte d'Admin (épinglée), le compte propre à cet appareil ([username], [password]) et le relais
 * de la Station ([relay], pour l'ouvrir hors de chez soi) ; le nom de l'invitation et son échéance (secondes Unix de la
 * Station : l'accès tombe avec elle). Il porte le mot de passe du compte : jamais dans un journal.
 */
class Invited(
    val hostname: String,
    val address: String,
    val port: Int,
    val fingerprint: String?,
    val username: String,
    val password: String,
    val relay: HostRelay?,
    val inviteName: String,
    val expires: Long,
    val viaRelay: Boolean,
) {
    override fun toString() = "Invited($hostname, $address:$port)"

    /**
     * L'ordinateur à garder : un ordinateur appairé (compte propre à l'appareil), avec son relais. Le même ordinateur déjà
     * sur l'accueil (même empreinte d'Admin, sinon même adresse et même port) prend le nouvel accès au lieu d'être ajouté
     * deux fois : il garde sa carte (identifiant, réglages, dernière session, réveil) ; [fallbackName] sert si la Station
     * ne dit pas son nom.
     */
    fun host(existing: List<Host>, fallbackName: String = ""): Host {
        val fp = fingerprint?.trim()?.takeIf { it.isNotEmpty() }
        val same = existing.firstOrNull { Core.sameFingerprint(it.fingerprint, fp) } ?: existing.firstOrNull { it.address == address && it.port == port }
        val name = hostname.ifBlank { same?.name ?: fallbackName }.ifBlank { address }
        if (same != null) {
            return same.copy(
                name = name, address = address, port = port, fingerprint = fp ?: same.fingerprint, user = username, paired = true,
                relay = relay ?: same.relay,
            )
        }
        return Host(name = name, address = address, port = port, fingerprint = fp, user = username, paired = true, relay = relay)
    }

    companion object {
        fun parse(o: JSONObject): Invited? {
            val username = o.text("username") ?: return null
            return Invited(
                hostname = o.optString("hostname"),
                address = o.optString("address"),
                port = o.optInt("port"),
                fingerprint = o.text("fingerprint"),
                username = username,
                password = o.optString("password"),
                relay = HostRelay.parse(o.optJSONObject("relay")),
                inviteName = o.optString("invite_name"),
                expires = o.optLong("expires"),
                viaRelay = o.optBoolean("via_relay"),
            )
        }
    }
}

/** Réponse de « Ajouter l'ordinateur ». */
sealed class InviteReply {
    data class Added(val invited: Invited) : InviteReply()

    /**
     * [code] : celui du cœur (`InviteError::code` : [Invite.BAD_PASSWORD] et suivants, 1 pour une demande illisible) ;
     * [kind] : `bad_password`, `no_invite`, `full`, `too_many`, `access_off`, `other` ; [message] : le texte à montrer
     * sous le champ, dans la langue de l'app (« Mot de passe refusé. »…).
     */
    data class Refused(val code: Int, val kind: String, val message: String) : InviteReply()
}

object Invite {
    /**
     * « Ajouter l'ordinateur » : le mot de passe de l'invitation (tel que tapé : tirets, espaces et majuscules ne
     * comptent pas), par le même chemin que « Rejoindre » (le réseau, ou le relais de bout en bout, Admin épinglé). Hors
     * du fil principal (15 s au plus).
     */
    suspend fun redeem(code: InviteCode, password: String, device: DeviceIdentity): InviteReply {
        val text = withContext(Dispatchers.IO) { Native.inviteRedeem(code.json, password, device.json()) }
        return readRedeem(text)
    }

    /** La réponse du cœur à [redeem]. Illisible : un refus sans texte (l'écran en met un), jamais une chute de l'app. */
    internal fun readRedeem(text: String?): InviteReply {
        val o = runCatching { JSONObject(text ?: "") }.getOrNull() ?: return InviteReply.Refused(1, "other", "")
        val code = o.optInt("code", 1)
        if (code == 0) {
            return o.optJSONObject("value")?.let(Invited::parse)?.let { InviteReply.Added(it) } ?: InviteReply.Refused(1, "other", "")
        }
        return InviteReply.Refused(code, o.text("kind") ?: "other", o.optString("error"))
    }

    /** « Ajouter l'ordinateur » reste inactif tant que le champ est vide (ou n'a que des espaces et des tirets). */
    fun ready(password: String): Boolean = password.any { it != ' ' && it != '-' && !it.isWhitespace() }

    /** Codes du cœur (`InviteError::code`). */
    const val BAD_PASSWORD = 60
    const val NO_INVITE = 61
    const val FULL = 62
    const val TOO_MANY = 63
    const val ACCESS_OFF = 64
    const val OTHER = 65
}
