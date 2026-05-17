import CI from 'ci-info';
import { Option } from 'clipanion';
import kleur from 'kleur';
import supportsColor from 'supports-color';
import { isEnum } from 'typanion';

export function isColorEnabled(): boolean {
  return kleur.enabled;
}

export function enableColor(): void {
  kleur.enabled = true;
}

export function disableColor(): void {
  kleur.enabled = false;
}

export function autoColor(): void {
  kleur.enabled = CI.GITHUB_ACTIONS
    ? true
    : supportsColor.stdout !== false && supportsColor.stdout.level > 0;
}

const COLOR_MODES = ['on', 'off', 'auto'] as const;
export type ColorMode = (typeof COLOR_MODES)[number];

export const ColorModeOption = Option.String('--color', 'auto', {
  validator: isEnum(COLOR_MODES),
});

export function setColorMode(mode: ColorMode) {
  switch (mode) {
    case 'on':
      enableColor();
      break;
    case 'off':
      disableColor();
      break;
    case 'auto':
      autoColor();
      break;
  }
}

export const colors = {
  info(str: string | number) {
    return kleur.bold().blue(str);
  },
  warn(str: string | number) {
    return kleur.bold().yellow(str);
  },
  success(str: string | number) {
    return kleur.bold().green(str);
  },
  error(str: string | number) {
    return kleur.bold().red(str);
  },
  dim(str: string | number) {
    return kleur.dim(str);
  },
};

export const c = colors;
