import { resolve } from 'node:path'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Trois pages, servies par l'app Tauri : l'accueil (index.html), la barre de
// session (bar.html) et le fond de la connexion des autres écrans (fond.html).
export default defineConfig({
  plugins: [svelte()],
  base: './',
  build: {
    target: 'es2022',
    rollupOptions: {
      input: {
        main: resolve(import.meta.dirname, 'index.html'),
        bar: resolve(import.meta.dirname, 'bar.html'),
        fond: resolve(import.meta.dirname, 'fond.html'),
      },
    },
  },
})
