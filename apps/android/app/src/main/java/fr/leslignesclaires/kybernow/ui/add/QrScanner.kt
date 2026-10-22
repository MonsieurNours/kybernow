package fr.leslignesclaires.kybernow.ui.add

import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.concurrent.Executors

/** Aperçu de la caméra et lecture des QR codes (zxing, sans services Google). */
@Composable
fun QrScanner(modifier: Modifier, onText: (String) -> Unit) {
    val owner = LocalLifecycleOwner.current
    val executor = remember { Executors.newSingleThreadExecutor() }
    val reader = remember {
        MultiFormatReader().apply { setHints(mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE))) }
    }
    DisposableEffect(Unit) { onDispose { executor.shutdown() } }
    Box(modifier.clip(RoundedCornerShape(18.dp))) {
        AndroidView(factory = { ctx ->
            val view = PreviewView(ctx)
            val future = ProcessCameraProvider.getInstance(ctx)
            future.addListener({
                val provider = future.get()
                val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
                val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
                analysis.setAnalyzer(executor) { image ->
                    val plane = image.planes[0]
                    val buf = plane.buffer
                    val data = ByteArray(buf.remaining()).also { buf.get(it) }
                    val source = PlanarYUVLuminanceSource(data, plane.rowStride, image.height, 0, 0, image.width, image.height, false)
                    val text = runCatching { reader.decodeWithState(BinaryBitmap(HybridBinarizer(source))).text }.getOrNull()
                    reader.reset()
                    image.close()
                    if (text != null) ContextCompat.getMainExecutor(ctx).execute { onText(text) }
                }
                provider.unbindAll()
                runCatching { provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis) }
            }, ContextCompat.getMainExecutor(ctx))
            view
        }, modifier = Modifier)
    }
}
