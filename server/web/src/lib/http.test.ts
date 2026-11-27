import { afterEach, describe, expect, it, vi } from 'vitest'
import { isQuiet, NetworkError, send } from './http'

describe('send', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('Station injoignable : une erreur traduite, pas celle du navigateur', async () => {
    vi.stubGlobal('fetch', () => Promise.reject(new TypeError('Failed to fetch')))
    const e = await send('GET', '/api/status').catch((e) => e)
    expect(e).toBeInstanceOf(NetworkError)
    expect(e.status).toBe(0)
    expect(e.message).not.toContain('Failed to fetch')
    expect(isQuiet(e)).toBe(true)
  })

  it('une annulation reste une annulation', async () => {
    vi.stubGlobal('fetch', () => Promise.reject(new DOMException('stop', 'AbortError')))
    const e = await send('GET', '/api/status').catch((e) => e)
    expect(e).toBeInstanceOf(DOMException)
  })

  it('écriture : en-tête de la console et JSON', async () => {
    let init: RequestInit | undefined
    vi.stubGlobal('fetch', (_: string, i: RequestInit) => {
      init = i
      return Promise.resolve(new Response('{"ok":true}', { status: 200 }))
    })
    const { data, json } = await send('POST', '/api/x', { a: 1 }, { keepalive: true })
    expect(json).toBe(true)
    expect(data).toEqual({ ok: true })
    const h = init!.headers as Record<string, string>
    expect(h['x-kyber-admin']).toBe('1')
    expect(h['content-type']).toBe('application/json')
    expect(init!.keepalive).toBe(true)
  })
})
