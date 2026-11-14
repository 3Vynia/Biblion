import { expect, test, type Page } from '@playwright/test'

const calls = (page: Page, cmd: string) =>
  page.evaluate(
    (c) => (window as unknown as { __calls: { cmd: string; args: Record<string, unknown> }[] }).__calls.filter((x) => x.cmd === c),
    cmd,
  )

async function openEditor(page: Page) {
  await page.goto('/dev/mock.html')
  await page.getByRole('button', { name: /All books/ }).click()
  await page.locator('.cell .card').first().click()
  const panel = page.getByRole('complementary', { name: 'Book details' })
  await panel.getByRole('button', { name: 'Edit details' }).click()
  return { panel, editor: page.getByRole('dialog', { name: 'Edit details' }) }
}

test('edit series, number, authors and subjects, and confirm', async ({ page }) => {
  const { panel, editor } = await openEditor(page)
  await expect(editor.getByRole('button', { name: 'Save changes…' })).toBeDisabled()

  await editor.getByLabel('Series', { exact: true }).fill('Crossed')
  await editor.getByLabel('Number in series').fill('12')
  const authors = editor.getByRole('textbox', { name: 'Authors' })
  await authors.fill('Ennis, Garth')
  await authors.press('Enter')
  await editor.getByRole('textbox', { name: 'Subjects' }).fill('Horror')
  await editor.getByRole('textbox', { name: 'Subjects' }).press(',')
  await expect(editor.locator('.status')).toContainText('authors, series, subjects')

  await editor.getByRole('button', { name: 'Save changes…' }).click()
  const ask = page.getByRole('alertdialog')
  await expect(ask).toContainText('The file itself stays as it is')
  await ask.getByRole('button', { name: 'Cancel' }).click()
  expect(await calls(page, 'update_metadata')).toHaveLength(0)

  await editor.getByRole('button', { name: 'Save changes…' }).click()
  await ask.getByRole('button', { name: 'Save changes' }).click()
  await expect(editor).toHaveCount(0)
  const m = (await calls(page, 'update_metadata'))[0].args.metadata as { series: string; series_index: number; authors: string[]; tags: string[] }
  expect([m.series, m.series_index]).toEqual(['Crossed', 12])
  expect(m.authors.at(-1)).toBe('Ennis, Garth')
  expect(m.tags).toContain('Horror')
  await expect(panel).toContainText('Crossed · #12')
  await expect(panel.getByRole('status')).toHaveText(/Details saved/)
})

test('a new cover from an image file, and leaving asks before discarding', async ({ page }) => {
  const { editor } = await openEditor(page)
  await page.evaluate(() => ((window as unknown as { __nextPath?: string }).__nextPath = '/home/me/notes.txt'))
  await editor.getByRole('button', { name: 'Choose image…' }).click()
  await expect(editor).toContainText('Choose a JPEG, PNG')

  await page.evaluate(() => ((window as unknown as { __nextPath?: string }).__nextPath = '/home/me/crossed-12.jpg'))
  await editor.getByRole('button', { name: 'Choose image…' }).click()
  await expect(editor.getByRole('img', { name: 'New cover' })).toBeVisible()
  await expect(editor.locator('.status')).toContainText('cover')

  // Esc with changes asks first; cancelling keeps the editor.
  await editor.getByLabel('Title', { exact: true }).focus()
  await page.keyboard.press('Escape')
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click()
  await expect(editor).toBeVisible()

  await editor.getByRole('button', { name: 'Save changes…' }).click()
  await page.getByRole('alertdialog').getByRole('button', { name: 'Save changes' }).click()
  await expect(editor).toHaveCount(0)
  expect((await calls(page, 'cover_from_file'))[0].args.path).toBe('/home/me/crossed-12.jpg')
  expect(await calls(page, 'update_metadata')).toHaveLength(0)
})

test('an empty title cannot be saved', async ({ page }) => {
  const { editor } = await openEditor(page)
  await editor.getByLabel('Title', { exact: true }).fill('  ')
  await editor.getByLabel('Publisher').fill('Avatar Press')
  await expect(editor.getByRole('button', { name: 'Save changes…' })).toBeDisabled()
})
