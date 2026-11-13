import { expect, test, type Page } from '@playwright/test'

const nextPath = (page: Page, path: string) => page.evaluate((p) => ((window as unknown as { __nextPath?: string }).__nextPath = p), path)
const calls = (page: Page, cmd: string) =>
  page.evaluate(
    (c) => (window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls.filter((x) => x.cmd === c),
    cmd,
  )

test.beforeEach(async ({ page }) => {
  await page.goto('/dev/mock.html')
  await expect(page.getByRole('button', { name: /All books/ })).toBeVisible()
  await page.keyboard.press('Control+,')
  await page.getByRole('button', { name: 'Backup', exact: true }).click()
})

test('back up to a file', async ({ page }) => {
  await nextPath(page, '/home/me/biblion-backup.json')
  await page.getByRole('button', { name: 'Back up now…' }).click()
  await expect(page.locator('.backup-message')).toContainText('24 highlights and notes')
  expect((await calls(page, 'export_backup'))[0].args.path).toBe('/home/me/biblion-backup.json')
})

test('restoring shows what changes and asks first', async ({ page }) => {
  await nextPath(page, '/home/me/old.json')
  await page.getByRole('button', { name: 'Restore from a file…' }).click()
  const dialog = page.getByRole('alertdialog')
  await expect(dialog).toContainText('61 of its 64 books are in this library')
  await expect(dialog).toContainText('Not found: Lost Book (Some One)')
  await expect(dialog).toContainText('9 highlights and notes')
  await expect(dialog).toContainText('Nothing in your library is deleted')
  await dialog.getByRole('button', { name: 'Cancel' }).click()
  expect(await calls(page, 'restore_backup')).toHaveLength(0)

  // With preferences, from an automatic backup.
  await page.getByText('Also restore preferences').click()
  await page.locator('.auto-backups li').first().getByRole('button', { name: 'Restore' }).click()
  await expect(dialog).toContainText('preferences (reading, appearance, ambience) are replaced')
  await dialog.getByRole('button', { name: 'Restore' }).click()
  await expect(page.locator('.backup-message')).toContainText('Restored: 9 highlights and notes')
  const [restore] = await calls(page, 'restore_backup')
  expect(restore.args).toEqual({ path: '/data/backups/biblion-2026-11-12.json', settings: true })
})

test('a file that is not a backup is refused', async ({ page }) => {
  await nextPath(page, '/home/me/notes.txt')
  await page.getByRole('button', { name: 'Restore from a file…' }).click()
  await expect(page.locator('#backup .error')).toContainText('not a Biblion backup')
})

test('automatic backups can be turned off', async ({ page }) => {
  await page.getByText('Automatic backups').click()
  await expect.poll(async () => (await calls(page, 'set_auto_backup')).at(-1)?.args.on).toBe(false)
})
