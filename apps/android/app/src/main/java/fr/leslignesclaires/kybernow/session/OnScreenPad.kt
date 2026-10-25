package fr.leslignesclaires.kybernow.session

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import fr.leslignesclaires.kybernow.core.Pad
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.hypot
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

/** Ce qui se sent à l'appui ([Haptics]) : jamais de vibration continue au toucher. */
enum class Feel {
    /** Un bouton, la croix, un stick enfoncé : un clic net. */
    CLICK,
    /** Une gâchette : un clic léger au début de la course… */
    TRIGGER_START,
    /** … et un clic franc en bout de course. */
    TRIGGER_END,
    /** Un stick quitte le centre : un tic. */
    STICK_LEAVE,
    /** Un stick change de direction (huit) : un tic très léger. */
    STICK_SECTOR,
    /** Un stick arrive en butée : un tic. */
    STICK_EDGE,
}

/** Ce que la manette à l'écran produit : boutons et axes de [Pad] pour sa place de joueur, et ce qui se sent. */
interface PadOut {
    fun button(button: Int, pressed: Boolean)
    /** Sticks −32768 à 32767 (Y positif vers le bas), gâchettes 0 à 32767. */
    fun axis(axis: Int, value: Int)
    fun feel(f: Feel)

    companion object {
        val NONE = object : PadOut {
            override fun button(button: Int, pressed: Boolean) {}
            override fun axis(axis: Int, value: Int) {}
            override fun feel(f: Feel) {}
        }
    }
}

/** Les commandes de la manette à l'écran ; [simple] : présentes dans la disposition « Simple ». */
enum class PadControl(val simple: Boolean) {
    LT(false), LB(false), LEFT_STICK(true), DPAD(true),
    RT(false), RB(false), FACE(true), RIGHT_STICK(false),
    VIEW(false), GUIDE(false), MENU(true), SWITCH(true),
}

/**
 * Une commande dessinée, en pixels : centre, demi-largeur et demi-hauteur (rayon pour un rond),
 * et la marge de toucher autour du dessin ([reach]) : les zones de toucher sont plus grandes que
 * ce qu'on voit.
 */
class PadShape(val control: PadControl, val cx: Float, val cy: Float, val hw: Float, val hh: Float, val round: Boolean, val reach: Float) {
    val top get() = cy - hh
    val bottom get() = cy + hh
    val left get() = cx - hw
    val right get() = cx + hw

    /** Distance du point au dessin (0 dedans). */
    fun gap(x: Float, y: Float): Float =
        if (round) max(0f, hypot(x - cx, y - cy) - hw)
        else hypot(max(abs(x - cx) - hw, 0f), max(abs(y - cy) - hh, 0f))

    fun reaches(x: Float, y: Float) = gap(x, y) <= reach
}

/**
 * La disposition de la manette, en pixels, pour un écran et ses zones sûres (encoche, gestes du
 * système) : une manette Xbox pour les deux pouces, en paysage. À gauche, LT et LB en haut, le
 * stick gauche, la croix en dessous vers l'intérieur ; à droite, RT et RB, A, B, X, Y, le stick
 * droit en dessous vers l'intérieur ; au milieu, en bas, Affichage, Guide et Menu, et le petit
 * bouton de disposition au-dessus de Guide. Posée en bas de l'écran : au téléphone, elle en prend
 * la hauteur ; sur un écran plus haut (tablette, portrait), elle remonte un peu, sous les pouces
 * d'une tablette tenue par les côtés. La disposition « Simple » garde les mêmes places (Menu
 * passe au milieu, à la place de Guide) : on passe de l'une à l'autre sans que rien ne bouge.
 *
 * Unité [u] : le dp de la taille choisie (× 0,85 ; 1 ; 1,18), réduit si la manette ne tient pas.
 */
class PadGeometry(val u: Float, private val shapes: Map<PadControl, PadShape>) {
    val faceOffset get() = 46 * u
    val faceRadius get() = 24 * u
    val knobRadius get() = 25 * u
    /** Course du bouton du stick dans le dessin. */
    val knobTravel get() = 31 * u
    /** Course du doigt pour un stick à fond. */
    val stickTravel get() = 44 * u
    /** Demi-largeur d'une branche de la croix. */
    val dpadArm get() = 17 * u
    /** Au-delà, un doigt a glissé (toucher, double toucher). */
    val slop get() = 10 * u

