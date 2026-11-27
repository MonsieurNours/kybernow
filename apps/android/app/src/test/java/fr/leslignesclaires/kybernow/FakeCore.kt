package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.core.Core
import org.json.JSONObject

/**
 * Un faux cœur pour les essais JVM, qui n'ont pas `libkybernow.so` ([Core.source]). Il ne refait pas les règles du
 * cœur (elles ont leurs essais en Rust) : il rend des réponses écrites d'avance, de la forme de celles du cœur, pour
 * que l'app soit essayée sur ce qu'elle en fait. Chaque essai qui en a besoin le pose et le retire ([install], [reset]).
 */
open class FakeCore : Core.Source {
    override fun shortFingerprint(fp: String) = "7C41 E09B AABB CCDD"
    override fun fingerprintSame(a: String, b: String) = a.lowercase().removePrefix("sha256:") == b.lowercase().removePrefix("sha256:")
    override fun parseAddress(text: String, defaultPort: Int): String? = null
    override fun valuesJson(capabilitiesJson: String) =
        """{"manual_mbps":{"min":5,"step":5,"max":500,"max_pyrowave":2500},"station_notice_wait_secs":10,"station_notice_before":"0.11.2","admin_port":9443,"address_example":"192.168.1.20:9443","default_relay":"share.kybernow.app"}"""
    override fun lendAvailabilityLabel(state: String) = when (state) {
        "free" -> "Disponible"
        "busy" -> "Occupé"
        "lent" -> "Prêté à quelqu'un d'autre"
        "locked" -> "Verrouillé"
        else -> "Prêt coupé"
    }
    override fun relayErrorText(code: Int, name: String?) = when (code) {
        30 -> "Le relais ne répond pas."
        31 -> "$name n'est pas joignable : l'ordinateur est éteint ou en veille."
        32 -> "$name n'est pas joignable hors de chez vous (accès coupé dans Admin)."
        else -> null
    }
    override fun relayPathLabel(transport: Int) = listOf("Direct", "Par le relais", "Par le relais (TCP)").getOrNull(transport)
    override fun journalDeposit(relay: String, header: String, body: String): String? = null
    override fun journalState(relay: String, id: String, owner: String) = 90
    override fun journalDiscard(relay: String, id: String, owner: String) = false
    override fun relayLabel(addr: String) = addr.substringBefore('#')
    override fun lendWarning(os: String) = when {
        os.startsWith("Windows") -> "Votre ami joue dans votre session Windows. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance."
        os.startsWith("macOS") -> "Votre ami joue dans votre session du Mac. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance."
        else -> "Votre ami joue dans votre session. Il ne voit ni votre bureau ni vos fenêtres, mais un jeu peut lire vos fichiers : prêtez à des amis de confiance."
    }
    override fun looksLikeInvite(text: String) = text.contains("borrow")
    override fun pyrowaveLoweredText(width: Int, height: Int, refresh: Int, mbps: Int) = "Définition ramenée à $width × $height pour tenir dans $mbps Mbit/s."
    override fun discoveryFound(json: String): String? {
        val o = JSONObject(json)
        val host = o.optJSONArray("addresses")?.optString(0)?.takeIf { it.isNotEmpty() } ?: return null
        val txt = o.optJSONObject("txt") ?: JSONObject()
        return JSONObject().put("id", o.getString("fullname")).put("name", txt.optString("name", "Station")).put("host", host)
            .put("port", o.getInt("port")).put("fingerprint", txt.opt("fp") ?: JSONObject.NULL).toString()
    }
    override fun recordingClock(seconds: Long) = "%02d:%02d".format(seconds / 60, seconds % 60)
    override fun profileSettings(profile: String, hevc: Boolean, hevc444: Boolean, av1: Boolean): String {
        val (yuv, mbps, reliable) = when (profile) {
            "jeu" -> Triple(false, 60, false)
            "bureau" -> Triple(true, 90, true)
            else -> Triple(true, 300, false)
        }
        return JSONObject().put("profile", profile).put("codec", if (hevc) "hevc" else "h264").put("yuv444", yuv && hevc && hevc444)
            .put("mbps", mbps).put("reliable", reliable).toString()
    }
    override fun accountSettingsKnown(syncedJson: String): String =
        ok(JSONObject(syncedJson).optJSONObject("values") ?: JSONObject())
    override fun accountSettingsApply(valuesJson: String, baseJson: String, capsJson: String): String {
        val values = JSONObject(valuesJson)
        val caps = JSONObject(capsJson)
        val o = JSONObject(baseJson)
        val c = values.optString("codec")
        if (c == "h264" || (c.isNotEmpty() && caps.optBoolean(c))) o.put("codec", c)
        if (values.has("mbps")) o.put("mbps", values.optInt("mbps"))
        if (values.has("hdr")) o.put("hdr", values.optBoolean("hdr"))
        return ok(o)
    }
    override fun accountSettingsFromStream(settingsJson: String): String {
        val s = JSONObject(settingsJson)
        return ok(
            JSONObject().put("profile", s.opt("profile") ?: JSONObject.NULL).put("codec", s.optString("codec"))
                .put("yuv444", s.optBoolean("yuv444")).put("mbps", s.optInt("mbps")).put("hdr", s.optBoolean("hdr")),
        )
    }
    override fun welcome(stateJson: String, op: String, hasAccount: Boolean, computer: String, now: Long) =
        JSONObject().put("state", JSONObject(stateJson.ifBlank { "{}" })).put("value", !hasAccount).toString()
    override fun welcomeSignUpUrl(relay: String) = "https://share.kybernow.app/compte/inscription?lang=fr"
    override fun welcomeStationUrl() = "https://www.kybernow.app/#telecharger"

    private fun ok(value: JSONObject) = JSONObject().put("code", 0).put("value", value).toString()

    companion object {
        fun install(fake: Core.Source = FakeCore()) {
            Core.source = fake
        }

        fun reset() {
            Core.source = Core.NativeSource
        }
    }
}
