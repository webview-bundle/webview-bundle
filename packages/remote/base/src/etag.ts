import { createHash } from 'node:crypto';
import type { UpdateData } from './schema.js';
import { stringifyUpdateData } from './utils.js';

export function generateUpdateDataETag(data: UpdateData): string {
  const etag = createHash('sha256').update(stringifyUpdateData(data), 'utf8').digest('hex');
  return `"${etag}"`;
}
