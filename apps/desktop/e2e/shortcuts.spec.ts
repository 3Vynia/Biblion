import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.goto('/dev/mock.html')
  await expect(page.getByRole('button', { name: /All books/ })).toBeVisible()
})

test('? shows every shortcut, Esc hides them', async ({ page }) => {
  await page.keyboard.press('Shift+?')
  const sheet = page.getByRole('dialog', { name: 'Keyboard shortcuts' })
  await expect(sheet).toBeVisible()
  await expect(sheet).toContainText('Move between books')
  await page.keyboard.press('Escape')
  await expect(sheet).toHaveCount(0)
  await page.keyboard.press('F1')
  await expect(sheet).toBeVisible()
})

test('Ctrl+number jumps to sections and / focuses search', async ({ page }) => {
  await page.keyboard.press('Control+6')
  await expect(page.getByRole('heading', { name: 'Reading stats', level: 1 })).toBeVisible()
  await page.keyboard.press('Control+,')
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible()
  await page.keyboard.press('Control+2')
  await page.keyboard.press('/')
  await expect(page.locator('#search')).toBeFocused()
  // Typing "/" or "?" in the search box is just text.
  await page.keyboard.type('a/b?')
  await expect(page.locator('#search')).toHaveValue('a/b?')
  await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toHaveCount(0)
})

test('arrows move through the grid and Enter reads', async ({ page }) => {
  await page.keyboard.press('Control+2')
  const cards = page.locator('.cell .card')
  await cards.first().click()
  await expect(page.getByRole('complementary', { name: 'Book details' })).toBeVisible()
  await page.keyboard.press('ArrowRight')
  await expect(page.locator('[data-index="1"]')).toBeFocused()
  const second = await page.locator('[data-index="1"]').getAttribute('title')
  // The open details follow the cursor.
  await expect(page.getByRole('complementary', { name: 'Book details' })).toContainText(second!)
  await page.keyboard.press('ArrowDown')
  await expect.poll(async () => Number(await page.locator('.card:focus').getAttribute('data-index'))).toBeGreaterThan(1)
  await page.keyboard.press('End')
  await expect(page.locator('[data-index="2999"]')).toBeFocused()
  await page.keyboard.press('Home')
  await expect(page.locator('[data-index="0"]')).toBeFocused()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('dialog', { name: /^Reading/ })).toBeVisible()
})

test('reader keys open panels, hide the bars and size the text', async ({ page }) => {
  await page.keyboard.press('Control+2')
  await page.locator('.cell .card').first().dblclick()
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await expect(reader.getByText(/^Page 1 of/)).toBeVisible()
  await page.keyboard.press('t')
  await expect(reader.getByRole('button', { name: 'Contents' })).toHaveClass(/\bon\b/)
  await page.keyboard.press('Escape')
  await expect(reader.getByRole('button', { name: 'Contents' })).not.toHaveClass(/\bon\b/)
  await page.keyboard.press('h')
  await expect(reader.locator('header')).toHaveClass(/hidden/)
  await page.keyboard.press('h')
  await page.keyboard.press('Control+=')
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as { __calls: { cmd: string; args: { value: { fontSize: number } } }[] }).__calls
          .filter((c) => c.cmd === 'set_reader_settings')
          .at(-1)?.args.value.fontSize,
      ),
    )
    .toBe(110)
  await page.keyboard.press('Shift+?')
  await expect(page.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(reader).toBeVisible()
})
