import { describe, expect, it } from 'vitest';
import { snakeCase } from './utils.js';

describe('snakeCase', () => {
  it('make string into snake case', () => {
    expect(snakeCase('camelCase')).toBe('camel_case');
    expect(snakeCase('a word to snake case')).toBe('a_word_to_snake_case');
    expect(snakeCase('PascalCase')).toBe('pascal_case');
    expect(snakeCase('PascalCase_with_camelCase')).toBe('pascal_case_with_camel_case');
  });
});
