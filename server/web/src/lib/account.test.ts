// Ce qu'Admin écrit d'un accès entré par un compte KyberNow. `npm test`.

import { beforeAll, describe, expect, it } from 'vitest'

import type { AccountAccess } from './api'
import { detail, rightsText, who } from './account'
import { setLangChoice } from './i18n/index.svelte'

const now = 1_795_000_000

function entry(over: Partial<AccountAccess> = {}): AccountAccess {
  return {
    id: 'abc',
    account: 'acc_lea',
    device: 'dev_1',
    name: 'Léa Martin',
    device_name: 'iPad de Léa',
    rights: ['share', 'session'],
    first_seen: now - 10 * 86_400,
    last_seen: now - 30,
    via: 'relay',
    blocked: null,
    account_blocked: false,
    owner: false,
    ...over,
  }
}

describe('accès par un compte KyberNow', () => {
  beforeAll(() => setLangChoice('fr'))

  it('nomme la personne et son appareil', () => {
    expect(who(entry())).toBe('Léa Martin · iPad de Léa')
    expect(who(entry({ device_name: ' ' }))).toBe('Léa Martin')
  })

  it('range les droits et ignore les inconnus', () => {
    expect(rightsText(['admin', 'plus-tard', 'session'])).toBe('Session, Administrer')
    expect(rightsText([])).toBe('Aucun droit')
  })

  it('dit les droits, la voie et la dernière venue', () => {
    expect(detail(entry(), now)).toMatch(/^Session, Partage · hors de chez vous · /)
    expect(detail(entry({ via: 'lan' }), now)).toMatch(/· sur le réseau ·/)
    // Vu pour la première fois aujourd'hui : « nouvel appareil ».
    expect(detail(entry({ first_seen: now - 300, owner: true, rights: ['session', 'admin'] }), now)).toMatch(/^Propriétaire, Session, Administrer · nouvel appareil · /)
  })

  it('dit quand il a été bloqué', () => {
    expect(detail(entry({ blocked: now }), now)).toMatch(/^Bloqué le /)
    expect(detail(entry({ blocked: now, account_blocked: true }), now)).toMatch(/^Compte bloqué le /)
  })

  it('en anglais aussi', () => {
    setLangChoice('en')
    expect(rightsText(['session', 'share'])).toBe('Session, Sharing')
    expect(detail(entry(), now)).toMatch(/^Session, Sharing · away from home · /)
    setLangChoice('fr')
  })
})
