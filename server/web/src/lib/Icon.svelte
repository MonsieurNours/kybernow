<!-- Icônes en trait (24 × 24), dans l'esprit de Phosphor. -->
<script lang="ts">
  let { name, size = 18, stroke = 1.8 }: { name: string; size?: number; stroke?: number } = $props()

  const paths: Record<string, string> = {
    overview: 'M4.5 3h5A1.5 1.5 0 0 1 11 4.5v5A1.5 1.5 0 0 1 9.5 11h-5A1.5 1.5 0 0 1 3 9.5v-5A1.5 1.5 0 0 1 4.5 3zM14.5 3h5A1.5 1.5 0 0 1 21 4.5v2A1.5 1.5 0 0 1 19.5 8h-5A1.5 1.5 0 0 1 13 6.5v-2A1.5 1.5 0 0 1 14.5 3zM14.5 10h5a1.5 1.5 0 0 1 1.5 1.5v8a1.5 1.5 0 0 1-1.5 1.5h-5a1.5 1.5 0 0 1-1.5-1.5v-8a1.5 1.5 0 0 1 1.5-1.5zM4.5 13h5a1.5 1.5 0 0 1 1.5 1.5v5A1.5 1.5 0 0 1 9.5 21h-5A1.5 1.5 0 0 1 3 19.5v-5A1.5 1.5 0 0 1 4.5 13z',
    image: 'M4 4h11a1.5 1.5 0 0 1 1.5 1.5v7A1.5 1.5 0 0 1 15 14H4a1.5 1.5 0 0 1-1.5-1.5v-7A1.5 1.5 0 0 1 4 4zM6.5 18h6M9.5 14v4M19.5 8.5a4 4 0 0 1 0 5M21.5 6.5a7 7 0 0 1 0 9',
    key: 'M8 11a4 4 0 1 1 0 8 4 4 0 0 1 0-8zM11 12l9-9M17 6l3 3M14.5 8.5l2 2',
    gear: 'M12 9a3 3 0 1 1 0 6 3 3 0 0 1 0-6zM12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1',
    check: 'M5 12l5 5 9-10',
    plus: 'M12 5v14M5 12h14',
    download: 'M12 4v11M7 10l5 5 5-5M5 20h14',
    copy: 'M10 8h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H10a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2zM16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3',
    trash: 'M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3',
    warning: 'M12 3l9.5 17h-19zM12 10v4M12 17.5v.01',
    info: 'M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM12 11v5M12 7.5v.01',
    x: 'M6 6l12 12M18 6L6 18',
    chevron: 'M9 6l6 6-6 6',
    power: 'M12 3v8M7 6.5a7 7 0 1 0 10 0',
    restart: 'M20 12a8 8 0 1 1-2.3-5.6M20 4v4h-4',
    stop: 'M7.5 6h9A1.5 1.5 0 0 1 18 7.5v9a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 6 16.5v-9A1.5 1.5 0 0 1 7.5 6z',
    play: 'M8 5l11 7-11 7z',
    logout: 'M15 4h3a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-3M10 8l-4 4 4 4M6 12h10',
    mac: 'M5.5 5h13A1.5 1.5 0 0 1 20 6.5V16H4V6.5A1.5 1.5 0 0 1 5.5 5zM2 19h20',
    tv: 'M5 4h14a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zM8 20h8',
    phone: 'M9.5 2.5h5A2.5 2.5 0 0 1 17 5v14a2.5 2.5 0 0 1-2.5 2.5h-5A2.5 2.5 0 0 1 7 19V5a2.5 2.5 0 0 1 2.5-2.5zM11 18.5h2',
    tablet: 'M6.5 3h11A1.5 1.5 0 0 1 19 4.5v15a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 19.5v-15A1.5 1.5 0 0 1 6.5 3zM11 18h2',
    browser: 'M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18zM3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18',
    other: 'M5 7h14a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2zM8 12h.01M12 12h.01M16 12h.01',
  }
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width={stroke} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name] ?? paths.other} />
</svg>

<style>
  svg {
    flex: none;
  }
</style>
