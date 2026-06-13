import type { PackageJson } from 'type-fest';

export interface BundleInfoResolverParams {
  /**
   * Resolved "package.json" file data from the package.
   */
  packageJson?: PackageJson;
  /**
   * Absolute path to the package directory.
   */
  dir?: string;
  /**
   * Absolute path to the bundle file.
   */
  file?: string;
}

/**
 * Version resolver to determine the version of the bundle to be used in remote or builtin.
 *
 * From "package.json", will use the "version" field from "package.json",
 * if not specified, will be throw error.
 *
 * From "git", will use the "HEAD" commit hash.
 *
 * Or, specify a custom version string or a function that returns a version string.
 *
 * @default { from: 'package.json' }
 */
export type VersionResolver =
  | { from: 'package.json' }
  | { from: 'git' }
  | string
  | ((params: BundleInfoResolverParams) => string | Promise<string>);

/**
 * Bundle name resolver to determine the name of the bundle to be used in remote or builtin.
 *
 * From "package.json", will use the "name" field from "package.json",
 * if not specified, will be throw error.
 * If "name" is with a scope prefix, it will be removed.
 *
 * Or, specify a custom bundle name string or a function that returns a bundle name string.
 *
 * @default { from: 'package.json' }
 */
export type BundleNameResolver =
  | { from: 'package.json' }
  | string
  | ((params: BundleInfoResolverParams) => string | Promise<string>);
