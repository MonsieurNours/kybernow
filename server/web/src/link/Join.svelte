<!--
  « Rejoindre un partage » (KyberNow Share, docs/link-ui.md) : le code (celui
  du lien `?join=`, ou les six chiffres tapés), le prénom et la « Qualité de
  l'image » (retenus pour la fois suivante), sans mot de passe d'Admin. La
  Station répond avec le jeton de l'invité ; avant d'entrer, on vérifie que ce
  navigateur sait lire un format de l'image.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { backend, JoinRefusal } from './backend'
  import { supportedCodecs } from './engine'
  import { isIsolated } from './device'
  import { autoCaps, digits, groupCode, joinRefusalText, leaveShare, loadName, loadQuality, QUALITIES, qualityName, saveName, saveQuality, unreadableFormat, type GuestStart } from './share'
  import GateFrame from './GateFrame.svelte'
  import Segmented from '../lib/Segmented.svelte'
  import type { ShareQuality } from '../lib/api'
  import { t } from '../lib/i18n/index.svelte'

  let {
    hostname,
    code = '',
    onJoined,
    onCancel,
  }: {
    hostname: string | null
    /** Code déjà connu (lien `?join=`). */
    code?: string
    onJoined: (start: GuestStart) => void
    /** Retour à l'écran d'où l'on vient (mot de passe, accueil) ; absent quand on arrive par le lien. */
    onCancel?: () => void
  } = $props()

  let typed = $state(untrack(() => groupCode(code)))
  let name = $state(loadName())
  let quality = $state<ShareQuality>(loadQuality())
  let error = $state('')
  let busy = $state(false)
  /** Trop d'essais : le bouton attend la fin du délai donné par la Station. */
  let waiting = $state(false)
  let waitTimer: ReturnType<typeof setTimeout> | null = null
  let codeInput: HTMLInputElement
  let nameInput: HTMLInputElement

  const complete = $derived(digits(typed).length === 6 && name.trim().length > 0)

  onMount(() => {
    // Le code connu, c'est le prénom qu'on peut relire ; sinon, le code d'abord.
    ;(digits(typed).length === 6 ? nameInput : codeInput).focus()
    return () => {
      if (waitTimer) clearTimeout(waitTimer)
    }
  })

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
      const join = await backend.joinShare(digits(typed), name.trim(), quality, auto)
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
  <div class="head">
    <span class="lbl host"><span class="dot"></span>{hostname ?? t('web.app.thisComputer')}</span>
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

  .quality {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 2px;
  }
</style>
