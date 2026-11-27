<!--
  Le panneau du test du profil Auto (docs/link-ui.md, « Profil Auto et débit adaptatif » ; contrat :
  docs/profil-auto.md). Il s'ouvre en choisissant Auto sans test gardé, ou par « Refaire le test » ; le test
  part tout de suite (`autotest_run`, une dizaine de secondes) et montre ses étapes (cinq) au fil des
  `autotest-progress` du cœur : en attente, en cours (roue), cochée avec sa mesure, passée avec sa raison.
  « Annuler » arrête le test et garde le profil d'avant. À la fin, « Réglages proposés » (une ligne par
  choix : titre, valeur, raison), « Appliquer » (Auto devient le profil de cet ordinateur avec ces
  réglages), « Annuler » et « Refaire le test ». Échec (Station injoignable) : son texte et « Réessayer ».
-->
<script lang="ts">
  import Sheet from './lib/Sheet.svelte'
  import { onMount } from 'svelte'
  import Icon from './lib/Icon.svelte'
  import type { AutoChoice, AutoOutcome, AutoProgress, AutoResult, AutoStep, Computer, Settings } from './lib/api'
  import { invoke, listen } from './lib/api'
  import { t } from './lib/i18n/index.svelte'

  let {
    computer,
    onclose,
    onapplied,
  }: {
    computer: Computer
    onclose: () => void
    /** « Appliquer » : les réglages de l'ordinateur tels qu'ils sont enregistrés (profil Auto). */
    onapplied: (settings: Settings) => void
  } = $props()

  type Phase = 'running' | 'summary' | 'error'
  let phase = $state<Phase>('running')
  let steps = $state<AutoStep[]>([])
  let result = $state<AutoResult | null>(null)
  let choices = $state<AutoChoice[]>([])
  let error = $state<string | null>(null)
  let applying = $state(false)
  /** Le test en cours (un « Refaire le test » remplace le précédent). */
  let run = 0

  async function start() {
    const n = ++run
    phase = 'running'
    steps = []
    result = null
    choices = []
    error = null
    let out: AutoOutcome
    try {
      out = await invoke<AutoOutcome>('autotest_run', { id: computer.id })
    } catch (e) {
      out = { result: null, choices: [], error: String(e), cancelled: false }
    }
    if (n !== run) return
    if (out.cancelled) {
      onclose()
    } else if (out.result) {
      result = out.result
      choices = out.choices
      phase = 'summary'
    } else {
      error = out.error
      phase = 'error'
    }
  }

  onMount(() => {
    const off = listen<AutoProgress>('autotest-progress', (e) => {
      if (e.id === computer.id && phase === 'running') steps = e.progress.steps
    })
    start()
    return () => {
      off.then((f) => f())
      // Fermé pendant le test (« Annuler », Échap) : il s'arrête.
      if (phase === 'running') invoke('autotest_cancel', { id: computer.id }).catch(() => {})
    }
  })

  /** « Annuler » : pendant le test, il s'arrête (le panneau se ferme à sa réponse) ; sinon, rien n'est changé. */
  function cancel() {
    if (applying) return
    if (phase === 'running') {
      run++
      invoke('autotest_cancel', { id: computer.id }).catch(() => {})
    }
    onclose()
  }

  async function apply() {
    if (!result || applying) return
    applying = true
    try {
      onapplied(await invoke<Settings>('autotest_apply', { id: computer.id, result: $state.snapshot(result) }))
      onclose()
    } catch (e) {
      error = String(e)
      phase = 'error'
    } finally {
      applying = false
    }
  }
</script>

