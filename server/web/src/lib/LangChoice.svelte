<!-- Langue de la console et du client web : Auto (celle du navigateur), Français, English. -->
<script lang="ts">
  import Segmented from './Segmented.svelte'
  import { i18n, setLangChoice, t, type LangChoice } from './i18n/index.svelte'

  let { compact = false }: { compact?: boolean } = $props()

  const options = $derived([
    { value: 'auto' as LangChoice, label: t('common.langAuto') },
    { value: 'fr' as LangChoice, label: t('common.langFr') },
    { value: 'en' as LangChoice, label: t('common.langEn') },
  ])
</script>

{#if compact}
  <select class="lang" aria-label={t('common.language')} value={i18n.choice} onchange={(e) => setLangChoice(e.currentTarget.value as LangChoice)}>
    {#each options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
  </select>
{:else}
  <Segmented label={t('common.language')} {options} value={i18n.choice} onchange={setLangChoice} />
{/if}

<style>
  .lang {
    height: 28px;
    padding: 0 8px;
    border-radius: 8px;
    border: 0;
    background: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--line);
    color: var(--muted);
    font-size: 12.5px;
  }
</style>
