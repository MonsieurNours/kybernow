<!-- QR code d'un lien, sur fond clair. -->
<script lang="ts">
  import qrcode from 'qrcode-generator'

  let { text, size = 168, label }: { text: string; size?: number; label: string } = $props()

  const svg = $derived.by(() => {
    const code = qrcode(0, 'M')
    code.addData(text)
    code.make()
    const n = code.getModuleCount()
    let path = ''
    for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) if (code.isDark(y, x)) path += `M${x} ${y}h1v1h-1z`
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-2 -2 ${n + 4} ${n + 4}" shape-rendering="crispEdges"><rect x="-2" y="-2" width="${n + 4}" height="${n + 4}" fill="#F2F8FA" rx="2"/><path d="${path}" fill="#05070A"/></svg>`
  })
</script>

<div role="img" aria-label={label} style="width: {size}px; height: {size}px">{@html svg}</div>

<style>
  div {
    flex: none;
  }

  div :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
