import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// En dev (`npm run dev`), l'API est servie par kyber-admin lancé avec --http.
export default defineConfig({
  plugins: [svelte()],
  server: {
    proxy: { '/api': 'http://127.0.0.1:9443' },
  },
})
