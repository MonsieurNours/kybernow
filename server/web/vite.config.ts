import { resolve } from 'node:path'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Deux pages : Admin (/) et le client web (/link/).
// En dev (`npm run dev`), l'API et les appels du moteur de Kyber sont servis
// par kyber-admin (https://127.0.0.1:9448, relié à Kyber).
const backend = { target: 'https://127.0.0.1:9448', secure: false }
const kyber = ['/session', '/kymux', '/capabilities', '/enumerate_displays', '/stop_streaming', '/set_bitrate', '/force_idr']

export default defineConfig({
  plugins: [svelte()],
  build: {
    rollupOptions: {
      input: {
        console: resolve(import.meta.dirname, 'index.html'),
        link: resolve(import.meta.dirname, 'link/index.html'),
      },
    },
  },
  server: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
    proxy: {
      '/api': backend,
      '/ws': { ...backend, ws: true },
      // Image et entrées en WebSocket (Safari, iPhone, iPad, Firefox, tactile
      // avec le moteur d'origine).
      '/websocket': { ...backend, ws: true },
      '^/link/(kyclient_wasm.*|audio_worklet|spinlock)\\.(js|wasm)$': backend,
      ...Object.fromEntries(kyber.map((p) => [p, backend])),
    },
  },
})