    operator fun get(c: PadControl): PadShape = shapes.getValue(c)

    fun shapes(simple: Boolean): List<PadShape> = shapes.values.filter { !simple || it.control.simple }

    /** La commande sous le doigt : la plus proche dont la zone le contient, ou aucune. */
    fun hit(x: Float, y: Float, simple: Boolean): PadControl? =
        shapes(simple).filter { it.reaches(x, y) }
            .minWithOrNull(compareBy<PadShape>({ it.gap(x, y) }, { hypot(x - it.cx, y - it.cy) }))?.control

    /** Le centre d'un bouton A, B, X ou Y. */
    fun faceCenter(button: Int): Pair<Float, Float> {
        val f = this[PadControl.FACE]
        val o = faceOffset
        return when (button) {
            Pad.A -> f.cx to f.cy + o
            Pad.B -> f.cx + o to f.cy
            Pad.X -> f.cx - o to f.cy
            else -> f.cx to f.cy - o
        }
    }

    companion object {
        val SIZES = floatArrayOf(0.85f, 1f, 1.18f)
        /** Ce que prend la manette, en u, marges comprises. */
        const val WIDTH = 632f
        const val HEIGHT = 328f

        /**
         * [left], [top], [right], [bottom] : les zones sûres (px) ; [density] : px par dp ;
         * [size] : 0 petite, 1 moyenne, 2 grande ; [simple] : disposition « Simple ».
         */
        fun compute(width: Float, height: Float, left: Float, top: Float, right: Float, bottom: Float, density: Float, size: Int, simple: Boolean = false): PadGeometry {
            val base = density * SIZES[size.coerceIn(0, 2)]
            val availW = max(1f, width - left - right)
            val availH = max(1f, height - top - bottom)
            val u = base * min(1f, min(availW / (WIDTH * base), availH / (HEIGHT * base)))
            val m = 10 * u
            val l = left + m
            val r = width - right - m
            // Un écran bien plus haut que la manette : elle remonte, 120 u au plus.
            val lift = ((availH - HEIGHT * u - 160 * u) * 0.5f).coerceIn(0f, 120 * u)
            val b = height - bottom - m - lift
            val y0 = b - 308 * u
            val mid = (l + r) / 2
            val row = b - 22 * u
            fun rect(c: PadControl, cx: Float, cy: Float, hw: Float, hh: Float, reach: Float) = c to PadShape(c, cx, cy, hw * u, hh * u, false, reach * u)
            fun round(c: PadControl, cx: Float, cy: Float, radius: Float, reach: Float) = c to PadShape(c, cx, cy, radius * u, radius * u, true, reach * u)
            return PadGeometry(
                u,
                mapOf(
                    rect(PadControl.LT, l + 25 * u, y0 + 32 * u, 25f, 32f, 12f),
                    rect(PadControl.LB, l + 108 * u, y0 + 43 * u, 46f, 21f, 12f),
                    round(PadControl.LEFT_STICK, l + 64 * u, y0 + 142 * u, 56f, 22f),
                    rect(PadControl.DPAD, l + 156 * u, y0 + 252 * u, 48f, 48f, 16f),
                    rect(PadControl.RT, r - 25 * u, y0 + 32 * u, 25f, 32f, 12f),
                    rect(PadControl.RB, r - 108 * u, y0 + 43 * u, 46f, 21f, 12f),
                    round(PadControl.FACE, r - 64 * u, y0 + 142 * u, 70f, 16f),
                    round(PadControl.RIGHT_STICK, r - 156 * u, y0 + 252 * u, 56f, 22f),
                    rect(PadControl.VIEW, mid - 66 * u, row, 24f, 16f, 12f),
                    round(PadControl.GUIDE, mid, row, 22f, 12f),
                    rect(PadControl.MENU, if (simple) mid else mid + 66 * u, row, 24f, 16f, 12f),
                    round(PadControl.SWITCH, mid, row - 56 * u, 15f, 10f),
                ),
            )
        }
    }
}

/**
 * Où en est un stick, pour ce qu'il fait sentir : sorti du centre ou non, sa direction (huit
 * secteurs de 45°, 0 à droite, dans le sens des aiguilles d'une montre puisque Y descend), en
 * butée ou non. Chaque seuil a son hystérésis : un doigt qui tremble à une frontière ne fait pas
 * de tics en rafale.
 */
