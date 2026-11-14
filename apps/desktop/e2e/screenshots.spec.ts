// Screenshots for the README, from the mock library of well-known books.
// Run with: SCREENSHOTS=1 pnpm exec playwright test e2e/screenshots.spec.ts
import { expect, test, type Page } from '@playwright/test'

const OUT = '../../docs/screenshots'

test.skip(!process.env.SCREENSHOTS, 'only when making screenshots')
test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 })

async function open(page: Page, theme: 'light' | 'dark') {
  await page.goto('/dev/mock.html?showcase')
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('radio', { name: theme === 'dark' ? 'Dark' : 'Light' }).click()
}

const settle = (page: Page) => page.waitForTimeout(1600)

test('home', async ({ page }) => {
  await open(page, 'light')
  await page.getByRole('button', { name: 'Home' }).click()
  await settle(page)
  await page.screenshot({ path: `${OUT}/home.png` })
})

test('library with book details', async ({ page }) => {
  await open(page, 'dark')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell').filter({ has: page.getByText('Dune', { exact: true }) }).locator('.card').first().click()
  await expect(page.getByRole('complementary', { name: 'Book details' })).toBeVisible()
  await settle(page)
  await page.screenshot({ path: `${OUT}/library.png` })
})

test('reader', async ({ page }) => {
  await open(page, 'light')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell').filter({ has: page.getByText('Frankenstein', { exact: true }) }).locator('.card').first().dblclick()
  await expect(page.getByRole('dialog', { name: /^Reading/ })).toBeVisible()
  await page.waitForTimeout(2500)
  await page.keyboard.press('a')
  await page
    .getByRole('radio', { name: /Sepia/ })
    .or(page.getByRole('button', { name: /Sepia/ }))
    .first()
    .click({ timeout: 3000 })
    .catch(() => {})
  await page.keyboard.press('Escape')
  await settle(page)
  await page.screenshot({ path: `${OUT}/reader.png` })
})

test('online details', async ({ page }) => {
  await open(page, 'dark')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell').filter({ has: page.getByText('Dune', { exact: true }) }).locator('.card').first().click()
  await page.getByRole('button', { name: 'Find details online' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Turn on and search' }).click()
  await expect(page.locator('.result').first()).toBeVisible()
  await page.waitForTimeout(2500)
  await page.screenshot({ path: `${OUT}/online-details.png` })
})

test('ambience', async ({ page }) => {
  await open(page, 'dark')
  await page.getByRole('button', { name: 'Ambience', exact: true }).click()
  await page.waitForTimeout(1200)
  await page.getByRole('button', { name: 'Ambience', exact: true }).click()
  await page.locator('#ambience').getByRole('tab', { name: 'Sounds & scenes' }).click()
  await page.locator('#ambience').getByRole('button', { name: 'Rainy library' }).click()
  await page.waitForTimeout(2500)
  await page.screenshot({ path: `${OUT}/ambience.png` })
})

test('stats', async ({ page }) => {
  await open(page, 'light')
  await page.getByRole('button', { name: 'Stats' }).click()
  await page.waitForTimeout(2200)
  await page.screenshot({ path: `${OUT}/stats.png` })
})
