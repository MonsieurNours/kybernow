package fr.leslignesclaires.kybernow.model

import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.cert.X509Certificate
import java.util.Base64
import java.util.zip.GZIPInputStream
import java.util.zip.GZIPOutputStream
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec
import javax.net.ssl.HttpsURLConnection
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager

/*
 * « Envoyer le journal » sur Android TV (docs/link-ui.md, « Réglages de l'app › À propos » ; contrat : docs/relais.md,
 * « Dépôt d'un journal ») : pas de feuille de partage au téléviseur, le journal passe par le relais et un QR code que le
 * téléphone scanne. Le journal est compressé (gzip), chiffré ici en AES-256-GCM avec une clé tirée pour ce dépôt, et la
 * clé ne quitte l'appareil que dans le QR code, après `#` (le navigateur du téléphone ne l'envoie jamais) : le relais ne
 * peut pas le lire. [JournalSeal] est la partie pure (format, troncature, adresse, réponses), essayée en JVM ;
 * [JournalRelay] fait les appels, hors du fil principal.
 */

/** Le relais d'un dépôt : `hôte[:port]` (l'autorité des pages du relais) et, s'il en a une, l'empreinte épinglée. */
data class JournalRelay(val host: String, val port: Int, val pin: ByteArray?) {
    /** `hôte`, ou `hôte:port` hors du 443 ; une IPv6 entre crochets. */
    val authority: String
        get() {
            val h = if (host.contains(':')) "[$host]" else host
            return if (port == 443) h else "$h:$port"
        }

    override fun equals(other: Any?): Boolean =
        other is JournalRelay && host == other.host && port == other.port &&
            (if (pin == null || other.pin == null) pin == other.pin else pin.contentEquals(other.pin))

    override fun hashCode() = 31 * (31 * host.hashCode() + port) + (pin?.contentHashCode() ?: 0)

    companion object {
        /** Le relais public de KyberNow. */
        const val DEFAULT = "share.kybernow.app"

        /**
         * Lit un relais sous sa forme normale (`share.kybernow.app`, `relais.exemple.org:9443`,
         * `relais.exemple.org#sha256:aa:bb:…`, voir `rules::relay`) ; nul s'il ne se lit pas.
         */
        fun parse(addr: String): JournalRelay? {
            val s = addr.trim().removePrefix("https://").trimEnd('/')
            val main = s.substringBefore('#')
            val pin = if (s.contains('#')) parsePin(s.substringAfter('#')) ?: return null else null
            val host: String
            val port: Int
            if (main.startsWith("[")) {
                host = main.substring(1).substringBefore(']', "").ifEmpty { return null }
                val after = main.substringAfter(']')
                port = when {
                    after.isEmpty() -> 443
                    after.startsWith(":") -> after.drop(1).toIntOrNull() ?: return null
                    else -> return null
                }
            } else {
                val i = main.lastIndexOf(':')
                if (i >= 0 && main.indexOf(':') == i) {
                    host = main.substring(0, i)
                    port = main.substring(i + 1).toIntOrNull() ?: return null
                } else {
                    host = main
                    port = 443
                }
            }
            if (host.isBlank() || port !in 1..65535 || host.any { it.isWhitespace() || it == '/' || it == '@' }) return null
            return JournalRelay(host.lowercase(), port, pin)
        }

        /** `sha256:aa:bb:…` ou les seuls chiffres : les 32 octets ; nul si ce n'en est pas une. */
        private fun parsePin(text: String): ByteArray? {
            val hex = text.trim().lowercase().removePrefix("sha256:").filter { it != ':' && !it.isWhitespace() }
            if (hex.length != 64 || hex.any { it !in "0123456789abcdef" }) return null
            return ByteArray(32) { i -> hex.substring(2 * i, 2 * i + 2).toInt(16).toByte() }
        }
    }
}

/** La réponse du dépôt (`POST /v1/journal`). */
sealed class JournalPost {
    /** `201` : l'identifiant, le jeton de suivi (jamais montré) et la durée de vie, en secondes. */
    data class Posted(val id: String, val owner: String, val expiresIn: Int) : JournalPost()
    /** `429` : trop d'envois depuis cette adresse ; réessayer dans [retryAfter] secondes. */
    data class TooMany(val retryAfter: Int) : JournalPost()
    /** `503` : le relais garde déjà trop de dépôts. */
    data object Full : JournalPost()
    /** `413` : trop gros (jamais : l'app reste sous la limite). */
    data object TooLarge : JournalPost()
    /** `404` : un relais d'avant, sans dépôt. */
    data object OldRelay : JournalPost()
    /** Le relais ne répond pas, ou une réponse qui ne se lit pas. */
    data object Unreachable : JournalPost()
}

/** L'état d'un dépôt (`GET /v1/journal/<id>/state`). */
enum class JournalState { WAITING, OPENED, GONE, UNKNOWN }