data class StickZone(val out: Boolean = false, val sector: Int = -1, val edge: Boolean = false) {
    companion object {
        val CENTER = StickZone()
        const val LEAVE = 0.22f
        const val BACK = 0.16f
        const val EDGE = 0.97f
        const val EDGE_BACK = 0.9f
        /** Il faut dépasser la frontière d'un secteur de tant de degrés pour en changer. */
        const val SECTOR_MARGIN = 6f

        /** La zone d'un stick en ([x], [y]) (−1 à 1, Y vers le bas), depuis la précédente. */
        fun of(x: Float, y: Float, before: StickZone): StickZone {
            val m = hypot(x, y)
            val out = m >= if (before.out) BACK else LEAVE
            if (!out) return CENTER
            val edge = m >= if (before.edge) EDGE_BACK else EDGE
            val angle = (Math.toDegrees(atan2(y, x).toDouble()).toFloat() + 360f) % 360f
            val raw = ((angle + 22.5f) / 45f).toInt() % 8
            val keep = before.out && before.sector >= 0 && raw != before.sector && degrees(angle, before.sector * 45f) <= 22.5f + SECTOR_MARGIN
            return StickZone(true, if (keep) before.sector else raw, edge)
        }

        /** Le tic du passage de [before] à [after], ou aucun : butée d'abord, puis sortie du centre, puis direction. */
        fun tick(before: StickZone, after: StickZone): Feel? = when {
            !after.out -> null
            after.edge && !before.edge -> Feel.STICK_EDGE
            !before.out -> Feel.STICK_LEAVE
            after.sector != before.sector -> Feel.STICK_SECTOR
            else -> null
        }

        private fun degrees(a: Float, b: Float): Float {
            val d = abs(a - b) % 360f
            return if (d > 180f) 360f - d else d
        }
    }
}

/** Un stick pour le dessin : position (−1 à 1, Y vers le bas), touché, enfoncé (L3, R3). */
data class StickState(val x: Float = 0f, val y: Float = 0f, val touched: Boolean = false, val clicked: Boolean = false)

/**
 * La manette à l'écran : chaque doigt prend la commande qu'il touche et la garde jusqu'à ce qu'il
 * se lève (plusieurs doigts à la fois : deux sticks et des boutons). Les boutons de [Pad] et les
 * axes partent par [out], pour la place de joueur de la manette ; l'état ([pressed], [left],
 * [right], [lt], [rt]) sert au dessin.
 *
 * - **Sticks** : le doigt posé sur le bouton du stick part de là (pas de saut) ; posé ailleurs sur
 *   le stick, il le pousse tout de suite vers lui. Toucher deux fois un stick l'enfonce (L3, R3)
 *   tant que le second doigt reste posé (disposition « Complète »).
 * - **Gâchettes** progressives : la position du doigt sur la gâchette donne la valeur, d'un appui
 *   léger en haut à fond en bas ; glisser vers le bas l'enfonce, comme on tire une gâchette.
 * - **Croix** : huit directions (les diagonales enfoncent deux flèches), le doigt glisse de l'une
 *   à l'autre. **A, B, X, Y** : le doigt passe d'un bouton à l'autre en glissant.
 * - Un toucher hors des commandes ne fait rien sur l'ordinateur ([onTapOutside] : il replie le
 *   clavier).
 */
class OnScreenPad(private val out: PadOut) {
    /** La disposition à l'écran, posée par le dessin. */
    var geometry: PadGeometry? = null
    /** Disposition « Simple » : stick gauche, croix, A, B, X, Y et Menu. */
    var simple by mutableStateOf(false)
        private set
    /** Le petit bouton de disposition. */
    var onSwitch: () -> Unit = {}
    /** Un toucher hors des commandes. */
    var onTapOutside: () -> Unit = {}

    var pressed by mutableStateOf(emptySet<Int>())
    var left by mutableStateOf(StickState())
    var right by mutableStateOf(StickState())
    var lt by mutableFloatStateOf(0f)
    var rt by mutableFloatStateOf(0f)
    var switchDown by mutableStateOf(false)

