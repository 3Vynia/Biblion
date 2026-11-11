import { expect, test } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.goto('/dev/mock.html')
})

test('the sidebar collapses to icons and remembers it', async ({ page }) => {
  const sidebar = page.locator('.sidebar')
  await expect(sidebar).not.toHaveClass(/collapsed/)
  await page.getByRole('button', { name: 'Hide sidebar' }).click()
  await expect(sidebar).toHaveClass(/collapsed/)
  await expect.poll(() => sidebar.evaluate((el) => el.getBoundingClientRect().width)).toBeLessThan(80)
  await page.keyboard.press('Control+b')
  await expect(sidebar).not.toHaveClass(/collapsed/)
})

test('settings change theme, accent and cover size', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  await expect(page.getByRole('heading', { name: 'Settings', level: 1 })).toBeVisible()

  await page.getByRole('radio', { name: 'Dark' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark')
  await page.getByRole('radio', { name: 'Light' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')

  await page.getByRole('radio', { name: 'Teal' }).click()
  await expect.poll(() => page.evaluate(() => document.documentElement.style.getPropertyValue('--accent'))).toBe('#0f766e')

  await page.getByRole('button', { name: 'Large', exact: true }).click()
  await page.getByRole('button', { name: /All books/ }).click()
  const width = await page.locator('.cell').first().evaluate((el) => el.getBoundingClientRect().width)
  expect(width).toBeGreaterThan(190)
})

test('the book panel shows full metadata and stats', async ({ page }) => {
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().click()
  const panel = page.getByRole('complementary', { name: 'Book details' })
  await expect(panel.getByText('Translated by')).toBeVisible()
  await expect(panel.getByText('98,765')).toBeVisible()
  await expect(panel.getByText('6h 36m')).toBeVisible()
  await expect(panel.getByText('1 image', { exact: true })).toHaveCount(0)
  await expect(panel.getByText('images', { exact: true })).toBeVisible()
  await expect(panel.getByText('EPUB 3.0')).toBeVisible()
  await expect(panel.getByText('978-84-01-35230-9')).toBeVisible()
  const subject = (await panel.locator('.tag').first().textContent())!.trim()
  await panel.locator('.tag').first().click()
  await expect(page.getByRole('heading', { name: subject, level: 1 })).toBeVisible()
})

test('dictionaries are listed and removing one asks first; online lookups can be turned off', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings' }).click()
  const row = page.locator('.folder', { hasText: 'Test English Dictionary' })
  await expect(row).toBeVisible()
  await row.getByRole('button', { name: 'Remove' }).click()
  await expect(page.getByRole('alertdialog')).toContainText('Remove “Test English Dictionary”?')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(row).toBeVisible()
  await row.getByRole('button', { name: 'Remove' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Remove dictionary' }).click()
  await expect(page.getByText('No offline dictionaries yet.')).toBeVisible()

  const online = page.getByRole('switch', { name: /Look words up online/ })
  await expect(online).toBeChecked()
  await page.getByText('Look words up online').click()
  await expect(online).not.toBeChecked()
})
