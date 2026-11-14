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

test('online voices are connected with a key in Settings and offered in the player', async ({ page }) => {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: 'Settings' }).click()
  const privacy = page.locator('#privacy')
  await privacy.getByRole('button', { name: 'Connect…' }).click()
  await privacy.getByLabel('Key').fill('bad')
  await privacy.getByLabel('Region').fill('westeurope')
  await privacy.getByRole('button', { name: 'Connect', exact: true }).click()
  await expect(privacy.locator('.error')).toContainText('did not accept the key')
  await privacy.getByLabel('Key').fill('a-good-key')
  await privacy.getByRole('button', { name: 'Connect', exact: true }).click()
  await expect(privacy).toContainText('Connected · westeurope')
  await expect(privacy.locator('.summary')).toContainText('Microsoft')

  // In a book, Álvaro is offered with the voices on this computer.
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().dblclick()
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await expect(reader.getByRole('button', { name: 'Read aloud' })).toBeEnabled()
  await page.keyboard.press('l')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Download' }).click()
  const bar = reader.getByRole('region', { name: 'Read aloud' })
  await expect(bar.getByRole('button', { name: 'Pause reading' })).toBeVisible()
  await expect(bar.getByLabel('Voice').locator('optgroup[label="Online · Microsoft"] option')).toHaveText(['Álvaro (narrator)', 'Jorge'])
  await bar.getByLabel('Voice').selectOption('azure:alvaro')
  await expect.poll(async () => (await calls(page, 'tts_speak')).at(-1)?.args.voice).toBe('azure:alvaro')
  await page.keyboard.press('l')
  await expect(bar).toHaveCount(0)

  // Disconnecting asks first.
  await page.keyboard.press('Escape')
  await page.getByRole('button', { name: 'Settings' }).click()
  await privacy.getByRole('button', { name: 'Disconnect' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Disconnect' }).click()
  await expect(privacy.getByRole('button', { name: 'Connect…' })).toBeVisible()
})
