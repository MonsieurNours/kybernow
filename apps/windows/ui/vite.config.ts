import { resolve } from 'node:path'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Quatre pages, servies par l'app Tauri : l'accueil (index.html), la barre de
// session (bar.html), le fond de la connexion des autres écrans (fond.html) et
// la connexion d'une session suivante, dans sa fenêtre (connexion.html,
// plusieurs ordinateurs).
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
        connexion: resolve(import.meta.dirname, 'connexion.html'),
      },
    },
  },
})
