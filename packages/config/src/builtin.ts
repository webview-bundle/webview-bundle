import type { HttpOptions } from '@wvb/node';
import type { BundleNameResolver, VersionResolver } from './common.js';
import type { IntegrityMakeConfig, RemoteConfig, SignatureSignConfig } from './remote/index.js';

export type BuiltinBundleMatches =
  | string
  | RegExp
  | Array<string | RegExp>
  | ((info: { name: string; version: string }) => boolean | Promise<boolean>);

export interface BuiltinDownloadConfig {
  /**
   * Concurrency of the download bundles.
   */
  concurrency?: number;
  http?: HttpOptions;
}

/**
 * Install builtin bundles from a remote target.
 */
export interface BuiltinRemoteTargetConfig extends Pick<RemoteConfig, 'endpoint'> {
  download?: BuiltinDownloadConfig;
}

/**
 * Install builtin bundles from a local target.
 */
export interface BuiltinLocalTargetConfig {
  /**
   * Workspace directory path to install builtin bundles.
   * Glob pattern is supported.
   *
   * Automatically resolve the workspace directory which includes a config file inside.
   * (e.g. wvb.config.ts)
   */
  workspaces: string[] | (() => string[] | Promise<string[]>);
  bundleName?: BundleNameResolver;
  version?: VersionResolver;
  integrity?: boolean | IntegrityMakeConfig;
  signature?: SignatureSignConfig;
  /**
   * Whether to pack the bundle before installing.
   * This option is used when the workspace detected.
   * @default true
   */
  packBeforeInstall?: boolean;
}

export type BuiltinTarget =
  | ({ type: 'remote' } & BuiltinRemoteTargetConfig)
  | ({ type: 'local' } & BuiltinLocalTargetConfig);

export interface BuiltinConfig {
  /**
   * Directory path where to download builtin bundles from target.
   * @default ".wvb/builtin/bundles"
   */
  outDir?: string;
  /**
   * Target to install builtin bundles.
   * @default { type: "remote" }
   */
  target?: BuiltinTarget;
  /**
   * Patterns to which bundles should be included from target bundles.
   */
  include?: BuiltinBundleMatches;
  /**
   * Patterns to which bundles should be excluded from target bundles.
   */
  exclude?: BuiltinBundleMatches;
  /**
   * Clean up builtin directory before the operation.
   * @default true
   */
  clean?: boolean;
}
