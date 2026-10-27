package fr.leslignesclaires.kybernow.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.unit.dp

/**
 * Le QR code d'un texte (l'adresse d'Admin, le lien d'un partage), noir sur blanc, net à toute
 * taille. [description] : ce que lit un lecteur d'écran (par défaut le texte).
 */
@Composable
fun QrCode(text: String, size: Float, description: String = text) {
    val s = LocalScale.current
    // Un texte vide ou trop long n'a pas de QR code : rien n'est dessiné (zxing lève une exception, qui ferait tomber l'app).
    val image = remember(text) {
        runCatching {
            val m = com.google.zxing.qrcode.QRCodeWriter().encode(text, com.google.zxing.BarcodeFormat.QR_CODE, 0, 0, mapOf(com.google.zxing.EncodeHintType.MARGIN to 2))
            val w = m.width
            val h = m.height
            val px = IntArray(w * h) { i -> if (m.get(i % w, i / w)) 0xFF05070A.toInt() else 0xFFFFFFFF.toInt() }
            android.graphics.Bitmap.createBitmap(px, w, h, android.graphics.Bitmap.Config.ARGB_8888).asImageBitmap()
        }.getOrNull()
    } ?: return
    Image(
        image, contentDescription = description,
        modifier = Modifier.width(s.dp(size)).height(s.dp(size)).clip(RoundedCornerShape(10.dp)),
        filterQuality = FilterQuality.None,
    )
}
