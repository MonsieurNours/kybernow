package fr.leslignesclaires.kybernow.session

import android.annotation.SuppressLint
import android.content.Context
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.MediaRecorder
import android.media.audiofx.AcousticEchoCanceler
import android.media.audiofx.AudioEffect
import android.media.audiofx.NoiseSuppressor
import android.os.Process
import android.util.Log
import fr.leslignesclaires.kybernow.model.MicInputs
import fr.leslignesclaires.kybernow.model.MicPrefs

/**
 * La capture du micro pendant la session (docs/micro.md, « Les apps ») : `AudioRecord` sur son propre fil, 48 kHz
 * mono, par trames de 10 ms données à [push] (échantillons, nombre, fréquence) ; le cœur encode et envoie.
 *
 * « Réduire l'écho et le bruit » allumé : source `VOICE_COMMUNICATION` et, quand l'appareil les a,
 * `AcousticEchoCanceler` et `NoiseSuppressor` sur la session de l'enregistrement. Coupé (micro de studio) : la source
 * `UNPROCESSED` quand l'appareil la propose, sinon `MIC`, sans effet. Le micro choisi dans les Réglages de l'app passe
 * par `setPreferredDevice` ; débranché, celui du système. Le mode du son (`MODE_IN_COMMUNICATION`) n'est pas touché :
 * il ferait passer le son de la session dans l'écouteur du téléphone.
 *
 * Flottants d'abord ; un appareil qui ne les capte pas : 16 bits, ramenés en flottants ici.
 */
class MicCapture(private val context: Context, private val push: (FloatArray, Int, Int) -> Unit) {
    /** La capture en cours : son fil lit tant qu'elle est la sienne (un arrêt suivi d'un départ ne mêle pas deux fils). */
    @Volatile private var current: Any? = null
    private var thread: Thread? = null

    val active get() = current != null

    fun start() {
        if (current != null) return
        val token = Any()
        current = token
        thread = Thread({
            runCatching { loop(token) }.onFailure { Log.w(TAG, "micro : capture arrêtée (${it.message})") }
            if (current === token) current = null
        }, "kybernow-micro").apply { start() }
    }

    fun stop() {
        current = null
        thread?.let { if (it !== Thread.currentThread()) it.join(500) }
        thread = null
    }

    @SuppressLint("MissingPermission")
    private fun open(source: Int, encoding: Int): AudioRecord? {
        val min = AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, encoding)
        if (min <= 0) return null
        val bytes = if (encoding == AudioFormat.ENCODING_PCM_FLOAT) 4 else 2
        return runCatching {
            AudioRecord.Builder()
                .setAudioSource(source)
                .setAudioFormat(AudioFormat.Builder().setSampleRate(RATE).setChannelMask(AudioFormat.CHANNEL_IN_MONO).setEncoding(encoding).build())
                // Quatre trames de marge : le fil lit toutes les 10 ms.
                .setBufferSizeInBytes(maxOf(min, FRAME * bytes * 4))
                .build()
        }.getOrNull()?.takeIf { it.state == AudioRecord.STATE_INITIALIZED || run { it.release(); false } }
    }

    private fun loop(token: Any) {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
        val voice = MicPrefs.voiceProcessing(context)
        val source = when {
            voice -> MediaRecorder.AudioSource.VOICE_COMMUNICATION
            context.getSystemService(AudioManager::class.java)?.getProperty(AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED) == "true" ->
                MediaRecorder.AudioSource.UNPROCESSED
            else -> MediaRecorder.AudioSource.MIC
        }
        var float = true
        val rec = open(source, AudioFormat.ENCODING_PCM_FLOAT) ?: open(source, AudioFormat.ENCODING_PCM_16BIT).also { float = false } ?: run {
            Log.w(TAG, "micro : AudioRecord impossible (source $source)")
            return
        }
        MicInputs.preferred(context)?.let { rec.setPreferredDevice(it) }
        val effects = mutableListOf<AudioEffect>()
        if (voice) {
            if (AcousticEchoCanceler.isAvailable()) AcousticEchoCanceler.create(rec.audioSessionId)?.let { it.enabled = true; effects += it }
            if (NoiseSuppressor.isAvailable()) NoiseSuppressor.create(rec.audioSessionId)?.let { it.enabled = true; effects += it }
        }
        Log.i(TAG, "micro : capture ${rec.sampleRate} Hz, ${if (float) "flottants" else "16 bits"}, source $source, ${effects.size} effet(s)")
        try {
            rec.startRecording()
            val rate = rec.sampleRate
            val out = FloatArray(FRAME)
            val pcm = if (float) null else ShortArray(FRAME)
            while (current === token) {
                val n = if (pcm == null) rec.read(out, 0, FRAME, AudioRecord.READ_BLOCKING) else rec.read(pcm, 0, FRAME, AudioRecord.READ_BLOCKING)
                if (n < 0) {
                    Log.w(TAG, "micro : lecture refusée ($n)")
                    break
                }
                if (n == 0) continue
                if (pcm != null) for (i in 0 until n) out[i] = pcm[i] / 32768f
                push(out, n, rate)
            }
        } finally {
            runCatching { rec.stop() }
            effects.forEach { runCatching { it.release() } }
            rec.release()
        }
    }

    private companion object {
        const val TAG = "KyberNow"
        const val RATE = 48_000
        /** 10 ms à 48 kHz. */
        const val FRAME = 480
    }
}
