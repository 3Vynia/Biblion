import { expect, test, type Page } from '@playwright/test'

const saved = (page: Page) =>
  page.evaluate(() => {
    const calls = (window as unknown as { __calls: { cmd: string; args: { value: Record<string, unknown> } }[] }).__calls
    return calls.filter((c) => c.cmd === 'set_ambience_settings').at(-1)?.args.value
  })

async function openAmbience(page: Page) {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: 'Settings' }).click()
  await page.getByRole('button', { name: 'Ambience', exact: true }).click()
  return page.locator('#ambience')
}

test('a scene starts playing and shows up in the sidebar', async ({ page }) => {
  const panel = await openAmbience(page)
  await panel.getByRole('button', { name: 'Rainy library' }).click()
  await expect(panel.getByRole('switch', { name: 'Rain', exact: true })).toHaveAttribute('aria-checked', 'true')

  // Picking a scene starts it.
  const mini = page.locator('.sidebar')
  await expect(mini).toContainText('Rainy library')
  await mini.getByRole('button', { name: 'Pause ambience' }).click()
  await expect(panel.getByRole('button', { name: 'Play ambience' })).toBeVisible()
  await expect(mini.getByRole('button', { name: 'Pause ambience' })).toHaveCount(0)
})

test('modes, sliders and the sleep timer are saved', async ({ page }) => {
  const panel = await openAmbience(page)
  await panel.getByRole('radio', { name: 'Focus' }).click()
  await expect(panel.getByRole('radio', { name: 'Focus' })).toHaveAttribute('aria-checked', 'true')
  await panel.getByRole('radio', { name: '30 min' }).click()
  await expect
    .poll(async () => {
      const v = (await saved(page)) as { timerMinutes: number; soundscape: { mode: string; on: boolean } } | undefined
      return v && [v.soundscape.on, v.soundscape.mode, v.timerMinutes]
    })
    .toEqual([true, 'focus', 30])

  await panel.getByRole('button', { name: 'Turn the soundscape off' }).click()
  await expect(panel.getByRole('radio', { name: 'Focus' })).toHaveAttribute('aria-checked', 'false')
})

test('saving a mix and deleting it asks first', async ({ page }) => {
  const panel = await openAmbience(page)
  await panel.getByRole('switch', { name: 'Fireplace' }).click()
  await panel.getByRole('button', { name: 'Save mix' }).click()
  await page.getByLabel('Scene name').fill('My corner')
  await page.getByRole('button', { name: 'Save scene' }).click()
  await expect(panel.getByRole('button', { name: 'My corner', exact: true })).toBeVisible()

  // Cancelling keeps the scene.
  await panel.getByRole('button', { name: 'Delete scene My corner' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(panel.getByRole('button', { name: 'My corner', exact: true })).toBeVisible()

  await panel.getByRole('button', { name: 'Delete scene My corner' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete scene' }).click()
  await expect(panel.getByRole('button', { name: 'My corner', exact: true })).toHaveCount(0)
})

test('the reader has its own ambience panel', async ({ page }) => {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: 'All books' }).click()
  await page.locator('.cell .card').first().dblclick()
  const reader = page.getByRole('dialog', { name: /^Reading/ })
  await expect(reader).toBeVisible()
  await reader.getByRole('button', { name: 'Ambience', exact: true }).click()
  await reader.getByRole('button', { name: 'Seaside' }).click()
  await expect(reader.getByRole('button', { name: 'Ambience', exact: true })).toHaveClass(/playing/)
})
