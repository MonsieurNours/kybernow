<!-- Sous l'aide du même réseau : « Préparation du lien hors de chez vous… », puis « Pas encore joignable hors de chez vous. ». Styles : bar/panels.css. -->
<script lang="ts">
  import { t } from '../lib/i18n/index.svelte'
  import type { RelayPhase } from '../lib/relay.svelte'

  let { phase }: { phase: RelayPhase } = $props()
</script>

{#if phase === 'waiting'}
  <p class="help" role="status"><span class="spin" aria-hidden="true"></span>{t('share.relay_preparing')}</p>
{:else if phase === 'unreachable'}<p class="help">{t('share.relay_down')}</p>{/if}
