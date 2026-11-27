<!--
  Session dans le navigateur : la page de l'image. La session elle-même (connexion, replis, fin) est
  dans livesession.svelte.ts, l'invité, le prêt, l'enregistrement, le micro et les mesures dans leurs
  modules ; la barre au pointeur (SessionBar, ses Statistiques) et celle du toucher (TouchBar) dans
  leurs composants. Ici : la toile et la couche tactile, le clavier de l'appareil et sa rangée de
  touches, les avis, le bandeau PyroWave, les panneaux « Partager » et « Prêter l'ordinateur… », les
  raccourcis, et ce qui suit la page (focus, onglet masqué, fermeture). Rien ne s'affiche par-dessus
  l'image quand elle arrive (docs/link-ui.md).
-->
<script lang="ts">
  import { onMount, tick, untrack } from 'svelte'
  import Mark from '../lib/Mark.svelte'
  import { t } from '../lib/i18n/index.svelte'
  import { PointerBar } from './bar.svelte'
  import type { BorrowStart } from './lend'
  import { hasKeyboardLock, hasTouchScreen, isAppleDevice, isTouch } from './device'
  import { diag, diagEnabled, installDiag, watchDiag } from './diag'
  import { isFixedEngine } from './engine'
  import { gamepadsChanged, installGamepadGate, removeGamepadGate } from './gamepads'
  import { installInputGuard, releaseAll, removeInputGuard } from './guard'
  import { allowPackets, engineInputs, forgetInputSocket, injected, watchInputSocket } from './inputs'
  import { MAC_FORCE_QUIT, sendCombo } from './keysmenu'
  import { installAbsoluteMouse, installPointerGate, isRemoteCursorHidden, releasePointer, removeAbsoluteMouse, removePointerGate, setPositionMapper } from './pointer'
  import type { Prefs, TouchMode } from './prefs'
  import { onGpuLost } from './pyrowave/stats'
  import { LiveSession } from './livesession.svelte'
  import type { GuestStart } from './share'
  import { canvas, Stage } from './stage'
  import { MAC_SPECIAL_LABELS, SPECIAL_KEYS, TEXT_ATTRIBUTE, TouchInput, TypedText, sendKey } from './touch'
  import LendAsk from './LendAsk.svelte'
  import LendBox from './LendBox.svelte'
  import SessionBar from './SessionBar.svelte'
  import SessionOverlay from './SessionOverlay.svelte'
  import SessionStats from './SessionStats.svelte'
  import ShareBox from './ShareBox.svelte'
  import TouchBar from './TouchBar.svelte'

  /** `guest` : la session d'un invité du partage (KyberNow Share, docs/kybernow-share.md) : pas de
   * préparation de l'écran ni de jeton d'Admin, une barre réduite, les droits de son rôle.
   * `borrow` : la session d'un ami qui emprunte l'ordinateur (docs/pret.md) : son écran virtuel, le
   * clavier, la souris et les manettes, ni presse-papiers ni réglages de la Station ; `onLendEnd` dit
   * pourquoi elle n'a pas pu commencer, ou si le prêt est fini. */
  let {
    hostname,
    prefs = $bindable(),
    guest = null,
    borrow = null,
    onEnd,
    onLendEnd = () => {},
  }: {
    hostname: string
    prefs: Prefs
    guest?: GuestStart | null
    borrow?: BorrowStart | null
    onEnd: () => void
    onLendEnd?: (error: string | null, over: boolean) => void
  } = $props()

  let sessionEl: HTMLElement
  let textInput: HTMLTextAreaElement
  let fullscreen = $state(!!document.fullscreenElement)
  let touchMode = $state<TouchMode>(untrack(() => prefs.touch))
  let keyboardOpen = $state(false)
  /** Pointeur du pavé tactile, dessiné par la page. */
  let cursor = $state<{ x: number; y: number } | null>(null)
  /** Touches collantes de la rangée au-dessus du clavier de l'appareil. */
  let mods = $state(new Set<string>())
  let diagText = $state('')
  let touch: TouchInput | null = null
  /** Le dernier appui sur le trait du haut est un doigt (la barre attend alors un temps sans toucher). */
  let edgeTouch = false

  // La session, fixée pour toute la durée de la page (rôle compris).
  const s = new LiveSession({
    hostname: () => hostname,
    prefs: () => prefs,
    guest: untrack(() => guest),
    borrow: untrack(() => borrow),
    onEnd: () => onEnd(),
    onLendEnd: (error, over) => onLendEnd(error, over),
    view: {
      freshCanvas: () => stage.freshCanvas(),
      fit: () => stage.fit(),
      textInput: () => textInput ?? null,
      firstImage,
      closeChrome: () => bar.reset(),
    },
  })
  const stage = new Stage(() => s.canvasSize)
  const bar = new PointerBar(s)
  s.metrics.visible = () => bar.visible || bar.menu === 'stats' || bar.touchOpen
  s.metrics.detailed = () => bar.menu === 'stats'

  /** Première image : la toile prend le clavier ; le pointeur du pavé tactile part du centre (sa couche
   * était masquée quand il a été placé). */
  function firstImage() {
    canvas()?.focus()
    tick().then(() => {
      if (!touch || !stage.touchLayer) return
      const r = stage.touchLayer.getBoundingClientRect()
      touch.cursor = { x: r.left + r.width / 2, y: r.top + r.height / 2 }
      if (isTouch && touchMode === 'trackpad') cursor = touch.cursor
    })
  }

  // Un invité sans clavier ni souris n'envoie rien de la page, et ce qu'il tenait est relâché (touches,
  // boutons, pointeur capturé, clavier de l'appareil, menus de la souris et des touches).
  $effect(() => {
    allowPackets(s.canKbm)
    if (s.canKbm) return
    untrack(() => {
      releaseAll()
      releasePointer()
      if (bar.menu === 'keys' || bar.menu === 'mouse') bar.close()
      if (keyboardOpen) closeKeyboard()
    })
  })

  // Le clavier du navigateur et le champ de texte du moteur suivent les mêmes droits : en plein écran
  // (Chrome, Edge), toutes les touches vont à l'ordinateur seulement avec le clavier et la souris ; sans,
  // elles reviennent au navigateur (Échap, raccourcis), et le champ que le moteur lit n'écoute plus.
  $effect(() => {
    const kbm = s.canKbm
    untrack(() => {
      textInput?.toggleAttribute(TEXT_ATTRIBUTE, engineInputs() && kbm)
      if (!s.guest || !hasKeyboardLock || !document.fullscreenElement) return
      const keyboard = (navigator as any).keyboard
      if (kbm) keyboard.lock().catch(() => {})
      else keyboard.unlock?.()
    })
  })

  // Les manettes d'un invité suivent son rôle (voir `gamepads.ts`) : le moteur ne les voit que s'il joue.
  let padsAllowed = untrack(() => s.canPlay)
  $effect(() => {
    const play = s.canPlay
    untrack(() => {
      if (play === padsAllowed) return
      padsAllowed = play
      gamepadsChanged(play)
    })
  })

  async function toggleFullscreen() {
    if (document.fullscreenElement) await document.exitFullscreen().catch(() => {})
    else await document.documentElement.requestFullscreen({ navigationUI: 'hide' }).catch(() => {})
  }

  async function onFullscreenChange() {
    fullscreen = !!document.fullscreenElement
    stage.forgetRect()
    // Plein écran : toutes les touches vont à l'ordinateur (Chrome, Edge).
    if (fullscreen && hasKeyboardLock && s.canKbm) await (navigator as any).keyboard.lock().catch(() => {})
    setTimeout(stage.fit, 50)
  }

  function pressSpecial(k: (typeof SPECIAL_KEYS)[number]) {
    if (k.sticky) {
      const next = new Set(mods)
      if (next.has(k.key)) next.delete(k.key)
      else next.add(k.key)
      mods = next
      return
    }
    sendKey(canvas(), k.key, k.code, mods)
    mods = new Set()
  }

  // Clavier de l'appareil : texte en Unicode (par le moteur s'il est corrigé), touches par leur code.
  const typed = new TypedText(
    () => textInput ?? null,
    canvas,
    () => mods,
    () => (mods = new Set()),
  )

  async function openKeyboard() {
    keyboardOpen = true
    bar.touchOpen = false
    bar.guide = false
    await tick()
    textInput.focus()
    stage.trackViewport(keyboardOpen)
  }

  function closeKeyboard() {
    keyboardOpen = false
    textInput?.blur()
    tick().then(() => stage.trackViewport(keyboardOpen))
  }

  /** Clavier fermé par l'appareil (bouton du clavier, autre app) : la rangée de touches part avec lui.
   * Les touches de la rangée gardent le focus. */
  function onTextBlur() {
    if (keyboardOpen) closeKeyboard()
  }

  function setTouchMode(m: TouchMode) {
    touchMode = m
    prefs.touch = m
    if (touch) touch.mode = m
    cursor = m === 'trackpad' ? (touch?.cursor ?? null) : null
  }

  /** Ctrl + Alt + Maj : la barre, et la souris rendue ; avec une lettre, les raccourcis de la session
   * (docs/link-ui.md : D se déconnecter, F plein écran, M souris, R enregistrer, S statistiques, V micro). */
  function shortcut(e: KeyboardEvent) {
    if (!(e.ctrlKey && e.altKey && e.shiftKey)) return
    if (['Control', 'Alt', 'Shift'].includes(e.key)) {
      releasePointer()
      bar.toggleVisible()
      return
    }
    const actions: Record<string, (() => void) | null> = {
      KeyD: () => void s.quit(),
      KeyF: () => void toggleFullscreen(),
      KeyM: s.canKbm ? () => s.controls.toggleMouse() : null,
      KeyR: s.hosting && s.recording.state ? () => void s.recording.toggle(false) : null,
      KeyS: isTouch ? null : () => bar.toggle('stats'),
      KeyV: s.mic.shown ? () => s.mic.toggle(false) : null,
    }
    const run = s.phase === 'live' ? actions[e.code] : null
    if (!run) return
    e.preventDefault()
    e.stopPropagation()
    if (!e.repeat) run()
  }

  onMount(() => {
    installDiag()
    // Avant le moteur : seuls les clics sur l'image partent à l'ordinateur, et tout ce qui y est tenu
    // enfoncé est relâché quand la page perd la main.
    installInputGuard({
      isVideo: (target) => target === canvas() || (!!stage.touchLayer && target === stage.touchLayer),
      canvas,
      onRelease: () => {
        if (mods.size) mods = new Set()
      },
      allowed: () => ({ keyboard: s.canKbm, mouse: s.canKbm }),
      // Un clavier d'Apple a sa touche ⌘ : elle part comme ⌘, sans échange.
      swapControl: () => s.stationMac && !isAppleDevice && prefs.controlAsCommand,
    })
    installPointerGate(() => s.controls.pointerAllowed())
    installAbsoluteMouse()
    watchInputSocket()
    if (s.guest) installGamepadGate(() => s.canPlay)
    setPositionMapper(stage.position)
    bar.host = sessionEl
    void s.start()
    // Un invité signale sa présence dès l'arrivée : sans nouvelles pendant 20 s, il sort du partage. Celui
    // d'un ami part avec la réponse de `borrow` (son secret et son rythme).
    if (s.g) s.heartbeat.start({ guest: s.g.join.guest })
    else if (!s.b) s.learnStation()

    const timers = [
      // Le temps qui reste au prêt d'un ami (« Encore 1 h 20 »).
      setInterval(() => s.borrowing?.tick(), 10_000),
      // Le temps de l'enregistrement, chaque seconde (barre fermée comprise).
      setInterval(() => s.recording.tick(), 1000),
      // Souris « Auto » de l'hôte.
      setInterval(() => void s.controls.pollForeground(), 1000),
      setInterval(() => s.metrics.halfSecond(), 500),
      setInterval(() => s.metrics.second(), 1000),
      // La latence du signal de vie (le Worker envoie la dernière connue).
      setInterval(() => s.updateBeat(), 5000),
      setInterval(() => s.phase === 'live' && !s.g && !s.b && void s.refreshTicket(), 3_600_000),
    ]
    const stopDiag = watchDiag(
      (text) => (diagText = text),
      () => {
        const c = canvas()
        const r = c?.getBoundingClientRect()
        return [
          `positions page ${injected.count} · sorties ${diag.outside} · dernière ${diag.lastX},${diag.lastY} · toile ${r ? `${Math.round(r.left)},${Math.round(r.top)} ${Math.round(r.width)}×${Math.round(r.height)}` : '—'}`,
          `capture ${document.pointerLockElement ? 'oui' : 'non'} · pointeur de l'ordinateur ${isRemoteCursorHidden() ? 'masqué' : 'visible'} · transport ${s.plan?.protocol ?? '—'} · moteur ${isFixedEngine() ? 'corrigé' : "d'origine"}${engineInputs() ? ', entrées par le moteur' : ''}`,
        ]
      },
    )

    const onBlur = () => {
      bar.pageFocused = false
      bar.cancelDwell()
    }
    const onFocus = () => (bar.pageFocused = true)
    const onLockChange = () => {
      bar.pointerLocked = !!document.pointerLockElement
      if (bar.pointerLocked) bar.cancelDwell()
    }
    bar.pageFocused = document.hasFocus()
    const resize = () => {
      stage.fit()
      stage.forgetRect()
      void bar.relayout()
    }
    const viewport = () => {
      stage.forgetRect()
      if (keyboardOpen) stage.trackViewport(keyboardOpen)
    }
    // Ctrl+W, ⌘W, ⌘Q en pleine session : le navigateur demande confirmation. Un invité sort sans façon (il
    // rejoint de nouveau avec le code), un ami aussi (fermer la fenêtre ne rend pas l'ordinateur).
    const beforeUnload = (e: BeforeUnloadEvent) => {
      if (s.g || s.b || s.phase !== 'live' || s.ended) return
      e.preventDefault()
      e.returnValue = ''
    }
    // Tout clic ou toute touche est un geste : le son peut y reprendre.
    const gesture = () => void s.controls.soundGesture()
    const leave = () => s.leavePage()
    // Onglet qui passe en arrière-plan : un signal tout de suite, et le débit plafonné jusqu'au retour.
    const visibility = () => {
      if (document.visibilityState === 'hidden' && s.heartbeat.running) s.heartbeat.now()
      s.controls.applyBitrate()
    }
    const listen: [EventTarget | undefined | null, string, EventListener, boolean?][] = [
      [window, 'keydown', shortcut as EventListener, true],
      [window, 'blur', onBlur],
      [window, 'focus', onFocus],
      [document, 'pointerlockchange', onLockChange],
      [window, 'resize', resize],
      [window, 'scroll', stage.forgetRect, true],
      [window.visualViewport, 'resize', viewport],
      [window.visualViewport, 'scroll', viewport],
      [window, 'beforeunload', beforeUnload as EventListener],
      [document, 'fullscreenchange', onFullscreenChange],
      [window, 'pointerdown', gesture],
      [window, 'keydown', gesture],
      [window, 'pagehide', leave],
      [document, 'visibilitychange', visibility],
    ]
    for (const [target, type, fn, capture] of listen) target?.addEventListener(type, fn, capture)
    if (document.fullscreenElement) void onFullscreenChange()

    // Safari : pas de correction automatique (elle remplacerait un mot déjà parti).
    textInput.setAttribute('autocorrect', 'off')
    // Écran tactile, même avec une souris ou un pavé à côté (iPad avec clavier) : les doigts passent par la
    // couche tactile. Avec un pointeur précis à côté, le doigt touche directement (le pointeur reste la souris).
    if (hasTouchScreen && stage.touchLayer) {
      touch = new TouchInput(stage.touchLayer, canvas, isTouch ? touchMode : 'direct', (p) => (cursor = isTouch && touchMode === 'trackpad' ? p : null))
      if (isTouch && touchMode === 'trackpad') cursor = touch.cursor
    }
    const offGpuLost = onGpuLost(() => void s.onGpuLost())

    return () => {
      offGpuLost()
      for (const timer of timers) clearInterval(timer)
      stopDiag()
      for (const [target, type, fn, capture] of listen) target?.removeEventListener(type, fn, capture)
      s.destroy()
      bar.destroy()
      stage.destroy()
      releasePointer()
      removePointerGate()
      removeAbsoluteMouse()
      removeInputGuard()
      removeGamepadGate()
      allowPackets(true)
      setPositionMapper(() => null)
      forgetInputSocket()
      touch?.destroy()
    }
  })
