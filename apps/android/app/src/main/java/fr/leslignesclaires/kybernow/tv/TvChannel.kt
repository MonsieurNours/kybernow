package fr.leslignesclaires.kybernow.tv

import android.content.ContentUris
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.core.content.FileProvider
import androidx.core.net.toUri
import androidx.tvprovider.media.tv.Channel
import androidx.tvprovider.media.tv.ChannelLogoUtils
import androidx.tvprovider.media.tv.TvContractCompat
import android.media.tv.TvContract.PreviewPrograms
import fr.leslignesclaires.kybernow.KyberNowApp
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.FormFactor
import fr.leslignesclaires.kybernow.model.Host
import java.util.concurrent.Executors

/**
 * La rangée « Reprendre sur un ordinateur » de l'accueil d'Android TV (comme
 * le Top Shelf de l'Apple TV) : chaque ordinateur avec sa dernière image ;
 * un clic ouvre directement la session (`kybernow://connect/<id>`). Les
 * images sont prêtées aux seuls lanceurs de l'accueil (FileProvider). Google
 * TV ne montre pas ces rangées ; l'accueil d'Android TV (Shield) oui.
 */
object TvChannel {
    private val worker = Executors.newSingleThreadExecutor()
    private const val PREFS = "tv"
    // Valeurs de TvContract (PreviewProgramColumns, que Kotlin ne voit pas à travers PreviewPrograms).
    private const val TYPE_GAME = 12
    private const val ASPECT_RATIO_16_9 = 0

    fun hostIdOf(uri: Uri): String? =
        if (uri.scheme == "kybernow" && uri.host == "connect") uri.pathSegments.firstOrNull() else null

    fun publish(context: Context, hosts: List<Host>) {
        if (KyberNowApp.instance.form != FormFactor.TV) return
        val app = context.applicationContext
        worker.execute { runCatching { sync(app, hosts) } }
    }

    private fun sync(context: Context, hosts: List<Host>) {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val resolver = context.contentResolver
        var channelId = prefs.getLong("channel", -1L)
        if (channelId < 0) {
            val channel = Channel.Builder()
                .setType(TvContractCompat.Channels.TYPE_PREVIEW)
                .setDisplayName(context.getString(R.string.tv_channel))
                .setAppLinkIntentUri("kybernow://home".toUri())
                .build()
            val uri = resolver.insert(TvContractCompat.Channels.CONTENT_URI, channel.toContentValues()) ?: return
            channelId = ContentUris.parseId(uri)
            prefs.edit().putLong("channel", channelId).apply()
            BitmapFactory.decodeResource(context.resources, R.mipmap.ic_launcher)?.let { ChannelLogoUtils.storeChannelLogo(context, channelId, it) }
            TvContractCompat.requestChannelBrowsable(context, channelId)
        }
        // On repart de zéro : quelques ordinateurs seulement.
        resolver.delete(TvContractCompat.buildPreviewProgramsUriForChannel(channelId), null, null)
        val launchers = launcherPackages(context)
        for (h in hosts) {
            val image = KyberNowApp.instance.hosts.imageFile(h.id)
            val poster = if (image.exists()) {
                FileProvider.getUriForFile(context, "${context.packageName}.images", image).also { uri ->
                    launchers.forEach { context.grantUriPermission(it, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
                }
            } else {
                "android.resource://${context.packageName}/${R.drawable.tv_banner}".toUri()
            }
            val values = ContentValues().apply {
                put(PreviewPrograms.COLUMN_CHANNEL_ID, channelId)
                put(PreviewPrograms.COLUMN_TYPE, TYPE_GAME)
                put(PreviewPrograms.COLUMN_TITLE, h.name)
                put(PreviewPrograms.COLUMN_SHORT_DESCRIPTION, listOfNotNull(h.os, h.gpu).joinToString(" · "))
                put(PreviewPrograms.COLUMN_POSTER_ART_URI, poster.toString())
                put(PreviewPrograms.COLUMN_POSTER_ART_ASPECT_RATIO, ASPECT_RATIO_16_9)
                put(PreviewPrograms.COLUMN_INTENT_URI, "kybernow://connect/${h.id}")
                put(PreviewPrograms.COLUMN_INTERNAL_PROVIDER_ID, h.id)
            }
            resolver.insert(PreviewPrograms.CONTENT_URI, values)
        }
    }

    /** Les lanceurs de l'accueil, seuls à recevoir le droit de lire les images. */
    private fun launcherPackages(context: Context): List<String> {
        val home = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME)
        return context.packageManager.queryIntentActivities(home, PackageManager.MATCH_DEFAULT_ONLY).map { it.activityInfo.packageName }.distinct()
    }
}
