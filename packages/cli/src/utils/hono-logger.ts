import type { MiddlewareHandler } from 'hono';

function colorStatus(status: number): string {
  switch ((status / 100) | 0) {
    case 5: // red = error
      return `\x1b[31m${status}\x1b[0m`;
    case 4: // yellow = warning
      return `\x1b[33m${status}\x1b[0m`;
    case 3: // cyan = redirect
      return `\x1b[36m${status}\x1b[0m`;
    case 2: // green = success
      return `\x1b[32m${status}\x1b[0m`;
    default:
      return `${status}`;
  }
}

function humanize(times: string[]): string {
  const [delimiter, separator] = [',', '.'];
  const orderTimes = times.map(v => v.replace(/(\d)(?=(\d\d\d)+(?!\d))/g, `$1${delimiter}`));
  return orderTimes.join(separator);
}

function time(start: number) {
  const delta = Date.now() - start;
  return humanize([delta < 1000 ? `${delta}ms` : `${Math.round(delta / 1000)}s`]);
}

type LogType = 'incoming' | 'outgoing';
function logTypePrefix(type: LogType): string {
  switch (type) {
    case 'incoming':
      return '<--';
    case 'outgoing':
      return '-->';
  }
}

type PrintFunc = (str: string, ...rest: string[]) => void;

function log(
  fn: PrintFunc,
  type: LogType,
  method: string,
  path: string,
  status: number = 0,
  colorEnabled = false,
  elapsed?: string
): void {
  const prefix = logTypePrefix(type);
  const out =
    type === 'incoming'
      ? `${prefix} ${method} ${path}`
      : `${prefix} ${method} ${path} ${colorEnabled ? colorStatus(status) : status} ${elapsed}`;
  fn(out);
}

export const logMiddleware = (fn: PrintFunc, colorEnabled?: boolean): MiddlewareHandler => {
  return async function logger(c, next) {
    const { method, url } = c.req;
    const path = url.slice(url.indexOf('/', 8));
    log(fn, 'incoming', method, path, 0, colorEnabled);
    const start = Date.now();
    await next();
    log(fn, 'outgoing', method, path, c.res.status, colorEnabled, time(start));
  };
};
