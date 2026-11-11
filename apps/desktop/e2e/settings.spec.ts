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
  await expect(panel.getByText('6 h 36 min')).toBeVisible()
  await expect(panel.getByText('EPUB 3.0')).toBeVisible()
  await expect(panel.getByText('978-84-01-35230-9')).toBeVisible()
  await panel.getByRole('button', { name: /Adventure/ }).click()
  await expect(page.getByRole('heading', { name: 'Adventure' })).toBeVisible()
})
