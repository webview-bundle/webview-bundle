import { stripColor } from '../console.js';

export class ApiError extends Error {
  readonly name = 'OperationError';
  readonly originalErrors?: unknown;

  constructor(message?: string, originalErrors?: unknown) {
    super(message != null ? stripColor(message) : message);
    this.originalErrors = originalErrors;
  }
}

export function isApiError(e: unknown): e is ApiError {
  return (
    e instanceof ApiError ||
    (e != null && typeof e === 'object' && (e as ApiError)?.name === 'OperationError')
  );
}
