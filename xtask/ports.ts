import type { Repository } from 'es-git';
import { runCommand } from './child_process.ts';
import { GIT_SIGNATURE } from './consts.ts';
import { createGitHubPort } from './github.ts';
import { registryOfManifest } from './registry.ts';
import type { VersionedFileType } from './versioned-file.ts';

/**
 * The side-effect boundary of the release pipeline. Everything that touches the outside world —
 * child processes, the package registries, git effects, the GitHub API — goes through a port, so
 * the planning/apply logic can be exercised in tests with in-memory fakes. Local git *reads*
 * (history, trees, tags) intentionally stay on the es-git `Repository` directly.
 */
export interface Ports {
  proc: ProcPort;
  registry: RegistryPort;
  /** `null` outside a command that opened the repository (git effects unavailable). */
  git: GitPort | null;
  /** `null` when running without a GitHub token: GitHub/push effects are logged, not executed. */
  github: GitHubPort | null;
}

export interface ProcPort {
  run(
    cmd: string,
    args: string[],
    opts: { cwd: string; prefix?: string }
  ): Promise<{ exitCode: number | undefined; output: string }>;
}

export interface RegistryPort {
  /** Whether `name@version` is already live in the manifest type's registry (`null` = unknown). */
  exists(type: VersionedFileType, name: string, version: string): Promise<boolean | null>;
}

export interface GitPort {
  /** Create an annotated tag named `name` at `HEAD`; returns the created tag name. */
  createTag(name: string): string;
  /** Push tag refspecs to `origin`. */
  pushTags(refspecs: string[]): Promise<void>;
}

export interface GitHubRelease {
  id: number;
  htmlUrl: string;
}

export interface GitHubReleaseAsset {
  id: number;
  name: string;
  state: string;
}

export interface AssetFile {
  /** Absolute path to the file on disk. */
  path: string;
  /** Name the asset is uploaded under. */
  name: string;
}

export interface GitHubPort {
  findReleaseByTag(tag: string): Promise<GitHubRelease | null>;
  createRelease(params: {
    tag: string;
    name: string;
    body?: string;
    prerelease?: boolean;
    targetCommitish?: string;
  }): Promise<GitHubRelease>;
  updateReleaseBody(releaseId: number, body: string): Promise<void>;
  listReleaseAssets(releaseId: number): Promise<GitHubReleaseAsset[]>;
  deleteReleaseAsset(assetId: number): Promise<void>;
  uploadReleaseAsset(releaseId: number, asset: AssetFile): Promise<void>;
}

const procPort: ProcPort = {
  run(cmd, args, opts) {
    return runCommand(cmd, args, { cwd: opts.cwd, prefix: opts.prefix });
  },
};

const registryPort: RegistryPort = {
  exists(type, name, version) {
    return registryOfManifest(type).exists(name, version);
  },
};

function createGitPort(repo: Repository, githubToken: string | null): GitPort {
  return {
    createTag(name) {
      const head = repo.head().target();
      if (head == null) {
        throw new Error('cannot resolve git `HEAD`');
      }
      const commit = repo.getCommit(head);
      const tagId = repo.createTag(name, commit.asObject(), name, { tagger: GIT_SIGNATURE });
      return repo.getTag(tagId).name() ?? name;
    },
    async pushTags(refspecs) {
      if (githubToken == null) {
        throw new Error('a github token is required to push tags');
      }
      const remote = repo.getRemote('origin');
      await remote.push(refspecs, { credential: { type: 'Plain', password: githubToken } });
    },
  };
}

/** Real ports for a command run. `repo`/`githubToken` gate the git/GitHub effects. */
export function createPorts(opts: { repo?: Repository; githubToken?: string } = {}): Ports {
  return {
    proc: procPort,
    registry: registryPort,
    git: opts.repo != null ? createGitPort(opts.repo, opts.githubToken ?? null) : null,
    github: opts.githubToken != null ? createGitHubPort(opts.githubToken) : null,
  };
}

/** Ports for plain write/command actions (no git/GitHub effects). */
export const defaultPorts: Ports = {
  proc: procPort,
  registry: registryPort,
  git: null,
  github: null,
};
