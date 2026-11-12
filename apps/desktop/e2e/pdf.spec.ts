import { expect, test, type Page } from '@playwright/test'

async function openPdf(page: Page) {
  const ranges: string[] = []
  page.on('request', (r) => r.url().includes('sample.pdf') && ranges.push(r.headers()['range'] ?? 'none'))
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: /Formats/ }).click()
  await page.locator('.tile', { hasText: 'PDF' }).click()
  await page.locator('.cell .card').first().dblclick()
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await expect(reader.locator('.page canvas').first()).toBeVisible()
  await expect(reader.locator('.sub')).toHaveText('Page 1 of 5')
  return { reader, ranges }
}

const calls = (page: Page, cmd: string) =>
  page.evaluate(
    (c) => (window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls.filter((x) => x.cmd === c),
    cmd,
  )

test('reads the PDF in ranges, renders text and makes a cover', async ({ page }) => {
  const { reader, ranges } = await openPdf(page)
  expect(ranges.length).toBeGreaterThan(0)
  expect(ranges.every((r) => r.startsWith('bytes='))).toBe(true)
  await expect(reader.locator('.page .textLayer span').first()).toBeAttached()
  await expect.poll(async () => (await calls(page, 'set_cover')).length).toBe(1)
})

test('outline, search, zoom and progress', async ({ page }) => {
  const { reader } = await openPdf(page)
  await page.getByRole('button', { name: 'Contents' }).click()
  await page.getByRole('button', { name: 'Chapter Three' }).click()
  await expect(reader.locator('.sub')).toHaveText('Page 3 of 5')
  await expect.poll(async () => (await calls(page, 'set_progress')).at(-1)?.args.locator).toBe('page:2')

  await page.getByRole('button', { name: 'Search in document' }).click()
  await page.getByPlaceholder('Search in this document').fill('logbook')
  await page.keyboard.press('Enter')
  await expect(reader.locator('.hit-row')).toHaveCount(1)
  await expect(reader.locator('.hit-row')).toContainText('Page 3')
  await expect(reader.locator('.textLayer .hit').first()).toBeAttached()

  const zoom = reader.locator('.zoom-value')
  const before = await zoom.textContent()
  await page.getByRole('button', { name: 'Zoom in' }).click()
  await expect(zoom).not.toHaveText(before!)
  await expect(reader.locator('.sub')).toHaveText('Page 3 of 5')
})

test('selected text offers Copy and Define, not highlights', async ({ page }) => {
  const { reader } = await openPdf(page)
  await reader.locator('.page .textLayer span').first().waitFor({ state: 'attached' })
  await page.evaluate(() => {
    const span = [...document.querySelectorAll('.textLayer span')].find((s) => s.textContent?.includes('keeper'))!
    const text = span.firstChild!
    const at = text.textContent!.indexOf('keeper')
    const range = document.createRange()
    range.setStart(text, at)
    range.setEnd(text, at + 'keeper'.length)
    getSelection()!.removeAllRanges()
    getSelection()!.addRange(range)
    document.querySelector('.scroller')!.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }))
  })
  const menu = page.getByRole('toolbar', { name: 'Selection' })
  await expect(menu.getByRole('button', { name: 'Define' })).toBeVisible()
  await expect(menu.getByRole('button', { name: /Highlight in/ })).toHaveCount(0)
  await menu.getByRole('button', { name: 'Define' }).click()
  await expect(page.getByRole('dialog', { name: 'Definition of keeper' })).toContainText('A person who looks after')
})

test('PDFs without a cover get one in the background', async ({ page }) => {
  await page.addInitScript(() => ((window as unknown as { __mockCoverQueue: boolean }).__mockCoverQueue = true))
  await page.goto('/dev/mock.html')
  // A finished scan starts the job; trigger one from Settings.
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('button', { name: 'Scan now' }).click()
  await expect.poll(async () => (await calls(page, 'set_cover')).length, { timeout: 20_000 }).toBeGreaterThan(3)
})
