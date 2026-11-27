package fr.leslignesclaires.kybernow.model

import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

/**
 * Une liste gardée en JSON (ordinateurs, ordinateurs prêtés, consoles), lue entrée par entrée (AND-22) : une entrée
 * illisible (écrite par une version plus récente, abîmée) est laissée de côté, au lieu de rendre une liste vide que
 * l'enregistrement suivant aurait écrite à la place de toutes les autres.
 */
fun <T> parseEach(json: String?, parse: (JSONObject) -> T): List<T> {
    val a = runCatching { JSONArray(json ?: "[]") }.getOrNull() ?: return emptyList()
    return (0 until a.length()).mapNotNull { i ->
        runCatching { parse(a.getJSONObject(i)) }
            .onFailure { Log.w("KyberNow", "liste enregistrée : entrée $i illisible, laissée de côté ($it)") }
            .getOrNull()
    }
}
