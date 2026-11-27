import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'

// Version affichée : celle de l'installateur (Cargo.toml), une seule source.
const cargo = readFileSync(new URL('../Cargo.toml', import.meta.url), 'utf8')
const version = /^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1] ?? '0.0.0'

// Composants et polices partagés avec l'interface de l'app (apps/windows/ui) :
// une seule copie (marque, interrupteur, polices).
const appUi = fileURLToPath(new URL('../../ui/src', import.meta.url))

export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  // Une seule copie de Svelte : celle de l'installateur, même pour les fichiers de l'app.
  resolve: { alias: { $appui: appUi }, dedupe: ['svelte'] },
  server: { fs: { allow: ['.', appUi] } },
})
