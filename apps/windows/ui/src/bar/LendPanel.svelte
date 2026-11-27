<!--
  Le panneau « Prêter l'ordinateur… » de l'hôte (docs/pret-du-pc.md) : les demandes, le code créé (son lien, son QR code) ou
  la durée et le compte Steam avant « Créer un code ». Ce que fait chaque bouton reste dans la barre (Bar.svelte, `on`) ;
  styles : bar/panels.css.
-->
<script lang="ts">
  import Segmented from '../lib/Segmented.svelte'
  import type { LendPanel, LendRequest } from '../lib/api'
  import { lendDurations } from '../lib/api'
  import { t } from '../lib/i18n/index.svelte'
  import type { RelayPhase } from '../lib/relay.svelte'
  import CodeTop from './CodeTop.svelte'
  import RelayLine from './RelayLine.svelte'

  let {
    lend,
    name,
    text,
    slotAsked,
    acting,
    relay,
    copied,
    duration = $bindable(),
    steam = $bindable(),
    trustOnce,
    warning,
    requestMeta,
    on,
  }: {
    lend: LendPanel | null
    name: string
    text: string | null
    slotAsked: boolean
    acting: boolean
    relay: RelayPhase
    copied: 'ok' | 'fail' | null
    duration: string
    steam: 'friend' | 'host'
    trustOnce: boolean
    /** L'avertissement, selon le système de la Station (le cœur) : « votre session Windows », « du Mac », ou « votre session ». */
    warning: string
    requestMeta: (r: LendRequest) => string
    on: { answer: (r: LendRequest, accept: boolean) => void; copy: () => void; cancelCode: () => void; createCode: () => void }
  } = $props()
</script>

{#if text}
  <p class="note" role="note">{text}</p>
{:else}
  <h3>{t('lend.panel_title', { name: name })}</h3>
  {#if lend?.notice?.reason === 'slot_start' && lend.notice.text && !slotAsked}<p class="help" role="status">{lend.notice.text}</p>{/if}
  {#if lend && lend.requests.length > 0}
    <div class="eyebrow list-title">{t('share.requests')}</div>
    <ul class="guests requests">
      {#each lend.requests as r (r.id)}
        <li>
          <div class="who">
            <!-- Pendant son créneau, l'ami passe avant : le texte du créneau, et « Accepter » seul (sans « Refuser »). -->
            <span class="gname">{r.kind === 'slot' ? t('lend.slot_notice', { name: r.name }) : t('lend.wants_to_borrow', { name: r.name })}</span>
            {#if requestMeta(r)}<span class="gmeta">{requestMeta(r)}</span>{/if}
          </div>
          <div class="acts">
            <button class="accept" aria-disabled={acting} onclick={() => on.answer(r, true)}>{t('share.accept')}</button>
            {#if r.kind !== 'slot'}<button class="danger" aria-disabled={acting} onclick={() => on.answer(r, false)}>{t('share.decline')}</button>{/if}
          </div>
        </li>
      {/each}
    </ul>
    <p class="help">{t('lend.accept_closes')}</p>
  {/if}
  {#if lend?.code}
    <CodeTop code={lend.code.code} qr={lend.code.qr} address={lend.code.web || lend.code.link} phase={relay} done={copied} copy={on.copy} />
    <p class="help">{t('lend.code_valid')}</p>
    <p class="help">{t('lend.code_help')}</p>
    <RelayLine phase={relay} />
    <p class="help strong">{t('lend.session_closes')}</p>
    <div class="foot">
      <button class="ghost danger" aria-disabled={acting} onclick={on.cancelCode}>{t('lend.cancel_code')}</button>
    </div>
  {:else if !lend?.borrower}
    <div class="col">
      <span class="eyebrow">{t('lend.duration')}</span>
      <Segmented
        label={t('lend.duration')}
        options={lendDurations.map((d) => ({ value: d === null ? 'none' : String(d), label: t(`lend.duration_${d ?? 'none'}`) }))}
        value={duration}
        disabled={acting}
        onchange={(v) => (duration = v)}
      />
    </div>
    <div class="col">
      <span class="eyebrow">{t('lend.steam')}</span>
      <Segmented
        label={t('lend.steam')}
        options={[
          { value: 'friend' as const, label: t('lend.steam_friend') },
          { value: 'host' as const, label: t('lend.steam_host') },
        ]}
        value={steam}
        disabled={acting}
        onchange={(v) => (steam = v)}
      />
    </div>
    {#if trustOnce && warning}<p class="note" role="note">{warning}</p>{/if}
    <div class="foot">
      <button class="primary" disabled={acting} onclick={on.createCode}>{t('lend.create_code')}</button>
    </div>
  {/if}
{/if}

<style>
  /* Le style de base des boutons d'un menu de la barre (Bar.svelte, `.menu button`), pour les boutons de ce composant. */
  :global(.menu) button {
    display: grid;
    grid-template-columns: 18px auto 1fr;
    align-items: center;
    gap: 8px;
    padding: 9px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text);
    text-align: left;
    font-size: 13px;
  }

  :global(.menu) button:hover {
    background: rgb(255 255 255 / 0.07);
  }
</style>
