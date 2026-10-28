<!--
  Rubrique Statistiques : la dernière session de ce navigateur (médiane, règle
  de trame, 95e centile, durée, ce qui a été transmis), cet ordinateur
  (système, carte graphique, adresse, empreinte) et ce que ce navigateur sait
  faire.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from '../lib/Icon.svelte'
  import { ago } from '../lib/format'
  import { i18n, t } from '../lib/i18n/index.svelte'
  import { backend, type Info } from './backend'
  import { hasKeyboardLock, type Mode } from './device'
  import type { CodecSupport } from './engine'
  import FrameRuler from './FrameRuler.svelte'
  import { viaRelay, type Plan } from './plan'
  import { loadLastSession } from './prefs'

  let {
    info,
    mode,
    plan,
    codecs,
    protocol,
    blocked,
  }: { info: Info | null; mode: Mode; plan: Plan | null; codecs: CodecSupport[] | null; protocol: 'kymux' | 'websocket' | null; blocked: boolean } = $props()

  const last = loadLastSession()
  let fingerprint = $state<string | null>(null)

  onMount(() => {
    backend
      .fingerprint()
      .then((f) => (fingerprint = f))
      .catch(() => {})
  })

  /** « 8,9 » en français, « 8.9 » en anglais. */
  const decimal = (v: number) => {
    const text = v >= 100 ? String(Math.round(v)) : v.toFixed(1)
    return i18n.lang === 'fr' ? text.replace('.', ',') : text
  }

  /** « 35 s », « 42 min », « 1 h 05 ». */
  function length(seconds: number): string {
    if (seconds < 60) return t('web.stats.seconds', { n: Math.round(seconds) })
    const m = Math.floor(seconds / 60)
    if (m < 60) return t('web.stats.minutes', { n: m })
    return t('web.stats.hours', { h: Math.floor(m / 60), m: String(m % 60).padStart(2, '0') })
  }

  /** Quatre premiers groupes de quatre de l'empreinte (`sha256:ab:cd:…`). */
  const shortFingerprint = $derived(
    fingerprint
      ? (fingerprint
          .replace(/^sha256:/, '')
          .replace(/:/g, '')
          .toUpperCase()
          .match(/.{1,4}/g)
          ?.slice(0, 4)
          .join(' ') ?? '')
      : '',
  )

  const hardware = $derived(!!codecs?.find((c) => c.codec === (plan?.codec ?? 'h264'))?.hardware)
  const can444 = $derived(!!codecs?.find((c) => c.codec === (plan?.codec ?? 'h264'))?.yuv444)
</script>

<div class="card last">
  <div class="head">
    <span class="lbl">{t('web.stats.last')}</span>
    {#if last}<span class="when">{length(last.duration)} · {ago(last.endedAt)}</span>{/if}
  </div>
  {#if last}
    <div class="big">
      <span class="mono value">{decimal(last.median)}&nbsp;ms</span>
      <span class="mono unit">{t('web.stats.median')}</span>
    </div>
    <FrameRuler median={last.median} p95={last.p95} {decimal} />
    {#if last.p95 !== null}
      <p class="p95">{t('web.stats.p95', { ms: `${decimal(last.p95)} ms` })}</p>
    {/if}
    <p class="sent">{last.width} × {last.height} · {last.refresh} Hz · {last.codec} · {last.mbps} Mbit/s</p>
  {:else}
    <p class="none">{t('web.stats.none')}</p>
  {/if}
  <p class="sub">{t('web.stats.help')}</p>
</div>

<div class="group">
  <span class="lbl">{t('web.stats.computer')}</span>
  <dl>
    {#if info?.os}<div><dt>{t('web.stats.system')}</dt><dd>{info.os}</dd></div>{/if}
    {#if info?.gpu}<div><dt>{t('web.stats.gpu')}</dt><dd>{info.gpu}</dd></div>{/if}
    <div><dt>{t('web.stats.address')}</dt><dd class="mono">{location.host}</dd></div>
    {#if shortFingerprint}<div><dt>{t('web.stats.fingerprint')}</dt><dd class="mono">{shortFingerprint}</dd></div>{/if}
  </dl>
</div>

<div class="group">
  <div class="browser-head">
    <span class="lbl">{t('web.browser.title')}</span>
    <span class="state" class:off={blocked}>{blocked ? t('web.browser.missing') : t('web.browser.ready')}</span>
  </div>
  <ul>
    {#if codecs}
      <li class:off={!hardware}><Icon name={hardware ? 'check' : 'info'} size={14} stroke={2.4} />{hardware ? t('web.browser.hardware') : t('web.browser.software')}</li>
      <li class:off={!can444}><Icon name={can444 ? 'check' : 'info'} size={14} stroke={2.4} />{can444 ? t('web.browser.text444') : t('web.browser.text444Missing')}</li>
    {/if}
    {#if protocol === 'websocket'}
      <li class="off"><Icon name="info" size={14} stroke={2.4} />{viaRelay() ? t('web.browser.relay') : t('web.browser.websocket')}</li>
    {/if}
    <li class:off={!hasKeyboardLock}><Icon name={hasKeyboardLock ? 'check' : 'info'} size={14} stroke={2.4} />{hasKeyboardLock ? t('web.browser.allKeys') : t('web.browser.keysMenu')}</li>
    <li class="neutral"><Icon name="image" size={14} stroke={2} />{t('web.browser.display', { width: mode.width, height: mode.height, refresh: mode.refresh })}</li>
  </ul>
</div>

<style>
  .card {
    border-radius: 14px;
    padding: 16px 18px;
    background: var(--ink-3);
    box-shadow: inset 0 0 0 1px #1e2832;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .head,
  .browser-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .when {
    font-size: 12.5px;
    color: var(--muted);
    text-align: right;
  }

  .big {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .value {
    font-size: 30px;
    font-weight: 500;
    line-height: 1.1;
    color: var(--text);
  }

  .unit {
    font-size: 12px;
    color: var(--muted);
  }

  .p95 {
    font-size: 12px;
    color: var(--faint);
  }

  .sent {
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    color: var(--signal-light);
  }

  .none {
    font-size: 14px;
    color: var(--muted);
  }

  .sub {
    font-size: 12px;
    color: var(--faint);
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 6px;
  }

  dl {
    margin: 0;
    display: flex;
    flex-direction: column;
  }

  dl > div {
    display: grid;
    grid-template-columns: minmax(110px, auto) minmax(0, 1fr);
    gap: 12px;
    padding: 9px 0;
    border-top: 1px solid var(--line);
    font-size: 13.5px;
  }

  dt {
    color: var(--faint);
  }

  dd {
    margin: 0;
    text-align: right;
    overflow-wrap: anywhere;
  }

  dd.mono {
    font-size: 12px;
  }

  .state {
    font-size: 13px;
    font-weight: 600;
    color: var(--ok-text);
  }

  .state.off {
    color: var(--warn-text);
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
  }

  li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  li :global(svg) {
    flex: none;
    margin-top: 3px;
    color: var(--ok);
  }

  li.off {
    color: var(--faint);
  }

  li.neutral :global(svg) {
    color: var(--signal);
  }

  li.off :global(svg) {
    color: var(--faint);
  }
</style>