<Sheet onclose={cancel} outside={phase !== 'running'}>
  <div class="sheet" role="dialog" aria-modal="true" aria-labelledby="autotest-title">
    <button type="button" class="close" aria-label={t('common.close')} disabled={applying} onclick={cancel}><Icon name="x" size={14} /></button>
    <div class="head">
      <span class="wand"><Icon name="wand" size={20} stroke={1.7} /></span>
      <h2 id="autotest-title">{t('autotest.title')}</h2>
    </div>
    <p class="hint">{t('autotest.subtitle')}</p>

    {#if phase === 'running'}
      <ol class="steps" aria-live="polite">
        {#each steps as s (s.step)}
          <li class={s.state}>
            <span class="mark" aria-hidden="true">
              {#if s.state === 'running'}<span class="spin"></span>
              {:else if s.state === 'done'}<Icon name="check" size={14} stroke={2.4} />
              {:else if s.state === 'skipped'}<Icon name="minimize" size={14} stroke={2.2} />
              {:else}<span class="dot"></span>{/if}
            </span>
            <span class="title">{s.title}</span>
            {#if s.text}<span class="text" class:mono={s.state === 'done'}>{s.text}</span>{/if}
          </li>
        {/each}
      </ol>
      <div class="row">
        <button type="button" class="ghost" onclick={cancel}>{t('autotest.cancel')}</button>
      </div>
    {:else if phase === 'summary'}
      <div class="eyebrow">{t('autotest.summary')}</div>
      <ul class="choices">
        {#each choices as c (c.key)}
          <li>
            <span class="title">{c.title}</span>
            <span class="value">{c.value}</span>
            <span class="reason">{c.reason}</span>
          </li>
        {/each}
      </ul>
      <div class="row">
        <button type="button" class="primary" disabled={applying} onclick={apply}>{t('autotest.apply')}</button>
        <button type="button" class="ghost" disabled={applying} onclick={cancel}>{t('autotest.cancel')}</button>
        <button type="button" class="link" disabled={applying} onclick={start}>{t('autotest.again')}</button>
      </div>
    {:else}
      <div class="error" role="alert">{error}</div>
      <div class="row">
        <button type="button" class="primary" onclick={start}>{t('autotest.retry')}</button>
        <button type="button" class="ghost" onclick={cancel}>{t('autotest.cancel')}</button>
      </div>
    {/if}
  </div>
</Sheet>

<style>

  .sheet {
    position: relative;
    width: min(560px, 100%);
    max-height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 36px 40px 40px;
    border: 1px solid var(--line-2);
    border-radius: 22px;
    background: var(--ink-1);
    box-shadow: 0 40px 120px rgb(0 0 0 / 0.6);
    animation: sheet-rise 0.32s var(--ease-out);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-right: 36px;
  }

  .wand {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 38px;
    height: 38px;
    border-radius: 10px;
    background: rgb(154 140 255 / 0.14);
    color: var(--signal);
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

  .steps,
  .choices {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: var(--ink-0);
  }

  .steps li {
    display: grid;
    grid-template-columns: 22px 1fr;
    column-gap: 14px;
    row-gap: 2px;
    align-items: center;
    padding: 13px 18px;
    font-size: 14px;
  }

  .steps li + li,
  .choices li + li {
    border-top: 1px solid var(--line);
  }

  .steps .text {
    grid-column: 2;
    font-size: 13px;
    color: var(--muted);
  }

  .steps .waiting .title {
    color: var(--muted);
  }

  .steps .skipped .text {
    color: var(--muted);
    font-style: italic;
  }

  .mark {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 11px;
  }

  .done .mark {
    background: rgb(76 214 138 / 0.16);
    color: var(--ok);
  }

  .skipped .mark {
    background: rgb(255 255 255 / 0.06);
    color: var(--muted);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 4px;
    border: 1.5px solid var(--line-2);
  }

  .spin {
    width: 16px;
    height: 16px;
    border-radius: 8px;
    border: 2px solid rgb(154 140 255 / 0.25);
    border-top-color: var(--signal);
    animation: turn 0.8s linear infinite;
  }

  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }

  .choices li {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    column-gap: 16px;
    row-gap: 2px;
    padding: 12px 18px;
    font-size: 14px;
  }

  .choices .title {
    color: var(--muted);
  }

  .choices .value {
    font-weight: 600;
    text-align: right;
  }

  .choices .reason {
    grid-column: 1 / -1;
    font-size: 13px;
    color: var(--muted);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
  }

  .link {
    margin-left: auto;
    height: 40px;
    padding: 0 6px;
    border: 0;
    background: none;
    color: var(--signal);
    font-size: 14px;
    font-weight: 600;
  }

  .link:hover:not(:disabled) {
    text-decoration: underline;
  }
</style>
