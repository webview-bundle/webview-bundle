/** biome-ignore-all lint/complexity/noBannedTypes: expected */
import type { Context, Env, Hono, Input, Schema } from 'hono';
import type { BlankEnv, BlankSchema, HTTPResponseError } from 'hono/types';
import type { Exception } from './exception.js';
import type { Update } from './schema.js';

export type SignaturePolicy = 'strict' | 'optional' | 'off';

export interface ExceptionContext {
  request: Request;
  exception: Exception;
}

export interface ErrorContext {
  request: Request;
  error: Error | HTTPResponseError;
}

export interface GetUpdateParams<
  E extends Env = any,
  P extends string = any,
  I extends Input = {},
> {
  context: Context<E, P, I>;
  runtimeVersion: number;
  channel?: string;
}

export interface DownloadParams<E extends Env = any, P extends string = any, I extends Input = {}> {
  context: Context<E, P, I>;
  bundleName: string;
  version: string;
}

export interface Config<
  E extends Env = BlankEnv,
  S extends Schema = BlankSchema,
  BasePath extends string = '/',
> {
  getUpdate: (params: GetUpdateParams<E>) => Promise<Update | null>;
  download: (params: DownloadParams<E>) => Promise<Response | undefined>;
  app?: Hono<E, S, BasePath>;
  /**
   * @default 'optional'
   */
  signaturePolicy?: SignaturePolicy;
  disableIfNoneMatch?: boolean;
  onException?: (ctx: ExceptionContext) => Response | undefined;
  onError?: (ctx: ErrorContext) => Response | undefined;
}
