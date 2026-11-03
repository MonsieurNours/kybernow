<!-- Fenêtre « Autorisations… » de l'icône : les mêmes trois lignes que l'installation. -->
<script lang="ts">
  import Mark from '../../lib/Mark.svelte'
  import { t } from '../../lib/i18n/index.svelte'
  import { send } from '../bridge'
  import Lights from '../Lights.svelte'
  import PermissionRows from '../PermissionRows.svelte'
  import { allGranted, app } from '../state.svelte'

  const count = $derived(
    app.permissions ? [app.permissions.screen, app.permissions.accessibility, app.permissions.microphone === 'granted'].filter(Boolean).length : 0,
  )

  function close() {
    send({ cmd: 'permissionsDone' })
    send({ cmd: 'close' })
  }
</script>

<div class="solo">
  <div class="top"><Lights closeOnly /></div>
  <main>
    <div class="solo-head">
      <Mark size={30} state="server" />
      <span class="lbl-m">{t('mac.permissions.label', { count })}</span>
    </div>
    <h1 class="t small">{allGranted() ? t('mac.permissions.allTitle') : t('mac.permissions.title')}</h1>
    <p class="lead-m">{t('mac.permissions.lead')}</p>
    <div class="list"><PermissionRows /></div>
    <div class="foot-m end">
      <button type="button" class="btn primary-m" onclick={close}>{t('mac.permissions.close')}</button>
    </div>
  </main>
</div>

<style>
  .top {
    position: relative;
    padding: 16px 20px 0;
  }

  .list {
    margin-top: 20px;
  }
</style>
