import { afterEach, describe, expect, it, vi } from 'vitest'
import { forgetTicket, streamUser, ticket } from './ticket'

describe('jeton de streaming de la page (WEB-14)', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  it('le compte donné par Admin, sans jeton', async () => {
    let signed = 0
    vi.stubGlobal('fetch', (path: string) => {
      if (path === '/api/web/ticket') signed++
      if (path === '/api/session') return Promise.resolve(new Response(JSON.stringify({ authenticated: true, stream_user: 'kn' })))
      return Promise.resolve(new Response(JSON.stringify({ token: 't', user: 'autre', expires_in: 3600 })))
    })
    forgetTicket()
    expect(await streamUser('donné')).toBe('donné')
    expect(await streamUser()).toBe('kn')
    expect(signed).toBe(0)
  })

  it('un seul jeton signé, réutilisé tant qu’il lui reste plus d’une heure', async () => {
    vi.useFakeTimers()
    let signed = 0
    vi.stubGlobal('fetch', (path: string) => {
      if (path === '/api/web/ticket') signed++
      // Une Station 0.11.7 : `/api/session` sans le compte.
      if (path === '/api/session') return Promise.resolve(new Response(JSON.stringify({ authenticated: true })))
      return Promise.resolve(new Response(JSON.stringify({ token: `t${signed}`, user: 'kn', expires_in: 12 * 3600, kyber_port: null, hostname: null })))
    })
    forgetTicket()
    expect(await streamUser()).toBe('kn')
    expect((await ticket()).token).toBe('t1')
    expect(await streamUser()).toBe('kn')
    expect(signed).toBe(1)
    vi.advanceTimersByTime(11 * 3600 * 1000 + 1000)
    expect((await ticket()).token).toBe('t2')
    expect(signed).toBe(2)
    forgetTicket()
    await ticket()
    expect(signed).toBe(3)
  })
})
