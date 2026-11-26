package fr.leslignesclaires.kybernow.core

import org.json.JSONObject

/*
 * Mise à jour des Stations proposée par l'app (docs/link-ui.md, « Mise à jour des Stations ») : le calcul est celui du
 * cœur (`station_offer`, par [Native.stationOfferEvaluate]), pareil dans toutes les apps ; l'app garde, par ordinateur,
 * son `Memory` en JSON ([fr.leslignesclaires.kybernow.model.HostStore.stationMemory]).
 */

/** Le bandeau de l'accueil pour l'ordinateur choisi (`station_offer::Banner`). */
sealed interface StationBanner {
    data object None : StationBanner
    /** « KyberNow Station {v} est disponible pour cet ordinateur. Environ une minute. » ; « Mettre à jour », « Plus tard ». */
    data class Update(val version: String) : StationBanner
    /** « Mise à jour de la Station… » (lancée d'ici ou d'ailleurs). */
    data object Installing : StationBanner
    /** La version s'installe avec l'installateur, sur l'ordinateur ; « Plus tard » seul. */
    data class Setup(val version: String) : StationBanner
    /** Hors de chez vous : l'app ne peut pas la mettre à jour ; « Plus tard » seul. */
    data class Away(val version: String) : StationBanner
}

/**
 * Ce que l'app montre d'un ordinateur : le [banner] de l'accueil, la puce « · Mise à jour » ([chip]), la version de
 * l'avis après une mise à jour de l'app ([noticeVersion]), et s'il faut demander à la Station de vérifier
 * ([shouldCheck], `Console::station_check`).
 */
data class StationOffer(
    val banner: StationBanner = StationBanner.None,
    val chip: Boolean = false,
    val noticeVersion: String? = null,
    val shouldCheck: Boolean = false,
) {
    companion object {
        /** L'avis après une mise à jour de l'app attend au plus ce temps les vérifications (`station_offer::NOTICE_WAIT_SECS`). */
        const val NOTICE_WAIT_MS = 10_000L

        fun now(): Long = System.currentTimeMillis() / 1000

        /** Le calcul du cœur pour un ordinateur ; [memory] : son `Memory` gardé (JSON). */
        fun evaluate(
            station: StationInfo?, mayUpdate: Boolean, online: Boolean, away: Boolean, inSession: Boolean,
            memory: String, appVersion: String, now: Long = now(),
        ): StationOffer {
            val input = JSONObject()
                .put("station", station?.json() ?: JSONObject.NULL)
                .put("may_update", mayUpdate).put("online", online).put("away", away).put("in_session", inSession)
                .put("now", now).put("app_version", appVersion)
                .put("memory", runCatching { JSONObject(memory) }.getOrDefault(JSONObject()))
            return parse(Native.stationOfferEvaluate(input.toString()))
        }

        /** La sortie de `station_offer::evaluate_json` ; illisible : rien à montrer. */
        fun parse(json: String): StationOffer {
            val o = runCatching { JSONObject(json) }.getOrNull() ?: return StationOffer()
            val b = o.optJSONObject("banner")
            val version = b?.optString("version")?.takeIf { it.isNotEmpty() && it != "null" }
            val banner = when (b?.optString("kind")) {
                "update" -> version?.let { StationBanner.Update(it) }
                "installing" -> StationBanner.Installing
                "setup" -> version?.let { StationBanner.Setup(it) }
                "away" -> version?.let { StationBanner.Away(it) }
                else -> null
            } ?: StationBanner.None
            return StationOffer(
                banner = banner,
                chip = o.optBoolean("chip"),
                noticeVersion = o.optString("notice_version").takeIf { !o.isNull("notice_version") && it.isNotEmpty() },
                shouldCheck = o.optBoolean("should_check"),
            )
        }

        /** `station_offer::BEFORE_NOTICE` : dernière version lancée supposée quand l'app n'en a pas gardé mais connaît des ordinateurs. */
        const val BEFORE_NOTICE = "0.11.2"

        /** Premier lancement après une mise à jour de l'app (jamais au tout premier lancement : [last] nul). */
        fun appUpdated(last: String?, current: String): Boolean = Native.stationOfferAppUpdated(last, current)

        /** « Plus tard » pour [version] (`Memory::snooze`) : le `Memory` à garder. */
        fun snooze(memory: String, version: String, now: Long = now()): String = edit(memory) { it.put("snoozed_version", version).put("snoozed_at", now) }

        /** Une vérification vient d'être demandée (`Memory::checked`) : le `Memory` à garder. */
        fun checked(memory: String, appVersion: String, now: Long = now()): String = edit(memory) { it.put("checked_app_version", appVersion).put("checked_at", now) }

        private fun edit(memory: String, f: (JSONObject) -> JSONObject): String =
            f(runCatching { JSONObject(memory) }.getOrDefault(JSONObject())).toString()
    }
}
