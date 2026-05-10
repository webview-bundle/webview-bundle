import type { BuiltinConfig } from './builtin.js';
import type { PackConfig } from './pack.js';
import type { RemoteConfig } from './remote/index.js';
import type { ServeConfig } from './serve.js';

export type ConfigInputFnObj = () => Config;
export type ConfigInputFnPromise = () => Promise<Config>;
export type ConfigInputFn = () => Config | Promise<Config>;

export type ConfigInput =
  | Config
  | Promise<Config>
  | ConfigInputFnObj
  | ConfigInputFnPromise
  | ConfigInputFn;

export function defineConfig(config: Config): Config;
export function defineConfig(config: Promise<Config>): Promise<Config>;
export function defineConfig(config: ConfigInputFnObj): ConfigInputFnObj;
export function defineConfig(config: ConfigInputFnPromise): ConfigInputFnPromise;
export function defineConfig(config: ConfigInputFn): ConfigInputFn;
export function defineConfig(config: ConfigInput): ConfigInput;
export function defineConfig(config: ConfigInput): ConfigInput {
  return config;
}

export interface Config {
  /**
   * Project root directory. Can be an absolute path, or a path relative
   * from the current file.
   * @default process.cwd()
   */
  root?: string;
  pack?: PackConfig;
  remote?: RemoteConfig;
  serve?: ServeConfig;
  builtin?: BuiltinConfig;
}
