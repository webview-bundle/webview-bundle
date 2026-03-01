import { stripColor } from '../console.js';

export class OperationError extends Error {
  readonly name = 'OperationError';
  readonly originalErrors?: unknown;

  constructor(message?: string, originalErrors?: unknown) {
    super(message != null ? stripColor(message) : message);
    this.originalErrors = originalErrors;
  }
}

export function isOperationError(e: unknown): e is OperationError {
  return (
    e instanceof OperationError ||
    (e != null && typeof e === 'object' && (e as OperationError)?.name === 'OperationError')
  );
}
