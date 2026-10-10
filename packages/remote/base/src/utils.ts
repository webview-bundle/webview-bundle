import type { UpdateData } from './schema.js';

export function stringifyUpdateData(data: UpdateData): string {
  return JSON.stringify(data, (_key, value: unknown) => {
    if (value == null || typeof value !== 'object' || Array.isArray(value)) {
      return value;
    }

    const record = value as Record<string, unknown>;
    return Object.fromEntries(
      Object.keys(record)
        .sort()
        .map(key => [key, record[key]])
    );
  });
}