    private sealed class Touch(val control: PadControl?, val x0: Float, val y0: Float, val at: Long) {
        var moved = false
        class Stick(val isRight: Boolean, val ox: Float, val oy: Float, val clicked: Boolean, x0: Float, y0: Float, at: Long) :
            Touch(if (isRight) PadControl.RIGHT_STICK else PadControl.LEFT_STICK, x0, y0, at) { var zone = StickZone.CENTER }
        class Trigger(val isRight: Boolean, x0: Float, y0: Float, at: Long) : Touch(if (isRight) PadControl.RT else PadControl.LT, x0, y0, at) { var end = false }
        class Button(control: PadControl, val button: Int, x0: Float, y0: Float, at: Long) : Touch(control, x0, y0, at)
        class Face(var button: Int, x0: Float, y0: Float, at: Long) : Touch(PadControl.FACE, x0, y0, at)
        class Dpad(x0: Float, y0: Float, at: Long) : Touch(PadControl.DPAD, x0, y0, at) { var dirs = emptySet<Int>() }
        class Switch(x0: Float, y0: Float, at: Long) : Touch(PadControl.SWITCH, x0, y0, at) { var x = x0; var y = y0 }
        class Outside(x0: Float, y0: Float, at: Long) : Touch(null, x0, y0, at)
        /** Un second doigt sur une commande déjà tenue : rien. */
        class Ignored(x0: Float, y0: Float, at: Long) : Touch(null, x0, y0, at)
    }

    private val touches = LinkedHashMap<Long, Touch>()
    /** Dernier toucher bref de chaque stick (gauche, droit), pour le double toucher. */
    private val lastTap = longArrayOf(NEVER, NEVER)
    /** Dernière valeur partie de chaque axe. */
    private val sent = IntArray(6)

    fun down(id: Long, x: Float, y: Float, at: Long) {
        val g = geometry ?: return
        val c = g.hit(x, y, simple)
        val held = touches.values.any { it.control == c && c != null }
        val t: Touch = when {
            c == null -> Touch.Outside(x, y, at)
            held && c != PadControl.FACE -> Touch.Ignored(x, y, at)
            c == PadControl.LEFT_STICK || c == PadControl.RIGHT_STICK -> startStick(g, c == PadControl.RIGHT_STICK, x, y, at)
            c == PadControl.LT || c == PadControl.RT -> Touch.Trigger(c == PadControl.RT, x, y, at).also {
                // Posée tout en bas, la gâchette est d'emblée à fond : le clic franc seul.
                if (triggerValue(g[c], y) < 1f) out.feel(Feel.TRIGGER_START)
                trigger(g, it, y)
            }
            c == PadControl.DPAD -> Touch.Dpad(x, y, at).also { dpad(g, it, x, y) }
            c == PadControl.FACE -> Touch.Face(nearestFace(g, x, y), x, y, at).also { press(it.button); out.feel(Feel.CLICK) }
            c == PadControl.SWITCH -> Touch.Switch(x, y, at).also { switchDown = true; out.feel(Feel.CLICK) }
            else -> Touch.Button(c, buttonOf(c), x, y, at).also { press(it.button); out.feel(Feel.CLICK) }
        }
        touches[id] = t
    }

    fun move(id: Long, x: Float, y: Float) {
        val g = geometry ?: return
        val t = touches[id] ?: return
        if (!t.moved && hypot(x - t.x0, y - t.y0) > g.slop) t.moved = true
        when (t) {
            is Touch.Stick -> stick(g, t, x, y)
            is Touch.Trigger -> trigger(g, t, y)
            is Touch.Dpad -> dpad(g, t, x, y)
            is Touch.Face -> {
                val n = nearestFace(g, x, y)
                val (nx, ny) = g.faceCenter(n)
                if (n != t.button && hypot(x - nx, y - ny) <= g.faceRadius + 6 * g.u) {
                    releaseFace(t)
                    t.button = n
                    press(n)
                    out.feel(Feel.CLICK)
                }
            }
            is Touch.Switch -> { t.x = x; t.y = y }
            else -> {}
        }
    }

