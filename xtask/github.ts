import fs from 'node:fs/promises';
import path from 'node:path';
import { retry } from '@octokit/plugin-retry';
import { Octokit } from '@octokit/rest';
import { execa } from 'execa';
import mime from 'mime-types';
import { glob } from 'tinyglobby';
import { c } from './console.ts';
import { GITHUB_REPO, ROOT_DIR } from './consts.ts';
import type { Package } from './package.ts';

export type GitHubClient = Octokit;

export function createGitHubClient(token: string): GitHubClient {
  const Client = Octokit.plugin(retry);
  return new Client({ auth: token, userAgent: 'webview-bundle' });
}

export interface AssetFile {
  /** Absolute path to the file on disk. */
  path: string;
  /** Name the asset is uploaded under. */
  name: string;
}

/** Resolve a package's configured `assets` globs to concrete files on disk. */
export async function resolveAssets(pkg: Package): Promise<AssetFile[]> {
  if (pkg.assets.length === 0) {
    return [];
  }
  const files = await glob([...pkg.assets], { cwd: pkg.absolutePath, onlyFiles: true });
  return files.map(file => ({
    path: path.join(pkg.absolutePath, file),
    name: path.basename(file),
  }));
}

/** Upload files to a GitHub release; returns the uploaded file names. */
export async function uploadReleaseAssets(
  client: GitHubClient,
  releaseId: number,
  assets: AssetFile[]
): Promise<string[]> {
  const uploaded: string[] = [];
  for (const asset of assets) {
    const data = await fs.readFile(asset.path);
    await client.rest.repos.uploadReleaseAsset({
      owner: GITHUB_REPO.owner,
      repo: GITHUB_REPO.name,
      release_id: releaseId,
      name: asset.name,
      data: data as unknown as string,
      headers: {
        'content-type': mime.lookup(asset.name) || 'application/octet-stream',
        'content-length': String(data.byteLength),
      },
    });
    console.log(`  ${c.dim(`asset: ${asset.name}`)}`);
    uploaded.push(asset.name);
  }
  return uploaded;
}

interface GhResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

/** Run a `gh` subcommand and capture its output. Never rejects — inspect `exitCode`. */
async function runGh(args: string[]): Promise<GhResult> {
  const { exitCode, stdout, stderr } = await (execa as any)('gh', args, {
    cwd: ROOT_DIR,
    reject: false,
  });
  return {
    exitCode: typeof exitCode === 'number' ? exitCode : 1,
    stdout: typeof stdout === 'string' ? stdout : '',
    stderr: typeof stderr === 'string' ? stderr : '',
  };
}

/** Run a `gh` subcommand expecting success; returns stdout, throws on a non-zero exit. */
async function ghText(args: string[]): Promise<string> {
  const result = await runGh(args);
  if (result.exitCode !== 0) {
    throw new Error(`gh ${args.slice(0, 2).join(' ')} failed: ${result.stderr.trim()}`);
  }
  return result.stdout.trim();
}

export interface CreatePullRequestOptions {
  base: string;
  head: string;
  title: string;
  body: string;
  draft: boolean;
}

/** The number of the open PR for `head`, or `null` if there is none. */
export async function findOpenPullRequest(head: string): Promise<number | null> {
  const result = await runGh(['pr', 'list', '--head', head, '--state', 'open', '--json', 'number']);
  if (result.exitCode !== 0 || result.stdout.trim().length === 0) {
    return null;
  }
  try {
    return (JSON.parse(result.stdout) as Array<{ number: number }>)[0]?.number ?? null;
  } catch {
    return null;
  }
}

/** Open a pull request; returns its URL. */
export async function createPullRequest(opts: CreatePullRequestOptions): Promise<string> {
  const args = [
    'pr',
    'create',
    '--base',
    opts.base,
    '--head',
    opts.head,
    '--title',
    opts.title,
    '--body',
    opts.body,
  ];
  if (opts.draft) {
    args.push('--draft');
  }
  return ghText(args);
}

/** Update an existing pull request's title and body. */
export async function updatePullRequest(
  number: number,
  opts: { title: string; body: string }
): Promise<void> {
  await ghText(['pr', 'edit', String(number), '--title', opts.title, '--body', opts.body]);
}