</script>

<div class="session" class:touch={isTouch} bind:this={sessionEl} onpointermove={(e) => bar.onPointerMove(e)} onpointerleave={(e) => bar.onPointerLeave(e)} role="application" aria-label={t('session.aria', { host: s.hostname })}>
  <div class="stage" class:kb={keyboardOpen} bind:this={stage.el} oncontextmenu={(e) => e.preventDefault()} role="presentation">
    <div class="canvas-box" bind:this={stage.box}></div>
    <div class="touch-layer" class:active={hasTouchScreen && s.phase === 'live' && s.canKbm} bind:this={stage.touchLayer}></div>
    {#if cursor && s.phase === 'live'}
      <div class="tcursor" style="left: {cursor.x}px; top: {cursor.y}px" aria-hidden="true"></div>
    {/if}
  </div>

  {#if s.phase !== 'live' || s.guest?.switching}
    <SessionOverlay {s} />
  {/if}

  {#if s.phase === 'live' && (s.reconnecting || s.notice)}
    <div class="toast frost" role="status">{s.reconnecting || s.notice}</div>
  {/if}

  {#if s.phase === 'live' && s.pyroBanner}
    <!-- PyroWave sans câble connu ou par le relais : au début de la session, 8 s (docs/link-ui.md, « PyroWave »). -->
    <div class="pwbanner frost" class:below={!!(s.reconnecting || s.notice)} role="status">
      <div class="pwtext">
        <strong>{s.pyroBanner === 'relay' ? t('session.pyrowave.titleRelay') : t('session.pyrowave.titleNetwork')}</strong>
        <span>{s.pyroBanner === 'relay' ? t('session.pyrowave.textRelay') : t('session.pyrowave.textNetwork')}</span>
      </div>
      <button type="button" class="pwswitch" onclick={() => s.leavePyroWave()}>{t('session.pyrowave.switch', { format: s.autoFormat() })}</button>
      <button type="button" class="pwclose" aria-label={t('session.pyrowave.close')} onclick={() => s.hidePyroBanner()}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
      </button>
    </div>
  {/if}

  {#if s.phase === 'live' && s.controls.soundBlocked && prefs.audio && !s.controls.muted}
    <button type="button" class="sound frost" onpointerdown={(e) => e.preventDefault()} onclick={() => s.controls.unlockSound()}>
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 9h4l5-4v14l-5-4H4z" /><path d="M17 9a4 4 0 0 1 0 6" /><path d="M19.5 6.5a8 8 0 0 1 0 11" /></svg>
      {isTouch ? t('session.sound.tap') : t('session.sound.click')}
    </button>
  {/if}

  {#if s.phase === 'live' && s.recording.state?.active && (isTouch ? !bar.touchOpen : !bar.visible)}
    <!-- Barre fermée : la pastille reste là, toujours visible (tout le monde dans la session le voit). -->
    <div class="recfloat" role="status" aria-label={s.g ? s.recording.hint : s.recording.pill}>
      <span class="beat" aria-hidden="true"></span>
      <span class="mono">{s.g ? s.recording.pill : s.recording.time}</span>
    </div>
  {/if}

  {#if !isTouch && s.phase === 'live'}
    {#if bar.edgeShown}
      <!-- Le trait du bord haut, touchable aussi (docs/link-ui.md, « Barre de session au pointeur »). -->
      <button type="button" class="edge" aria-label={t('session.bar.show')} title={t('session.bar.showHint')} onpointerdown={(e) => (edgeTouch = e.pointerType === 'touch')} onclick={() => bar.show(edgeTouch)}><span></span></button>
    {/if}
    <SessionBar {s} {bar} {fullscreen} ontogglefullscreen={toggleFullscreen} />
    {#if bar.menu === 'stats'}
      <SessionStats {s} onclose={() => bar.close()} />
    {/if}
  {/if}

  <!-- Tactile : languette, barre compacte, clavier -->
  {#if isTouch && s.phase === 'live'}
    <TouchBar {s} {bar} {touchMode} onmode={setTouchMode} onkeyboard={openKeyboard} />
    {#if !keyboardOpen}
      {#if s.canKbm}
        <button type="button" class="fab frost" onpointerdown={(e) => e.preventDefault()} onclick={openKeyboard} aria-label={t('session.touch.openKeyboard')}>
          <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M8 14h8" /></svg>
        </button>
      {/if}
    {:else}
      <div class="keys" role="toolbar" aria-label={t('session.touch.specialKeys')}>
        {#each SPECIAL_KEYS as k (k.code)}
          <!-- Face à une Station Mac : « ⌃ », « ⌥ », « ⌘ » au lieu de « Ctrl », « Alt », « ⊞ » (mêmes touches envoyées). -->
          {@const mac = s.stationMac ? MAC_SPECIAL_LABELS[k.code] : undefined}
          <button type="button" class="key" aria-pressed={k.sticky ? mods.has(k.key) : undefined} aria-label={mac?.aria ?? k.aria} onpointerdown={(e) => e.preventDefault()} onclick={() => pressSpecial(k)}>{mac?.label ?? k.label}</button>
        {/each}
        {#if s.stationMac}
          <button type="button" class="key" onpointerdown={(e) => e.preventDefault()} onclick={() => sendCombo(canvas(), MAC_FORCE_QUIT)}>{MAC_FORCE_QUIT.label}</button>
        {:else}
          <button type="button" class="key" onpointerdown={(e) => e.preventDefault()} onclick={() => s.engine?.ctrlAltDel()}>{t('session.touch.cad')}</button>
        {/if}
        <button type="button" class="key close" aria-label={t('session.touch.closeKeyboard')} onclick={closeKeyboard}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 9l6 6 6-6" /></svg>
        </button>
      </div>
    {/if}
  {/if}

  {#if s.hosting && s.phase === 'live'}
    {@const host = s.hosting}
    {#if bar.menu === 'share'}
      <ShareBox share={host.share} support={host.shareSupport} computer={s.hostname} onshare={(view) => (host.share = view)} onclose={() => bar.close()} />
    {/if}
    {#if bar.menu === 'lend'}
      <LendBox view={host.lend} support={host.lendSupport} computer={s.hostname} ops={host.ops} onlend={(view) => (host.lend = view)} onclose={() => bar.close()} />
    {:else if host.lend && !host.accepted}
      <!-- Une demande (ou le créneau d'un ami) : visible même barre fermée. -->
      <LendAsk view={host.lend} ops={host.ops} onlend={(view) => (host.lend = view)} onaccepted={(name) => host.acceptedBy(name)} />
    {/if}
  {/if}

  {#if diagEnabled && diagText}
    <pre class="diag mono" aria-hidden="true">{diagText}</pre>
  {/if}

  <textarea
    class="ime"
    bind:this={textInput}
    disabled={!s.canKbm}
    onkeydown={typed.keydown}
    onbeforeinput={typed.beforeinput}
    oncompositionstart={typed.compositionstart}
    oncompositionend={typed.compositionend}
    oninput={typed.input}
    onblur={onTextBlur}
    autocapitalize="off"
    autocomplete="off"
    spellcheck="false"
    aria-label={t('session.textInput')}
  ></textarea>
</div>

<style>
  .session {
    /* Hauteur de la rangée de touches au-dessus du clavier de l'appareil. */
    --keys-h: 52px;
    position: fixed;
    inset: 0;
    background: #000;
    overflow: hidden;
    color: var(--text);
    user-select: none;
    -webkit-user-select: none;
  }

  .stage {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
  }

  /* Clavier ouvert : l'image au-dessus de la rangée de touches, elle-même juste au-dessus du clavier
     (variables posées par `Stage.trackViewport`). */
  .stage.kb {
    top: var(--vv-top, 0px);
    bottom: auto;
    height: calc(var(--vv-h, 100%) - var(--keys-h));
  }

  .canvas-box {
    display: grid;
    place-items: center;
    width: 100%;
    height: 100%;
  }

  .canvas-box :global(canvas) {
    display: block;
    outline: none;
    touch-action: none;
  }

  .touch-layer {
    position: absolute;
    inset: 0;
    display: none;
    touch-action: none;
  }

  .touch-layer.active {
    display: block;
  }

  .tcursor {
    position: fixed;
    width: 0;
    height: 0;
    pointer-events: none;
  }

  .tcursor::before {
    content: '';
    position: absolute;
    left: -7px;
    top: -7px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.6);
  }

  .frost {
    background: rgb(9 12 17 / 0.7);
    backdrop-filter: blur(26px) saturate(1.4);
    -webkit-backdrop-filter: blur(26px) saturate(1.4);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1), inset 0 1px 0 rgb(255 255 255 / 0.07), 0 18px 50px -12px rgb(0 0 0 / 0.7);
  }

  /* Le trait du haut : la barre au clic, sans viser le bord de l'écran. */
  .edge {
    position: absolute;
    left: 50%;
    top: 0;
    width: 220px;
    height: 18px;
    translate: -50% 0;
    padding: 0;
    border: 0;
    background: none;
    display: flex;
    justify-content: center;
    z-index: 4;
  }

  .edge span {
    width: 100%;
    height: 4px;
    border-radius: 0 0 4px 4px;
    background: rgb(255 255 255 / 0.3);
    transition:
      height 0.2s,
      background-color 0.2s;
  }

  .edge:hover span {
    height: 8px;
    background: rgb(154 140 255 / 0.7);
  }

  .toast {
    position: absolute;
    left: 50%;
    translate: -50% 0;
    max-width: calc(100% - 32px);
    padding: 10px 16px;
    border-radius: 12px;
    font-size: 13px;
    color: var(--text-2);
    display: flex;
    align-items: center;
    gap: 10px;
    pointer-events: none;
    top: 80px;
  }

  /* PyroWave : bandeau du début, sous l'avis s'il y en a un. */
  .pwbanner {
    position: absolute;
    left: 50%;
    translate: -50% 0;
    top: 80px;
    max-width: calc(100% - 32px);
    padding: 10px 10px 10px 16px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    gap: 12px;
    box-shadow: inset 0 0 0 1px var(--warn-line);
    z-index: 3;
  }

  .pwbanner.below {
    top: 132px;
  }

  .pwtext {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 13px;
    color: var(--text-2);
  }

  .pwtext strong {
    color: var(--warn-text);
    font-weight: 600;
  }

  .pwswitch {
    height: 32px;
    padding: 0 12px;
    border-radius: 8px;
    font-size: 13px;
    white-space: nowrap;
  }

  .pwclose {
    width: 32px;
    height: 32px;
    padding: 0;
    display: grid;
    place-items: center;
    background: none;
    color: var(--muted);
  }

  /* Son en attente d'un geste : au-dessus de tout, en bas au centre. */
  .sound {
    position: absolute;
    left: 50%;
    bottom: max(24px, env(safe-area-inset-bottom));
    translate: -50% 0;
    height: 46px;
    padding: 0 18px;
    border: 0;
    border-radius: 23px;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
    z-index: 7;
  }

  /* L'enregistrement, barre fermée : la pastille rouge et son rond qui bat une fois par seconde. */
  .recfloat {
    position: absolute;
    top: max(12px, env(safe-area-inset-top));
    left: max(12px, env(safe-area-inset-left));
    z-index: 6;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    padding: 0 11px;
    border-radius: 14px;
    background: var(--bad-bg);
    box-shadow: inset 0 0 0 1px var(--bad-line);
    color: var(--bad-text);
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    pointer-events: none;
  }

  .beat {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--bad);
    flex: none;
    animation: beat 1s ease-in-out infinite;
  }

  @keyframes beat {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  .fab {
    position: absolute;
    right: max(20px, env(safe-area-inset-right));
    bottom: max(20px, env(safe-area-inset-bottom));
    width: 54px;
    height: 54px;
    border: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--text);
    z-index: 5;
  }

  .keys {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(var(--vv-top, 0px) + var(--vv-h, 100%) - var(--keys-h));
    height: var(--keys-h);
    box-sizing: border-box;
    padding: 6px max(12px, env(safe-area-inset-left));
    background: var(--ink-2);
    border-top: 1px solid #1e2832;
    display: flex;
    align-items: center;
    gap: 6px;
    overflow-x: auto;
    z-index: 6;
  }

  .key {
    flex: none;
    height: 38px;
    min-width: 44px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px #1e2832;
    color: var(--text);
    font-size: 13px;
  }

  .key[aria-pressed='true'] {
    background: rgb(154 140 255 / 0.18);
    box-shadow: inset 0 0 0 1px rgb(154 140 255 / 0.6);
  }

  .key.close {
    margin-left: auto;
    display: grid;
    place-items: center;
  }

  .diag {
    position: absolute;
    left: 12px;
    bottom: 12px;
    margin: 0;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgb(0 0 0 / 0.75);
    color: #c9f2da;
    font-size: 11px;
    line-height: 1.6;
    pointer-events: none;
    z-index: 9;
  }

  /* Champ caché qui fait apparaître le clavier de l'appareil. */
  .ime {
    position: fixed;
    left: 0;
    bottom: 0;
    width: 1px;
    height: 1px;
    opacity: 0;
    padding: 0;
    border: 0;
    resize: none;
    font-size: 16px;
  }

  @media (prefers-reduced-motion: reduce) {
    .beat {
      animation: none;
    }
  }
</style>
