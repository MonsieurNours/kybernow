package fr.leslignesclaires.kybernow.model

import org.json.JSONObject

/*
 * « Envoyer le journal » sur Android TV (docs/link-ui.md, « Réglages de l'app › À propos » ; contrat : docs/relais.md,
 * « Dépôt d'un journal ») : pas de feuille de partage au téléviseur, le journal passe par le relais et un QR code que le
 * téléphone scanne. La compression, le chiffrement `KNJ1` (clé seulement dans le QR code) et les appels au relais
 * (empreinte épinglée d'un relais chez soi comprise) sont ceux du cœur (`journal::deposit`, `state`, `discard`, par
 * `Core.journalDeposit`…) ; ici, la lecture de ses réponses, essayée en JVM.
 */

/** Un dépôt fait : le relais tel que l'app l'a donné, de quoi suivre et retirer le dépôt, l'adresse du QR code (clé comprise). */
data class JournalDeposit(val relay: String, val id: String, val owner: String, val url: String, val expiresIn: Int)

/** La réponse du dépôt. */
sealed class JournalPost {
    data class Posted(val deposit: JournalDeposit) : JournalPost()
    /** `429` ou `503` : trop d'envois, ou relais plein. */
    data object TooMany : JournalPost()
    /** `404` : un relais d'avant, sans dépôt. */
    data object OldRelay : JournalPost()
    /** Le relais ne répond pas, une réponse illisible, ou rien n'a pu partir. */
    data object Unreachable : JournalPost()
}

/** L'état d'un dépôt (`GET /v1/journal/<id>/state`). */
enum class JournalState { WAITING, OPENED, GONE, UNKNOWN }

object JournalUpload {
    /** L'en-tête (appareil, version, fins précédentes : tout ce qui précède [Journal.AFTER_HEAD]) et le reste du journal. */
    fun split(text: ByteArray): Pair<String, String> {
        val end = Journal.headEnd(text)
        return text.copyOfRange(0, end).toString(Charsets.UTF_8) to text.copyOfRange(end, text.size).toString(Charsets.UTF_8)
    }

    /** La réponse du cœur au dépôt : `{relay, id, owner, url, expires_in}`, ou `{error, detail}` (90, 91, 92, 1). */
    fun readPost(json: JSONObject?): JournalPost {
        val o = json ?: return JournalPost.Unreachable
        if (o.has("error")) return when (o.optInt("error")) {
            91 -> JournalPost.OldRelay
            92 -> JournalPost.TooMany
            else -> JournalPost.Unreachable
        }
        val id = o.optString("id")
        val owner = o.optString("owner")
        val url = o.optString("url")
        if (id.isEmpty() || owner.isEmpty() || !url.startsWith("https://")) return JournalPost.Unreachable
        return JournalPost.Posted(JournalDeposit(o.optString("relay"), id, owner, url, o.optInt("expires_in", 3600).coerceIn(1, 3600)))
    }

    /** L'état d'un dépôt d'après le code du cœur (`journal::Remote::code`). */
    fun readState(code: Int): JournalState = when (code) {
        0 -> JournalState.WAITING
        1 -> JournalState.OPENED
        93 -> JournalState.GONE
        else -> JournalState.UNKNOWN
    }
}
