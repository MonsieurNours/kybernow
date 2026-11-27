// Le menu « Touches » de la barre de session (docs/link-ui.md, « Barre de session ») : les dix
// combinaisons d'un PC, chacune avec son indication en petit dessous, et face à une Station Mac
// (« Face à une Station Mac ») ses cinq équivalents, envoyés tels quels (⊞ est ⌘ sur le Mac, sans
// l'échange de « Ctrl agit comme ⌘ »). « ⌥ ⌘ Échap » sert aussi au-dessus du clavier à l'écran.

import { t } from '../lib/i18n/index.svelte'
import { sendKey } from './touch'

/** Combinaison envoyée d'un coup : les modificateurs (`key` : « Meta », « Control », « Alt »,
 * « Shift »), puis la touche. */
export interface Combo {
  mods: string[]
  key: string
  code: string
  readonly label: string
}

/** Une entrée du menu : sa combinaison (ou Ctrl + Alt + Suppr, que le moteur envoie lui-même),
 * son libellé et son indication. */
export type KeyEntry = (Combo | { cad: true; readonly label: string }) & { readonly sub: string }

const entry = (mods: string[], key: string, code: string, label: string, sub: string | null): KeyEntry => ({
  mods,
  key,
  code,
  get label() {
    return t(label)
  },
  get sub() {
    return sub ? t(sub) : ''
  },
})

/** Les dix entrées d'un PC, dans l'ordre de link-ui. */
export const PC_KEYS: KeyEntry[] = [
  {
    cad: true,
    get label() {
      return t('session.keys.cad')
    },
    get sub() {
      return t('session.keys.cadSub')
    },
  },
  entry([], 'Meta', 'MetaLeft', 'session.keys.win', 'session.keys.winSub'),
  entry(['Alt'], 'Tab', 'Tab', 'session.keys.altTab', 'session.keys.altTabSub'),
  entry([], 'PrintScreen', 'PrintScreen', 'session.keys.print', 'session.keys.printSub'),
  entry(['Meta'], 'd', 'KeyD', 'session.keys.desktop', 'session.keys.desktopSub'),
  entry(['Meta'], 'l', 'KeyL', 'session.keys.lock', 'session.keys.lockSub'),
  entry(['Alt'], 'F4', 'F4', 'session.keys.close', 'session.keys.closeSub'),
  entry([], 'Escape', 'Escape', 'web.keys.esc', null),
  entry(['Control', 'Shift'], 'Escape', 'Escape', 'session.keys.taskManager', 'session.keys.taskManagerSub'),
  entry([], 'F11', 'F11', 'session.keys.f11', 'session.keys.f11Sub'),
]

/** « ⌥ ⌘ Échap » (forcer à quitter), au menu « Touches » et à la place de « Ctrl+Alt+Suppr »
 * au-dessus du clavier à l'écran, face à une Station Mac. */
export const MAC_FORCE_QUIT: Combo = {
  mods: ['Alt', 'Meta'],
  key: 'Escape',
  code: 'Escape',
  get label() {
    return t('session.keys.macForceQuit')
  },
}

/** Les cinq entrées face à une Station Mac. */
export const MAC_KEYS: KeyEntry[] = [
  entry(['Meta'], 'Tab', 'Tab', 'session.keys.macSwitch', 'session.keys.macSwitchSub'),
  entry(['Meta'], ' ', 'Space', 'session.keys.macSpotlight', 'session.keys.macSpotlightSub'),
  entry(['Control', 'Meta'], 'q', 'KeyQ', 'session.keys.macLock', 'session.keys.macLockSub'),
  entry(['Shift', 'Meta'], '5', 'Digit5', 'session.keys.macScreenshot', 'session.keys.macScreenshotSub'),
  entry(MAC_FORCE_QUIT.mods, MAC_FORCE_QUIT.key, MAC_FORCE_QUIT.code, 'session.keys.macForceQuit', 'session.keys.macForceQuitSub'),
]

/** Envoie une combinaison à la toile du moteur, telle quelle. */
export function sendCombo(target: HTMLElement | null, c: Combo) {
  sendKey(target, c.key, c.code, new Set(c.mods))
}
