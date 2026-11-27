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
        val raw = JSONObject().put("from", from).put("accounts", accounts).put("consoles", consoles).toString()
        // Mis en forme par le cœur (`Bundle`), le même paquet que les autres apps.
        return PsNative.bundleNormalize(raw) ?: raw
    }

    /** Ce qu'un paquet reçu a apporté : les nombres du paquet (« {n} compte(s) et {n} console(s) reçus. »). */
    data class Received(val accounts: Int, val consoles: Int)

    /**
     * Fusion : même identifiant de compte ou même console, rien en double, les associations s'ajoutent (une association
     * déjà là reste). Les Stations sont reliées aux ordinateurs de l'appareil par l'identifiant, l'empreinte, puis
     * l'adresse ; inconnues, ignorées. Nul si le paquet ne se lit pas.
     */
    fun merge(store: PsStore, hosts: List<Host>, json: String): Received? = runCatching {
        // Le cœur lit le paquet (formes tolérées, identifiants PSN vérifiés), rapproche ses comptes de ceux de
        // l'appareil et ses Stations des ordinateurs (`playstation::bundle`) ; l'app range le résultat.
        val local = JSONArray()
        for (a in store.accounts.value) store.psnId(a.id)?.let { local.put(JSONObject().put("id", a.id).put("psn_id", it)) }
        val known = JSONArray()
        for (h in hosts) known.put(JSONObject().put("id", h.id).put("fingerprint", h.fingerprint ?: JSONObject.NULL).put("address", h.address))
        val plan = JSONObject(PsNative.bundlePlan(json, local.toString(), known.toString()) ?: return@runCatching null)
        val accounts = plan.getJSONArray("accounts")
        val consoles = plan.getJSONArray("consoles")
        for (i in 0 until accounts.length()) {
            val a = accounts.getJSONObject(i)
            val name = a.optString("name").takeIf { !a.isNull("name") && it.isNotEmpty() }
            val kept = store.keep(a.getString("psn_id"), null, a.getString("id"))
            // Un compte déjà là prend le nom du paquet seulement s'il n'en avait pas.
            if (name != null && kept.account.name == null) store.rename(kept.account.id, name)
        }
        for (i in 0 until consoles.length()) {
            val c = consoles.getJSONObject(i)
            val hostId = c.getString("host_id")
            val address = if (c.isNull("address")) "" else c.optString("address")
            val target = if (c.isNull("target")) "" else c.optString("target")
            val model = c.optString("model").ifEmpty { "PS5" }
            val pairings = c.optJSONObject("pairings") ?: JSONObject()
            for (account in pairings.keys()) {
                val existing = store.consoleByHost(hostId)
                if (existing != null && store.registered(existing, account) != null) continue
                store.paired(hostId, c.optString("name"), model, address, target, pairings.getString(account), account)
            }
            val matched = c.optJSONArray("hosts") ?: JSONArray()
            for (j in 0 until matched.length()) store.addStation(hostId, matched.getString(j))
        }
        Received(accounts.length(), consoles.length())
    }.getOrNull()

    fun parseWaiting(json: String): List<Waiting> = runCatching {
        val a = JSONArray(json)
        List(a.length()) { a.getJSONObject(it).let { o -> Waiting(o.optString("name"), o.optString("platform"), o.optString("address")) } }
    }.getOrDefault(emptyList())
}
