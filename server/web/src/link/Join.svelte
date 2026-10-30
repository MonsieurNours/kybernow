<!--
  « Rejoindre un partage » (KyberNow Share, docs/link-ui.md) : le code (celui
  du lien `?join=`, ou les six chiffres tapés), le prénom et la « Qualité de
  l'image » (retenus pour la fois suivante), sans mot de passe d'Admin. La
  Station répond avec le jeton de l'invité ; avant d'entrer, on vérifie que ce
  navigateur sait lire un format de l'image. Par le relais, la demande attend
  d'abord l'accord de l'hôte (« En attente de l'accord de l'hôte… », « Annuler ») ;
  l'écran reste titré « Rejoindre un partage », sans nom d'ordinateur. Un code
  de prêt (docs/pret.md, « Codes ») : la Station le dit (`lend`), et l'ami
  emprunte l'ordinateur (`onLend`, la session d'emprunt fait la suite).
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { backend, JoinRefusal, type GuestJoin, type GuestPending } from './backend'
  import { supportedCodecs } from './engine'
  import { isIsolated } from './device'
  import { setRelay } from './plan'
  import { autoCaps, digits, groupCode, joinRefusalText, leaveShare, loadName, loadQuality, QUALITIES, qualityName, saveName, saveQuality, unreadableFormat, type GuestStart } from './share'
  import type { BorrowStart } from './lend'
  import GateFrame from './GateFrame.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import type { ShareQuality } from '../lib/api'
  import { t } from '../lib/i18n/index.svelte'

  let {
    hostname,
    code = '',
    error: initialError = '',
    onJoined,
    onLend,
    onCancel,
  }: {
    /** Nom de l'ordinateur ; `null` par le relais (il n'est connu qu'après l'accord de l'hôte) : rien n'est montré. */
    hostname: string | null
    /** Code déjà connu (lien `?join=`). */
    code?: string
    /** Refus laissé par une session d'emprunt qui n'a pas pu commencer. */
    error?: string
    onJoined: (start: GuestStart) => void
    /** Le code est celui d'un prêt : l'ami emprunte l'ordinateur. */
    onLend: (start: BorrowStart) => void
    /** Retour à l'écran d'où l'on vient (mot de passe, accueil) ; absent quand on arrive par le lien. */
    onCancel?: () => void
  } = $props()

  let typed = $state(untrack(() => groupCode(code)))
  let name = $state(loadName())
  let quality = $state<ShareQuality>(loadQuality())
  let error = $state(untrack(() => initialError))
  let busy = $state(false)
  /** Trop d'essais : le bouton attend la fin du délai donné par la Station. */
  let waiting = $state(false)
  let waitTimer: ReturnType<typeof setTimeout> | null = null
  let codeInput = $state<HTMLInputElement | null>(null)
  let nameInput = $state<HTMLInputElement | null>(null)
  /** Une demande par le relais attend l'accord de l'hôte. */
  let pending = $state<GuestPending | null>(null)
  /** Arrête l'attente en cours (« Annuler », page quittée). */
  let waitAbort: AbortController | null = null

  const complete = $derived(digits(typed).length === 6 && name.trim().length > 0)

  onMount(() => {
    // Le code connu, c'est le prénom qu'on peut relire ; sinon, le code d'abord.
    ;(digits(typed).length === 6 ? nameInput : codeInput)?.focus()
    // Page fermée pendant l'attente : la demande disparaît chez l'hôte (`leave` en `keepalive`).
    const onHide = () => {
      if (pending) void leaveShare(pending)
    }
    window.addEventListener('pagehide', onHide)
    return () => {
      window.removeEventListener('pagehide', onHide)
      if (waitTimer) clearTimeout(waitTimer)
      cancelWait()
    }
  })

  /** Le relais ne répond pas : trois essais, deux secondes d'écart, avant de le dire. */
  const RELAY_TRIES = 3

  /**
   * Attend l'accord de l'hôte (`POST /api/share/wait`, tenu 25 s au plus, rappelé tant qu'il attend) :
   * la réponse de `join` une fois acceptée, `null` si l'attente est annulée. Refus : `JoinRefusal`.
   */
  async function waitForHost(p: GuestPending): Promise<GuestJoin | null> {
    const abort = new AbortController()
    waitAbort = abort
    pending = p
    let failures = 0
    try {
      while (!abort.signal.aborted) {
        try {
          const join = await backend.shareWait(p, abort.signal)
          if (join) return join
          failures = 0
        } catch (e) {
          if (abort.signal.aborted) return null
          if (!(e instanceof JoinRefusal && e.kind === 'relay') || ++failures >= RELAY_TRIES) {
            // La demande ne sert plus : Admin l'oublie (sans effet si elle est déjà finie).
            if (e instanceof JoinRefusal && e.kind === 'relay') void leaveShare(p)
            throw e
          }
          await new Promise((done) => setTimeout(done, 2000))
        }
      }
      return null
    } finally {
      if (waitAbort === abort) waitAbort = null
      pending = null
    }
  }

  /** « Annuler » : la demande est retirée chez l'hôte, et le formulaire revient. */
  function cancelWait() {
    const p = pending
    if (!waitAbort) return
    waitAbort.abort()
    waitAbort = null
    pending = null
    if (p) void leaveShare(p)
  }

  function onCode(e: Event & { currentTarget: HTMLInputElement }) {
    typed = groupCode(e.currentTarget.value)
    e.currentTarget.value = typed
    error = ''
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault()
    if (!complete || busy || waiting) return
    error = ''
    if (!isIsolated) {
      error = t('web.home.blockedIsolation')
      return
    }
    busy = true
    try {
      // « Auto » : la Station choisit d'après l'écran de cet appareil et ce que son décodeur suit.
      const auto = quality === 'auto' ? await autoCaps() : undefined
      const answer = await backend.joinShare(digits(typed), name.trim(), quality, auto)
      let join: GuestJoin
      if ('pending' in answer) {
        // Par le relais : l'hôte accepte chaque invité. L'image passera en WebSocket.
        setRelay(true)
        const accepted = await waitForHost(answer)
        if (!accepted) return
        join = accepted
      } else {
        join = answer
      }
      if (join.relay) setRelay(true)
      // Le cookie de l'invité est posé : le moteur de Kyber peut se charger.
      let codecs
      try {
        codecs = await supportedCodecs()
      } catch {
        void leaveShare(join)
        error = t('web.home.blockedEngine')
        return
      }
      const unreadable = unreadableFormat(join.codec, join.yuv444, codecs)
      if (unreadable) {
        void leaveShare(join)
        error = t('share.join.unreadable', { format: unreadable })
        return
      }
      saveName(name)
      saveQuality(quality)
      onJoined({ join, codecs, quality })
    } catch (e) {
      if (e instanceof JoinRefusal && e.kind === 'lend') {
        // Un code de prêt : le prénom est retenu, la session d'emprunt prépare l'ordinateur.
        saveName(name)
        onLend({ request: { code: digits(typed) }, name: name.trim(), host: null })
        return
      }
      error = joinRefusalText(e)
      if (e instanceof JoinRefusal && e.kind === 'too_many') {
        waiting = true
        waitTimer = setTimeout(() => (waiting = false), Math.min(e.retryAfter ?? 60, 90) * 1000)
      }
    } finally {
      busy = false
    }
  }
