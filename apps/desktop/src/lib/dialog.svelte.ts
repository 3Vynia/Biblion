// App-wide dialogs. Every edit or delete goes through `confirm()`, which
// resolves to true only when the user explicitly accepts. `prompt()` asks for
// a value (e.g. a new name); callers confirm the change afterwards.

export interface ConfirmOptions {
  title: string
  message: string
  confirmLabel: string
  /** Red button, for actions that remove something. */
  danger?: boolean
}

export interface PromptOptions {
  title: string
  label: string
  value: string
  confirmLabel: string
  maxLength?: number
}

type Pending =
  | ({ kind: 'confirm'; resolve: (ok: boolean) => void } & ConfirmOptions)
  | ({ kind: 'prompt'; resolve: (value: string | null) => void } & PromptOptions)

class DialogState {
  current = $state<Pending | null>(null)

  confirm(options: ConfirmOptions): Promise<boolean> {
    this.#cancel()
    return new Promise((resolve) => (this.current = { kind: 'confirm', ...options, resolve }))
  }

  prompt(options: PromptOptions): Promise<string | null> {
    this.#cancel()
    return new Promise((resolve) => (this.current = { kind: 'prompt', ...options, resolve }))
  }

  /** Accept: `true` for confirms, the typed value for prompts. */
  accept(value?: string) {
    const c = this.current
    this.current = null
    if (c?.kind === 'confirm') c.resolve(true)
    else if (c?.kind === 'prompt') c.resolve(value?.trim() || null)
  }

  #cancel() {
    const c = this.current
    this.current = null
    if (c?.kind === 'confirm') c.resolve(false)
    else if (c?.kind === 'prompt') c.resolve(null)
  }

  cancel() {
    this.#cancel()
  }
}

export const dialog = new DialogState()
export const confirm = (options: ConfirmOptions) => dialog.confirm(options)
export const prompt = (options: PromptOptions) => dialog.prompt(options)
