// Les règles d'un invité du partage (guestrules.ts) : droits, avis des changements de rôle, image
// ajustée par la Station, « Qualité de l'image ». `npm test`.

import { describe, expect, it, vi } from 'vitest'

// `share.ts` passe par `device.ts`, qui lit le navigateur au chargement : seul le nom des qualités sert.
vi.mock('./share', () => ({ qualityName: (q: string) => `qualité ${q}` }))

import type { GuestImage } from '../lib/api'
import { imageNotice, lightened, near, qualityStep, rights, roleNotices } from './guestrules'

const image = (over: Partial<GuestImage> = {}): GuestImage => ({ quality: 'auto', width: 1280, height: 720, fps: 30, codec: 'h264', follows_host: false, reason: 'asked', ...over })

describe('droits d’un invité', () => {
  it('deux droits indépendants', () => {
    expect(rights('spectator')).toEqual({ pads: false, kbm: false })
    expect(rights('player')).toEqual({ pads: true, kbm: false })
    expect(rights('kbm')).toEqual({ pads: false, kbm: true })
    expect(rights('player_kbm')).toEqual({ pads: true, kbm: true })
  })

  it('un avis par droit donné ou repris', () => {
    expect(roleNotices('spectator', 'player_kbm')).toEqual(['share.guest.canPlay', 'share.guest.gotKbm'])
    expect(roleNotices('player_kbm', 'player')).toEqual(['share.guest.lostKbm'])
    expect(roleNotices('player', 'kbm')).toEqual(['share.guest.gotKbm', 'share.guest.lostPads'])
  })

  it('plus aucun droit : « Vous regardez de nouveau. » à la place des reprises', () => {
    expect(roleNotices('player_kbm', 'spectator')).toEqual(['share.guest.watching'])
  })
})

describe('image ajustée par la Station', () => {
  it('pas d’avis pour une image demandée', () => {
    expect(imageNotice(image())).toEqual({ key: '', text: '' })
    expect(imageNotice(image({ reason: 'capped' }))).toEqual({ key: '', text: '' })
  })

  it('un avis par changement, avec sa clé', () => {
    const lag = imageNotice(image({ reason: 'lag' }))
    expect(lag.key).toBe('lag 1280x720@30')
    expect(lag.text).not.toBe('')
    expect(imageNotice(image({ reason: 'limit' })).key).toBe('limit 1280x720@30')
    expect(imageNotice(image({ reason: 'limit', width: 1920, height: 1080 })).key).not.toBe(imageNotice(image({ reason: 'limit' })).key)
  })

  it('une image plus petite qui n’est pas l’écran de l’hôte a été allégée', () => {
    const before = { width: 1920, height: 1080 }
    expect(lightened(before, { width: 1280, height: 720 }, [])).toBe(true)
    expect(lightened(before, { width: 1280, height: 720 }, [{ width: 1284, height: 720 }])).toBe(false)
    expect(lightened(before, { width: 2560, height: 1440 }, [])).toBe(false)
    expect(lightened(null, { width: 1280, height: 720 }, [])).toBe(false)
  })

  it('tailles presque égales à 8 pixels près', () => {
    expect(near({ width: 1920, height: 1080 }, { width: 1928, height: 1072 })).toBe(true)
    expect(near({ width: 1920, height: 1080 }, { width: 1929, height: 1080 })).toBe(false)
  })
})

describe('« Qualité de l’image » changée', () => {
  const plan = { codec: 'hevc', yuv444: false }

  it('rien quand le préréglage ne change pas', () => {
    expect(qualityStep('1080p60', '1080p60', null, plan, image(), null)).toBe('none')
  })

  it('relance quand le format change', () => {
    expect(qualityStep('host', '1080p60', { codec: 'h264', yuv444: false }, plan, image(), null)).toBe('restart')
    expect(qualityStep('1080p60', 'host', { codec: 'hevc', yuv444: true }, plan, image(), null)).toBe('restart')
  })

  it('attend la nouvelle taille dans le flux, sauf si c’est déjà elle', () => {
    expect(qualityStep('host', '720p30', { codec: 'hevc', yuv444: false }, plan, image(), { width: 1920, height: 1080 })).toEqual({ target: { width: 1280, height: 720 } })
    expect(qualityStep('host', '720p30', null, plan, image(), { width: 1280, height: 720 })).toBe('none')
    expect(qualityStep('host', '720p30', null, plan, image({ width: null, height: null }), null)).toEqual({ target: null })
  })
})
