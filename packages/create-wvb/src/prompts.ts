import { confirm, input, select } from '@inquirer/prompts';

/**
 * Thrown when the user aborts a prompt (Ctrl-C). `@inquirer/prompts` does not re-export
 * `ExitPromptError`, and more than one `@inquirer/core` copy can be resolved at once, so `instanceof`
 * is unreliable — the error name is the only stable signal.
 */
export class CancelledError extends Error {
  constructor() {
    super('Cancelled.');
    this.name = 'CancelledError';
  }
}

export function isCancel(error: unknown): boolean {
  return (
    error instanceof CancelledError || (error instanceof Error && error.name === 'ExitPromptError')
  );
}

async function guard<T>(run: () => Promise<T>): Promise<T> {
  try {
    return await run();
  } catch (error) {
    if (isCancel(error)) {
      throw new CancelledError();
    }
    throw error;
  }
}

export function isInteractive(): boolean {
  return process.stdin.isTTY === true && process.stdout.isTTY === true && process.env.CI == null;
}

export interface Choice<T> {
  readonly name: string;
  readonly value: T;
  readonly description?: string;
  readonly disabled?: string | false;
}

export function promptText(
  message: string,
  options: { readonly default?: string; readonly validate?: (value: string) => string | null }
): Promise<string> {
  return guard(() =>
    input({
      message,
      default: options.default,
      validate: value => {
        const problem = options.validate?.(value.trim());
        return problem == null ? true : problem;
      },
    })
  );
}

export function promptSelect<T>(message: string, choices: readonly Choice<T>[]): Promise<T> {
  return guard(() =>
    select({
      message,
      choices: choices.map(choice => ({
        name: choice.name,
        value: choice.value,
        description: choice.description,
        disabled: choice.disabled,
      })),
      loop: false,
    })
  );
}

export function promptConfirm(message: string, initial: boolean): Promise<boolean> {
  return guard(() => confirm({ message, default: initial }));
}
