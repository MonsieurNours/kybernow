// Face à une Station Mac (docs/link-ui.md, « Face à une Station Mac ») : la
// touche ⊞ d'un PC y arrive en ⌘. « Ctrl agit comme ⌘ », allumé par défaut,
// échange Ctrl et ⊞ (à gauche comme à droite) sur les touches tapées. Le
// client web n'a pas le cœur des apps : mêmes règles que `keymap.rs`
// (`station_is_mac`, `control_as_command`, `swap_control_command`) et
// `InputPipeline::outgoing_scancode`, sur les `code` du navigateur que lit le
// moteur de Kyber.

/** La Station est un Mac : son système (`os` de `/api/stream/info`,
 * « macOS 26.1 ») commence par « macOS » (casse et espaces de tête ignorés). */
export function stationIsMac(os: string | null | undefined): boolean {
  return /^\s*macos/i.test(os ?? '')
}

/** « Ctrl agit comme ⌘ » pour cet ordinateur : seulement face à un Mac, et
 * allumé tant que l'utilisateur ne l'a pas éteint (`setting` absent : jamais changé). */
export function controlAsCommand(setting: boolean | null | undefined, os: string | null | undefined): boolean {
  return stationIsMac(os) && (setting ?? true)
}

const SWAP: Record<string, string> = {
  ControlLeft: 'MetaLeft',
  MetaLeft: 'ControlLeft',
  ControlRight: 'MetaRight',
  MetaRight: 'ControlRight',
}

/** Ctrl ↔ ⊞, à gauche comme à droite ; les autres touches inchangées. */
export function swapControlCommand(code: string): string {
  return SWAP[code] ?? code
}

/** Le `key` qui va avec un code échangé. */
export function keyOfCode(code: string): string {
  return code.startsWith('Meta') ? 'Meta' : code.startsWith('Control') ? 'Control' : code
}

/**
 * Le code à envoyer pour une touche tapée : Ctrl et ⊞ échangés quand `on`.
 * Un relâchement suit l'appui : la touche se relâche telle qu'elle est partie,
 * même si le réglage a changé entre-temps (`isHeld` : ce que l'ordinateur
 * tient déjà, par code envoyé). Les combinaisons d'un menu (« ⌘ ⇥ ») et les
 * touches à l'écran partent telles quelles, sans passer par ici.
 */
export function outgoingCode(code: string, pressed: boolean, on: boolean, isHeld: (code: string) => boolean): string {
  const swapped = swapControlCommand(code)
  if (swapped === code) return code
  if (!pressed) {
    const self = isHeld(code)
    const other = isHeld(swapped)
    if (self && !other) return code
    if (!self && other) return swapped
  }
  return on ? swapped : code
}
