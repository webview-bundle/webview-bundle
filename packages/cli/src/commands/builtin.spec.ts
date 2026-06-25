import { Cli } from 'clipanion';
import { describe, expect, it } from 'vitest';
import { BuiltinCommand } from './builtin.js';

function parse(argv: string[]): BuiltinCommand {
  const cli = new Cli();
  cli.register(BuiltinCommand);
  return cli.process(argv) as BuiltinCommand;
}

describe('builtin --android/--ios accept a boolean or a string', () => {
  it('treats a bare flag as boolean true (auto-detect)', () => {
    const cmd = parse(['builtin', '--android']);
    expect(cmd.android).toBe(true);
  });

  it('accepts an explicit path via `=` (string form)', () => {
    expect(parse(['builtin', '--android=./android/app']).android).toBe('./android/app');
    expect(parse(['builtin', '--ios=./ios']).ios).toBe('./ios');
  });

  it('is undefined when the flag is omitted', () => {
    const cmd = parse(['builtin']);
    expect(cmd.android).toBeUndefined();
    expect(cmd.ios).toBeUndefined();
  });
});