</script>

<GateFrame>
  {#if pending}
    <!-- Par le relais : ni le prénom de l'hôte ni le nom de l'ordinateur ne sont connus avant l'accord. -->
    <div class="head">
      <h1 class="disp">{t('share.join.title')}</h1>
      <p class="waiting" role="status"><span class="dot" aria-hidden="true"></span>{t('share.join.waiting')}</p>
    </div>
    <button type="button" class="ghost" onclick={cancelWait}>{t('share.cancel')}</button>
  {:else}
    <div class="head">
      {#if hostname}<span class="lbl host"><span class="dot"></span>{hostname}</span>{/if}
      <h1 class="disp">{t('share.join.title')}</h1>
      <p>{t('share.join.lead')}</p>
    </div>
    <form onsubmit={submit}>
      <label class="sr-only" for="share-code">{t('share.codeLabel')}</label>
      <input
        id="share-code"
        class="code mono"
        bind:this={codeInput}
        value={typed}
        oninput={onCode}
        placeholder="123 456"
        inputmode="numeric"
        autocomplete="off"
        maxlength="7"
        aria-label={t('share.codeLabel')}
      />
      <label class="sr-only" for="share-name">{t('share.join.name')}</label>
      <input id="share-name" bind:this={nameInput} bind:value={name} placeholder={t('share.join.name')} autocomplete="given-name" maxlength="40" />
      <div class="quality">
        <span class="lbl" aria-hidden="true">{t('share.quality.title')}</span>
        <Segmented label={t('share.quality.title')} options={QUALITIES.map((q) => ({ value: q, label: qualityName(q) }))} value={quality} disabled={busy} onchange={(q: ShareQuality) => (quality = q)} />
      </div>
      {#if error}<p class="error" role="alert">{error}</p>{/if}
      <button type="submit" class="primary" disabled={!complete || busy || waiting}>{busy ? t('share.join.looking') : t('share.join.submit')}</button>
      {#if onCancel}<button type="button" class="ghost" disabled={busy} onclick={onCancel}>{t('share.cancel')}</button>{/if}
    </form>
  {/if}
</GateFrame>

<style>
  .code {
    font-size: 26px;
    letter-spacing: 0.12em;
    text-align: center;
    height: 60px;
  }

  .code::placeholder {
    color: var(--ghost);
  }

  .ghost {
    height: 46px;
  }

  .waiting {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .waiting .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    background: var(--signal);
    box-shadow: 0 0 10px var(--signal);
    animation: respire 1.6s ease-in-out infinite;
  }

  .quality {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 2px;
  }

  /* Les cinq choix sur la largeur des champs : une ligne sur un ordinateur, des lignes pleines sur un
     téléphone. */
  .quality :global(.seg) {
    display: flex;
    width: 100%;
  }

  .quality :global(.seg button) {
    flex: 1 1 auto;
    padding: 0 10px;
  }
</style>
