import { expect, test } from '@playwright/test'

test('the stats screen shows the year, sets a goal and has a table view', async ({ page }) => {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: 'Stats', exact: true }).click()
  await expect(page.getByRole('heading', { name: 'Reading stats', level: 1 })).toBeVisible()
  await expect(page.locator('.hero')).toContainText(' h ')
  await expect(page.getByRole('img', { name: /^Reading days in \d{4}/ })).toBeVisible()

  await page.getByRole('button', { name: 'Set a goal' }).click()
  await page.getByLabel(/Books to finish in/).fill('50')
  await page.getByRole('button', { name: 'Save goal' }).click()
  await expect(page.getByRole('meter', { name: 'Reading goal' })).toHaveAttribute('aria-valuemax', '50')
  await expect(page.locator('.goal-value')).toContainText('of 50')

  await page.getByRole('button', { name: 'Show table' }).click()
  await expect(page.locator('table tbody tr')).toHaveCount(12)

  // Earlier years can be browsed, later ones not.
  await expect(page.getByRole('button', { name: 'Next year' })).toBeDisabled()
  await page.getByRole('button', { name: 'Previous year' }).click()
  await expect(page.getByRole('button', { name: 'Set a goal' })).toHaveCount(0)
})

test('reading a book logs the pages turned', async ({ page }) => {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().dblclick()
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await expect(reader.getByText(/^Page 1 of/)).toBeVisible()
  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowRight')
  await page.waitForTimeout(500)
  await page.keyboard.press('Escape')
  await expect(reader).toHaveCount(0)
  const logged = await page.evaluate(() =>
    (window as unknown as { __calls: { cmd: string; args: { pages: number; day: string } }[] }).__calls.filter((c) => c.cmd === 'log_reading'),
  )
  expect(logged.reduce((n, c) => n + c.args.pages, 0)).toBeGreaterThanOrEqual(1)
  expect(logged[0].args.day).toMatch(/^\d{4}-\d{2}-\d{2}$/)
})
