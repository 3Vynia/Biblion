import { expect, test, type Page } from '@playwright/test'

async function openComic(page: Page) {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: /Formats/ }).click()
  await page.locator('.tile', { hasText: 'CBZ' }).click()
  await page.locator('.cell .card').first().dblclick()
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await expect(reader).toBeVisible()
  await expect(reader.locator('header')).toContainText('Page 1 of 7')
  return reader
}

const progressCalls = (page: Page) =>
  page.evaluate(() =>
    (window as unknown as { __calls: { cmd: string; args: { locator: string } }[] }).__calls
      .filter((c) => c.cmd === 'set_progress')
      .map((c) => c.args.locator),
  )

test('pages turn as spreads after the cover and progress is saved', async ({ page }) => {
  const reader = await openComic(page)
  await expect(reader.locator('.spread img')).toHaveCount(1) // the cover is alone
  await page.keyboard.press('ArrowRight')
  await expect(reader.locator('header')).toContainText('Page 2 of 7')
  await expect(reader.locator('.spread img')).toHaveCount(2) // wide window: two pages
  await expect.poll(async () => (await progressCalls(page)).at(-1)).toBe('page:1')
  await page.keyboard.press('End')
  await expect(reader.locator('header')).toContainText('Page 7 of 7')
  await page.keyboard.press('Home')
  await expect(reader.locator('header')).toContainText('Page 1 of 7')
})

test('manga mode turns pages right to left; strip mode shows every page', async ({ page }) => {
  const reader = await openComic(page)
  await page.getByRole('button', { name: 'Comic settings' }).click()
  await page.getByRole('button', { name: 'Manga (right to left)' }).click()
  await page.getByRole('button', { name: 'Comic settings' }).click()
  await page.keyboard.press('ArrowLeft')
  await expect(reader.locator('header')).toContainText('Page 2 of 7')
  await expect(reader.locator('.spread')).toHaveClass(/rtl/)
  await page.keyboard.press('ArrowRight')
  await expect(reader.locator('header')).toContainText('Page 1 of 7')

  await page.getByRole('button', { name: 'Comic settings' }).click()
  await page.getByRole('button', { name: 'Strip', exact: true }).click()
  await expect(reader.locator('.strip-page')).toHaveCount(7)
})
