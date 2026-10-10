import { z } from 'zod/v4';
import { Exception } from './exception.js';

const RuntimeVersionSchema = z.int().min(1);

export function parseRuntimeVersion(val: string | undefined): number {
  const parsed = RuntimeVersionSchema.safeParse(val);
  if (!parsed.success) {
    throw new Exception('invalid_runtime_version', { requestedRuntimeVersion: val });
  }
  return parsed.data;
}
