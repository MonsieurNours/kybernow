<!--
  Écran final, Station rattachée pendant l'installation : les trois accès
  allumés (maquette, « Terminé »), réglables ensuite dans Admin.
-->
<script lang="ts">
  import { t } from './i18n/index.svelte'
  import type { Attached } from './account.svelte'

  /** `columns` : les trois accès côte à côte (écran large). */
  let { attached, columns = false }: { attached: Attached; columns?: boolean } = $props()

  const who = $derived(attached.email ?? attached.owner)
  const rows = $derived([
    {
      on: true,
      title: attached.organization ? t('accountDone.attachedOrg', { name: attached.owner }) : t('accountDone.attached'),
      text: t('accountDone.attachedDetail', { who, relay: attached.relay }),
    },
    { on: attached.accept, title: t('accountDone.accept'), text: attached.accept ? t('accountDone.acceptDetail') : t('accountDone.acceptOff') },
    { on: attached.remoteAccess, title: t('accountDone.remote'), text: attached.remoteAccess ? t('accountDone.remoteDetail') : t('accountDone.remoteOff') },
  ])
</script>

<div class="checks" class:columns>
  <div class="rows">
  {#each rows as row (row.title)}
    <div class="row" class:off={!row.on}>
      {#if row.on}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--ok)" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"></path></svg>
      {:else}
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--faint)" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12"></path></svg>
      {/if}
      <span class="text"><strong>{row.title}</strong><span>{row.text}</span></span>
    </div>
  {/each}
  </div>
  <span class="note">{t('accountDone.settings')}</span>
</div>

<style>
  .checks {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px 20px;
    border-radius: 16px;
    background: var(--ink-2);
    border: 1px solid var(--line);
    max-width: 640px;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .columns .rows {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 16px;
  }

  .columns .text span {
    font-size: 13px;
  }

  .row {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  .row svg {
    flex-shrink: 0;
    margin-top: 2px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .text strong {
    font-size: 15px;
    font-weight: 600;
  }

  .text span {
    font-size: 14px;
    color: var(--muted);
  }

  .row.off strong {
    color: var(--muted);
  }

  .note {
    font-size: 13px;
    color: var(--faint);
  }
</style>