object JournalSeal {
    /** Le début d'un dépôt. */
    val MAGIC = "KNJ1".toByteArray(Charsets.US_ASCII)
    const val KEY_SIZE = 32
    const val NONCE_SIZE = 12
    const val TAG_BITS = 128
    /** Le journal compressé tient en 2 Mio : le relais prend 2 Mio et 64 octets par dépôt (en-tête et étiquette compris). */
    const val MAX_COMPRESSED = 2 * 1024 * 1024
    /** Ce qui remplace le début coupé du journal, entre l'en-tête et la fin gardée. */
    const val CUT_MARK = "\n===== début du journal coupé : %d octets retirés pour tenir en 2 Mio =====\n"

    private val random = SecureRandom()

    fun newKey(): ByteArray = ByteArray(KEY_SIZE).also(random::nextBytes)
    fun newNonce(): ByteArray = ByteArray(NONCE_SIZE).also(random::nextBytes)

    fun gzip(data: ByteArray): ByteArray {
        val out = ByteArrayOutputStream(data.size / 4 + 64)
        GZIPOutputStream(out).use { it.write(data) }
        return out.toByteArray()
    }

    fun gunzip(data: ByteArray): ByteArray = GZIPInputStream(ByteArrayInputStream(data)).use { it.readBytes() }

    /**
     * Le journal compressé, en [max] octets au plus : tel quel s'il y tient ; sinon l'en-tête (les [head] premiers
     * octets : appareil, version, fins précédentes) puis la fin du journal, la plus récente, à partir d'un début de
     * ligne, avec une ligne qui dit ce qui a été retiré.
     */
    fun fit(text: ByteArray, head: Int, max: Int = MAX_COMPRESSED): ByteArray {
        val whole = gzip(text)
        if (whole.size <= max) return whole
        val h = head.coerceIn(0, text.size)
        val rest = text.size - h
        var keep = rest.toLong()
        var size = whole.size.toLong()
        // La fin gardée, ramenée selon ce que la compression a donné à l'essai précédent.
        repeat(12) {
            keep = (keep * max / size * 9 / 10).coerceAtMost(rest.toLong() - 1).coerceAtLeast(0)
            var from = text.size - keep.toInt()
            // Commencer à une ligne entière.
            while (from < text.size && text[from - 1] != '\n'.code.toByte()) from++
            val cut = from - h
            val out = ByteArrayOutputStream(h + (text.size - from) + 128)
            out.write(text, 0, h)
            out.write(String.format(java.util.Locale.ROOT, CUT_MARK, cut).toByteArray(Charsets.UTF_8))
            out.write(text, from, text.size - from)
            val z = gzip(out.toByteArray())
            if (z.size <= max) return z
            size = z.size.toLong()
        }
        // L'en-tête seul ne tient pas (jamais en pratique) : sa fin, coupée.
        return gzip(text.copyOfRange(maxOf(0, text.size - max / 2), text.size))
    }

    /** Le dépôt : `KNJ1`, le nonce, puis le texte chiffré (AES-256-GCM) et son étiquette de 16 octets. */
    fun seal(compressed: ByteArray, key: ByteArray, nonce: ByteArray = newNonce()): ByteArray {
        require(key.size == KEY_SIZE && nonce.size == NONCE_SIZE)
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, "AES"), GCMParameterSpec(TAG_BITS, nonce))
        return MAGIC + nonce + c.doFinal(compressed)
    }

    /** Ce que fait la page du téléphone (pour les essais) : le journal compressé, ou nul si le dépôt ne s'ouvre pas. */
    fun open(deposit: ByteArray, key: ByteArray): ByteArray? = runCatching {
        if (deposit.size < MAGIC.size + NONCE_SIZE + TAG_BITS / 8 || !deposit.copyOfRange(0, 4).contentEquals(MAGIC)) return null
        val nonce = deposit.copyOfRange(4, 4 + NONCE_SIZE)
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, "AES"), GCMParameterSpec(TAG_BITS, nonce))
        c.doFinal(deposit, 4 + NONCE_SIZE, deposit.size - 4 - NONCE_SIZE)
    }.getOrNull()

    /** La clé dans l'adresse : base64url sans remplissage (43 signes pour 32 octets). */
    fun keyText(key: ByteArray): String = Base64.getUrlEncoder().withoutPadding().encodeToString(key)

    /** L'adresse du QR code : `https://<relais>/journal/<id>#k=<clé>`. */
    fun qrUrl(relay: JournalRelay, id: String, key: ByteArray) = "https://${relay.authority}/journal/$id#k=${keyText(key)}"

    /** Un identifiant ou un jeton du relais : 22 signes base64url (128 bits) ; on n'en accepte pas d'autres dans une adresse. */
    fun validToken(s: String) = s.length in 16..64 && s.all { it.isLetterOrDigit() || it == '-' || it == '_' }

    /** Lit la réponse du dépôt : [status] HTTP (−1 : pas de réponse), le corps, l'en-tête `Retry-After`. */
    fun readPost(status: Int, body: String?, retryAfter: String?): JournalPost = when (status) {
        201, 200 -> runCatching {
            val o = JSONObject(body ?: "")
            val id = o.getString("id")
            val owner = o.getString("owner")
            if (!validToken(id) || !validToken(owner)) JournalPost.Unreachable
            else JournalPost.Posted(id, owner, o.optInt("expires_in", 3600).coerceIn(1, 3600))
        }.getOrDefault(JournalPost.Unreachable)
        429 -> JournalPost.TooMany(
            retryAfter?.trim()?.toIntOrNull()
                ?: runCatching { JSONObject(body ?: "").optInt("retry_after", 3600) }.getOrDefault(3600),
        )
        503 -> JournalPost.Full
        413 -> JournalPost.TooLarge
        404 -> JournalPost.OldRelay
        else -> JournalPost.Unreachable
    }

    /** Lit la réponse du suivi : `{"state":"waiting"|"opened"}`, `404` une fois expiré ou retiré. */
    fun readState(status: Int, body: String?): JournalState = when (status) {
        200 -> when (runCatching { JSONObject(body ?: "").optString("state") }.getOrNull()) {
            "waiting" -> JournalState.WAITING
            "opened" -> JournalState.OPENED
            else -> JournalState.UNKNOWN
        }
        404, 410 -> JournalState.GONE
        else -> JournalState.UNKNOWN
    }
}