    fun up(id: Long, at: Long) {
        val t = touches.remove(id) ?: return
        end(t)
        when (t) {
            is Touch.Stick -> lastTap[if (t.isRight) 1 else 0] = if (!t.clicked && !t.moved && at - t.at <= TAP_MS) at else NEVER
            is Touch.Switch -> if (geometry?.get(PadControl.SWITCH)?.reaches(t.x, t.y) == true) onSwitch()
            is Touch.Outside -> if (!t.moved && at - t.at <= TAP_MS) onTapOutside()
            else -> {}
        }
    }

    /** Tout relâché, sur l'ordinateur et au dessin (manette retirée, arrière-plan, fin de session). */
    fun releaseAll() {
        touches.values.forEach(::end)
        touches.clear()
        pressed.forEach { out.button(it, false) }
        pressed = emptySet()
        for (a in sent.indices) if (sent[a] != 0) axis(a, 0f)
        left = StickState()
        right = StickState()
        lt = 0f
        rt = 0f
        switchDown = false
        lastTap.fill(NEVER)
    }

    /** Passe d'une disposition à l'autre : les commandes qui disparaissent sont relâchées, les autres restent tenues. */
    fun setLayout(simple: Boolean) {
        if (this.simple == simple) return
        if (simple) {
            val gone = touches.filterValues { it.control != null && !it.control.simple }
            gone.forEach { (id, t) -> touches.remove(id); end(t) }
            // Le stick gauche reste, sans L3 (absent de la disposition « Simple »).
            if (Pad.LEFT_STICK in pressed) release(Pad.LEFT_STICK)
            left = left.copy(clicked = false)
        }
        this.simple = simple
    }

    // MARK: - Commandes

    private fun startStick(g: PadGeometry, isRight: Boolean, x: Float, y: Float, at: Long): Touch.Stick {
        val s = g[if (isRight) PadControl.RIGHT_STICK else PadControl.LEFT_STICK]
        val onKnob = hypot(x - s.cx, y - s.cy) <= g.knobRadius * 1.3f
        val clicked = !simple && at - lastTap[if (isRight) 1 else 0] <= DOUBLE_TAP_MS
        val t = Touch.Stick(isRight, if (onKnob) x else s.cx, if (onKnob) y else s.cy, clicked, x, y, at)
        if (clicked) {
            press(if (isRight) Pad.RIGHT_STICK else Pad.LEFT_STICK)
            out.feel(Feel.CLICK)
        }
        stick(g, t, x, y)
        return t
    }

    private fun stick(g: PadGeometry, t: Touch.Stick, x: Float, y: Float) {
        var nx = (x - t.ox) / g.stickTravel
        var ny = (y - t.oy) / g.stickTravel
        val m = hypot(nx, ny)
        if (m > 1f) { nx /= m; ny /= m }
        // Un rien de zone morte : le doigt posé ne fait pas dériver le stick.
        if (m < DEAD) { nx = 0f; ny = 0f }
        val zone = StickZone.of(nx, ny, t.zone)
        StickZone.tick(t.zone, zone)?.let(out::feel)
        t.zone = zone
        val (ax, ay) = if (t.isRight) Pad.RIGHT_X to Pad.RIGHT_Y else Pad.LEFT_X to Pad.LEFT_Y
        axis(ax, nx)
        axis(ay, ny)
        val st = StickState(nx, ny, touched = true, clicked = t.clicked)
        if (t.isRight) right = st else left = st
    }

    private fun trigger(g: PadGeometry, t: Touch.Trigger, y: Float) {
        val s = g[if (t.isRight) PadControl.RT else PadControl.LT]
        val v = triggerValue(s, y)
        if (v >= 1f && !t.end) { t.end = true; out.feel(Feel.TRIGGER_END) }
        if (v < 0.9f) t.end = false
        axis(if (t.isRight) Pad.R2 else Pad.L2, v)
        if (t.isRight) rt = v else lt = v
    }

    private fun dpad(g: PadGeometry, t: Touch.Dpad, x: Float, y: Float) {
        val dirs = dpadDirections(g[PadControl.DPAD], x, y)
        if (dirs == t.dirs) return
        (t.dirs - dirs).forEach(::release)
        val added = dirs - t.dirs
        added.forEach(::press)
        t.dirs = dirs
        if (added.isNotEmpty()) out.feel(Feel.CLICK)
    }

