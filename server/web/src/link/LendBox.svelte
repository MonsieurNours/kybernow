<!--
  « Prêter l'ordinateur… » dans la barre de session de l'hôte (docs/pret.md,
  docs/link-ui.md) : le même panneau que celui d'Admin (`LendPanel.svelte`),
  sur les routes de l'hôte avec la session d'Admin. Un code créé ici vaut
  accord : la session se fermera quand l'ami arrivera (le panneau le dit).
  L'avertissement (« Votre ami joue dans votre session Windows… ») est montré
  la première fois.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import type { LendOps, LendView } from '../lib/api'
  import LendPanel from '../lib/LendPanel.svelte'
  import Icon from '../lib/Icon.svelte'
  import { t } from '../lib/i18n/index.svelte'

  let {
    view,
    support,
    computer,
    ops,
    onlend,
    onclose,
  }: {
    /** La vue du signe de vie de l'hôte (`null` quand il n'y a rien à dire). */
    view: LendView | null
    /** `ok`, ou `off` (prêt coupé dans Admin). */
    support: 'ok' | 'off' | 'none'
    /** Nom de l'ordinateur : titre du code. */
    computer: string
    ops: LendOps
    onlend: (view: LendView) => void
    onclose: () => void
  } = $props()

  const WARNED_KEY = 'kybernow.lend.warned'

  /** La vue relue à l'ouverture (le signe de vie ne la donne pas quand rien n'est en cours). */
  let local = $state<LendView | null>(null)
  let failed = $state(false)
  let warn = $state(false)
  let message = $state<{ kind: 'ok' | 'error'; text: string } | null>(null)
  let timer: ReturnType<typeof setTimeout> | null = null

  const current = $derived(view ?? local)

  async function reread() {
    try {
      local = await ops.view()
      failed = false
    } catch {
      failed = !current
    }
  }

  onMount(() => {
    try {
      warn = !localStorage.getItem(WARNED_KEY)
      localStorage.setItem(WARNED_KEY, '1')
    } catch {
      warn = true
    }
    if (support === 'ok') reread()
    return () => {
      if (timer) clearTimeout(timer)
    }
  })

  // Plus rien dans le signe de vie (code annulé, expiré) : la vue est relue.
  let had = false
  $effect(() => {
    const now = !!view
    if (had && !now && support === 'ok') reread()
    had = now
  })

  function say(kind: 'ok' | 'error', text: string) {
    message = { kind, text }
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => (message = null), 4000)
  }

  function changed(next: LendView) {
    local = next
    onlend(next)
  }
</script>

<section class="lendbox glass" aria-label={t('lend.open')}>
  <button type="button" class="close" aria-label={t('web.settings.close')} onclick={onclose}><Icon name="x" size={16} stroke={2} /></button>
  <span class="lbl eyebrow">{t('lend.open')}</span>
  {#if support === 'off'}
    <p class="reason">{t('lend.off')}</p>
  {:else if current}
    {#if warn}<p class="warn" role="note"><Icon name="warning" size={16} /><span>{t('lend.warning')}</span></p>{/if}
    <LendPanel view={current} {computer} {ops} own admin={false} onchange={changed} notify={say} refresh={reread} />
  {:else if failed}
    <p class="reason bad" role="alert">{t('web.app.noConsole')}</p>
    <button type="button" class="ghost" onclick={reread}>{t('web.home.retry')}</button>
  {:else}
    <p class="reason" aria-busy="true">{t('web.gate.wait')}</p>
  {/if}
  {#if message}<p class="message" class:bad={message.kind === 'error'} role="status">{message.text}</p>{/if}
</section>

<style>
  .glass {
    background: rgb(9 12 17 / 0.78);
    backdrop-filter: blur(26px) saturate(1.4);
    -webkit-backdrop-filter: blur(26px) saturate(1.4);
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.1), inset 0 1px 0 rgb(255 255 255 / 0.07), 0 18px 50px -12px rgb(0 0 0 / 0.7);
  }

  .lendbox {
    position: absolute;
    left: 50%;
    top: 80px;
    translate: -50% 0;
    width: min(720px, calc(100% - 32px));
    max-height: calc(100% - 100px);
    overflow-y: auto;
    border-radius: 16px;
    padding: 22px 24px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    z-index: 6;
    user-select: text;
    -webkit-user-select: text;
  }

  .close {
    position: absolute;
    right: 12px;
    top: 12px;
    width: 36px;
    height: 36px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--muted);
    display: grid;
    place-items: center;
    z-index: 1;
  }

  .close:hover {
    background: rgb(255 255 255 / 0.07);
    color: #fff;
  }

  .eyebrow {
    min-height: 20px;
    display: flex;
    align-items: center;
  }

  .warn {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 12px 14px;
    border-radius: 12px;
    font-size: 13px;
    line-height: 1.45;
    background: var(--warn-bg);
    box-shadow: inset 0 0 0 1px var(--warn-line);
    color: var(--warn-text);
  }

  .reason {
    font-size: 14px;
    line-height: 1.5;
    color: var(--text-2);
    padding-right: 36px;
  }

  .reason.bad,
  .message.bad {
    color: var(--bad-text);
  }

  .ghost {
    align-self: flex-start;
  }

  .message {
    font-size: 13px;
    color: var(--ok-text);
  }

  @media (max-width: 760px) {
    .lendbox {
      top: 64px;
      padding: 18px 16px 16px;
      max-height: calc(100% - 76px);
    }
  }
</style>