/** Les appels au relais (bloquants : hors du fil principal). */
object JournalRelayCalls {
    private const val CONNECT_MS = 8_000
    private const val READ_MS = 30_000

    /** Dépose le journal chiffré. */
    fun post(relay: JournalRelay, deposit: ByteArray): JournalPost = runCatching {
        val c = open(relay, "/v1/journal", "POST")
        c.doOutput = true
        c.setRequestProperty("Content-Type", "application/octet-stream")
        c.setFixedLengthStreamingMode(deposit.size)
        try {
            c.outputStream.use { it.write(deposit) }
            val status = c.responseCode
            JournalSeal.readPost(status, body(c), c.getHeaderField("Retry-After"))
        } finally {
            c.disconnect()
        }
    }.getOrDefault(JournalPost.Unreachable)

    /** L'état du dépôt, pour dire « Journal ouvert sur le téléphone. ». */
    fun state(relay: JournalRelay, id: String, owner: String): JournalState = runCatching {
        val c = open(relay, "/v1/journal/$id/state", "GET")
        c.setRequestProperty("x-kn-owner", owner)
        try {
            JournalSeal.readState(c.responseCode, body(c))
        } finally {
            c.disconnect()
        }
    }.getOrDefault(JournalState.UNKNOWN)

    /** « Terminé » : retire un dépôt pas encore ouvert. */
    fun delete(relay: JournalRelay, id: String, owner: String) {
        runCatching {
            val c = open(relay, "/v1/journal/$id", "DELETE")
            c.setRequestProperty("x-kn-owner", owner)
            try { c.responseCode } finally { c.disconnect() }
        }
    }

    private fun body(c: HttpURLConnection): String? =
        runCatching { (if (c.responseCode >= 400) c.errorStream else c.inputStream)?.use { it.readBytes().toString(Charsets.UTF_8) } }.getOrNull()

    private fun open(relay: JournalRelay, path: String, method: String): HttpsURLConnection {
        val c = URL("https://${relay.authority}$path").openConnection() as HttpsURLConnection
        c.requestMethod = method
        c.connectTimeout = CONNECT_MS
        c.readTimeout = READ_MS
        c.useCaches = false
        c.instanceFollowRedirects = false
        relay.pin?.let { pin ->
            // Relais à empreinte épinglée : le certificat feuille doit être celui-là (ses autorités ne comptent pas).
            val ctx = SSLContext.getInstance("TLS")
            ctx.init(null, arrayOf<TrustManager>(PinnedTrust(pin)), null)
            c.sslSocketFactory = ctx.socketFactory
            c.hostnameVerifier = javax.net.ssl.HostnameVerifier { _, _ -> true }
        }
        return c
    }
}

/** Accepte le seul certificat dont l'empreinte SHA-256 (du certificat entier, en DER) est [pin]. */
class PinnedTrust(private val pin: ByteArray) : X509TrustManager {
    override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) = throw java.security.cert.CertificateException("client")

    override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
        val leaf = chain?.firstOrNull() ?: throw java.security.cert.CertificateException("aucun certificat")
        if (!matches(leaf.encoded, pin)) throw java.security.cert.CertificateException("empreinte du relais différente")
    }

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()

    companion object {
        fun matches(der: ByteArray, pin: ByteArray) = MessageDigest.isEqual(MessageDigest.getInstance("SHA-256").digest(der), pin)
    }
}
