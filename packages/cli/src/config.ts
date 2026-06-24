import fs from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import type {
  BundleInfoResolverParams,
  BundleNameResolver,
  Config,
  ConfigInput,
  PackageJson,
  VersionResolver,
} from '@wvb/config';
import { DEFAULT_CONFIG_FILES } from './constants.js';
import {
  findNearestPackageJson,
  findNearestPackageJsonFilePath,
  isEsmFile,
  pathExists,
} from './fs.js';
import { isNodeBuiltin } from './module.js';

interface NodeModuleWithCompile extends NodeJS.Module {
  _compile(code: string, filename: string): any;
}

export async function loadConfigFile(
  filePath?: string,
  cwd: string = process.cwd()
): Promise<{ config: Config; configFile: string; configFileDependencies: string[] } | null> {
  let resolvedFilePath: string | undefined;
  if (filePath != null) {
    resolvedFilePath = path.isAbsolute(filePath) ? filePath : path.resolve(cwd, filePath);
  } else {
    for (const file of DEFAULT_CONFIG_FILES) {
      const candidateFile = path.resolve(cwd, file);
      if (!(await pathExists(candidateFile))) {
        continue;
      }
      resolvedFilePath = candidateFile;
      break;
    }
  }

  if (resolvedFilePath == null) {
    return null;
  }

  const isDeno = typeof process.versions.deno === 'string';
  const isEsm = isDeno ? true : await isEsmFile(resolvedFilePath);
  const bundle = await bundleConfigFile(resolvedFilePath, isEsm, cwd);

  if (isEsm) {
    const pkgJsonFilePath = await findNearestPackageJsonFilePath(cwd);
    let tmpDir: string | null =
      pkgJsonFilePath != null
        ? path.join(path.dirname(pkgJsonFilePath), '.wvb')
        : path.join(cwd, '.wvb');
    try {
      await fs.mkdir(tmpDir, { recursive: true });
    } catch (e: any) {
      if (e.code === 'EACCES') {
        tmpDir = null;
      } else {
        throw e;
      }
    }
    const fileName = path.basename(resolvedFilePath);
    const hash = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
    const tmpFilePath =
      tmpDir != null
        ? path.join(tmpDir, `${fileName}.${hash}.mjs`)
        : `${resolvedFilePath}.${hash}.mjs`;
    await fs.writeFile(tmpFilePath, bundle.code);
    try {
      const imported = await import(pathToFileURL(tmpFilePath).href);
      const config = await normalizeConfigInput(imported.default);
      return {
        config,
        configFile: resolvedFilePath,
        configFileDependencies: bundle.dependencies,
      };
    } finally {
      await fs.unlink(tmpFilePath).catch(() => {});
    }
  }

  const _require = createRequire(import.meta.url);
  const ext = path.extname(resolvedFilePath);
  const realFilePath = await fs.realpath(resolvedFilePath);
  const loaderExt = ext in _require.extensions ? ext : '.js';
  const defaultLoader = _require.extensions[loaderExt]!;
  _require.extensions[loaderExt] = (module: NodeJS.Module, filePath: string) => {
    if (filePath === realFilePath) {
      (module as NodeModuleWithCompile)._compile(bundle.code, filePath);
    } else {
      defaultLoader(module, filePath);
    }
  };
  const raw = _require(resolvedFilePath);
  _require.extensions[loaderExt] = defaultLoader;

  const config = await normalizeConfigInput((raw.__esModule as boolean) ? raw.default : raw);
  return {
    config,
    configFile: resolvedFilePath,
    configFileDependencies: bundle.dependencies,
  };
}

async function normalizeConfigInput(input: ConfigInput | undefined): Promise<Config> {
  const value = typeof input === 'function' ? input() : input;
  return (await value) ?? {};
}

