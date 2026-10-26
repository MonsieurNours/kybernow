<!--
  Fond de la connexion sur un écran de la session autre que l'écran principal
  (docs/link-ui.md, « Plusieurs écrans ») : la couleur de l'écran de connexion,
  sans texte, jusqu'à la première image de cet écran ; pour un écran que
  l'ordinateur n'a pas pu créer, une seule ligne. L'app (src/fonds.rs) ouvre
  et ferme cette fenêtre, plein écran, sans jamais lui donner le clavier.
-->
<script lang="ts">
  import { onMount } from 'svelte'
  import type { AppInfo, FondState } from './lib/api'
  import { invoke, listen } from './lib/api'
  import { setLang, t } from './lib/i18n/index.svelte'

  let state = $state<FondState>({ not_created: false })

  onMount(() => {
    const off = [listen<FondState>('fond-state', (s) => (state = s)), listen<string>('lang', setLang)]
    invoke<FondState>('fond_state').then((s) => (state = s))
    // Langue de l'app si la fenêtre a été ouverte depuis un changement de langue.
    invoke<AppInfo>('app_info').then((i) => setLang(i.lang))
    return () => off.forEach((p) => p.then((f) => f()))
  })
</script>

<main>
  {#if state.not_created}
    <p role="status">{t('fond.not_created')}</p>
  {/if}
</main>

<style>
  main {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 32px;
    background: var(--ink-0);
  }

  p {
    margin: 0;
    font-size: 15px;
    line-height: 1.5;
    text-align: center;
    color: var(--muted);
  }
</style>
