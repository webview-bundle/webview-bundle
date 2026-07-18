import util from 'node:util';
import CI from 'ci-info';
import { Option } from 'clipanion';
import kleur from 'kleur';
import supportsColor from 'supports-color';

export const ColorOption = Option.String('--color', 'auto', {
  description: 'Set the color mode for output. ["off", "on", "auto"] [Default: "auto"]',
  env: 'COLOR',
});

type ColorMode = 'off' | 'on' | 'auto';

function normalizeColor(val: string): ColorMode {
  switch (val.trim().toLowerCase()) {
    case '0':
    case 'off':
    case 'false':
    case 'no':
      return 'off';
    case '1':
    case 'on':
    case 'true':
    case 'yes':
      return 'on';
    default:
      return 'auto';
  }
}

export function configureColor(val: typeof ColorOption) {
  switch (normalizeColor(val)) {
    case 'off':
      kleur.enabled = false;
      break;
    case 'on':
      kleur.enabled = true;
      break;
    case 'auto':
      kleur.enabled = CI.GITHUB_ACTIONS
        ? true
        : supportsColor.stdout !== false && supportsColor.stdout.level > 0;
      break;
  }
}

export function isColorEnabled(): boolean {
  return kleur.enabled;
}

export const colors = {
  debug: (msg: string | number) => kleur.gray(msg),
  info: (msg: string | number) => kleur.white(msg),
  warn: (msg: string | number) => kleur.yellow(msg),
  error: (msg: string | number) => kleur.red(msg),
  success: (msg: string | number) => kleur.green(msg),
  header: (x: [string, string]) => kleur.gray(`${x[0]}: ${x[1]}`),
  bytes: (msg: string | number) => kleur.gray(msg),
  bold: (msg: string | number) => kleur.bold(msg),
  progress: (msg: string | number) => kleur.cyan(msg),
  underline: (msg: string | number) => kleur.underline(msg),
};
export const c = colors;

export function stripColor(message: string): string {
  return util.stripVTControlCharacters(message);
}
