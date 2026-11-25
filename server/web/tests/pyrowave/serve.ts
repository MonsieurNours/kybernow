// Serveur de la page de mesure : deno run -A tests/pyrowave/serve.ts <dossier de prepare.sh> [port]
// Sert bench.html, bench.js (empaqueté par deno bundle) et data/* ; les
// résultats postés par la page sont écrits sur la sortie et dans <dossier>/results.txt.
const dir = Deno.args[0]
const port = Number(Deno.args[1] ?? 8765)
const here = new URL('.', import.meta.url).pathname
const bundle = await new Deno.Command(Deno.execPath(), {
  args: ['bundle', '--platform', 'browser', `${here}bench.ts`, '-o', `${dir}/bench.js`],
}).output()
if (!bundle.success) {
  console.error(new TextDecoder().decode(bundle.stderr))
  Deno.exit(1)
}
const page = `<!doctype html><meta charset="utf-8"><title>PyroWave WebGPU</title><pre id="out"></pre><script type="module" src="bench.js"></script>`
Deno.serve({ port, hostname: '127.0.0.1' }, async (req) => {
  const url = new URL(req.url)
  const headers = { 'Cache-Control': 'no-store' }
  if (req.method === 'POST' && url.pathname === '/result') {
    const text = await req.text()
    console.log(text)
    await Deno.writeTextFile(`${dir}/results.txt`, text + '\n\n', { append: true })
    return new Response('ok', { headers })
  }
  if (url.pathname === '/' || url.pathname === '/bench.html') return new Response(page, { headers: { ...headers, 'Content-Type': 'text/html' } })
  if (url.pathname === '/bench.js') return new Response(await Deno.readFile(`${dir}/bench.js`), { headers: { ...headers, 'Content-Type': 'text/javascript' } })
  if (url.pathname.startsWith('/data/')) {
    const name = url.pathname.slice(6).replace(/[^A-Za-z0-9._-]/g, '')
    try {
      return new Response(await Deno.readFile(`${dir}/${name}`), { headers })
    } catch {
      return new Response('absent', { status: 404 })
    }
  }
  return new Response('absent', { status: 404 })
})