    /** Ce qu'un doigt tient, relâché sur l'ordinateur et au dessin. */
    private fun end(t: Touch) {
        when (t) {
            is Touch.Stick -> {
                if (t.clicked) release(if (t.isRight) Pad.RIGHT_STICK else Pad.LEFT_STICK)
                val (ax, ay) = if (t.isRight) Pad.RIGHT_X to Pad.RIGHT_Y else Pad.LEFT_X to Pad.LEFT_Y
                axis(ax, 0f)
                axis(ay, 0f)
                if (t.isRight) right = StickState() else left = StickState()
            }
            is Touch.Trigger -> {
                axis(if (t.isRight) Pad.R2 else Pad.L2, 0f)
                if (t.isRight) rt = 0f else lt = 0f
            }
            is Touch.Button -> release(t.button)
            is Touch.Face -> releaseFace(t)
            is Touch.Dpad -> { t.dirs.forEach(::release); t.dirs = emptySet() }
            is Touch.Switch -> switchDown = false
            else -> {}
        }
    }

    /** Le bouton d'un doigt sur A, B, X, Y, sauf si un autre doigt le tient aussi. */
    private fun releaseFace(t: Touch.Face) {
        if (touches.values.none { it !== t && it is Touch.Face && it.button == t.button }) release(t.button)
    }

    private fun press(b: Int) {
        if (b in pressed) return
        pressed = pressed + b
        out.button(b, true)
    }

    private fun release(b: Int) {
        if (b !in pressed) return
        pressed = pressed - b
        out.button(b, false)
    }

    /** −1 à 1 pour un stick, 0 à 1 pour une gâchette ; ne part que s'il a changé. */
    private fun axis(a: Int, v: Float) {
        val value = (v * 32767f).roundToInt().coerceIn(-32768, 32767)
        if (sent[a] == value) return
        sent[a] = value
        out.axis(a, value)
    }

    companion object {
        /** Un toucher bref : un double toucher de stick, un toucher hors des commandes. */
        const val TAP_MS = 300L
        const val DOUBLE_TAP_MS = 320L
        const val DEAD = 0.05f
        /** Au plus léger, une gâchette touchée vaut tant (un toucher bref reste un appui franc au milieu). */
        const val TRIGGER_MIN = 0.15f
        private const val NEVER = Long.MIN_VALUE / 2

        fun buttonOf(c: PadControl): Int = when (c) {
            PadControl.LB -> Pad.L1
            PadControl.RB -> Pad.R1
            PadControl.VIEW -> Pad.BACK
            PadControl.GUIDE -> Pad.GUIDE
            else -> Pad.START
        }

        /** La gâchette sous le doigt : de [TRIGGER_MIN] en haut à 1 en bas (atteint un peu avant le bord). */
        fun triggerValue(s: PadShape, y: Float): Float {
            val t = ((y - s.top) / (s.hh * 2 * 0.9f)).coerceIn(0f, 1f)
            return if (t >= 1f) 1f else TRIGGER_MIN + (1f - TRIGGER_MIN) * t
        }

        /** Les flèches sous le doigt : une direction franche à moins de 30° d'un axe, sinon la diagonale ; rien au centre. */
        fun dpadDirections(s: PadShape, x: Float, y: Float): Set<Int> {
            val dx = x - s.cx
            val dy = y - s.cy
            if (hypot(dx, dy) < s.hw * 0.22f) return emptySet()
            val a = (Math.toDegrees(atan2(dy, dx).toDouble()).toFloat() + 360f) % 360f
            for ((c, b) in listOf(0f to Pad.RIGHT, 90f to Pad.DOWN, 180f to Pad.LEFT, 270f to Pad.UP)) {
                val d = abs(a - c).let { if (it > 180f) 360f - it else it }
                if (d <= 30f) return setOf(b)
            }
            return when {
                a < 90f -> setOf(Pad.RIGHT, Pad.DOWN)
                a < 180f -> setOf(Pad.DOWN, Pad.LEFT)
                a < 270f -> setOf(Pad.LEFT, Pad.UP)
                else -> setOf(Pad.UP, Pad.RIGHT)
            }
        }

        private fun nearestFace(g: PadGeometry, x: Float, y: Float): Int =
            listOf(Pad.A, Pad.B, Pad.X, Pad.Y).minBy { b -> g.faceCenter(b).let { (cx, cy) -> hypot(x - cx, y - cy) } }
    }
}
