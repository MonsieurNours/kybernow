<!--
  « Rejoindre avec un code » (KyberNow Share, docs/link-ui.md « Partage ») : les six
  chiffres affichés chez la personne qui partage (ou ceux d'un lien kybernow://join,
  déjà remplis), votre prénom (proposé : le nom de ce PC, retenu pour la fois
  suivante) et la « Qualité de l'image » (« Auto » proposé, le dernier choix sinon :
  « Auto » envoie l'écran de ce PC et ce que sa carte graphique décode). Le code part
  aux ordinateurs du réseau, ou à celui du lien ; celui qui partage répond. La session
  s'ouvre ensuite (App.svelte) : rien n'est ajouté à la liste des ordinateurs. Le refus
  se lit tel que le cœur le dit (« Aucun partage avec ce code sur le réseau. », « Ce
  partage est complet… », « Trop d'essais de code… ») ; le format de l'image de l'hôte
  n'en est plus un : un format que ce PC ne lit pas lui vaut une image à part.

  Hors de chez soi (docs/relais.md) : le code part aussi au relais, et l'hôte accepte
  chaque invité venu d'internet. L'écran reste titré « Rejoindre un partage » (le prénom
  de l'hôte et le nom de l'ordinateur ne sont connus qu'après l'accord) et montre « En
  attente de l'accord de l'hôte… » avec « Annuler » ; l'app tient la demande
  (`share_wait`, un appel de 25 s rappelé jusqu'à la réponse). La session s'ouvre si l'hôte
  accepte ; sinon le refus se lit sous le formulaire (« L'hôte a refusé. », « L'hôte n'a
  pas répondu. », « Le relais ne répond pas. », « Trop de demandes en attente… »).
-->
<script lang="ts">
  import { onDestroy, onMount, untrack } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import Segmented from './lib/Segmented.svelte'
  import type { ShareJoinOutcome, ShareJoined, ShareQuality, ShareWaitOutcome } from './lib/api'
  import { invoke, shareQualities } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    code: initialCode = '',
    link = null,
    full = false,
    onclose,
    onjoined,
  }: {
    /** Le code d'un lien, déjà rempli. */
    code?: string
    /** Le lien `kybernow://join?…` : le code ne part qu'à l'ordinateur qu'il désigne. */
    link?: string | null
    /** Quatre sessions sont déjà ouvertes : « Rejoindre » inactif, avec la raison. */
    full?: boolean
    onclose: () => void
    /** Le partage est rejoint : la session s'ouvre. */
    onjoined: (joined: ShareJoined) => void
  } = $props()

  /** « 123 456 » : les six chiffres, groupés par trois. */
  const group = (d: string) => (d.length > 3 ? `${d.slice(0, 3)} ${d.slice(3)}` : d)

  let digits = $state(untrack(() => (initialCode ?? '').replace(/\D/g, '').slice(0, 6)))
  let name = $state('')
  /** Le prénom a été tapé : celui proposé n'arrive plus par-dessus. */
  let touched = false
  /** « Qualité de l'image » : « Auto » proposé, ou le dernier choix (retenu pour la fois suivante). */
  let quality = $state<ShareQuality>('auto')
  /** Elle a été choisie : celle retenue n'arrive plus par-dessus. */
  let picked = false
  /**
   * `searching` : le code part au réseau et au relais ; `waiting` : une demande venue par le relais attend
   * l'accord de l'hôte (« Annuler » la retire).
   */
  let phase = $state<'form' | 'searching' | 'waiting'>('form')
  const working = $derived(phase !== 'form')
  /** Le numéro de la demande en attente (`share_join`), tant qu'elle l'est. */
  let wait: number | null = null
  let error = $state<string | null>(null)
  let codeField = $state<HTMLInputElement | null>(null)
  let nameField = $state<HTMLInputElement | null>(null)

  onMount(() => {
    invoke<{ name: string; quality: ShareQuality }>('share_defaults')
      .then((d) => {
        if (!touched) name = d.name
        if (!picked && shareQualities.includes(d.quality)) quality = d.quality
      })
      .catch(() => {})
    // Le code d'un lien est là : le prénom à confirmer ; sinon, le code d'abord.
    ;(digits.length === 6 ? nameField : codeField)?.focus()
  })

  function typed(e: Event) {
    const el = e.currentTarget as HTMLInputElement
    digits = el.value.replace(/\D/g, '').slice(0, 6)
    el.value = group(digits)
    error = null
  }

  const ready = $derived(digits.length === 6 && !working && !full)

  async function join(e: Event) {
    e.preventDefault()
    if (!ready) return
    phase = 'searching'
    error = null
    try {
      const r = await invoke<ShareJoinOutcome>('share_join', { code: digits, name, quality, link })
      if (r.kind === 'joined') onjoined(r)
      else await follow(r.wait)
    } catch (err) {
      error = String(err)
      phase = 'form'
    }
  }

  /**
   * Attend l'accord de l'hôte : `share_wait` tient l'appel 25 s au plus, on le rappelle tant que l'hôte n'a pas
   * répondu (120 s au plus, ensuite c'est un refus « L'hôte n'a pas répondu. »).
   */
  async function follow(id: number) {
    wait = id
    phase = 'waiting'
    try {
      while (wait === id) {
        const r = await invoke<ShareWaitOutcome>('share_wait', { wait: id })
        // « Annuler » est passé pendant l'appel.
        if (wait !== id) return
        if (r.kind === 'waiting') continue
        wait = null
        if (r.kind === 'joined') onjoined(r)
        else phase = 'form'
        return
      }
    } catch (err) {
      // Refusée, sans réponse de l'hôte, relais muet : le formulaire revient, avec la raison.
      if (wait === id) {
        wait = null
        error = String(err)
        phase = 'form'
      }
    }
  }

  /** « Annuler » : la demande est retirée chez l'hôte, le formulaire revient tel qu'il était. */
  async function cancel() {
    const id = wait
    if (id === null) return
    wait = null
    phase = 'form'
    error = null
    try {
      await invoke('share_cancel', { wait: id })
    } catch {}
  }

  /** La feuille se ferme avec une demande en attente (la fenêtre, l'app) : elle est retirée. */
  onDestroy(() => {
    if (wait !== null) invoke('share_cancel', { wait }).catch(() => {})
  })

  /** Échap et la croix : pendant l'attente, « Annuler » ; pendant la recherche, rien (elle finit en quelques secondes). */
  function dismiss() {
    if (phase === 'waiting') cancel()
    else if (phase === 'form') onclose()
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && dismiss()} />

<div class="veil" role="presentation" onclick={(e) => e.target === e.currentTarget && dismiss()}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="join-title">
    <button type="button" class="close" aria-label={t('common.close')} disabled={phase === 'searching'} onclick={dismiss}><Icon name="x" size={14} /></button>
    <h2 id="join-title">{t('share.join_title')}</h2>
    <p class="hint">{t('share.join_help')}</p>

    <form onsubmit={join}>
    <label class="col">
      <span class="eyebrow">{t('share.code')}</span>
      <input
        class="field code mono"
        inputmode="numeric"
        autocomplete="off"
        spellcheck="false"
        maxlength="7"
        placeholder="123 456"
        value={group(digits)}
        bind:this={codeField}
        oninput={typed}
        disabled={working}
      />
    </label>
    <label class="col">
      <span class="eyebrow">{t('share.your_name')}</span>
      <input
        class="field"
        autocomplete="nickname"
        maxlength="40"
        bind:value={name}
        bind:this={nameField}
        oninput={() => (touched = true)}
        disabled={working}
      />
    </label>
    <div class="col">
      <span class="eyebrow">{t('share.quality')}</span>
      <Segmented
        label={t('share.quality')}
        options={shareQualities.map((q) => ({ value: q, label: t(`share.quality_${q}`) }))}
        value={quality}
        disabled={working}
        onchange={(q) => {
          quality = q
          picked = true
        }}
      />
    </div>

    {#if phase === 'searching'}
      <div class="wait" role="status"><span class="pulse"></span>{t('share.searching')}</div>
    {:else if phase === 'waiting'}
      <div class="wait" role="status"><span class="pulse"></span>{t('share.waiting')}</div>
    {/if}
    {#if full}<p class="hint limit" role="note">{t('common.four_at_most')}</p>{/if}
    {#if error}<div class="error" role="alert">{error}</div>{/if}

    <div class="row">
      {#if phase === 'waiting'}
        <button type="button" class="ghost" onclick={cancel}>{t('common.cancel')}</button>
      {:else}
        <button type="submit" class="primary" disabled={!ready}>{t('share.join')}</button>
      {/if}
    </div>
    </form>
  </div>
</div>

<style>
  .veil {
    position: absolute;
    inset: 0;
    z-index: 30;
    background: rgb(5 7 10 / 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 56px 24px 24px;
    animation: fade 0.25s var(--ease-out);
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  .sheet {
    position: relative;
    /* Assez large pour que les cinq choix de « Qualité de l'image » tiennent sur une ligne (ils passent à la ligne sur un écran étroit). */
    width: min(560px, 100%);
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 36px 40px 40px;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
    animation: rise 0.32s var(--ease-out);
  }

  @keyframes rise {
    from {
      transform: translateY(18px) scale(0.985);
      opacity: 0;
    }
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  h2 {
    margin: 0;
    font-size: 25px;
    font-weight: 640;
    font-stretch: 125%;
  }

  .close {
    position: absolute;
    right: 16px;
    top: 16px;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .close:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.06);
  }

  .hint {
    margin: -8px 0 0;
    font-size: 14px;
    line-height: 1.55;
    color: var(--muted);
  }

  .hint.limit {
    margin: 0;
    color: var(--warn);
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .code {
    height: 64px;
    font-size: 32px;
    letter-spacing: 0.12em;
  }

  .row {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .wait {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 18px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--ink-0);
    font-size: 14px;
  }

  .pulse {
    width: 10px;
    height: 10px;
    border-radius: 5px;
    background: var(--signal);
    box-shadow: 0 0 0 4px rgb(154 140 255 / 0.18), 0 0 14px var(--signal);
    animation: breathe 1.6s ease-in-out infinite;
  }

  @keyframes breathe {
    50% {
      box-shadow: 0 0 0 7px rgb(154 140 255 / 0.08), 0 0 6px var(--signal);
    }
  }
</style>
