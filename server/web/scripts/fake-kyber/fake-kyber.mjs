// Kyber factice, pour essayer le client web (/link/) sans Kyber : un HTTPS
// auto-signé qui sert un moteur factice (`kyclient_wasm.js`, à côté) et répond
// à ce qu'Admin demande à Kyber pour le partage (KyberNow Share).
//
//   node server/web/scripts/fake-kyber/fake-kyber.mjs [port]      (18443 par défaut)
//
// Il écrit les deux variables d'environnement à donner à Admin
// (`KYBERNOW_KYBER_ADDR`, `KYBERNOW_KYBER_FINGERPRINT`, voir server/README.md,
// « Développer sur le Mac »). Le format de l'image en cours se règle en direct :
//
//   curl -sk https://127.0.0.1:18443/__set -d '{"codec":"hevc","yuv444":true,"display":1234}'
//
// (`codec` : h264, hevc, av1 ou null, image pas encore démarrée ; `display` :
// l'identifiant de l'écran partagé, celui que donne `GET /api/share`.)
// Il demande `openssl` pour le certificat, une fois (gardé dans le dossier
// temporaire).

import { execFileSync } from 'node:child_process'
import { X509Certificate } from 'node:crypto'
import fs from 'node:fs'
import https from 'node:https'
import os from 'node:os'
import path from 'node:path'

const here = path.dirname(new URL(import.meta.url).pathname)
const port = Number(process.argv[2] || 18443)
const work = path.join(os.tmpdir(), 'kybernow-fake-kyber')
const keyFile = path.join(work, 'key.pem')
const certFile = path.join(work, 'cert.pem')

fs.mkdirSync(work, { recursive: true })
if (!fs.existsSync(certFile)) {
  execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', keyFile, '-out', certFile, '-days', '30', '-subj', '/CN=localhost'], { stdio: 'ignore' })
}
const cert = fs.readFileSync(certFile)
const fingerprint = `sha256:${new X509Certificate(cert).fingerprint256}`

let state = { codec: null, yuv444: false, display: 2221634456 }

const server = https.createServer({ key: fs.readFileSync(keyFile), cert }, (req, res) => {
  const url = new URL(req.url, 'https://x')
  let body = ''
  req.on('data', (c) => (body += c))
  req.on('end', () => {
    console.log(req.method, url.pathname, body.slice(0, 200))
    const json = (value, status = 200) => {
      res.writeHead(status, { 'content-type': 'application/json' })
      res.end(JSON.stringify(value))
    }
    switch (url.pathname) {
      case '/webclient/kyclient_wasm.js':
        res.writeHead(200, { 'content-type': 'application/javascript' })
        return res.end(fs.readFileSync(path.join(here, 'kyclient_wasm.js')))
      case '/enumerate_displays':
        return json([{ id: state.display, name: 'DISPLAY1', width: 1920, height: 1080 }])
      case '/kybernow/sessions':
        return json({ streams: state.codec ? [{ display: state.display, codec: state.codec, yuv444: state.yuv444 }] : [] })
      case '/kybernow/role':
        return json({ ok: true })
      case '/kybernow/close':
        return json({ closed: 1 })
      case '/__set':
        state = { ...state, ...JSON.parse(body || '{}') }
        return json(state)
      default:
        res.writeHead(404)
        res.end('not found')
    }
  })
})

server.listen(port, '127.0.0.1', () => {
  console.log(`Kyber factice sur 127.0.0.1:${port}. Pour Admin :`)
  console.log(`  KYBERNOW_KYBER_ADDR=127.0.0.1:${port} KYBERNOW_KYBER_FINGERPRINT=${fingerprint}`)
})
