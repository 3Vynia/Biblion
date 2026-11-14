import { describe, expect, it, vi } from 'vitest'

vi.mock('./api', () => ({ api: {}, errorMessage: String }))
vi.mock('./ambience/store.svelte', () => ({ ambience: { duck: () => {} } }))

const { ssmlToSegments, voiceLang, pickVoice } = await import('./readAloud.svelte')

describe('reading aloud', () => {
  it('splits an SSML block into sentences at its marks', () => {
    const ssml =
      '<speak xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="es"><mark name="0"/>Era una tarde &amp; llovía. ' +
      '<mark name="1"/>Ella <emphasis>leía</emphasis>.<break/><mark name="2"/>  <mark name="3"/>“Fin”</speak>'
    expect(ssmlToSegments(ssml)).toEqual([
      { mark: '0', text: 'Era una tarde & llovía.' },
      { mark: '1', text: 'Ella leía.' },
      { mark: '3', text: '“Fin”' },
    ])
  })

  it('keeps text before the first mark, and skips blocks with nothing to say', () => {
    expect(ssmlToSegments('<speak>Intro <mark name="a"/>Next</speak>')).toEqual([
      { mark: null, text: 'Intro' },
      { mark: 'a', text: 'Next' },
    ])
    expect(ssmlToSegments('<speak><mark name="0"/> — * </speak>')).toEqual([])
    expect(ssmlToSegments('<speak>Caf&#233; &#x2014; ok</speak>')[0].text).toBe('Café — ok')
  })

  it('picks a voice for the book language', () => {
    const voices = [
      { id: 'ef_dora+af_bella', name: 'Dora · Bella', lang: 'es' },
      { id: 'ef_dora', name: 'Dora', lang: 'es' },
      { id: 'em_alex', name: 'Alex', lang: 'es' },
      { id: 'af_heart', name: 'Heart', lang: 'en-us' },
      { id: 'bf_emma', name: 'Emma', lang: 'en-gb' },
    ]
    expect(voiceLang('es-ES')).toBe('es')
    expect(voiceLang('en')).toBe('en-us')
    expect(voiceLang('en-GB')).toBe('en-gb')
    expect(voiceLang('fr')).toBeNull()
    expect(pickVoice(voices, 'es', {})).toBe('ef_dora+af_bella')
    expect(pickVoice(voices, 'es', { es: 'em_alex' })).toBe('em_alex')
    expect(pickVoice(voices, 'en-us', { 'en-us': 'gone' })).toBe('af_heart')
    expect(pickVoice(voices, null, {})).toBe('ef_dora+af_bella')
  })
})
