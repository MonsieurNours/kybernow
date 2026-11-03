<!--
  Feux de la fenêtre. Dans l'installateur, macOS dessine les vrais boutons
  à cet endroit (barre de titre transparente) : on ne fait que réserver la
  place, et la zone sert à déplacer la fenêtre. Dans un navigateur
  (simulation), on dessine des feux factices pour juger la maquette.
-->
<script lang="ts">
  import { isNative, send } from './bridge'

  let { closeOnly = false, disabled = false }: { closeOnly?: boolean; disabled?: boolean } = $props()

  function drag(event: MouseEvent) {
    if (event.button === 0) send({ cmd: 'drag' })
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="lights" onmousedown={drag} aria-hidden="true">
  {#if !isNative}
    <span style:background={disabled ? '#3A434D' : '#FF5F57'}></span>
    <span style:background={closeOnly || disabled ? '#3A434D' : '#FEBC2E'}></span>
    <span style:background={closeOnly || disabled ? '#3A434D' : '#28C840'}></span>
  {/if}
</div>

<style>
  .lights {
    position: relative;
    z-index: 2;
    display: flex;
    gap: 8px;
    height: 12px;
    min-width: 52px;
  }

  span {
    width: 12px;
    height: 12px;
    border-radius: 50%;
  }
</style>
