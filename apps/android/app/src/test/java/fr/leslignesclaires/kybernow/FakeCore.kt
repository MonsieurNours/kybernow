package fr.leslignesclaires.kybernow

import fr.leslignesclaires.kybernow.core.Core
import org.json.JSONArray
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
    /** Les réglages d'un profil (`StreamSettings::for_profile`), écrits d'avance. */
    private fun profileSettings(profile: String, hevc: Boolean, hevc444: Boolean): JSONObject {
        val (yuv, mbps, reliable) = when (profile) {
            "jeu" -> Triple(false, 60, false)
            "bureau" -> Triple(true, 90, true)
            else -> Triple(true, 300, false)
        }
        return JSONObject().put("profile", profile).put("codec", if (hevc) "hevc" else "h264").put("yuv444", yuv && hevc && hevc444)
            .put("mbps", mbps).put("reliable", reliable)
    }

    /**
     * Le plan d'une session (`plan::plan`), réduit : le profil (Auto : le format et le débit de son test), un format ou
     * un transport qui s'en écarte et un débit à la main le rendent « Personnalisé » (débit du profil, sans le calcul par
     * les pixels), le 4:4:4 en HEVC seulement, « Passer en HEVC », l'H.264 à 4096 de large au plus.
     */
    override fun plan(choicesJson: String, capabilitiesJson: String, width: Int, height: Int, refresh: Int): String? {
        val c = JSONObject(choicesJson)
        val caps = JSONObject(capabilitiesJson)
        val profile = c.optString("profile", "jeu")
        val auto = c.optJSONObject("auto")?.takeIf { profile == "auto" }
        val s = auto?.let { JSONObject().put("profile", "auto").put("codec", it.optString("codec")).put("yuv444", false).put("mbps", it.optInt("mbps")).put("reliable", it.optBoolean("reliable")) }
            ?: profileSettings(if (profile == "auto") "jeu" else profile, caps.optBoolean("hevc"), caps.optBoolean("hevc444"))
        var custom = false
        c.optString("codec").takeIf { it.isNotEmpty() && it != s.optString("codec") }?.let { s.put("codec", it); custom = true }
        if (c.has("reliable") && c.getBoolean("reliable") != s.optBoolean("reliable")) s.put("reliable", c.getBoolean("reliable")).also { custom = true }
        if (c.has("mbps")) s.put("mbps", c.getInt("mbps")).also { custom = true }
        if (custom) s.put("profile", JSONObject.NULL)
        if (c.optBoolean("hevc_instead")) s.put("codec", "hevc").put("wired", false)
        if (s.optString("codec") != "hevc") s.put("yuv444", false)
        s.put("adaptive", !c.optBoolean("fixed")).put("encoder_max", c.opt("encoder_max") ?: JSONObject.NULL)
        var (w, h) = width to height
        if (s.optString("codec") == "h264" && w > 4096) {
            h = h * 4096 / w / 2 * 2
            w = 4096
        }
        val mode = JSONObject().put("width", w).put("height", h).put("refresh", refresh)
        return JSONObject().put("settings", s).put("mode", mode).put("mbps", s.optInt("mbps")).put("label", "").toString()
    }
    /**
     * La mémoire des réglages synchronisés (`account::AccountSync`), réduite à ce que l'app en attend : la même forme
     * `{list, applied, pending}`, les mêmes opérations ; les clés reçues ne sont pas vérifiées (le cœur a ses essais).
     */
    override fun accountSync(op: String, sync: String, station: String, a: String, b: String): String? {
        val m = JSONObject(sync.ifBlank { "{}" })
        val list = m.optJSONArray("list") ?: JSONArray()
        val applied = m.optJSONObject("applied") ?: JSONObject()
        val pending = m.optJSONObject("pending") ?: JSONObject()
        fun obj(t: String) = JSONObject(t.ifBlank { "{}" })
        fun over(base: JSONObject, top: JSONObject) = JSONObject(base.toString()).also { o -> top.keys().forEach { o.put(it, top.get(it)) } }
        fun find(l: JSONArray, st: String) = (0 until l.length()).map { l.getJSONObject(it) }.firstOrNull { it.optString("station") == st }
        var out: Any = JSONObject.NULL
        var newList = list
        when (op) {
            "to_send" -> {
                val waiting = pending.optJSONObject(station) ?: JSONObject()
                out = over(if (find(list, station) == null) over(obj(b), waiting) else waiting, obj(a))
            }
            "sent" -> {
                val st = obj(a)
                val at = st.optString("station")
                pending.remove(at)
                applied.put(at, st.optLong("updated_at"))
                newList = JSONArray((0 until list.length()).map { list.getJSONObject(it) }.filter { it.optString("station") != at } + st)
            }
            "unsent" -> pending.put(station, over(pending.optJSONObject(station) ?: JSONObject(), obj(a)))
            "refused" -> pending.optJSONObject(station)?.let { p ->
                val sent = obj(a)
                sent.keys().asSequence().toList().forEach { k -> if (p.has(k) && p.get(k).toString() == sent.get(k).toString()) p.remove(k) }
                if (p.length() == 0) pending.remove(station)
            }
            "forget" -> {
                applied.remove(station)
                pending.remove(station)
                newList = JSONArray((0 until list.length()).map { list.getJSONObject(it) }.filter { it.optString("station") != station })
            }
            "fetched" -> {
                newList = JSONArray(a)
                applied.keys().asSequence().toList().forEach { if (find(newList, it) == null) applied.remove(it) }
                pending.keys().asSequence().toList().forEach { if (find(newList, it) == null && find(list, it) != null) pending.remove(it) }
                out = JSONArray().also { arr ->
                    for (i in 0 until newList.length()) {
                        val st = newList.getJSONObject(i)
                        val at = st.optString("station")
                        if (pending.has(at) || st.optLong("updated_at") <= applied.optLong(at, 0)) continue
                        applied.put(at, st.optLong("updated_at"))
                        val values = st.optJSONObject("values") ?: JSONObject()
                        if (values.length() > 0) arr.put(JSONObject().put("station", at).put("values", values))
                    }
                }
            }
            "last" -> out = find(list, station) ?: JSONObject.NULL
            "pending" -> out = JSONObject(pending.toString())
            else -> return JSONObject().put("error", "opération inconnue").toString()
        }
        val next = JSONObject().put("list", newList).put("applied", applied).put("pending", pending)
        return JSONObject().put("sync", next).put("value", out).toString()
    }
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
    /** La ligne « Stylet » hors session (`PenChoice::ui_json`), en français. */
    override fun penTexts(): String? = penJson(shown = false, mode = "pressure")
    override fun penNotice(mode: String) = if (mode == "mouse") "Stylet\u202f: souris" else "Stylet\u202f: pression"

    private fun ok(value: JSONObject) = JSONObject().put("code", 0).put("value", value).toString()

    companion object {
        /** `PenChoice::ui_json` du cœur, en français, écrit d'avance. */
        fun penJson(shown: Boolean, mode: String): String = JSONObject()
            .put("shown", shown).put("mode", mode).put("title", "Stylet").put("label", if (mode == "mouse") "Souris" else "Pression")
            .put("choices", JSONArray().put(JSONObject().put("key", "pressure").put("label", "Pression")).put(JSONObject().put("key", "mouse").put("label", "Souris")))
            .put("help", "Pression\u202f: le stylet arrive comme un vrai stylet, pour dessiner. Souris\u202f: il sert de souris, sans la pression ni Windows Ink.")
            .toString()

        fun install(fake: Core.Source = FakeCore()) {
            Core.source = fake
        }

        fun reset() {
            Core.source = Core.NativeSource
        }
    }
}
