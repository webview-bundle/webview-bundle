/**
 * Tiny assertion helpers with no test-runner dependency, so the abstract cases
 * stay runner-agnostic: a thrown {@link AssertionError} fails whatever test (or
 * plain `for await` loop) is running them.
 */
export class AssertionError extends Error {
  override name = 'AssertionError';
}

function format(value: unknown): string {
  return typeof value === 'string' ? JSON.stringify(value) : String(value);
}

export function assert(condition: unknown, message: string): asserts condition {
  if (!condition) {
    throw new AssertionError(message);
  }
}

export function assertEqual<T>(actual: T, expected: T, message?: string): void {
  if (!Object.is(actual, expected)) {
    const prefix = message ? `${message}: ` : '';
    throw new AssertionError(`${prefix}expected ${format(expected)}, got ${format(actual)}`);
  }
}

export function assertContains(haystack: string, needle: string, message?: string): void {
  if (!haystack.includes(needle)) {
    const prefix = message ? `${message}: ` : '';
    throw new AssertionError(`${prefix}expected ${format(haystack)} to contain ${format(needle)}`);
  }
}

export function assertGreaterThan(actual: number, threshold: number, message?: string): void {
  if (!(actual > threshold)) {
    const prefix = message ? `${message}: ` : '';
    throw new AssertionError(`${prefix}expected ${actual} to be greater than ${threshold}`);
  }
}