async function bundleConfigFile(
  absoluteFilepath: string,
  isEsm: boolean,
  cwd?: string
): Promise<{
  code: string;
  dependencies: string[];
}> {
  const dirname = path.dirname(absoluteFilepath);
  const filename = absoluteFilepath;
  const { rolldown } = await import('rolldown');
  const bundle = await rolldown({
    input: absoluteFilepath,
    platform: 'node',
    external: id => {
      if (
        !id ||
        id.startsWith('\0') ||
        id.startsWith('.') ||
        id.startsWith('#') ||
        path.isAbsolute(id)
      ) {
        return false;
      }
      if (isNodeBuiltin(id) || id.startsWith('npm:')) {
        return true;
      }
      return true;
    },
    transform: {
      target: `node${process.versions.node}`,
      define: {
        __dirname: JSON.stringify(dirname),
        __filename: JSON.stringify(filename),
        'import.meta.url': JSON.stringify(pathToFileURL(filename).href),
        'import.meta.filename': JSON.stringify(filename),
        'import.meta.dirname': JSON.stringify(dirname),
        'import.meta.env': 'process.env',
      },
    },
    cwd,
  });

  const bundleOutput = await bundle.generate({
    format: isEsm ? 'esm' : 'cjs',
    sourcemap: 'inline',
    keepNames: true,
    inlineDynamicImports: true,
  });
  if (bundleOutput.output[0] == null || bundleOutput.output[0].type !== 'chunk') {
    throw new Error('no output chunk found');
  }
  const dependencies = await bundle.watchFiles;
  return {
    code: bundleOutput.output[0].code,
    dependencies,
  };
}

export interface InlineConfig extends Config {
  configFile?: string | false;
}

export interface ResolvedConfig
  extends Readonly<
    Omit<Config, 'root'> & {
      root: string;
      configFile: string | undefined;
      configFileDependencies: string[] | undefined;
      inlineConfig: InlineConfig;
      packageJson: PackageJson | undefined;
    }
  > {}

export async function resolveConfig(inlineConfig: InlineConfig): Promise<ResolvedConfig> {
  let config = inlineConfig;
  let configFileDependencies: string[] = [];
  let { configFile } = config;
  if (config.configFile !== false) {
    const loaded = await loadConfigFile(config.configFile, config.root);
    if (loaded != null) {
      config = { ...loaded.config, ...config };
      configFile = loaded.configFile;
      configFileDependencies = loaded.configFileDependencies;
    }
  }
  const root = config.root ?? process.cwd();
  const packageJson = await findNearestPackageJson(root);
  const resolved: ResolvedConfig = {
    ...config,
    root,
    configFile: configFile != null && configFile !== false ? configFile : undefined,
    configFileDependencies,
    inlineConfig,
    packageJson,
  };
  return resolved;
}

/**
 * Resolve the output path for the packed Webview Bundle.
 *
 * Returns the configured `pack.outFile` as-is, otherwise falls back to
 * ".wvb/<name>" derived from "package.json". The path is relative to the config
 * root (unless absolute) and has no ".wvb" extension forced here — callers apply
 * `withWvbExtension`/`toAbsolutePath` as needed.
 */
export function resolveOutFile(config: ResolvedConfig): string | undefined {
  if (config.pack?.outFile != null) {
    return config.pack.outFile;
  }
  const pkgName = config.packageJson?.name;
  if (pkgName != null) {
    return path.join('.wvb', getDefaultBundleName(pkgName));
  }
  return undefined;
}

function getDefaultBundleName(pkgName: string): string {
  const name = pkgName.includes('/') ? pkgName.split('/')[1]! : pkgName;
  return name;
}

export async function resolveVersion(
  config: ResolvedConfig,
  version: VersionResolver | undefined,
  params?: Partial<BundleInfoResolverParams>
): Promise<string | undefined> {
  if (typeof version === 'function') {
    return version({
      packageJson: config.packageJson,
      dir: config.root,
      ...params,
    });
  }
  if (typeof version === 'string') {
    return version;
  }
  return config.packageJson?.version;
}

export async function resolveBundleName(
  config: ResolvedConfig,
  bundleName: BundleNameResolver | undefined,
  params?: Partial<BundleInfoResolverParams>
): Promise<string | undefined> {
  if (typeof bundleName === 'function') {
    return bundleName({
      packageJson: config.packageJson,
      dir: config.root,
      ...params,
    });
  }
  if (typeof bundleName === 'string') {
    return bundleName;
  }
  const pkgName = config.packageJson?.name;
  if (pkgName != null) {
    return getDefaultBundleName(pkgName);
  }
  return undefined;
}
