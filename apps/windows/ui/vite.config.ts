import { resolve } from 'node:path'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Deux pages, servies par l'app Tauri : l'accueil (index.html) et la barre
// de session (bar.html).
export default defineConfig({
  plugins: [svelte()],
  base: './',
  build: {
    target: 'es2022',
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, 'index.html'),
        bar: resolve(import.meta.dirname, 'bar.html'),
      },
    },
  },
})
