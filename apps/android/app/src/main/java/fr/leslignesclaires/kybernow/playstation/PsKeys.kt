package fr.leslignesclaires.kybernow.playstation

import android.view.KeyEvent

/**
 * Les touches du clavier pour une console (docs/playstation.md, « Clavier et souris ») : le cœur traduit le clavier en
 * manette d'après des usages USB HID (page 7), par la place de la touche, comme l'app Mac (kVK → HID) : la disposition
 * par défaut (ZQSD en AZERTY, WASD en QWERTY) tombe sous les doigts quelle que soit la langue du clavier. Android donne
 * la place de la touche par `KeyEvent.scanCode` (code evdev de Linux) ; à défaut, par son code de touche.
 */
object PsKeys {
    /** evdev → HID. */
    private val EVDEV = mapOf(
        1 to 0x29, 2 to 0x1E, 3 to 0x1F, 4 to 0x20, 5 to 0x21, 6 to 0x22, 7 to 0x23, 8 to 0x24, 9 to 0x25, 10 to 0x26, 11 to 0x27,
        12 to 0x2D, 13 to 0x2E, 14 to 0x2A, 15 to 0x2B,
        16 to 0x14, 17 to 0x1A, 18 to 0x08, 19 to 0x15, 20 to 0x17, 21 to 0x1C, 22 to 0x18, 23 to 0x0C, 24 to 0x12, 25 to 0x13,
        26 to 0x2F, 27 to 0x30, 28 to 0x28, 29 to 0xE0,
        30 to 0x04, 31 to 0x16, 32 to 0x07, 33 to 0x09, 34 to 0x0A, 35 to 0x0B, 36 to 0x0D, 37 to 0x0E, 38 to 0x0F, 39 to 0x33,
        40 to 0x34, 41 to 0x35, 42 to 0xE1, 43 to 0x31,
        44 to 0x1D, 45 to 0x1B, 46 to 0x06, 47 to 0x19, 48 to 0x05, 49 to 0x11, 50 to 0x10, 51 to 0x36, 52 to 0x37, 53 to 0x38,
        54 to 0xE5, 55 to 0x55, 56 to 0xE2, 57 to 0x2C, 58 to 0x39,
        59 to 0x3A, 60 to 0x3B, 61 to 0x3C, 62 to 0x3D, 63 to 0x3E, 64 to 0x3F, 65 to 0x40, 66 to 0x41, 67 to 0x42, 68 to 0x43,
        69 to 0x53, 70 to 0x47, 71 to 0x5F, 72 to 0x60, 73 to 0x61, 74 to 0x56, 75 to 0x5C, 76 to 0x5D, 77 to 0x5E, 78 to 0x57,
        79 to 0x59, 80 to 0x5A, 81 to 0x5B, 82 to 0x62, 83 to 0x63, 86 to 0x64, 87 to 0x44, 88 to 0x45,
        96 to 0x58, 97 to 0xE4, 98 to 0x54, 100 to 0xE6, 102 to 0x4A, 103 to 0x52, 104 to 0x4B, 105 to 0x50, 106 to 0x4F,
        107 to 0x4D, 108 to 0x51, 109 to 0x4E, 110 to 0x49, 111 to 0x4C, 119 to 0x48, 125 to 0xE3, 126 to 0xE7, 127 to 0x65,
    )

    /** Code de touche d'Android → HID, quand le clavier ne donne pas de code evdev (claviers virtuels, télécommandes). */
    private val KEYCODE = mapOf(
        KeyEvent.KEYCODE_ESCAPE to 0x29, KeyEvent.KEYCODE_ENTER to 0x28, KeyEvent.KEYCODE_SPACE to 0x2C, KeyEvent.KEYCODE_TAB to 0x2B,
        KeyEvent.KEYCODE_DEL to 0x2A, KeyEvent.KEYCODE_FORWARD_DEL to 0x4C, KeyEvent.KEYCODE_DPAD_UP to 0x52, KeyEvent.KEYCODE_DPAD_DOWN to 0x51,
        KeyEvent.KEYCODE_DPAD_LEFT to 0x50, KeyEvent.KEYCODE_DPAD_RIGHT to 0x4F, KeyEvent.KEYCODE_SHIFT_LEFT to 0xE1, KeyEvent.KEYCODE_SHIFT_RIGHT to 0xE5,
        KeyEvent.KEYCODE_CTRL_LEFT to 0xE0, KeyEvent.KEYCODE_CTRL_RIGHT to 0xE4, KeyEvent.KEYCODE_ALT_LEFT to 0xE2, KeyEvent.KEYCODE_ALT_RIGHT to 0xE6,
        KeyEvent.KEYCODE_GRAVE to 0x35, KeyEvent.KEYCODE_BACKSLASH to 0x31,
    ) + (0..25).associate { KeyEvent.KEYCODE_A + it to 0x04 + it } + (0..9).associate { (if (it == 0) KeyEvent.KEYCODE_0 else KeyEvent.KEYCODE_1 + it - 1) to (if (it == 0) 0x27 else 0x1E + it - 1) }

    /** L'usage HID de la touche ; −1 : inconnue. */
    fun usage(e: KeyEvent): Int = EVDEV[e.scanCode] ?: KEYCODE[e.keyCode] ?: -1

    /** Le nom d'un usage (lettres et chiffres de la place QWERTY, symboles des touches spéciales). */
    fun label(usage: Int, space: String): String = when (usage) {
        in 0x04..0x1D -> ('A' + (usage - 0x04)).toString()
        in 0x1E..0x26 -> ('1' + (usage - 0x1E)).toString()
        0x27 -> "0"
        0x28 -> "↩"
        0x29 -> "⎋"
        0x2A -> "⌫"
        0x2B -> "⇥"
        0x2C -> space
        0x2D -> "-"
        0x2E -> "="
        0x2F -> "["
        0x30 -> "]"
        0x31 -> "\\"
        0x33 -> ";"
        0x34 -> "'"
        0x35 -> "`"
        0x36 -> ","
        0x37 -> "."
        0x38 -> "/"
        0x39 -> "⇪"
        in 0x3A..0x45 -> "F${usage - 0x3A + 1}"
        0x4A -> "↖"
        0x4B -> "⇞"
        0x4C -> "⌦"
        0x4D -> "↘"
        0x4E -> "⇟"
        0x4F -> "→"
        0x50 -> "←"
        0x51 -> "↓"
        0x52 -> "↑"
        0xE0 -> "Ctrl"
        0xE1 -> "⇧"
        0xE2 -> "Alt"
        0xE3 -> "⊞"
        0xE4 -> "Ctrl ▸"
        0xE5 -> "⇧ ▸"
        0xE6 -> "Alt ▸"
        else -> "0x%02X".format(usage)
    }
}
