package fr.leslignesclaires.kybernow.session

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Path
import android.graphics.RectF
import android.view.View

/**
 * Le pointeur de l'ordinateur, dessiné par l'app à la position qu'elle tient
 * (Windows masque le sien sans souris branchée). Sa forme est celle de
 * l'ordinateur (une flèche en attendant), deux fois plus grande au téléviseur.
 */
class PointerView(context: Context, private val factor: Float) : View(context) {
    private var shape: Bitmap? = null
    private var hotX = 0
    private var hotY = 0
    private var x = 0f
    private var y = 0f
    private var shown = false
    private val paint = Paint(Paint.FILTER_BITMAP_FLAG or Paint.ANTI_ALIAS_FLAG)
    private val arrow = Path().apply {
        moveTo(0f, 0f); lineTo(0f, 17f); lineTo(4.5f, 13f); lineTo(7.5f, 19.5f); lineTo(10f, 18.5f); lineTo(7f, 12f); lineTo(12.5f, 12f); close()
    }
    private val fill = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFFFFFFFF.toInt() }
    private val stroke = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFF05070A.toInt(); style = Paint.Style.STROKE; strokeWidth = 1.2f }

    init {
        isClickable = false
        isFocusable = false
        visibility = GONE
    }

    fun setShape(bitmap: Bitmap, hx: Int, hy: Int) {
        shape = bitmap
        hotX = hx
        hotY = hy
        invalidate()
    }

    /** Position en pixels de la vue ; `visible` faux : rien de dessiné. */
    fun place(vx: Float, vy: Float, visible: Boolean) {
        if (vx == x && vy == y && visible == shown) return
        x = vx
        y = vy
        shown = visible
        visibility = if (visible) VISIBLE else GONE
        invalidate()
    }

    override fun onDraw(canvas: Canvas) {
        if (!shown) return
        val s = shape
        val k = factor * resources.displayMetrics.density / 2f
        if (s != null) {
            val w = s.width * k
            val h = s.height * k
            canvas.drawBitmap(s, null, RectF(x - hotX * k, y - hotY * k, x - hotX * k + w, y - hotY * k + h), paint)
        } else {
            canvas.save()
            canvas.translate(x, y)
            canvas.scale(k * 1.4f, k * 1.4f)
            canvas.drawPath(arrow, fill)
            canvas.drawPath(arrow, stroke)
            canvas.restore()
        }
    }
}
