// Captures par CDP (le Chrome sans fenêtre impose 500 px de large minimum avec --window-size).
// node cap.mjs '<json jobs>'  ; job = {url, out, w, h, scroll?, js?, wait?, mobile?}
import { spawn } from 'node:child_process';
import { writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
const require_tmp = () => tmpdir() + '/kn-chr-';
const jobs = JSON.parse(process.argv[2]);
const port = 9333 + Math.floor(Math.random() * 500);
const dir = mkdtempSync(require_tmp());
const ch = spawn('/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', ['--headless=new', '--disable-gpu', '--hide-scrollbars', `--remote-debugging-port=${port}`, `--user-data-dir=${dir}`, 'about:blank'], { stdio: 'ignore' });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let tgt;
for (let i = 0; i < 50; i++) { try { const l = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json(); tgt = l.find((t) => t.type === 'page'); if (tgt) break; } catch {} await sleep(150); }
const ws = new WebSocket(tgt.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener('open', r));
let id = 0; const pend = new Map();
ws.addEventListener('message', (e) => { const m = JSON.parse(e.data); if (m.id && pend.has(m.id)) { pend.get(m.id)(m); pend.delete(m.id); } });
const send = (method, params = {}) => new Promise((r) => { const i = ++id; pend.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
await send('Page.enable'); await send('Runtime.enable');
for (const j of jobs) {
  const w = j.w || 390, h = j.h || 844;
  await send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: j.mobile ?? (w < 900) });
  if (j.dark !== undefined) await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: j.reduce ? 'reduce' : 'no-preference' }] });
  await send('Page.navigate', { url: j.url });
  await sleep(j.wait || 2500);
  if (j.js) { const r = await send('Runtime.evaluate', { expression: j.js, awaitPromise: true, returnByValue: true }); if (r.result?.result?.value !== undefined) console.log(j.out, '→', JSON.stringify(r.result.result.value)); await sleep(j.after || 700); }
  if (j.scroll !== undefined) { await send('Runtime.evaluate', { expression: `window.scrollTo(0, ${typeof j.scroll === 'string' ? j.scroll : j.scroll}); 0` }); await sleep(900); }
  const info = await send('Runtime.evaluate', { expression: `JSON.stringify({sw: document.documentElement.scrollWidth, iw: innerWidth, sh: document.documentElement.scrollHeight, y: scrollY})`, returnByValue: true });
  console.log(j.out, info.result.result.value);
  if (j.out) { const s = await send('Page.captureScreenshot', { format: 'png' }); writeFileSync(j.out, Buffer.from(s.result.data, 'base64')); }
}
ws.close(); ch.kill();
