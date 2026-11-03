<!-- Étape 5 · Autorisations : trois accords de macOS, une seule fois. -->
<script lang="ts">
  import { t } from '../../lib/i18n/index.svelte'
  import { send } from '../bridge'
  import PermissionRows from '../PermissionRows.svelte'
  import { allGranted, app, go } from '../state.svelte'

  const count = $derived(
    app.permissions ? [app.permissions.screen, app.permissions.accessibility, app.permissions.microphone === 'granted'].filter(Boolean).length : 0,
  )

  function finish() {
    // Kyber relancé : il prend les accords tout juste donnés.
    send({ cmd: 'permissionsDone' })
    go('done')
  }
</script>

<div class="permissions">
  <span class="lbl-m kn-enter">{t('mac.permissions.label', { count })}</span>
  <h1 class="t kn-enter kn-d1">{t('mac.permissions.title')}</h1>
  <p class="lead-m kn-enter kn-d2">{t('mac.permissions.lead')}</p>
  <div class="list kn-enter kn-d2"><PermissionRows /></div>
</div>

<div class="foot-m">
  <p class="foot-note">{t('mac.permissions.later')}</p>
  <div class="buttons">
    <button type="button" class="btn" onclick={finish}>{t('mac.permissions.skip')}</button>
    <button type="button" class="btn primary-m" disabled={!allGranted()} onclick={finish}>{t('common.continue')}</button>
  </div>
</div>

<style>
  .permissions {
    display: flex;
    flex-direction: column;
  }

  .list {
    margin-top: 22px;
  }

  .foot-note {
    max-width: 360px;
  }
</style>
