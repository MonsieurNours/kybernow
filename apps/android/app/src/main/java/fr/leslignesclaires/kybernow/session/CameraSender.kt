package fr.leslignesclaires.kybernow.session

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.hardware.camera2.CameraAccessException
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.util.Log
import android.util.Range
import android.util.Size
import android.view.Surface
import fr.leslignesclaires.kybernow.R
import fr.leslignesclaires.kybernow.core.Native
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * La caméra de l'appareil vers l'ordinateur (docs/webcam.md, règles dans docs/link-ui.md, « Caméra de l'appareil ») :
 * Camera2 dans la surface d'entrée d'un encodeur H.264 matériel (MediaCodec, temps réel, une image clé toutes les 2 s
 * et à la demande, débit du cœur relu chaque seconde), puis chaque image en Annex B au cœur (`Native.cameraFrame`),
 * SPS et PPS devant chaque image clé. L'image du capteur part telle quelle, avec les quarts de tour qui la redressent :
 * l'ordinateur la tourne.
 */
class CameraSender(private val context: Context, private val handle: () -> Long) {
    /** Un problème (0 accès refusé, 1 caméra prise par une autre app, 2 débranchée, 3 pas d'encodeur) et le nom de la caméra. */
    var onProblem: ((Int, String) -> Unit)? = null
    /** La taille capturée (pour `cameraStart`), sur le fil de la caméra. */
    var onOpened: ((Int, Int) -> Unit)? = null
    /** La rotation de l'écran de l'appareil, en degrés (0, 90, 180, 270), lue à chaque image. */
    var displayDegrees: () -> Int = { 0 }

    private val thread = HandlerThread("kybernow-camera").apply { start() }
    private val handler = Handler(thread.looper)
    private var device: CameraDevice? = null
    private var session: CameraCaptureSession? = null
    private var codec: MediaCodec? = null
    private var input: Surface? = null
    private var config: ByteArray? = null
    private var out: ByteBuffer = ByteBuffer.allocateDirect(256 * 1024).order(ByteOrder.BIG_ENDIAN)
    private var sensor = 0
    private var front = false
    private var bitrate = 0
    private var lastBitrateCheck = 0L
    private var name = ""

    companion object {
        private const val TAG = "KyberNowCamera"

        /** Les caméras de l'appareil, avant d'abord : identifiant et libellé (« Caméra avant », « Caméra arrière »). */
        fun cameras(context: Context): List<Pair<String, String>> {
            val manager = context.getSystemService(CameraManager::class.java) ?: return emptyList()
            return try {
                manager.cameraIdList.mapNotNull { id ->
                    val c = manager.getCameraCharacteristics(id)
                    val caps = c.get(CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES) ?: intArrayOf()
                    if (CameraCharacteristics.REQUEST_AVAILABLE_CAPABILITIES_BACKWARD_COMPATIBLE !in caps) return@mapNotNull null
                    val label = when (c.get(CameraCharacteristics.LENS_FACING)) {
                        CameraCharacteristics.LENS_FACING_FRONT -> context.getString(R.string.camera_front)
                        CameraCharacteristics.LENS_FACING_BACK -> context.getString(R.string.camera_back)
                        else -> context.getString(R.string.camera_external)
                    }
                    id to label
                }.distinctBy { it.second }.sortedBy { if (it.second == context.getString(R.string.camera_front)) 0 else 1 }
            } catch (e: CameraAccessException) {
                emptyList()
            }
        }

        /** L'appareil sait envoyer sa caméra : une caméra au moins (pas sur un téléviseur). */
        fun available(context: Context): Boolean =
            context.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY) && cameras(context).isNotEmpty()

        /** Par défaut : la caméra avant, sinon la première. */
        fun defaultCamera(context: Context): String? = cameras(context).firstOrNull()?.first

        fun permitted(context: Context): Boolean =
            context.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
    }

    /** Ouvre la caméra `id` (nul : celle par défaut) à la définition de l'ordinateur ; `onOpened` suit. */
    @SuppressLint("MissingPermission")
    fun open(id: String?, resolution1080: Boolean) = handler.post {
        closeOnThread()
        if (!permitted(context)) return@post problem(0)
        val manager = context.getSystemService(CameraManager::class.java) ?: return@post problem(2)
        val list = cameras(context)
        val chosen = list.firstOrNull { it.first == id } ?: list.firstOrNull() ?: return@post problem(2)
        name = chosen.second
        val chars = try { manager.getCameraCharacteristics(chosen.first) } catch (e: Exception) { return@post problem(2) }
        sensor = chars.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
        front = chars.get(CameraCharacteristics.LENS_FACING) == CameraCharacteristics.LENS_FACING_FRONT
        val size = pickSize(chars, if (resolution1080) Size(1920, 1080) else Size(1280, 720))
        val fps = chars.get(CameraCharacteristics.CONTROL_AE_AVAILABLE_TARGET_FPS_RANGES)
            ?.filter { it.upper >= 30 }?.minByOrNull { it.upper - it.lower + (it.upper - 30) * 10 }
        if (!makeEncoder(size)) return@post problem(3)
        try {
            manager.openCamera(chosen.first, object : CameraDevice.StateCallback() {
                override fun onOpened(camera: CameraDevice) {
                    device = camera
                    startCapture(camera, fps)
                    onOpened?.invoke(size.width, size.height)
                }

                override fun onDisconnected(camera: CameraDevice) {
                    camera.close()
                    if (device == camera) problem(2)
                }

                override fun onError(camera: CameraDevice, error: Int) {
                    camera.close()
                    if (device == camera || device == null) problem(if (error == ERROR_CAMERA_IN_USE || error == ERROR_MAX_CAMERAS_IN_USE) 1 else 2)
                }
            }, handler)
        } catch (e: SecurityException) {
            problem(0)
        } catch (e: CameraAccessException) {
            problem(1)
        }
    }

    /** Arrête la capture et l'encodeur. */
    fun close() = handler.post { closeOnThread() }

    /** La prochaine image sera une image clé. */
    fun forceKeyframe() = handler.post {
        codec?.setParameters(Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0) })
    }

    fun release() {
        close()
        thread.quitSafely()
    }

    private fun problem(which: Int) {
        closeOnThread()
        onProblem?.invoke(which, name)
    }

    /** La taille demandée si la caméra la sait, sinon la plus grande au même rapport en dessous, sinon la plus proche. */
    private fun pickSize(chars: CameraCharacteristics, wanted: Size): Size {
        val sizes = chars.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)?.getOutputSizes(MediaCodec::class.java)
            ?: return wanted
        sizes.firstOrNull { it.width == wanted.width && it.height == wanted.height }?.let { return it }
        val below = sizes.filter { it.width <= wanted.width && it.height <= wanted.height }
        return below.filter { it.width * 9 == it.height * 16 }.maxByOrNull { it.width * it.height }
            ?: below.maxByOrNull { it.width * it.height }
            ?: sizes.minByOrNull { it.width * it.height }
            ?: wanted
    }

    private fun makeEncoder(size: Size): Boolean {
        bitrate = Native.cameraTargetBitrate(handle()).takeIf { it > 0 } ?: 1_500_000
        val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, size.width, size.height).apply {
            setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface)
            setInteger(MediaFormat.KEY_BIT_RATE, bitrate)
            setInteger(MediaFormat.KEY_FRAME_RATE, 30)
            setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 2)
            setInteger(MediaFormat.KEY_PRIORITY, 0)
            setInteger(MediaFormat.KEY_LATENCY, 1)
            setInteger(MediaFormat.KEY_BITRATE_MODE, MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR)
        }
        return try {
            val c = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
            c.setCallback(object : MediaCodec.Callback() {
                override fun onInputBufferAvailable(codec: MediaCodec, index: Int) {}
                override fun onOutputBufferAvailable(codec: MediaCodec, index: Int, info: MediaCodec.BufferInfo) = deliver(codec, index, info)
                override fun onError(codec: MediaCodec, e: MediaCodec.CodecException) {
                    Log.w(TAG, "encodeur : ${e.diagnosticInfo}")
                }
                override fun onOutputFormatChanged(codec: MediaCodec, format: MediaFormat) {}
            }, handler)
            c.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            input = c.createInputSurface()
            c.start()
            codec = c
            true
        } catch (e: Exception) {
            Log.w(TAG, "encodeur H.264 indisponible : $e")
            false
        }
    }

    private fun startCapture(camera: CameraDevice, fps: Range<Int>?) {
        val surface = input ?: return problem(3)
        try {
            @Suppress("DEPRECATION")
            camera.createCaptureSession(listOf(surface), object : CameraCaptureSession.StateCallback() {
                override fun onConfigured(s: CameraCaptureSession) {
                    session = s
                    val request = camera.createCaptureRequest(CameraDevice.TEMPLATE_RECORD).apply {
                        addTarget(surface)
                        fps?.let { set(CaptureRequest.CONTROL_AE_TARGET_FPS_RANGE, it) }
                    }.build()
                    try { s.setRepeatingRequest(request, null, handler) } catch (e: Exception) { problem(1) }
                }

                override fun onConfigureFailed(s: CameraCaptureSession) = problem(1)
            }, handler)
        } catch (e: CameraAccessException) {
            problem(1)
        }
    }

    /** Une sortie de l'encodeur : les paramètres gardés, l'image (paramètres devant une image clé) au cœur. */
    private fun deliver(c: MediaCodec, index: Int, info: MediaCodec.BufferInfo) {
        try {
            val buffer = c.getOutputBuffer(index) ?: return
            if (info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0) {
                config = ByteArray(info.size).also { buffer.position(info.offset); buffer.get(it) }
                return
            }
            if (info.size <= 0) return
            val key = info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0
            val prefix = if (key) config else null
            val need = info.size + (prefix?.size ?: 0)
            if (out.capacity() < need) out = ByteBuffer.allocateDirect(need * 2)
            out.clear()
            prefix?.let { out.put(it) }
            buffer.position(info.offset)
            buffer.limit(info.offset + info.size)
            out.put(buffer)
            val result = Native.cameraFrame(handle(), out, 0, need, info.presentationTimeUs, key, quarterTurns())
            if (result == 1) forceKeyframe()
            updateBitrate()
        } finally {
            c.releaseOutputBuffer(index, false)
        }
    }

    /** Les quarts de tour (sens horaire) qui redressent l'image du capteur, d'après la tenue de l'appareil. */
    private fun quarterTurns(): Int {
        val deg = displayDegrees()
        val rotation = if (front) (sensor + deg) % 360 else (sensor - deg + 360) % 360
        return rotation / 90
    }

    private fun updateBitrate() {
        val now = System.nanoTime()
        if (now - lastBitrateCheck < 1_000_000_000L) return
        lastBitrateCheck = now
        val target = Native.cameraTargetBitrate(handle())
        if (target > 0 && target != bitrate) {
            bitrate = target
            codec?.setParameters(Bundle().apply { putInt(MediaCodec.PARAMETER_KEY_VIDEO_BITRATE, target) })
        }
    }

    private fun closeOnThread() {
        try { session?.close() } catch (_: Exception) {}
        session = null
        try { device?.close() } catch (_: Exception) {}
        device = null
        try { codec?.stop() } catch (_: Exception) {}
        try { codec?.release() } catch (_: Exception) {}
        codec = null
        input?.release()
        input = null
        config = null
    }
}
