import { expect, test, type Frame, type Page } from '@playwright/test'

const calls = (page: Page, cmd: string) =>
  page.evaluate((c) => (window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls.filter((x) => x.cmd === c), cmd)

async function openReader(page: Page): Promise<Frame> {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().dblclick()
  await expect(page.getByRole('dialog', { name: /^Reading/ })).toBeVisible()
  let frame: Frame | undefined
  await expect
    .poll(async () => {
      for (const f of page.frames()) if (f.url().startsWith('blob:') && (await f.locator('p').count().catch(() => 0)) > 0) frame = f
      return !!frame
    })
    .toBe(true)
  return frame!
}

test('reading aloud asks before downloading the voice, then reads sentence by sentence', async ({ page }) => {
  const frame = await openReader(page)
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await reader.getByRole('button', { name: 'Read aloud' }).click()

  // The first time, the voice is downloaded only after asking.
  const ask = page.getByRole('alertdialog')
  await expect(ask).toContainText('Download the voice?')
  await expect(ask).toContainText('Nothing you read is ever sent anywhere')
  await ask.getByRole('button', { name: 'Download' }).click()

  const bar = reader.getByRole('region', { name: 'Read aloud' })
  await expect(bar.getByRole('button', { name: 'Pause reading' })).toBeVisible()
  await expect(bar.locator('.sentence')).not.toHaveText('')
  // A Spanish voice for this book, and the sentence is marked in the page.
  await expect.poll(async () => ((await calls(page, 'tts_speak'))[0]?.args.voice as string) ?? '').toMatch(/^(ef|em|af|bf)_/)
  await expect.poll(() => frame.evaluate(() => (CSS as unknown as { highlights?: Map<string, unknown> }).highlights?.has('biblion-tts') ?? false)).toBe(true)

  // Sentences move on by themselves, and can be skipped.
  const first = await bar.locator('.sentence').textContent()
  await expect(bar.locator('.sentence')).not.toHaveText(first!, { timeout: 8000 })
  await bar.getByRole('button', { name: 'Pause reading' }).click()
  await expect(bar.getByRole('button', { name: 'Continue reading' })).toBeVisible()
  // The marked sentence is the one being read, even across paragraphs.
  for (let i = 0; i < 4; i++) {
    const spoken = (await bar.locator('.sentence').textContent())!.trim()
    const marked = await frame.evaluate(() => {
      const h = (CSS as unknown as { highlights: Map<string, Set<Range>> }).highlights.get('biblion-tts')
      return h ? [...h][0].toString().replace(/\s+/g, ' ').trim() : ''
    })
    expect(marked).toBe(spoken)
    await bar.getByRole('button', { name: 'Next sentence' }).click()
    await expect(bar.locator('.sentence')).not.toHaveText(spoken)
    await bar.getByRole('button', { name: 'Pause reading' }).click({ timeout: 2000 }).catch(() => {})
  }
  await bar.getByRole('button', { name: 'Continue reading' }).click()
  await bar.getByRole('button', { name: 'Next sentence' }).click()

  // A slower pace is remembered and used from the current sentence on.
  await bar.getByRole('button', { name: 'Slower' }).click()
  await expect(bar).toContainText('0.95×')
  await expect.poll(async () => (await calls(page, 'tts_speak')).at(-1)?.args.speed).toBe(0.95)

  // L stops it.
  await page.keyboard.press('l')
  await expect(bar).toHaveCount(0)
  await expect.poll(() => frame.evaluate(() => (CSS as unknown as { highlights?: Map<string, unknown> }).highlights?.has('biblion-tts') ?? false)).toBe(false)
})

test('declining the download closes the player', async ({ page }) => {
  await openReader(page)
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await page.keyboard.press('l')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(reader.getByRole('region', { name: 'Read aloud' })).toHaveCount(0)
  expect(await calls(page, 'tts_install')).toHaveLength(0)
})
