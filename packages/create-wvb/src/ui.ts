import util from 'node:util';
import kleur from 'kleur';

export function configureColor(mode: 'on' | 'off' | 'auto'): void {
  switch (mode) {
    case 'on':
      kleur.enabled = true;
      break;
    case 'off':
      kleur.enabled = false;
      break;
    case 'auto':
      kleur.enabled =
        process.env.NO_COLOR == null &&
        process.env.TERM !== 'dumb' &&
        process.stdout.isTTY === true;
      break;
  }
}

export const colors = {
  title: (msg: string) => kleur.bold().cyan(msg),
  muted: (msg: string) => kleur.gray(msg),
  accent: (msg: string) => kleur.cyan(msg),
  success: (msg: string) => kleur.green(msg),
  warn: (msg: string) => kleur.yellow(msg),
  error: (msg: string) => kleur.red(msg),
  bold: (msg: string) => kleur.bold(msg),
  code: (msg: string) => kleur.bold().white(msg),
  underline: (msg: string) => kleur.underline(msg),
};
export const c = colors;

const symbols = {
  bullet: '•',
  check: '✔',
  cross: '✖',
  warn: '▲',
  arrow: '→',
  add: '+',
};

export function stripColor(message: string): string {
  return util.stripVTControlCharacters(message);
}

function write(line = ''): void {
  process.stdout.write(`${line}\n`);
}

export function blank(): void {
  write();
}

export function intro(version: string): void {
  blank();
  write(`  ${c.title('create-wvb')} ${c.muted(version)}`);
  write(`  ${c.muted('Offline-first web apps that run inside native webviews.')}`);
  blank();
}

export function heading(text: string): void {
  blank();
  write(`  ${c.bold(text)}`);
  blank();
}

export function step(text: string, detail?: string): void {
  const suffix = detail == null ? '' : ` ${c.muted(detail)}`;
  write(`  ${c.success(symbols.check)} ${text}${suffix}`);
}

export function added(path: string, detail?: string): void {
  const suffix = detail == null ? '' : ` ${c.muted(detail)}`;
  write(`    ${c.muted(symbols.add)} ${path}${suffix}`);
}

export function info(text: string): void {
  write(`  ${c.muted(symbols.bullet)} ${c.muted(text)}`);
}

export function warn(text: string): void {
  write(`  ${c.warn(symbols.warn)} ${text}`);
}

export function error(text: string): void {
  process.stderr.write(`  ${c.error(symbols.cross)} ${text}\n`);
}

export function note(lines: readonly string[]): void {
  for (const line of lines) {
    write(`    ${c.muted(symbols.bullet)} ${line}`);
  }
}

export function callout(title: string, lines: readonly string[], tone: 'warn' | 'muted'): void {
  const paint = tone === 'warn' ? c.warn : c.muted;
  blank();
  write(`  ${paint(`${symbols.warn} ${title}`)}`);
  blank();
  for (const line of lines) {
    write(`    ${c.muted(symbols.bullet)} ${line}`);
  }
  blank();
}

export function command(text: string): void {
  write(`    ${c.code(text)}`);
}

export function link(label: string, url: string): void {
  write(`  ${c.muted(label)}  ${c.accent(c.underline(url))}`);
}

export function rule(): void {
  blank();
  write(c.muted('  ─────────────────────────────────────────────────────────'));
  blank();
}

export function outro(name: string, description: string, elapsedMs: number): void {
  const seconds = (elapsedMs / 1000).toFixed(1);
  write(
    `  ${c.success(symbols.check)} Created ${c.bold(name)} ${c.muted(`(${description})`)} ${c.muted(`in ${seconds}s`)}`
  );
}
