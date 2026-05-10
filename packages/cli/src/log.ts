import util from 'node:util';
import {
  type LogLevel as AllLogLevel,
  configure,
  getConsoleSink,
  getLogger as getLoggerApi,
  type Logger as LoggerType,
  type LogRecord,
} from '@logtape/logtape';
import { Option } from 'clipanion';
import { isBoolean, isEnum } from 'typanion';
import { c } from './console.js';

const LOG_LEVELS = ['debug', 'info', 'warning', 'error'] as const;
export type LogLevel = (typeof LOG_LEVELS)[number];

export function compareLogLevel(source: LogLevel, target: LogLevel): -1 | 0 | 1 {
  const sourceLevel = LOG_LEVELS.indexOf(source);
  const targetLevel = LOG_LEVELS.indexOf(target);
  if (sourceLevel === targetLevel) {
    return 0;
  }
  return sourceLevel < targetLevel ? -1 : 1;
}

export function isLogLevelAtLeast(source: LogLevel, target: LogLevel): boolean {
  return compareLogLevel(source, target) <= 0;
}

export const LogLevelOption = Option.String('--log-level', 'info', {
  description: 'Set the log level for output. [Default: "info"]',
  validator: isEnum(LOG_LEVELS),
  env: 'LOG_LEVEL',
});

export const LogVerboseOption = Option.String('--log-verbose', false, {
  tolerateBoolean: true,
  validator: isBoolean(),
  description: 'Enable verbose logging. [Default: false]',
});

export interface ConfigureLoggerOptions {
  /** @default 'debug' */
  level?: LogLevel;
  /** @default false */
  verbose?: boolean;
}

function levelColor(level: AllLogLevel, message: string): string {
  switch (level) {
    case 'debug':
    case 'trace':
      return c.debug(message);
    case 'info':
      return c.bold(c.info(message));
    case 'warning':
      return c.bold(c.warn(message));
    case 'error':
    case 'fatal':
      return c.bold(c.error(message));
  }
}

const levelAbbreviations: Record<AllLogLevel, string> = {
  trace: 'TRC',
  debug: 'DBG',
  info: 'INF',
  warning: 'WRN',
  error: 'ERR',
  fatal: 'FTL',
};

const padZero = (n: number): string => (n < 10 ? `0${n}` : `${n}`);
const padThree = (n: number): string => (n < 10 ? `00${n}` : n < 100 ? `0${n}` : `${n}`);

function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  const yyyy = date.getUTCFullYear();
  const MM = padZero(date.getUTCMonth() + 1);
  const dd = padZero(date.getUTCDate());
  const hh = padZero(date.getUTCHours());
  const mm = padZero(date.getUTCMinutes());
  const sss = padThree(date.getUTCSeconds());
  return `${yyyy}-${MM}-${dd} ${hh}:${mm}:${sss} +00:00`;
}

function formatCategory(category: readonly string[]): string {
  return category.join('/');
}

function formatValue(value: unknown, verbose: boolean): string {
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'object' && value != null) {
    if (value instanceof Error) {
      if (verbose && value.stack != null) {
        return c.bold(c.error(`\n${value.stack}\n`));
      }
      return c.bold(c.error(`${value.name}: ${value.message}`));
    }
    return c.bold(c.debug(util.inspect('%O', value)));
  }
  return c.bold(String(value));
}

export async function configureLogger(options?: ConfigureLoggerOptions) {
  const { level = 'debug', verbose = false } = options ?? {};
  await configure({
    sinks: {
      console: getConsoleSink({
        formatter(record: LogRecord): readonly unknown[] {
          const { category, message, level, timestamp } = record;
          const levelMsg = levelColor(record.level, `[${levelAbbreviations[level]}]`);
          let prefixMsg = levelMsg;
          if (verbose) {
            const timestampMsg = levelColor(record.level, `[${formatTimestamp(timestamp)}]`);
            prefixMsg = `${timestampMsg}${prefixMsg}`;

            const categoryMsg = levelColor(record.level, `[${formatCategory(category)}]`);
            prefixMsg = `${prefixMsg}${categoryMsg}`;
          }
          const formattedMsg = message.map(x => formatValue(x, verbose)).join('');
          return [prefixMsg, formattedMsg];
        },
      }),
    },
    loggers: [
      {
        category: ['wvb'],
        sinks: ['console'],
        lowestLevel: level,
      },
      {
        category: ['logtape'],
        sinks: ['console'],
        lowestLevel: 'error',
      },
    ],
  });
}

export type Logger = LoggerType;

export function getLogger(...categories: string[]): Logger {
  return getLoggerApi(['wvb', ...categories]);
}
