package fr.leslignesclaires.kybernow.playstation

import fr.leslignesclaires.kybernow.core.PsNative
import fr.leslignesclaires.kybernow.model.Host
import org.json.JSONArray
import org.json.JSONObject

/** Un appareil du réseau qui attend un envoi (« {nom} · Apple TV »). */
data class Waiting(val name: String, val platform: String, val address: String)

/**
 * « Envoyer à un appareil » et « Recevoir d'un autre appareil » (docs/playstation.md, « Plusieurs appareils ») : le
 * paquet (comptes avec leur identifiant, consoles, associations, Stations qui les voient), chiffré par le cœur
 * (`playstation::handoff`, SPAKE2 sur le code à 6 chiffres). Même format que les apps Apple
 * (`apps/macos/KyberNow/PlayStationSync.swift`).
 */
object Handoff {
    /** Le paquet de tout ce que l'appareil garde ; [from] : le nom de l'appareil. */
    fun bundle(store: PsStore, hosts: List<Host>, from: String): String {
        val accounts = JSONArray()
        for (a in store.accounts.value) {
            val id = store.psnId(a.id) ?: continue
            accounts.put(JSONObject().put("id", a.id).put("name", a.name ?: JSONObject.NULL).put("psn_id", id))
        }
        val consoles = JSONArray()
        for (c in store.consoles.value) {
            val pairings = JSONObject()
            val paired = store.pairedAccounts(c)
            for (a in paired) store.registered(c, a.id)?.let { pairings.put(a.id, it) }
            if (pairings.length() == 0) continue
            val stations = JSONArray()
            for (sid in store.stations(c.hostId)) {
                val h = hosts.firstOrNull { it.id == sid } ?: continue
                stations.put(JSONObject().put("id", h.id).put("name", h.name).put("address", h.address).put("fingerprint", h.fingerprint ?: JSONObject.NULL))
            }
            consoles.put(
                JSONObject().put("host_id", c.hostId).put("name", c.name).put("model", c.model)
                    .put("addresses", JSONArray(listOf(c.address).filter { it.isNotBlank() }))
                    .put("accounts", JSONArray(paired.map { it.id })).put("target", c.target)
                    .put("pairings", pairings).put("stations", stations),
            )
        }
        return JSONObject().put("from", from).put("accounts", accounts).put("consoles", consoles).toString()
    }

    /** Ce qu'un paquet reçu a apporté : les nombres du paquet (« {n} compte(s) et {n} console(s) reçus. »). */
    data class Received(val accounts: Int, val consoles: Int)

    /**
     * Fusion : même identifiant de compte ou même console, rien en double, les associations s'ajoutent (une association
     * déjà là reste). Les Stations sont reliées aux ordinateurs de l'appareil par l'identifiant, l'empreinte, puis
     * l'adresse ; inconnues, ignorées. Nul si le paquet ne se lit pas.
     */
    fun merge(store: PsStore, hosts: List<Host>, json: String): Received? = runCatching {
        val o = JSONObject(json)
        val accounts = o.optJSONArray("accounts") ?: JSONArray()
        val consoles = o.optJSONArray("consoles") ?: JSONArray()
        // Identifiant du compte dans le paquet → identifiant ici.
        val ids = mutableMapOf<String, String>()
        for (i in 0 until accounts.length()) {
            val a = accounts.getJSONObject(i)
            // Toléré : `account` ou `account_id` à la place de `psn_id` (paquet de webOS).
            val psn = listOf("psn_id", "account", "account_id").map { a.optString(it) }.firstOrNull { it.isNotEmpty() && PsNative.accountIdValid(it) } ?: continue
            val name = a.optString("name").takeIf { !a.isNull("name") && it.isNotEmpty() }
            val kept = store.keep(psn, null, a.optString("id"))
            // Un compte déjà là prend le nom du paquet seulement s'il n'en avait pas.
            if (name != null && kept.account.name == null) store.rename(kept.account.id, name)
            ids[a.optString("id").uppercase()] = kept.account.id
        }
        for (i in 0 until consoles.length()) {
            val c = consoles.getJSONObject(i)
            val hostId = c.optString("host_id").uppercase().takeIf { it.isNotEmpty() } ?: continue
            val address = c.optJSONArray("addresses")?.let { a -> (0 until a.length()).map { a.getString(it) } }?.firstOrNull() ?: c.optString("address")
            // Association en texte JSON (apps Apple) ou en objet ; toléré : un tableau `[{account, registered}]`.
            val pairings = mutableMapOf<String, String>()
            c.optJSONObject("pairings")?.let { p -> for (k in p.keys()) pairings[k] = p.get(k).toString() }
            c.optJSONArray("pairings")?.let { p -> for (j in 0 until p.length()) p.optJSONObject(j)?.let { e -> e.opt("registered")?.let { r -> pairings[e.optString("account")] = r.toString() } } }
            val model = c.optString("model", "PS5").uppercase()
            for ((key, registered) in pairings) {
                val account = ids[key.uppercase()] ?: continue
                val existing = store.consoleByHost(hostId)
                if (existing != null && store.registered(existing, account) != null) continue
                store.paired(hostId, c.optString("name"), model, address, c.optString("target"), registered, account)
            }
            val stations = c.optJSONArray("stations") ?: JSONArray()
            for (j in 0 until stations.length()) {
                // Toléré : l'identifiant de la Station seul (paquet de webOS).
                val st = stations.optJSONObject(j) ?: JSONObject().put("id", stations.optString(j))
                val fp = st.optString("fingerprint").takeIf { !st.isNull("fingerprint") && it.isNotEmpty() }
                val h = hosts.firstOrNull { it.id == st.optString("id") }
                    ?: fp?.let { f -> hosts.firstOrNull { it.fingerprint == f } }
                    ?: hosts.firstOrNull { it.address == st.optString("address") }
                    ?: continue
                store.addStation(hostId, h.id)
            }
        }
        Received(accounts.length(), consoles.length())
    }.getOrNull()

    fun parseWaiting(json: String): List<Waiting> = runCatching {
        val a = JSONArray(json)
        List(a.length()) { a.getJSONObject(it).let { o -> Waiting(o.optString("name"), o.optString("platform"), o.optString("address")) } }
    }.getOrDefault(emptyList())
}
