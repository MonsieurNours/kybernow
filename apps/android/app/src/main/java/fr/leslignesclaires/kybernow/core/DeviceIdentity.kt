package fr.leslignesclaires.kybernow.core

import android.app.UiModeManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.provider.Settings
import fr.leslignesclaires.kybernow.BuildConfig
import org.json.JSONObject
import java.util.UUID

/** Forme de l'appareil : décide de l'interface (focus au téléviseur, au toucher ailleurs). */
enum class FormFactor(val adminKind: String) {
    TV("tv"),
    PHONE("phone"),
    TABLET("tablet"),
}

/** L'appareil tel qu'il se présente à Admin (`id`, `name`, `kind`, `os`, `app`). */
data class DeviceIdentity(val id: String, val name: String, val kind: String, val os: String, val app: String) {
    fun json(): String = JSONObject()
        .put("id", id)
        .put("name", name)
        .put("kind", kind)
        .put("os", os)
        .put("app", app)
        .toString()

    companion object {
        fun formFactor(context: Context): FormFactor {
            val ui = context.getSystemService(UiModeManager::class.java)
            if (ui?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION ||
                context.packageManager.hasSystemFeature("android.software.leanback_only")
            ) return FormFactor.TV
            return if (context.resources.configuration.smallestScreenWidthDp >= 600) FormFactor.TABLET else FormFactor.PHONE
        }

        /** Un identifiant tiré une fois et gardé : la session est identifiée par l'appareil. */
        fun of(context: Context): DeviceIdentity {
            val prefs = context.getSharedPreferences("device", Context.MODE_PRIVATE)
            val id = prefs.getString("id", null) ?: UUID.randomUUID().toString().also { prefs.edit().putString("id", it).apply() }
            val name = Settings.Global.getString(context.contentResolver, Settings.Global.DEVICE_NAME)
                ?.takeIf { it.isNotBlank() }
                ?: Build.MODEL
            return DeviceIdentity(
                id = id,
                name = name,
                kind = formFactor(context).adminKind,
                os = "Android ${Build.VERSION.RELEASE}",
                app = "KyberNow Link ${BuildConfig.VERSION_NAME} (Android)",
            )
        }
    }
}
